//! 入口:事件循环 + 后台刷新线程(经 mpsc 回传),UI 永不阻塞.
//!
//! 布局为壳 + 三页(配额/Token 用量/热力图);宽窗口侧栏可点,窄窗口自动收起.
//! 点击图表/热力格查看明细;键盘 ↑↓/Tab/1·2·3/?/r/q·Esc 等操作.
//! 刷新线程一次带回 quota(可失败)与 ledger(永不完全失败,单源错误在 UI 展示),
//! 因此 OMP 挂了 Token 页照常可用.

use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_token_monitor::{backend, ledger, model, pace, ui};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use model::Snapshot;
use ui::{HitRegions, Page, TrendMetric, TrendRange};

enum Msg {
    Refreshed(Result<(Snapshot, u64), String>, Box<ledger::LedgerReport>),
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn spawn_refresh(tx: mpsc::Sender<Msg>) {
    std::thread::spawn(move || {
        // 四家额度独立刷新；各卡的兼容兜底由 backend 管理。
        let t = std::thread::spawn(backend::fetch_snapshot);
        let quota = match t
            .join()
            .unwrap_or_else(|_| Err(anyhow::anyhow!("配额采集线程 panic")))
        {
            Ok(snap) => Ok((snap, now_ms() as u64)),
            Err(e) => Err(e.to_string()),
        };
        // ledger 采集与 quota 无关:本地日志扫描 + SQLite 直查 + 可选估价
        let rep = ledger::collect(chrono::Local::now());
        let _ = tx.send(Msg::Refreshed(quota, Box::new(rep)));
    });
}

/// 配额阈值通知:等级跃迁才发 toast(避免每 60s 重复轰炸)。
/// 订阅到期提醒:档位只升不降,上升时 toast;续费回落静默更新状态.
/// 预测提醒(与 GUI pace 同逻辑):比匀速快且预计耗尽 → 首次触发 toast.
fn notify_pace(snap: &Snapshot, fetched_at_ms: u64, risk: &mut std::collections::HashMap<String, bool>) {
    let zh = ui::i18n::lang() == ui::i18n::Lang::Zh;
    let mut next: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for card in &snap.cards {
        let model::Card::Live(r) = card else { continue };
        if r.source == model::QuotaSource::CpaLocal {
            continue;
        }
        let provider = r.provider.title().split(" ·").next().unwrap_or("").to_string();
        for l in &r.limits {
            let key = format!("{provider} · {}", l.label);
            let forecast = pace::forecast(l, fetched_at_ms as i64);
            let at_risk = forecast.is_some_and(|v| v.exhaust_in_ms.is_some());
            if at_risk
                && risk.get(&key) != Some(&true)
                && let Some(v) = forecast
            {
                let mins = v.exhaust_in_ms.unwrap_or(0) / 60_000;
                let title = format!("{key}");
                let body = if zh {
                    format!("比匀速快 {:.0} 个百分点;预计 {} 分钟后耗尽", v.delta_points, mins)
                } else {
                    format!(
                        "{:.0} points ahead of pace; may run out in {} min",
                        v.delta_points, mins
                    )
                };
                ui::notify::toast(&title, &body);
            }
            next.insert(key, at_risk);
        }
    }
    *risk = next;
}

fn notify_expiry(snap: &Snapshot, prev: &mut Vec<(String, u8)>) {
    let now = now_ms();
    let mut next = Vec::with_capacity(prev.len());
    for card in &snap.cards {
        let model::Card::Live(r) = card else { continue };
        let Some(ends) = r.subscription_ends_at_ms else { continue };
        let provider = r.provider.title().split(" ·").next().unwrap_or("subscription");
        let level = ui::notify::expiry_level(Some(ends), now);
        let before = prev
            .iter()
            .find(|(k, _)| *k == provider)
            .map(|(_, v)| *v)
            .unwrap_or(0);
        if level > before {
            ui::notify::toast(
                &ui::notify::expiry_title(provider),
                &ui::notify::expiry_message(ends, now),
            );
        }
        next.push((provider.to_string(), level));
    }
    *prev = next;
}

fn notify_quota(snap: &Snapshot, prev: &mut Vec<(String, u8)>) {
    let mut next = Vec::with_capacity(prev.len());
    for card in &snap.cards {
        let model::Card::Live(r) = card else { continue };
        if r.source == model::QuotaSource::CpaLocal {
            continue;
        }
        // toast 标题用短名("OpenCode Go · $10/mo" → "OpenCode Go")
        let provider = r.provider.title().split(" ·").next().unwrap_or("usage");
        for l in &r.limits {
            let key = format!("{provider}·{}", l.label);
            let now = ui::notify::level_for(l.used_pct);
            let before = prev
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| *v)
                .unwrap_or(0);
            if let Some(ev) = ui::notify::transition(before, now) {
                ui::notify::toast(
                    &format!("{provider} · {}", l.label),
                    &format!("{:.0}% {}", l.used_pct, ev.message()),
                );
            }
            next.push((key, now));
        }
    }
    *prev = next;
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let res = run(&mut terminal);

