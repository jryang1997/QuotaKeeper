//! 近 24 小时 / 近 14 天按 Agent 堆叠趋势。

use chrono::{DateTime, Local};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ledger::types::CLIENTS;
use crate::ledger::{LedgerReport, client_label, format_tokens, format_usd, types::Agg};

use super::{HitRegions, TrendMetric, TrendRange, i18n, theme};

const RAMP: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

struct PlotBucket {
    label: String,
    detail_label: String,
    total: f64,
    parts: Vec<(&'static str, f64)>,
}

fn value(agg: &Agg, metric: TrendMetric) -> f64 {
    match metric {
        TrendMetric::Tokens => agg.total().max(0) as f64,
        TrendMetric::Cost => agg.cost().max(0.0),
    }
}

fn data(ledger: &LedgerReport, range: TrendRange, metric: TrendMetric) -> Vec<PlotBucket> {
    match range {
        TrendRange::H24 => ledger
            .trend_24h
            .iter()
            .map(|b| PlotBucket {
                label: DateTime::from_timestamp_millis(b.start_ms)
                    .map(|dt| dt.with_timezone(&Local).format("%H:%M").to_string())
                    .unwrap_or_default(),
                detail_label: DateTime::from_timestamp_millis(b.start_ms)
                    .map(|dt| {
                        let start = dt.with_timezone(&Local);
                        let end = (dt + chrono::Duration::hours(1)).with_timezone(&Local);
                        format!("{}–{}", start.format("%H:%M"), end.format("%H:%M"))
                    })
                    .unwrap_or_default(),
                total: value(&b.agg, metric),
                parts: b
                    .per_client
                    .iter()
                    .map(|(client, agg)| (*client, value(agg, metric)))
                    .collect(),
            })
            .collect(),
        TrendRange::D14 => ledger
            .by_day
            .iter()
            .map(|d| PlotBucket {
                label: d.date.get(5..).unwrap_or(&d.date).to_owned(),
                detail_label: d.date.clone(),
                total: value(&d.agg, metric),
                parts: d
                    .per_client
                    .iter()
                    .map(|(client, agg)| (*client, value(agg, metric)))
                    .collect(),
            })
            .collect(),
    }
}

fn format_value(v: f64, metric: TrendMetric) -> String {
    match metric {
        TrendMetric::Tokens => format!("{} tok", format_tokens(v.round() as i64)),
        TrendMetric::Cost => format_usd(v),
    }
}

pub(crate) fn detail(
    ledger: &LedgerReport,
    range: TrendRange,
    metric: TrendMetric,
    index: usize,
) -> Option<(String, String)> {
    let buckets = data(ledger, range, metric);
    let bucket = buckets.get(index)?;
    let heading = format!(
        "{}  {}",
        bucket.detail_label,
        format_value(bucket.total, metric)
    );
    let mut parts = bucket.parts.clone();
    parts.sort_by(|a, b| b.1.total_cmp(&a.1));
    let breakdown = parts
        .iter()
        .filter(|(_, v)| *v > 0.0)
        .take(3)
        .map(|(client, v)| format!("{} {}", client_label(client), format_value(*v, metric)))
        .collect::<Vec<_>>()
        .join(" · ");
    Some((heading, breakdown))
}

fn series_color(client: &str) -> ratatui::style::Color {
    let idx = CLIENTS
        .iter()
        .position(|(id, _)| *id == client)
        .unwrap_or(0);
    theme::get().series[idx]
}

pub(crate) fn render(
    frame: &mut Frame,
    area: Rect,
    ledger: &LedgerReport,
    range: TrendRange,
    metric: TrendMetric,
    hits: &mut HitRegions,
) {
    if area.width < 20 || area.height < 4 {
        return;
    }
    let th = theme::get();
    let buckets = data(ledger, range, metric);
    let max = buckets.iter().map(|b| b.total).fold(0.0f64, f64::max);
    if max <= 0.0 {
        let label = if metric == TrendMetric::Cost {
            i18n::t(i18n::K::NoCost)
        } else {
            i18n::t(i18n::K::NoData)
        };
        frame.render_widget(
            Paragraph::new(label).style(Style::default().fg(th.dim)),
            area,
        );
        return;
    }

    let mut legend = Vec::new();
    let mut used = 0u16;
    for (client, name) in CLIENTS {
        if !buckets
            .iter()
            .any(|b| b.parts.iter().any(|(id, v)| *id == client && *v > 0.0))
        {
            continue;
        }
        let width = name.len() as u16 + 2;
        if used + width > area.width {
            break;
        }
        legend.push(Span::styled(
            format!("{name}  "),
            Style::default().fg(series_color(client)),
        ));
        used += width;
    }
    frame.render_widget(
        Paragraph::new(Line::from(legend)),
        Rect { height: 1, ..area },
    );

    let max_label = match metric {
        TrendMetric::Tokens => format_tokens(max.ceil() as i64),
        TrendMetric::Cost => format_usd(max),
    };
    let axis_w = (max_label.len() as u16 + 1).min(10);
    let plot_x = area.x + axis_w;
    let plot_w = area.width.saturating_sub(axis_w);
    let plot_y = area.y + 1;
    let plot_h = area.height.saturating_sub(2);
    if plot_w == 0 || plot_h == 0 {
        return;
    }
    for (i, ch) in max_label.chars().take(axis_w as usize).enumerate() {
        frame.buffer_mut()[(area.x + i as u16, plot_y)]
            .set_char(ch)
            .set_style(Style::default().fg(th.dim));
    }
    frame.buffer_mut()[(area.x, plot_y + plot_h - 1)]
        .set_char('0')
        .set_style(Style::default().fg(th.dim));

    let visible = buckets.len().min(plot_w as usize);
    let first = buckets.len() - visible;
    let slot_w = (plot_w / visible as u16).max(1);
    let bar_w = if slot_w > 1 { slot_w - 1 } else { 1 };
    for (visible_i, bucket) in buckets.iter().skip(first).enumerate() {
        let idx = first + visible_i;
        let bx = plot_x + visible_i as u16 * slot_w;
        if bx >= area.right() {
            break;
        }
        let cells = (bucket.total / max * plot_h as f64).ceil() as u16;
        for row in 0..cells.min(plot_h) {
            let value_at_center = (max * (row as f64 + 0.5) / plot_h as f64).min(bucket.total);
            let mut cumulative = 0.0;
            let mut color = th.accent;
            for (client, part) in &bucket.parts {
                cumulative += part;
                if cumulative >= value_at_center {
                    color = series_color(client);
                    break;
                }
            }
            let ch = if row + 1 == cells && bucket.total / max * (plot_h as f64) < cells as f64 {
                let fraction = (bucket.total / max * plot_h as f64).fract();
                RAMP[(fraction * 8.0).ceil().clamp(1.0, 8.0) as usize]
            } else {
                '█'
            };
            for col in 0..bar_w.min(area.right() - bx) {
                frame.buffer_mut()[(bx + col, plot_y + plot_h - 1 - row)]
                    .set_char(ch)
                    .set_style(Style::default().fg(color));
            }
        }
        hits.trend_buckets.push((
            Rect {
                x: bx,
                y: plot_y,
                width: slot_w.min(area.right() - bx),
                height: plot_h,
            },
            idx,
        ));
        let label_this = match range {
            TrendRange::H24 => idx.is_multiple_of(6) || idx == buckets.len() - 1,
            TrendRange::D14 => idx == 0 || idx == buckets.len() / 2 || idx == buckets.len() - 1,
        };
        if label_this {
            let label_x = if idx == buckets.len() - 1 {
                bx.min(area.right().saturating_sub(bucket.label.len() as u16))
            } else {
                bx
            };
            for (li, ch) in bucket.label.chars().enumerate() {
                let x = label_x + li as u16;
                if x < area.right() {
                    frame.buffer_mut()[(x, area.bottom() - 1)]
                        .set_char(ch)
                        .set_style(Style::default().fg(th.dim).add_modifier(Modifier::DIM));
                }
            }
        }
    }
}
