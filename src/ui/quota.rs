//! 配额页:每条额度用文字与横向块条呈现。
//! 阈值口径抄 OMP usage-cli(80/100):>=100 exhausted,>=80 warning。

use chrono::Datelike;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::model::{Card, Limit, ProviderId, Status, format_countdown};
use crate::ui::ViewState;
use crate::ui::i18n;
use crate::ui::theme as t;

fn status_color(s: Status) -> Color {
    let th = t::get();
    match s {
        Status::Ok => th.ok,
        Status::Warning => th.warn,
        Status::Exhausted => th.err,
    }
}

fn reset_text(l: &Limit) -> String {
    l.resets_in_ms
        .map(|ms| {
            if ms < 0 {
                "reset —".to_string()
            } else {
                format!("reset {}", format_countdown(ms))
            }
        })
        .unwrap_or_else(|| "reset —".to_string())
}

fn limit_text_line(l: &Limit, now: chrono::DateTime<chrono::Local>) -> Line<'_> {
    let mut spans = vec![
        Span::raw(l.label.clone()),
        Span::styled(
            format!("  {:.0}%", l.used_pct),
            Style::default()
                .fg(status_color(l.status))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {}", reset_text(l)),
            Style::default().fg(t::get().dim),
        ),
    ];
    // 燃烧率预测:窗口内匀速外推;急迫(<1h)用 err 色提醒
    if let Some((text, color)) = predict(l, now) {
        spans.push(Span::styled(
            format!(" · {text}"),
            Style::default().fg(color),
        ));
    }
    Line::from(spans)
}

