//! 入口:事件循环 + 后台刷新线程(经 mpsc 回传),UI 永不阻塞.
//!
//! 布局为壳 + 侧栏三页(配额/Token 用量/热力图);鼠标捕获已开启,点击侧栏切换
//! 页面、配额页右上 ⟳ 切换卡片/圆环、点击圆环循环 5h→周→月、点击表格行/热力格
//! 弹出明细;键盘 ↑↓/Tab/1·2·3/v/r/q·Esc 等价操作.
//! 刷新线程一次带回 quota(可失败)与 ledger(永不完全失败,单源错误在 UI 展示),
//! 因此 OMP 挂了 Token 页照常可用.

use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_token_monitor::{backend, goat, ledger, model, ui};

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
use ui::{HitRegions, Page, QuotaView};

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
        // backend A:OMP 两卡;backend B:GOAT 原生.互不阻塞,任一成功都出数.
        let t = std::thread::spawn(backend::fetch_snapshot);
        let goat_rep = goat::fetch_report().ok();
        let quota = match t
            .join()
            .unwrap_or_else(|_| Err(anyhow::anyhow!("OMP 采集线程 panic")))
        {
            Ok(mut snap) => {
                if let Some(r) = goat_rep
                    && let Some(slot) = snap.cards.iter_mut().find(|c| {
                        matches!(
                            c,
                            model::Card::Missing {
                                provider: model::ProviderId::Goat
                            }
                        )
                    })
                {
                    *slot = model::Card::Live(r);
                }
                Ok((snap, now_ms() as u64))
            }
            Err(e) => Err(e.to_string()),
        };
        // ledger 采集与 quota 无关:本地日志扫描 + SQLite 直查 + 可选估价
        let rep = ledger::collect(chrono::Local::now());
        let _ = tx.send(Msg::Refreshed(quota, Box::new(rep)));
    });
}

/// 配额阈值通知:等级跃迁才发 toast(避免每 60s 重复轰炸)。
fn notify_quota(snap: &Snapshot, prev: &mut Vec<(String, u8)>) {
    let mut next = Vec::with_capacity(prev.len());
    for card in &snap.cards {
        let model::Card::Live(r) = card else { continue };
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

    let mut page = Page::Quota;
    let mut quota_view = QuotaView::Cards;
    // 每张卡圆环显示的窗口:Go=Monthly,Codex=7 days,GOAT=Monthly(越界由渲染端钳制)
    let mut ring_window: [usize; 3] = [2, 1, 2];
    let mut sel_agent: Option<usize> = None;
    let mut sel_day: Option<usize> = None;
    let mut sel_heat: Option<String> = None;
    let mut chart_mode = false;
    let mut dim: usize = 0; // 用量页维度:0 Agent / 1 会话 / 2 项目
    // 配额阈值通知状态机:每 (卡, 额度) 记上一轮等级,跃迁才发 toast
    let mut prev_levels: Vec<(String, u8)> = Vec::new();
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
            sel_agent,
            sel_day,
            sel_heat: sel_heat.as_deref(),
            quota_view,
            ring_window: &ring_window,
            chart_mode,
            dim,
            tick,
        };
        terminal.draw(|f| ui::render(f, &view, &mut hits))?;
        if refreshing {
            tick += 1; // 刷新动画帧
        }

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => {
                        if sel_agent.is_some() || sel_day.is_some() || sel_heat.is_some() {
                            sel_agent = None;
                            sel_day = None;
                            sel_heat = None;
                        } else {
                            break;
                        }
                    }
                    KeyCode::Char('r') if !in_flight => {
                        spawn_refresh(tx.clone());
                        in_flight = true;
                        refreshing = true;
                    }
                    KeyCode::Char('v') => {
                        quota_view = match quota_view {
                            QuotaView::Cards => QuotaView::Rings,
                            QuotaView::Rings => QuotaView::Cards,
                        };
                    }
                    KeyCode::Char('g') => {
                        chart_mode = !chart_mode;
                        sel_day = None;
                    }
                    KeyCode::Char('p') => {
                        ui::theme::cycle();
                        ui::theme::save(&home);
                    }
                    KeyCode::Char('l') => {
                        ui::i18n::cycle();
                        ui::i18n::save(&home);
                    }
                    KeyCode::Char('s') => {
                        dim = (dim + 1) % 3;
                        sel_agent = None;
                    }
                    KeyCode::Tab | KeyCode::Down | KeyCode::Up => {
                        page = match page {
                            Page::Quota => Page::Usage,
                            Page::Usage => Page::Heatmap,
                            Page::Heatmap => Page::Quota,
                        };
                        sel_agent = None;
                        sel_day = None;
                        sel_heat = None;
                    }
                    KeyCode::Char('1') => {
                        page = Page::Quota;
                        sel_agent = None;
                        sel_day = None;
                        sel_heat = None;
                    }
                    KeyCode::Char('2') => {
                        page = Page::Usage;
                        sel_agent = None;
                        sel_day = None;
                        sel_heat = None;
                    }
                    KeyCode::Char('3') => {
                        page = Page::Heatmap;
                        sel_agent = None;
                        sel_day = None;
                        sel_heat = None;
                    }
                    _ => {}
                },
                Event::Mouse(m) if m.kind == MouseEventKind::Up(MouseButton::Left) => {
                    match hits.hit(m.column, m.row) {
                        Some(ui::Hit::Sidebar(p)) => {
                            page = p;
                            sel_agent = None;
                            sel_day = None;
                            sel_heat = None;
                        }
                        Some(ui::Hit::ToggleView) => {
                            quota_view = match quota_view {
                                QuotaView::Cards => QuotaView::Rings,
                                QuotaView::Rings => QuotaView::Cards,
                            };
                        }
                        Some(ui::Hit::ToggleChart) => {
                            chart_mode = !chart_mode;
                            sel_day = None;
                        }
                        Some(ui::Hit::DimTab(i)) => {
                            dim = i;
                            sel_agent = None;
                        }
                        Some(ui::Hit::Ring(i)) => {
                            // 点击环体循环该卡的窗口:5h → 周 → 月 → 5h
                            if let Some(snap) = &snapshot
                                && let Some(model::Card::Live(r)) = snap.cards.get(i)
                                && !r.limits.is_empty()
                            {
                                ring_window[i] = (ring_window[i] + 1) % r.limits.len();
                            }
                        }
                        Some(ui::Hit::AgentRow(i)) => {
                            sel_day = None;
                            sel_heat = None;
                            sel_agent = if sel_agent == Some(i) { None } else { Some(i) };
                        }
                        Some(ui::Hit::DayRow(i)) => {
                            sel_agent = None;
                            sel_heat = None;
                            sel_day = if sel_day == Some(i) { None } else { Some(i) };
                        }
                        Some(ui::Hit::HeatCell(date)) => {
                            sel_agent = None;
                            sel_day = None;
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
