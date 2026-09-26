//! 壳:顶栏 + 响应式侧栏 + 内容区 + 底栏.
//!
//! 交互:鼠标点击侧栏切换页面、点击图表/热力格切换明细;
//! 键盘 ↑↓/Tab/1·2·3 切页,? 显示帮助.
//! 命中区由渲染时一并填充,主循环用同一份数据做点击判定.
//! 颜色一律取自 theme(全局,`p` 循环).

pub mod heatmap;
pub mod i18n;
pub mod notify;
pub mod quota;
pub mod theme;
pub mod trend;
pub mod usage;

pub use theme::THEMES;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::ledger::LedgerReport;
use crate::model::Snapshot;

use theme as t;

fn accent() -> Style {
    Style::default().fg(t::get().accent)
}

fn dim() -> Style {
    Style::default().fg(t::get().dim)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Quota,
    Usage,
    Heatmap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendRange {
    H24,
    D14,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendMetric {
    Tokens,
    Cost,
}

/// 渲染时填充的点击命中区.主循环在 `Event::Mouse(Up(Left))` 时查这些矩形.
#[derive(Default)]
pub struct HitRegions {
    pub sidebar: Vec<(Rect, Page)>,
    /// usage 页"按 Agent"表行 → by_agent 下标
    pub agent_rows: Vec<(Rect, usize)>,
    /// usage 页"每日"表行 → by_day 下标
    pub day_rows: Vec<(Rect, usize)>,
    /// 热力图格子 → 日期 "YYYY-MM-DD"
    pub heat_cells: Vec<(Rect, String)>,
    /// usage 页 每日表/柱状图 切换按钮
    pub chart_toggle: Option<Rect>,
    pub trend_ranges: Vec<(Rect, TrendRange)>,
    pub trend_metrics: Vec<(Rect, TrendMetric)>,
    pub trend_buckets: Vec<(Rect, usize)>,
    /// usage 页维度标签 → 维度下标(0 Agent / 1 会话 / 2 项目)
    pub dim_tabs: Vec<(Rect, usize)>,
}

impl HitRegions {
    fn clear(&mut self) {
        self.sidebar.clear();
        self.agent_rows.clear();
        self.day_rows.clear();
        self.heat_cells.clear();
        self.chart_toggle = None;
        self.trend_ranges.clear();
        self.trend_metrics.clear();
        self.trend_buckets.clear();
        self.dim_tabs.clear();
    }

    pub fn hit(&self, column: u16, row: u16) -> Option<Hit> {
        let pos = Position::new(column, row);
        if let Some(rect) = &self.chart_toggle
            && rect.contains(pos)
        {
            return Some(Hit::ToggleChart);
        }
        for (rect, range) in &self.trend_ranges {
            if rect.contains(pos) {
                return Some(Hit::TrendRange(*range));
            }
        }
        for (rect, metric) in &self.trend_metrics {
            if rect.contains(pos) {
                return Some(Hit::TrendMetric(*metric));
            }
        }
        for (rect, idx) in &self.trend_buckets {
            if rect.contains(pos) {
                return Some(Hit::TrendBucket(*idx));
            }
        }
        for (rect, page) in &self.sidebar {
            if rect.contains(pos) {
                return Some(Hit::Sidebar(*page));
            }
        }
        for (rect, idx) in &self.agent_rows {
            if rect.contains(pos) {
                return Some(Hit::AgentRow(*idx));
            }
        }
        for (rect, idx) in &self.day_rows {
            if rect.contains(pos) {
                return Some(Hit::DayRow(*idx));
            }
        }
        for (rect, date) in &self.heat_cells {
            if rect.contains(pos) {
                return Some(Hit::HeatCell(date.clone()));
            }
        }
        for (rect, idx) in &self.dim_tabs {
            if rect.contains(pos) {
                return Some(Hit::DimTab(*idx));
            }
        }
        None
    }
}

#[derive(Debug)]
pub enum Hit {
    Sidebar(Page),
    ToggleChart,
    TrendRange(TrendRange),
    TrendMetric(TrendMetric),
    TrendBucket(usize),
    DimTab(usize),
    AgentRow(usize),
    DayRow(usize),
    HeatCell(String),
}

pub struct ViewState<'a> {
    pub page: Page,
    pub snapshot: Option<&'a Snapshot>,
    pub refreshing: bool,
    pub last_error: Option<&'a str>,
    pub age_secs: Option<u64>,
    pub ledger: Option<&'a LedgerReport>,
    pub sel_agent: Option<usize>,
    pub sel_day: Option<usize>,
    pub sel_trend: Option<usize>,
    /// 热力图选中的日期
    pub sel_heat: Option<&'a str>,
    /// 用量页每日:表格(false) / 柱状图(true)
    pub chart_mode: bool,
    pub trend_range: TrendRange,
    pub trend_metric: TrendMetric,
    /// 用量页维度:0 Agent / 1 会话 / 2 项目
    pub dim: usize,
    /// 刷新动画帧计数
    pub tick: u64,
    pub show_help: bool,
}

pub fn render(frame: &mut Frame, st: &ViewState, hits: &mut HitRegions) {
    hits.clear();
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
    header(frame, root[0], st);

    let narrow = frame.area().width < 76;
    let content = if narrow {
        root[1]
    } else {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(16), Constraint::Min(0)])
            .split(root[1]);
        sidebar(frame, body[0], st.page, hits);
        body[1]
    };

    match st.page {
        Page::Quota => quota::render(frame, content, st),
        Page::Usage => usage::render(frame, content, st, hits),
        Page::Heatmap => heatmap::render(frame, content, st, hits),
    }

    footer(frame, root[2], st);
    if st.show_help {
        help_overlay(frame, st.page);
    }
}

