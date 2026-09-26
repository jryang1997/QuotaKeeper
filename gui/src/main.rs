//! wry + tao 外壳:内嵌前端、自定义协议加载、IPC 推送与命令回传。
//!
//! 架构:tao 事件循环为主线程;采集线程经 mpsc 发 Refreshed;主循环收到后
//! evaluate_script 推 JSON 给前端。前端 → Rust 的命令走 wry IPC handler
//! (window.external.invoke)。前端资源由 include_dir 编入,自定义协议
//! `app://` 提供,单 exe 可拷走即用。

pub mod ipc;
pub mod refresh;

use std::borrow::Cow;
use std::sync::mpsc;

use include_dir::{Dir, include_dir};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::http::{Request, Response, header::CONTENT_TYPE};
use wry::{WebViewBuilder, WebViewId};

use refresh::Refreshed;

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
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

/// 从内嵌目录解析 `app://local/<path>`;目录命中则回 index.html(SPA 兜底)。
fn resolve(path: &str) -> Option<(Vec<u8>, &'static str)> {
    let p = path.trim_start_matches('/');
    let f = FRONTEND
        .get_file(p)
        .or_else(|| FRONTEND.get_file(format!("{p}/index.html")))?;
    Some((f.contents().to_vec(), mime(p)))
}

fn app_response(req: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
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
    let el = EventLoop::new();

    let icon = tao::window::Icon::from_rgba(load_icon_rgba(), 32, 32).ok();

    let win = WindowBuilder::new()
        .with_title("Token Monitor")
        .with_inner_size(tao::dpi::LogicalSize::new(1180.0, 760.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(860.0, 560.0))
        .with_window_icon(icon)
        .build(&el)?;

    let (tx, rx) = mpsc::channel::<Refreshed>();

    let ipc_tx = tx.clone();
    let webview = WebViewBuilder::new()
        .with_custom_protocol(String::from("app"), move |id: WebViewId, req| {
            let _ = id;
            app_response(req)
        })
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            if req.body().contains("refresh") {
                refresh::spawn(ipc_tx.clone());
            }
        })
        .with_url("app://local/index.html")
        .build(&win)?;

    refresh::spawn(tx.clone());
    let mut next_refresh = std::time::Instant::now() + refresh::PERIOD;
    let webview = std::cell::RefCell::new(webview);

    el.run(move |event, _target, control_flow| {
        match event {
            // 定时唤醒到点才刷;WakeUp 与 ResumeTimeReached 都可能是 WaitUntil 触发
            Event::NewEvents(tao::event::StartCause::ResumeTimeReached { .. })
                if refresh::is_due(next_refresh) =>
            {
                refresh::spawn(tx.clone());
                next_refresh = std::time::Instant::now() + refresh::PERIOD;
            }
            Event::MainEventsCleared => {
                while let Ok(r) = rx.try_recv() {
                    let quota_error = r.quota.as_ref().err().cloned();
                    let snap = match r.quota {
                        Ok(s) => s,
                        Err(_) => fallback_snapshot(),
                    };
                    let payload = ipc::push(r.fetched_at_ms, quota_error, &snap, &r.ledger);
                    let js = format!("window.__onPush && window.__onPush({payload});");
                    // WebView 未就绪时 evaluate_script 出错:丢弃本轮,60s 后自然重推
                    let _ = webview.borrow().evaluate_script(&js);
                    *control_flow = ControlFlow::WaitUntil(next_refresh);
                }
                *control_flow = ControlFlow::WaitUntil(next_refresh);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
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
            for c in row.chunks_exact(4) {
                flipped.extend_from_slice(&[c[2], c[1], c[0], c[3]]); // BGRA→RGBA + 行序翻转
            }
        }
        out.push((w, h, flipped));
    }
    Some(out)
}
