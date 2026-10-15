//! IPC 数据契约:LedgerReport / Snapshot → 前端 JSON。
//!
//! 只做序列化,不改口径。字段名 camelCase 直接喂 React;
//! 金额/百分比保持 f64,token 数保持 i64,时间一律 epoch 毫秒。

use serde::Serialize;

use quotakeeper::deepseek_balance::BalanceState;
use quotakeeper::ledger::{self, Agg};
use quotakeeper::model::{self, Card, Snapshot};

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
    pub model: String,
    pub raw_model: String,
    pub agg: AggDto,
    pub unpriced_messages: u64,
}

#[derive(Serialize)]
pub struct IntradayRowDto {
    pub date: String,
    pub hour: u8,
    pub client: &'static str,
    pub model: String,
    pub raw_model: String,
    pub agg: AggDto,
    pub unpriced_messages: u64,
}

#[derive(Serialize)]
pub struct LedgerDto {
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
                model: row.model.clone(),
                raw_model: row.raw_model.clone(),
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
                model: row.model.clone(),
                raw_model: row.raw_model.clone(),
                agg: agg_dto(&row.agg),
                unpriced_messages: row.unpriced_messages,
            })
            .collect(),
        generated_at_ms: r.generated_at_ms,
    }
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
    fn ledger_payload_keeps_model_names_and_both_cost_parts() {
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
