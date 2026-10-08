//! Cursor 订阅配额卡:读 Cursor 本机登录态,直调 usage-summary(未文档化接口)。
//!
//! - token 源(优先级):
//!   ① Cursor 编辑器的 `state.vscdb`(Windows `%APPDATA%\Cursor\User\globalStorage`,
//!      macOS/Linux 对应各自用户目录)ItemTable 的 `cursorAuth/accessToken`,
//!      值可能是 UTF-16LE BLOB 或明文;
//!   ② cursor-agent CLI 的 auth.json(`~/.cursor/auth.json` 或
//!      `~/.config/cursor/auth.json`,形状先例 vct-core cursor.rs)。
//! - 请求:`GET https://cursor.com/api/usage-summary`,Cookie
//!   `WorkosCursorSessionToken=<uid>%3A%3A<accessToken>`,uid 取 JWT `sub`
//!   的末段(参考 vct-core cursor.rs / CodexBar docs/cursor.md)。
//! - 安全边界:token 只读不写、不代刷(约 60 天一换,编辑器使用中自行续期);
//!   401/403 → 提示重新登录;端点无公开契约,响应形状变化整卡降级。

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::model::{Limit, ProviderId, QuotaSource, Report, status_for};

const USAGE_URL: &str = "https://cursor.com/api/usage-summary";
const TIMEOUT: Duration = Duration::from_secs(12);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// base64url 解码(标准字母表 + -/ 与 +/ 通吃,容忍 = 填充;手写避免引依赖)。
fn b64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for ch in input.bytes() {
        let v = match ch {
            b'A'..=b'Z' => (ch - b'A') as u32,
            b'a'..=b'z' => (ch - b'a' + 26) as u32,
            b'0'..=b'9' => (ch - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => return None,
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// JWT payload 的指定 claim(base64url 第二段)。
fn jwt_claim(token: &str, key: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = b64url_decode(payload)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get(key).cloned()
}

/// state.vscdb 的值可能是 UTF-16LE BLOB(带 BOM 或偶数长度含大量零字节)或明文。
fn decode_utf16_or_utf8(bytes: &[u8]) -> Option<String> {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16(&units).ok();
    }
    if bytes.len() >= 2
        && bytes.len() % 2 == 0
        && bytes
            .iter()
            .skip(1)
            .step_by(2)
            .filter(|&&b| b == 0)
            .count()
            > bytes.len() / 4
    {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16(&units).ok();
    }
    String::from_utf8(bytes.to_vec()).ok()
}

/// 归一化:剥空白;JSON 包裹则取 accessToken;接受裸 JWT。
fn normalize_token(text: &str) -> Option<String> {
    let t = text.trim();
    if t.starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(t).ok()?;
        let tok = v.get("accessToken")?.as_str()?.trim();
        return (!tok.is_empty()).then(|| tok.to_string());
    }
    t.starts_with("eyJ").then(|| t.to_string())
}

fn cursor_state_db_path() -> Option<PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let p = PathBuf::from(appdata)
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb");
        if p.is_file() {
            return Some(p);
        }
    }
    let home = crate::paths::home_dir();
    for p in [
        home.join(".config/Cursor/User/globalStorage/state.vscdb"),
        home.join("Library/Application Support/Cursor/User/globalStorage/state.vscdb"),
    ] {
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn token_from_state_db() -> Option<String> {
    let con = rusqlite::Connection::open_with_flags(
        cursor_state_db_path()?,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;
    let raw: rusqlite::types::Value = con
        .query_row(
            "SELECT value FROM ItemTable WHERE key = 'cursorAuth/accessToken'",
            [],
            |r| r.get(0),
        )
        .ok()?;
    let bytes = match raw {
        rusqlite::types::Value::Blob(b) => b,
        rusqlite::types::Value::Text(s) => s.into_bytes(),
        _ => return None,
    };
    let text = decode_utf16_or_utf8(&bytes)?;
    normalize_token(&text)
}

fn token_from_cli_auth() -> Option<String> {
    let home = crate::paths::home_dir();
    for p in [home.join(".cursor/auth.json"), home.join(".config/cursor/auth.json")] {
        if let Ok(text) = std::fs::read_to_string(&p)
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
        {
            return normalize_token(v.to_string().as_str()).or_else(|| {
                v.get("accessToken")
                    .and_then(|x| x.as_str())
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
            });
        }
    }
    None
}

fn local_token() -> Result<String> {
    token_from_state_db()
        .or_else(token_from_cli_auth)
        .ok_or_else(|| anyhow!("未找到 Cursor 登录态;启动 Cursor 编辑器或运行 `cursor-agent login` 后刷新"))
}

#[derive(Deserialize)]
struct UsageSummary {
    #[serde(rename = "billingCycleEnd")]
    billing_cycle_end: Option<String>,
    #[serde(rename = "membershipType")]
    membership_type: Option<String>,
    #[serde(rename = "individualUsage")]
    individual_usage: Option<IndividualUsage>,
}

#[derive(Deserialize)]
struct IndividualUsage {
    plan: Option<PlanUsage>,
}

#[derive(Deserialize)]
struct PlanUsage {
    #[serde(rename = "totalPercentUsed")]
    total_percent_used: Option<f64>,
}

fn membership_display(raw: Option<&str>) -> String {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => "Cursor".into(),
        Some(t) => {
            let id = t.to_lowercase();
            let tier = if id.contains("ultra") {
                "Ultra"
            } else if id.contains("teams") || id.contains("team") {
                "Teams"
            } else if id.contains("enterprise") {
                "Enterprise"
            } else if id.contains("pro") {
                "Pro"
            } else if id.contains("free") {
                "Free"
            } else {
                return t.to_string();
            };
            format!("Cursor {tier}")
        }
    }
}

fn parse_report(json: &str, now: i64) -> Result<Report> {
    let summary: UsageSummary =
        serde_json::from_str(json).map_err(|_| anyhow!("Cursor usage 响应结构已变化"))?;
    let pct = summary
        .individual_usage
        .as_ref()
        .and_then(|u| u.plan.as_ref())
        .and_then(|p| p.total_percent_used)
        .ok_or_else(|| anyhow!("Cursor usage 无 plan 百分比"))?;
    if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
        return Err(anyhow!("Cursor usage.totalPercentUsed 无效"));
    }
    let ends_ms = summary
        .billing_cycle_end
        .as_deref()
        .and_then(parse_iso_ms);
    let limits = vec![Limit {
        label: "Monthly plan".into(),
        used_pct: pct,
        resets_in_ms: ends_ms.map(|r| r - now),
        status: status_for(pct),
    }];
    Ok(Report {
        provider: ProviderId::Cursor,
        source: QuotaSource::CursorUsageSummary,
        plan_type: membership_display(summary.membership_type.as_deref()),
        limits,
        subscription_ends_at_ms: ends_ms,
        balance: None,
    })
}

