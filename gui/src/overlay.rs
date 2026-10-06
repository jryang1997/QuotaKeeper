//! 悬浮小窗:置顶显示当前套餐的 5 小时/周/月三环,上下按钮在有数据的套餐间切换。
//! 独立 tao 无边框窗口 + 置顶 + 不进任务栏;位置记在 ~/.tui-token-monitor/overlay.json。
//! 环口径与 TUI quota.rs 一致(按 limit 标签关键字归 5h/周/月三槽)。

use serde::Serialize;
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use tao::window::{Window, WindowBuilder};
use wry::WebViewBuilder;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWA_COLOR_NONE,
};
use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, HGDIOBJ, SetWindowRgn};

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
    save_geometry(x, y, None);
}

/// 位置 + 尺寸一起记:胶囊形态下用户拖过边角,下次要按原样回来。
pub fn save_geometry(x: i32, y: i32, size: Option<(u32, u32)>) {
    let Some(path) = position_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut value = serde_json::json!({ "x": x, "y": y });
    if let Some((w, h)) = size {
        value["w"] = serde_json::json!(w);
        value["h"] = serde_json::json!(h);
    }
    let _ = std::fs::write(&path, serde_json::to_string(&value).unwrap_or_default());
}

fn load_size() -> Option<(u32, u32)> {
    let text = std::fs::read_to_string(position_path()?).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let w = value["w"].as_u64()? as u32;
    let h = value["h"].as_u64()? as u32;
    (MIN_SIZE.0 <= w && w <= MAX_SIZE.0 && MIN_SIZE.1 <= h && h <= MAX_SIZE.1).then_some((w, h))
}

/// 胶囊默认态(逻辑像素):小小一颗,环居中,信息收成一行。
pub const DEFAULT_SIZE: (f64, f64) = (188.0, 96.0);
/// 缩放下限:还能握住、再小就看不见了。
pub const MIN_SIZE: (u32, u32) = (96, 64);
/// 放大到当小组件用。
pub const MAX_SIZE: (u32, u32) = (720, 560);

/// 系统圆角 + 排除屏幕捕获(抓桌面像素的前提)。
/// 玻璃质感说明:webview 的透明像素透出的是父窗表面而非桌面,DWM 背板在无边框
/// 窗上也不渲染,所以"毛玻璃"由 backdrop.rs 抓来的真实桌面像素在页面内做
/// backdrop-filter ——模糊和透视都发生在 CSS 里,不是自绘假象。
fn prepare_window(window: &Window) -> bool {
    let hwnd = window.hwnd() as windows_sys::Win32::Foundation::HWND;
    let corner = DWMWCP_ROUND as u32;
    let corner_raw = corner.to_ne_bytes();
    // Win11 会给圆角无边框窗画 1px 系统描边;胶囊外留了 4px 透明缩放带,
    // 描边悬在胶囊四周显形成"一圈矩形"。设为 NONE 才干净(Win10 无此属性,失败忽略)。
    let border_none = DWMWA_COLOR_NONE.to_ne_bytes();
    unsafe {
        DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE as u32, corner_raw.as_ptr().cast(), 4);
        DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR as u32, border_none.as_ptr().cast(), 4);
    };
    // QA 旋钮:TUI_TM_OVERLAY_NO_EXCLUDE=1 时不把自己从屏幕捕获里排除,
    // 桌面级截图(CopyFromScreen)才能拍到悬浮窗的真实观感;平时必须排除,
    // 否则抓背板会把自己抓进去。
    if std::env::var_os("TUI_TM_OVERLAY_NO_EXCLUDE").is_some() {
        return true;
    }
    crate::backdrop::exclude_from_capture(hwnd)
}

/// 抓当前窗口客户区的桌面像素并推给页面。webview 不是 Send,只能主线程调用;
/// 抓取本身很快(胶囊尺寸下 1~3ms),静止 500ms 一帧、拖动 120ms 跟手。
/// 用客户区原点:无边框可缩放窗带隐形缩放边框,outer_position 会带出 ~11px 偏移,
/// 背景像素和桌面就对不上了。
pub fn push_backdrop(window: &Window, view: &wry::WebView, seq: u64) {
    let Ok(position) = window.inner_position() else {
        return;
    };
    let size = window.inner_size();
    let Some(uri) =
        crate::backdrop::capture_region_data_uri(position.x, position.y, size.width, size.height)
    else {
        return;
    };
    let _ = view.evaluate_script(&format!("window.__onBackdrop?.({seq}, {uri:?})"));
}

