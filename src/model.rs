//! Quota 归一化模型:各服务独立采集后合并,保留每张卡的真实来源.
//!
//! 不变量:
//! - `used_pct` 恒为 0–100 百分制(OMP `amount.used` 已是该口径;GOAT `used/cap` 美元口径,见 tests/fixtures/goat.json).
//! - 状态只由 `status_for` 按阈值推导:>=100 exhausted,>=80 warning,其余 ok.
//! - 四家 provider 固定顺序展示,缺失的一律为 `Card::Missing`,保证 UI 永不缺卡.

use serde::Deserialize;

use std::collections::HashMap;

/// 默认比较基准为截至 2026-09-26 的三项基础美元月标价之和。
/// 不是用户实际扣款:GOAT 另收支付手续费,税费及地区/渠道价格也可能不同。
/// Antigravity 经 CPA 使用且没有固定月标价,因此不计入该基数。
pub const LIST_PRICE_BENCHMARK_MONTHLY_USD: f64 = crate::settings::DEFAULT_MONTHLY_PLAN_USD;

/// API 价格等值相对于用户月费基准的倍数。
pub fn api_equivalent_vs_monthly_plan_multiple(
    api_equivalent_usd: f64,
    monthly_plan_usd: f64,
) -> f64 {
    if api_equivalent_usd.is_finite() && monthly_plan_usd.is_finite() && monthly_plan_usd > 0.0 {
        api_equivalent_usd.max(0.0) / monthly_plan_usd
    } else {
        0.0
    }
}

/// 本月已记录的 API 等值金额相对于基础标价参照的倍数。
pub fn api_equivalent_vs_list_price_multiple(api_equivalent_usd: f64) -> f64 {
    api_equivalent_vs_monthly_plan_multiple(api_equivalent_usd, LIST_PRICE_BENCHMARK_MONTHLY_USD)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warning,
    Exhausted,
}