/// 刷新中的盲文旋转动画帧.
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn header(frame: &mut Frame, area: Rect, st: &ViewState) {
    let page = match st.page {
        Page::Quota => i18n::t(i18n::K::PageQuota),
        Page::Usage => i18n::t(i18n::K::PageTokens),
        Page::Heatmap => i18n::t(i18n::K::PageHeat),
    };
    let status = match (st.refreshing, st.age_secs, st.tick) {
        (true, _, tk) => format!(
            "  {} {}",
            SPINNER[(tk % SPINNER.len() as u64) as usize],
            i18n::t(i18n::K::Syncing)
        ),
        (false, Some(a), _) => format!("  · {}", i18n::age(a)),
        (false, None, _) => String::new(),
    };
    let mut lines = vec![Line::from(vec![
        Span::styled("◆ ", dim()),
        Span::styled(
            i18n::t(i18n::K::Title),
            accent().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · "),
        Span::raw(page),
        Span::styled(status, dim()),
    ])];
    if let Some(e) = st.last_error {
        lines.push(Line::from(vec![Span::styled(
            format!("⚠ {e}"),
            Style::default().fg(t::get().err),
        )]));
    }

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

const SIDEBAR_ITEMS: [(i18n::K, Page); 3] = [
    (i18n::K::PageQuota, Page::Quota),
    (i18n::K::PageTokens, Page::Usage),
    (i18n::K::PageHeat, Page::Heatmap),
];

/// 粗略显示宽度:CJK(≥0x2E80)记 2,其余记 1(侧栏标签够用).
fn disp_w(s: &str) -> u16 {
    s.chars()
        .map(|c| if c as u32 >= 0x2E80 { 2 } else { 1 })
        .sum()
}

fn sidebar(frame: &mut Frame, area: Rect, active: Page, hits: &mut HitRegions) {
    let th = t::get();
    // 圆角按钮 chips:每个高 3、间隔 1;激活态换 accent,未激活暗色
    for (i, (key, page)) in SIDEBAR_ITEMS.iter().enumerate() {
        let rect = Rect {
            x: area.x,
            y: area.y + i as u16 * 4,
            width: area.width,
            height: 3,
        };
        let active = *page == active;
        let label = i18n::t(*key);
        let (border_style, label_style) = if active {
            (
                Style::default().fg(th.accent),
                Style::default().fg(th.accent).add_modifier(Modifier::BOLD),
            )
        } else {
            (Style::default().fg(th.border), dim())
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let lw = disp_w(label);
        let lx = inner.x + (inner.width.saturating_sub(lw)) / 2;
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label, label_style))),
            Rect {
                x: lx,
                y: inner.y,
                width: lw.min(inner.width),
                height: 1,
            },
        );
        hits.sidebar.push((rect, *page));
    }
}

