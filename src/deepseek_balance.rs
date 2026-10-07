//! DeepSeek 官方预付费 API 余额。估算只使用同一账户的官方余额快照。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{credential, http, paths};

const URL: &str = "https://api.deepseek.com/user/balance";
const DAY_MS: u64 = 86_400_000;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BalanceState {
    MissingKey,
    Ready { is_available: bool, balances: Vec<CurrencyBalance> },
    Error { message: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct CurrencyBalance {
    pub currency: String,
    pub total: f64,
    pub granted: f64,
    pub topped_up: f64,
    pub daily_spend: Option<f64>,
    pub eta_days: Option<f64>,
}

#[derive(Deserialize)]
struct ApiResponse {
    is_available: bool,
    balance_infos: Vec<ApiBalance>,
}

#[derive(Deserialize)]
struct ApiBalance {
    currency: String,
    total_balance: String,
    granted_balance: String,
    topped_up_balance: String,
}

#[derive(Default, Serialize, Deserialize)]
struct History {
    account: String,
    samples: Vec<Sample>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Sample {
    at_ms: u64,
    currency: String,
    total: f64,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn history_path() -> std::path::PathBuf {
    paths::home_dir().join(".quotakeeper").join("deepseek-balance-history.json")
}

fn amount(value: &str) -> Result<f64, String> {
    let number = value.parse::<f64>().map_err(|_| "余额格式异常".to_string())?;
    if !number.is_finite() || number < 0.0 {
        return Err("余额格式异常".into());
    }
    Ok(number)
}

fn parse(body: ApiResponse) -> Result<Vec<CurrencyBalance>, String> {
    body.balance_infos.into_iter().map(|b| {
        if b.currency != "CNY" && b.currency != "USD" {
            return Err("余额币种暂不支持".into());
        }
        Ok(CurrencyBalance {
            currency: b.currency,
            total: amount(&b.total_balance)?,
            granted: amount(&b.granted_balance)?,
            topped_up: amount(&b.topped_up_balance)?,
            daily_spend: None,
            eta_days: None,
        })
    }).collect()
}

fn update_history(balances: &mut [CurrencyBalance], key: &str, at_ms: u64) {
    let path = history_path();
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let account = format!("{:016x}", hasher.finish());
    let mut history: History = std::fs::read(&path).ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
    if history.account != account {
        history = History { account, samples: vec![] };
    }
    history.samples.retain(|s| at_ms.saturating_sub(s.at_ms) <= 7 * DAY_MS);
    for balance in balances {
        let recent = history.samples.iter().rev().find(|s| s.currency == balance.currency);
        // A refill or grant invalidates the current decline baseline.
        if recent.is_some_and(|s| balance.total > s.total + 0.001) {
            history.samples.retain(|s| s.currency != balance.currency);
        }
        let should_record = history.samples.iter().rev().find(|s| s.currency == balance.currency)
            .is_none_or(|s| at_ms.saturating_sub(s.at_ms) >= 3_600_000 || (s.total - balance.total).abs() >= 0.001);
        if should_record {
            history.samples.push(Sample { at_ms, currency: balance.currency.clone(), total: balance.total });
        }
        let first = history.samples.iter().find(|s| s.currency == balance.currency);
        if let Some(first) = first {
            let span = at_ms.saturating_sub(first.at_ms);
            let used = first.total - balance.total;
            if span >= DAY_MS && used > 0.001 {
                let per_day = used / (span as f64 / DAY_MS as f64);
                balance.daily_spend = Some(per_day);
                balance.eta_days = Some(balance.total / per_day);
            }
        }
    }
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            if let Ok(bytes) = serde_json::to_vec(&history) {
                let _ = std::fs::write(path, bytes);
            }
        }
    }
}

pub fn fetch() -> BalanceState {
    let key = credential::deepseek_key().ok().flatten()
        .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok().filter(|s| credential::valid_api_key(s)));
    let Some(key) = key else { return BalanceState::MissingKey };
    let response = http::get(URL)
        .set("Authorization", &format!("Bearer {key}"))
        .timeout(std::time::Duration::from_secs(12))
        .call();
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(401 | 403, _)) => return BalanceState::Error { message: "API key 无效或没有余额查询权限".into() },
        Err(ureq::Error::Status(429, _)) => return BalanceState::Error { message: "查询频率受限，稍后重试".into() },
        Err(ureq::Error::Status(_, _)) | Err(ureq::Error::Transport(_)) =>
            return BalanceState::Error { message: "余额查询失败，稍后重试".into() },
    };
    let parsed: ApiResponse = match response.into_json() {
        Ok(value) => value,
        Err(_) => return BalanceState::Error { message: "余额响应无法解析".into() },
    };
    let available = parsed.is_available;
    match parse(parsed) {
        Ok(mut balances) => {
            update_history(&mut balances, &key, now_ms());
            BalanceState::Ready { is_available: available, balances }
        }
        Err(message) => BalanceState::Error { message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_official_balance_fields() {
        let body: ApiResponse = serde_json::from_str(r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"32.15","granted_balance":"2.00","topped_up_balance":"30.15"}]}"#).unwrap();
        let rows = parse(body).unwrap();
        assert_eq!(rows[0].total, 32.15);
        assert_eq!(rows[0].topped_up, 30.15);
    }
}
