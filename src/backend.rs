//! 数据源:配额优先复用 `omp usage --json`(OMP 已做登录态 + 配额归一化),
//! OMP 不可用时回退 Codex 原生配额(auth.json OAuth → wham/usage,照抄 CodexBar).
//!
//! 约定:后台线程里阻塞调用,超时 30s 后 kill,避免卡死刷新循环.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::model::{self, Snapshot};

const TIMEOUT: Duration = Duration::from_secs(30);

fn omp_candidates() -> Vec<String> {
    let mut v = vec!["omp".to_string()];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        v.push(format!("{local}\\omp\\omp.exe"));
    }
    v
}

/// 拉一次全量快照.返回的 Err 直接展示为顶栏横幅,不吞错.
pub fn fetch_snapshot() -> anyhow::Result<Snapshot> {
    let quota_res = omp_or_native();
    let mut snap = match quota_res {
        Ok(s) => s,
        Err(e) => {
            // 全失败:CPA 卡还能拿 → 构建 Missing 快照只挂 CPA 卡;否则报错
            match cpa_antigravity_card() {
                Some(card) => Snapshot {
                    generated_at_ms: now_ms(),
                    cards: model::ProviderId::all()
                        .into_iter()
                        .map(|p| {
                            if p == model::ProviderId::Antigravity {
                                card.clone()
                            } else {
                                model::Card::Missing { provider: p }
                            }
                        })
                        .collect(),
                },
                None => return Err(e),
            }
        }
    };
    // Antigravity (CPA) 卡:尽力而为,失败不影响其他卡
    if let Some(card) = cpa_antigravity_card()
        && let Some(slot) = snap.cards.iter_mut().find(|c| {
            matches!(
                c,
                model::Card::Missing {
                    provider: model::ProviderId::Antigravity
                }
            )
        })
    {
        *slot = card;
    }
    Ok(snap)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn omp_or_native() -> anyhow::Result<Snapshot> {
    let mut not_found = 0;
    for exe in omp_candidates() {
        match run_omp(&exe) {
            Ok(json) => return crate::model::parse_snapshot(&json),
            Err(e) => {
                if let Some(io) = e.downcast_ref::<std::io::Error>()
                    && io.kind() == std::io::ErrorKind::NotFound
                {
                    not_found += 1;
                    continue;
                }
                return Err(e);
            }
        }
    }
    // OMP 不可用 → Codex 原生回退(CodexBar 式直连,不依赖任何 CLI)
    match codex_native_quota() {
        Ok(s) => Ok(s),
        Err(ne) => anyhow::bail!("omp 未找到({not_found} 个候选)且 Codex 原生回退失败: {ne}"),
    }
}

// ---------------- Antigravity (CPA) 卡 ----------------

const CPA_BASE: &str = "http://127.0.0.1:8317";
const CPA_TIMEOUT: Duration = Duration::from_secs(6);

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

/// 解析 auth-files 响应,找 antigravity 凭证的 (email, disabled, failed).
fn parse_auth_files(json: &str) -> Option<(String, bool, i64)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let files = v.get("files")?.as_array()?;
    for f in files {
        let id = f.get("id").and_then(|x| x.as_str()).unwrap_or("");
        let ty = f.get("account_type").and_then(|x| x.as_str()).unwrap_or("");
        if id.starts_with("antigravity-") || ty == "antigravity" {
            return Some((
                f.get("email")
                    .or_else(|| f.get("account"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("antigravity")
                    .to_string(),
                f.get("disabled").and_then(|x| x.as_bool()).unwrap_or(false),
                f.get("failed").and_then(|x| x.as_i64()).unwrap_or(0),
            ));
        }
    }
    None
}

/// 今日经 CPA(provider='cpa')的 token 合计,以及近 7 天日均值.
fn cpa_today_and_avg7(db: &std::path::Path, today_start_ms: i64, week_start_ms: i64) -> (i64, f64) {
    let Ok(con) =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return (0, 0.0);
    };
    let q = |since: i64| -> i64 {
        con.query_row(
            "SELECT COALESCE(SUM(total_tokens),0) FROM messages \
             WHERE provider='cpa' AND timestamp >= ?1",
            [since],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
    };
    let today = q(today_start_ms);
    let week = q(week_start_ms);
    (today, week as f64 / 7.0)
}

/// Antigravity (CPA) 卡:凭证健康(auth-files)+ 今日强度(stats.db 今日 vs 7 日均值).
/// credits 百分比在当前 CPA 构建不可用(官方端点 403 + 管理接口无此数据),故用强度条替代.
pub fn cpa_antigravity_card() -> Option<model::Card> {
    let key = cpa_key()?;
    let resp = ureq::get(&format!("{CPA_BASE}/v0/management/auth-files"))
        .set("Authorization", &format!("Bearer {key}"))
        .timeout(CPA_TIMEOUT)
        .call()
        .ok()?;
    let body = resp.into_string().ok()?;
    let (email, disabled, failed) = parse_auth_files(&body)?;

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
    let (today, avg7) = cpa_today_and_avg7(
        &home.join(".omp").join("stats.db"),
        today_start,
        week_start_ms,
    );
    let used_pct = if avg7 > 0.0 {
        (today as f64 / avg7 * 100.0).clamp(0.0, 100.0)
    } else if today > 0 {
        100.0
    } else {
        0.0
    };
    // 重置点:下一个午夜
    let next_midnight = (now.date_naive() + chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .and_then(|d| d.and_local_timezone(now.timezone()).single())
        .map(|d| d.timestamp_millis());
    let resets_in = next_midnight.map(|t| t - now.timestamp_millis());

    let plan = if disabled {
        format!("{email} · disabled")
    } else if failed > 0 {
        format!("{email} · {failed} failed")
    } else {
        email
    };

    Some(model::Card::Live(model::Report {
        provider: model::ProviderId::Antigravity,
        plan_type: plan,
        limits: vec![model::Limit {
            label: "Today".into(),
            used_pct,
            resets_in_ms: resets_in,
            status: model::status_for(used_pct),
        }],
    }))
}

/// Codex 原生配额:读 ~/.codex/auth.json 的 OAuth token,直调 wham/usage.
/// 返回的快照只含 Codex 一张卡(Go/GOAT 由各自链路负责)。
pub fn codex_native_quota() -> anyhow::Result<Snapshot> {
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
        let used = w["used_percent"]
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("wham 响应缺 {key}.used_percent"))?
            .clamp(0.0, 100.0);
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
    Ok(Snapshot {
        generated_at_ms: now_ms,
        cards: vec![
            model::Card::Missing {
                provider: model::ProviderId::OpenCodeGo,
            },
            model::Card::Live(model::Report {
                provider: model::ProviderId::Codex,
                plan_type: "plus (native)".into(),
                limits,
            }),
            model::Card::Missing {
                provider: model::ProviderId::Goat,
            },
            model::Card::Missing {
                provider: model::ProviderId::Antigravity,
            },
        ],
    })
}

fn run_omp(exe: &str) -> anyhow::Result<String> {
    let mut child = Command::new(exe)
        .args(["usage", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
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
            anyhow::bail!("omp usage 超时(30s),已 kill");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
