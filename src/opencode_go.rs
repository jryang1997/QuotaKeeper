//! OpenCode Go account-wide usage from the provider's own read-only endpoint.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::model::{Limit, ProviderId, QuotaSource, Report, status_for};

const USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";

fn auth_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let path = PathBuf::from(xdg).join("opencode/auth.json");
        if path.is_file() {
            return Some(path);
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(PathBuf::from(home).join(".local/share/opencode/auth.json"))
}

fn key_from_first_party_auth() -> Option<String> {
    let content = std::fs::read_to_string(auth_path()?).ok()?;
    let auth: serde_json::Value = serde_json::from_str(&content).ok()?;
    let entry = &auth["opencode-go"];
    if entry["type"].as_str()? != "api" {
        return None;
    }
    let key = entry["key"].as_str()?.trim();
    crate::credential::valid_api_key(key).then(|| key.to_owned())
}

fn api_key() -> Result<String> {
    if let Some(key) = crate::credential::open_code_go_key().ok().flatten()
        && crate::credential::valid_api_key(&key)
    {
        return Ok(key);
    }
    for name in ["OPENCODE_GO_API_KEY", "OPENCODE_API_KEY"] {
        if let Ok(key) = std::env::var(name) {
            let key = key.trim();
            if crate::credential::valid_api_key(key) {
                return Ok(key.to_owned());
            }
        }
    }
    key_from_first_party_auth().ok_or_else(|| {
        anyhow!("OpenCode Go API key 未配置；可在设置页保存、设置 OPENCODE_GO_API_KEY，或使用 OpenCode /connect")
    })
}

#[derive(Deserialize)]
struct Window {
    status: String,
    percent: f64,
    #[serde(rename = "resetsAt")]
    resets_at: String,
}

#[derive(Deserialize)]
struct Usage {
    rolling: Window,
    weekly: Window,
    monthly: Window,
}

#[derive(Deserialize)]
struct Response {
    usage: Usage,
}

fn parse_window(label: &str, window: Window, now_ms: i64) -> Result<Limit> {
    if !window.percent.is_finite() || !(0.0..=100.0).contains(&window.percent) {
        return Err(anyhow!("OpenCode Go usage.percent 无效"));
    }
    if !matches!(window.status.as_str(), "ok" | "rate-limited") {
        return Err(anyhow!("OpenCode Go usage.status 无效"));
    }
    let used_pct = if window.status == "rate-limited" {
        100.0
    } else {
        window.percent
    };
    let reset_ms = chrono::DateTime::parse_from_rfc3339(&window.resets_at)
        .map_err(|_| anyhow!("OpenCode Go resetsAt 无效"))?
        .timestamp_millis();
    Ok(Limit {
        label: label.into(),
        used_pct,
        resets_in_ms: Some(reset_ms - now_ms),
        status: status_for(used_pct),
    })
}

fn parse_report(json: &str, now_ms: i64) -> Result<Report> {
    let response: Response =
        serde_json::from_str(json).map_err(|_| anyhow!("OpenCode Go usage 响应结构已变化"))?;
    Ok(Report {
        provider: ProviderId::OpenCodeGo,
        source: QuotaSource::OpenCodeGoApi,
        // A successful usage response proves Go access, not a settled invoice.
        plan_type: "Go access active".into(),
        limits: vec![
            parse_window("5 hours", response.usage.rolling, now_ms)?,
            parse_window("7 days", response.usage.weekly, now_ms)?,
            parse_window("Monthly", response.usage.monthly, now_ms)?,
        ],
        subscription_ends_at_ms: None,
        balance: None,
    })
}

pub fn fetch_report() -> Result<Report> {
    let key = api_key()?;
    let response = crate::http::get(USAGE_URL)
        .set("Authorization", &format!("Bearer {key}"))
        .set("Accept", "application/json")
        .set("User-Agent", "quotakeeper/0.1")
        .timeout(Duration::from_secs(12))
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(401, _) => anyhow!("OpenCode Go API key 无效或已撤销"),
            ureq::Error::Status(403, _) => anyhow!("OpenCode Go 权益不可用；请检查订阅状态"),
            ureq::Error::Status(429, _) => anyhow!("OpenCode Go 暂时限流"),
            ureq::Error::Status(code, _) => anyhow!("OpenCode Go 请求失败: HTTP {code}"),
            ureq::Error::Transport(_) => anyhow!("OpenCode Go 连接失败"),
        })?;
    let json = response.into_string()?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    parse_report(&json, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_response_keeps_three_independent_windows() {
        let report = parse_report(
            r#"{"usage":{"rolling":{"status":"ok","percent":25,"resetsAt":"2026-09-27T08:00:00Z"},"weekly":{"status":"rate-limited","percent":87,"resetsAt":"2026-09-28T08:00:00Z"},"monthly":{"status":"ok","percent":50.5,"resetsAt":"2026-10-01T08:00:00Z"}}}"#,
            1_790_489_600_000,
        )
        .unwrap();
        assert_eq!(report.source, QuotaSource::OpenCodeGoApi);
        assert_eq!(report.limits.len(), 3);
        assert_eq!(report.limits[0].used_pct, 25.0);
        assert_eq!(report.limits[1].used_pct, 100.0);
        assert_eq!(report.limits[2].used_pct, 50.5);
    }

    #[test]
    fn malformed_response_does_not_become_zero_usage() {
        assert!(parse_report(r#"{"usage":{"rolling":{"percent":0}}}"#, 0).is_err());
    }
}
