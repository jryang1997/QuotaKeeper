//! GLM Coding(智谱 z.ai / bigmodel)配额采集:官方 monitor 接口 + 订阅清单.
//!
//! 形状依据(2026-10-01 逐字段核对,三个独立实现互相印证):
//! - 官方插件 zai-org/zai-coding-plugins `glm-plan-usage`(Apache-2.0):
//!   `GET {base}/api/monitor/usage/quota/limit`;auth 为 `Authorization: <key 原值>`,
//!   带 Bearer 前缀同样被接受(CodexBar zai.js / dsh-provider-balance 均用 Bearer).
//! - dsh-provider-balance(Apache-2.0):`data.limits[]` 中 TOKENS_LIMIT 的 5h 窗口 =
//!   unit:3/number:5,周窗口 = unit:6;`percentage` 为已用百分比,`nextResetTime`
//!   为 epoch 毫秒;TIME_LIMIT 只有 currentValue/usage 绝对值;订阅期
//!   `GET {base}/api/biz/subscription/list` → `data[]{productName,inCurrentPeriod,nextRenewTime}`.
//! - CodexBar zai.js(MIT):planName/planType/packageName/level 兜底链;团队模式需
//!   Bigmodel-Organization/Project 双头,本卡不实现(缺头返回 200+空 data,恰好被空窗口检查拦下).
//!
//! 约束:key 只读内存送 Authorization 头,永不打印/落盘/进日志;非 https 的
//! 自定义 base 一律拒绝(fail-closed,抄 CodexBar).失败降级为 Missing 卡.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::model::{Limit, ProviderId, QuotaSource, Report, status_for};

const DEFAULT_BASE: &str = "https://open.bigmodel.cn";
const TIMEOUT: Duration = Duration::from_secs(15);

fn api_key() -> anyhow::Result<String> {
    if let Some(key) = crate::credential::glm_key().ok().flatten()
        && crate::credential::valid_api_key(&key)
    {
        return Ok(key);
    }
    for name in ["GLM_API_KEY", "Z_AI_API_KEY", "ZHIPU_API_KEY"] {
        if let Ok(k) = std::env::var(name)
            && crate::credential::valid_api_key(&k)
        {
            return Ok(k);
        }
    }
    anyhow::bail!("GLM 未配置:在应用设置保存 GLM Coding key,或设置 GLM_API_KEY")
}

/// 国内默认 open.bigmodel.cn;国际站设 GLM_API_BASE=https://api.z.ai.
fn api_base() -> anyhow::Result<String> {
    let base = std::env::var("GLM_API_BASE")
        .or_else(|_| std::env::var("Z_AI_API_BASE"))
        .unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let base = base.trim_end_matches('/').to_string();
    if !base.starts_with("https://") {
        anyhow::bail!("GLM_API_BASE 必须是 https:// 地址(保护 key 不走明文)");
    }
    Ok(base)
}

fn get_json<T: for<'de> Deserialize<'de>>(base: &str, path: &str, key: &str) -> anyhow::Result<T> {
    let resp = crate::http::get(&format!("{base}{path}"))
        .set("Authorization", &format!("Bearer {key}"))
        .set("Accept", "application/json")
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| anyhow::anyhow!("GLM {path} 请求失败:{e}"))?;
    resp.into_json::<T>()
        .map_err(|e| anyhow::anyhow!("GLM {path} 解析失败:{e}"))
}

#[derive(Deserialize, Default)]
struct QuotaResp {
    #[serde(default)]
    success: Option<bool>,
    #[serde(default)]
    code: Option<i64>,
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    data: Option<QuotaData>,
}

#[derive(Deserialize, Default)]
struct QuotaData {
    #[serde(default)]
    limits: Vec<LimitRow>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default, rename = "planName")]
    plan_name: Option<String>,
    #[serde(default, rename = "planType")]
    plan_type: Option<String>,
    #[serde(default, rename = "packageName")]
    package_name: Option<String>,
}

#[derive(Deserialize, Default)]
struct LimitRow {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    unit: Option<i64>,
    #[serde(default)]
    number: Option<i64>,
    #[serde(default)]
    percentage: Option<f64>,
    #[serde(default, rename = "currentValue")]
    current_value: Option<f64>,
    #[serde(default)]
    usage: Option<f64>,
    #[serde(default, rename = "nextResetTime")]
    next_reset_time: Option<i64>,
}

#[derive(Deserialize, Default)]
struct SubscriptionResp {
    #[serde(default)]
    data: Option<Vec<SubEntry>>,
}

#[derive(Deserialize, Default, Clone)]
struct SubEntry {
    #[serde(default, rename = "productName")]
    product_name: Option<String>,
    #[serde(default, rename = "inCurrentPeriod")]
    in_current_period: Option<bool>,
    #[serde(default, rename = "nextRenewTime")]
    next_renew_time: Option<String>,
}

