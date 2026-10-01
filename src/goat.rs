//! GOAT 原生采集:api.commandcode.ai alpha billing(内部/alpha 端点,非官方稳定契约).
//!
//! 口径复用 cmduse(cmd-usage 0.6.9 / cmduse-core)验证过的实现:
//! - 认证:应用的 Windows 凭据、`COMMAND_CODE_API_KEY`/`CMD_API_KEY` env，或官方 CLI 登录态.
//! - `GET /alpha/billing/credits` → balances + windowLimits(fiveHour/weekly used/cap/resetAt 毫秒).
//! - `GET /alpha/billing/subscriptions` → planId/status/currentPeriodEnd(月窗口推导用).
//! - 月窗口:used = cap - monthlyCredits(钳制),reset = periodEnd;5h/周逐字采用,不自推.
//! - 来源:man cmduse.1(https://docs.rs/crate/cmd-usage/0.6.9/source/cmduse.1),
//!   wire shapes(https://docs.rs/crate/cmduse-core/0.6.5/source/src/wire.rs).
//! - 风险:alpha 前缀随时变;401/403 停转重登(`cmdc login`);429/5xx 退避.
//!
//! 约束:key 只读内存送 Authorization 头,永不打印/落盘/进日志.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::model::{Limit, ProviderId, QuotaSource, Report, Status, status_for};

const API_BASE: &str = "https://api.commandcode.ai";
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Credits {
    monthly_credits: f64,
    #[allow(dead_code)]
    #[serde(default)]
    purchased_credits: f64,
    #[allow(dead_code)]
    #[serde(default)]
    free_credits: f64,
}

#[derive(Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
struct Window {
    #[serde(default)]
    used: Option<f64>,
    #[serde(default)]
    cap: f64,
    #[serde(default)]
    exceeded: bool,
    #[serde(default)]
    reset_at: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WindowLimits {
    #[serde(default)]
    five_hour: Option<Window>,
    #[serde(default)]
    weekly: Option<Window>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreditsResp {
    credits: Credits,
    #[serde(default)]
    window_limits: WindowLimits,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SubData {
    #[allow(dead_code)]
    #[serde(default)]
    status: String,
    #[serde(default)]
    plan_id: String,
    #[serde(default)]
    current_period_end: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    current_period_start: Option<String>,
}

#[derive(Deserialize, Default)]
struct SubscriptionsResp {
    #[serde(default)]
    data: Option<SubData>,
}

/// 官方当前账期用量汇总,与 dashboard「Current billing month」同源.
/// wire 实测(2026-09-28):无参 GET 即返回 periodBasis=billing-period 的账期累计;
/// totalTokens 为服务端 in+out 合计,别自算.
#[derive(Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub total_count: f64,
    pub total_tokens: u64,
    pub total_cost: f64,
}

/// 读 key:应用凭据优先,再用环境变量或官方 CLI 登录态.不回显内容.
fn api_key() -> anyhow::Result<String> {
    if let Some(key) = crate::credential::command_code_key().ok().flatten()
        && crate::credential::valid_api_key(&key)
    {
        return Ok(key);
    }
    if let Ok(k) = std::env::var("COMMAND_CODE_API_KEY")
        && crate::credential::valid_api_key(&k)
    {
        return Ok(k);
    }
    if let Ok(k) = std::env::var("CMD_API_KEY")
        && crate::credential::valid_api_key(&k)
    {
        return Ok(k);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| anyhow::anyhow!("GOAT 未登录:在真实终端运行 `cmdc login`"))?;
    let path = format!("{home}/.commandcode/auth.json");
    let text = std::fs::read_to_string(&path).map_err(|_| {
        anyhow::anyhow!(
            "GOAT 未登录:在真实终端运行 `cmdc login`(会在 ~/.commandcode/auth.json 写入 apiKey)"
        )
    })?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| anyhow::anyhow!("GOAT 登录态损坏:{path} 不是合法 JSON,重跑 `cmdc login`"))?;
    v.get("apiKey")
        .and_then(|k| k.as_str())
        .filter(|k| crate::credential::valid_api_key(k))
        .map(|k| k.to_string())
        .ok_or_else(|| anyhow::anyhow!("GOAT 登录态缺 apiKey 字段:重跑 `cmdc login`"))
}

fn get_json<T: for<'de> Deserialize<'de>>(path: &str, key: &str) -> anyhow::Result<T> {
    let resp = crate::http::get(&format!("{API_BASE}{path}"))
        .set("Authorization", &format!("Bearer {key}"))
        .set("Accept", "application/json")
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| anyhow::anyhow!("GOAT {path} 请求失败:{e}"))?;
    resp.into_json::<T>()
        .map_err(|e| anyhow::anyhow!("GOAT {path} 解析失败:{e}"))
}

