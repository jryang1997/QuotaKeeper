#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! wry + tao 外壳:内嵌前端、自定义协议加载、IPC 推送与命令回传。
//!
//! 架构:tao 事件循环为主线程;采集线程经 mpsc 发 Refreshed;主循环收到后
//! evaluate_script 推 JSON 给前端。前端 → Rust 的命令走 wry IPC handler
//! (window.external.invoke)。前端资源由 include_dir 编入,自定义协议
//! `app://` 提供；GNU 构建另需同目录的 WebView2Loader.dll（build.rs 自动复制）。

pub mod agents;
pub mod cache;
pub mod desktop;
pub mod export;
pub mod ipc;
pub mod lan;
pub mod backdrop;
pub mod overlay;
pub mod refresh;
pub mod single_instance;

use std::borrow::Cow;
use std::sync::mpsc;

use chrono::Timelike;
use include_dir::{Dir, include_dir};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use wry::http::{Request, Response, header::CONTENT_TYPE};
use wry::{WebViewBuilder, WebViewId};

use refresh::Refreshed;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IpcCommand {
    cmd: String,
    amount: Option<f64>,
    enabled: Option<bool>,
    quiet_hours: Option<bool>,
    pace_alerts: Option<bool>,
    close_behavior: Option<String>,
    global_shortcut: Option<bool>,
    autostart: Option<bool>,
    go_api_key: Option<String>,
    goat_api_key: Option<String>,
    deepseek_api_key: Option<String>,
    lang: Option<u8>,
    format: Option<String>,
    start: Option<String>,
    end: Option<String>,
    agent: Option<String>,
    project: Option<String>,
    provider: Option<String>,
    models: Option<Vec<String>>,
    image: Option<String>,
}

enum UserEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
    Quit,
    SetNotifications {
        enabled: bool,
        pace_alerts: bool,
        quiet_hours: bool,
        lang: u8,
    },
    SetCloseBehavior(String),
    SetGlobalShortcut(bool),
    SetAutostart(bool),
    SetGoApiKey(Option<String>),
    SetGoatApiKey(Option<String>),
    SetDeepSeekApiKey(Option<String>),
    QueryNativePreferences,
    GlobalShortcut,
    ShortcutStatus(bool),
    Export(String, export::ExportSelection),
    AgentsScanned(String),
    AgentUpdateFinished {
        results: Vec<(String, bool, String)>,
        payload: String,
    },
    OverlayToggle,
    OverlayHide,
    OverlayDrag,
    OverlaySwitch(i32),
    ProviderKeyReveal(String),
    PlanModelsImport(String, Vec<String>),
    SaveShareImage(String),
    SetLanDashboard(bool),
}