/// 唯一的状态推导入口.阈值抄 OMP usage-cli(80/100).
pub fn status_for(used_pct: f64) -> Status {
    if used_pct >= 100.0 {
        Status::Exhausted
    } else if used_pct >= 80.0 {
        Status::Warning
    } else {
        Status::Ok
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    OpenCodeGo,
    Codex,
    Goat,
    Antigravity,
}

impl ProviderId {
    pub fn from_omp(id: &str) -> Option<Self> {
        match id {
            "opencode-go" => Some(Self::OpenCodeGo),
            "openai-codex" => Some(Self::Codex),
            _ => None,
        }
    }

    /// 固定展示顺序.
    pub fn all() -> [Self; 4] {
        [Self::OpenCodeGo, Self::Codex, Self::Goat, Self::Antigravity]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::OpenCodeGo => "OpenCode Go",
            Self::Codex => "Codex",
            Self::Goat => "Command Code GOAT",
            Self::Antigravity => "Antigravity · CPA",
        }
    }

    pub fn missing_hint(&self) -> &'static str {
        match self {
            Self::OpenCodeGo => {
                "配置 Go API key（应用设置、OPENCODE_API_KEY 或 OpenCode auth.json）；OMP 可作兼容兜底"
            }
            Self::Codex => "未找到可用 Codex 登录态；请登录 Codex CLI 后刷新",
            // OMP 20 个 usage provider 无 command-code(registry.ts),原生 GOAT 通道才有.
            Self::Goat => {
                "配置 GOAT API key（应用设置或 COMMAND_CODE_API_KEY）；也可使用 cmdc 登录态"
            }
            Self::Antigravity => "CPA 未运行或 ~/.tui-token-monitor/cpa-key 未配置",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaSource {
    OpenCodeGoApi,
    CodexAppServer,
    CodexAccount,
    CommandCodeApi,
    CpaGoogleQuota,
    CpaLocal,
    OmpFallback,
}

impl QuotaSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenCodeGoApi => "OpenCode Go usage API · 本应用直连",
            Self::CodexAppServer => "Codex 官方 app-server · 额度读取",
            Self::CodexAccount => "Codex 登录态 → ChatGPT 未公开用量接口 · 兼容兜底",
            Self::CommandCodeApi => "Command Code /alpha/billing · 本应用直连（未文档化接口）",
            Self::CpaGoogleQuota => "CPA 代查 Google 上游配额 · 内部接口",
            Self::CpaLocal => "CPA 本机管理接口；若有 OMP stats.db 则附本机活动数据",
            Self::OmpFallback => "OMP usage --json · 兼容兜底",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Limit {
    pub label: String,
    pub used_pct: f64,
    /// resetsAt - fetchedAt,毫秒;None 表示 OMP 未给 resetsAt.
    pub resets_in_ms: Option<i64>,
    pub status: Status,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub provider: ProviderId,
    pub source: QuotaSource,
    pub plan_type: String,
    pub limits: Vec<Limit>,
}

#[derive(Debug, Clone)]
pub enum Card {
    Live(Report),
    Missing { provider: ProviderId },
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    #[allow(dead_code)]
    pub generated_at_ms: i64,
    pub cards: Vec<Card>,
}

/// `omp usage --json` 的最小子集 schema(只取渲染需要的字段).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OmpDoc {
    #[serde(default)]
    generated_at: i64,
    #[serde(default)]
    reports: Vec<OmpReport>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OmpReport {
    provider: String,
    fetched_at: i64,
    #[serde(default)]
    limits: Vec<OmpLimit>,
    #[serde(default)]
    metadata: OmpMeta,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct OmpMeta {
    #[serde(default)]
    plan_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OmpLimit {
    #[serde(default)]
    label: String,
    #[serde(default)]
    window: OmpWindow,
    #[serde(default)]
    amount: OmpAmount,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct OmpWindow {
    #[serde(default)]
    resets_at: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct OmpAmount {
    #[serde(default)]
    used: f64,
    #[serde(default)]
    used_fraction: Option<f64>,
}

/// 解析 `omp usage --json` 全文为四卡快照.未知 provider 直接丢弃.
pub fn parse_snapshot(json: &str) -> anyhow::Result<Snapshot> {
    let doc: OmpDoc = serde_json::from_str(json)?;
    let mut by_provider: HashMap<ProviderId, Report> = HashMap::new();
    for r in &doc.reports {
        let Some(provider) = ProviderId::from_omp(&r.provider) else {
            continue;
        };
        let limits = r
            .limits
            .iter()
            .map(|l| {
                let used_pct = l
                    .amount
                    .used_fraction
                    .map(|f| f * 100.0)
                    .unwrap_or(l.amount.used)
                    .clamp(0.0, 100.0);
                Limit {
                    label: l.label.clone(),
                    used_pct,
                    resets_in_ms: l.window.resets_at.map(|t| t - r.fetched_at),
                    status: status_for(used_pct),
                }
            })
            .collect();
        by_provider.insert(
            provider,
            Report {
                provider,
                source: QuotaSource::OmpFallback,
                plan_type: r.metadata.plan_type.clone(),
                limits,
            },
        );
    }
    let cards = ProviderId::all()
        .into_iter()
        .map(|p| match by_provider.remove(&p) {
            Some(rep) => Card::Live(rep),
            None => Card::Missing { provider: p },
        })
        .collect();
    Ok(Snapshot {
        generated_at_ms: doc.generated_at,
        cards,
    })
}

/// 倒计时格式化:2d 5h / 5h 12m / 12m 30s / 45s.负数或缺失由调用方显示 "—".
pub fn format_countdown(ms: i64) -> String {
    let mut secs = ms / 1000;
    let days = secs / 86_400;
    secs %= 86_400;
    let hours = secs / 3_600;
    secs %= 3_600;
    let mins = secs / 60;
    secs %= 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/usage.json");

    #[test]
    fn fixture_parses_to_expected_percentages() {
        let snap = parse_snapshot(FIXTURE).unwrap();
        assert_eq!(snap.cards.len(), 4);
        let go = match &snap.cards[0] {
            Card::Live(r) => r,
            Card::Missing { .. } => panic!("go 应为 Live"),
        };
        assert_eq!(go.limits.len(), 3);
        assert!((go.limits[1].used_pct - 53.0).abs() < 1e-9);
        assert!((go.limits[2].used_pct - 26.0).abs() < 1e-9);
        let codex = match &snap.cards[1] {
            Card::Live(r) => r,
            Card::Missing { .. } => panic!("codex 应为 Live"),
        };
        assert_eq!(codex.limits.len(), 2);
        assert!((codex.limits[0].used_pct - 7.0).abs() < 1e-9);
        assert!((codex.limits[1].used_pct - 70.0).abs() < 1e-9);
    }

    #[test]
    fn recomputed_status_matches_omp_fixture() {
        // OMP 原文:go 全 ok;codex 7% ok / 70% ok.阈值语义必须一致.
        let snap = parse_snapshot(FIXTURE).unwrap();
        for card in &snap.cards {
            if let Card::Live(r) = card {
                for l in &r.limits {
                    assert_eq!(l.status, Status::Ok, "{}", l.label);
                }
            }
        }
    }

    #[test]
    fn status_thresholds() {
        assert_eq!(status_for(79.9), Status::Ok);
        assert_eq!(status_for(80.0), Status::Warning);
        assert_eq!(status_for(99.9), Status::Warning);
        assert_eq!(status_for(100.0), Status::Exhausted);
        assert_eq!(status_for(140.0), Status::Exhausted);
    }

    #[test]
    fn bundled_plan_multiple_uses_monthly_usd_price() {
        assert_eq!(LIST_PRICE_BENCHMARK_MONTHLY_USD, 40.0);
        assert_eq!(api_equivalent_vs_list_price_multiple(0.0), 0.0);
        assert_eq!(api_equivalent_vs_list_price_multiple(40.0), 1.0);
        assert_eq!(api_equivalent_vs_list_price_multiple(100.0), 2.5);
        assert_eq!(api_equivalent_vs_list_price_multiple(-10.0), 0.0);
        assert_eq!(api_equivalent_vs_list_price_multiple(f64::NAN), 0.0);
        assert_eq!(api_equivalent_vs_monthly_plan_multiple(60.0, 30.0), 2.0);
        assert_eq!(api_equivalent_vs_monthly_plan_multiple(60.0, 0.0), 0.0);
    }

    #[test]
    fn countdown_formats() {
        assert_eq!(format_countdown(45_000), "45s");
        assert_eq!(format_countdown(90_000), "1m 30s");
        assert_eq!(format_countdown(3_661_000), "1h 1m");
        assert_eq!(format_countdown(190_800_000), "2d 5h");
    }

    #[test]
    fn absent_provider_becomes_missing_card() {
        // fixture(omp 路径)无 goat → 第三张卡恒为 Missing(GOAT 未登录态).
        let snap = parse_snapshot(FIXTURE).unwrap();
        assert!(matches!(
            snap.cards[2],
            Card::Missing {
                provider: ProviderId::Goat
            }
        ));
    }

    #[test]
    fn unknown_provider_is_ignored() {
        let doc = r#"{"generatedAt":1,"reports":[{"provider":"nope","fetchedAt":1,"limits":[],"metadata":{}}]}"#;
        let snap = parse_snapshot(doc).unwrap();
        assert!(snap.cards.iter().all(|c| matches!(c, Card::Missing { .. })));
    }
}
