//! Per-provider quota collection. First-party or direct provider paths run independently;
//! OMP is a per-card compatibility fallback, never the primary source.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::model::{self, Card, ProviderId, QuotaSource, Report, Snapshot};

const TIMEOUT: Duration = Duration::from_secs(12);

fn omp_candidates() -> Vec<String> {
    let mut v = vec!["omp".to_string()];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        v.push(format!("{local}\\omp\\omp.exe"));
    }
    v
}

/// Fetch all providers independently. One failed source cannot erase other live cards.
pub fn fetch_snapshot() -> anyhow::Result<Snapshot> {
    let (codex, go, goat, cpa) = std::thread::scope(|scope| {
        let codex = scope.spawn(|| {
            crate::codex_app_server::fetch_report().or_else(|_| codex_native_quota_report())
        });
        let go = scope.spawn(crate::opencode_go::fetch_report);
        let goat = scope.spawn(crate::goat::fetch_report);
        let cpa = scope.spawn(cpa_antigravity_card);
        (codex.join(), go.join(), goat.join(), cpa.join())
    });

    let mut cards = model::ProviderId::all().map(|provider| Card::Missing { provider });
    if let Ok(Ok(report)) = codex {
        cards[1] = Card::Live(report);
    }
    if let Ok(Ok(report)) = go {
        cards[0] = Card::Live(report);
    }
    if let Ok(Ok(report)) = goat {
        cards[2] = Card::Live(report);
    }
    if let Ok(Some(card)) = cpa {
        cards[3] = card;
    }

    if (matches!(cards[0], Card::Missing { .. }) || matches!(cards[1], Card::Missing { .. }))
        && let Ok(fallback) = omp_snapshot()
    {
        merge_missing(&mut cards, fallback.cards);
    }

    if cards
        .iter()
        .all(|card| matches!(card, Card::Missing { .. }))
    {
        anyhow::bail!("额度来源不可用")
    }
    Ok(Snapshot {
        generated_at_ms: now_ms(),
        cards: cards.into_iter().collect(),
    })
}

