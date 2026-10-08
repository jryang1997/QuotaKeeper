//! MiniMax 编程套餐(Coding Plan)余量采集:`sk-cp-` 密钥调官方 remains 端点.
//!
//! 形状依据(2026-10-08,三个独立实现互相印证):
//! - cc-switch `coding_plan.rs`:只取 `model_name == "general"`;5h 窗口用
//!   `current_interval_remaining_percent` + `end_time`;周窗口仅 `current_weekly_status == 1` 时启用.
//! - opencodex / openchamber:同一端点同一字段.openchamber 文档注明旧端点的
//!   `*_usage_count` 是已用量、新 token_plan 端点语义相反,所以本卡只读百分比字段,不读计数字段.
//! - 国内站 `api.minimaxi.com`(默认),国际站 `api.minimax.io` 经 MINIMAX_API_BASE 指定.
//!
//! 约束:key 只读内存送 Authorization 头,永不打印/落盘;非 https 的自定义 base 一律拒绝.
//! 端点只给窗口重置时间,没有套餐到期日,卡面如实不留订阅期.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::model::{Limit, ProviderId, QuotaSource, Report, status_for};

const DEFAULT_BASE: &str = "https://api.minimaxi.com";
const REMAINS_PATH: &str = "/v1/api/openplatform/coding_plan/remains";
const CODING_MODEL: &str = "general";
const TIMEOUT: Duration = Duration::from_secs(15);

fn api_key() -> anyhow::Result<String> {
    if let Some(key) = crate::credential::minimax_key().ok().flatten()
        && crate::credential::valid_api_key(&key)
    {
        return Ok(key);
    }
    if let Ok(key) = std::env::var("MINIMAX_CODING_API_KEY")
        && crate::credential::valid_api_key(&key)
    {
        return Ok(key);
    }
    anyhow::bail!("MiniMax 未配置:在凭据管理器添加 TuiTokenMonitor/MiniMax,或设置 MINIMAX_CODING_API_KEY")
}

fn api_base() -> anyhow::Result<String> {
    let base = std::env::var("MINIMAX_API_BASE").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let base = base.trim_end_matches('/').to_string();
    if !base.starts_with("https://") {
        anyhow::bail!("MINIMAX_API_BASE 必须是 https:// 地址(保护 key 不走明文)");
    }
    Ok(base)
}

#[derive(Deserialize, Default)]
struct RemainsResp {
    #[serde(default)]
    base_resp: Option<BaseResp>,
    #[serde(default)]
    model_remains: Vec<ModelRemain>,
}

#[derive(Deserialize, Default)]
struct BaseResp {
    #[serde(default)]
    status_code: Option<i64>,
    #[serde(default)]
    status_msg: Option<String>,
}

#[derive(Deserialize, Default)]
struct ModelRemain {
    #[serde(default)]
    model_name: String,
    #[serde(default)]
    current_interval_remaining_percent: Option<f64>,
    #[serde(default)]
    end_time: Option<i64>,
    #[serde(default)]
    current_weekly_status: Option<i64>,
    #[serde(default)]
    current_weekly_remaining_percent: Option<f64>,
    #[serde(default)]
    weekly_end_time: Option<i64>,
}

fn window(label: &str, remaining_pct: f64, resets_at_ms: Option<i64>, now_ms: i64) -> Limit {
    let used_pct = (100.0 - remaining_pct).clamp(0.0, 100.0);
    Limit {
        label: label.into(),
        used_pct,
        resets_in_ms: resets_at_ms.map(|t| (t - now_ms).max(0)),
        status: status_for(used_pct),
    }
}

/// remains 响应 → 卡面报告.业务码非 0、缺 general 条目、无可用窗口一律 Err.
fn report_from(body: RemainsResp, now_ms: i64) -> anyhow::Result<Report> {
    if let Some(base) = body.base_resp.as_ref()
        && base.status_code.unwrap_or(-1) != 0
    {
        anyhow::bail!(
            "MiniMax remains 返回失败:code={:?} msg={}",
            base.status_code,
            base.status_msg.as_deref().unwrap_or("n/a")
        );
    }
    let coding = body
        .model_remains
        .iter()
        .find(|row| row.model_name == CODING_MODEL)
        .ok_or_else(|| anyhow::anyhow!("MiniMax remains 缺少 general 编程套餐条目"))?;
    let mut limits = Vec::new();
    if let Some(remaining) = coding.current_interval_remaining_percent.filter(|v| v.is_finite()) {
        limits.push(window("5 hours", remaining, coding.end_time, now_ms));
    }
    if coding.current_weekly_status == Some(1)
        && let Some(remaining) = coding.current_weekly_remaining_percent.filter(|v| v.is_finite())
    {
        limits.push(window("7 days", remaining, coding.weekly_end_time, now_ms));
    }
    if limits.is_empty() {
        anyhow::bail!("MiniMax remains 无可用窗口数据");
    }
    Ok(Report {
        provider: ProviderId::MiniMax,
        source: QuotaSource::MinimaxRemainsApi,
        plan_type: "MiniMax Coding Plan".into(),
        limits,
        subscription_ends_at_ms: None,
        balance: None,
    })
}