/// planId → 展示名.顺序抄 cmduse-core NAME_RULES(enterprise/provider 优先).
fn plan_name(plan_id: &str) -> String {
    let id = plan_id.to_lowercase();
    if id.contains("enterprise") {
        "Enterprise".into()
    } else if id.contains("provider") {
        "Provider".into()
    } else if id.contains("team") {
        "Team Pro".into()
    } else if id.contains("max") && id.contains("20") {
        "Max 20x".into()
    } else if id.contains("max") {
        "Max 10x".into()
    } else if id.contains("goat") {
        "GOAT".into()
    } else if id.contains("pro") {
        "Pro".into()
    } else if id.contains("go") {
        "Go".into()
    } else if id.is_empty() {
        "订阅类型未知".into()
    } else {
        plan_id.into()
    }
}

/// planId → 月池(美元).None = PAYG 无月 cap(Enterprise/Provider/未知).
fn plan_monthly_cap(plan_id: &str) -> Option<f64> {
    let id = plan_id.to_lowercase();
    if id.contains("enterprise") || id.contains("provider") {
        None
    } else if id.contains("goat") {
        Some(70.0)
    } else if id.contains("max") && id.contains("20") {
        Some(300.0)
    } else if id.contains("max") {
        Some(150.0)
    } else if id.contains("team") {
        Some(40.0)
    } else if id.contains("pro") {
        Some(80.0)
    } else if id.contains("go") {
        Some(10.0)
    } else {
        None
    }
}

fn parse_iso_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp_millis())
}

fn window_limit(label: &str, w: Window, now_ms: i64) -> Option<Limit> {
    let used = w.used.filter(|value| value.is_finite() && *value >= 0.0)?;
    if !w.cap.is_finite() || w.cap <= 0.0 {
        return None;
    }
    let used_pct = (used / w.cap * 100.0).clamp(0.0, 100.0);
    Some(Limit {
        label: label.to_string(),
        used_pct,
        resets_in_ms: w.reset_at.map(|r| r as i64 - now_ms),
        status: if w.exceeded {
            Status::Exhausted
        } else {
            status_for(used_pct)
        },
    })
}

fn monthly_limit(cap: f64, remaining: f64, period_end: Option<&str>, now_ms: i64) -> Limit {
    let used = (cap - remaining).clamp(0.0, cap);
    let used_pct = (used / cap * 100.0).clamp(0.0, 100.0);
    Limit {
        label: "Monthly (inferred)".to_string(),
        used_pct,
        resets_in_ms: period_end.and_then(parse_iso_ms).map(|r| r - now_ms),
        status: status_for(used_pct),
    }
}

fn build_report(credits: &CreditsResp, subs: &SubData, now_ms: i64) -> anyhow::Result<Report> {
    let mut limits = Vec::new();
    if let Some(w) = credits.window_limits.five_hour
        && let Some(l) = window_limit("5 hours", w, now_ms)
    {
        limits.push(l);
    }
    if let Some(w) = credits.window_limits.weekly
        && let Some(l) = window_limit("7 days", w, now_ms)
    {
        limits.push(l);
    }
    if let Some(cap) = plan_monthly_cap(&subs.plan_id)
        && credits.credits.monthly_credits.is_finite()
        && credits.credits.monthly_credits >= 0.0
    {
        limits.push(monthly_limit(
            cap,
            credits.credits.monthly_credits,
            subs.current_period_end.as_deref(),
            now_ms,
        ));
    }
    anyhow::ensure!(
        !limits.is_empty(),
        "GOAT response contains no valid quota windows"
    );
    Ok(Report {
        provider: ProviderId::Goat,
        source: QuotaSource::CommandCodeApi,
        plan_type: if subs.status.is_empty() {
            plan_name(&subs.plan_id)
        } else {
            format!("{} · {}", plan_name(&subs.plan_id), subs.status)
        },
        limits,
        // 订阅周期结束 = 续费/到期时间(currentPeriodEnd),供到期提醒用.
        subscription_ends_at_ms: subs.current_period_end.as_deref().and_then(parse_iso_ms),
    })
}

