//! IPC 数据契约:LedgerReport / Snapshot → 前端 JSON。
//!
//! 只做序列化,不改口径。字段名 camelCase 直接喂 React;
//! 金额/百分比保持 f64,token 数保持 i64,时间一律 epoch 毫秒。

use serde::Serialize;
use serde_json::Value;

use quotakeeper::deepseek_balance::BalanceState;
use quotakeeper::ledger::{self, Agg};
use quotakeeper::model::{self, Card, Snapshot};

pub const LEDGER_SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
pub struct AggDto {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
    pub messages: u64,
    pub cost: f64,
    pub cost_reported: f64,
    pub cost_estimated: f64,
    /// 成本是否有非估价来源(纯估价时前端打 ~ 号)。
    pub cost_is_estimate: bool,
    #[serde(rename = "total")]
    pub total: i64,
}

pub fn agg_dto(a: &Agg) -> AggDto {
    AggDto {
        input: a.input,
        output: a.output,
        cache_read: a.cache_read,
        cache_write: a.cache_write,
        reasoning: a.reasoning,
        messages: a.messages,
        cost: a.cost(),
        cost_reported: a.cost_reported,
        cost_estimated: a.cost_estimated,
        cost_is_estimate: a.cost_reported == 0.0 && a.cost_estimated > 0.0,
        total: a.total(),
    }
}

