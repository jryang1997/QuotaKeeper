//! 悬浮小窗:置顶显示当前套餐的 5 小时/周/月三环,上下按钮在有数据的套餐间切换。
//! 独立 tao 无边框窗口 + 置顶 + 不进任务栏;位置记在 ~/.tui-token-monitor/overlay.json。
//! 环口径与 TUI quota.rs 一致(按 limit 标签关键字归 5h/周/月三槽)。

use serde::Serialize;
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use tao::window::{Window, WindowBuilder};
use wry::WebViewBuilder;
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMSBT_TRANSIENTWINDOW,
};

use crate::UserEvent;

use tui_token_monitor::model::{Card, QuotaSource, Snapshot};
use tui_token_monitor::ui::notify::level_for;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayRing {
    pub label: &'static str,
    pub used_pct: f64,
    pub resets_in_ms: Option<i64>,
    /// 0 健康 / 1 预警 / 2 耗尽,与托盘同口径
    pub level: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayData {
    pub title: String,
    pub plan_type: String,
    /// 固定顺序:5H、WEEK、MONTH;没有的槽位不出现
    pub rings: Vec<OverlayRing>,
    pub index: usize,
    pub total: usize,
}

/// 有配额环的 Live 卡(CPA 本地活动卡无环,同托盘口径排除)。
fn live_cards(snapshot: &Snapshot) -> Vec<&tui_token_monitor::model::Report> {
    snapshot
        .cards
        .iter()
        .filter_map(|card| match card {
            Card::Live(report) if report.source != QuotaSource::CpaLocal => Some(report),
            _ => None,
        })
        .collect()
}

pub fn live_len(snapshot: &Snapshot) -> usize {
    live_cards(snapshot).len()
}

pub fn data_for(snapshot: &Snapshot, index: usize) -> OverlayData {
    let cards = live_cards(snapshot);
    if cards.is_empty() {
        return OverlayData {
            title: "Token Monitor".into(),
            plan_type: String::new(),
            rings: Vec::new(),
            index: 0,
            total: 0,
        };
    }
    let report = cards[index.min(cards.len() - 1)];
    const SLOT_LABELS: [&str; 3] = ["5H", "WEEK", "MONTH"];
    let mut slots: [Option<OverlayRing>; 3] = [None, None, None];
    for limit in &report.limits {
        let label = limit.label.to_lowercase();
        let slot = if label.contains("5h") || label.contains("5 hour") {
            0
        } else if (label.contains("7") && label.contains("day")) || label.contains("week") {
            1
        } else if label.contains("month") {
            2
        } else {
            continue;
        };
        if slots[slot].is_none() {
            slots[slot] = Some(OverlayRing {
                label: SLOT_LABELS[slot],
                used_pct: limit.used_pct,
                resets_in_ms: limit.resets_in_ms,
                level: level_for(limit.used_pct),
            });
        }
    }
    OverlayData {
        title: report.provider.title().to_string(),
        plan_type: report.plan_type.clone(),
        rings: slots.into_iter().flatten().collect(),
        index: index.min(cards.len() - 1),
        total: cards.len(),
    }
}

pub fn push(webview: &wry::WebView, snapshot: &Snapshot, index: usize) {
    if let Ok(json) = serde_json::to_string(&data_for(snapshot, index)) {
        let _ = webview.evaluate_script(&format!("window.__onOverlayData?.({json});"));
    }
}

fn position_path() -> Option<std::path::PathBuf> {
    dirs_home().map(|home| home.join(".tui-token-monitor").join("overlay.json"))
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("USERPROFILE").map(std::path::PathBuf::from)
}

fn load_position() -> Option<(i32, i32)> {
    let text = std::fs::read_to_string(position_path()?).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some((value["x"].as_i64()? as i32, value["y"].as_i64()? as i32))
}

pub fn save_position(x: i32, y: i32) {
    let Some(path) = position_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &path,
        serde_json::to_string(&serde_json::json!({ "x": x, "y": y })).unwrap_or_default(),
    );
}

/// Win11 22H2+ 系统级 Acrylic 背板:桌面在窗口背后被真正模糊,webview 的
/// backdrop-filter 看不到窗外,玻璃质感靠这里。老系统调用失败返回 false,
/// 页面据 window.__overlayAcrylic 走自带上色兜底。
fn apply_acrylic_backdrop(window: &Window) -> bool {
    let hwnd = window.hwnd() as windows_sys::Win32::Foundation::HWND;
    let set = |attribute: u32, value: u32| -> bool {
        let raw = value.to_ne_bytes();
        let hr = unsafe { DwmSetWindowAttribute(hwnd, attribute, raw.as_ptr().cast(), 4) };
        hr == 0
    };
    set(DWMWA_USE_IMMERSIVE_DARK_MODE as u32, 1)
        & set(DWMWA_SYSTEMBACKDROP_TYPE as u32, DWMSBT_TRANSIENTWINDOW as u32)
        & set(DWMWA_WINDOW_CORNER_PREFERENCE as u32, DWMWCP_ROUND as u32)
}

/// 无边框置顶小窗;返回窗口与配套 webview,IPC 经 proxy 回主循环。
pub(crate) fn create(
    target: &EventLoopWindowTarget<UserEvent>,
    proxy: EventLoopProxy<UserEvent>,
) -> anyhow::Result<(Window, wry::WebView)> {
    let mut builder = WindowBuilder::new()
        .with_title("Token Monitor 悬浮窗")
        .with_inner_size(LogicalSize::new(232.0, 268.0))
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top(true)
        .with_skip_taskbar(true)
        .with_visible(false);
    if let Some((x, y)) = load_position() {
        builder = builder.with_position(PhysicalPosition::new(x, y));
    } else if let Some(monitor) = target.primary_monitor() {
        // 默认落在主屏右下角,避开托盘与主窗口
        let bounds = monitor.size();
        let scale = monitor.scale_factor();
        let (w, h) = (232.0 * scale, 268.0 * scale);
        builder = builder.with_position(PhysicalPosition::new(
            bounds.width as i32 - w as i32 - 24,
            bounds.height as i32 - h as i32 - 72,
        ));
    }
    let window = builder.build(target)?;
    let acrylic = apply_acrylic_backdrop(&window);
    let webview = WebViewBuilder::new()
        .with_url("app://local/overlay.html")
        .with_transparent(true)
        // 自定义协议按 webview 注册:悬浮窗要单独挂一遍
        .with_custom_protocol(String::from("app"), move |_id: wry::WebViewId, req| {
            crate::app_response(req)
        })
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            let event = match req.body().parse::<serde_json::Value>() {
                Ok(value) => match value["cmd"].as_str() {
                    Some("overlayHide") => Some(UserEvent::OverlayHide),
                    Some("overlayDrag") => Some(UserEvent::OverlayDrag),
                    Some("overlaySwitch") => {
                        Some(UserEvent::OverlaySwitch(value["direction"].as_i64().unwrap_or(0).signum() as i32))
                    }
                    _ => None,
                },
                Err(_) => None,
            };
            if let Some(event) = event {
                let _ = proxy.send_event(event);
            }
        })
        .build(&window)?;
    webview.evaluate_script(&format!(
        "window.__overlayAcrylic = {acrylic};"
    ))?;
    Ok((window, webview))
}