/// 拉一次 GOAT 快照.无登录态/断网一律 Err,调用方转 Missing 卡展示.
pub fn fetch_report() -> anyhow::Result<Report> {
    let key = api_key()?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let credits: CreditsResp = get_json("/alpha/billing/credits", &key)?;
    // 订阅接口失败时保留真实 5h/7d 窗口，不推测套餐与月额度。
    let subs: SubData = get_json::<SubscriptionsResp>("/alpha/billing/subscriptions", &key)
        .map(|r| r.data.unwrap_or_default())
        .unwrap_or_default();
    build_report(&credits, &subs, now_ms)
}

/// 官方当前账期用量汇总;无登录态/断网/端点变更一律 Err,调用方回退本机账本.
pub fn fetch_usage_summary() -> anyhow::Result<UsageSummary> {
    let key = api_key()?;
    get_json("/alpha/usage/summary", &key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureDoc {
        credits: Credits,
        window_limits: WindowLimits,
        subscription: SubData,
    }

    const FIXTURE: &str = include_str!("../tests/fixtures/goat.json");
    // fixture resetAt 相对该 now 的偏移:5h=2h22m,周=1d18h(抄 cmduse.1 示例).
    const NOW_MS: i64 = 1_790_000_000_000;

    fn fixture_report() -> Report {
        let doc: FixtureDoc = serde_json::from_str(FIXTURE).unwrap();
        build_report(
            &CreditsResp {
                credits: doc.credits,
                window_limits: doc.window_limits,
            },
            &doc.subscription,
            NOW_MS,
        )
        .unwrap()
    }

    #[test]
    fn goat_fixture_windows_match_cmduse_math() {
        let r = fixture_report();
        assert_eq!(r.provider, ProviderId::Goat);
        assert_eq!(r.plan_type, "GOAT · active");
        assert_eq!(r.limits.len(), 3);
        // 5h:1.55/14;周:19.61/35;月:(70-44.15)/70.
        assert!((r.limits[0].used_pct - (1.55 / 14.0 * 100.0)).abs() < 1e-9);
        assert!((r.limits[1].used_pct - (19.61 / 35.0 * 100.0)).abs() < 1e-9);
        assert!((r.limits[2].used_pct - (25.85 / 70.0 * 100.0)).abs() < 1e-9);
        assert_eq!(r.limits[0].resets_in_ms, Some(8_520_000));
        assert_eq!(r.limits[1].resets_in_ms, Some(151_200_000));
        assert_eq!(
            crate::model::format_countdown(r.limits[0].resets_in_ms.unwrap()),
            "2h 22m"
        );
        assert_eq!(
            crate::model::format_countdown(r.limits[1].resets_in_ms.unwrap()),
            "1d 18h"
        );
    }
    /// 真实 200 响应形状回归(2026-09-25 实测,已脱敏):小数 used/cap + 13 位 resetAt + individual-goat.
    #[test]
    fn live_shape_parses_float_windows() {
        let doc: FixtureDoc = serde_json::from_str(
            r#"{"credits":{"monthlyCredits":14.9067062948,"purchasedCredits":0.0,"freeCredits":0.0},"windowLimits":{"fiveHour":{"used":0.736504846,"cap":14.0,"resetAt":1790265034888.0},"weekly":{"used":10.3077762467,"cap":35.0,"resetAt":1790435108074.0}},"subscription":{"status":"active","planId":"individual-goat","currentPeriodStart":"2026-09-01T09:17:45.000Z","currentPeriodEnd":"2026-10-01T09:17:45.000Z"}}"#,
        )
        .unwrap();
        let r = build_report(
            &CreditsResp {
                credits: doc.credits,
                window_limits: doc.window_limits,
            },
            &doc.subscription,
            NOW_MS,
        )
        .unwrap();
        assert_eq!(r.plan_type, "GOAT · active");
        assert_eq!(r.limits.len(), 3);
        assert!((r.limits[0].used_pct - (0.736504846 / 14.0 * 100.0)).abs() < 1e-9);
        assert!((r.limits[1].used_pct - (10.3077762467 / 35.0 * 100.0)).abs() < 1e-9);
        assert!((r.limits[2].used_pct - ((70.0 - 14.9067062948) / 70.0 * 100.0)).abs() < 1e-9);
    }

    #[test]
    fn payg_plan_skips_monthly_card() {
        let doc: FixtureDoc = serde_json::from_str(
            r#"{"credits":{"monthlyCredits":0.0},"windowLimits":{"fiveHour":{"used":1.0,"cap":14.0}},"subscription":{"status":"active","planId":"individual-provider"}}"#,
        )
        .unwrap();
        let r = build_report(
            &CreditsResp {
                credits: doc.credits,
                window_limits: doc.window_limits,
            },
            &doc.subscription,
            NOW_MS,
        )
        .unwrap();
        assert_eq!(r.plan_type, "Provider · active");
        assert_eq!(r.limits.len(), 1);
    }

    #[test]
    fn failed_subscription_never_invents_goat_monthly_cap() {
        let doc: FixtureDoc = serde_json::from_str(FIXTURE).unwrap();
        let report = build_report(
            &CreditsResp {
                credits: doc.credits,
                window_limits: doc.window_limits,
            },
            &SubData::default(),
            NOW_MS,
        )
        .unwrap();
        assert_eq!(report.plan_type, "订阅类型未知");
        assert_eq!(report.limits.len(), 2);
        let empty: CreditsResp =
            serde_json::from_str(r#"{"credits":{"monthlyCredits":1}}"#).unwrap();
        assert!(build_report(&empty, &SubData::default(), NOW_MS).is_err());
    }

    #[test]
    fn zero_cap_window_is_dropped() {
        assert!(window_limit("5 hours", Window::default(), NOW_MS).is_none());
        let missing: Window = serde_json::from_str(r#"{"cap":14}"#).unwrap();
        assert!(window_limit("5 hours", missing, NOW_MS).is_none());
        let zero: Window = serde_json::from_str(r#"{"cap":14,"used":0}"#).unwrap();
        assert_eq!(window_limit("5 hours", zero, NOW_MS).unwrap().used_pct, 0.0);
        let negative: Window = serde_json::from_str(r#"{"cap":14,"used":-1}"#).unwrap();
        assert!(window_limit("5 hours", negative, NOW_MS).is_none());
    }

    /// 真实 usage summary 响应形状回归(2026-09-28 实测,已脱敏).
    #[test]
    fn usage_summary_parses_live_shape() {
        let doc: UsageSummary = serde_json::from_str(
            r#"{"totalCount":5752,"totalCost":59.05022897839999,"averageCost":0.010266034245201668,"successRate":100,"completedCount":5752,"failedCount":0,"totalTokensIn":1050023401,"totalTokensOut":4664676,"totalTokens":1054688077,"totalCredits":59.05022897839999,"totalFreeCredits":0,"totalMonthlyCredits":59.05022897839999,"totalPurchasedCredits":0,"periodBasis":"billing-period"}"#,
        )
        .unwrap();
        assert_eq!(doc.total_tokens, 1_054_688_077);
        assert_eq!(doc.total_count, 5752.0);
        assert!((doc.total_cost - 59.05022897839999).abs() < 1e-9);
    }

    #[test]
    fn usage_summary_rejects_missing_fields() {
        for doc in ["{}", r#"{"totalTokens":42}"#] {
            assert!(serde_json::from_str::<UsageSummary>(doc).is_err());
        }
    }

    #[test]
    fn plan_names_follow_cmduse_order() {
        assert_eq!(plan_name("individual-goat"), "GOAT");
        assert_eq!(plan_name("individual-max-20"), "Max 20x");
        assert_eq!(plan_name("individual-max-10"), "Max 10x");
        // provider 含 "pro" 子串,必须先匹配到 Provider.
        assert_eq!(plan_name("individual-provider"), "Provider");
    }
}
