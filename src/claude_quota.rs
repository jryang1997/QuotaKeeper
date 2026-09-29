//! Claude Pro/Max 订阅配额卡:读 Claude Code 本机登录态,直调官方 oauth usage 接口。
//!
//! - 凭据:`CLAUDE_CONFIG_DIR`(或 `~/.claude`)下 `.credentials.json` 的
//!   `claudeAiOauth.accessToken`(Windows 亦落盘;macOS 另有 Keychain 层,此处不读)。
//! - 接口:`GET https://api.anthropic.com/api/oauth/usage`,头
//!   `anthropic-beta: oauth-2025-04-20`;获取方式调研自 CodexBar(MIT)的
//!   ClaudeOAuthUsageFetcher,记录见 docs/research/codexbar-data-sources.md。
//! - 安全边界:**不做 token refresh**。refresh token 是旋转式的,第三方代刷
//!   会让 Claude Code 自身的登录态失效;access token 过期即报错,
//!   提示重新运行 `claude` 登录(Claude Code 使用中会自行续期)。

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::model::{Limit, ProviderId, QuotaSource, Report, status_for};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const TIMEOUT: Duration = Duration::from_secs(12);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn credentials_path() -> PathBuf {
    crate::paths::claude_dir().join(".credentials.json")
}

#[derive(Deserialize)]
struct OauthCredentials {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeOauth>,
}

#[derive(Deserialize)]
struct ClaudeOauth {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier")]
    rate_limit_tier: Option<String>,
}

struct LocalCredentials {
    access_token: String,
    plan: String,
}

/// subscriptionType / rateLimitTier → 展示名(如 default_claude_max_5x → Max 5x)。
fn plan_name(subscription_type: Option<&str>, rate_limit_tier: Option<&str>) -> String {
    let raw = subscription_type.or(rate_limit_tier).unwrap_or("");
    let id = raw.to_lowercase();
    let tier = if id.contains("max_20") {
        "Max 20x"
    } else if id.contains("max_5") {
        "Max 5x"
    } else if id.contains("max") {
        "Max"
    } else if id.contains("pro") {
        "Pro"
    } else if id.contains("free") {
        "Free"
    } else if raw.is_empty() {
        return "Claude 订阅".into();
    } else {
        return raw.to_string();
    };
    format!("Claude {tier}")
}

fn local_credentials() -> Result<LocalCredentials> {
    let path = credentials_path();
    let text = std::fs::read_to_string(&path).map_err(|_| {
        anyhow!(
            "Claude Code 登录态不存在({});运行 `claude` 登录后刷新",
            path.display()
        )
    })?;
    let creds: OauthCredentials = serde_json::from_str(&text)
        .map_err(|_| anyhow!("Claude Code 登录态结构已变化:{}", path.display()))?;
    let oauth = creds.claude_ai_oauth.ok_or_else(|| {
        anyhow!(
            "Claude Code 登录态缺 claudeAiOauth 节点(setup-token 凭据不支持订阅用量,需 `claude` 登录)"
        )
    })?;
    if let Some(expires_at) = oauth.expires_at
        && now_ms() >= expires_at
    {
        return Err(anyhow!("Claude 登录态已过期;运行 `claude` 重新登录后刷新"));
    }
    let plan = plan_name(
        oauth.subscription_type.as_deref(),
        oauth.rate_limit_tier.as_deref(),
    );
    Ok(LocalCredentials {
        access_token: oauth.access_token,
        plan,
    })
}

#[derive(Deserialize)]
struct UsageWindow {
    utilization: f64,
    #[serde(rename = "resets_at")]
    resets_at: String,
}

#[derive(Deserialize)]
struct UsageResponse {
    five_hour: Option<UsageWindow>,
    seven_day: Option<UsageWindow>,
    seven_day_opus: Option<UsageWindow>,
}

fn parse_window(label: &str, window: UsageWindow, now: i64) -> Result<Limit> {
    if !window.utilization.is_finite() || !(0.0..=100.0).contains(&window.utilization) {
        return Err(anyhow!("Claude usage.utilization 无效"));
    }
    let reset_ms = chrono::DateTime::parse_from_rfc3339(&window.resets_at)
        .map_err(|_| anyhow!("Claude usage.resets_at 无效"))?
        .timestamp_millis();
    Ok(Limit {
        label: label.into(),
        used_pct: window.utilization,
        resets_in_ms: Some(reset_ms - now),
        status: status_for(window.utilization),
    })
}