fn parse_iso_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

pub fn fetch_report() -> Result<Report> {
    let token = local_token()?;
    // JWT exp(epoch 秒)过期即拒绝:Cursor 编辑器开着会自行续期,重登一次即可。
    if let Some(exp) = jwt_claim(&token, "exp").and_then(|v| v.as_i64())
        && exp > 0
        && now_ms() / 1000 >= exp
    {
        return Err(anyhow!(
            "Cursor 登录态已过期;启动 Cursor 编辑器或运行 `cursor-agent login` 后刷新"
        ));
    }
    let uid = jwt_claim(&token, "sub")
        .and_then(|v| v.as_str().map(str::to_string))
        .and_then(|sub| {
            sub.split('|')
                .last()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow!("Cursor 登录态缺 sub claim(结构已变化)"))?;
    let cookie = format!("WorkosCursorSessionToken={uid}%3A%3A{token}");
    let response = crate::http::get(USAGE_URL)
        .set("Cookie", &cookie)
        .set("Accept", "application/json")
        .set("User-Agent", "quotakeeper/0.1")
        .timeout(TIMEOUT)
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(401, _) => {
                anyhow!("Cursor 登录态无效;启动 Cursor 编辑器或 `cursor-agent login` 后刷新")
            }
            ureq::Error::Status(403, _) => anyhow!("Cursor 用量不可读;重新登录 Cursor 后刷新"),
            ureq::Error::Status(429, _) => anyhow!("Cursor usage 限流;稍后自动重试"),
            ureq::Error::Status(code, _) => anyhow!("Cursor usage 请求失败: HTTP {code}"),
            ureq::Error::Transport(_) => anyhow!("Cursor usage 连接失败"),
        })?;
    let json = response.into_string()?;
    parse_report(&json, now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b64url_decode_handles_urlsafe_and_padding() {
        assert_eq!(b64url_decode("aGVsbG8=").unwrap(), b"hello");
        // URL-safe 字母表(- _)与无填充
        assert_eq!(b64url_decode("-_8").unwrap(), vec![0xFB, 0xFF]);
        assert!(b64url_decode("!!!").is_none());
    }

    #[test]
    fn jwt_claim_reads_sub_and_exp() {
        // payload = {"sub":"user_abc|01H...|12345","exp":1900000000}
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyX2FiY3wwMUguLi58MTIzNDUiLCJleHAiOjE5MDAwMDAwMDB9.sig";
        assert_eq!(
            jwt_claim(token, "sub").unwrap().as_str().unwrap(),
            "user_abc|01H...|12345"
        );
        assert_eq!(jwt_claim(token, "exp").unwrap().as_i64().unwrap(), 1_900_000_000);
        assert!(jwt_claim("not-a-jwt", "sub").is_none());
    }

    #[test]
    fn utf16le_blob_and_plaintext_decode() {
        // "eyJ" 的 UTF-16LE(无 BOM,奇偶零填充特征)
        let utf16: Vec<u8> = "eyJhbGciOiJ"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            decode_utf16_or_utf8(&utf16).unwrap().trim_end_matches('\0'),
            "eyJhbGciOiJ"
        );
        assert_eq!(
            decode_utf16_or_utf8(b"eyJhbGciOiJ").unwrap(),
            "eyJhbGciOiJ"
        );
        // 带 BOM
        let mut bom = vec![0xFF, 0xFE];
        bom.extend("tok".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode_utf16_or_utf8(&bom).unwrap(), "tok");
    }

    #[test]
    fn normalize_token_unwraps_json_and_accepts_bare_jwt() {
        assert_eq!(
            normalize_token("{\"accessToken\":\"eyJabc.def.ghi\"}").unwrap(),
            "eyJabc.def.ghi"
        );
        assert_eq!(normalize_token("  eyJabc.def.ghi ").unwrap(), "eyJabc.def.ghi");
        assert!(normalize_token("garbage").is_none());
    }

    #[test]
    fn usage_summary_parses_billing_cycle_and_percent() {
        let json = r#"{
            "billingCycleStart": "2026-09-01T00:00:00.000Z",
            "billingCycleEnd": "2026-10-01T00:00:00.000Z",
            "membershipType": "pro",
            "individualUsage": {"plan": {"totalPercentUsed": 44.0}}
        }"#;
        let now = chrono::DateTime::parse_from_rfc3339("2026-09-29T00:00:00Z")
            .unwrap()
            .timestamp_millis();
        let report = parse_report(json, now).unwrap();
        assert_eq!(report.provider, ProviderId::Cursor);
        assert_eq!(report.plan_type, "Cursor Pro");
        assert_eq!(report.limits.len(), 1);
        assert!((report.limits[0].used_pct - 44.0).abs() < f64::EPSILON);
        // 2026-09-29 → 2026-10-01 恰好 2 天
        let two_days_ms = 2 * 86_400_000;
        assert_eq!(report.limits[0].resets_in_ms, Some(two_days_ms));
        assert_eq!(report.subscription_ends_at_ms, Some(now + two_days_ms));
    }

    #[test]
    fn usage_summary_without_plan_percent_is_an_error() {
        assert!(parse_report(r#"{"membershipType":"free"}"#, 0).is_err());
        assert!(parse_report(
            r#"{"individualUsage": {"plan": {"totalPercentUsed": 500.0}}}"#,
            0
        )
        .is_err());
    }

    #[test]
    fn membership_display_maps_tiers() {
        assert_eq!(membership_display(Some("pro")), "Cursor Pro");
        assert_eq!(membership_display(Some("ultra")), "Cursor Ultra");
        assert_eq!(membership_display(Some("teams")), "Cursor Teams");
        assert_eq!(membership_display(Some("free")), "Cursor Free");
        assert_eq!(membership_display(Some("")), "Cursor");
        assert_eq!(membership_display(None), "Cursor");
        assert_eq!(membership_display(Some("custom")), "custom");
    }
}