/// 拉一次 MiniMax 编程套餐快照.无 key/断网/业务失败一律 Err,调用方转 Missing 卡.
pub fn fetch_report() -> anyhow::Result<Report> {
    let key = api_key()?;
    let base = api_base()?;
    let body: RemainsResp = crate::http::get(&format!("{base}{REMAINS_PATH}"))
        .set("Authorization", &format!("Bearer {key}"))
        .set("Accept", "application/json")
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| anyhow::anyhow!("MiniMax remains 请求失败:{e}"))?
        .into_json()
        .map_err(|e| anyhow::anyhow!("MiniMax remains 解析失败:{e}"))?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    report_from(body, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    const END_MS: i64 = 1_780_329_600_000;
    const WEEK_END_MS: i64 = 1_780_848_000_000;

    fn parse(json: &str, now_ms: i64) -> anyhow::Result<Report> {
        report_from(serde_json::from_str(json).unwrap(), now_ms)
    }

    #[test]
    fn general_row_maps_to_five_hour_and_weekly_windows() {
        let json = format!(
            r#"{{
            "model_remains":[
                {{"model_name":"video","current_interval_remaining_percent":50.0}},
                {{"model_name":"general","current_interval_remaining_percent":98.0,
                  "current_weekly_remaining_percent":95.0,"current_interval_status":1,
                  "current_weekly_status":1,"end_time":{END_MS},"weekly_end_time":{WEEK_END_MS}}}
            ],
            "base_resp":{{"status_code":0,"status_msg":"success"}}
        }}"#
        );
        let report = parse(&json, END_MS - 2 * 3_600_000).unwrap();
        assert_eq!(report.provider, ProviderId::MiniMax);
        assert_eq!(report.limits.len(), 2);
        assert_eq!(report.limits[0].label, "5 hours");
        assert!((report.limits[0].used_pct - 2.0).abs() < 1e-9);
        assert_eq!(report.limits[0].resets_in_ms, Some(2 * 3_600_000));
        assert_eq!(report.limits[1].label, "7 days");
        assert!((report.limits[1].used_pct - 5.0).abs() < 1e-9);
        assert_eq!(report.subscription_ends_at_ms, None);
    }

    #[test]
    fn weekly_window_is_skipped_when_plan_has_no_weekly_limit() {
        let json = r#"{"model_remains":[{"model_name":"general",
            "current_interval_remaining_percent":60.0,"current_weekly_status":3,
            "current_weekly_remaining_percent":100.0}]}"#;
        let report = parse(json, 0).unwrap();
        assert_eq!(report.limits.len(), 1);
        assert_eq!(report.limits[0].label, "5 hours");
    }

    #[test]
    fn remaining_percent_is_the_only_source_of_usage_not_count_fields() {
        let json = r#"{"model_remains":[{"model_name":"general",
            "current_interval_total_count":1500,"current_interval_usage_count":1200,
            "current_interval_remaining_percent":60.0}]}"#;
        let report = parse(json, 0).unwrap();
        assert!((report.limits[0].used_pct - 40.0).abs() < 1e-9);
    }

    #[test]
    fn out_of_range_percent_is_clamped_and_exhaustion_is_flagged() {
        let json = r#"{"model_remains":[{"model_name":"general",
            "current_interval_remaining_percent":-5.0}]}"#;
        let report = parse(json, 0).unwrap();
        assert!((report.limits[0].used_pct - 100.0).abs() < 1e-9);
        assert_eq!(report.limits[0].status, Status::Exhausted);
    }

    #[test]
    fn business_error_inside_http_200_is_an_error() {
        let json = r#"{"base_resp":{"status_code":1004,"status_msg":"login fail"},
            "model_remains":[{"model_name":"general","current_interval_remaining_percent":90.0}]}"#;
        let err = parse(json, 0).unwrap_err().to_string();
        assert!(err.contains("1004") && err.contains("login fail"));
    }

    #[test]
    fn missing_general_row_or_windows_is_an_error() {
        let only_video = r#"{"model_remains":[{"model_name":"video",
            "current_interval_remaining_percent":50.0}]}"#;
        assert!(parse(only_video, 0).is_err());

        let no_percent = r#"{"model_remains":[{"model_name":"general"}]}"#;
        assert!(parse(no_percent, 0).is_err());
    }
}
