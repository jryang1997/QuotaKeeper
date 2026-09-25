//! 盲文环形 gauge:每终端格 2×4 子像素采样,环带内的点按位掩码映射到
//! U+2800–28FF 盲文字符(Sarasa 全覆盖,已验证)。
//!
//! - 支持多**同心环带**(`bands`,外→内排列,如 5h/周/月 三个环"环环相扣"),
//!   每带独立比例与颜色,带间留缝;
//! - 弧线 = 已用比例,自顶部(12 点方向)顺时针;
//! - 一格内的盲文点只有一种前景色:取最外环的命中,弧点优先;
//! - 环心无点的格子留给 label,直接写 buffer(不会擦掉环带)。

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

/// 子像素(行 0..4, 列 0..2)→ 盲文位。
const BRAILLE_BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

const BLANK: char = '\u{2800}';

/// 中心孔半径占比(为大数字环心留空间)。
const HOLE_RATIO: f64 = 0.52;

/// 一个环带:比例 + 弧线样式 + 轨道样式(同一圈上,弧外部分为轨道)。
#[derive(Debug, Clone, Copy)]
pub struct Band {
    pub ratio: f64,
    pub arc: Style,
    pub track: Style,
}

pub struct RingGauge {
    pub bands: Vec<Band>,
    /// 环心文字(小字号回退),如 "57%"。
    pub label: Option<String>,
    /// 环心大字(点阵),渲染不下时回退 label。
    pub big: Option<String>,
    pub label_style: Style,
}

impl RingGauge {
    /// 单环便捷构造(兼容旧用法)。
    pub fn new(ratio: f64, arc_style: Style, track_style: Style, label: Option<String>) -> Self {
        Self {
            bands: vec![Band {
                ratio,
                arc: arc_style,
                track: track_style,
            }],
            label,
            big: None,
            label_style: arc_style.add_modifier(Modifier::BOLD),
        }
    }

    /// 同心环带(bands 外 → 内),`big` 为环心大字(数字+%)。
    pub fn concentric(
        bands: Vec<Band>,
        label: Option<String>,
        big: Option<String>,
        label_style: Style,
    ) -> Self {
        Self {
            bands,
            label,
            big,
            label_style,
        }
    }

    /// 顶部为 0、顺时针的角度 ∈ [0, 2π)。屏幕坐标 y 向下。
    fn angle_cw_from_top(dx: f64, dy: f64) -> f64 {
        let mut phi = dx.atan2(-dy);
        if phi < 0.0 {
            phi += std::f64::consts::TAU;
        }
        phi
    }

    /// 各环带的 (r_out, r_in),外 → 内;带间留缝,中心留大字孔。
    fn band_radii(&self, r_max: f64) -> Vec<(f64, f64)> {
        let n = self.bands.len().max(1);
        let gap = r_max * 0.06;
        let hole = r_max * HOLE_RATIO;
        let stroke = (r_max - hole - gap * (n - 1) as f64) / n as f64;
        (0..n)
            .map(|i| {
                let r_out = r_max - i as f64 * (stroke + gap);
                (r_out, r_out - stroke)
            })
            .collect()
    }

    fn label_style(&self) -> Style {
        self.label_style
    }
}

