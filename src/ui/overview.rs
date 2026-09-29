//! 总览仪表板页:周期 tabs + 头部汇总 + TOKENS/RUNS/来源健康三卡 + 每日活动表。
//! 设计语言学习 CodeBurn 的仪表板网格(周期 tabs、分段色条、脚注口径),
//! 视觉沿用本应用 █░ 粗块 + 圆角边框 + 四主题。

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::ledger::{DailyTotal, LedgerReport};
use crate::ui::i18n::{self, Lang};
use crate::ui::theme as t;
use crate::ui::{HitRegions, ViewState, disp_w};

/// 总览周期:驱动头部汇总、三卡与每日表的日期范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Today,
    D7,
    D30,
    D90,
    All,
}

pub const PERIODS: [Period; 5] = [
    Period::Today,
    Period::D7,
    Period::D30,
    Period::D90,
    Period::All,
];

impl Period {
    fn label(self) -> &'static str {
        i18n::t(match self {
            Period::Today => i18n::K::PToday,
            Period::D7 => i18n::K::P7,
            Period::D30 => i18n::K::P30,
            Period::D90 => i18n::K::P90,
            Period::All => i18n::K::PAll,
        })
    }

    /// 周期覆盖的天数;All 返回 None(全部历史)。
    fn days(self) -> Option<usize> {
        match self {
            Period::Today => Some(1),
            Period::D7 => Some(7),
            Period::D30 => Some(30),
            Period::D90 => Some(90),
            Period::All => None,
        }
    }

    /// ← 键:更长的周期。
    pub fn prev(self) -> Self {
        match self {
            Period::Today => Period::All,
            Period::D7 => Period::Today,
            Period::D30 => Period::D7,
            Period::D90 => Period::D30,
            Period::All => Period::D90,
        }
    }

    /// → 键:更短的周期。
    pub fn next(self) -> Self {
        match self {
            Period::Today => Period::D7,
            Period::D7 => Period::D30,
            Period::D30 => Period::D90,
            Period::D90 => Period::All,
            Period::All => Period::Today,
        }
    }
}

/// 所选周期的逐日切片(旧 → 新)。
pub fn period_days<'a>(period: Period, ledger: &'a LedgerReport) -> Vec<&'a DailyTotal> {
    match period.days() {
        None => ledger.daily_totals.iter().collect(),
        Some(n) => ledger
            .daily_totals
            .iter()
            .rev()
            .take(n)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
    }
}

// ---------------- 点阵大数字(3×5,▀▄█ 双行半块渲染) ----------------

/// 每字形 5 行 × 3 列位图。
const GLYPHS: [(&str, [u8; 5]); 16] = [
    ("0", [0b111, 0b101, 0b101, 0b101, 0b111]),
    ("1", [0b010, 0b110, 0b010, 0b010, 0b111]),
    ("2", [0b111, 0b001, 0b111, 0b100, 0b111]),
    ("3", [0b111, 0b001, 0b011, 0b001, 0b111]),
    ("4", [0b101, 0b101, 0b111, 0b001, 0b001]),
    ("5", [0b111, 0b100, 0b111, 0b001, 0b111]),
    ("6", [0b111, 0b100, 0b111, 0b101, 0b111]),
    ("7", [0b111, 0b001, 0b001, 0b010, 0b010]),
    ("8", [0b111, 0b101, 0b111, 0b101, 0b111]),
    ("9", [0b111, 0b101, 0b111, 0b001, 0b111]),
    (".", [0b000, 0b000, 0b000, 0b000, 0b010]),
    (",", [0b000, 0b000, 0b000, 0b010, 0b100]),
    ("K", [0b101, 0b101, 0b110, 0b101, 0b101]),
    ("M", [0b101, 0b111, 0b111, 0b101, 0b101]),
    ("B", [0b110, 0b101, 0b110, 0b101, 0b110]),
    (" ", [0b000, 0b000, 0b000, 0b000, 0b000]),
];

