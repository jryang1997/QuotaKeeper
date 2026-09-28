//! First-party Codex app-server `account/rateLimits/read` adapter.
//! Only reads account state; Codex owns OAuth refresh and credential storage.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::model::{Limit, ProviderId, QuotaSource, Report, status_for};

const TIMEOUT: Duration = Duration::from_secs(15);

fn send(stdin: &mut impl Write, value: Value) -> Result<()> {
    serde_json::to_writer(&mut *stdin, &value)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn response(rx: &mpsc::Receiver<Value>, id: i64, deadline: Instant) -> Result<Value> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(anyhow!("Codex app-server 超时"));
        }
        let value = rx
            .recv_timeout(remaining)
            .map_err(|_| anyhow!("Codex app-server 未响应"))?;
        if value["id"].as_i64() != Some(id) {
            continue;
        }
        if value.get("error").is_some() {
            return Err(anyhow!("Codex app-server 额度请求失败"));
        }
        return value
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow!("Codex app-server 响应缺少 result"));
    }
}

fn window(value: &Value, fallback: &str, now_ms: i64) -> Result<Option<Limit>> {
    if value.is_null() {
        return Ok(None);
    }
    let used = value["usedPercent"]
        .as_f64()
        .ok_or_else(|| anyhow!("Codex app-server usedPercent 缺失"))?;
    if !used.is_finite() || !(0.0..=100.0).contains(&used) {
        return Err(anyhow!("Codex app-server usedPercent 无效"));
    }
    let minutes = value["windowDurationMins"].as_u64();
    let label = match minutes {
        Some(300) => "5 hours".to_string(),
        Some(10_080) => "7 days".to_string(),
        Some(value) => format!("{value} min"),
        None => fallback.to_string(),
    };
    let resets_in_ms = value["resetsAt"]
        .as_i64()
        .map(|seconds| seconds.saturating_mul(1000).saturating_sub(now_ms));
    Ok(Some(Limit {
        label,
        used_pct: used,
        resets_in_ms,
        status: status_for(used),
    }))
}

fn parse_report(account: &Value, rates: &Value, now_ms: i64) -> Result<Report> {
    let bucket = rates
        .pointer("/rateLimitsByLimitId/codex")
        .filter(|value| value.is_object())
        .or_else(|| rates.get("rateLimits").filter(|value| value.is_object()))
        .ok_or_else(|| anyhow!("Codex app-server 未提供 Codex 额度"))?;
    let mut limits = Vec::new();
    if let Some(limit) = window(&bucket["primary"], "Primary", now_ms)? {
        limits.push(limit);
    }
    if let Some(limit) = window(&bucket["secondary"], "Secondary", now_ms)? {
        limits.push(limit);
    }
    if limits.is_empty() {
        return Err(anyhow!("Codex app-server 无可用额度窗口"));
    }
    let plan = bucket["planType"]
        .as_str()
        .or_else(|| account.pointer("/account/planType").and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .unwrap_or("Codex access");
    Ok(Report {
        provider: ProviderId::Codex,
        source: QuotaSource::CodexAppServer,
        plan_type: plan.to_string(),
        limits,
    })
}

pub fn fetch_report() -> Result<Report> {
    let mut candidates = vec!["codex".to_string()];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(format!("{local}\\Programs\\OpenAI\\Codex\\bin\\codex.exe"));
    }
    let mut child = candidates
        .into_iter()
        .find_map(|program| {
            Command::new(program)
                .arg("app-server")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()
        })
        .ok_or_else(|| anyhow!("Codex 官方 app-server 未安装或无法启动"))?;
    let result = (|| -> Result<Report> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Codex stdout 不可用"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Codex stdin 不可用"))?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if let Ok(value) = serde_json::from_str::<Value>(&line)
                    && tx.send(value).is_err()
                {
                    break;
                }
            }
        });
        let deadline = Instant::now() + TIMEOUT;
        send(
            &mut stdin,
            json!({"method":"initialize","id":0,"params":{"clientInfo":{"name":"tui_token_monitor","title":"Token Monitor","version":env!("CARGO_PKG_VERSION")}}}),
        )?;
        response(&rx, 0, deadline)?;
        send(&mut stdin, json!({"method":"initialized","params":{}}))?;
        send(
            &mut stdin,
            json!({"method":"account/read","id":1,"params":{"refreshToken":false}}),
        )?;
        let account = response(&rx, 1, deadline)?;
        send(
            &mut stdin,
            json!({"method":"account/rateLimits/read","id":2}),
        )?;
        let rates = response(&rx, 2, deadline)?;
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        parse_report(&account, &rates, now_ms)
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_bucket_response_uses_codex_and_real_durations() {
        let account = json!({"account":{"type":"chatgpt","planType":"pro"}});
        let rates = json!({
            "rateLimits":{"primary":{"usedPercent":99,"windowDurationMins":15,"resetsAt":1000}},
            "rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":25,"windowDurationMins":300,"resetsAt":1000},"secondary":null}}
        });
        let report = parse_report(&account, &rates, 0).unwrap();
        assert_eq!(report.source, QuotaSource::CodexAppServer);
        assert_eq!(report.plan_type, "pro");
        assert_eq!(report.limits.len(), 1);
        assert_eq!(report.limits[0].label, "5 hours");
        assert_eq!(report.limits[0].used_pct, 25.0);
    }

    #[test]
    fn missing_windows_are_not_invented() {
        let account = json!({});
        let rates = json!({"rateLimits":{"primary":null,"secondary":null}});
        assert!(parse_report(&account, &rates, 0).is_err());
    }

    #[test]
    #[ignore = "requires a locally signed-in Codex installation"]
    fn installed_codex_app_server_returns_real_limits() {
        let report = fetch_report().unwrap();
        assert_eq!(report.source, QuotaSource::CodexAppServer);
        assert!(!report.limits.is_empty());
    }
}
