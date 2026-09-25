//! Token 用量页:今日/本周/本月汇总 + 按 Agent 表(含 14 天迷你走势)+ 每日
//! 表格/趋势柱状图双视图 + 可点击行明细.
//!
//! 成本口径:$ = 来源报告价(OMP) + LiteLLM 估价,查不到价的显示 "—".
//! 小终端下每日区自上而下被截断,命中区只对可见行生成.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::ledger::{LedgerReport, client_label, format_tokens, format_usd, types::Agg};

use super::{HitRegions, ViewState};
use crate::ui::i18n;
use crate::ui::theme as t;

pub fn render(frame: &mut Frame, area: Rect, st: &ViewState, hits: &mut HitRegions) {
    let Some(ledger) = st.ledger else {
        frame.render_widget(Paragraph::new(i18n::t(i18n::K::Loading)), area);
        return;
    };

    let agent_rows_shown = ledger.by_agent.len().min(8);
    let agent_h = agent_rows_shown as u16 + 2; // 表头 + 行 + 表头下留白
    let detail_h = if st.sel_agent.is_some() || st.sel_day.is_some() {
        6
    } else {
        0
    };

    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(1), // 维度标签行
            Constraint::Length(agent_h),
            Constraint::Length(1),
            Constraint::Min(3),
        ])
        .split(area);

    summary(frame, main[0], ledger);
    render_dim_tabs(frame, main[1], st.dim, hits);

    let agent_area = main[2];
    match st.dim {
        1 => render_session_table(frame, agent_area, ledger),
        2 => render_project_table(frame, agent_area, ledger),
        _ => render_agent_table(frame, agent_area, ledger, st.sel_agent, hits),
    }

    // 源状态行(左) + 每日视图切换按钮(右)
    let src_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(14)])
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
    let btn_rect = Rect {
        x: src_cols[1].x,
        y: src_cols[1].y,
        width: src_cols[1].width,
        height: 1,
    };
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

    let day_area = if detail_h > 0 {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(detail_h)])
            .split(main[4]);
        render_detail(frame, split[1], ledger, st);
        split[0]
    } else {
        main[4]
    };
    if st.chart_mode {
        render_day_chart(frame, day_area, ledger, hits);
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
fn render_dim_tabs(frame: &mut Frame, area: Rect, active: usize, hits: &mut HitRegions) {
    let th = t::get();
    let labels = [
        (i18n::t(i18n::K::DimAgent), 0usize),
        (i18n::t(i18n::K::DimSession), 1),
        (i18n::t(i18n::K::DimProject), 2),
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
}

fn cost_span(agg: &Agg) -> Span<'static> {
    let c = agg.cost();
    Span::raw(if c > 0.0 {
        format_usd(c)
    } else {
        "—".to_string()
    })
}

fn summary(frame: &mut Frame, area: Rect, ledger: &LedgerReport) {
    let th = t::get();
    let cells = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);
    for (i, (title, agg)) in [
        (i18n::t(i18n::K::SToday), &ledger.today),
        (i18n::t(i18n::K::SWeek), &ledger.week),
        (i18n::t(i18n::K::SMonth), &ledger.month),
    ]
    .into_iter()
    .enumerate()
    {
        let p = Paragraph::new(vec![
            Line::from(Span::styled(title, Style::default().fg(th.dim))),
            Line::from(Span::styled(
                format!("{} {}", format_tokens(agg.total()), "tok"),
                Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                cost_span(agg),
                Span::raw(format!(" · {} 条", agg.messages)),
            ]),
        ]);
        frame.render_widget(p, cells[i]);
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
    let mut spans = vec![Span::styled("源:", Style::default().fg(th.dim))];
    for (i, s) in ledger.sources.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(th.dim)));
        }
        if let Some(e) = &s.error {
            spans.push(Span::styled(
                format!("⚠ {}:{e}", client_label(s.client)),
                Style::default().fg(th.err),
            ));
        } else {
            spans.push(Span::styled(
                format!("{} {}", client_label(s.client), s.records),
                Style::default().fg(th.dim),
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

/// 8 级部分块 ramp(照抄 tokscale bar_chart,含 0 = 空格).
const RAMP: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// 每日趋势柱状图:直接写 buffer,8 级 ramp 做亚行精度;
/// 轴线用 ASCII '-'(U+2500 在 CJK locale 宽度歧义,tokscale 的教训)。
fn render_day_chart(frame: &mut Frame, area: Rect, ledger: &LedgerReport, hits: &mut HitRegions) {
    let th = t::get();
    if area.width < 20 || area.height < 4 || ledger.by_day.is_empty() {
        return;
    }
    let n = ledger.by_day.len();
    let max = ledger
        .by_day
        .iter()
        .map(|d| d.agg.total())
        .max()
        .unwrap_or(1)
        .max(1);
    let y_label_w = format_tokens(max).chars().count() as u16 + 1; // "44M|"
    let label_row = area.bottom() - 1;
    let chart = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height - 1, // 底行留给 X 标签
    };
    let axis_color = Style::default().fg(th.dim);
    let bar_color = Style::default().fg(th.accent);

    // Y 轴:顶部最大值、底部 0
    let top = format!("{}|", format_tokens(max));
    for (i, ch) in top.chars().enumerate() {
        if (i as u16) < chart.width {
            frame.buffer_mut()[(chart.x + i as u16, chart.y)]
                .set_char(ch)
                .set_style(axis_color);
        }
    }
    for (i, ch) in "0|".chars().enumerate() {
        if (i as u16) < chart.width {
            frame.buffer_mut()[(chart.x + i as u16, label_row - 1)]
                .set_char(ch)
                .set_style(axis_color);
        }
    }

    let plot_x = chart.x + y_label_w;
    let plot_w = chart.width.saturating_sub(y_label_w);
    let bw = (plot_w / n as u16).max(2);
    let rows = chart.height.saturating_sub(1) as f64; // 顶行留给最大值标签
    let plot_y = chart.y + 1;
    let max_f = max as f64;

    for (i, d) in ledger.by_day.iter().enumerate() {
        let bx = plot_x + i as u16 * bw;
        if bx + bw > plot_x + plot_w {
            break; // 右截断
        }
        let v = d.agg.total() as f64;
        for r in 0..rows as u16 {
            let y = plot_y + r;
            // 该行覆盖的值区间 (row_top, row_bottom]
            let row_top = max_f * (rows - r as f64) / rows;
            let row_bottom = max_f * (rows - r as f64 - 1.0) / rows;
            let ch = if v >= row_top && row_top > 0.0 {
                '█'
            } else if v > row_bottom {
                let frac = (v - row_bottom) / (row_top - row_bottom).max(1e-9);
                RAMP[(frac * 8.0).floor().clamp(1.0, 8.0) as usize]
            } else {
                continue;
            };
            for c in 0..bw.max(1) {
                let x = bx + c;
                if x < plot_x + plot_w {
                    frame.buffer_mut()[(x, y)].set_char(ch).set_style(bar_color);
                }
            }
        }
        // 命中区:整根柱子
        hits.day_rows.push((
            Rect {
                x: bx,
                y: plot_y,
                width: bw,
                height: rows as u16,
            },
            i,
        ));
        // X 标签:首/中/末
        if i == 0 || i == n / 2 || i == n - 1 {
            let label = &d.date[5..];
            for (ci, ch) in label.chars().enumerate() {
                let x = bx + ci as u16;
                if x < plot_x + plot_w {
                    frame.buffer_mut()[(x, label_row)]
                        .set_char(ch)
                        .set_style(axis_color);
                }
            }
        }
    }
}

fn render_detail(frame: &mut Frame, area: Rect, ledger: &LedgerReport, st: &ViewState) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(i) = st.sel_agent
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
            sel_heat: None,
            quota_view: crate::ui::QuotaView::Cards,
            ring_window: &[0, 0, 0],
            chart_mode: false,
            dim: 0,
            tick: 0,
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
        term.draw(|f| render(f, f.area(), &st, &mut hits)).unwrap();
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
            project: Some("C:\\Users\\dev\\Desktop\\tui_token_moniter".into()),
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
        assert!(flat(&term).contains("tui_token_moniter"), "缺项目名");

        // 项目维度
        st.dim = 2;
        term.draw(|f| crate::ui::render(f, &st, &mut hits)).unwrap();
        let text = flat(&term);
        assert!(text.contains("tui_token_moniter"), "缺项目行");
        assert!(text.contains("1"), "缺会话数");
        // 维度标签命中区
        assert_eq!(hits.dim_tabs.len(), 3);
        let (rect, idx) = hits.dim_tabs[1];
        let hit_res = hits.hit(rect.x + 1, rect.y);
        assert!(
            matches!(hit_res, Some(super::super::Hit::DimTab(i)) if i == idx),
            "dim tab 命中失败: {hit_res:?} rect={rect:?}"
        );
    }
}