impl Widget for RingGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 5 || area.height < 4 || self.bands.is_empty() {
            return;
        }
        // 子像素网格:每格 2 列 × 4 行(终端格约 1:2,子像素近似方形)
        let w = area.width as f64 * 2.0;
        let h = area.height as f64 * 4.0;
        let (cx, cy) = (w / 2.0, h / 2.0);
        let r_max = w.min(h) / 2.0 - 1.0;
        if r_max < 6.0 {
            return;
        }
        let radii = self.band_radii(r_max);

        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let mut bits = 0u8;
                let mut style: Option<Style> = None;
                let mut style_rank = u8::MAX;
                for (row, bits_row) in BRAILLE_BITS.iter().enumerate() {
                    for (col, bit) in bits_row.iter().enumerate() {
                        let sx = (x - area.x) as f64 * 2.0 + col as f64 + 0.5;
                        let sy = (y - area.y) as f64 * 4.0 + row as f64 + 0.5;
                        let (dx, dy) = (sx - cx, sy - cy);
                        let dist = dx.hypot(dy);
                        // 找命中的环带(外环优先)
                        for (band_idx, ((r_out, r_in), band)) in
                            radii.iter().zip(&self.bands).enumerate()
                        {
                            if *r_in <= dist && dist <= *r_out {
                                bits |= bit;
                                let is_arc = Self::angle_cw_from_top(dx, dy)
                                    <= band.ratio.clamp(0.0, 1.0) * std::f64::consts::TAU;
                                // 样式:环带越靠外越优先;同一带内弧点优先于轨道点
                                let rank = (band_idx as u8) * 2 + u8::from(!is_arc);
                                if rank < style_rank {
                                    style_rank = rank;
                                    style = Some(if is_arc { band.arc } else { band.track });
                                }
                                break;
                            }
                        }
                    }
                }
                if bits == 0 {
                    continue;
                }
                let ch = char::from_u32(0x2800 + bits as u32).unwrap_or(BLANK);
                buf[(x, y)]
                    .set_char(ch)
                    .set_style(style.unwrap_or_default());
            }
        }

        // 环心大字:角点全部落在中心孔内才渲染,否则回退小字号
        if let Some(big) = &self.big
            && let Some(g) = crate::ui::bigtext::grid(big)
        {
            let half_w = g.cols as f64; // 子像素半宽(2 子像素/列)
            let half_h = g.rows as f64 * 2.0; // 子像素半高(4 子像素/行)
            let corner = half_w.hypot(half_h);
            if corner <= r_max * HOLE_RATIO {
                crate::ui::bigtext::render(&g, area, buf, self.label_style());
                return;
            }
        }

        // 环心小字回退:只覆盖无盲文点的格子
        if let Some(label) = &self.label
            && !label.is_empty()
        {
            let lw = label.chars().count() as u16;
            if lw + 2 <= area.width {
                let y = area.y + area.height / 2;
                let x0 = area.x + (area.width - lw) / 2;
                for (i, ch) in label.chars().enumerate() {
                    let x = x0 + i as u16;
                    let cell = &mut buf[(x, y)];
                    let dotted = cell
                        .symbol()
                        .chars()
                        .any(|c| ('\u{2801}'..='\u{28FF}').contains(&c));
                    if dotted {
                        continue;
                    }
                    cell.set_char(ch).set_style(self.label_style());
                }
            }
        }
    }
}

