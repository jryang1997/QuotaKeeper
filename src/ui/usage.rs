//! Token 用量页:今日/本周/本月汇总 + 按 Agent 表(含 14 天迷你走势)+ 每日
//! 表格/趋势柱状图双视图 + 可点击行明细.
//!
//! 成本口径:$ = 来源报告价(OMP) + LiteLLM 估价,查不到价的显示 "—".
//! 小终端下每日区自上而下被截断,命中区只对可见行生成.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::ledger::{LedgerReport, client_label, format_tokens, format_usd, types::Agg};
#[cfg(test)]
use crate::model::ProviderId;
use crate::model::{self, Card, Snapshot};

use super::{HitRegions, TrendMetric, TrendRange, ViewState, quota, trend};
use crate::ui::i18n;
use crate::ui::theme as t;

pub fn render(frame: &mut Frame, area: Rect, st: &ViewState, hits: &mut HitRegions) {
    let Some(ledger) = st.ledger else {
        frame.render_widget(Paragraph::new(i18n::t(i18n::K::Loading)), area);
        return;
    };

    let top_h = if area.width >= 90 { 2 } else { 3 };
    let agent_rows_shown = ledger.by_agent.len().min(8);
    let room_for_agents = area.height.saturating_sub(top_h + 1 + 1 + 4);
    let agent_h = (agent_rows_shown as u16 + 1).min(room_for_agents).max(2);

    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_h),
            Constraint::Length(1), // 维度标签行
            Constraint::Length(agent_h),
            Constraint::Length(1),
            Constraint::Min(3),
        ])
        .split(area);

    right_now(frame, main[0], st, ledger);
    render_dim_tabs(frame, main[1], st.dim, hits, ledger);

    let agent_area = main[2];
    match st.dim {
        1 => render_session_table(frame, agent_area, ledger),
        2 => render_project_table(frame, agent_area, ledger),
        3 => render_model_table(frame, agent_area, ledger, st.model_offset),
        _ => render_agent_table(frame, agent_area, ledger, st.sel_agent, hits),
    }

    // 源状态行(左) + 每日视图切换按钮(右)
    let src_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(11),
            Constraint::Length(13),
            Constraint::Length(9),
        ])
        .split(main[3]);
    frame.render_widget(sources_line(ledger), src_cols[0]);
    let zh = i18n::lang() == i18n::Lang::Zh;
    let toggle_label = if zh {
        if st.chart_mode {
            "▤ 每日表"
        } else {
            "▁ 趋势图"
        }
    } else if st.chart_mode {
        "▤ Daily"
    } else {
        "▁ Trend"
    };
    let btn_rect = src_cols[1];
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            toggle_label,
            Style::default()
                .fg(t::get().accent)
                .add_modifier(Modifier::BOLD),
        ))),
        btn_rect,
    );
    hits.chart_toggle = Some(btn_rect);

    if st.chart_mode {
        render_chart_controls(frame, src_cols[2], src_cols[3], st, hits);
    }

    let selected = st.sel_agent.is_some() || st.sel_day.is_some() || st.sel_trend.is_some();
    let day_area = if selected && main[4].height >= 7 {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(3)])
            .split(main[4]);
        render_detail(frame, split[1], ledger, st);
        split[0]
    } else if selected {
        render_detail(frame, main[4], ledger, st);
        return;
    } else {
        main[4]
    };
    if st.chart_mode {
        trend::render(
            frame,
            day_area,
            ledger,
            st.trend_range,
            st.trend_metric,
            hits,
        );
    } else {
        render_day_table(frame, day_area, ledger, st.sel_day, hits);
    }
}

/// 粗略显示宽度:CJK(≥0x2E80)记 2,其余记 1.
pub(crate) fn disp_w(s: &str) -> u16 {
    s.chars()
        .map(|c| if c as u32 >= 0x2E80 { 2 } else { 1 })
        .sum()
}

/// 项目路径只显示末段.
fn path_tail(p: &str) -> String {
    p.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(p)
        .to_string()
}

