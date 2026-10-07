//! 主题系统:四套精选配色(GitHub Light / Tokyo Night / Gruvbox Dark / Catppuccin Mocha),
//! `p` 键循环,选择持久化到 `~/.quotakeeper/theme`。
//!
//! 语义槽位(不绑定具体颜色):
//! - accent:标题/高亮/图表主色;ok/warn/err:配额状态色;
//! - dim:次要文字;track:块条与环的轨道;border:边框;
//! - select_bg:选中行背景;heat[5]:热力图五级(0 级 = 空日/轨道)。
//!
//! 颜色一律 RGB,兼容浅色(GitHub Light)与深色终端。

use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub dim: Color,
    pub track: Color,
    pub border: Color,
    pub select_bg: Color,
    pub select_fg: Color,
    /// Agent 图表用色，按 ledger::types::CLIENTS 固定顺序。
    pub series: [Color; 6],
    /// 热力图五级,索引 0 = 空日底色。
    pub heat: [Color; 5],
}

pub const THEMES: [Theme; 4] = [
    // GitHub Light:贡献图官方色阶,浅色终端下的基准
    Theme {
        name: "GitHub Light",
        accent: Color::Rgb(9, 105, 218),
        ok: Color::Rgb(26, 127, 55),
        warn: Color::Rgb(154, 103, 0),
        err: Color::Rgb(207, 34, 46),
        dim: Color::Rgb(110, 119, 129),
        track: Color::Rgb(208, 215, 222),
        border: Color::Rgb(198, 204, 214),
        select_bg: Color::Rgb(221, 228, 236),
        select_fg: Color::Rgb(31, 35, 40),
        series: [
            Color::Rgb(9, 105, 218),
            Color::Rgb(130, 80, 223),
            Color::Rgb(5, 133, 118),
            Color::Rgb(180, 95, 18),
            Color::Rgb(191, 56, 93),
            Color::Rgb(48, 115, 170),
        ],
        heat: [
            Color::Rgb(208, 215, 222),
            Color::Rgb(155, 233, 168),
            Color::Rgb(64, 196, 99),
            Color::Rgb(48, 161, 78),
            Color::Rgb(33, 110, 57),
        ],
    },
    // Tokyo Night
    Theme {
        name: "Tokyo Night",
        accent: Color::Rgb(122, 162, 247),
        ok: Color::Rgb(158, 206, 106),
        warn: Color::Rgb(224, 175, 104),
        err: Color::Rgb(247, 118, 142),
        dim: Color::Rgb(89, 91, 122),
        track: Color::Rgb(54, 57, 82),
        border: Color::Rgb(41, 46, 66),
        select_bg: Color::Rgb(41, 46, 66),
        select_fg: Color::Rgb(192, 202, 245),
        series: [
            Color::Rgb(122, 162, 247),
            Color::Rgb(187, 154, 247),
            Color::Rgb(115, 218, 202),
            Color::Rgb(255, 182, 108),
            Color::Rgb(247, 142, 178),
            Color::Rgb(137, 200, 238),
        ],
        heat: [
            Color::Rgb(41, 46, 66),
            Color::Rgb(31, 86, 71),
            Color::Rgb(41, 132, 98),
            Color::Rgb(58, 197, 133),
            Color::Rgb(118, 235, 166),
        ],
    },
    // Gruvbox Dark
    Theme {
        name: "Gruvbox Dark",
        accent: Color::Rgb(131, 165, 152),
        ok: Color::Rgb(184, 187, 38),
        warn: Color::Rgb(250, 189, 47),
        err: Color::Rgb(251, 73, 52),
        dim: Color::Rgb(146, 131, 116),
        track: Color::Rgb(60, 56, 54),
        border: Color::Rgb(80, 73, 69),
        select_bg: Color::Rgb(80, 73, 69),
        select_fg: Color::Rgb(251, 241, 199),
        series: [
            Color::Rgb(131, 165, 152),
            Color::Rgb(211, 134, 155),
            Color::Rgb(142, 192, 124),
            Color::Rgb(254, 128, 25),
            Color::Rgb(177, 98, 134),
            Color::Rgb(184, 187, 38),
        ],
        heat: [
            Color::Rgb(60, 56, 54),
            Color::Rgb(82, 86, 34),
            Color::Rgb(130, 132, 36),
            Color::Rgb(184, 187, 38),
            Color::Rgb(220, 223, 88),
        ],
    },
    // Catppuccin Mocha
    Theme {
        name: "Catppuccin Mocha",
        accent: Color::Rgb(137, 180, 250),
        ok: Color::Rgb(166, 227, 161),
        warn: Color::Rgb(249, 226, 175),
        err: Color::Rgb(243, 139, 168),
        dim: Color::Rgb(108, 112, 134),
        track: Color::Rgb(69, 71, 90),
        border: Color::Rgb(88, 91, 112),
        select_bg: Color::Rgb(88, 91, 112),
        select_fg: Color::Rgb(205, 214, 244),
        series: [
            Color::Rgb(137, 180, 250),
            Color::Rgb(203, 166, 247),
            Color::Rgb(148, 226, 213),
            Color::Rgb(250, 179, 135),
            Color::Rgb(245, 194, 231),
            Color::Rgb(137, 220, 235),
        ],
        heat: [
            Color::Rgb(69, 71, 90),
            Color::Rgb(58, 98, 76),
            Color::Rgb(94, 140, 92),
            Color::Rgb(166, 227, 161),
            Color::Rgb(220, 245, 200),
        ],
    },
];