#[derive(Serialize)]
pub struct LimitDto {
    pub label: String,
    pub used_pct: f64,
    pub resets_in_ms: Option<i64>,
    /// "ok" | "warning" | "exhausted"
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct BalanceDto {
    pub amount: f64,
    pub currency: String,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
pub enum CardDto {
    Live {
        provider: &'static str,
        title: &'static str,
        plan_type: String,
        source: &'static str,
        method: &'static str,
        /// 订阅周期结束(续费/到期)时间;None = 该来源不提供.
        #[serde(skip_serializing_if = "Option::is_none")]
        subscription_ends_at_ms: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        balance: Option<BalanceDto>,
        limits: Vec<LimitDto>,
    },
    Missing {
        provider: &'static str,
        title: &'static str,
        hints: [&'static str; 2],
        source: &'static str,
        method: &'static str,
    },
}

fn status_name(s: model::Status) -> &'static str {
    match s {
        model::Status::Ok => "ok",
        model::Status::Warning => "warning",
        model::Status::Exhausted => "exhausted",
    }
}

pub fn cards(snap: &Snapshot) -> Vec<CardDto> {
    snap.cards
        .iter()
        .map(|c| match c {
            Card::Live(r) => CardDto::Live {
                provider: provider_slug(r.provider),
                title: r.provider.title(),
                plan_type: r.plan_type.clone(),
                source: r.source.label(),
                method: match r.provider {
                    model::ProviderId::OpenCodeGo | model::ProviderId::Codex => "provider_fraction",
                    model::ProviderId::Goat => "goat_credits",
                    model::ProviderId::Antigravity
                        if r.source == model::QuotaSource::CpaGoogleQuota =>
                    {
                        "provider_fraction"
                    }
                    model::ProviderId::Antigravity => "activity_vs_average",
                    model::ProviderId::Claude
                    | model::ProviderId::Cursor
                    | model::ProviderId::Glm
                    | model::ProviderId::Kimi
                    | model::ProviderId::MiniMax => "provider_fraction",
                },
                subscription_ends_at_ms: r.subscription_ends_at_ms,
                balance: r.balance.as_ref().map(|b| BalanceDto {
                    amount: b.amount,
                    currency: b.currency.clone(),
                }),
                limits: r
                    .limits
                    .iter()
                    .map(|l| LimitDto {
                        label: l.label.clone(),
                        used_pct: l.used_pct,
                        resets_in_ms: l.resets_in_ms,
                        status: status_name(l.status),
                    })
                    .collect(),
            },
            Card::Missing { provider } => CardDto::Missing {
                provider: provider_slug(*provider),
                title: provider.title(),
                hints: provider.missing_hints(),
                source: match provider {
                    model::ProviderId::OpenCodeGo => "OpenCode Go 官方 usage API · 可选 OMP 兜底",
                    model::ProviderId::Codex => "Codex 官方 app-server · 可选旧接口/OMP 兜底",
                    model::ProviderId::Goat => "Command Code billing API",
                    model::ProviderId::Antigravity => "CPA 管理接口 · 可代查 Google 上游配额",
                    model::ProviderId::Claude => "Claude 官方 oauth/usage（Claude Code 登录态）",
                    model::ProviderId::Cursor => "Cursor usage-summary（Cursor 登录态）",
                    model::ProviderId::Glm => "z.ai / bigmodel 官方 monitor 接口",
                    model::ProviderId::Kimi => "Kimi 官方 coding usage 接口",
                    model::ProviderId::MiniMax => "MiniMax 官方 coding_plan remains 接口",
                },
                method: "unavailable",
            },
        })
        .collect()
}

fn provider_slug(p: model::ProviderId) -> &'static str {
    match p {
        model::ProviderId::OpenCodeGo => "opencode-go",
        model::ProviderId::Codex => "codex",
        model::ProviderId::Goat => "goat",
        model::ProviderId::Antigravity => "antigravity",
        model::ProviderId::Claude => "claude",
        model::ProviderId::Cursor => "cursor",
        model::ProviderId::Glm => "glm",
        model::ProviderId::Kimi => "kimi",
        model::ProviderId::MiniMax => "minimax",
    }
}

#[derive(Serialize)]
pub struct SourceStatusDto {
    pub client: &'static str,
    pub label: &'static str,
    pub records: usize,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct AnalysisRowDto {
    pub date: String,
    pub client: &'static str,
    pub project: Option<String>,
    pub model: String,
    pub raw_model: String,
    pub price_basis_id: usize,
    pub agg: AggDto,
    pub unpriced_messages: u64,
}

#[derive(Serialize)]
pub struct IntradayRowDto {
    pub date: String,
    pub hour: u8,
    pub client: &'static str,
    pub project: Option<String>,
    pub model: String,
    pub raw_model: String,
    pub price_basis_id: usize,
    pub agg: AggDto,
    pub unpriced_messages: u64,
}

#[derive(Serialize)]
pub struct PriceBasisDto {
    pub id: usize,
    pub basis: ledger::pricing::PriceBasis,
}

#[derive(Serialize)]
pub struct LedgerDto {
    pub schema_version: u32,
    pub report_id: String,
    pub timezone_offset: String,
    pub price_snapshot: ledger::pricing::PriceSnapshot,
    pub price_bases: Vec<PriceBasisDto>,
    pub sources: Vec<SourceStatusDto>,
    pub today: AggDto,
    pub week: AggDto,
    pub month: AggDto,
    pub analysis_rows: Vec<AnalysisRowDto>,
    pub intraday_rows: Vec<IntradayRowDto>,
    pub generated_at_ms: i64,
}

/// 顶层推送负载:一次刷新的全部数据,前端按页取用。
#[derive(Serialize)]
pub struct PushDto {
    pub fetched_at_ms: u64,
    pub quota_fetched_at_ms: u64,
    pub quota_checked_at_ms: u64,
    #[serde(rename = "quotaError")]
    /// quota 拉取失败原因;None = 正常。
    pub quota_error: Option<String>,
    pub cards: Vec<CardDto>,
    pub deepseek_balance: BalanceState,
    pub ledger: LedgerDto,
}


pub fn ledger_dto(r: &ledger::LedgerReport) -> LedgerDto {
    LedgerDto {
        schema_version: LEDGER_SCHEMA_VERSION,
        report_id: r.report_id.clone(),
        timezone_offset: r.timezone_offset.clone(),
        price_snapshot: r.price_snapshot.clone(),
        price_bases: r
            .price_bases
            .iter()
            .enumerate()
            .map(|(id, basis)| PriceBasisDto {
                id,
                basis: basis.clone(),
            })
            .collect(),
        sources: r
            .sources
            .iter()
            .map(|s| SourceStatusDto {
                client: s.client,
                label: ledger::types::client_label(s.client),
                records: s.records,
                error: s.error.clone(),
            })
            .collect(),
        today: agg_dto(&r.today),
        week: agg_dto(&r.week),
        month: agg_dto(&r.month),
        analysis_rows: r
            .analysis_rows
            .iter()
            .map(|row| AnalysisRowDto {
                date: row.date.clone(),
                client: row.client,
                project: row.project.clone(),
                model: row.model.clone(),
                raw_model: row.raw_model.clone(),
                price_basis_id: row.price_basis_id,
                agg: agg_dto(&row.agg),
                unpriced_messages: row.unpriced_messages,
            })
            .collect(),
        intraday_rows: r
            .intraday_rows
            .iter()
            .map(|row| IntradayRowDto {
                date: row.date.clone(),
                hour: row.hour,
                client: row.client,
                project: row.project.clone(),
                model: row.model.clone(),
                raw_model: row.raw_model.clone(),
                price_basis_id: row.price_basis_id,
                agg: agg_dto(&row.agg),
                unpriced_messages: row.unpriced_messages,
            })
            .collect(),
        generated_at_ms: r.generated_at_ms,
    }
}

fn compatible_rate(value: &Value) -> bool {
    value["usd_per_token"]
        .as_f64()
        .is_some_and(|rate| rate.is_finite() && rate >= 0.0)
        && value["entry"].is_string()
        && value["origin"]
            .as_str()
            .is_some_and(|origin| matches!(origin, "upstream" | "supplement" | "legacy_unknown"))
}

fn compatible_estimate(value: &Value) -> bool {
    value["matched_entry"].is_string()
        && value["match_method"].as_str().is_some_and(|method| {
            matches!(
                method,
                "exact" | "case_insensitive" | "explicit_alias" | "canonical"
                    | "official_reference" | "reverse_alias" | "unique_suffix" | "majority_reference"
            )
        })
        && value["family_fallback"].is_boolean()
        && value["cache_fill"].is_boolean()
        && ["input", "output"].iter().all(|key| compatible_rate(&value[key]))
        && ["cache_read", "cache_write"].iter().all(|key| {
            value
                .get(key)
                .is_some_and(|rate| rate.is_null() || compatible_rate(rate))
        })
}

fn compatible_basis(value: &Value) -> bool {
    match value["kind"].as_str() {
        Some("reported") => true,
        Some("estimated") => compatible_estimate(value),
        Some("unpriced") => {
            value["reason"].as_str().is_some_and(|reason| {
                matches!(
                    reason,
                    "unknown_model" | "ambiguous_name" | "ambiguous_suffix" | "missing_cache_read"
                        | "missing_cache_write" | "missing_cache_read_and_write" | "non_finite_amount"
                )
            }) && value
                .get("attempted")
                .is_some_and(|attempted| attempted.is_null() || compatible_estimate(attempted))
        }
        _ => false,
    }
}

pub fn compatible_ledger(value: &Value) -> bool {
    if value["schema_version"].as_u64() != Some(LEDGER_SCHEMA_VERSION.into())
        || !value["report_id"].as_str().is_some_and(|id| !id.is_empty())
        || !value["timezone_offset"]
            .as_str()
            .is_some_and(|offset| !offset.is_empty())
        || value["generated_at_ms"].as_i64().is_none()
    {
        return false;
    }
    let snapshot = &value["price_snapshot"];
    if !snapshot["version"].as_str().is_some_and(|version| !version.is_empty())
        || snapshot["loaded_at_ms"].as_i64().is_none()
        || !snapshot
            .get("fetched_at_ms")
            .is_some_and(|time| time.is_null() || time.as_i64().is_some())
        || !snapshot
            .get("dataset_url")
            .is_some_and(|url| url.is_null() || url.is_string())
        || !snapshot["source"]
            .as_str()
            .is_some_and(|source| matches!(source, "cache_v2" | "legacy_cache" | "supplements_only"))
        || !snapshot["cache_state"].as_str().is_some_and(|state| {
            matches!(state, "fresh" | "stale" | "unknown_age" | "missing" | "corrupt" | "not_loaded")
        })
    {
        return false;
    }
    let Some(bases) = value["price_bases"].as_array() else {
        return false;
    };
    let mut ids = std::collections::BTreeSet::new();
    for entry in bases {
        let Some(id) = entry["id"].as_u64() else {
            return false;
        };
        if !ids.insert(id) || !compatible_basis(&entry["basis"]) {
            return false;
        }
    }
    let valid_agg = |agg: &Value| {
        ["input", "output", "cache_read", "cache_write", "reasoning", "total"]
            .iter()
            .all(|key| agg[key].as_i64().is_some())
            && agg["messages"].as_u64().is_some()
            && ["cost", "cost_reported", "cost_estimated"]
                .iter()
                .all(|key| agg[key].as_f64().is_some_and(f64::is_finite))
            && agg["cost_is_estimate"].is_boolean()
    };
    let valid_rows = |key: &str, hourly: bool| {
        value[key].as_array().is_some_and(|rows| {
            rows.iter().all(|row| {
                row["date"].is_string() && row["client"].is_string()
                    && row["model"].as_str().is_some_and(|model| !model.is_empty())
                    && row["raw_model"].is_string()
                    && row
                        .get("project")
                        .is_some_and(|project| project.is_null() || project.is_string())
                    && row["price_basis_id"].as_u64().is_some_and(|id| ids.contains(&id))
                    && row["unpriced_messages"].as_u64().is_some()
                    && valid_agg(&row["agg"])
                    && (!hourly || row["hour"].as_u64().is_some_and(|hour| hour < 24))
            })
        })
    };
    ["today", "week", "month"].iter().all(|key| valid_agg(&value[key]))
        && value["sources"].as_array().is_some_and(|sources| {
            sources.iter().all(|source| {
                source["client"].is_string() && source["label"].is_string()
                    && source["records"].as_u64().is_some()
                    && source
                        .get("error")
                        .is_some_and(|error| error.is_null() || error.is_string())
            })
        })
        && valid_rows("analysis_rows", false)
        && valid_rows("intraday_rows", true)
}

pub fn push(
    fetched_at_ms: u64,
    quota_fetched_at_ms: u64,
    quota_checked_at_ms: u64,
    quota_error: Option<String>,
    snap: &Snapshot,
    rep: &ledger::LedgerReport,
    deepseek_balance: BalanceState,
) -> String {
    serde_json::to_string(&PushDto {
        fetched_at_ms,
        quota_fetched_at_ms,
        quota_checked_at_ms,
        quota_error,
        cards: cards(snap),
        deepseek_balance,
        ledger: ledger_dto(rep),
    })
    .unwrap_or_else(|_| {
        r#"{"fetched_at_ms":0,"quota_fetched_at_ms":0,"quota_checked_at_ms":0,"quotaError":"serialize failed","cards":[],"ledger":null}"#.into()
    })
}

#[cfg(test)]
pub(crate) fn fixture_pricing(models: serde_json::Value) -> ledger::pricing::Pricing {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    let home = std::env::temp_dir().join(format!(
        "quotakeeper-gui-prices-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    ));
    let path = ledger::pricing::cache_path(&home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec(&serde_json::json!({
        "schema_version": 2,
        "dataset_url": "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json",
        "fetched_at_ms": 1,
        "models": models,
    })).unwrap()).unwrap();
    let pricing = ledger::pricing::Pricing::load(&home);
    std::fs::remove_dir_all(home).unwrap();
    pricing
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use quotakeeper::ledger::{TokenBreakdown, UsageRecord};

    #[test]
    fn ledger_payload_keeps_models_cost_parts_and_frozen_pricing() {
        let now = chrono::Local
            .with_ymd_and_hms(2026, 9, 25, 12, 0, 0)
            .unwrap();
        let records = [
            ("OpenAI/GPT-5", Some(1.25), false),
            ("openrouter/gpt-5", Some(0.125), true),
            ("", None, false),
        ]
        .map(|(model, cost, cost_is_estimate)| UsageRecord {
            client: "codex",
            model: model.into(),
            ts_ms: now.timestamp_millis(),
            tokens: TokenBreakdown {
                input: 8,
                output: 3,
                cache_read: 2,
                cache_write: 1,
                reasoning: 1,
            },
            cost,
            cost_is_estimate,
            session: None,
            project: Some("alpha".into()),
            duration_ms: 0,
            ttft_ms: 0,
        });
        let pricing = fixture_pricing(serde_json::json!({
            "openrouter/gpt-5": { "input": 0.015625, "output": 0.0, "cache_read": 0.0, "cache_write": 0.0 }
        }));
        let report = ledger::aggregate(records.into(), now, &pricing);
        let wire = serde_json::to_value(ledger_dto(&report)).unwrap();

        assert!(compatible_ledger(&wire));
        assert_eq!(wire["schema_version"], 1);
        assert_eq!(wire["report_id"], report.report_id);
        assert_eq!(wire["timezone_offset"], now.format("%:z").to_string());
        assert_eq!(
            wire["price_snapshot"],
            serde_json::to_value(&report.price_snapshot).unwrap()
        );
        let bases = wire["price_bases"].as_array().unwrap();
        assert_eq!(bases.len(), 3);
        for (id, basis) in report.price_bases.iter().enumerate() {
            assert_eq!(bases[id]["id"], id);
            assert_eq!(bases[id]["basis"], serde_json::to_value(basis).unwrap());
        }
        assert_eq!(wire["today"]["cost_reported"], 1.25);
        assert_eq!(wire["today"]["cost_estimated"], 0.125);
        assert_eq!(wire["today"]["cost"], 1.375);
        assert_eq!(wire["today"]["cost_is_estimate"], false);
        assert_eq!(wire["today"]["total"], 42);
        assert_eq!(wire["today"]["messages"], 3);
        for rows in ["analysis_rows", "intraday_rows"] {
            let rows = wire[rows].as_array().unwrap();
            assert_eq!(rows.len(), 3);
            for (raw, model, reported, estimated, unpriced, is_estimate) in [
                ("OpenAI/GPT-5", "gpt-5", 1.25, 0.0, 0, false),
                ("openrouter/gpt-5", "gpt-5", 0.0, 0.125, 0, true),
                ("", "unknown", 0.0, 0.0, 1, false),
            ] {
                let row = rows.iter().find(|row| row["raw_model"] == raw).unwrap();
                assert_eq!(row["model"], model);
                assert_eq!(row["client"], "codex");
                assert_eq!(row["project"], "alpha");
                let id = row["price_basis_id"].as_u64().unwrap() as usize;
                assert_eq!(bases[id]["id"], id);
                assert_eq!(
                    bases[id]["basis"]["kind"],
                    if unpriced > 0 {
                        "unpriced"
                    } else if is_estimate {
                        "estimated"
                    } else {
                        "reported"
                    }
                );
                assert_eq!(row["unpriced_messages"], unpriced);
                assert_eq!(row["agg"]["input"], 8);
                assert_eq!(row["agg"]["output"], 3);
                assert_eq!(row["agg"]["cache_read"], 2);
                assert_eq!(row["agg"]["cache_write"], 1);
                assert_eq!(row["agg"]["reasoning"], 1);
                assert_eq!(row["agg"]["total"], 14);
                assert_eq!(row["agg"]["messages"], 1);
                assert_eq!(row["agg"]["cost_reported"], reported);
                assert_eq!(row["agg"]["cost_estimated"], estimated);
                assert_eq!(row["agg"]["cost"], reported + estimated);
                assert_eq!(row["agg"]["cost_is_estimate"], is_estimate);
            }
        }
    }
}
