//! 壳:顶栏 + 响应式侧栏 + 内容区 + 底栏.
//!
//! 交互:鼠标点击侧栏切换页面、点击图表/热力格切换明细;
//! 键盘 ↑↓/Tab/1·2·3·4 切页,总览页 ←→ 切周期,? 显示帮助.
//! 命中区由渲染时一并填充,主循环用同一份数据做点击判定.
//! 颜色一律取自 theme(全局,`p` 循环).

pub mod heatmap;
pub mod i18n;
pub mod notify;
pub mod overview;
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
    Overview,
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
    /// usage 页维度标签 → 维度下标(0 Agent / 1 会话 / 2 项目 / 3 模型)
    pub dim_tabs: Vec<(Rect, usize)>,
    /// 总览页周期 tabs → 周期
    pub period_tabs: Vec<(Rect, crate::ui::overview::Period)>,
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
        self.period_tabs.clear();
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
        for (rect, period) in &self.period_tabs {
            if rect.contains(pos) {
                return Some(Hit::PeriodTab(*period));
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
    PeriodTab(crate::ui::overview::Period),
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
    /// 总览页周期
    pub period: crate::ui::overview::Period,
    pub sel_agent: Option<usize>,
    pub sel_day: Option<usize>,
    pub sel_trend: Option<usize>,
    /// 热力图选中的日期
    pub sel_heat: Option<&'a str>,
    /// 用量页每日:表格(false) / 柱状图(true)
    pub chart_mode: bool,
    pub trend_range: TrendRange,
    pub trend_metric: TrendMetric,
    /// 用量页维度:0 Agent / 1 会话 / 2 项目 / 3 模型
    pub dim: usize,
    /// 模型列表当前滚动位置.
    pub model_offset: usize,
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

    // Usage needs 75 columns (68 column constraints + 7 separators), plus 16 for the sidebar.
    let narrow = frame.area().width < 91;
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
        Page::Overview => overview::render(frame, content, st, hits),
        Page::Quota => quota::render(frame, content, st),
        Page::Usage => usage::render(frame, content, st, hits),
        Page::Heatmap => heatmap::render(frame, content, st, hits),
    }

    footer(frame, root[2], st);
    if st.show_help {
        help_overlay(frame, st.page, st.page == Page::Usage && st.dim == 3);
    }
}

/// 刷新中的盲文旋转动画帧.
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn header(frame: &mut Frame, area: Rect, st: &ViewState) {
    let page = match st.page {
        Page::Overview => i18n::t(i18n::K::PageOverview),
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

const SIDEBAR_ITEMS: [(i18n::K, Page); 4] = [
    (i18n::K::PageOverview, Page::Overview),
    (i18n::K::PageQuota, Page::Quota),
    (i18n::K::PageTokens, Page::Usage),
    (i18n::K::PageHeat, Page::Heatmap),
];

/// 粗略显示宽度:CJK(≥0x2E80)记 2,其余记 1(侧栏标签够用).
pub(crate) fn disp_w(s: &str) -> u16 {
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
    let hint = match (i18n::lang(), st.page, st.dim == 3) {
        (i18n::Lang::Zh, Page::Usage, true) => "↑↓ 模型 · Tab 页 · g 图 · t 时间 · c 金额 · ? 帮助",
        (i18n::Lang::Zh, Page::Usage, false) => "↑↓ 页 · g 图 · t 时间 · c 金额 · ? 帮助",
        (i18n::Lang::Zh, Page::Overview, _) => "←→ 周期 · r 刷新 · ? 帮助 · q 退出",
        (i18n::Lang::Zh, _, _) => "↑↓ 页 · r 刷新 · ? 帮助 · q 退出",
        (i18n::Lang::En, Page::Usage, true) => {
            "↑↓ models · Tab page · g chart · t range · c cost · ? help"
        }
        (i18n::Lang::En, Page::Usage, false) => "↑↓ page · g chart · t range · c cost · ? help",
        (i18n::Lang::En, _, _) => "↑↓ page · r refresh · ? help · q quit",
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

fn help_overlay(frame: &mut Frame, page: Page, model_list_active: bool) {
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
            "1/2/3/4  总览 / 配额 / 用量 / 热力图 · 总览页 ←→ 切周期",
            if model_list_active {
                "↑/k  上一模型    ↓/j  下一模型    Tab  下一页"
            } else {
                "↑/k  上一页    ↓/j/Tab  下一页"
            },
            "r  刷新    p  主题    l  语言",
            "Esc  关闭详情    q  退出",
        ]
    } else {
        vec![
            "1/2/3/4  Overview / Quota / Usage / Heatmap · ←→ period",
            if model_list_active {
                "↑/k  previous model    ↓/j  next model    Tab  next page"
            } else {
                "↑/k  previous    ↓/j/Tab  next"
            },
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
            period: crate::ui::overview::Period::D30,
            sel_agent: None,
            sel_day: None,
            sel_trend: None,
            sel_heat: None,
            chart_mode: false,
            trend_range: TrendRange::H24,
            trend_metric: TrendMetric::Tokens,
            dim: 0,
            model_offset: 0,
            tick: 0,
            show_help: false,
        }
    }

    #[test]
    fn sidebar_hit_regions() {
        let backend = TestBackend::new(100, 24);
        let mut term = Terminal::new(backend).unwrap();
        let mut hits = HitRegions::default();
        let st = view(Page::Quota);
        term.draw(|f| render(f, &st, &mut hits)).unwrap();
        assert_eq!(hits.sidebar.len(), 4);
        // chips:每个高 3、间隔 1,内区从 x=1 起
        assert!(matches!(hits.hit(1, 4), Some(Hit::Sidebar(Page::Overview))));
        assert!(matches!(hits.hit(1, 8), Some(Hit::Sidebar(Page::Quota))));
        assert!(matches!(hits.hit(1, 12), Some(Hit::Sidebar(Page::Usage))));
        assert!(matches!(hits.hit(1, 16), Some(Hit::Sidebar(Page::Heatmap))));
        let mut hits2 = HitRegions::default();
        term.draw(|f| render(f, &view(Page::Usage), &mut hits2))
            .unwrap();
    }

    #[test]
    fn sidebar_breakpoint_preserves_minimum_agent_table_width() {
        let mut term_90 = Terminal::new(TestBackend::new(90, 24)).unwrap();
        let mut hits_90 = HitRegions::default();
        term_90
            .draw(|f| render(f, &view(Page::Usage), &mut hits_90))
            .unwrap();
        assert!(hits_90.sidebar.is_empty());

        let mut quota_80 = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut quota_hits_80 = HitRegions::default();
        quota_80
            .draw(|f| render(f, &view(Page::Quota), &mut quota_hits_80))
            .unwrap();
        assert!(quota_hits_80.sidebar.is_empty());

        let mut term_91 = Terminal::new(TestBackend::new(91, 24)).unwrap();
        let mut hits_91 = HitRegions::default();
        term_91
            .draw(|f| render(f, &view(Page::Usage), &mut hits_91))
            .unwrap();
        assert_eq!(hits_91.sidebar.len(), 4);
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
            t::THEMES
                .iter()
                .any(|theme| text.contains(theme.name.split(' ').next().unwrap())),
            "底栏缺主题名"
        );
    }
}