/// 会话维度表(近 30 天,按最近活跃).
fn render_session_table(frame: &mut Frame, area: Rect, ledger: &LedgerReport) {
    let th = t::get();
    let header = Row::new([
        i18n::t(i18n::K::HSession),
        i18n::t(i18n::K::HProject),
        i18n::t(i18n::K::HClient),
        i18n::t(i18n::K::HTotal),
        i18n::t(i18n::K::HCost),
        i18n::t(i18n::K::HLast),
    ])
    .style(Style::default().fg(th.dim).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = ledger
        .by_session
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(Span::styled(
                    s.session
                        .chars()
                        .rev()
                        .take(8)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>(),
                    Style::default().fg(th.dim),
                )),
                Cell::from(
                    s.project
                        .as_deref()
                        .map(path_tail)
                        .unwrap_or_else(|| "—".into()),
                ),
                Cell::from(client_label(s.client).to_string()),
                Cell::from(Span::styled(
                    format_tokens(s.agg.total()),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Cell::from(cost_span(&s.agg)),
                Cell::from(Span::styled(
                    i18n::rel(ledger.generated_at_ms.saturating_sub(s.last_ts)),
                    Style::default().fg(th.dim),
                )),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Min(12),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .column_spacing(1);
    frame.render_widget(table, area);
}

/// 项目维度表(近 30 天,按总量).
fn render_project_table(frame: &mut Frame, area: Rect, ledger: &LedgerReport) {
    let th = t::get();
    let header = Row::new([
        i18n::t(i18n::K::HProject),
        i18n::t(i18n::K::HSession),
        i18n::t(i18n::K::HTotal),
        i18n::t(i18n::K::HCost),
        i18n::t(i18n::K::HLast),
    ])
    .style(Style::default().fg(th.dim).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = ledger
        .by_project
        .iter()
        .map(|p| {
            Row::new(vec![
                Cell::from(path_tail(&p.project)),
                Cell::from(format!("{}", p.sessions)),
                Cell::from(Span::styled(
                    format_tokens(p.agg.total()),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Cell::from(cost_span(&p.agg)),
                Cell::from(Span::styled(
                    i18n::rel(ledger.generated_at_ms.saturating_sub(p.last_ts)),
                    Style::default().fg(th.dim),
                )),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Min(14),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .column_spacing(1);
    frame.render_widget(table, area);
}

/// 维度标签:Agent | 会话 | 项目(可点击).
fn render_dim_tabs(
    frame: &mut Frame,
    area: Rect,
    active: usize,
    hits: &mut HitRegions,
    ledger: &LedgerReport,
) {
    let th = t::get();
    let labels = [
        (i18n::t(i18n::K::DimAgent), 0usize),
        (i18n::t(i18n::K::DimSession), 1),
        (i18n::t(i18n::K::DimProject), 2),
        (i18n::t(i18n::K::DimModel), 3),
    ];
    let mut x = area.x;
    for (label, idx) in labels {
        let w = disp_w(label) + 2;
        let active_tab = active == idx;
        let style = if active_tab {
            Style::default()
                .fg(th.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(th.dim)
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label, style))),
            Rect {
                x,
                y: area.y,
                width: w,
                height: 1,
            },
        );
        hits.dim_tabs.push((
            Rect {
                x,
                y: area.y,
                width: w,
                height: 1,
            },
            idx,
        ));
        x += w + 2;
    }

    let monthly_value = monthly_value_label(ledger, crate::settings::monthly_plan_benchmark());
    let available = area
        .width
        .saturating_sub(x.saturating_sub(area.x).saturating_add(2));
    if disp_w(&monthly_value) <= available {
        frame.render_widget(
            Paragraph::new(monthly_value)
                .alignment(Alignment::Right)
                .style(Style::default().fg(th.dim)),
            Rect {
                x: x + 2,
                y: area.y,
                width: available,
                height: 1,
            },
        );
    }
}

/// 精确显示 token 数,每三位加分隔符.
fn exact_token_count(value: i64) -> String {
    let mut reversed = String::new();
    for (index, digit) in value.to_string().chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            reversed.push(',');
        }
        reversed.push(digit);
    }
    reversed.chars().rev().collect()
}

/// 模型维度表(近 30 天,完整列表可用 ↑↓ 滚动).
fn render_model_table(frame: &mut Frame, area: Rect, ledger: &LedgerReport, offset: usize) {
    let th = t::get();
    if ledger.by_model.is_empty() {
        frame.render_widget(Paragraph::new(i18n::t(i18n::K::NoData)), area);
        return;
    }

    let visible = area.height.saturating_sub(1).max(1) as usize;
    let start = offset.min(ledger.by_model.len().saturating_sub(1));
    let end = start.saturating_add(visible).min(ledger.by_model.len());
    let header = Row::new([
        format!(
            "{} {}-{}/{} ↑↓",
            i18n::t(i18n::K::DimModel),
            start + 1,
            end,
            ledger.by_model.len()
        ),
        i18n::t(i18n::K::HClient).to_string(),
        i18n::t(i18n::K::HTotal).to_string(),
        i18n::t(i18n::K::HCost).to_string(),
    ])
    .style(Style::default().fg(th.dim).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = ledger.by_model[start..end]
        .iter()
        .map(|model| {
            let clients = model
                .clients
                .iter()
                .map(|client| client_label(client))
                .collect::<Vec<_>>()
                .join(", ");
            Row::new([
                model.model.clone(),
                clients,
                exact_token_count(model.agg.total()),
                if model.agg.cost() > 0.0 {
                    format_usd(model.agg.cost())
                } else {
                    "—".to_string()
                },
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Min(18),
            Constraint::Min(12),
            Constraint::Length(12),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .column_spacing(1);
    frame.render_widget(table, area);
}

fn monthly_value_label(ledger: &LedgerReport, benchmark: crate::settings::PlanBenchmark) -> String {
    let price = if i18n::lang() == i18n::Lang::Zh {
        format!(
            "{}{}",
            format_usd(benchmark.monthly_usd),
            if benchmark.is_custom {
                "我的月费"
            } else {
                "月基准"
            }
        )
    } else {
        format!(
            "{} {}",
            format_usd(benchmark.monthly_usd),
            if benchmark.is_custom {
                "my plan/mo"
            } else {
                "list/mo"
            }
        )
    };
    let cost = ledger.month.cost();
    if cost > 0.0 {
        format!(
            "API≈{} · {:.2}×{}",
            format_usd(cost),
            model::api_equivalent_vs_monthly_plan_multiple(cost, benchmark.monthly_usd),
            price
        )
    } else if ledger.month.total() == 0 {
        format!("API≈$0.00 · 0.00×{}", price)
    } else {
        format!("API— · —×{}", price)
    }
}

fn cost_span(agg: &Agg) -> Span<'static> {
    let c = agg.cost();
    Span::raw(if c > 0.0 {
        format_usd(c)
    } else {
        "—".to_string()
    })
}

fn short_window(label: &str) -> &str {
    let lower = label.to_ascii_lowercase();
    if lower.contains("5h") || lower.contains("5 hour") {
        "5h"
    } else if lower.contains("7d") || lower.contains("7 day") || lower.contains("week") {
        "7d"
    } else if lower.contains("month") || lower.contains('月') {
        if i18n::lang() == i18n::Lang::Zh {
            "月"
        } else {
            "mo"
        }
    } else {
        label
    }
}

fn tightest_quota(snapshot: Option<&Snapshot>) -> Option<String> {
    let now = chrono::Local::now();
    let mut exhausted: Option<(f64, String)> = None;
    let mut with_eta: Option<(i64, String)> = None;
    let mut fallback: Option<(f64, String)> = None;
    for card in &snapshot?.cards {
        let Card::Live(report) = card else { continue };
        if report.source == crate::model::QuotaSource::CpaLocal {
            continue;
        }
        let name = report
            .provider
            .title()
            .split(" ·")
            .next()
            .unwrap_or("Quota");
        for limit in &report.limits {
            let base = format!(
                "{} {} {:.0}%",
                name,
                short_window(&limit.label),
                limit.used_pct
            );
            if limit.used_pct >= 100.0
                && exhausted
                    .as_ref()
                    .is_none_or(|(pct, _)| limit.used_pct > *pct)
            {
                exhausted = Some((limit.used_pct, base.clone()));
            }
            if fallback
                .as_ref()
                .is_none_or(|(pct, _)| limit.used_pct > *pct)
            {
                fallback = Some((limit.used_pct, base.clone()));
            }
            if let Some(secs) = quota::eta_seconds(limit, now)
                && with_eta.as_ref().is_none_or(|(best, _)| secs < *best)
            {
                with_eta = Some((secs, base));
            }
        }
    }
    if let Some((_, base)) = exhausted {
        Some(base)
    } else if let Some((secs, base)) = with_eta {
        let countdown = crate::model::format_countdown(secs.saturating_mul(1000));
        Some(format!("{base} · ~{countdown}"))
    } else {
        fallback.map(|(_, text)| text)
    }
}

fn right_now(frame: &mut Frame, area: Rect, st: &ViewState, ledger: &LedgerReport) {
    if area.height == 0 {
        return;
    }
    let th = t::get();
    let recent_total: i64 = ledger.recent_by_agent.iter().map(|a| a.agg.total()).sum();
    let recent = if recent_total > 0 {
        let top = &ledger.recent_by_agent[0];
        if area.width < 60 {
            format!(
                "{} {} · {} {}",
                i18n::t(i18n::K::Last60),
                format_tokens(recent_total),
                client_label(top.client),
                format_tokens(top.agg.total())
            )
        } else {
            format!(
                "{} {} · {}/min · {} {}",
                i18n::t(i18n::K::Last60),
                format_tokens(recent_total),
                format_tokens(ledger.burn_rate.unwrap_or(0.0).round() as i64),
                client_label(top.client),
                format_tokens(top.agg.total()),
            )
        }
    } else if let Some(last) = ledger.last_activity_ms {
        format!(
            "{} {} · {} {}",
            i18n::t(i18n::K::Last60),
            i18n::t(i18n::K::Idle),
            i18n::t(i18n::K::LastRecord),
            i18n::rel(ledger.generated_at_ms.saturating_sub(last))
        )
    } else {
        format!("{} {}", i18n::t(i18n::K::Last60), i18n::t(i18n::K::Idle))
    };
    let quota = format!(
        "{} {}",
        i18n::t(i18n::K::QuotaShort),
        tightest_quota(st.snapshot).unwrap_or_else(|| "—".into())
    );
    let summary = [
        (i18n::t(i18n::K::SToday), &ledger.today),
        (i18n::t(i18n::K::SWeek), &ledger.week),
        (i18n::t(i18n::K::SMonth), &ledger.month),
    ]
    .into_iter()
    .map(|(title, agg)| {
        if area.width < 65 {
            return format!("{title} {}", format_tokens(agg.total()));
        }
        let cost = if agg.cost() > 0.0 {
            format_usd(agg.cost())
        } else {
            "—".into()
        };
        format!("{title} {} {cost}", format_tokens(agg.total()))
    })
    .collect::<Vec<_>>()
    .join("  ·  ");

    let emphasis = Style::default().fg(th.accent).add_modifier(Modifier::BOLD);
    if area.width >= 90 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(area);
        frame.render_widget(
            Paragraph::new(recent).style(emphasis),
            Rect {
                height: 1,
                ..cols[0]
            },
        );
        frame.render_widget(
            Paragraph::new(quota).style(Style::default().fg(th.dim)),
            Rect {
                height: 1,
                ..cols[1]
            },
        );
        if area.height > 1 {
            frame.render_widget(
                Paragraph::new(summary),
                Rect {
                    y: area.y + 1,
                    height: 1,
                    ..area
                },
            );
        }
    } else {
        for (i, (text, style)) in [
            (recent, emphasis),
            (quota, Style::default().fg(th.dim)),
            (summary, Style::default()),
        ]
        .into_iter()
        .enumerate()
        {
            if i as u16 >= area.height {
                break;
            }
            frame.render_widget(
                Paragraph::new(text).style(style),
                Rect {
                    y: area.y + i as u16,
                    height: 1,
                    ..area
                },
            );
        }
    }
}

fn render_chart_controls(
    frame: &mut Frame,
    range_area: Rect,
    metric_area: Rect,
    st: &ViewState,
    hits: &mut HitRegions,
) {
    let th = t::get();
    let selected = Style::default()
        .fg(th.accent)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let normal = Style::default().fg(th.dim);
    let ranges = [
        ("24h", TrendRange::H24, 0u16, 4u16),
        ("14d", TrendRange::D14, 5, 4),
    ];
    for (label, value, offset, width) in ranges {
        if offset + width > range_area.width {
            continue;
        }
        let rect = Rect {
            x: range_area.x + offset,
            y: range_area.y,
            width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(label).style(if st.trend_range == value {
                selected
            } else {
                normal
            }),
            rect,
        );
        hits.trend_ranges.push((rect, value));
    }
    let metrics = [
        ("Tok", TrendMetric::Tokens, 0u16, 4u16),
        ("$", TrendMetric::Cost, 5, 2),
    ];
    for (label, value, offset, width) in metrics {
        if offset + width > metric_area.width {
            continue;
        }
        let rect = Rect {
            x: metric_area.x + offset,
            y: metric_area.y,
            width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(label).style(if st.trend_metric == value {
                selected
            } else {
                normal
            }),
            rect,
        );
        hits.trend_metrics.push((rect, value));
    }
}

fn bar_spans(share: f64) -> Vec<Span<'static>> {
    let th = t::get();
    let filled = (share.clamp(0.0, 1.0) * 10.0).round() as usize;
    vec![
        Span::styled("█".repeat(filled), Style::default().fg(th.accent)),
        Span::styled("░".repeat(10 - filled), Style::default().fg(th.track)),
    ]
}

/// 14 天迷你走势:▁▂▃▄▅▆▇█ 按 当日值/最大值 分级,零值留空.
fn sparkline(series: &[i64]) -> String {
    let max = series.iter().copied().max().unwrap_or(0).max(1) as f64;
    const RAMP: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    series
        .iter()
        .map(|&v| {
            if v <= 0 {
                ' '
            } else {
                RAMP[((v as f64 / max) * 8.0).round().clamp(1.0, 8.0) as usize - 1]
            }
        })
        .collect()
}

fn render_agent_table(
    frame: &mut Frame,
    area: Rect,
    ledger: &LedgerReport,
    sel: Option<usize>,
    hits: &mut HitRegions,
) {
    let max_total = ledger
        .by_agent
        .iter()
        .map(|a| a.agg.total())
        .max()
        .unwrap_or(1)
        .max(1);
    let th = t::get();
    let header = Row::new([
        i18n::t(i18n::K::HAgent),
        i18n::t(i18n::K::HIn),
        i18n::t(i18n::K::HOut),
        i18n::t(i18n::K::HCache),
        i18n::t(i18n::K::HTotal),
        i18n::t(i18n::K::HCost),
        i18n::t(i18n::K::H14),
        i18n::t(i18n::K::HShare),
    ])
    .style(Style::default().fg(th.dim).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = ledger
        .by_agent
        .iter()
        .take(8)
        .enumerate()
        .map(|(i, a)| {
            let mut style = Style::default();
            if Some(i) == sel {
                style = style.bg(th.select_bg).fg(th.select_fg);
            }
            // 近 14 天该 agent 的每日总量 → 迷你走势。
            // 隔天取样 7 个点:▁▂▃ 等部分块是东亚歧义宽度,在 CJK 终端占 2 格,
            // 14 个点必然撑爆列宽(实测教训);7 点 × 2 格 = 恒定 14 列。
            let all: Vec<i64> = ledger
                .by_day
                .iter()
                .map(|d| {
                    d.per_client
                        .iter()
                        .find(|(c, _)| *c == a.client)
                        .map(|(_, agg)| agg.total())
                        .unwrap_or(0)
                })
                .collect();
            let series: Vec<i64> = all.iter().skip(1).step_by(2).copied().collect();
            Row::new(vec![
                Cell::from(Span::raw(client_label(a.client).to_string())),
                Cell::from(format_tokens(a.agg.input)),
                Cell::from(format_tokens(a.agg.output)),
                Cell::from(format_tokens(a.agg.cache_read + a.agg.cache_write)),
                Cell::from(Span::styled(
                    format_tokens(a.agg.total()),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Cell::from(cost_span(&a.agg)),
                Cell::from(Span::styled(
                    sparkline(&series),
                    Style::default().fg(th.accent),
                )),
                Cell::from(Line::from(bar_spans(
                    a.agg.total() as f64 / max_total as f64,
                ))),
            ])
            .style(style)
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(13),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Length(14),
            Constraint::Min(4),
        ],
    )
    .header(header)
    .column_spacing(1);
    frame.render_widget(table, area);

    // 命中区:表头 1 行,行从 y+1 起;只对可见行生成
    let visible = area.height.saturating_sub(1) as usize;
    for i in 0..ledger.by_agent.len().min(visible) {
        hits.agent_rows.push((
            Rect {
                x: area.x,
                y: area.y + 1 + i as u16,
                width: area.width,
                height: 1,
            },
            i,
        ));
    }
}

fn sources_line(ledger: &LedgerReport) -> Paragraph<'static> {
    let th = t::get();
    let mut spans = Vec::new();
    for s in &ledger.sources {
        if let Some(e) = &s.error {
            if !spans.is_empty() {
                spans.push(Span::styled(" · ", Style::default().fg(th.dim)));
            }
            spans.push(Span::styled(
                format!("⚠ {}:{e}", client_label(s.client)),
                Style::default().fg(th.err),
            ));
        }
    }
    Paragraph::new(Line::from(spans))
}

fn render_day_table(
    frame: &mut Frame,
    area: Rect,
    ledger: &LedgerReport,
    sel: Option<usize>,
    hits: &mut HitRegions,
) {
    let th = t::get();
    let header = Row::new([
        i18n::t(i18n::K::HDate),
        i18n::t(i18n::K::HTotal),
        i18n::t(i18n::K::HCost),
        i18n::t(i18n::K::HTop),
    ])
    .style(Style::default().fg(th.dim).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = ledger
        .by_day
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let mut style = Style::default();
            if Some(i) == sel {
                style = style.bg(th.select_bg).fg(th.select_fg);
            }
            let top = d
                .per_client
                .iter()
                .max_by_key(|(_, a)| a.total())
                .map(|(c, a)| format!("{} {}", client_label(c), format_tokens(a.total())))
                .unwrap_or_else(|| "—".into());
            Row::new(vec![
                Cell::from(d.date[5..].to_string()),
                Cell::from(Span::styled(
                    format_tokens(d.agg.total()),
                    if d.agg.total() > 0 {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(th.dim)
                    },
                )),
                Cell::from(cost_span(&d.agg)),
                Cell::from(Span::styled(top, Style::default().fg(th.dim))),
            ])
            .style(style)
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .column_spacing(1);
    frame.render_widget(table, area);

    let visible = area.height.saturating_sub(1) as usize;
    for i in 0..ledger.by_day.len().min(visible) {
        hits.day_rows.push((
            Rect {
                x: area.x,
                y: area.y + 1 + i as u16,
                width: area.width,
                height: 1,
            },
            i,
        ));
    }
}

fn render_detail(frame: &mut Frame, area: Rect, ledger: &LedgerReport, st: &ViewState) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some((heading, breakdown)) = st
        .sel_trend
        .and_then(|i| trend::detail(ledger, st.trend_range, st.trend_metric, i))
    {
        lines.push(Line::from(Span::styled(
            heading,
            Style::default().fg(t::get().accent),
        )));
        lines.push(Line::from(breakdown));
    } else if let Some(i) = st.sel_agent
        && let Some(a) = ledger.by_agent.get(i)
    {
        lines.push(Line::from(Span::styled(
            format!(
                "30d {} {} ({}):",
                client_label(a.client),
                i18n::t(i18n::K::ByModel),
                i18n::msgs(a.agg.messages)
            ),
            Style::default().fg(t::get().accent),
        )));
        for (model, agg) in a.models.iter().take(4) {
            lines.push(Line::from(format!(
                "  {model}  {} tok  {}",
                format_tokens(agg.total()),
                if agg.cost() > 0.0 {
                    format_usd(agg.cost())
                } else {
                    "—".to_string()
                }
            )));
        }
        if a.models.len() > 4 {
            let more = match i18n::lang() {
                i18n::Lang::Zh => format!("  …等 {} 个模型", a.models.len()),
                i18n::Lang::En => format!("  …+{} models", a.models.len()),
            };
            lines.push(Line::from(more));
        }
    } else if let Some(i) = st.sel_day
        && let Some(d) = ledger.by_day.get(i)
    {
        lines.push(Line::from(Span::styled(
            format!("{} {}", d.date, i18n::t(i18n::K::Detail)),
            Style::default().fg(t::get().accent),
        )));
        for (client, agg) in d.per_client.iter().take(4) {
            lines.push(Line::from(format!(
                "  {}  {} tok  {}",
                client_label(client),
                format_tokens(agg.total()),
                if agg.cost() > 0.0 {
                    format_usd(agg.cost())
                } else {
                    "—".to_string()
                }
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{SourceStatus, aggregate};
    use crate::ui::{Page, ViewState};
    use chrono::{Local, TimeZone};
    use ratatui::{Terminal, backend::TestBackend};

    fn fixture_ledger() -> LedgerReport {
        let now = Local
            .with_ymd_and_hms(2026, 9, 25, 12, 0, 0)
            .single()
            .unwrap();
        let mk = |client, model: &str, days_ago: u32, input: i64| crate::ledger::UsageRecord {
            client,
            model: model.into(),
            ts_ms: (now - chrono::Duration::days(days_ago as i64)).timestamp_millis(),
            tokens: crate::ledger::TokenBreakdown {
                input,
                output: input / 2,
                cache_read: input,
                cache_write: 0,
                reasoning: 0,
            },
            cost: Some(0.5),
            cost_is_estimate: false,
            session: None,
            project: None,
        };
        let records = vec![
            mk("codex", "gpt-6-sol", 0, 1_000),
            mk("omp", "deepseek/deepseek-v4.1-flash", 1, 2_000),
            mk("omp", "muse-spark-1.3", 0, 500),
            mk("zcode", "GLM-5.3-Flash", 0, 300),
        ];
        let mut rep = aggregate(records, now);
        rep.sources = vec![
            SourceStatus {
                client: "codex",
                records: 1,
                error: None,
            },
            SourceStatus {
                client: "omp",
                records: 2,
                error: None,
            },
            SourceStatus {
                client: "claude",
                records: 0,
                error: Some("未找到会话文件".into()),
            },
        ];
        rep
    }

    fn view<'a>(ledger: &'a LedgerReport) -> ViewState<'a> {
        ViewState {
            page: Page::Usage,
            snapshot: None,
            refreshing: false,
            last_error: None,
            age_secs: None,
            ledger: Some(ledger),
            sel_agent: None,
            sel_day: None,
            sel_trend: None,
            sel_heat: None,
            chart_mode: false,
            trend_range: super::TrendRange::H24,
            trend_metric: super::TrendMetric::Tokens,
            dim: 0,
            model_offset: 0,
            tick: 0,
            show_help: false,
        }
    }

    #[test]
    fn renders_tables_and_hit_regions() {
        let ledger = fixture_ledger();
        let backend = TestBackend::new(100, 32);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let st = view(&ledger);
        term.draw(|f| render(f, f.area(), &st, &mut hits)).unwrap();
        // TestBackend 里宽字符占两格(第二格为空格),去空白后再断言
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect::<String>()
            .split_whitespace()
            .collect();
        assert!(text.contains("Codex"), "缺 agent 行");
        assert!(text.contains("今日"), "缺汇总");
        assert!(text.contains("未找到会话文件"), "缺源状态");
        assert!(text.contains("09-25"), "缺每日行");
        assert_eq!(hits.agent_rows.len(), ledger.by_agent.len());
        assert_eq!(hits.day_rows.len(), 14);
        // 点击第一行 agent → 命中下标 0
        let (rect, idx) = hits.agent_rows[0];
        assert!(
            matches!(hits.hit(rect.x + 1, rect.y), Some(super::super::Hit::AgentRow(i)) if i == idx)
        );
    }

    #[test]
    fn renders_monthly_value_and_plan_multiple_at_80_columns() {
        let ledger = fixture_ledger();
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let st = view(&ledger);
        term.draw(|f| render(f, f.area(), &st, &mut hits)).unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        let compact_text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact_text.contains("API≈$2.00"), "缺本月估值");
        assert!(compact_text.contains("0.05×"), "缺套餐倍数");
        assert!(
            compact_text.contains("$40.00月基准"),
            "缺套餐基础标价参照: {text}"
        );
        assert_eq!(hits.agent_rows.len(), ledger.by_agent.len());
    }

    #[test]
    fn marks_month_value_unavailable_when_usage_has_no_price() {
        let mut ledger = fixture_ledger();
        ledger.month.cost_reported = 0.0;
        ledger.month.cost_estimated = 0.0;
        let label = monthly_value_label(
            &ledger,
            crate::settings::PlanBenchmark {
                monthly_usd: crate::settings::DEFAULT_MONTHLY_PLAN_USD,
                is_custom: false,
            },
        );
        assert!(label.contains("API—"), "应标明当前没有可用估价: {label}");
        assert!(label.contains("—×"), "倍数也应显示不可估: {label}");
        assert!(
            !label.contains("0.00×"),
            "不能把无价格误显示为 0 倍: {label}"
        );
    }

    #[test]
    fn detail_panel_renders_on_selection() {
        let ledger = fixture_ledger();
        let backend = TestBackend::new(100, 32);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let mut st = view(&ledger);
        st.sel_agent = Some(0);
        term.draw(|f| render(f, f.area(), &st, &mut hits)).unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect::<String>()
            .split_whitespace()
            .collect();
        assert!(text.contains("按模型"), "缺明细面板");
    }

    #[test]
    fn no_panic_on_small_terminal() {
        let ledger = fixture_ledger();
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let mut st = view(&ledger);
        st.sel_day = Some(0);
        st.chart_mode = true;
        term.draw(|f| render(f, f.area(), &st, &mut hits)).unwrap();
        let backend = TestBackend::new(60, 20);
        let mut compact = Terminal::new(backend).unwrap();
        st.sel_day = None;
        compact
            .draw(|f| crate::ui::render(f, &st, &mut hits))
            .unwrap();
    }

    #[test]
    fn recent_header_and_stacked_ranges_render_and_click() {
        let ledger = fixture_ledger();
        let backend = TestBackend::new(100, 32);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let mut st = view(&ledger);
        st.chart_mode = true;
        term.draw(|f| render(f, f.area(), &st, &mut hits)).unwrap();
        let flat = |t: &Terminal<TestBackend>| {
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert!(flat(&term).contains("60m"));
        assert!(!flat(&term).contains("正在运行"));
        assert_eq!(hits.trend_buckets.len(), 24);
        let (rect, idx) = hits.trend_buckets[23];
        assert!(
            matches!(hits.hit(rect.x, rect.y), Some(super::super::Hit::TrendBucket(i)) if i == idx)
        );
        assert!(flat(&term).contains("Codex"));

        st.trend_range = TrendRange::D14;
        st.trend_metric = TrendMetric::Cost;
        hits = HitRegions::default();
        term.draw(|f| render(f, f.area(), &st, &mut hits)).unwrap();
        assert_eq!(hits.trend_buckets.len(), 14);
        let detail = trend::detail(&ledger, TrendRange::D14, TrendMetric::Cost, 13).unwrap();
        assert!(detail.0.contains('$'));
        assert!(detail.1.contains("Codex"));
    }

    #[test]
    fn quota_teaser_excludes_antigravity_intensity() {
        use crate::model::{Limit, QuotaSource, Report, Status};
        let snapshot = Snapshot {
            generated_at_ms: 0,
            cards: vec![
                Card::Live(Report {
                    provider: ProviderId::Antigravity,
                    source: QuotaSource::CpaLocal,
                    plan_type: "CPA".into(),
                    limits: vec![Limit {
                        label: "5 hours".into(),
                        used_pct: 99.0,
                        resets_in_ms: Some(9_000_000),
                        status: Status::Warning,
                    }],
                    subscription_ends_at_ms: None,
                }),
                Card::Live(Report {
                    provider: ProviderId::Codex,
                    source: QuotaSource::CodexAppServer,
                    plan_type: "Plus".into(),
                    limits: vec![Limit {
                        label: "5 hours".into(),
                        used_pct: 50.0,
                        resets_in_ms: Some(9_000_000),
                        status: Status::Ok,
                    }],
                    subscription_ends_at_ms: None,
                }),
            ],
        };
        let teaser = tightest_quota(Some(&snapshot)).unwrap();
        assert!(teaser.contains("Codex"));
        assert!(!teaser.contains("Antigravity"));
        assert!(teaser.contains('~'));
    }

    #[test]
    fn exhausted_real_quota_precedes_a_valid_eta() {
        use crate::model::{Limit, QuotaSource, Report, Status};
        let snapshot = Snapshot {
            generated_at_ms: 0,
            cards: vec![
                Card::Live(Report {
                    provider: ProviderId::Codex,
                    source: QuotaSource::CodexAppServer,
                    plan_type: "Plus".into(),
                    limits: vec![Limit {
                        label: "5 hours".into(),
                        used_pct: 100.0,
                        resets_in_ms: Some(1_000_000),
                        status: Status::Exhausted,
                    }],
                    subscription_ends_at_ms: None,
                }),
                Card::Live(Report {
                    provider: ProviderId::OpenCodeGo,
                    source: QuotaSource::OpenCodeGoApi,
                    plan_type: "Go".into(),
                    limits: vec![Limit {
                        label: "5 hours".into(),
                        used_pct: 70.0,
                        resets_in_ms: Some(9_000_000),
                        status: Status::Ok,
                    }],
                    subscription_ends_at_ms: None,
                }),
            ],
        };
        let teaser = tightest_quota(Some(&snapshot)).unwrap();
        assert!(teaser.contains("Codex"));
        assert!(teaser.contains("100%"));
        assert!(!teaser.contains("OpenCode"));
    }

    /// 会话/项目维度表渲染(带 dim 标签命中区).
    #[test]
    fn session_and_project_tables_render() {
        let now = Local
            .with_ymd_and_hms(2026, 9, 25, 12, 0, 0)
            .single()
            .unwrap();
        let records = vec![crate::ledger::UsageRecord {
            client: "omp",
            model: "deepseek-flash".into(),
            ts_ms: now.timestamp_millis(),
            tokens: crate::ledger::TokenBreakdown {
                input: 100,
                output: 10,
                ..Default::default()
            },
            cost: Some(0.2),
            cost_is_estimate: false,
            session: Some("sess0123456789abcdef".into()),
            project: Some("C:\\Users\\dev\\demo-project".into()),
        }];
        let ledger = aggregate(records, now);
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let mut st = view(&ledger);
        st.dim = 1;
        term.draw(|f| crate::ui::render(f, &st, &mut hits)).unwrap();
        let flat = |t: &Terminal<TestBackend>| {
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
                .split_whitespace()
                .collect::<String>()
        };

        assert!(flat(&term).contains("89abcdef"), "缺会话短 id");
        assert!(flat(&term).contains("demo-project"), "缺项目名");

        // 项目维度
        st.dim = 2;
        term.draw(|f| crate::ui::render(f, &st, &mut hits)).unwrap();
        let text = flat(&term);
        assert!(text.contains("demo-project"), "缺项目行");
        assert!(text.contains("1"), "缺会话数");
        // 维度标签命中区
        assert_eq!(hits.dim_tabs.len(), 4);
        let (rect, idx) = hits.dim_tabs[1];
        let hit_res = hits.hit(rect.x + 1, rect.y);
        assert!(
            matches!(hit_res, Some(super::super::Hit::DimTab(i)) if i == idx),
            "dim tab 命中失败: {hit_res:?} rect={rect:?}"
        );
    }
}