fn footer(frame: &mut Frame, area: Rect, st: &ViewState) {
    let hint = match (i18n::lang(), st.page) {
        (i18n::Lang::Zh, Page::Usage) => "↑↓ 页 · g 图 · t 时间 · c 金额 · ? 帮助",
        (i18n::Lang::Zh, _) => "↑↓ 页 · r 刷新 · ? 帮助 · q 退出",
        (i18n::Lang::En, Page::Usage) => "↑↓ page · g chart · t range · c cost · ? help",
        (i18n::Lang::En, _) => "↑↓ page · r refresh · ? help · q quit",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(hint, dim()),
            Span::styled(
                format!("   · {}", t::get().name),
                Style::default().fg(t::get().border),
            ),
        ])),
        area,
    );
}

fn help_overlay(frame: &mut Frame, page: Page) {
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(58);
    let height = if page == Page::Usage { 9 } else { 7 };
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height: height.min(area.height),
    };
    let zh = i18n::lang() == i18n::Lang::Zh;
    let mut lines = if zh {
        vec![
            "1/2/3  配额 / 用量 / 热力图",
            "↑/k  上一页    ↓/j/Tab  下一页",
            "r  刷新    p  主题    l  语言",
            "Esc  关闭详情    q  退出",
        ]
    } else {
        vec![
            "1/2/3  Quota / Usage / Heatmap",
            "↑/k  previous    ↓/j/Tab  next",
            "r  refresh    p  theme    l  language",
            "Esc  close detail    q  quit",
        ]
    };
    if page == Page::Usage {
        lines.push(if zh {
            "g  图表    t  时间    c  金额    s  分类"
        } else {
            "g  chart    t  range    c  cost    s  group"
        });
    }
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines.join("\n")).block(
            Block::default()
                .title(if zh {
                    " 快捷键 · ? 关闭 "
                } else {
                    " Keys · ? to close "
                })
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(accent()),
        ),
        rect,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn view<'a>(page: Page) -> ViewState<'a> {
        ViewState {
            page,
            snapshot: None,
            refreshing: false,
            last_error: None,
            age_secs: None,
            ledger: None,
            sel_agent: None,
            sel_day: None,
            sel_trend: None,
            sel_heat: None,
            chart_mode: false,
            trend_range: TrendRange::H24,
            trend_metric: TrendMetric::Tokens,
            dim: 0,
            tick: 0,
            show_help: false,
        }
    }

    #[test]
    fn sidebar_hit_regions() {
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let st = view(Page::Quota);
        term.draw(|f| render(f, &st, &mut hits)).unwrap();
        assert_eq!(hits.sidebar.len(), 3);
        // chips:每个高 3、间隔 1,内区从 x=1 起
        assert!(matches!(hits.hit(1, 4), Some(Hit::Sidebar(Page::Quota))));
        assert!(matches!(hits.hit(1, 8), Some(Hit::Sidebar(Page::Usage))));
        assert!(matches!(hits.hit(1, 12), Some(Hit::Sidebar(Page::Heatmap))));
        let mut hits2 = HitRegions::default();
        term.draw(|f| render(f, &view(Page::Usage), &mut hits2))
            .unwrap();
    }

    #[test]
    fn footer_shows_theme_name() {
        let backend = TestBackend::new(100, 24);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        term.draw(|f| render(f, &view(Page::Quota), &mut hits))
            .unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect::<String>()
            .split_whitespace()
            .collect();
        assert!(
            text.contains(t::get().name.split(' ').next().unwrap()),
            "底栏缺主题名"
        );
    }
}
