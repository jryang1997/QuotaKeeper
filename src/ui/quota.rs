//! 配额页:卡片视图(每条额度 = 文字行 + █░ 粗块条)与圆环视图。
//!
//! 圆环视图是"环环相扣"的同心三环:外=月、中=周、内=5h,同时呈现三个窗口;
//! 聚焦窗口(点击环体或 ring_window)以饱和色高亮并在环心显示百分比,
//! 其余两环降饱和;右上 ⟳ 按钮或 `v` 键与卡片视图互切。
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
use crate::ui::i18n;
use crate::ui::theme as t;
use crate::ui::{Band, HitRegions, QuotaView, RingGauge, ViewState};

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

fn short_name(p: ProviderId) -> String {
    p.title().split(" ·").next().unwrap_or(">").to_string()
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

/// 燃烧率预测:窗口内匀速外推 —— 已过 E 用了 X% → 剩余 (100-X)% 还需 E*(100-X)/X.
/// 返回 (文案, 颜色);<1h 判急迫用 err 色,否则暗色。X=0 或窗口未知返回 None.
fn predict(l: &Limit, now: chrono::DateTime<chrono::Local>) -> Option<(String, Color)> {
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
    let cd = crate::model::format_countdown((secs * 1000.0) as i64);
    let zh = i18n::lang() == i18n::Lang::Zh;
    let text = if zh {
        format!("预计 {cd} 耗尽")
    } else {
        format!("ETA {cd}")
    };
    Some((
        text,
        if secs < 3600.0 {
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

pub fn render(frame: &mut Frame, area: Rect, st: &ViewState, hits: &mut HitRegions) {
    match st.quota_view {
        QuotaView::Cards => render_cards(frame, area, st),
        QuotaView::Rings => render_rings(frame, area, st, hits),
    }
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
    let constraints: Vec<Constraint> = snap
        .cards
        .iter()
        .map(|c| Constraint::Length(card_height(c)))
        .collect();
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    for (card, a) in snap.cards.iter().zip(areas.iter()) {
        render_card(frame, *a, card, th);
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

// ---------------- 圆环视图(同心环) ----------------

fn render_rings(frame: &mut Frame, area: Rect, st: &ViewState, hits: &mut HitRegions) {
    let th = t::get();
    let Some(snap) = st.snapshot else {
        frame.render_widget(Paragraph::new(i18n::t(i18n::K::Loading)), area);
        return;
    };
    let n = snap.cards.len().max(1) as u32;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints((0..n).map(|_| Constraint::Ratio(1, n)).collect::<Vec<_>>())
        .split(area);
    for (i, (card, col)) in snap.cards.iter().zip(cols.iter()).enumerate() {
        let win = st.ring_window[i.min(st.ring_window.len() - 1)];
        match card {
            Card::Live(r) => {
                // plan_type 与 short_name 重复时只显示一个
                let sn = short_name(r.provider);
                let title = if r.plan_type.is_empty()
                    || sn.to_lowercase().contains(&r.plan_type.to_lowercase())
                {
                    sn
                } else {
                    format!("{sn} · {}", r.plan_type)
                };
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        title,
                        Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
                    ))),
                    *col,
                );
                let rest = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(6),
                        Constraint::Length(r.limits.len() as u16),
                    ])
                    .split(Rect {
                        x: col.x,
                        y: col.y + 1,
                        width: col.width,
                        height: col.height.saturating_sub(1),
                    });
                let focus = win.min(r.limits.len().saturating_sub(1));

                // 同心环带:外 → 内 = limits 逆序(月最外,5h 最内)
                let mut bands = Vec::new();
                for (j, l) in r.limits.iter().enumerate().rev() {
                    let c = status_color(l.status);
                    let (arc, track) = if j == focus {
                        (Style::default().fg(c), Style::default().fg(th.track))
                    } else {
                        (
                            Style::default().fg(t::blend(c, th.track, 0.55)),
                            Style::default().fg(t::blend(th.track, th.dim, 0.35)),
                        )
                    };
                    bands.push(Band {
                        ratio: l.used_pct / 100.0,
                        arc,
                        track,
                    });
                }
                let focused = &r.limits[focus];
                let ring_area = Rect {
                    x: rest[0].x + 2,
                    y: rest[0].y,
                    width: rest[0].width.saturating_sub(4),
                    height: rest[0].height,
                };
                frame.render_widget(
                    RingGauge::concentric(
                        bands,
                        None,
                        Some(format!("{:.0}%", focused.used_pct)),
                        Style::default()
                            .fg(status_color(focused.status))
                            .add_modifier(Modifier::BOLD),
                    ),
                    ring_area,
                );
                hits.ring.push((rest[0], i));

                for (j, lim) in r.limits.iter().enumerate() {
                    let focused_line = j == focus;
                    let marker = if focused_line { "▸ " } else { "  " };
                    frame.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(marker),
                            Span::raw(lim.label.clone()),
                            Span::styled(
                                format!("  {:.0}%", lim.used_pct),
                                Style::default().fg(status_color(lim.status)),
                            ),
                        ]))
                        .style(if focused_line {
                            Style::default().fg(th.accent).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(th.dim)
                        }),
                        Rect {
                            x: rest[1].x + 2,
                            y: rest[1].y + j as u16,
                            width: rest[1].width.saturating_sub(2),
                            height: 1,
                        },
                    );
                }
            }
            Card::Missing { provider } => {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        short_name(*provider),
                        Style::default().fg(th.dim),
                    ))),
                    *col,
                );
                let rest_h = col.height.saturating_sub(1);
                let ring_area = Rect {
                    x: col.x + 2,
                    y: col.y + 1,
                    width: col.width.saturating_sub(4),
                    height: rest_h,
                };
                frame.render_widget(
                    RingGauge::new(
                        0.0,
                        Style::default().fg(th.track),
                        Style::default().fg(th.track),
                        Some("—".into()),
                    ),
                    ring_area,
                );
                hits.ring.push((ring_area, i));
                let hint = match provider {
                    ProviderId::Goat => i18n::t(i18n::K::MGoat),
                    _ => i18n::t(i18n::K::MGo),
                };
                frame.render_widget(
                    Paragraph::new(Span::styled(hint, Style::default().fg(th.dim))),
                    Rect {
                        x: col.x + 1,
                        y: col.bottom().saturating_sub(1),
                        width: col.width.saturating_sub(1),
                        height: 1,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Page;
    use ratatui::{Terminal, backend::TestBackend};

    fn view<'a>(quota_view: QuotaView, snap: &'a crate::model::Snapshot) -> ViewState<'a> {
        ViewState {
            page: Page::Quota,
            snapshot: Some(snap),
            refreshing: false,
            last_error: None,
            age_secs: Some(3),
            ledger: None,
            sel_agent: None,
            sel_day: None,
            sel_heat: None,
            quota_view,
            ring_window: &[1, 1, 2],
            chart_mode: false,
            dim: 0,
            tick: 0,
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

    /// 无头渲染:卡片视图四卡标题 + █░ 块条 + 顶栏按钮都在。
    #[test]
    fn cards_view_renders_headless() {
        let s = snap();
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        term.draw(|f| crate::ui::render(f, &view(QuotaView::Cards, &s), &mut hits))
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
        assert!(text.contains("圆环"), "缺顶栏切换按钮");
    }

    /// 圆环视图:盲文点 + 环心百分比 + 环命中区。
    #[test]
    fn rings_view_renders_headless() {
        let s = snap();
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        term.draw(|f| crate::ui::render(f, &view(QuotaView::Rings, &s), &mut hits))
            .unwrap();
        let b = term.backend().buffer();
        let text: String = b
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect::<String>()
            .split_whitespace()
            .collect();
        assert!(text.contains("卡片"), "缺顶栏切换按钮");
        assert!(text.contains("53%"), "缺 Go 卡环心百分比(周窗口)");
        assert!(text.contains("70%"), "缺 Codex 环心百分比");
        assert!(
            b.content().iter().any(|c| {
                c.symbol()
                    .chars()
                    .any(|ch| ('\u{2801}'..='\u{28FF}').contains(&ch))
            }),
            "缺盲文环"
        );
        assert_eq!(hits.ring.len(), 4, "应有四个环命中区");
    }
}
