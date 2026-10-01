//! 热力图页:GitHub 风格的 26 周 × 7 天每日 Token 网格(口径照抄 tokscale stats).
//!
//! - 每格 "██" 两字宽、前景上色;空日 "· ";选中格 "▓▓" 反白;
//! - 5 级色阶按 tokens/max 归一化,浅色主题用 GitHub light 绿阶;
//! - 顶部月标签,左侧 周一/周三/周五,图例 "少 ██ ██ ██ ██ ██ 多";
//! - 右截断(只显示最近能放下的周),点击格子下钻当日按 agent 明细.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::ledger::{LedgerReport, client_label, format_tokens};

use super::{HitRegions, ViewState};
use crate::ui::i18n;
use crate::ui::theme as t;

const LABEL_W: u16 = 4;
const CELL_W: u16 = 2;

/// 五级色阶:0 = 空日,1–4 按 tokens/max 归一化.
fn level_color(tokens: i64, max: f64) -> Option<Color> {
    if tokens <= 0 || max <= 0.0 {
        return None;
    }
    let heat = t::get().heat;
    let tt = tokens as f64 / max;
    Some(if tt < 0.25 {
        heat[1]
    } else if tt < 0.5 {
        heat[2]
    } else if tt < 0.75 {
        heat[3]
    } else {
        heat[4]
    })
}

pub fn render(frame: &mut Frame, area: Rect, st: &ViewState, hits: &mut HitRegions) {
    let Some(ledger) = st.ledger else {
        frame.render_widget(Paragraph::new(i18n::t(i18n::K::Loading)), area);
        return;
    };
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(if st.sel_heat.is_some() { 0 } else { 10 }),
            Constraint::Length(if st.sel_heat.is_some() { 7 } else { 0 }),
        ])
        .split(area);
    render_grid(frame, split[0], ledger, st.sel_heat, hits);
    render_profile(frame, split[1], ledger);
    if let Some(date) = st.sel_heat {
        render_detail(frame, split[2], ledger, date);
    }
}

/// 时段 Profile(照抄 tokscale hourly_profile 口径):时段四桶 + 星期分布 + 峰值.
fn render_profile(frame: &mut Frame, area: Rect, ledger: &LedgerReport) {
    if area.height < 5 || ledger.hourly.iter().all(|&h| h == 0) {
        return;
    }
    let th = t::get();
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    // ---- 左:时段分布 ----
    let periods: [(&str, Vec<usize>); 4] = [
        (i18n::t(i18n::K::Morning), vec![5, 6, 7, 8, 9, 10, 11]),
        (i18n::t(i18n::K::Daytime), vec![12, 13, 14, 15, 16]),
        (i18n::t(i18n::K::Evening), vec![17, 18, 19, 20, 21]),
        (i18n::t(i18n::K::Night), vec![22, 23, 0, 1, 2, 3, 4]),
    ];
    let hour_tok = |h: &usize| ledger.hourly[*h];
    let sums: Vec<i64> = periods
        .iter()
        .map(|(_, hs)| hs.iter().map(hour_tok).sum())
        .collect();
    let max_p = sums.iter().copied().max().unwrap_or(1).max(1);
    let total: i64 = ledger.hourly.iter().sum();
    let bar_w = (cols[0].width as usize).saturating_sub(18).clamp(8, 40);

    let mut left = vec![Line::from(Span::styled(
        i18n::t(i18n::K::ProfileHours),
        Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
    ))];
    for ((label, _), sum) in periods.iter().zip(&sums) {
        let filled = (*sum as f64 / max_p as f64 * bar_w as f64).round() as usize;
        let pct = if total > 0 {
            *sum as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        left.push(Line::from(vec![
            Span::styled(format!("{label:>8} "), Style::default().fg(th.dim)),
            Span::styled("█".repeat(filled), Style::default().fg(th.accent)),
            Span::styled("░".repeat(bar_w - filled), Style::default().fg(th.track)),
            Span::raw(format!(" {:>5.1}%", pct)),
        ]));
    }
    // 峰值小时
    if let Some((h, _)) = ledger
        .hourly
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| **v)
        .filter(|(_, v)| **v > 0)
    {
        left.push(Line::from(Span::styled(
            format!("{} {:02}:00", i18n::t(i18n::K::Peak), h),
            Style::default().fg(th.dim),
        )));
    }
    frame.render_widget(Paragraph::new(left), cols[0]);

    // ---- 右:星期分布 ----
    let zh_names = ["一", "二", "三", "四", "五", "六", "日"];
    let en_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let zh = i18n::lang() == i18n::Lang::Zh;
    let max_w = ledger.weekday.iter().copied().max().unwrap_or(1).max(1);
    let bar_w2 = (cols[1].width as usize).saturating_sub(14).clamp(8, 40);
    let mut right = vec![Line::from(Span::styled(
        i18n::t(i18n::K::ProfileWeekday),
        Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
    ))];
    for (i, &v) in ledger.weekday.iter().enumerate() {
        let name = if zh { zh_names[i] } else { en_names[i] };
        let filled = (v as f64 / max_w as f64 * bar_w2 as f64).round() as usize;
        right.push(Line::from(vec![
            Span::styled(format!("{name:>4} "), Style::default().fg(th.dim)),
            Span::styled("█".repeat(filled), Style::default().fg(th.accent)),
            Span::styled("░".repeat(bar_w2 - filled), Style::default().fg(th.track)),
        ]));
    }
    frame.render_widget(Paragraph::new(right), cols[1]);
}