fn merge_missing(cards: &mut [Card; 4], fallback: Vec<Card>) {
    for card in fallback {
        let Card::Live(report) = card else { continue };
        if let Some(index) = ProviderId::all()
            .iter()
            .position(|provider| *provider == report.provider)
            && matches!(cards[index], Card::Missing { .. })
        {
            cards[index] = Card::Live(report);
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn omp_snapshot() -> anyhow::Result<Snapshot> {
    for exe in omp_candidates() {
        match run_omp(&exe) {
            Ok(json) => return crate::model::parse_snapshot(&json),
            Err(e) => {
                if let Some(io) = e.downcast_ref::<std::io::Error>()
                    && io.kind() == std::io::ErrorKind::NotFound
                {
                    continue;
                }
                return Err(e);
            }
        }
    }
    anyhow::bail!("OMP compatibility fallback unavailable")
}

// ---------------- Antigravity (CPA) 卡 ----------------

const CPA_BASE: &str = "http://127.0.0.1:8317";
const CPA_TIMEOUT: Duration = Duration::from_secs(6);
const CPA_QUOTA_URLS: [&str; 3] = [
    "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:retrieveUserQuotaSummary",
    "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
];
const CPA_TIER_URL: &str = "https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";

#[derive(Debug)]
struct CpaAuth {
    auth_index: String,
    project_id: Option<String>,
    disabled: bool,
    failed: i64,
}

fn cpa_key() -> Option<String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    std::fs::read_to_string(
        std::path::PathBuf::from(home)
            .join(".tui-token-monitor")
            .join("cpa-key"),
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// 只提取配额查询必需的元数据；不读取或保存账号凭据。
fn parse_auth_files(json: &str) -> Option<Vec<CpaAuth>> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let files = v.get("files")?.as_array()?;
    let mut accounts = Vec::new();
    for f in files {
        let id = f.get("id").and_then(|x| x.as_str()).unwrap_or("");
        let ty = f.get("account_type").and_then(|x| x.as_str()).unwrap_or("");
        if id.starts_with("antigravity-")
            || ty == "antigravity"
            || f.get("provider").and_then(|x| x.as_str()) == Some("antigravity")
        {
            let auth_index = f
                .get("auth_index")
                .and_then(|x| match x {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .unwrap_or_default();
            let project_id = [
                "/project_id",
                "/projectId",
                "/metadata/project_id",
                "/metadata/projectId",
                "/attributes/project_id",
                "/attributes/projectId",
                "/attributes/gemini_virtual_project",
            ]
            .iter()
            .find_map(|path| {
                f.pointer(path)
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
            })
            .map(str::to_string);
            accounts.push(CpaAuth {
                auth_index,
                project_id,
                disabled: f.get("disabled").and_then(|x| x.as_bool()).unwrap_or(false),
                failed: f.get("failed").and_then(|x| x.as_i64()).unwrap_or(0),
            });
        }
    }
    Some(accounts)
}

/// CPA 在服务端将 $TOKEN$ 替换为选中凭据的当前访问令牌。
/// 目标 URL 只由本模块的固定常量提供，管理 key 只发送给环回 CPA。
fn cpa_api_call(
    key: &str,
    auth_index: &str,
    url: &str,
    data: serde_json::Value,
) -> Option<serde_json::Value> {
    let payload = serde_json::json!({
        "auth_index": auth_index,
        "method": "POST",
        "url": url,
        "header": {
            "Authorization": "Bearer $TOKEN$",
            "Content-Type": "application/json",
            "User-Agent": "antigravity/cli/1.0.13 (aidev_client; os_type=darwin; arch=arm64)"
        },
        "data": data.to_string(),
    });
    let response = ureq::post(&format!("{CPA_BASE}/v0/management/api-call"))
        .set("Authorization", &format!("Bearer {key}"))
        .timeout(CPA_TIMEOUT)
        .send_json(payload)
        .ok()?;
    let mut raw = String::new();
    response
        .into_reader()
        .take(2_000_001)
        .read_to_string(&mut raw)
        .ok()?;
    if raw.len() > 2_000_000 {
        return None;
    }
    let envelope: serde_json::Value = serde_json::from_str(&raw).ok()?;
    if envelope.get("status_code").and_then(|x| x.as_u64()) != Some(200) {
        return None;
    }
    let body = envelope.get("body")?;
    if let Some(text) = body.as_str() {
        serde_json::from_str(text).ok()
    } else {
        Some(body.clone())
    }
}

fn parse_cpa_quota(
    v: &serde_json::Value,
    account: Option<usize>,
    now_ms: i64,
) -> Vec<model::Limit> {
    let mut limits = Vec::new();
    let Some(groups) = v.get("groups").and_then(|x| x.as_array()) else {
        return limits;
    };
    for group in groups {
        let group_name = group
            .get("displayName")
            .or_else(|| group.get("display_name"))
            .and_then(|x| x.as_str())
            .unwrap_or("Models");
        let group_label = if group_name.contains("Gemini") {
            "Gemini"
        } else if group_name.contains("Claude") || group_name.contains("GPT") {
            "Claude/GPT"
        } else {
            group_name
        };
        let Some(buckets) = group.get("buckets").and_then(|x| x.as_array()) else {
            continue;
        };
        for bucket in buckets {
            let Some(remaining) = bucket
                .get("remainingFraction")
                .or_else(|| bucket.get("remaining_fraction"))
                .and_then(|x| x.as_f64())
                .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
            else {
                continue;
            };
            let window = bucket.get("window").and_then(|x| x.as_str()).unwrap_or("");
            let window_label = match window {
                "weekly" => "7 days",
                "5h" => "5h",
                _ => continue,
            };
            let used_pct = (1.0 - remaining) * 100.0;
            let reset_at = bucket
                .get("resetTime")
                .or_else(|| bucket.get("reset_time"))
                .and_then(|x| x.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp_millis());
            let prefix = account.map(|n| format!("账号 {n} · ")).unwrap_or_default();
            limits.push(model::Limit {
                label: format!("{prefix}{group_label} · {window_label}"),
                used_pct,
                resets_in_ms: reset_at.map(|t| (t - now_ms).max(0)),
                status: model::status_for(used_pct),
            });
        }
    }
    limits
}

fn parse_cpa_tier(v: &serde_json::Value) -> Option<String> {
    let tier = ["paidTier", "paid_tier"]
        .iter()
        .find_map(|key| v.get(*key))
        .filter(|tier| {
            tier.get("id")
                .and_then(|x| x.as_str())
                .is_some_and(|s| !s.is_empty())
        })
        .or_else(|| {
            ["currentTier", "current_tier"]
                .iter()
                .find_map(|key| v.get(*key))
        })?;
    let id = tier.get("id").and_then(|x| x.as_str()).unwrap_or("");
    let label = match id {
        "free-tier" => "Google Free",
        "g1-pro-tier" => "Google AI Pro",
        "g1-ultra-tier" => "Google AI Ultra",
        "g1-ultra-lite-tier" => "Google AI Ultra Lite",
        _ => tier
            .get("name")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())?,
    };
    Some(label.to_string())
}

fn cpa_upstream_report(key: &str, accounts: &[CpaAuth]) -> Option<Report> {
    let eligible: Vec<_> = accounts
        .iter()
        .filter(|a| !a.disabled && !a.auth_index.is_empty() && a.project_id.is_some())
        .take(8)
        .collect();
    if eligible.is_empty() {
        return None;
    }
    let multi = accounts.len() > 1;
    let now = now_ms();
    let mut limits = Vec::new();
    let mut first_successful_index = None;
    let mut success_count = 0;
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = eligible
            .iter()
            .enumerate()
            .map(|(position, account)| {
                scope.spawn(move || {
                    let project = account.project_id.as_deref().unwrap_or("");
                    let quota = CPA_QUOTA_URLS.iter().find_map(|url| {
                        cpa_api_call(
                            key,
                            &account.auth_index,
                            url,
                            serde_json::json!({"project": project}),
                        )
                    });
                    (
                        position,
                        quota
                            .map(|v| parse_cpa_quota(&v, multi.then_some(position + 1), now))
                            .unwrap_or_default(),
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .collect::<Vec<_>>()
    });
    for (position, account_limits) in results {
        if !account_limits.is_empty() {
            if first_successful_index.is_none() {
                first_successful_index = Some(&eligible[position].auth_index);
            }
            success_count += 1;
            limits.extend(account_limits);
        }
    }
    if limits.is_empty() {
        return None;
    }
    let tier = first_successful_index
        .filter(|_| accounts.len() == 1)
        .and_then(|index| {
            cpa_api_call(
                key,
                index,
                CPA_TIER_URL,
                serde_json::json!({"metadata":{"ideType":"ANTIGRAVITY"}}),
            )
        })
        .and_then(|v| parse_cpa_tier(&v));
    let plan_type = match tier {
        Some(tier) => format!(
            "{tier} · CPA 代查上游 · {success_count}/{} 账号",
            accounts.len()
        ),
        None => format!(
            "CPA 代查 Google 配额 · {success_count}/{} 账号",
            accounts.len()
        ),
    };
    Some(Report {
        provider: ProviderId::Antigravity,
        source: QuotaSource::CpaGoogleQuota,
        plan_type,
        limits,
    })
}

/// 今日经 CPA(provider='cpa')的 token 合计,以及近 7 天日均值.
fn cpa_today_and_avg7(
    db: &std::path::Path,
    today_start_ms: i64,
    week_start_ms: i64,
) -> Option<(i64, f64)> {
    let con = rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .ok()?;
    let q = |since: i64| -> rusqlite::Result<i64> {
        con.query_row(
            "SELECT COALESCE(SUM(total_tokens),0) FROM messages \
             WHERE provider='cpa' AND timestamp >= ?1",
            [since],
            |r| r.get::<_, i64>(0),
        )
    };
    let today = q(today_start_ms).ok()?;
    let week = q(week_start_ms).ok()?;
    Some((today, week as f64 / 7.0))
}

/// Antigravity (CPA) 卡:优先经 CPA 代查上游真实配额；失败时回退凭证健康与本机活动。
pub fn cpa_antigravity_card() -> Option<model::Card> {
    let key = cpa_key()?;
    let resp = ureq::get(&format!("{CPA_BASE}/v0/management/auth-files"))
        .set("Authorization", &format!("Bearer {key}"))
        .timeout(CPA_TIMEOUT)
        .call()
        .ok()?;
    let mut body = String::new();
    resp.into_reader()
        .take(2_000_001)
        .read_to_string(&mut body)
        .ok()?;
    if body.len() > 2_000_000 {
        return None;
    }
    let accounts = parse_auth_files(&body)?;
    if accounts.is_empty() {
        return None;
    }
    if let Some(report) = cpa_upstream_report(&key, &accounts) {
        return Some(Card::Live(report));
    }
    let disabled = accounts.iter().all(|a| a.disabled);
    let failed = accounts.iter().map(|a| a.failed).sum::<i64>();

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .ok()?;
    let now = chrono::Local::now();
    let today_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|d| d.and_local_timezone(now.timezone()).single())
        .map(|d| d.timestamp_millis())
        .unwrap_or(0);
    let week_start_ms = today_start - 6 * 86_400_000;
    let activity = cpa_today_and_avg7(
        &home.join(".omp").join("stats.db"),
        today_start,
        week_start_ms,
    );
    // 重置点:下一个午夜
    let next_midnight = (now.date_naive() + chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .and_then(|d| d.and_local_timezone(now.timezone()).single())
        .map(|d| d.timestamp_millis());
    let resets_in = next_midnight.map(|t| t - now.timestamp_millis());

    let plan = if disabled {
        "CPA 凭证已禁用".to_string()
    } else if failed > 0 {
        format!("CPA 凭证异常 · {failed} 次失败")
    } else {
        "CPA 凭证可用 · 上游额度暂不可读".to_string()
    };
    let limits = activity
        .map(|(today, avg7)| {
            let used_pct = if avg7 > 0.0 {
                (today as f64 / avg7 * 100.0).clamp(0.0, 100.0)
            } else if today > 0 {
                100.0
            } else {
                0.0
            };
            vec![model::Limit {
                label: "Today vs 7-day activity".into(),
                used_pct,
                resets_in_ms: resets_in,
                status: model::status_for(used_pct),
            }]
        })
        .unwrap_or_default();
    Some(Card::Live(Report {
        provider: ProviderId::Antigravity,
        source: QuotaSource::CpaLocal,
        plan_type: plan,
        limits,
    }))
}

/// Codex 原生配额:读 ~/.codex/auth.json 的 OAuth token,直调 wham/usage.
/// 返回的快照只含 Codex 一张卡(Go/GOAT 由各自链路负责)。
fn codex_native_quota_report() -> anyhow::Result<Report> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .map_err(|_| anyhow::anyhow!("无 HOME"))?;
    let auth_path = home.join(".codex").join("auth.json");
    let text = std::fs::read_to_string(&auth_path)
        .map_err(|_| anyhow::anyhow!("~/.codex/auth.json 不存在(未用 Codex CLI 登录)"))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("auth.json 解析失败: {e}"))?;
    let access = v
        .pointer("/tokens/access_token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("auth.json 无 tokens.access_token"))?
        .to_string();
    let account = v
        .pointer("/tokens/account_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();

    let resp = ureq::get("https://chatgpt.com/backend-api/wham/usage")
        .set("Authorization", &format!("Bearer {access}"))
        .set("ChatGPT-Account-Id", &account)
        .timeout(Duration::from_secs(15))
        .call()?;
    let v: serde_json::Value = resp
        .into_json()
        .map_err(|e| anyhow::anyhow!("wham/usage 响应解析失败: {e}"))?;
    let rl = &v["rate_limit"];

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let mut limits = Vec::new();
    for (key, label) in [
        ("primary_window", "5 hours"),
        ("secondary_window", "7 days"),
    ] {
        let w = &rl[key];
        let Some(used) = w["used_percent"].as_f64() else {
            continue;
        };
        if !used.is_finite() || !(0.0..=100.0).contains(&used) {
            continue;
        }
        // resets_at 为 epoch 秒
        let resets_in_ms = w["resets_at"].as_i64().map(|s| s * 1000 - now_ms);
        limits.push(model::Limit {
            label: label.into(),
            used_pct: used,
            resets_in_ms,
            status: model::status_for(used),
        });
    }
    if limits.is_empty() {
        anyhow::bail!("wham/usage 无窗口数据");
    }
    Ok(Report {
        provider: ProviderId::Codex,
        source: QuotaSource::CodexAccount,
        plan_type: v["plan_type"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Codex access")
            .to_string(),
        limits,
    })
}

fn run_omp(exe: &str) -> anyhow::Result<String> {
    let mut cmd = Command::new(exe);
    cmd.args(["usage", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: 避免子进程弹黑框/改写主控台标题
    }
    let mut child = cmd.spawn()?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let out = child.wait_with_output()?;
            if !status.success() {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                anyhow::bail!("omp usage 失败: {err}");
            }
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        if start.elapsed() > TIMEOUT {
            let _ = child.kill();
            anyhow::bail!("omp usage 超时(12s),已 kill");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omp_fallback_fills_only_missing_providers() {
        let mut cards = ProviderId::all().map(|provider| Card::Missing { provider });
        cards[1] = Card::Live(Report {
            provider: ProviderId::Codex,
            source: QuotaSource::CodexAppServer,
            plan_type: "pro".into(),
            limits: vec![],
        });
        let fallback = model::parse_snapshot(include_str!("../tests/fixtures/usage.json")).unwrap();
        merge_missing(&mut cards, fallback.cards);
        assert!(matches!(&cards[0], Card::Live(r) if r.source == QuotaSource::OmpFallback));
        assert!(matches!(&cards[1], Card::Live(r) if r.source == QuotaSource::CodexAppServer));
        assert!(matches!(cards[2], Card::Missing { .. }));
    }

    #[test]
    fn cpa_auth_files_keep_account_index_and_project_without_credentials() {
        let accounts = parse_auth_files(r#"{"files":[{"id":"other","auth_index":"0"},{"id":"antigravity-a","auth_index":"17","project_id":"project-a","disabled":false},{"account_type":"antigravity","auth_index":18,"metadata":{"projectId":"project-b"},"disabled":true}]}"#).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].auth_index, "17");
        assert_eq!(accounts[0].project_id.as_deref(), Some("project-a"));
        assert_eq!(accounts[1].auth_index, "18");
        assert!(accounts[1].disabled);
    }

    #[test]
    fn cpa_quota_summary_converts_remaining_fraction_and_reset() {
        let payload = serde_json::json!({"groups":[
            {"displayName":"Gemini Models","buckets":[
                {"bucketId":"gemini-weekly","window":"weekly","remainingFraction":0.86,"resetTime":"2026-09-30T05:04:06Z"},
                {"bucketId":"gemini-5h","window":"5h","remainingFraction":1.0,"resetTime":"2026-09-27T01:48:19Z"}]},
            {"displayName":"Claude and GPT models","buckets":[
                {"bucketId":"3p-weekly","window":"weekly","remainingFraction":0.0,"resetTime":"2026-10-02T05:08:39Z"}]}
        ]});
        let now = chrono::DateTime::parse_from_rfc3339("2026-09-27T00:00:00Z")
            .unwrap()
            .timestamp_millis();
        let limits = parse_cpa_quota(&payload, None, now);
        assert_eq!(limits.len(), 3);
        assert_eq!(limits[0].label, "Gemini · 7 days");
        assert!((limits[0].used_pct - 14.0).abs() < 0.001);
        assert!(limits[0].resets_in_ms.unwrap() > 0);
        assert_eq!(limits[1].used_pct, 0.0);
        assert_eq!(limits[2].label, "Claude/GPT · 7 days");
        assert_eq!(limits[2].status, model::Status::Exhausted);
    }

    #[test]
    fn cpa_invalid_or_unknown_buckets_do_not_invent_quota() {
        let payload = serde_json::json!({"groups":[{"displayName":"Gemini Models","buckets":[
            {"window":"weekly","remainingFraction":1.1},
            {"window":"monthly","remainingFraction":0.5},
            {"window":"5h"}
        ]}]});
        assert!(parse_cpa_quota(&payload, None, 0).is_empty());
    }

    #[test]
    fn cpa_tier_prefers_valid_paid_tier_and_falls_back_to_current() {
        let paid =
            serde_json::json!({"currentTier":{"id":"free-tier"},"paidTier":{"id":"g1-pro-tier"}});
        assert_eq!(parse_cpa_tier(&paid).as_deref(), Some("Google AI Pro"));
        let current = serde_json::json!({"current_tier":{"id":"free-tier"},"paid_tier":{"name":"incomplete"}});
        assert_eq!(parse_cpa_tier(&current).as_deref(), Some("Google Free"));
    }

    #[test]
    #[ignore = "requires the user's running CPA and configured local management key"]
    fn local_cpa_returns_real_upstream_antigravity_quota() {
        let Some(Card::Live(report)) = cpa_antigravity_card() else {
            panic!("CPA card unavailable")
        };
        assert_eq!(report.source, QuotaSource::CpaGoogleQuota);
        assert!(
            report
                .limits
                .iter()
                .any(|limit| limit.label.contains("Gemini"))
        );
    }
}