fn quota_levels(snap: &tui_token_monitor::model::Snapshot) -> Vec<(String, u8, f64)> {
    use tui_token_monitor::model::{Card, QuotaSource};
    snap.cards
        .iter()
        .flat_map(|card| match card {
            Card::Live(report) if report.source != QuotaSource::CpaLocal => report
                .limits
                .iter()
                .map(|limit| {
                    let title = format!(
                        "{} · {}",
                        report
                            .provider
                            .title()
                            .split(" ·")
                            .next()
                            .unwrap_or("Token"),
                        limit.label
                    );
                    (
                        title,
                        tui_token_monitor::ui::notify::level_for(limit.used_pct),
                        limit.used_pct,
                    )
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn in_quiet_hours() -> bool {
    let hour = chrono::Local::now().hour();
    !(8..22).contains(&hour)
}

/// 模型名只放行常规字符,防把任意内容写进 settings.json。
fn valid_model_name(model: &str) -> bool {
    let model = model.trim();
    !model.is_empty()
        && model.len() <= 128
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/:@+".contains(c))
}

/// 首页套餐卡可展示的 API key(仅应用自己存在凭据管理器里的)。
fn provider_key(slug: &str) -> Option<String> {
    use tui_token_monitor::credential;
    match slug {
        "opencode-go" => credential::open_code_go_key().ok().flatten(),
        "goat" => credential::command_code_key().ok().flatten(),
        "glm" => credential::glm_key().ok().flatten(),
        "kimi" => credential::kimi_key().ok().flatten(),
        _ => None,
    }
}

/// LAN 看板状态 JSON:实际监听端口 + 局域网完整地址;server 为 None 即未运行。
fn lan_status_json(server: &Option<lan::LanServer>) -> String {
    let (port, url) = match server.as_ref() {
        Some(server) => match lan::lan_ip() {
            Some(ip) => (
                server.port.to_string(),
                format!("\"http://{ip}:{}/\"", server.port),
            ),
            None => (server.port.to_string(), "null".to_string()),
        },
        None => ("null".to_string(), "null".to_string()),
    };
    format!("{{\"enabled\":{},\"port\":{port},\"url\":{url}}}", server.is_some())
}

pub static FRONTEND: Dir = include_dir!("$CARGO_MANIFEST_DIR/frontend/dist");

fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

/// 从内嵌目录解析 `app://local/<path>`;目录命中则回 index.html(SPA 兜底)。
pub(crate) fn resolve(path: &str) -> Option<(Vec<u8>, &'static str)> {
    let p = path.trim_start_matches('/');
    let f = FRONTEND
        .get_file(p)
        .or_else(|| FRONTEND.get_file(format!("{p}/index.html")))?;
    Some((f.contents().to_vec(), mime(p)))
}

pub(crate) fn app_response(req: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = req.uri().path().to_string();
    match resolve(&path) {
        Some((body, m)) => Response::builder()
            .status(200)
            .header(CONTENT_TYPE, m)
            .body(Cow::Owned(body))
            .unwrap(),
        None => Response::builder()
            .status(404)
            .header(CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Cow::Borrowed(b"not found".as_slice()))
            .unwrap(),
    }
}

fn main() -> anyhow::Result<()> {
    let Some(_instance) = single_instance::acquire()? else {
        return Ok(());
    };
    let el = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = el.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Tray(event));
    }));
    let proxy = el.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));
    let shortcut_proxy = el.create_proxy();
    let shortcut_status_proxy = el.create_proxy();
    let shortcut_control = desktop::global_shortcut_worker(
        move || {
            let _ = shortcut_proxy.send_event(UserEvent::GlobalShortcut);
        },
        move |available| {
            let _ = shortcut_status_proxy.send_event(UserEvent::ShortcutStatus(available));
        },
    );

    let tray_menu = Menu::new();
    let show_item = MenuItem::new("显示 Token 监控", true, None);
    let refresh_item = MenuItem::new("刷新数据", true, None);
    let overlay_item = MenuItem::new("悬浮窗（置顶三环）", true, None);
    let settings_item = MenuItem::new("设置", true, None);
    let quit_item = MenuItem::new("退出", true, None);
    tray_menu.append_items(&[&show_item, &refresh_item, &overlay_item, &settings_item, &quit_item])?;

    let icon = tao::window::Icon::from_rgba(load_icon_rgba(), 32, 32).ok();

    let win = WindowBuilder::new()
        .with_title("Token Monitor")
        .with_inner_size(tao::dpi::LogicalSize::new(1180.0, 760.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(960.0, 640.0))
        .with_window_icon(icon)
        .build(&el)?;

    let (tx, rx) = mpsc::channel::<Refreshed>();

    let ipc_tx = tx.clone();
    let ipc_proxy = el.create_proxy();
    let overlay_proxy = ipc_proxy.clone();
    // devtools 常开:release 下 wry 默认禁用,关掉后 CDP 视觉验收无从下手
    let webview = WebViewBuilder::new()
        .with_devtools(true)
        .with_custom_protocol(String::from("app"), move |id: WebViewId, req| {
            let _ = id;
            app_response(req)
        })
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            let Ok(command) = serde_json::from_str::<IpcCommand>(req.body()) else {
                return;
            };
            match command.cmd.as_str() {
                "refresh" => refresh::spawn(ipc_tx.clone()),
                "setPlanBenchmark" => {
                    if let Some(amount) = command.amount
                        && tui_token_monitor::settings::save_monthly_plan_benchmark(amount).is_ok()
                    {
                        refresh::spawn_ledger(ipc_tx.clone());
                    }
                }
                "resetPlanBenchmark" => {
                    if tui_token_monitor::settings::reset_monthly_plan_benchmark().is_ok() {
                        refresh::spawn_ledger(ipc_tx.clone());
                    }
                }
                "quit" => {
                    let _ = ipc_proxy.send_event(UserEvent::Quit);
                }
                "setNotifications" => {
                    let _ = ipc_proxy.send_event(UserEvent::SetNotifications {
                        enabled: command.enabled.unwrap_or(false),
                        pace_alerts: command.pace_alerts.unwrap_or(false),
                        quiet_hours: command.quiet_hours.unwrap_or(false),
                        lang: command.lang.unwrap_or(0),
                    });
                }
                "setCloseBehavior" => {
                    if let Some(value) = command.close_behavior {
                        let _ = ipc_proxy.send_event(UserEvent::SetCloseBehavior(value));
                    }
                }
                "setGlobalShortcut" => {
                    let _ = ipc_proxy.send_event(UserEvent::SetGlobalShortcut(
                        command.global_shortcut.unwrap_or(false),
                    ));
                }
                "setAutostart" => {
                    let _ = ipc_proxy
                        .send_event(UserEvent::SetAutostart(command.autostart.unwrap_or(false)));
                }
                "queryNativePreferences" => {
                    let _ = ipc_proxy.send_event(UserEvent::QueryNativePreferences);
                }
                "setGoApiKey" => {
                    if let Some(key) = command.go_api_key {
                        let _ = ipc_proxy.send_event(UserEvent::SetGoApiKey(Some(key)));
                    }
                }
                "deleteGoApiKey" => {
                    let _ = ipc_proxy.send_event(UserEvent::SetGoApiKey(None));
                }
                "setGoatApiKey" => {
                    if let Some(key) = command.goat_api_key {
                        let _ = ipc_proxy.send_event(UserEvent::SetGoatApiKey(Some(key)));
                    }
                }
                "deleteGoatApiKey" => {
                    let _ = ipc_proxy.send_event(UserEvent::SetGoatApiKey(None));
                }
                "setDeepSeekApiKey" => {
                    if let Some(key) = command.deepseek_api_key {
                        let _ = ipc_proxy.send_event(UserEvent::SetDeepSeekApiKey(Some(key)));
                    }
                }
                "deleteDeepSeekApiKey" => {
                    let _ = ipc_proxy.send_event(UserEvent::SetDeepSeekApiKey(None));
                }
                "exportUsage" => {
                    if let Some(format) = command.format {
                        let selection = export::ExportSelection {
                            start: command.start,
                            end: command.end,
                            agent: command.agent,
                            project: command.project,
                        };
                        let _ = ipc_proxy.send_event(UserEvent::Export(format, selection));
                    }
                }
                "scanAgents" => agents::scan(ipc_proxy.clone()),
                "toggleOverlay" => {
                    let _ = ipc_proxy.send_event(UserEvent::OverlayToggle);
                }
                "revealProviderKey" => {
                    if let Some(provider) = command.provider.as_deref().filter(|id| !id.is_empty()) {
                        let _ = ipc_proxy.send_event(UserEvent::ProviderKeyReveal(provider.to_string()));
                    }
                }
                "importPlanModels" => {
                    if let (Some(provider), Some(models)) = (command.provider.clone(), command.models.clone())
                        && !provider.is_empty()
                        && !models.is_empty()
                        && models.iter().all(|model| valid_model_name(model))
                    {
                        let _ = ipc_proxy.send_event(UserEvent::PlanModelsImport(provider, models));
                    }
                }
                "saveShareImage" => {
                    if let Some(image) = command.image {
                        let _ = ipc_proxy.send_event(UserEvent::SaveShareImage(image));
                    }
                }
                "setLanDashboard" => {
                    let _ = ipc_proxy.send_event(UserEvent::SetLanDashboard(command.enabled.unwrap_or(false)));
                }
                "runAgentUpdate" => {
                    if let Some(agent) = command.agent.as_deref().filter(|id| !id.is_empty()) {
                        agents::update(ipc_proxy.clone(), agent.to_string());
                    }
                }
                _ => {}
            }
        })
        .with_url("app://local/index.html")
        .build(&win)?;

    refresh::spawn_quota(tx.clone());
    refresh::spawn_ledger(tx.clone());
    let mut next_ledger_refresh = std::time::Instant::now() + refresh::LEDGER_PERIOD;
    let mut next_quota_refresh = std::time::Instant::now() + refresh::QUOTA_PERIOD;
    let mut next_backdrop = std::time::Instant::now();
    let mut backdrop_seq = 0u64;
    let mut overlay_dirty = true;
    let mut latest_snapshot = fallback_snapshot();
    let mut quota_error = None;
    let mut alert_tracker = tui_token_monitor::ui::notify::AlertTracker::default();
    let mut notify_enabled = false;
    let mut pace_alerts_enabled = false;
    let mut quiet_hours = false;
    let mut close_behavior = String::from("tray");
    let mut notify_lang = 0;
    let mut latest_quota_fetched_at_ms = 0_u64;
    let mut latest_quota_checked_at_ms = 0_u64;
    let mut latest_ledger: Option<std::sync::Arc<tui_token_monitor::ledger::LedgerReport>> = None;
    let mut latest_ledger_fetched_at_ms = 0_u64;
    let mut latest_goat_usage = None::<tui_token_monitor::goat::UsageSummary>;
    let mut latest_deepseek_balance = tui_token_monitor::deepseek_balance::BalanceState::MissingKey;
    let webview = std::cell::RefCell::new(webview);
    let mut overlay: Option<(tao::window::Window, wry::WebView)> = None;
    let mut overlay_index: usize = 0;
    let mut overlay_last_save = std::time::Instant::now();
    let mut lan_server = if tui_token_monitor::settings::lan_dashboard_enabled() {
        lan::LanServer::start()
    } else {
        None
    };
    let mut tray = None;
    let mut tray_level = None::<u8>;
    let mut last_tray_click = None::<std::time::Instant>;
    let mut cache_replayed = false;
    let mut cache_last_write = std::time::Instant::now() - std::time::Duration::from_secs(3600);
    let mut cache_saved_quota_at = 0_u64;

    el.run(move |event, target, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) => {
                if let Ok(icon) = tray_icon::Icon::from_rgba(load_icon_rgba(), 32, 32) {
                    tray = TrayIconBuilder::new()
                        .with_icon(icon)
                        .with_tooltip("Token Monitor · 左键打开，右键查看更多")
                        .with_menu(Box::new(tray_menu.clone()))
                        .build()
                        .ok();
                }
            }
            Event::UserEvent(UserEvent::Tray(TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            })) => {
                let now = std::time::Instant::now();
                if last_tray_click
                    .is_none_or(|previous| now.duration_since(previous).as_millis() > 350)
                {
                    let show = !win.is_visible();
                    win.set_visible(show);
                    if show {
                        win.set_focus();
                    }
                    last_tray_click = Some(now);
                }
            }
            Event::UserEvent(UserEvent::Menu(event)) if event.id == show_item.id() => {
                win.set_visible(true);
                win.set_focus();
            }
            Event::UserEvent(UserEvent::Menu(event)) if event.id == refresh_item.id() => {
                refresh::spawn(tx.clone());
            }
            Event::UserEvent(UserEvent::Menu(event)) if event.id == overlay_item.id() => {
                if let Some((window, _)) = overlay.as_ref() {
                    let show = !window.is_visible();
                    window.set_visible(show);
                    if show {
                        window.set_focus();
                    }
                } else if let Ok((window, view)) = overlay::create(target, overlay_proxy.clone()) {
                    window.set_visible(true);
                    overlay::push(&view, &latest_snapshot, overlay_index);
                    overlay = Some((window, view));
                }
            }
            Event::UserEvent(UserEvent::Menu(event)) if event.id == settings_item.id() => {
                win.set_visible(true);
                win.set_focus();
                let _ = webview
                    .borrow()
                    .evaluate_script("window.__showSettings?.()");
            }
            Event::UserEvent(UserEvent::Menu(event)) if event.id == quit_item.id() => {
                tray.take();
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Quit) => {
                tray.take();
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::SetNotifications {
                enabled,
                pace_alerts,
                quiet_hours: quiet,
                lang,
            }) => {
                notify_enabled = enabled;
                pace_alerts_enabled = pace_alerts;
                quiet_hours = quiet;
                notify_lang = lang;
            }
            Event::UserEvent(UserEvent::SetCloseBehavior(value)) => {
                if matches!(value.as_str(), "tray" | "exit") {
                    close_behavior = value;
                }
            }
            Event::UserEvent(UserEvent::SetGlobalShortcut(enabled)) => {
                let _ = shortcut_control.send(enabled);
            }
            Event::UserEvent(UserEvent::GlobalShortcut) => {
                let show = !win.is_visible();
                win.set_visible(show);
                if show { win.set_focus(); }
            }
            Event::UserEvent(UserEvent::ShortcutStatus(available)) => {
                let _ = webview.borrow().evaluate_script(&format!("window.__onShortcutStatus?.({available})"));
            }
            Event::UserEvent(UserEvent::SetAutostart(enabled)) => {
                let result = desktop::set_autostart(enabled).is_ok();
                let _ = webview.borrow().evaluate_script(&format!("window.__onAutostartResult?.({result})"));
            }
            Event::UserEvent(UserEvent::QueryNativePreferences) => {
                // webview 就绪信号:先把上次留存的 push 回放给前端(旧数据立即可见),
                // 真实采集到位后由正常循环覆盖。
                if !cache_replayed {
                    cache_replayed = true;
                    if let Some(payload) = cache::load()
                        && let Some(stale) = cache::mark_stale(&payload)
                    {
                        let js = format!("window.__onPush && window.__onPush({stale});");
                        let _ = webview.borrow().evaluate_script(&js);
                    }
                }
                let enabled = desktop::autostart_enabled().unwrap_or(false);
                let go_key_stored = tui_token_monitor::credential::has_open_code_go_key();
                let goat_key_stored = tui_token_monitor::credential::has_command_code_key();
                let deepseek_key_stored = tui_token_monitor::credential::has_deepseek_key();
                let plan_models = serde_json::to_string(&tui_token_monitor::settings::plan_models())
                    .unwrap_or_else(|_| "{}".into());
                let lan_status = lan_status_json(&lan_server);
                let _ = webview.borrow().evaluate_script(&format!("window.__onNativeState?.({{autostart:{enabled},goKeyStored:{go_key_stored},goatKeyStored:{goat_key_stored},deepseekKeyStored:{deepseek_key_stored},planModels:{plan_models},lan:{lan_status}}})"));
            }
            Event::UserEvent(UserEvent::SetGoApiKey(key)) => {
                let result = match key {
                    Some(key) => tui_token_monitor::credential::set_open_code_go_key(&key),
                    None => tui_token_monitor::credential::delete_open_code_go_key(),
                };
                let success = result.is_ok();
                let stored = tui_token_monitor::credential::has_open_code_go_key();
                let _ = webview.borrow().evaluate_script(&format!("window.__onGoKeyResult?.({{success:{success},stored:{stored}}})"));
                if success { refresh::spawn_quota(tx.clone()); }
            }
            Event::UserEvent(UserEvent::SetGoatApiKey(key)) => {
                let result = match key {
                    Some(key) => tui_token_monitor::credential::set_command_code_key(&key),
                    None => tui_token_monitor::credential::delete_command_code_key(),
                };
                let success = result.is_ok();
                let stored = tui_token_monitor::credential::has_command_code_key();
                let _ = webview.borrow().evaluate_script(&format!("window.__onGoatKeyResult?.({{success:{success},stored:{stored}}})"));
                if success { refresh::spawn_quota(tx.clone()); }
            }
            Event::UserEvent(UserEvent::SetDeepSeekApiKey(key)) => {
                let result = match key {
                    Some(key) => tui_token_monitor::credential::set_deepseek_key(&key),
                    None => tui_token_monitor::credential::delete_deepseek_key(),
                };
                let success = result.is_ok();
                let stored = tui_token_monitor::credential::has_deepseek_key();
                let _ = webview.borrow().evaluate_script(&format!("window.__onDeepSeekKeyResult?.({{success:{success},stored:{stored}}})"));
                if success { refresh::spawn_quota(tx.clone()); }
            }
            Event::UserEvent(UserEvent::OverlayToggle) => {
                if let Some((window, view)) = overlay.as_ref() {
                    let show = !window.is_visible();
                    window.set_visible(show);
                    if show {
                        window.set_focus();
                        overlay::push(view, &latest_snapshot, overlay_index);
                    }
                } else if let Ok((window, view)) = overlay::create(target, overlay_proxy.clone()) {
                    window.set_visible(true);
                    overlay::push(&view, &latest_snapshot, overlay_index);
                    overlay = Some((window, view));
                }
            }
            Event::UserEvent(UserEvent::OverlayHide) => {
                if let Some((window, _)) = overlay.as_ref() {
                    window.set_visible(false);
                }
            }
            Event::UserEvent(UserEvent::OverlayDrag) => {
                if let Some((window, _)) = overlay.as_ref() {
                    let _ = window.drag_window();
                }
            }
            Event::UserEvent(UserEvent::OverlaySwitch(direction)) => {
                overlay_dirty = true;
                let total = overlay::live_len(&latest_snapshot);
                if total > 0 {
                    overlay_index = ((overlay_index as i64 + direction as i64).rem_euclid(total as i64)) as usize;
                }
                if let Some((window, view)) = overlay.as_ref()
                    && window.is_visible()
                {
                    overlay::push(view, &latest_snapshot, overlay_index);
                }
            }
            Event::UserEvent(UserEvent::ProviderKeyReveal(provider)) => {
                // 凭据管理器按需读,内存即用即弃;前端只在悬停时请求
                let key = provider_key(&provider);
                let payload = serde_json::json!({ "provider": provider, "key": key });
                let _ = webview.borrow().evaluate_script(&format!(
                    "window.__onProviderKey?.({})",
                    payload
                ));
            }
            Event::UserEvent(UserEvent::PlanModelsImport(provider, models)) => {
                let ok = tui_token_monitor::settings::save_plan_models(&provider, &models).is_ok();
                let payload = serde_json::json!({ "provider": provider, "ok": ok });
                let _ = webview.borrow().evaluate_script(&format!(
                    "window.__onPlanModelsSaved?.({})",
                    payload
                ));
            }
            Event::UserEvent(UserEvent::SaveShareImage(image)) => {
                let status = match export::save_png(&image) {
                    Ok(Some(_)) => "saved",
                    Ok(None) => "canceled",
                    Err(_) => "error",
                };
                let message = serde_json::to_string(status).unwrap_or_else(|_| "\"error\"".into());
                let _ = webview
                    .borrow()
                    .evaluate_script(&format!("window.__onShareImageSaved?.({message})"));
            }
            Event::UserEvent(UserEvent::SetLanDashboard(enabled)) => {
                let _ = tui_token_monitor::settings::save_lan_dashboard_enabled(enabled);
                lan_server = None; // Drop 即停
                if enabled {
                    lan_server = lan::LanServer::start();
                }
                let payload = lan_status_json(&lan_server);
                let _ = webview
                    .borrow()
                    .evaluate_script(&format!("window.__onLanStatus?.({payload})"));
            }
            Event::UserEvent(UserEvent::Export(format, selection)) => {
                let status = if let Some(report) = latest_ledger.as_ref() {
                    match export::save_report(report, &format, &selection) {
                        Ok(Some(_)) => "saved",
                        Ok(None) => "canceled",
                        Err(_) => "error",
                    }
                } else {
                    "unavailable"
                };
                let message = serde_json::to_string(status).unwrap_or_else(|_| "\"error\"".into());
                let _ = webview
                    .borrow()
                    .evaluate_script(&format!("window.__onExportResult?.({message})"));
            }
            Event::UserEvent(UserEvent::AgentsScanned(payload)) => {
                let _ = webview
                    .borrow()
                    .evaluate_script(&format!("window.__onAgentsScanned?.({payload});"));
            }
            Event::UserEvent(UserEvent::AgentUpdateFinished {
                results,
                payload,
                ..
            }) => {
                let report = serde_json::to_string(&results).unwrap_or_else(|_| "[]".into());
                let _ = webview.borrow().evaluate_script(&format!(
                    "window.__onAgentUpdateResult?.({{results:{report}}});window.__onAgentsScanned?.({payload});"
                ));
            }
            // 本地用量每 5 秒刷新,配额每 60 秒刷新。
            Event::NewEvents(tao::event::StartCause::ResumeTimeReached { .. }) => {
                let now = std::time::Instant::now();
                if refresh::is_due(next_ledger_refresh) {
                    refresh::spawn_ledger(tx.clone());
                    next_ledger_refresh = now + refresh::LEDGER_PERIOD;
                }
                if refresh::is_due(next_quota_refresh) {
                    refresh::spawn_quota(tx.clone());
                    next_quota_refresh = now + refresh::QUOTA_PERIOD;
                }
                // 悬浮窗毛玻璃:静止时 500ms 一帧,刚动过就按 60ms 快速跟上,
                // 否则拖动时背景图会明显滞后于窗口位置
                if overlay.as_ref().is_some_and(|(window, _)| window.is_visible())
                    && (refresh::is_due(next_backdrop) || overlay_dirty)
                {
                    next_backdrop = now
                        + std::time::Duration::from_millis(if overlay_dirty { 120 } else { 500 });
                    overlay_dirty = false;
                    backdrop_seq += 1;
                    if let Some((window, view)) = overlay.as_ref() {
                        overlay::push_backdrop(window, view, backdrop_seq);
                    }
                }
            }
            Event::MainEventsCleared => {
                while let Ok(r) = rx.try_recv() {
                    if let Some(quota) = r.quota {
                        latest_quota_checked_at_ms = r.fetched_at_ms;
                        match quota {
                            Ok(snapshot) => {
                                let current = quota_levels(&snapshot);
                                let level = current.iter().map(|(_, level, _)| *level).max().unwrap_or(3);
                                if tray_level != Some(level) {
                                    if let Some(tray_icon) = tray.as_ref()
                                        && let Ok(icon) = tray_icon::Icon::from_rgba(tray_icon_rgba(level), 32, 32)
                                    {
                                        let _ = tray_icon.set_icon(Some(icon));
                                    }
                                    tray_level = Some(level);
                                }
                                let alerts = alert_tracker.update(&snapshot, r.fetched_at_ms as i64);
                                if let Some(tray_icon) = tray.as_ref() {
                                    let tooltip = current
                                        .iter()
                                        .max_by(|a, b| a.2.total_cmp(&b.2))
                                        .map(|(title, _, pct)| {
                                            format!("Token Monitor · {title} {pct:.0}%")
                                        })
                                        .unwrap_or_else(|| {
                                            "Token Monitor · 暂无可用额度".to_string()
                                        });
                                    let _ = tray_icon.set_tooltip(Some(tooltip));
                                }
                                let lang = if notify_lang == 1 {
                                    tui_token_monitor::ui::i18n::Lang::En
                                } else {
                                    tui_token_monitor::ui::i18n::Lang::Zh
                                };
                                if !(quiet_hours && in_quiet_hours()) {
                                    if notify_enabled {
                                        for alert in alerts.quota {
                                            tui_token_monitor::ui::notify::toast(
                                                &alert.title,
                                                &format!("{:.0}% {}", alert.used_pct, alert.event.message_for(lang)),
                                            );
                                        }
                                        for alert in alerts.expiry {
                                            tui_token_monitor::ui::notify::toast(
                                                &tui_token_monitor::ui::notify::expiry_title_for(&alert.provider, lang),
                                                &tui_token_monitor::ui::notify::expiry_message_for(
                                                    alert.ends_at_ms, r.fetched_at_ms as i64, lang,
                                                ),
                                            );
                                        }
                                    }
                                    if pace_alerts_enabled {
                                        for alert in alerts.pace {
                                            let minutes = alert.exhaust_in_ms / 60_000;
                                            let message = if notify_lang == 1 {
                                                format!("Current pace is {:.0} points ahead; quota may run out in {minutes} min", alert.delta_points)
                                            } else {
                                                format!("比匀速快 {:.0} 个百分点；预计 {minutes} 分钟后耗尽", alert.delta_points)
                                            };
                                            tui_token_monitor::ui::notify::toast(&alert.title, &message);
                                        }
                                    }
                                }
                                latest_snapshot = snapshot;
                                quota_error = None;
                                latest_quota_fetched_at_ms = r.fetched_at_ms;
                            }
                            Err(error) => {
                                quota_error = Some(error);
                                if tray_level != Some(3) {
                                    if let Some(tray_icon) = tray.as_ref()
                                        && let Ok(icon) = tray_icon::Icon::from_rgba(tray_icon_rgba(3), 32, 32)
                                    {
                                        let _ = tray_icon.set_icon(Some(icon));
                                    }
                                    tray_level = Some(3);
                                }
                                if let Some(tray_icon) = tray.as_ref() {
                                    let _ =
                                        tray_icon.set_tooltip(Some("Token Monitor · 额度读取失败"));
                                }
                            }
                        }
                    }
                    if let Some(usage) = r.goat_usage {
                        latest_goat_usage = usage;
                    }
                    if let Some(balance) = r.deepseek_balance {
                        latest_deepseek_balance = balance;
                    }
                    if let Some(ledger) = r.ledger {
                        latest_ledger = Some(std::sync::Arc::from(*ledger));
                        latest_ledger_fetched_at_ms = r.fetched_at_ms;
                    }
                    if let Some(ledger) = latest_ledger.as_ref() {
                        let monthly_value = ipc::monthly_value_dto(ledger);
                        let payload = ipc::push(
                            latest_ledger_fetched_at_ms,
                            latest_quota_fetched_at_ms,
                            latest_quota_checked_at_ms,
                            quota_error.clone(),
                            &latest_snapshot,
                            ledger,
                            latest_goat_usage,
                            latest_deepseek_balance.clone(),
                        );
                        lan::publish(
                            &latest_snapshot,
                            Some(ledger),
                            latest_goat_usage.as_ref(),
                            Some(monthly_value.month_api_equivalent_usd),
                            Some(monthly_value.list_price_multiple),
                        );
                        let js = format!("window.__onPush && window.__onPush({payload});");
                        // WebView 未就绪时 evaluate_script 出错;下轮采集会自然重推
                        let _ = webview.borrow().evaluate_script(&js);
                        // 留存给下次启动回放:配额一有新结果就写,否则 30s 节流
                        if latest_quota_fetched_at_ms != cache_saved_quota_at
                            || cache_last_write.elapsed() >= std::time::Duration::from_secs(30)
                        {
                            cache::save(&payload);
                            cache_last_write = std::time::Instant::now();
                            cache_saved_quota_at = latest_quota_fetched_at_ms;
                        }
                    }
                }
                // 悬浮窗在每次数据落定后重推(webview 刚建好时的首推可能丢失)
                if let Some((window, view)) = overlay.as_ref()
                    && window.is_visible()
                {
                    overlay::push(view, &latest_snapshot, overlay_index);
                }
                *control_flow = ControlFlow::WaitUntil(std::cmp::min(
                    std::cmp::min(next_ledger_refresh, next_quota_refresh),
                    next_backdrop,
                ));
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Moved(position),
                ..
            } => {
                // 悬浮窗拖动落点节流落盘
                if overlay.as_ref().is_some_and(|(window, _)| window.id() == window_id) {
                    overlay_dirty = true;
                    if overlay_last_save.elapsed() >= std::time::Duration::from_secs(1) {
                        overlay_last_save = std::time::Instant::now();
                        overlay::save_position(position.x, position.y);
                    }
                }
            }
            // 悬浮窗缩放:尺寸与几何一起记,背景立即重抓(尺寸变了旧图不对)
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(_),
                ..
            } => {
                if let Some((window, _)) = overlay.as_ref()
                    && window.id() == window_id
                {
                    overlay_dirty = true;
                    let size = window.inner_size();
                    if let Ok(position) = window.outer_position() {
                        overlay::save_geometry(
                            position.x,
                            position.y,
                            Some((size.width, size.height)),
                        );
                    }
                }
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if overlay.as_ref().is_some_and(|(window, _)| window.id() == window_id) {
                    if let Some((window, _)) = overlay.as_ref() {
                        window.set_visible(false);
                    }
                } else if tray.is_some() && close_behavior == "tray" {
                    win.set_visible(false);
                } else {
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}

/// quota 全挂时前端也要拿到 ledger 与空卡(四卡永不缺位,同 TUI 语义)。
fn fallback_snapshot() -> tui_token_monitor::model::Snapshot {
    use tui_token_monitor::model::{Card, ProviderId, Snapshot};
    Snapshot {
        generated_at_ms: refresh::now_ms() as i64,
        cards: ProviderId::all()
            .into_iter()
            .map(|p| Card::Missing { provider: p })
            .collect(),
    }
}

fn load_icon_rgba() -> Vec<u8> {
    // assets/app.ico 的 32x32 帧;解析失败退回透明方块,不阻断启动。
    let ico = include_bytes!("../../assets/app.ico");
    if let Some(frames) = parse_ico(ico)
        && let Some((w, h, rgba)) = frames.into_iter().find(|(w, h, _)| *w == 32 && *h == 32)
        && (w as usize) * (h as usize) * 4 == rgba.len()
    {
        return rgba;
    }
    vec![0u8; 32 * 32 * 4]
}

/// Add a high-contrast risk dot to the app icon. 0=healthy, 1=warning, 2=exhausted, 3=stale.
fn tray_icon_rgba(level: u8) -> Vec<u8> {
    let mut rgba = load_icon_rgba();
    let color = match level {
        0 => [28, 176, 122, 255],
        1 => [245, 174, 52, 255],
        2 => [224, 74, 70, 255],
        _ => [128, 143, 159, 255],
    };
    for y in 0..32_i32 {
        for x in 0..32_i32 {
            let distance = (x - 24).pow(2) + (y - 24).pow(2);
            if distance <= 81 {
                let index = (y as usize * 32 + x as usize) * 4;
                rgba[index..index + 4].copy_from_slice(if distance > 49 {
                    &[255, 255, 255, 255]
                } else {
                    &color
                });
            }
        }
    }
    rgba
}

/// 最小 ICO/BMP 帧解析(32bpp BGRA,自底向上)。
fn parse_ico(data: &[u8]) -> Option<Vec<(u8, u8, Vec<u8>)>> {
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let e = 6 + i * 16;
        let _w = data[e];
        let _h = data[e + 1];
        let off = u32::from_le_bytes(data[e + 12..e + 16].try_into().ok()?) as usize;
        let size = u32::from_le_bytes(data[e + 8..e + 12].try_into().ok()?) as usize;
        let bmp = data.get(off..off + size)?;
        // BITMAPINFOHEADER:宽高(高度为两倍,含 mask)
        let bw = i32::from_le_bytes(bmp[4..8].try_into().ok()?);
        let bh2 = i32::from_le_bytes(bmp[8..12].try_into().ok()?);
        let bpp = u16::from_le_bytes(bmp[14..16].try_into().ok()?);
        if bpp != 32 || bw <= 0 || bh2 <= 0 {
            continue;
        }
        let (w, h) = (bw as u8, (bh2 / 2) as u8);
        let px = &bmp[40..40 + w as usize * h as usize * 4];
        let stride = w as usize * 4;
        let mut flipped = Vec::with_capacity(px.len());
        for row in px.chunks_exact(stride).rev() {
            for c in row.as_chunks::<4>().0 {
                flipped.extend_from_slice(&[c[2], c[1], c[0], c[3]]); // BGRA→RGBA + 行序翻转
            }
        }
        out.push((w, h, flipped));
    }
    Some(out)
}