fn max_tokens(ledger: &LedgerReport) -> f64 {
    ledger
        .daily_totals
        .iter()
        .map(|d| d.tokens)
        .max()
        .unwrap_or(0) as f64
}

fn render_grid(
    frame: &mut Frame,
    area: Rect,
    ledger: &LedgerReport,
    sel: Option<&str>,
    hits: &mut HitRegions,
) {
    let th = t::get();
    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", i18n::t(i18n::K::HeatTitle)),
            Style::default().fg(th.accent),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.border));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let total = ledger.daily_totals.len();
    if total == 0 {
        return;
    }
    let weeks = total / 7;
    let max = max_tokens(ledger);
    // 右截断:只显示最近能放下的周(宽度预算 = 内宽 - 日标签列)
    let grid_w = inner.width.saturating_sub(LABEL_W);
    let weeks_to_show = ((grid_w / CELL_W) as usize).min(weeks);
    let start_week = weeks - weeks_to_show;

    // 顶部:月标签(某周第一天的月份与上一个不同则标出)
    let mut last_month = String::new();
    for w in 0..weeks_to_show {
        let idx = (start_week + w) * 7;
        let month = ledger.daily_totals[idx].date[5..7].to_string();
        if month != last_month {
            last_month = month.clone();
            let x = inner.x + LABEL_W + w as u16 * CELL_W;
            if x + 2 <= inner.right() {
                frame.render_widget(
                    Paragraph::new(Span::styled(month, Style::default().fg(th.dim))),
                    Rect {
                        x,
                        y: inner.y,
                        width: 4,
                        height: 1,
                    },
                );
            }
        }
    }

    // 行标签:周一/周三/周五(奇数行)
    for (row, label) in [
        (1usize, i18n::t(i18n::K::WMon)),
        (3, i18n::t(i18n::K::WWed)),
        (5, i18n::t(i18n::K::WFri)),
    ] {
        frame.render_widget(
            Paragraph::new(Span::styled(label, Style::default().fg(th.dim))),
            Rect {
                x: inner.x,
                y: inner.y + 1 + row as u16,
                width: LABEL_W,
                height: 1,
            },
        );
    }

    // 格子
    let grid_y = inner.y + 1;
    for w in 0..weeks_to_show {
        for d in 0..7 {
            let idx = (start_week + w) * 7 + d;
            let dt = &ledger.daily_totals[idx];
            let x = inner.x + LABEL_W + w as u16 * CELL_W;
            let y = grid_y + d as u16;
            let rect = Rect {
                x,
                y,
                width: CELL_W,
                height: 1,
            };
            let selected = sel == Some(dt.date.as_str());
            let (text, style) = match (level_color(dt.tokens, max), selected) {
                (None, _) => (
                    Span::styled("· ", Style::default().fg(th.track)),
                    Style::default(),
                ),
                (Some(c), false) => (Span::raw("██"), Style::default().fg(c)),
                (Some(c), true) => (Span::raw("▓▓"), Style::default().fg(th.select_fg).bg(c)),
            };
            frame.render_widget(Paragraph::new(Line::from(text)).style(style), rect);
            hits.heat_cells.push((rect, dt.date.clone()));
        }
    }

    // 图例 + 汇总
    let zh = i18n::lang() == i18n::Lang::Zh;
    let mut legend = vec![Span::styled(
        if zh { "少 " } else { "Less " },
        Style::default().fg(th.dim),
    )];
    for c in th.heat[1..5].iter() {
        legend.push(Span::styled("██", Style::default().fg(*c)));
        legend.push(Span::raw(" "));
    }
    legend.push(Span::styled(
        if zh { "多" } else { "More" },
        Style::default().fg(th.dim),
    ));
    let total_label = if zh { "26 周合计" } else { "26w total" };
    legend.push(Span::styled(
        format!(
            "   {total_label} {} tok · {}",
            format_tokens(ledger.daily_totals.iter().map(|d| d.tokens).sum()),
            format_usd_total(ledger)
        ),
        Style::default().fg(th.dim),
    ));
    frame.render_widget(
        Paragraph::new(Line::from(legend)),
        Rect {
            x: inner.x,
            y: inner.bottom().saturating_sub(1),
            width: inner.width,
            height: 1,
        },
    );
}