fn glyph(ch: char) -> &'static [u8; 5] {
    GLYPHS
        .iter()
        .find(|(c, _)| *c == ch.to_string())
        .map(|(_, rows)| rows)
        .unwrap_or(&[0b000, 0b000, 0b000, 0b000, 0b000])
}

/// 点阵数字:返回 3 行文本(每行两个半块行合并,▀▄█)。
pub fn dot_lines(value: &str, color: ratatui::style::Color) -> Vec<Line<'static>> {
    // 逐字形拼接(每字形占 3 列 + 1 列间隔)
    let chars: Vec<char> = value.chars().collect();
    let bit = |row: usize, col: usize| -> bool {
        let glyph_idx = col / 4;
        let inner = col % 4;
        if inner == 3 || glyph_idx >= chars.len() {
            return false;
        }
        glyph(chars[glyph_idx])[row] & (0b100 >> inner) != 0
    };
    let pair = |top: usize| -> Line<'static> {
        let mut s = String::new();
        for col in 0..chars.len() * 4 {
            let (a, b) = (bit(top, col), bit(top + 1, col));
            s.push(match (a, b) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        Line::from(Span::styled(s, Style::default().fg(color)))
    };
    vec![pair(0), pair(2), {
        let mut s = String::new();
        for col in 0..chars.len() * 4 {
            s.push(if bit(4, col) { '▀' } else { ' ' });
        }
        Line::from(Span::styled(s, Style::default().fg(color)))
    }]
}

/// tokens 的大数缩写(点阵字形集内:数字 + K/M/B)。
pub fn dot_tokens(v: i64) -> String {
    let f = v as f64;
    if f >= 1e9 {
        format!("{:.2}B", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.2}M", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.1}K", f / 1e3)
    } else {
        v.to_string()
    }
}

/// runs 的千分位完整数字(点阵含逗号字形)。
fn dot_runs(v: u64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

// ---------------- 火花线 minibar ----------------

const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// 逐日值 → 单行火花线(CJK 终端 ▁..█ 占 2 格,调用方按 ×2 预算列宽)。
fn sparkline(values: &[i64]) -> String {
    let max = values.iter().copied().max().unwrap_or(0);
    values
        .iter()
        .map(|&v| {
            if max <= 0 {
                SPARK[0]
            } else {
                let level = ((v as f64 / max as f64) * (SPARK.len() - 1) as f64).round() as usize;
                SPARK[level.min(SPARK.len() - 1)]
            }
        })
        .collect()
}

// ---------------- 分段迷你条(in/out/cache 三色) ----------------

/// 按 Agg 五桶分段的 █ 条:input 绿 / output 蓝 / cache 紫,宽度按比例。
fn seg_bar(agg: &crate::ledger::Agg, width: usize) -> Vec<Span<'static>> {
    let th = t::get();
    let total = agg.total().max(1) as f64;
    let parts = [
        (agg.input + agg.cache_read, th.series[2]), // 输入含缓存读
        (agg.output, th.series[0]),                 // 输出
        (agg.cache_write + agg.reasoning, th.series[1]), // 缓存写/推理
    ];
    let mut spans = Vec::with_capacity(parts.len());
    let mut used = 0usize;
    for (i, (v, color)) in parts.iter().enumerate() {
        let w = if i == parts.len() - 1 {
            width.saturating_sub(used)
        } else {
            ((*v as f64 / total) * width as f64).round() as usize
        };
        let w = w.min(width - used);
        used += w;
        spans.push(Span::styled("█".repeat(w), Style::default().fg(*color)));
    }
    spans
}

// ---------------- 页面渲染 ----------------

