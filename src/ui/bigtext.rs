//! 点阵大数字:3×6 子像素位图的数字字体(0-9 与 %),用 ▀▄█ 半块渲染成
//! 3 行文本(每行覆盖 2 个子像素行)。用于圆环环心的大号百分比。
//!
//! 位图约定:每行 3 位,MSB = 左列。第 6 行为空(字距)。

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

const GLYPH_H: usize = 6;
const GLYPH_W: usize = 3;

const FONT: [(char, [u8; GLYPH_H]); 11] = [
    ('0', [0b111, 0b101, 0b101, 0b101, 0b111, 0b000]),
    ('1', [0b010, 0b110, 0b010, 0b010, 0b111, 0b000]),
    ('2', [0b111, 0b001, 0b111, 0b100, 0b111, 0b000]),
    ('3', [0b111, 0b001, 0b111, 0b001, 0b111, 0b000]),
    ('4', [0b101, 0b101, 0b111, 0b001, 0b001, 0b000]),
    ('5', [0b111, 0b100, 0b111, 0b001, 0b111, 0b000]),
    ('6', [0b111, 0b100, 0b111, 0b101, 0b111, 0b000]),
    ('7', [0b111, 0b001, 0b010, 0b010, 0b010, 0b000]),
    ('8', [0b111, 0b101, 0b111, 0b101, 0b111, 0b000]),
    ('9', [0b111, 0b101, 0b111, 0b001, 0b111, 0b000]),
    ('%', [0b101, 0b001, 0b010, 0b100, 0b101, 0b000]),
];

fn glyph(c: char) -> Option<&'static [u8; GLYPH_H]> {
    FONT.iter().find(|(gc, _)| *gc == c).map(|(_, rows)| rows)
}

/// 渲染结果:字符网格(cols × rows),元素为 '█' / '▀' / '▄' / ' '。
pub struct BigGrid {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<Vec<char>>,
}

/// 把纯数字+百分比字符串转成 3 行高的半块网格;含非法字符返回 None。
/// 文本总宽 = 字符数 × 3 + (字符数-1) 字距。
pub fn grid(s: &str) -> Option<BigGrid> {
    let glyphs: Vec<&[u8; GLYPH_H]> = s
        .chars()
        .map(|c| if c == '%' { glyph('%') } else { glyph(c) })
        .collect::<Option<Vec<_>>>()?;
    let n = glyphs.len();
    if n == 0 {
        return None;
    }
    let cols = (n * (GLYPH_W + 1)) as u16 - 1; // 字符间距 1 列
    let rows = (GLYPH_H / 2) as u16; // 6 半行 = 3 文本行
    // 每格 2 位:bit0 = 上半,bit1 = 下半
    let mut cells = vec![vec![0u8; cols as usize]; rows as usize];
    for (gi, g) in glyphs.iter().enumerate() {
        let gx = gi * (GLYPH_W + 1);
        for sub_row in 0..GLYPH_H {
            for col in 0..GLYPH_W {
                let on = g[sub_row] & (0b100 >> col) != 0;
                if !on {
                    continue;
                }
                let text_row = sub_row / 2;
                let mask = if sub_row % 2 == 0 { 1u8 } else { 2u8 };
                cells[text_row][gx + col] |= mask;
            }
        }
    }
    let cells = cells
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|m| match m {
                    1 => '▀',
                    2 => '▄',
                    3 => '█',
                    _ => ' ',
                })
                .collect::<Vec<char>>()
        })
        .collect();
    Some(BigGrid { cols, rows, cells })
}

/// 把网格写入 buffer 的 (x, y) 起点;返回是否完整放下。
pub fn render(grid: &BigGrid, area: Rect, buf: &mut Buffer, style: Style) -> bool {
    let (gw, gh) = (grid.cols, grid.rows);
    if area.width < gw || area.height < gh {
        return false;
    }
    let x0 = area.x + (area.width - gw) / 2;
    let y0 = area.y + (area.height - gh) / 2;
    for (r, row) in grid.cells.iter().enumerate() {
        for (c, ch) in row.iter().enumerate() {
            if *ch != ' ' {
                buf[(x0 + c as u16, y0 + r as u16)]
                    .set_char(*ch)
                    .set_style(style);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, style::Color};

    #[test]
    fn grid_dimensions() {
        let g = grid("57%").unwrap();
        // 3 字符 × 3 列 + 2 间距 = 11
        assert_eq!(g.cols, 11);
        assert_eq!(g.rows, 3);
        assert!(grid("x").is_none());
    }

    #[test]
    fn half_blocks_compose() {
        // '1' = [010,110,010,010,111,000]
        let g = grid("1").unwrap();
        // 行 0(上 010)→ 中列 '▀';行 1(下 110)→ 左中 '▄' → 中列上下叠加 '█'
        assert_eq!(g.cells[0][1], '█');
        assert_eq!(g.cells[0][0], '▄');
        // 行 2+3(010/010)→ 中列 '█'
        assert_eq!(g.cells[1][1], '█');
        // 行 4(111)→ 三列 '▀'(只有上半)
        assert_eq!(g.cells[2][0], '▀');
        assert_eq!(g.cells[2][2], '▀');
        // 空行 5 → 空格
        assert!(g.cells.iter().all(|r| r.len() == 3));
    }

    #[test]
    fn renders_into_buffer() {
        let backend = TestBackend::new(20, 8);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let g = grid("100").unwrap();
            render(
                &g,
                f.area(),
                f.buffer_mut(),
                Style::default().fg(Color::Cyan),
            );
        })
        .unwrap();
        let b = term.backend().buffer();
        // 居中:x0 = (20-11)/2 = 4, y0 = (8-3)/2 = 2
        // '1' 的底行(111→▀×3)在 (4..6, 4);'0' 的左列竖线在 (8, y)
        assert_eq!(b[(4, 4)].symbol(), "▀");
        assert_eq!(b[(5, 4)].symbol(), "▀");
        assert_eq!(b[(6, 4)].symbol(), "▀");
        assert_eq!(b[(8, 3)].symbol(), "█"); // '0' 左上角(111 行 0-1)
    }

    #[test]
    fn too_small_area_returns_false() {
        let backend = TestBackend::new(5, 8);
        let mut term = Terminal::new(backend).unwrap();
        let mut ok_flag = true;
        term.draw(|f| {
            let g = grid("57%").unwrap();
            ok_flag = render(&g, f.area(), f.buffer_mut(), Style::default());
        })
        .unwrap();
        assert!(!ok_flag);
    }
}