fn format_usd_total(ledger: &LedgerReport) -> String {
    let c: f64 = ledger.daily_totals.iter().map(|d| d.cost).sum();
    if c > 0.0 {
        crate::ledger::format_usd(c)
    } else {
        "—".to_string()
    }
}

fn render_detail(frame: &mut Frame, area: Rect, ledger: &LedgerReport, date: &str) {
    let mut lines = vec![Line::from(Span::styled(
        format!("{} {}", date, i18n::t(i18n::K::Detail)),
        Style::default().fg(t::get().accent),
    ))];
    match ledger.daily_detail.get(date) {
        Some(per) => {
            for (client, agg) in per.iter().take(5) {
                lines.push(Line::from(format!(
                    "  {}  {} tok  {}",
                    client_label(client),
                    format_tokens(agg.total()),
                    if agg.cost() > 0.0 {
                        crate::ledger::format_usd(agg.cost())
                    } else {
                        "—".to_string()
                    }
                )));
            }
        }
        None => lines.push(Line::from("  无数据")),
    }
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{DailyTotal, SourceStatus, aggregate};
    use crate::ui::{Page, ViewState};
    use chrono::{Local, TimeZone};
    use ratatui::{Terminal, backend::TestBackend};

    fn fixture_ledger() -> LedgerReport {
        let now = Local
            .with_ymd_and_hms(2026, 9, 25, 12, 0, 0)
            .single()
            .unwrap();
        let mk = |client, days_ago: i64, input: i64| crate::ledger::UsageRecord {
            client,
            model: "m".into(),
            ts_ms: (now - chrono::Duration::days(days_ago)).timestamp_millis(),
            tokens: crate::ledger::TokenBreakdown {
                input,
                output: 1,
                ..Default::default()
            },
            cost: Some(0.25),
            cost_is_estimate: false,
            session: None,
            project: None,
        };
        // 25 天前(恰好在 26 周窗口内)与今天
        let records = vec![mk("omp", 25, 400), mk("omp", 0, 800), mk("zcode", 0, 100)];
        let mut rep = aggregate(records, now);
        rep.sources = vec![SourceStatus {
            client: "omp",
            records: 3,
            error: None,
        }];
        rep
    }

    fn view<'a>(ledger: &'a LedgerReport) -> ViewState<'a> {
        ViewState {
            page: Page::Heatmap,
            snapshot: None,
            refreshing: false,
            last_error: None,
            age_secs: None,
            ledger: Some(ledger),
            period: crate::ui::overview::Period::D30,
            sel_agent: None,
            sel_day: None,
            sel_trend: None,
            sel_heat: None,
            chart_mode: false,
            quota_remaining: false,
            trend_range: crate::ui::TrendRange::H24,
            trend_metric: crate::ui::TrendMetric::Tokens,
            dim: 0,
            model_offset: 0,
            tick: 0,
            show_help: false,
        }
    }

    #[test]
    fn level_color_thresholds() {
        let heat = t::get().heat;
        assert_eq!(level_color(0, 100.0), None);
        assert_eq!(level_color(10, 100.0), Some(heat[1]));
        assert_eq!(level_color(30, 100.0), Some(heat[2]));
        assert_eq!(level_color(60, 100.0), Some(heat[3]));
        assert_eq!(level_color(90, 100.0), Some(heat[4]));
    }

    #[test]
    fn grid_renders_with_legend_and_hit_cells() {
        let ledger = fixture_ledger();
        let backend = TestBackend::new(110, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let st = view(&ledger);
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
        assert!(text.contains("26周"), "缺标题");
        assert!(text.contains("少"), "缺图例");
        assert!(text.contains("09"), "缺月标签");
        assert_eq!(hits.heat_cells.len(), 26 * 7, "命中区应覆盖整月网格");
        // 点击今天的格子 → 命中返回该日期
        let (rect, date) = hits
            .heat_cells
            .iter()
            .find(|(_, d)| d == "2026-09-25")
            .unwrap()
            .clone();
        assert!(matches!(
            hits.hit(rect.x, rect.y),
            Some(super::super::Hit::HeatCell(d)) if d == date
        ));
    }

    #[test]
    fn detail_panel_shows_per_client() {
        let ledger = fixture_ledger();
        let backend = TestBackend::new(110, 24);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let mut st = view(&ledger);
        st.sel_heat = Some("2026-09-25");
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
        assert!(text.contains("明细"), "缺明细标题");
        assert!(text.contains("OMP"), "缺按 agent 行");
    }

    #[test]
    fn daily_totals_type_usage() {
        // 防止 DailyTotal 字段被误删的编译期哨兵
        let d = DailyTotal {
            date: "x".into(),
            tokens: 1,
            cost: 0.5,
            messages: 2,
            agg: crate::ledger::Agg::default(),
        };
        assert_eq!(d.tokens, 1);
        assert_eq!(d.messages, 2);
    }
}