static THEME_IDX: AtomicUsize = AtomicUsize::new(0);

pub fn index() -> usize {
    THEME_IDX.load(Ordering::Relaxed)
}

pub fn get() -> &'static Theme {
    &THEMES[index() % THEMES.len()]
}

/// 循环到下一套主题,返回新主题名。
pub fn cycle() -> &'static str {
    let next = (index() + 1) % THEMES.len();
    THEME_IDX.store(next, Ordering::Relaxed);
    THEMES[next].name
}

pub fn set_by_name(name: &str) -> bool {
    if let Some(i) = THEMES
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(name))
    {
        THEME_IDX.store(i, Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// 把两个 RGB 颜色按 t 混合(0 = a,1 = b);非 RGB 颜色直接返回 b。
pub fn blend(a: Color, b: Color, t: f64) -> Color {
    let (Color::Rgb(ra, ga, ba), Color::Rgb(rb, gb, bb)) = (a, b) else {
        return b;
    };
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f64 * (1.0 - t) + y as f64 * t).round() as u8;
    Color::Rgb(mix(ra, rb), mix(ga, gb), mix(ba, bb))
}

const FILE: &str = "theme";

pub fn load(home: &std::path::Path) {
    let path = home.join(".quotakeeper").join(FILE);
    if let Ok(name) = std::fs::read_to_string(path) {
        set_by_name(name.trim());
    }
}

pub fn save(home: &std::path::Path) {
    let dir = home.join(".quotakeeper");
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(dir.join(FILE), get().name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_all_themes() {
        let start = index();
        for i in 1..=THEMES.len() {
            cycle();
            assert_eq!(index(), (start + i) % THEMES.len());
        }
        // 回到起点
        assert_eq!(index(), start);
    }

    #[test]
    fn set_by_name_is_case_insensitive_and_rejects_unknown() {
        assert!(set_by_name("tokyo night"));
        assert_eq!(get().name, "Tokyo Night");
        assert!(!set_by_name("不存在的主题"));
        set_by_name("GitHub Light");
        assert_eq!(index(), 0);
    }

    #[test]
    fn blend_mixes_rgb_and_falls_back() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(100, 200, 100);
        assert_eq!(blend(a, b, 0.5), Color::Rgb(50, 100, 50));
        assert_eq!(blend(Color::Green, b, 0.9), b); // 非 RGB 回退
    }

    #[test]
    fn heat_levels_are_five() {
        for t in &THEMES {
            assert_eq!(t.heat.len(), 5);
        }
    }
}