fn parse_report(json: &str, now: i64, plan: String) -> Result<Report> {
    let response: UsageResponse =
        serde_json::from_str(json).map_err(|_| anyhow!("Claude usage 响应结构已变化"))?;
    let mut limits = Vec::new();
    if let Some(w) = response.five_hour {
        limits.push(parse_window("5 hours", w, now)?);
    }
    if let Some(w) = response.seven_day {
        limits.push(parse_window("7 days", w, now)?);
    }
    if let Some(w) = response.seven_day_opus {
        limits.push(parse_window("7 days Opus", w, now)?);
    }
    if limits.is_empty() {
        return Err(anyhow!("Claude usage 响应无可用窗口"));
    }
    Ok(Report {
        provider: ProviderId::Claude,
        source: QuotaSource::ClaudeOauth,
        plan_type: plan,
        limits,
    })
}

pub fn fetch_report() -> Result<Report> {
    let creds = local_credentials()?;
    let response = crate::http::get(USAGE_URL)
        .set("Authorization", &format!("Bearer {}", creds.access_token))
        .set("anthropic-beta", "oauth-2025-04-20")
        .set("Accept", "application/json")
        .set("User-Agent", "tui-token-monitor/0.1")
        .timeout(TIMEOUT)
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(401, _) => anyhow!("Claude 登录态无效;运行 `claude` 重新登录"),
            ureq::Error::Status(403, _) => {
                anyhow!("Claude 订阅用量不可读(登录态需含 user:profile scope;setup-token 不支持)")
            }
            ureq::Error::Status(429, _) => anyhow!("Claude usage 接口限流;稍后自动重试"),
            ureq::Error::Status(code, _) => anyhow!("Claude usage 请求失败: HTTP {code}"),
            ureq::Error::Transport(_) => anyhow!("Claude usage 连接失败"),
        })?;
    let json = response.into_string()?;
    let now = now_ms();
    parse_report(&json, now, creds.plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_response_parses_windows_in_order() {
        let json = r#"{
            "five_hour": {"utilization": 41.5, "resets_at": "2026-09-29T08:00:00Z"},
            "seven_day": {"utilization": 12.0, "resets_at": "2026-10-03T00:00:00Z"},
            "seven_day_opus": {"utilization": 3.0, "resets_at": "2026-10-03T00:00:00Z"}
        }"#;
        let now = chrono::DateTime::parse_from_rfc3339("2026-09-29T06:00:00Z")
            .unwrap()
            .timestamp_millis();
        let report = parse_report(json, now, "Claude Max 5x".into()).unwrap();
        assert_eq!(report.provider, ProviderId::Claude);
        assert_eq!(report.plan_type, "Claude Max 5x");
        let labels: Vec<&str> = report.limits.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["5 hours", "7 days", "7 days Opus"]);
        let five = &report.limits[0];
        assert!((five.used_pct - 41.5).abs() < f64::EPSILON);
        assert_eq!(five.resets_in_ms, Some(2 * 3_600_000));
    }

    #[test]
    fn usage_response_without_windows_is_an_error() {
        let now = 0;
        assert!(parse_report("{}", now, "Claude Pro".into()).is_err());
        assert!(parse_report(
            r#"{"five_hour": {"utilization": 900.0, "resets_at": "2026-09-29T08:00:00Z"}}"#,
            now,
            "Claude Pro".into()
        )
        .is_err());
    }

    #[test]
    fn plan_name_maps_subscription_tiers() {
        assert_eq!(plan_name(Some("default_claude_max_5x"), None), "Claude Max 5x");
        assert_eq!(plan_name(Some("default_claude_max_20x"), None), "Claude Max 20x");
        assert_eq!(plan_name(Some("claude_pro"), None), "Claude Pro");
        assert_eq!(plan_name(None, Some("default_claude_max_5x")), "Claude Max 5x");
        assert_eq!(plan_name(None, None), "Claude 订阅");
        assert_eq!(plan_name(Some("custom_tier"), None), "custom_tier");
    }

    #[test]
    fn credentials_parse_and_expiry_gate() {
        let creds: OauthCredentials = serde_json::from_str(
            r#"{"claudeAiOauth": {"accessToken": "sk-ant-oat01-x", "expiresAt": 1, "subscriptionType": "default_claude_max_5x"}}"#,
        )
        .unwrap();
        let oauth = creds.claude_ai_oauth.unwrap();
        assert_eq!(oauth.access_token, "sk-ant-oat01-x");
        // expiresAt=1ms 必然过期:local_credentials 走文件,这里直接验证字段读取。
        assert!(oauth.expires_at.is_some());

        let no_oauth: OauthCredentials = serde_json::from_str(r#"{"other": 1}"#).unwrap();
        assert!(no_oauth.claude_ai_oauth.is_none());
    }
}