/// 粗块条:█ 实 / ░ 虚,占满 inner 宽度。
fn limit_bar_line(l: &Limit, width: u16) -> Line<'static> {
    let width = width.max(4) as usize;
    let filled = ((l.used_pct / 100.0).clamp(0.0, 1.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    Line::from(vec![
        Span::styled(
            "█".repeat(filled),
            Style::default().fg(status_color(l.status)),
        ),
        Span::styled(
            "░".repeat(width - filled),
            Style::default().fg(t::get().track),
        ),
    ])
}

/// 窗口长度(按额度标签关键字);Monthly 走自然月,单独处理.
fn window_len(label: &str) -> Option<chrono::Duration> {
    let l = label.to_lowercase();
    if l.contains("5h") || l.contains("5 hour") {
        Some(chrono::Duration::hours(5))
    } else if (l.contains("7") && l.contains("day")) || l.contains("week") {
        Some(chrono::Duration::weeks(1))
    } else {
        None
    }
}

/// 按窗口已过时间和已用百分比线性外推剩余秒数。
pub(crate) fn eta_seconds(l: &Limit, now: chrono::DateTime<chrono::Local>) -> Option<i64> {
    let x = l.used_pct;
    if x <= 0.0 || x >= 100.0 {
        return None;
    }
    let l_l = l.label.to_lowercase();
    let (elapsed_secs, _total_secs) = if l_l.contains("month") || l_l.contains("月") {
        // 自然月:已过 = 本月起点至今
        let som_ms = now
            .date_naive()
            .with_day(1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .and_then(|d| d.and_local_timezone(now.timezone()).single())
            .map(|d| d.timestamp_millis())?;
        let e = (now.timestamp_millis() - som_ms) / 1000;
        let days = chrono::NaiveDate::from_ymd_opt(
            now.year() + if now.month() == 12 { 1 } else { 0 },
            if now.month() == 12 {
                1
            } else {
                now.month() + 1
            },
            1,
        )
        .map(|d| d.num_days_from_ce() - now.date_naive().num_days_from_ce())
        .unwrap_or(30);
        (e as f64, (days * 86400) as f64)
    } else {
        let wl = window_len(&l.label)?;
        let r = l.resets_in_ms?;
        if r < 0 {
            return None;
        }
        // 窗口总长与剩余都是毫秒:已过 = 总长 - 剩余(转秒)
        let total_ms = wl.num_milliseconds() as f64;
        ((total_ms - r as f64) / 1000.0, total_ms / 1000.0)
    };
    if elapsed_secs <= 0.0 {
        return None;
    }
    let secs = elapsed_secs * (100.0 - x) / x;
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    Some(secs as i64)
}

/// 配额页沿用同一 ETA 计算，只负责文字与颜色。
fn predict(l: &Limit, now: chrono::DateTime<chrono::Local>) -> Option<(String, Color)> {
    let secs = eta_seconds(l, now)?;
    let cd = crate::model::format_countdown(secs.saturating_mul(1000));
    let zh = i18n::lang() == i18n::Lang::Zh;
    let text = if zh {
        format!("预计 {cd} 耗尽")
    } else {
        format!("ETA {cd}")
    };
    Some((
        text,
        if secs < 3600 {
            t::get().err
        } else {
            t::get().dim
        },
    ))
}

fn card_height(card: &Card) -> u16 {
    match card {
        // 每条额度两行(文字 + 块条)+ 上下边框
        Card::Live(r) => r.limits.len() as u16 * 2 + 2,
        Card::Missing { .. } => 3,
    }
}

fn compact_height(card: &Card) -> u16 {
    match card {
        Card::Live(r) => r.limits.len() as u16 + 2,
        Card::Missing { .. } => 3,
    }
}

pub fn render(frame: &mut Frame, area: Rect, st: &ViewState) {
    render_cards(frame, area, st);
}

/// 卡片标题:plan_type 与 provider 名重复时(如 "OpenCode Go · $10/mo · OpenCode Go")去掉重复段.
fn card_title(r: &crate::model::Report) -> String {
    let base = r.provider.title();
    if r.plan_type.is_empty() || base.to_lowercase().contains(&r.plan_type.to_lowercase()) {
        base.to_string()
    } else {
        format!("{base} · {}", r.plan_type)
    }
}

// ---------------- 卡片视图 ----------------

fn render_cards(frame: &mut Frame, area: Rect, st: &ViewState) {
    let th = t::get();
    let Some(snap) = st.snapshot else {
        frame.render_widget(Paragraph::new(i18n::t(i18n::K::Loading)), area);
        return;
    };
    let full_height: u16 = snap.cards.iter().map(card_height).sum();
    let compact = full_height > area.height;
    let compact_total: u16 = snap.cards.iter().map(compact_height).sum();
    if compact && compact_total > area.height && area.width >= 48 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(area);
        let middle = snap.cards.len().div_ceil(2);
        render_card_list(frame, columns[0], &snap.cards[..middle], th, true);
        render_card_list(frame, columns[1], &snap.cards[middle..], th, true);
    } else {
        render_card_list(frame, area, &snap.cards, th, compact);
    }
}

fn render_card_list(frame: &mut Frame, area: Rect, cards: &[Card], th: &t::Theme, compact: bool) {
    let constraints: Vec<Constraint> = cards
        .iter()
        .map(|c| {
            Constraint::Length(if compact {
                compact_height(c)
            } else {
                card_height(c)
            })
        })
        .collect();
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    for (card, a) in cards.iter().zip(areas.iter()) {
        if compact {
            render_compact_card(frame, *a, card, th);
        } else {
            render_card(frame, *a, card, th);
        }
    }
}

fn render_compact_card(frame: &mut Frame, area: Rect, card: &Card, th: &t::Theme) {
    let Card::Live(report) = card else {
        render_card(frame, area, card, th);
        return;
    };
    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", card_title(report)),
            Style::default().fg(th.accent),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.border));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    for (index, limit) in report.limits.iter().enumerate() {
        let bar_width = inner.width.saturating_sub(25).clamp(4, 14) as usize;
        let filled = ((limit.used_pct / 100.0).clamp(0.0, 1.0) * bar_width as f64).round() as usize;
        let label: String = limit.label.chars().take(12).collect();
        let line = Line::from(vec![
            Span::raw(format!("{label} ")),
            Span::styled(
                format!("{:.0}% ", limit.used_pct),
                Style::default()
                    .fg(status_color(limit.status))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "█".repeat(filled),
                Style::default().fg(status_color(limit.status)),
            ),
            Span::styled(
                "░".repeat(bar_width.saturating_sub(filled)),
                Style::default().fg(th.track),
            ),
            Span::styled(
                format!(" {}", reset_text(limit)),
                Style::default().fg(th.dim),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(line),
            Rect {
                x: inner.x,
                y: inner.y + index as u16,
                width: inner.width,
                height: 1,
            },
        );
    }
}

fn render_card(frame: &mut Frame, area: Rect, card: &Card, th: &t::Theme) {
    match card {
        Card::Live(r) => {
            let title = format!(" {} ", card_title(r));
            let block = Block::default()
                .title(Span::styled(title, Style::default().fg(th.accent)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.border));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let rows: Vec<Rect> = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    r.limits
                        .iter()
                        .flat_map(|_| [Constraint::Length(1), Constraint::Length(1)])
                        .collect::<Vec<_>>(),
                )
                .split(inner)
                .to_vec();
            let now = chrono::Local::now();
            for (l, pair) in r.limits.iter().zip(rows.chunks(2)) {
                frame.render_widget(Paragraph::new(limit_text_line(l, now)), pair[0]);
                frame.render_widget(Paragraph::new(limit_bar_line(l, inner.width)), pair[1]);
            }
        }
        Card::Missing { provider } => {
            let hint = match provider {
                ProviderId::Goat => i18n::t(i18n::K::MGoat),
                ProviderId::Antigravity => i18n::t(i18n::K::MAg),
                _ => i18n::t(i18n::K::MGo),
            };
            let block = Block::default()
                .title(format!(" {} ", provider.title()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.border))
                .style(Style::default().fg(th.dim));
            let p = Paragraph::new(Line::from(vec![Span::styled(
                hint,
                Style::default().fg(th.dim),
            )]))
            .block(block);
            frame.render_widget(p, area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{HitRegions, Page};
    use ratatui::{Terminal, backend::TestBackend};

    fn view<'a>(snap: &'a crate::model::Snapshot) -> ViewState<'a> {
        ViewState {
            page: Page::Quota,
            snapshot: Some(snap),
            refreshing: false,
            last_error: None,
            age_secs: Some(3),
            ledger: None,
            sel_agent: None,
            sel_day: None,
            sel_trend: None,
            sel_heat: None,
            chart_mode: false,
            trend_range: crate::ui::TrendRange::H24,
            trend_metric: crate::ui::TrendMetric::Tokens,
            dim: 0,
            tick: 0,
            show_help: false,
        }
    }

    fn snap() -> crate::model::Snapshot {
        crate::model::parse_snapshot(include_str!("../../tests/fixtures/usage.json")).unwrap()
    }

    /// 燃烧率预测:5h 窗口过了一半、用了 50% → 剩余还需约一个窗口长度。
    #[test]
    fn predict_extrapolates_within_window() {
        use chrono::TimeZone;
        let now = chrono::Local
            .with_ymd_and_hms(2026, 9, 25, 12, 0, 0)
            .single()
            .unwrap();
        let l = Limit {
            label: "5 hours".into(),
            used_pct: 50.0,
            // 窗口 5h = 18000s;剩 9000s → 已过 9000s;50% 用了 9000s → 剩余还需 9000s
            resets_in_ms: Some(9_000_000),
            status: crate::model::Status::Ok,
        };
        let (text, color) = predict(&l, now).expect("应能预测");
        assert!(text.contains("2h 30m"), "预测文案 = {text}");
        // 2.5h > 1h → 不急迫(非 err 色)
        assert_ne!(color, t::get().err);
    }

    /// 零用量 / 已耗尽 / 未知窗口 → 不预测。
    #[test]
    fn predict_skips_degenerate_cases() {
        use chrono::TimeZone;
        let now = chrono::Local
            .with_ymd_and_hms(2026, 9, 25, 12, 0, 0)
            .single()
            .unwrap();
        let mk = |pct: f64, label: &str| Limit {
            label: label.into(),
            used_pct: pct,
            resets_in_ms: Some(1_000_000),
            status: crate::model::Status::Ok,
        };
        assert!(predict(&mk(0.0, "5 hours"), now).is_none());
        assert!(predict(&mk(100.0, "5 hours"), now).is_none());
        assert!(predict(&mk(50.0, "weird window"), now).is_none());
    }

    /// 无头渲染:配额卡片标题与 █░ 块条。
    #[test]
    fn cards_view_renders_headless() {
        let s = snap();
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        term.draw(|f| crate::ui::render(f, &view(&s), &mut hits))
            .unwrap();
        // TestBackend 宽字符后带空格,去空白再断言 CJK
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect::<String>()
            .split_whitespace()
            .collect();
        assert!(text.contains("OpenCodeGo"), "缺 Go 卡");
        assert!(text.contains("GOAT"), "缺 GOAT 卡");
        assert!(text.contains('█') && text.contains('░'), "缺块条");
    }
}