pub fn render(frame: &mut Frame, area: Rect, st: &ViewState, hits: &mut HitRegions) {
    let Some(ledger) = st.ledger else {
        frame.render_widget(Paragraph::new(i18n::t(i18n::K::Loading)), area);
        return;
    };
    let zh = i18n::lang() == Lang::Zh;
    let days = period_days(st.period, ledger);

    // 周期 tabs 行
    let (tabs_row, body) = {
        let l = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(3)])
            .split(area);
        (l[0], l[1])
    };
    render_period_tabs(frame, tabs_row, st, hits);

    if area.width < 76 {
        // 窄窗口:汇总行(无卡框)+ 每日活动
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(3)])
            .split(body);
        render_summary_plain(frame, rows[0], &days, ledger, zh);
        render_daily_table(frame, rows[1], &days, zh);
        return;
    }

    let wide = area.width >= 118;
    let (summary, cards_row, daily) = if wide {
        let l = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(8), // 来源健康卡 6 行 + 边框 2
                Constraint::Min(3),
            ])
            .split(body);
        (l[0], l[1], l[2])
    } else {
        let l = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(body);
        (l[0], l[0], l[1]) // 中等宽度:三卡并入汇总区下方省略,直接 daily
    };
    render_summary_card(frame, summary, st.period, &days, zh);
    if wide {
        render_stat_cards(frame, cards_row, &days, ledger, zh);
    }
    render_daily_table(frame, daily, &days, zh);
}

