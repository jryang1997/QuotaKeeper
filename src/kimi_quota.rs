//! Kimi For Coding(月之暗面 kimi.com)配额采集:官方 coding usage 接口.
//!
//! 形状依据(2026-10-01 逐字段核对):
//! - dsh-provider-balance(Apache-2.0):`GET https://api.kimi.com/coding/v1/usages`
//!   (备选路径 `/v1/usages`),Bearer `sk-kimi-` key;响应 `usage` 为周池、
//!   `limits[]` 里 `window.duration==300 & timeUnit 含 MINUTE` 的为 5h 窗口;
//!   **limit/remaining 是字符串配额点数**,不是 token 也不是百分比;
//!   `user.membership.level` 为档位(LEVEL_BASIC/INTERMEDIATE/ADVANCED).
//! - CodexBar docs/kimi.md(MIT):官方支持该接口;Kimi Code 订阅与 Moonshot
//!   开放平台是两把 key,不互通;周池档位 Andante 1024 / Moderato 2048 /
//!   Allegretto 7168 次,统一 200 次/5h.
//! - 订阅期 `expireTime` 在 kimi.com web gRPC 里,API key 路径拿不到,卡面如实不留.
//!
//! 约束:key 只读内存送 Authorization 头,永不打印/落盘/进日志;失败降级为 Missing 卡.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::model::{Limit, ProviderId, QuotaSource, Report, status_for};

const DEFAULT_BASE: &str = "https://api.kimi.com";
const TIMEOUT: Duration = Duration::from_secs(15);

fn api_key() -> anyhow::Result<String> {
    if let Some(key) = crate::credential::kimi_key().ok().flatten()
        && crate::credential::valid_api_key(&key)
    {
        return Ok(key);
    }
    for name in ["KIMI_CODE_API_KEY", "KIMI_CODING_API_KEY"] {
        if let Ok(k) = std::env::var(name)
            && crate::credential::valid_api_key(&k)
        {
            return Ok(k);
        }
    }
    anyhow::bail!("Kimi 未配置:在应用设置保存 Kimi For Coding key,或设置 KIMI_CODE_API_KEY")
}

/// 中国站 api.kimi.com;国际站设 KIMI_API_BASE=https://api.kimi.ai.
fn api_base() -> anyhow::Result<String> {
    let base = std::env::var("KIMI_API_BASE").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let base = base.trim_end_matches('/').to_string();
    if !base.starts_with("https://") {
        anyhow::bail!("KIMI_API_BASE 必须是 https:// 地址(保护 key 不走明文)");
    }
    Ok(base)
}

fn get_json<T: for<'de> Deserialize<'de>>(base: &str, path: &str, key: &str) -> anyhow::Result<T> {
    let resp = crate::http::get(&format!("{base}{path}"))
        .set("Authorization", &format!("Bearer {key}"))
        .set("Accept", "application/json")
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| anyhow::anyhow!("Kimi {path} 请求失败:{e}"))?;
    resp.into_json::<T>()
        .map_err(|e| anyhow::anyhow!("Kimi {path} 解析失败:{e}"))
}

/// dsh 观察到 /coding/v1 与 /v1 两种前缀并存,按序探测.
fn fetch_usage(base: &str, key: &str) -> anyhow::Result<UsagesBody> {
    let mut last = None;
    for path in ["/coding/v1/usages", "/v1/usages"] {
        match get_json::<UsagesBody>(base, path, key) {
            Ok(body) => return Ok(body),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("Kimi usages 请求失败")))
}

/// 数字或字符串数值(Kimi 的配额点数以字符串下发).
#[derive(Deserialize)]
#[serde(untagged)]
enum Flex {
    Num(f64),
    Str(String),
}

impl Flex {
    fn f64(&self) -> Option<f64> {
        match self {
            Flex::Num(n) => Some(*n).filter(|n| n.is_finite()),
            Flex::Str(s) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        }
    }
}

#[derive(Deserialize, Default)]
struct KimiDetail {
    #[serde(default)]
    limit: Option<Flex>,
    #[serde(default)]
    remaining: Option<Flex>,
    #[serde(default, rename = "resetTime")]
    reset_time: Option<String>,
}

#[derive(Deserialize, Default)]
struct UsagesBody {
    #[serde(default)]
    user: Option<UserInfo>,
    #[serde(default)]
    usage: Option<KimiDetail>,
    #[serde(default)]
    limits: Vec<serde_json::Value>,
}

#[derive(Deserialize, Default)]
struct UserInfo {
    #[serde(default)]
    membership: Option<Membership>,
}

#[derive(Deserialize, Default)]
struct Membership {
    #[serde(default)]
    level: Option<String>,
}

/// 档位展示名:dsh 的 KIMI_LEVELS 映射,未知档位剥前缀小写兜底.
fn level_label(raw: &str) -> String {
    match raw {
        "LEVEL_BASIC" => "basic".into(),
        "LEVEL_INTERMEDIATE" => "pro".into(),
        "LEVEL_ADVANCED" => "max".into(),
        other => other.strip_prefix("LEVEL_").unwrap_or(other).to_lowercase(),
    }
}

fn parse_reset_ms(raw: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.timestamp_millis());
    }
    chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc().timestamp_millis())
}

