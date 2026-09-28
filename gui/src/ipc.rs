//! IPC 数据契约:LedgerReport / Snapshot → 前端 JSON。
//!
//! 只做序列化,不改口径。字段名 camelCase 直接喂 React;
//! 金额/百分比保持 f64,token 数保持 i64,时间一律 epoch 毫秒。

use serde::Serialize;
use std::collections::HashMap;

use tui_token_monitor::goat::UsageSummary;
use tui_token_monitor::ledger::{self, Agg};
use tui_token_monitor::model::{self, Card, Snapshot};
use tui_token_monitor::settings;

#[derive(Serialize)]
pub struct AggDto {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
    pub messages: u64,
    pub cost: f64,
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
#[serde(tag = "kind")]
pub enum CardDto {
    Live {
        provider: &'static str,
        title: &'static str,
        plan_type: String,
        source: &'static str,
        method: &'static str,
        limits: Vec<LimitDto>,
    },
    Missing {
        provider: &'static str,
        title: &'static str,
        hint: &'static str,
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
                },
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
                hint: provider.missing_hint(),
                source: match provider {
                    model::ProviderId::OpenCodeGo => "OpenCode Go 官方 usage API · 可选 OMP 兜底",
                    model::ProviderId::Codex => "Codex 官方 app-server · 可选旧接口/OMP 兜底",
                    model::ProviderId::Goat => "Command Code billing API",
                    model::ProviderId::Antigravity => "CPA 管理接口 · 可代查 Google 上游配额",
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
pub struct AgentRowDto {
    pub client: &'static str,
    pub label: &'static str,
    pub agg: AggDto,
    pub models: Vec<(String, AggDto)>,
}

#[derive(Serialize)]
pub struct ModelRowDto {
    pub model: String,
    /// 合并进该模型的原始记录名(排除 canonical 名自身)。
    pub aliases: Vec<String>,
    pub clients: Vec<&'static str>,
    pub agg: AggDto,
}

#[derive(Serialize)]
pub struct RecentAgentDto {
    pub client: &'static str,
    pub label: &'static str,
    pub agg: AggDto,
    pub last_ts: i64,
}

#[derive(Serialize)]
pub struct TrendBucketDto {
    pub start_ms: i64,
    pub agg: AggDto,
    pub per_client: Vec<(&'static str, AggDto)>,
}

#[derive(Serialize)]
pub struct DayRowDto {
    pub date: String,
    pub agg: AggDto,
    pub per_client: Vec<(&'static str, AggDto)>,
}

#[derive(Serialize)]
pub struct DailyTotalDto {
    pub date: String,
    pub tokens: i64,
    pub cost: f64,
    pub messages: u64,
}

/// 当月按 client 聚合(GUI 套餐卡「MONTHLY」行).
#[derive(Serialize)]
pub struct ClientAggDto {
    pub client: &'static str,
    pub agg: AggDto,
}

/// GOAT 官方当前账期用量;None = 不可用,前端回退本机账本口径.
#[derive(Serialize)]
pub struct GoatUsageDto {
    pub tokens: u64,
    pub requests: u64,
    pub cost: f64,
}

#[derive(Serialize)]
pub struct AnalysisRowDto {
    pub date: String,
    pub client: &'static str,
    pub project: Option<String>,
    pub agg: AggDto,
    pub unpriced_messages: u64,
}

#[derive(Serialize)]
pub struct SessionRowDto {
    pub client: &'static str,
    pub label: &'static str,
    pub session: String,
    pub project: Option<String>,
    pub agg: AggDto,
    pub last_ts: i64,
}

#[derive(Serialize)]
pub struct ProjectRowDto {
    pub project: String,
    pub agg: AggDto,
    pub sessions: usize,
    pub last_ts: i64,
}

#[derive(Serialize)]
pub struct LedgerDto {
    pub sources: Vec<SourceStatusDto>,
    pub today: AggDto,
    pub week: AggDto,
    pub month: AggDto,
    pub month_by_client: Vec<ClientAggDto>,
    pub by_agent: Vec<AgentRowDto>,
    pub by_model: Vec<ModelRowDto>,
    pub recent_by_agent: Vec<RecentAgentDto>,
    pub last_activity_ms: Option<i64>,
    pub trend_24h: Vec<TrendBucketDto>,
    pub by_day: Vec<DayRowDto>,
    pub daily_totals: Vec<DailyTotalDto>,
    pub daily_detail: HashMap<String, Vec<(&'static str, AggDto)>>,
    pub analysis_rows: Vec<AnalysisRowDto>,
    pub hourly: Vec<i64>,
    pub weekday: Vec<i64>,
    pub weekday_hour: [[i64; 24]; 7],
    pub by_session: Vec<SessionRowDto>,
    pub by_project: Vec<ProjectRowDto>,
    pub burn_rate: Option<f64>,
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
    pub goat_usage: Option<GoatUsageDto>,
    pub monthly_value: MonthlyValueDto,
    pub ledger: LedgerDto,
}

#[derive(Serialize)]
pub struct MonthlyValueDto {
    pub month_api_equivalent_usd: f64,
    pub default_list_price_benchmark_usd: f64,
    pub list_price_benchmark_usd: f64,
    pub list_price_multiple: f64,
    pub benchmark_is_custom: bool,
    pub reported_usd: f64,
    pub estimated_usd: f64,
}

fn monthly_value_dto(r: &ledger::LedgerReport) -> MonthlyValueDto {
    let month_api_equivalent_usd = r.month.cost();
    let benchmark = settings::monthly_plan_benchmark();
    MonthlyValueDto {
        month_api_equivalent_usd,
        default_list_price_benchmark_usd: model::LIST_PRICE_BENCHMARK_MONTHLY_USD,
        list_price_benchmark_usd: benchmark.monthly_usd,
        list_price_multiple: model::api_equivalent_vs_monthly_plan_multiple(
            month_api_equivalent_usd,
            benchmark.monthly_usd,
        ),
        benchmark_is_custom: benchmark.is_custom,
        reported_usd: r.month.cost_reported,
        estimated_usd: r.month.cost_estimated,
    }
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
        month_by_client: r
            .month_by_client
            .iter()
            .map(|(client, agg)| ClientAggDto {
                client,
                agg: agg_dto(agg),
            })
            .collect(),
        by_agent: r
            .by_agent
            .iter()
            .map(|a| AgentRowDto {
                client: a.client,
                label: ledger::types::client_label(a.client),
                agg: agg_dto(&a.agg),
                models: a
                    .models
                    .iter()
                    .map(|(m, g)| (m.clone(), agg_dto(g)))
                    .collect(),
            })
            .collect(),
        by_model: r
            .by_model
            .iter()
            .map(|m| ModelRowDto {
                model: m.model.clone(),
                aliases: m
                    .aliases
                    .iter()
                    .filter(|alias| alias.as_str() != m.model.as_str())
                    .cloned()
                    .collect(),
                clients: m
                    .clients
                    .iter()
                    .map(|client| ledger::types::client_label(client))
                    .collect(),
                agg: agg_dto(&m.agg),
            })
            .collect(),
        recent_by_agent: r
            .recent_by_agent
            .iter()
            .map(|a| RecentAgentDto {
                client: a.client,
                label: ledger::types::client_label(a.client),
                agg: agg_dto(&a.agg),
                last_ts: a.last_ts,
            })
            .collect(),
        last_activity_ms: r.last_activity_ms,
        trend_24h: r
            .trend_24h
            .iter()
            .map(|b| TrendBucketDto {
                start_ms: b.start_ms,
                agg: agg_dto(&b.agg),
                per_client: b.per_client.iter().map(|(c, g)| (*c, agg_dto(g))).collect(),
            })
            .collect(),
        by_day: r
            .by_day
            .iter()
            .map(|d| DayRowDto {
                date: d.date.clone(),
                agg: agg_dto(&d.agg),
                per_client: d.per_client.iter().map(|(c, g)| (*c, agg_dto(g))).collect(),
            })
            .collect(),
        daily_totals: r
            .daily_totals
            .iter()
            .map(|d| DailyTotalDto {
                date: d.date.clone(),
                tokens: d.tokens,
                cost: d.cost,
                messages: d.messages,
            })
            .collect(),
        daily_detail: r
            .daily_detail
            .iter()
            .map(|(date, rows)| {
                (
                    date.clone(),
                    rows.iter()
                        .map(|(client, agg)| (*client, agg_dto(agg)))
                        .collect(),
                )
            })
            .collect(),
        analysis_rows: r
            .analysis_rows
            .iter()
            .map(|row| AnalysisRowDto {
                date: row.date.clone(),
                client: row.client,
                project: row.project.clone(),
                agg: agg_dto(&row.agg),
                unpriced_messages: row.unpriced_messages,
            })
            .collect(),
        hourly: r.hourly.to_vec(),
        weekday: r.weekday.to_vec(),
        weekday_hour: r.weekday_hour,
        by_session: r
            .by_session
            .iter()
            .map(|s| SessionRowDto {
                client: s.client,
                label: ledger::types::client_label(s.client),
                session: s.session.clone(),
                project: s.project.clone(),
                agg: agg_dto(&s.agg),
                last_ts: s.last_ts,
            })
            .collect(),
        by_project: r
            .by_project
            .iter()
            .map(|p| ProjectRowDto {
                project: p.project.clone(),
                agg: agg_dto(&p.agg),
                sessions: p.sessions,
                last_ts: p.last_ts,
            })
            .collect(),
        burn_rate: r.burn_rate,
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
    goat_usage: Option<UsageSummary>,
) -> String {
    serde_json::to_string(&PushDto {
        fetched_at_ms,
        quota_fetched_at_ms,
        quota_checked_at_ms,
        quota_error,
        cards: cards(snap),
        goat_usage: goat_usage.map(|u| GoatUsageDto {
            tokens: u.total_tokens,
            requests: u.total_count as u64,
            cost: u.total_cost,
        }),
        monthly_value: monthly_value_dto(rep),
        ledger: ledger_dto(rep),
    })
    .unwrap_or_else(|_| {
        r#"{"fetchedAtMs":0,"quotaError":"serialize failed","cards":[],"ledger":null}"#.into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/usage.json");

    #[test]
    fn push_serializes_full_payload() {
        let snap = model::parse_snapshot(FIXTURE).unwrap();
        let rep = ledger::collect(chrono::Local::now());
        let s = push(123, 120, 122, None, &snap, &rep, None);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["cards"].as_array().unwrap().len(), 4);
        assert_eq!(v["quota_fetched_at_ms"], 120);
        assert_eq!(v["quota_checked_at_ms"], 122);
        assert!(v["goat_usage"].is_null());
        assert!(v["ledger"]["today"]["total"].is_i64());
        assert!(v["ledger"]["trend_24h"].as_array().unwrap().len() == 24);
        assert!(v["ledger"]["daily_totals"].as_array().unwrap().len() > 100);
        let benchmark = settings::monthly_plan_benchmark();
        assert_eq!(
            v["monthly_value"]["default_list_price_benchmark_usd"],
            model::LIST_PRICE_BENCHMARK_MONTHLY_USD
        );
        assert_eq!(
            v["monthly_value"]["list_price_benchmark_usd"],
            benchmark.monthly_usd
        );
        assert_eq!(
            v["monthly_value"]["benchmark_is_custom"],
            benchmark.is_custom
        );
        let month_api_equivalent_usd = v["monthly_value"]["month_api_equivalent_usd"]
            .as_f64()
            .unwrap();
        assert!((month_api_equivalent_usd - rep.month.cost()).abs() < 1e-9);
        let list_price_multiple = v["monthly_value"]["list_price_multiple"].as_f64().unwrap();
        assert!(
            (list_price_multiple
                - model::api_equivalent_vs_monthly_plan_multiple(
                    rep.month.cost(),
                    benchmark.monthly_usd,
                ))
            .abs()
                < 1e-9
        );
        let reported_usd = v["monthly_value"]["reported_usd"].as_f64().unwrap();
        assert!((reported_usd - rep.month.cost_reported).abs() < 1e-9);
        let estimated_usd = v["monthly_value"]["estimated_usd"].as_f64().unwrap();
        assert!((estimated_usd - rep.month.cost_estimated).abs() < 1e-9);
    }

    #[test]
    fn quota_error_flows_through() {
        let snap = model::parse_snapshot(FIXTURE).unwrap();
        let rep = ledger::collect(chrono::Local::now());
        let v: serde_json::Value =
            serde_json::from_str(&push(1, 0, 1, Some("omp down".into()), &snap, &rep, None))
                .unwrap();
        assert_eq!(v["quotaError"], "omp down");
    }

    #[test]
    fn goat_usage_flows_through_as_snake_case() {
        let snap = model::parse_snapshot(FIXTURE).unwrap();
        let rep = ledger::collect(chrono::Local::now());
        let summary = UsageSummary {
            total_count: 5752.0,
            total_tokens: 1_054_688_077,
            total_cost: 59.05,
        };
        let v: serde_json::Value = serde_json::from_str(&push(
            1, 0, 1, None, &snap, &rep, Some(summary),
        ))
        .unwrap();
        assert_eq!(v["goat_usage"]["tokens"], 1_054_688_077_u64);
        assert_eq!(v["goat_usage"]["requests"], 5752);
        assert!((v["goat_usage"]["cost"].as_f64().unwrap() - 59.05).abs() < 1e-9);
    }
}