/// 把窗口区域裁成页面里胶囊的实际形状。本机(Win11 25H2 + 当前 WebView2 运行时)
/// 页面透明处会被下层的重定向/子层表面合成成白底,with_transparent +
/// WS_EX_NOREDIRECTIONBITMAP 都压不住;与其逐层排查,不如按形状裁窗 ——
/// 胶囊外的一切(含白底)物理上不存在,观感还更精确。
pub fn apply_shape(window: &Window) {
    let size = window.inner_size();
    let scale = window.scale_factor().max(1.0);
    let inset = (4.0 * scale).round() as i32; // 页面 .glass 的 4px 边距(物理像素)
    let w = size.width as i32;
    let h = size.height as i32;
    if w <= inset * 2 || h <= inset * 2 {
        return;
    }
    // 宽版圆角 26px(CSS);胶囊档是整 pill,半径 = 玻璃高度一半
    let wide = w as f64 / scale >= 300.0;
    let radius = if wide { (26.0 * scale).round() as i32 } else { (h - inset * 2) / 2 };
    let rgn = unsafe { CreateRoundRectRgn(inset, inset, w - inset, h - inset, radius * 2, radius * 2) };
    if rgn.is_null() {
        return;
    }
    // 调用成功后区域归系统所有,不要 DeleteObject
    let hwnd = window.hwnd() as HWND;
    let ok = unsafe { SetWindowRgn(hwnd, rgn, 1) } != 0;
    if !ok {
        unsafe { DeleteObject(rgn as HGDIOBJ) };
    }
}

/// 无边框置顶小窗;返回窗口与配套 webview,IPC 经 proxy 回主循环。
pub(crate) fn create(
    target: &EventLoopWindowTarget<UserEvent>,
    proxy: EventLoopProxy<UserEvent>,
) -> anyhow::Result<(Window, wry::WebView)> {
    let saved = load_size();
    let mut builder = WindowBuilder::new()
        .with_title("Token Monitor 悬浮窗")
        .with_inner_size(LogicalSize::new(DEFAULT_SIZE.0, DEFAULT_SIZE.1))
        .with_min_inner_size(LogicalSize::new(MIN_SIZE.0 as f64, MIN_SIZE.1 as f64))
        .with_max_inner_size(LogicalSize::new(MAX_SIZE.0 as f64, MAX_SIZE.1 as f64))
        .with_decorations(false)
        // 窗口本体也要透明:webview 的透明像素透出的是父窗表面,不设这个
        // 就是一块白色圆角矩形垫在胶囊底下(CDP 里看不出来,页面上 alpha 正常)。
        // 光有 DWM 空区域技巧在本机不够(重定向表面仍被合成成白底),
        // 直接 WS_EX_NOREDIRECTIONBITMAP 把表面摘掉,webview 视觉直透桌面。
        .with_transparent(true)
        .with_no_redirection_bitmap(true)
        .with_resizable(true)
        .with_always_on_top(true)
        .with_skip_taskbar(true)
        .with_visible(false);
    if let Some((x, y)) = load_position() {
        builder = builder.with_position(PhysicalPosition::new(x, y));
    } else if let Some(monitor) = target.primary_monitor() {
        // 默认落在主屏右下角,避开托盘与主窗口
        let bounds = monitor.size();
        let scale = monitor.scale_factor();
        let (w, h) = (DEFAULT_SIZE.0 * scale, DEFAULT_SIZE.1 * scale);
        builder = builder.with_position(PhysicalPosition::new(
            bounds.width as i32 - w as i32 - 24,
            bounds.height as i32 - h as i32 - 72,
        ));
    }
    let window = builder.build(target)?;
    let backdrop_ok = prepare_window(&window);
    if let Some((w, h)) = saved {
        // 物理像素回写(tao 用逻辑尺寸建窗,按当前 scale 折算);
        // 历史几何可能存进过低于下限的尺寸(物理/逻辑混记),按当前缩放抬回下限
        let scale = window.scale_factor().max(1.0);
        let min_w = (MIN_SIZE.0 as f64 * scale) as u32;
        let min_h = (MIN_SIZE.1 as f64 * scale) as u32;
        let (w, h) = (w.max(min_w), h.max(min_h));
        window.set_inner_size(LogicalSize::new(w as f64 / scale, h as f64 / scale));
    }
    apply_shape(&window);
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
    let _ = webview.evaluate_script(&format!("window.__backdropSupported = {backdrop_ok}"));
    Ok((window, webview))
}