/// 测试辅助:某格是否为可见盲文点(排除 U+2800 空白)。
#[cfg(test)]
pub(crate) fn is_dotted(symbol: &str) -> bool {
    symbol
        .chars()
        .any(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, style::Color};

    fn render_ring(ratio: f64) -> Terminal<TestBackend> {
        let backend = TestBackend::new(21, 11);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            f.render_widget(
                RingGauge::new(
                    ratio,
                    Style::default().fg(Color::Green),
                    Style::default().fg(Color::DarkGray),
                    Some(format!("{:.0}%", ratio * 100.0)),
                ),
                f.area(),
            )
        })
        .unwrap();
        term
    }

    fn cell_fg(term: &Terminal<TestBackend>, x: u16, y: u16) -> Color {
        term.backend().buffer()[(x, y)].style().fg.unwrap()
    }

    #[test]
    fn ratio_zero_is_all_track() {
        let term = render_ring(0.0);
        let b = term.backend().buffer();
        let dotted = b.content().iter().filter(|c| is_dotted(c.symbol())).count();
        assert!(dotted > 40, "轨道点太少: {dotted}");
        for c in b.content().iter().filter(|c| is_dotted(c.symbol())) {
            assert_eq!(c.style().fg, Some(Color::DarkGray));
        }
    }

    #[test]
    fn ratio_one_is_all_arc() {
        let term = render_ring(1.0);
        for c in term
            .backend()
            .buffer()
            .content()
            .iter()
            .filter(|c| is_dotted(c.symbol()))
        {
            assert_eq!(c.style().fg, Some(Color::Green));
        }
    }

    #[test]
    fn quarter_ratio_fills_top_right() {
        // 顺时针自顶部 90° = 右上象限:顶部中点应为弧色,底部中点应为轨道色
        let term = render_ring(0.25);
        let (w, h) = (21, 11);
        let top_arc = (0..w).any(|x| {
            is_dotted(term.backend().buffer()[(x, 0)].symbol())
                && cell_fg(&term, x, 0) == Color::Green
        });
        let bottom_track = (0..w).any(|x| {
            is_dotted(term.backend().buffer()[(x, h - 1)].symbol())
                && cell_fg(&term, x, h - 1) == Color::DarkGray
        });
        assert!(top_arc, "顶部应为弧色");
        assert!(bottom_track, "底部应为轨道色");
    }

    #[test]
    fn center_label_is_written() {
        let term = render_ring(0.5);
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("50%"), "环心文字缺失");
    }

    #[test]
    fn tiny_area_does_not_panic() {
        let backend = TestBackend::new(4, 3);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            f.render_widget(
                RingGauge::new(0.5, Style::default(), Style::default(), Some("50%".into())),
                f.area(),
            )
        })
        .unwrap();
    }

    #[test]
    fn angle_reference_points() {
        use std::f64::consts::FRAC_PI_2;
        assert!(RingGauge::angle_cw_from_top(1.0, 0.0) - FRAC_PI_2 < 1e-9);
        assert!(RingGauge::angle_cw_from_top(0.0, 1.0) - std::f64::consts::PI < 1e-9);
        assert!((RingGauge::angle_cw_from_top(-1.0, 0.0) - 3.0 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn concentric_bands_render_inner_and_outer_colors() {
        // 两个同心环:外环绿(满弧),内环红(满弧);边缘区有绿点、中心区有红点
        let backend = TestBackend::new(25, 13);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            f.render_widget(
                RingGauge::concentric(
                    vec![
                        Band {
                            ratio: 1.0,
                            arc: Style::default().fg(Color::Green),
                            track: Style::default().fg(Color::DarkGray),
                        },
                        Band {
                            ratio: 1.0,
                            arc: Style::default().fg(Color::Red),
                            track: Style::default().fg(Color::DarkGray),
                        },
                    ],
                    None,
                    None,
                    Style::default(),
                ),
                f.area(),
            )
        })
        .unwrap();
        let b = term.backend().buffer();
        let has_green = b
            .content()
            .iter()
            .any(|c| is_dotted(c.symbol()) && c.style().fg == Some(Color::Green));
        let has_red = b
            .content()
            .iter()
            .any(|c| is_dotted(c.symbol()) && c.style().fg == Some(Color::Red));
        assert!(has_green, "缺外环绿点");
        assert!(has_red, "缺内环红点");
    }

    #[test]
    fn concentric_band_radii_are_ordered_and_gapped() {
        let g = RingGauge {
            bands: vec![
                Band {
                    ratio: 0.5,
                    arc: Style::default(),
                    track: Style::default(),
                },
                Band {
                    ratio: 0.5,
                    arc: Style::default(),
                    track: Style::default(),
                },
                Band {
                    ratio: 0.5,
                    arc: Style::default(),
                    track: Style::default(),
                },
            ],
            label: None,
            big: None,
            label_style: Style::default(),
        };
        let radii = g.band_radii(19.0);
        assert_eq!(radii.len(), 3);
        // 外 → 内递减
        assert!(radii[0].0 > radii[1].0 && radii[1].0 > radii[2].0);
        // 带间有缝:外环 r_in > 中环 r_out
        assert!(radii[0].1 > radii[1].0);
        // 内环不低于中心孔(大字环心)
        assert!(radii[2].1 >= 19.0 * HOLE_RATIO - 1e-9);
    }
}