    // 无论如何恢复终端(错误路径也不能留下 raw mode/鼠标捕获)
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    res
}

fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel::<Msg>();
    // 主题:启动时恢复上次选择
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    ui::theme::load(&home);
    ui::i18n::load(&home);

    let mut snapshot: Option<Snapshot> = None;
    let mut fetched_at_ms: Option<u64> = None;
    let mut last_error: Option<String> = None;
    let mut refreshing = true;
    let mut in_flight = true;
    let mut next_refresh = Instant::now();
    let mut tick: u64 = 0;

    let mut page = Page::Overview;
    // 总览页周期(←→ 或点击 tabs 切换)
    let mut period = ui::overview::Period::D30;
    let mut show_help = false;
    let mut sel_agent: Option<usize> = None;
    let mut sel_day: Option<usize> = None;
    let mut sel_trend: Option<usize> = None;
    let mut sel_heat: Option<String> = None;
    let mut chart_mode = true;
    let mut trend_range = TrendRange::H24;
    let mut trend_metric = TrendMetric::Tokens;
    let mut dim: usize = 0; // 用量页维度:0 Agent / 1 会话 / 2 项目 / 3 模型
    let mut model_offset: usize = 0;
    // 配额阈值通知状态机:每 (卡, 额度) 记上一轮等级,跃迁才发 toast
    let mut prev_levels: Vec<(String, u8)> = Vec::new();
    // 订阅到期通知状态机:每卡记上一轮档位,只升不降(续费回落静默)
    let mut prev_expiry: Vec<(String, u8)> = Vec::new();
    // 预测提醒:每 (卡, 窗口) 记风险位,进入风险首次 toast,回落后再入才再发
    let mut pace_risk: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    let mut ledger_report: Option<ledger::LedgerReport> = None;
    let mut hits = HitRegions::default();

    spawn_refresh(tx.clone());

    loop {
        // 注意:不用 next_refresh.elapsed(),未来时刻会 panic;直接比较 Instant.
        if !in_flight && Instant::now() >= next_refresh {
            spawn_refresh(tx.clone());
            in_flight = true;
            refreshing = true;
        }
        match rx.try_recv() {
            Ok(Msg::Refreshed(quota, rep)) => {
                match quota {
                    Ok((snap, at)) => {
                        notify_quota(&snap, &mut prev_levels);
                        notify_expiry(&snap, &mut prev_expiry);
                        notify_pace(&snap, at, &mut pace_risk);
                        snapshot = Some(snap);
                        fetched_at_ms = Some(at);
                        last_error = None;
                    }
                    Err(e) => {
                        last_error = Some(e);
                    }
                }
                ledger_report = Some(*rep);
                refreshing = false;
                in_flight = false;
                next_refresh = Instant::now() + Duration::from_secs(60);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }

        let age_secs = fetched_at_ms.map(|at| {
            (now_ms().max(0) as u64)
                .saturating_sub(at)
                .saturating_div(1000)
        });
        let view = ui::ViewState {
            page,
            snapshot: snapshot.as_ref(),
            refreshing,
            last_error: last_error.as_deref(),
            age_secs,
            ledger: ledger_report.as_ref(),
            period,
            sel_agent,
            sel_day,
            sel_trend,
            sel_heat: sel_heat.as_deref(),
            chart_mode,
            trend_range,
            trend_metric,
            dim,
            model_offset,
            tick,
            show_help,
        };
        terminal.draw(|f| ui::render(f, &view, &mut hits))?;
        if refreshing {
            tick += 1; // 刷新动画帧
        }

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press && show_help => {
                    if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                        show_help = false;
                    } else if key.code == KeyCode::Char('q') {
                        break;
                    }
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('?') => show_help = true,
                    KeyCode::Esc => {
                        sel_agent = None;
                        sel_day = None;
                        sel_trend = None;
                        sel_heat = None;
                    }
                    KeyCode::Char('r') if !in_flight => {
                        spawn_refresh(tx.clone());
                        in_flight = true;
                        refreshing = true;
                    }
                    KeyCode::Char('g') if page == Page::Usage => {
                        chart_mode = !chart_mode;
                        sel_day = None;
                        sel_trend = None;
                    }
                    KeyCode::Char('t') if page == Page::Usage => {
                        trend_range = match trend_range {
                            TrendRange::H24 => TrendRange::D14,
                            TrendRange::D14 => TrendRange::H24,
                        };
                        sel_trend = None;
                    }
                    KeyCode::Char('c') if page == Page::Usage => {
                        trend_metric = match trend_metric {
                            TrendMetric::Tokens => TrendMetric::Cost,
                            TrendMetric::Cost => TrendMetric::Tokens,
                        };
                        sel_trend = None;
                    }
                    KeyCode::Char('p') => {
                        ui::theme::cycle();
                        ui::theme::save(&home);
                    }
                    KeyCode::Char('l') => {
                        ui::i18n::cycle();
                        ui::i18n::save(&home);
                    }
                    KeyCode::Char('s') if page == Page::Usage => {
                        dim = (dim + 1) % 4;
                        model_offset = 0;
                        sel_agent = None;
                    }
                    KeyCode::Down | KeyCode::Char('j') if page == Page::Usage && dim == 3 => {
                        let model_count = ledger_report
                            .as_ref()
                            .map_or(0, |ledger| ledger.by_model.len());
                        model_offset = model_offset
                            .saturating_add(1)
                            .min(model_count.saturating_sub(1));
                    }
                    KeyCode::Up | KeyCode::Char('k') if page == Page::Usage && dim == 3 => {
                        model_offset = model_offset.saturating_sub(1);
                    }
                    KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
                        page = match page {
                            Page::Overview => Page::Quota,
                            Page::Quota => Page::Usage,
                            Page::Usage => Page::Heatmap,
                            Page::Heatmap => Page::Overview,
                        };
                        sel_agent = None;
                        sel_day = None;
                        sel_trend = None;
                        sel_heat = None;
                    }
                    KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
                        page = match page {
                            Page::Overview => Page::Heatmap,
                            Page::Quota => Page::Overview,
                            Page::Usage => Page::Quota,
                            Page::Heatmap => Page::Usage,
                        };
                        sel_agent = None;
                        sel_day = None;
                        sel_trend = None;
                        sel_heat = None;
                    }
                    KeyCode::Left if page == Page::Overview => {
                        period = period.prev();
                    }
                    KeyCode::Right if page == Page::Overview => {
                        period = period.next();
                    }
                    KeyCode::Char('1') => {
                        page = Page::Overview;
                        sel_agent = None;
                        sel_day = None;
                        sel_trend = None;
                        sel_heat = None;
                    }
                    KeyCode::Char('2') => {
                        page = Page::Quota;
                        sel_agent = None;
                        sel_day = None;
                        sel_trend = None;
                        sel_heat = None;
                    }
                    KeyCode::Char('3') => {
                        page = Page::Usage;
                        sel_agent = None;
                        sel_day = None;
                        sel_trend = None;
                        sel_heat = None;
                    }
                    KeyCode::Char('4') => {
                        page = Page::Heatmap;
                        sel_agent = None;
                        sel_day = None;
                        sel_trend = None;
                        sel_heat = None;
                    }
                    _ => {}
                },
                Event::Mouse(m)
                    if m.kind == MouseEventKind::Up(MouseButton::Left) && !show_help =>
                {
                    match hits.hit(m.column, m.row) {
                        Some(ui::Hit::Sidebar(p)) => {
                            page = p;
                            sel_agent = None;
                            sel_day = None;
                            sel_trend = None;
                            sel_heat = None;
                        }
                        Some(ui::Hit::PeriodTab(p)) => {
                            period = p;
                        }
                        Some(ui::Hit::ToggleChart) => {
                            chart_mode = !chart_mode;
                            sel_day = None;
                            sel_trend = None;
                        }
                        Some(ui::Hit::TrendRange(range)) => {
                            trend_range = range;
                            sel_trend = None;
                        }
                        Some(ui::Hit::TrendMetric(metric)) => {
                            trend_metric = metric;
                            sel_trend = None;
                        }
                        Some(ui::Hit::TrendBucket(i)) => {
                            sel_agent = None;
                            sel_day = None;
                            sel_trend = if sel_trend == Some(i) { None } else { Some(i) };
                        }
                        Some(ui::Hit::DimTab(i)) => {
                            dim = i;
                            model_offset = 0;
                            sel_agent = None;
                        }
                        Some(ui::Hit::AgentRow(i)) => {
                            sel_day = None;
                            sel_trend = None;
                            sel_heat = None;
                            sel_agent = if sel_agent == Some(i) { None } else { Some(i) };
                        }
                        Some(ui::Hit::DayRow(i)) => {
                            sel_agent = None;
                            sel_heat = None;
                            sel_day = if sel_day == Some(i) { None } else { Some(i) };
                            sel_trend = None;
                        }
                        Some(ui::Hit::HeatCell(date)) => {
                            sel_agent = None;
                            sel_day = None;
                            sel_trend = None;
                            sel_heat = if sel_heat.as_deref() == Some(date.as_str()) {
                                None
                            } else {
                                Some(date)
                            };
                        }
                        None => {}
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