#[cfg(test)]
mod tray_tests {
    use super::*;
    use tui_token_monitor::model::{
        Card, Limit, ProviderId, QuotaSource, Report, Snapshot, Status,
    };

    #[test]
    fn alert_levels_exclude_cpa_activity() {
        let snap = Snapshot {
            generated_at_ms: 0,
            cards: vec![
                Card::Live(Report {
                    provider: ProviderId::Codex,
                    source: QuotaSource::CodexAppServer,
                    plan_type: String::new(),
                    limits: vec![Limit {
                        label: "5h".into(),
                        used_pct: 85.0,
                        resets_in_ms: Some(1000),
                        status: Status::Warning,
                    }],
                    subscription_ends_at_ms: None,
                }),
                Card::Live(Report {
                    provider: ProviderId::Antigravity,
                    source: QuotaSource::CpaLocal,
                    plan_type: String::new(),
                    limits: vec![Limit {
                        label: "Today".into(),
                        used_pct: 100.0,
                        resets_in_ms: None,
                        status: Status::Exhausted,
                    }],
                    subscription_ends_at_ms: None,
                }),
            ],
        };
        let levels = quota_levels(&snap);
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].1, 1);
        assert_eq!(levels[0].2, 85.0);
    }

    #[test]
    fn alert_levels_include_cpa_upstream_quota() {
        let snap = Snapshot {
            generated_at_ms: 0,
            cards: vec![Card::Live(Report {
                provider: ProviderId::Antigravity,
                source: QuotaSource::CpaGoogleQuota,
                plan_type: String::new(),
                limits: vec![Limit {
                    label: "Gemini · 7 days".into(),
                    used_pct: 85.0,
                    resets_in_ms: Some(1000),
                    status: Status::Warning,
                }],
                subscription_ends_at_ms: None,
            })],
        };
        let levels = quota_levels(&snap);
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].1, 1);
    }
}