impl LimitRow {
    /// 已用百分比:percentage 缺失的 token 窗口不编造;TIME_LIMIT 用绝对值现算.
    fn to_limit(&self, now_ms: i64) -> Option<Limit> {
        let used_pct = if let Some(p) = self.percentage.filter(|p| p.is_finite()) {
            p.clamp(0.0, 100.0)
        } else if self.kind == "TIME_LIMIT" {
            let used = self.current_value.unwrap_or(0.0);
            let total = self.usage.unwrap_or(0.0);
            if !total.is_finite() || total <= 0.0 || !used.is_finite() {
                return None;
            }
            (used / total * 100.0).clamp(0.0, 100.0)
        } else {
            return None;
        };
        Some(Limit {
            label: self.label(),
            used_pct,
            resets_in_ms: self.next_reset_time.map(|t| (t - now_ms).max(0)),
            status: status_for(used_pct),
        })
    }

    fn label(&self) -> String {
        if self.kind == "TIME_LIMIT" {
            return "MCP tools".into();
        }
        match (self.unit, self.number) {
            (Some(3), Some(n)) => format!("{n} hours"),
            (Some(6), Some(1) | None) => "7 days".into(),
            (Some(6), Some(n)) => format!("{n} weeks"),
            (Some(1), Some(1)) => "daily".into(),
            (Some(1), Some(n)) => format!("{n} days"),
            (Some(5), Some(n)) => format!("{n} min"),
            _ => "window".into(),
        }
    }
}

/// limits[] → 卡面窗口:5h = unit:3/number:5 的 TOKENS_LIMIT,周 = unit:6;
/// 其余 token 窗口按出现顺序兜底补位(老套餐窗口标注漂移的容错,抄
/// dsh-provider-balance),TIME_LIMIT 作为 MCP 泳道追加在最后.
fn windows(rows: &[LimitRow], now_ms: i64) -> Vec<Limit> {
    let mut session: Option<Limit> = None;
    let mut weekly: Option<Limit> = None;
    let mut extra: Vec<Limit> = Vec::new();
    let mut tools: Vec<Limit> = Vec::new();
    for row in rows {
        let Some(limit) = row.to_limit(now_ms) else {
            continue;
        };
        if row.kind == "TIME_LIMIT" {
            tools.push(limit);
        } else if row.unit == Some(3) && row.number == Some(5) && session.is_none() {
            session = Some(limit);
        } else if row.unit == Some(6) && weekly.is_none() {
            weekly = Some(limit);
        } else {
            extra.push(limit);
        }
    }
    let mut pool = extra.into_iter();
    let session = session.or_else(|| pool.next());
    let weekly = weekly.or_else(|| pool.next());
    let mut limits = Vec::new();
    if let Some(l) = session {
        limits.push(l);
    }
    if let Some(l) = weekly {
        limits.push(l);
    }
    limits.extend(pool);
    limits.extend(tools);
    limits
}

/// plan 展示名:订阅清单 productName 优先,回退 quota 响应的
/// planName/planType/packageName,再回退 data.level(CodexBar 兜底链).
fn plan_name(sub: Option<&SubEntry>, data: &QuotaData) -> String {
    if let Some(name) = sub
        .and_then(|s| s.product_name.as_deref())
        .filter(|s| !s.is_empty())
    {
        return name.to_string();
    }
    for candidate in [&data.plan_name, &data.plan_type, &data.package_name] {
        if let Some(name) = candidate.as_deref().filter(|s| !s.is_empty()) {
            return name.to_string();
        }
    }
    if let Some(level) = data.level.as_deref().filter(|s| !s.is_empty()) {
        return level.to_string();
    }
    "GLM Coding".into()
}

/// nextRenewTime 字符串 → epoch 毫秒;容忍 ISO8601 / "YYYY-MM-DD HH:MM:SS" / 日期.
fn parse_renew_ms(raw: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp_millis());
    }
    chrono::NaiveDate::parse_from_str(raw.get(0..10)?, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(0, 0, 0)?
        .and_utc()
        .timestamp_millis()
        .into()
}