fn render_period_tabs(frame: &mut Frame, area: Rect, st: &ViewState, hits: &mut HitRegions) {
    let mut spans = Vec::with_capacity(PERIODS.len() * 3);
    let mut x = area.x;
    for &period in PERIODS.iter() {
        let label = period.label();
        let w = disp_w(label) + 2; // 方括号
        let rect = Rect { x, y: area.y, width: w, height: 1 };
        let active = period == st.period;
        let text = if active {
            format!("[{label}]")
        } else {
            format!(" {label} ")
        };
        spans.push(Span::styled(
            text,
            if active {
                Style::default().fg(t::get().accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t::get().dim)
            },
        ));
        spans.push(Span::raw("  "));
        hits.period_tabs.push((rect, period));
        x += w + 2;
    }
    let zh = i18n::lang() == Lang::Zh;
    spans.push(Span::styled(
        if zh { "←→ 切换" } else { "←→ switch" },
        Style::default().fg(t::get().dim),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// 周期聚合(Agg 求和;今日周期单日)。
fn period_agg(days: &[&DailyTotal]) -> crate::ledger::Agg {
    let mut agg = crate::ledger::Agg::default();
    for d in days {
        agg.merge(&d.agg);
    }
    agg
}

fn cache_hit(agg: &crate::ledger::Agg) -> Option<f64> {
    let denom = agg.input + agg.cache_read;
    (denom > 0).then(|| agg.cache_read as f64 / denom as f64 * 100.0)
}

fn fmt_usd(v: f64) -> String {
    if v >= 100.0 {
        format!("${:.0}", v)
    } else {
        format!("${:.2}", v)
    }
}

fn fmt_tokens_plain(v: i64) -> String {
    if v >= 1e9 as i64 {
        format!("{:.1}M", v as f64 / 1e6)
    } else if v >= 1e6 as i64 {
        format!("{:.1}M", v as f64 / 1e6)
    } else if v >= 1e3 as i64 {
        format!("{:.1}K", v as f64 / 1e3)
    } else {
        v.to_string()
    }
}

fn render_summary_card(frame: &mut Frame, area: Rect, period: Period, days: &[&DailyTotal], zh: bool) {
    let th = t::get();
    let agg = period_agg(days);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.accent))
        .title(Span::styled(
            format!(" {} · {} ", period.label(), i18n::t(i18n::K::Summary)),
            Style::default().fg(th.accent),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let hit = cache_hit(&agg);
    let line1 = vec![
        Span::styled(
            fmt_usd(agg.cost()),
            Style::default().fg(th.warn).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cost", Style::default().fg(th.dim)),
        Span::raw("   "),
        Span::styled(
            dot_runs(agg.messages),
            Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" calls", Style::default().fg(th.dim)),
        Span::raw("   "),
        Span::styled(
            hit.map(|h| format!("{h:.1}%")).unwrap_or_else(|| "—".into()),
            Style::default().fg(th.ok).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", i18n::t(i18n::K::CacheHit)), Style::default().fg(th.dim)),
    ];
    let flow = format!(
        "{} in · {} out · {} cached · {} written",
        fmt_tokens_plain(agg.input),
        fmt_tokens_plain(agg.output),
        fmt_tokens_plain(agg.cache_read),
        fmt_tokens_plain(agg.cache_write),
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(Paragraph::new(Line::from(line1)), rows[0]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(flow, Style::default().fg(th.dim)))),
        rows[1],
    );
    let _ = zh;
}

fn render_summary_plain(frame: &mut Frame, area: Rect, days: &[&DailyTotal], _ledger: &LedgerReport, zh: bool) {
    let th = t::get();
    let agg = period_agg(days);
    let hit = cache_hit(&agg).map(|h| format!("{h:.1}%")).unwrap_or_else(|| "—".into());
    let line = Line::from(vec![
        Span::styled(fmt_usd(agg.cost()), Style::default().fg(th.warn).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(dot_runs(agg.messages), Style::default().fg(th.accent)),
        Span::raw("  "),
        Span::styled(hit, Style::default().fg(th.ok)),
        Span::raw("  "),
        Span::styled(
            if zh { i18n::t(i18n::K::LocalLedger).to_string() } else { String::new() },
            Style::default().fg(th.dim),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_stat_cards(
    frame: &mut Frame,
    area: Rect,
    days: &[&DailyTotal],
    ledger: &LedgerReport,
    zh: bool,
) {
    let th = t::get();
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(5, 12),
            Constraint::Ratio(4, 12),
            Constraint::Ratio(3, 12),
        ])
        .split(area);
    let agg = period_agg(days);

    // 卡 1:累计 Tokens + 周期逐日 minibar
    stat_card(
        frame,
        cols[0],
        i18n::t(i18n::K::TotalTokens),
        &dot_lines(&dot_tokens(agg.total()), th.accent),
        Some(sparkline(&days.iter().map(|d| d.tokens).collect::<Vec<_>>())),
        &format!("{} · {}", i18n::t(i18n::K::LocalLedger), days.len()),
        th,
    );

    // 卡 2:累计调用 + 逐日 minibar
    stat_card(
        frame,
        cols[1],
        i18n::t(i18n::K::TotalRuns),
        &dot_lines(&dot_runs(agg.messages), th.ok),
        Some(sparkline(&days.iter().map(|d| d.messages as i64).collect::<Vec<_>>())),
        &format!("{} · {}", i18n::t(i18n::K::LocalLedger), days.len()),
        th,
    );

    // 卡 3:来源健康(六源状态行)
    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", i18n::t(i18n::K::SourceHealth)),
            Style::default().fg(th.border),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.border));
    let inner3 = block.inner(cols[2]);
    frame.render_widget(block, cols[2]);
    let rows: Vec<Line> = ledger
        .sources
        .iter()
        .map(|s| {
            let (dot, style) = if s.error.is_some() {
                ("●", Style::default().fg(th.err))
            } else if s.records > 0 {
                ("●", Style::default().fg(th.ok))
            } else {
                ("●", Style::default().fg(th.dim))
            };
            let note = s.error.clone().unwrap_or_else(|| s.records.to_string());
            Line::from(vec![
                Span::styled(format!("{dot} "), style),
                Span::styled(s.client.to_string(), Style::default().fg(th.border)),
                Span::styled(format!(" {note}"), Style::default().fg(th.dim)),
            ])
        })
        .collect();
    let _ = zh;
    frame.render_widget(Paragraph::new(rows), inner3);
}

#[allow(clippy::too_many_arguments)]
fn stat_card(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    digits: &[Line<'static>],
    spark: Option<String>,
    caption: &str,
    th: &t::Theme,
) {
    let block = Block::default()
        .title(Span::styled(
            format!(" {label} "),
            Style::default().fg(th.border),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.border));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    for (i, line) in digits.iter().enumerate() {
        frame.render_widget(Paragraph::new(line.clone()), rows[i]);
    }
    if let Some(sp) = spark {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(sp, Style::default().fg(th.accent)))),
            rows[3],
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(caption, Style::default().fg(th.dim)))),
        rows[4],
    );
}

/// 每日活动表:日期 + 分段条 + tokens + cost + calls,最新在前 + 脚注。
fn render_daily_table(frame: &mut Frame, area: Rect, days: &[&DailyTotal], zh: bool) {
    let th = t::get();
    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", i18n::t(i18n::K::DailyActivity)),
            Style::default().fg(th.accent),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.border));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // 只列活动日(有 tokens),最新在前;超出容量时脚注交代
    let active: Vec<&DailyTotal> = days
        .iter()
        .rev()
        .copied()
        .filter(|d| d.tokens > 0)
        .collect();
    let capacity = inner.height.saturating_sub(1) as usize; // 1 行脚注
    if active.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                if zh { "所选周期暂无本机记录" } else { "no local records in this period" },
                Style::default().fg(th.dim),
            ))),
            inner,
        );
        return;
    }
    let shown = &active[..active.len().min(capacity)];
    let max_tokens = shown.iter().map(|d| d.tokens).max().unwrap_or(1).max(1);
    let mut lines: Vec<Line> = Vec::with_capacity(shown.len() + 1);
    for d in shown {
        let md = d.date.get(5..).unwrap_or(&d.date).to_string();
        let bar_w = ((d.tokens as f64 / max_tokens as f64) * 12.0).round().max(2.0) as usize;
        let mut row = vec![Span::styled(format!("{md} "), Style::default().fg(th.dim))];
        row.extend(seg_bar(&d.agg, bar_w));
        row.push(Span::raw(" "));
        row.push(Span::styled(
            format!("{:>7}", fmt_tokens_plain(d.tokens)),
            Style::default().fg(th.border),
        ));
        row.push(Span::raw(" "));
        row.push(Span::styled(
            format!("{:>8}", fmt_usd(d.cost)),
            Style::default().fg(th.warn),
        ));
        row.push(Span::raw(" "));
        row.push(Span::styled(format!("{:>5}", d.messages), Style::default().fg(th.dim)));
        lines.push(Line::from(row));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    if active.len() > shown.len() {
        if let Some(row) = inner.rows().nth(inner.height.saturating_sub(1) as usize) {
            let footnote = if zh {
                format!("显示 {} / {} 个活动日 · 最新在前", shown.len(), active.len())
            } else {
                format!("showing {} of {} active days · newest first", shown.len(), active.len())
            };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(footnote, Style::default().fg(th.dim)))),
                row,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_days_slices_from_tail() {
        let ledger = fixture_ledger(200);
        assert_eq!(period_days(Period::Today, &ledger).len(), 1);
        assert_eq!(period_days(Period::D7, &ledger).len(), 7);
        assert_eq!(period_days(Period::D30, &ledger).len(), 30);
        assert_eq!(period_days(Period::D90, &ledger).len(), 90);
        // All 返回全部构造天数(fixture 200 天 > 26 周网格,切片不截断)
        assert_eq!(period_days(Period::All, &ledger).len(), 200);
    }

    #[test]
    fn dot_lines_render_block_glyphs() {
        let lines = dot_lines("42", ratatui::style::Color::Rgb(0, 0, 0));
        assert_eq!(lines.len(), 3);
        let text: String = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect();
        assert!(text.contains('█') || text.contains('▀') || text.contains('▄'));
        // 4 列/字形 × 2 字形
        assert_eq!(lines[0].spans[0].content.chars().count(), 8);
    }

    #[test]
    fn dot_tokens_scales() {
        assert_eq!(dot_tokens(1_150_000_000), "1.15B");
        assert_eq!(dot_tokens(14_700_000), "14.70M");
        assert_eq!(dot_tokens(980), "980");
    }

    #[test]
    fn seg_bar_widths_sum_to_width() {
        let mut agg = crate::ledger::Agg::default();
        agg.input = 600;
        agg.output = 300;
        agg.cache_read = 100;
        let spans = seg_bar(&agg, 12);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 12);
    }

    #[test]
    fn sparkline_maps_levels() {
        assert_eq!(sparkline(&[0, 0]), "▁▁");
        assert_eq!(sparkline(&[0, 10]), "▁█");
        let empty: Vec<i64> = Vec::new();
        assert_eq!(sparkline(&empty), "");
    }

    #[test]
    fn cache_hit_follows_gui_formula() {
        let mut agg = crate::ledger::Agg::default();
        agg.input = 100;
        agg.cache_read = 300;
        assert!((cache_hit(&agg).unwrap() - 75.0).abs() < f64::EPSILON);
        assert!(cache_hit(&crate::ledger::Agg::default()).is_none());
    }

    fn fixture_ledger(days: usize) -> LedgerReport {
        // 构造指定天数的 daily_totals(字段哨兵由 aggregate 测试覆盖,这里只测切片)
        let mut ledger = crate::ledger::aggregate(Vec::new(), chrono::Local::now());
        ledger.daily_totals = (0..days)
            .map(|i| crate::ledger::DailyTotal {
                date: format!("2026-01-{:02}", i + 1),
                tokens: i as i64,
                cost: 0.0,
                messages: 0,
                agg: crate::ledger::Agg::default(),
            })
            .collect();
        ledger
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::ledger::{DailyTotal, LedgerReport};
    use crate::ui::{Page, ViewState};
    use ratatui::{Terminal, backend::TestBackend};

    fn rich_ledger(now: chrono::DateTime<chrono::Local>) -> LedgerReport {
        let mut ledger = crate::ledger::aggregate(Vec::new(), now);
        let mut sources = Vec::new();
        for client in ["codex", "claude", "omp", "opencode", "zcode", "dsh"] {
            sources.push(crate::ledger::SourceStatus {
                client,
                records: 100,
                error: None,
            });
        }
        ledger.sources = sources;
        ledger.daily_totals = (0..182)
            .map(|i| {
                let mut agg = crate::ledger::Agg::default();
                agg.input = 1_000_000 - i * 3_000;
                agg.output = 300_000 - i * 800;
                agg.cache_read = 8_000_000 - i * 20_000;
                agg.cache_write = 500_000 - i * 1_000;
                agg.messages = 90 - (i % 60) as u64;
                let tokens = agg.total();
                DailyTotal {
                    date: (now.date_naive() - chrono::Duration::days(181 - i as i64))
                        .format("%Y-%m-%d")
                        .to_string(),
                    tokens,
                    cost: 1.0 + i as f64 % 7.0,
                    messages: agg.messages,
                    agg,
                }
            })
            .collect();
        ledger
    }

    #[test]
    fn overview_renders_tabs_cards_and_table() {
        let ledger = rich_ledger(chrono::Local::now());
        let view = ViewState {
            page: Page::Overview,
            snapshot: None,
            refreshing: false,
            last_error: None,
            age_secs: Some(3),
            ledger: Some(&ledger),
            period: Period::D30,
            sel_agent: None,
            sel_day: None,
            sel_trend: None,
            sel_heat: None,
            chart_mode: true,
            trend_range: crate::ui::TrendRange::H24,
            trend_metric: crate::ui::TrendMetric::Tokens,
            dim: 0,
            model_offset: 0,
            tick: 0,
            show_help: false,
        };
        let backend = TestBackend::new(150, 34);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        term.draw(|f| crate::ui::render(f, &view, &mut hits)).unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect::<String>()
            .split_whitespace()
            .collect::<String>();
        // 周期 tabs(五档,当前带方括号)
        assert!(text.contains("[30天]"), "缺周期 tabs");
        assert!(text.contains("90天") && text.contains("今日"));
        // 三卡
        assert!(text.contains("累计Tokens") || text.contains("TotalTokens"));
        assert!(text.contains("累计调用") || text.contains("TotalRuns"));
        assert!(text.contains("来源健康") || text.contains("Sourcehealth"));
        // 来源健康行(六源)
        assert!(text.contains("codex") && text.contains("claude"));
        // 每日活动表 + 脚注
        assert!(text.contains("每日活动") || text.contains("Dailyactivity"));
        assert!(text.contains("最新在前") || text.contains("newestfirst"));
        // 周期 tabs 命中区
        assert_eq!(hits.period_tabs.len(), 5);
    }
}
