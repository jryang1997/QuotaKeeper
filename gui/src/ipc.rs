//! IPC 数据契约:LedgerReport / Snapshot → 前端 JSON。
//!
//! 只做序列化,不改口径。字段名 camelCase 直接喂 React;
//! 金额/百分比保持 f64,token 数保持 i64,时间一律 epoch 毫秒。

use serde::Serialize;
use std::collections::HashMap;

use tui_token_monitor::ledger::{self, Agg};
use tui_token_monitor::model::{self, Card, Snapshot};

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
        limits: Vec<LimitDto>,
    },
    Missing {
        provider: &'static str,
        title: &'static str,
        hint: &'static str,
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
    pub by_agent: Vec<AgentRowDto>,
    pub recent_by_agent: Vec<RecentAgentDto>,
    pub last_activity_ms: Option<i64>,
    pub trend_24h: Vec<TrendBucketDto>,
    pub by_day: Vec<DayRowDto>,
    pub daily_totals: Vec<DailyTotalDto>,
    pub daily_detail: HashMap<String, Vec<(&'static str, AggDto)>>,
    pub hourly: Vec<i64>,
    pub weekday: Vec<i64>,
    pub by_session: Vec<SessionRowDto>,
    pub by_project: Vec<ProjectRowDto>,
    pub burn_rate: Option<f64>,
    pub generated_at_ms: i64,
}

/// 顶层推送负载:一次刷新的全部数据,前端按页取用。
#[derive(Serialize)]
pub struct PushDto {
    pub fetched_at_ms: u64,
    #[serde(rename = "quotaError")]
    /// quota 拉取失败原因;None = 正常。
    pub quota_error: Option<String>,
    pub cards: Vec<CardDto>,
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
        hourly: r.hourly.to_vec(),
        weekday: r.weekday.to_vec(),
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
    quota_error: Option<String>,
    snap: &Snapshot,
    rep: &ledger::LedgerReport,
) -> String {
    serde_json::to_string(&PushDto {
        fetched_at_ms,
        quota_error,
        cards: cards(snap),
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
        let s = push(123, None, &snap, &rep);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["cards"].as_array().unwrap().len(), 4);
        assert!(v["ledger"]["today"]["total"].is_i64());
        assert!(v["ledger"]["trend_24h"].as_array().unwrap().len() == 24);
        assert!(v["ledger"]["daily_totals"].as_array().unwrap().len() > 100);
    }

    #[test]
    fn quota_error_flows_through() {
        let snap = model::parse_snapshot(FIXTURE).unwrap();
        let rep = ledger::collect(chrono::Local::now());
        let v: serde_json::Value =
            serde_json::from_str(&push(1, Some("omp down".into()), &snap, &rep)).unwrap();
        assert_eq!(v["quotaError"], "omp down");
    }
}