/// 拉一次 GLM Coding 快照.无 key/断网/形状变更一律 Err,调用方转 Missing 卡.
pub fn fetch_report() -> anyhow::Result<Report> {
    let key = api_key()?;
    let base = api_base()?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let quota: QuotaResp = get_json(&base, "/api/monitor/usage/quota/limit", &key)?;
    // dsh 严格要求 success===true;这里放宽为"未显式失败且有 data"即接受.
    if quota.success == Some(false) || quota.data.is_none() {
        anyhow::bail!(
            "GLM quota/limit 返回失败:code={:?} msg={}",
            quota.code.unwrap_or(0),
            quota.msg.as_deref().unwrap_or("n/a")
        );
    }
    let data = quota.data.unwrap_or_default();
    // 订阅清单只提供套餐名与续费日,失败不拖垮配额卡(dsh 同策略).
    let sub_entry = get_json::<SubscriptionResp>(&base, "/api/biz/subscription/list", &key)
        .ok()
        .and_then(|r| r.data)
        .and_then(|list| {
            list.iter()
                .find(|e| e.in_current_period != Some(false))
                .or_else(|| list.first())
                .cloned()
        });
    let limits = windows(&data.limits, now_ms);
    if limits.is_empty() {
        anyhow::bail!("GLM quota/limit 无可用窗口数据");
    }
    let mut report = Report {
        provider: ProviderId::Glm,
        source: QuotaSource::ZaiMonitorApi,
        plan_type: plan_name(sub_entry.as_ref(), &data),
        limits,
        subscription_ends_at_ms: None,
    };
    report.subscription_ends_at_ms = sub_entry
        .as_ref()
        .and_then(|e| e.next_renew_time.as_deref())
        .and_then(parse_renew_ms);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: i64 = 1_790_000_000_000;

    fn row(json: &str) -> LimitRow {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn coding_plan_windows_map_to_session_weekly_and_mcp() {
        let rows: Vec<LimitRow> = serde_json::from_str(
            r#"[
            {"type":"TOKENS_LIMIT","unit":3,"number":5,"percentage":42.5,"nextResetTime":1790003600000},
            {"type":"TOKENS_LIMIT","unit":6,"number":1,"percentage":12.0,"nextResetTime":1790600000000},
            {"type":"TIME_LIMIT","unit":3,"number":5,"currentValue":80,"usage":200,"nextResetTime":1790003600000}
        ]"#,
        )
        .unwrap();
        let limits = windows(&rows, NOW_MS);
        assert_eq!(limits.len(), 3);
        assert_eq!(limits[0].label, "5 hours");
        assert!((limits[0].used_pct - 42.5).abs() < 1e-9);
        assert_eq!(limits[0].resets_in_ms, Some(3_600_000));
        assert_eq!(limits[1].label, "7 days");
        assert_eq!(limits[2].label, "MCP tools");
        assert!((limits[2].used_pct - 40.0).abs() < 1e-9);
    }

    #[test]
    fn unlabeled_windows_fill_session_then_weekly() {
        let rows: Vec<LimitRow> = serde_json::from_str(
            r#"[
            {"type":"TOKENS_LIMIT","percentage":10.0},
            {"type":"TOKENS_LIMIT","percentage":20.0},
            {"type":"TOKENS_LIMIT","percentage":30.0}
        ]"#,
        )
        .unwrap();
        let limits = windows(&rows, NOW_MS);
        assert_eq!(limits.len(), 3);
        assert_eq!(limits[0].label, "window");
        assert!((limits[0].used_pct - 10.0).abs() < 1e-9);
        assert!((limits[1].used_pct - 20.0).abs() < 1e-9);
        assert!((limits[2].used_pct - 30.0).abs() < 1e-9);
    }

    #[test]
    fn percentage_clamps_and_token_window_without_percentage_is_dropped() {
        let rows = vec![
            row(r#"{"type":"TOKENS_LIMIT","unit":3,"number":5,"percentage":140.0}"#),
            row(r#"{"type":"TOKENS_LIMIT","unit":6,"number":1}"#),
        ];
        let limits = windows(&rows, NOW_MS);
        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].status, crate::model::Status::Exhausted);
    }

    #[test]
    fn plan_prefers_subscription_then_quota_fields_then_level() {
        let sub = SubEntry {
            product_name: Some("GLM Coding Max".into()),
            in_current_period: Some(true),
            next_renew_time: Some("2026-10-01 09:17:45".into()),
        };
        let data = QuotaData::default();
        assert_eq!(plan_name(Some(&sub), &data), "GLM Coding Max");
        assert_eq!(
            parse_renew_ms("2026-10-01 09:17:45"),
            Some(1_790_846_265_000)
        );

        let data = QuotaData {
            plan_name: Some("Coding Pro".into()),
            ..Default::default()
        };
        assert_eq!(plan_name(None, &data), "Coding Pro");

        let data = QuotaData {
            level: Some("glm-coding-max".into()),
            ..Default::default()
        };
        assert_eq!(plan_name(None, &data), "glm-coding-max");
        assert_eq!(plan_name(None, &QuotaData::default()), "GLM Coding");
    }

    #[test]
    fn expired_or_future_subscription_entries_are_skipped() {
        let list: Vec<SubEntry> = serde_json::from_str(
            r#"[
            {"productName":"Old Plan","inCurrentPeriod":false,"nextRenewTime":"2026-09-01T00:00:00Z"},
            {"productName":"GLM Coding Max","inCurrentPeriod":true,"nextRenewTime":"2026-10-01T00:00:00Z"}
        ]"#,
        )
        .unwrap();
        let picked = list
            .iter()
            .find(|e| e.in_current_period != Some(false))
            .unwrap();
        assert_eq!(picked.product_name.as_deref(), Some("GLM Coding Max"));
        assert_eq!(
            parse_renew_ms(picked.next_renew_time.as_deref().unwrap()),
            Some(1_790_812_800_000)
        );
    }

    #[test]
    fn renew_date_only_is_parsed_as_utc_midnight() {
        assert_eq!(
            parse_renew_ms("2026-10-01"),
            Some(
                chrono::NaiveDate::from_ymd_opt(2026, 10, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp_millis()
            )
        );
        assert_eq!(parse_renew_ms("nonsense"), None);
    }
}
