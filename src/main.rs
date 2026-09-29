//! 入口:事件循环 + 后台刷新线程(经 mpsc 回传),UI 永不阻塞.
//!
//! 布局为壳 + 三页(配额/Token 用量/热力图);宽窗口侧栏可点,窄窗口自动收起.
//! 点击图表/热力格查看明细;键盘 ↑↓/Tab/1·2·3/?/r/q·Esc 等操作.
//! 配额与账本独立刷新;远程配额慢时本地用量先显示.

use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_token_monitor::{backend, ledger, ui};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use tui_token_monitor::model::Snapshot;
use ui::{HitRegions, Page, TrendMetric, TrendRange};

enum Msg {
    Quota(Result<(Snapshot, u64), String>),
    Ledger(Box<ledger::LedgerReport>),
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn spawn_refresh(tx: mpsc::Sender<Msg>) {
    let quota_tx = tx.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(backend::fetch_snapshot)
            .unwrap_or_else(|_| Err(anyhow::anyhow!("配额采集线程 panic")));
        let quota = match result {
            Ok(snap) => Ok((snap, now_ms() as u64)),
            Err(e) => Err(e.to_string()),
        };
        let _ = quota_tx.send(Msg::Quota(quota));
    });
    std::thread::spawn(move || {
        let rep = ledger::collect(chrono::Local::now());
        let _ = tx.send(Msg::Ledger(Box::new(rep)));
    });
}

fn show_alerts(alerts: ui::notify::AlertBatch, now_ms: i64) {
    for alert in alerts.quota {
        ui::notify::toast(
            &alert.title,
            &format!("{:.0}% {}", alert.used_pct, alert.event.message()),
        );
    }
    for alert in alerts.expiry {
        ui::notify::toast(
            &ui::notify::expiry_title(&alert.provider),
            &ui::notify::expiry_message(alert.ends_at_ms, now_ms),
        );
    }
    let zh = ui::i18n::lang() == ui::i18n::Lang::Zh;
    for alert in alerts.pace {
        let minutes = alert.exhaust_in_ms / 60_000;
        let body = if zh {
            format!("比匀速快 {:.0} 个百分点;预计 {minutes} 分钟后耗尽", alert.delta_points)
        } else {
            format!("{:.0} points ahead of pace; may run out in {minutes} min", alert.delta_points)
        };
        ui::notify::toast(&alert.title, &body);
    }
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
    let mut pending_refreshes = 2_u8;
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
    let mut alerts = ui::notify::AlertTracker::default();
    let mut ledger_report: Option<ledger::LedgerReport> = None;
    let mut hits = HitRegions::default();

    spawn_refresh(tx.clone());

    loop {
        // 注意:不用 next_refresh.elapsed(),未来时刻会 panic;直接比较 Instant.
        if pending_refreshes == 0 && Instant::now() >= next_refresh {
            spawn_refresh(tx.clone());
            pending_refreshes = 2;
            refreshing = true;
        }
        match rx.try_recv() {
            Ok(Msg::Quota(quota)) => {
                match quota {
                    Ok((snap, at)) => {
                        show_alerts(alerts.update(&snap, at as i64), at as i64);
                        snapshot = Some(snap);
                        fetched_at_ms = Some(at);
                        last_error = None;
                    }
                    Err(e) => {
                        last_error = Some(e);
                    }
                }
                pending_refreshes = pending_refreshes.saturating_sub(1);
            }
            Ok(Msg::Ledger(rep)) => {
                ledger_report = Some(*rep);
                pending_refreshes = pending_refreshes.saturating_sub(1);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
        if refreshing && pending_refreshes == 0 {
            refreshing = false;
            next_refresh = Instant::now() + Duration::from_secs(60);
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
                    KeyCode::Char('r') if pending_refreshes == 0 => {
                        spawn_refresh(tx.clone());
                        pending_refreshes = 2;
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