/// 配额点数窗口:used = limit - remaining;limit 缺失或 <=0 的行不编造直接丢弃.
fn point_window(detail: Option<&KimiDetail>, label: &str, now_ms: i64) -> Option<Limit> {
    let detail = detail?;
    let total = detail.limit.as_ref().and_then(Flex::f64)?;
    if total <= 0.0 {
        return None;
    }
    let remaining = detail
        .remaining
        .as_ref()
        .and_then(Flex::f64)
        .unwrap_or(total)
        .clamp(0.0, total);
    let used_pct = ((total - remaining) / total * 100.0).clamp(0.0, 100.0);
    Some(Limit {
        label: label.to_string(),
        used_pct,
        resets_in_ms: detail
            .reset_time
            .as_deref()
            .and_then(parse_reset_ms)
            .map(|t| (t - now_ms).max(0)),
        status: status_for(used_pct),
    })
}

/// 5h 窗口:limits[] 里 window.duration==300 且 timeUnit 含 MINUTE 的那条;
/// 兼容 {window, detail} 与裸 detail 两种形状(dsh 同逻辑).
fn session_limit(entries: &[serde_json::Value], now_ms: i64) -> Option<Limit> {
    for item in entries {
        let window = item.get("window");
        let is_session = window
            .and_then(|w| w.get("duration"))
            .and_then(|d| d.as_i64())
            == Some(300)
            && window
                .and_then(|w| w.get("timeUnit"))
                .and_then(|t| t.as_str())
                .is_some_and(|t| t.contains("MINUTE"));
        if !is_session {
            continue;
        }
        let detail_json = item.get("detail").unwrap_or(item);
        if let Ok(detail) = serde_json::from_value::<KimiDetail>(detail_json.clone()) {
            return point_window(Some(&detail), "5 hours", now_ms);
        }
    }
    None
}

/// 拉一次 Kimi Coding 快照.无 key/断网/形状变更一律 Err,调用方转 Missing 卡.
pub fn fetch_report() -> anyhow::Result<Report> {
    let key = api_key()?;
    let base = api_base()?;
    let body = fetch_usage(&base, &key)?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let mut limits = Vec::new();
    if let Some(l) = session_limit(&body.limits, now_ms) {
        limits.push(l);
    }
    if let Some(l) = point_window(body.usage.as_ref(), "7 days", now_ms) {
        limits.push(l);
    }
    if limits.is_empty() {
        anyhow::bail!("Kimi usages 无可用窗口数据");
    }
    let plan_type = body
        .user
        .as_ref()
        .and_then(|u| u.membership.as_ref())
        .and_then(|m| m.level.as_deref())
        .map(level_label)
        .unwrap_or_else(|| "Kimi Coding".into());
    Ok(Report {
        provider: ProviderId::Kimi,
        source: QuotaSource::KimiCodingApi,
        plan_type,
        limits,
        // 订阅期 expireTime 只在 web 会话里;API key 路径拿不到,不编造.
        subscription_ends_at_ms: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: i64 = 1_790_000_000_000;

    #[test]
    fn string_quota_points_parse_into_windows() {
        let detail: KimiDetail =
            serde_json::from_str(r#"{"limit":"1024","remaining":"768","resetTime":"2026-10-01T00:00:00Z"}"#)
                .unwrap();
        let l = point_window(Some(&detail), "7 days", NOW_MS).unwrap();
        assert!((l.used_pct - 25.0).abs() < 1e-9);
        assert_eq!(l.resets_in_ms, Some(1_790_812_800_000 - NOW_MS));
    }

    #[test]
    fn numeric_points_and_missing_remaining_mean_unused() {
        let detail: KimiDetail = serde_json::from_str(r#"{"limit":2048}"#).unwrap();
        let l = point_window(Some(&detail), "7 days", NOW_MS).unwrap();
        assert_eq!(l.used_pct, 0.0);
    }

    #[test]
    fn zero_or_invalid_limit_is_dropped() {
        let zero: KimiDetail = serde_json::from_str(r#"{"limit":0,"remaining":0}"#).unwrap();
        assert!(point_window(Some(&zero), "7 days", NOW_MS).is_none());
        let junk: KimiDetail = serde_json::from_str(r#"{"limit":"abc"}"#).unwrap();
        assert!(point_window(Some(&junk), "7 days", NOW_MS).is_none());
        assert!(point_window(None, "7 days", NOW_MS).is_none());
    }

    #[test]
    fn session_window_is_picked_from_300min_entry() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"window":{"duration":3600,"timeUnit":"TIME_UNIT_HOUR"},"detail":{"limit":"999","remaining":"0"}},
            {"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},"detail":{"limit":"200","remaining":"150","resetTime":"2026-10-01T00:00:00Z"}}
        ]"#,
        )
        .unwrap();
        let l = session_limit(&entries, NOW_MS).unwrap();
        assert_eq!(l.label, "5 hours");
        assert!((l.used_pct - 25.0).abs() < 1e-9);
    }

    #[test]
    fn bare_detail_entry_without_window_object_still_matches() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[{"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},"limit":"200","remaining":"150"}]"#,
        )
        .unwrap();
        let l = session_limit(&entries, NOW_MS).unwrap();
        assert!((l.used_pct - 25.0).abs() < 1e-9);
    }

    #[test]
    fn membership_levels_follow_dsh_mapping() {
        assert_eq!(level_label("LEVEL_BASIC"), "basic");
        assert_eq!(level_label("LEVEL_INTERMEDIATE"), "pro");
        assert_eq!(level_label("LEVEL_ADVANCED"), "max");
        assert_eq!(level_label("LEVEL_ENTERPRISE"), "enterprise");
        assert_eq!(level_label("weird"), "weird");
    }
}
