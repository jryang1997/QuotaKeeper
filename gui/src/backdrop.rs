//! 悬浮窗真毛玻璃的桌面采样:webview 的 backdrop-filter 看不到窗外,而 DWM
//! 背板在无边框窗上不渲染(见 overlay.rs 注释),所以改成自己抓背后桌面像素,
//! 以 data URI 喂给页面 CSS `backdrop-filter` —— 模糊与透视都发生在页面内。
//!
//! 前提:窗口必须 `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`,把自己从
//! 屏幕捕获里摘出去,否则抓到的像素里含窗口自己。

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetDC, HBITMAP,
    ReleaseDC, SelectObject, BITMAPINFO, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetDesktopWindow, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
};

/// 把窗口从屏幕捕获中排除:抓桌面像素时不会把自己抓进去(录屏/截图里也看不到)。
/// 不支持时返回 false,页面退化为无背景图的自绘玻璃。
pub fn exclude_from_capture(hwnd: HWND) -> bool {
    unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) != 0 }
}

/// 抓取在途标志:拖动时 60ms 一帧,但单帧抓取+模糊可能更慢,
/// 没有这个门会堆线程。RAII 保证异常路径也复位。
static IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct InFlightGuard;

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        IN_FLIGHT.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 抓取窗口所在矩形(物理像素)的屏幕内容,编码成 PNG data URI。
/// 失败返回 None —— 调用方保持上一帧背景,不清空。
pub fn capture_region_data_uri(left: i32, top: i32, width: u32, height: u32) -> Option<String> {
    use std::sync::atomic::Ordering;
    if IN_FLIGHT.swap(true, Ordering::Relaxed) {
        return None; // 上一帧还在跑,这一帧直接跳过
    }
    let _guard = InFlightGuard;
    if width < 8 || height < 8 {
        return None;
    }
    let desktop = unsafe { GetDesktopWindow() };
    let screen_dc = unsafe { GetDC(desktop) };
    if screen_dc.is_null() {
        return None;
    }
    let mem_dc = unsafe { CreateCompatibleDC(screen_dc) };
    if mem_dc.is_null() {
        unsafe { ReleaseDC(desktop, screen_dc) };
        return None;
    }
    let bitmap: HBITMAP = unsafe { CreateCompatibleBitmap(screen_dc, width as i32, height as i32) };
    if bitmap.is_null() {
        unsafe {
            DeleteDC(mem_dc);
            ReleaseDC(desktop, screen_dc);
        }
        return None;
    }
    let old = unsafe { SelectObject(mem_dc, bitmap) };
    let copied = unsafe {
        BitBlt(
            mem_dc,
            0,
            0,
            width as i32,
            height as i32,
            screen_dc,
            left,
            top,
            SRCCOPY,
        )
    } != 0;
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFO>() as u32;
    info.bmiHeader.biWidth = width as i32;
    // 负高度 = 自上而下,省一次垂直翻转
    info.bmiHeader.biHeight = -(height as i32);
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB as u32;
    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    let scan_lines = height;
    let got = if copied {
        unsafe {
            GetDIBits(
                mem_dc,
                bitmap,
                0,
                scan_lines,
                pixels.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            )
        }
    } else {
        0
    };
    unsafe {
        SelectObject(mem_dc, old);
        DeleteObject(bitmap as _);
        DeleteDC(mem_dc);
        ReleaseDC(desktop, screen_dc);
    }
    if got == 0 {
        eprintln!("[backdrop] capture failed at {left},{top} {width}x{height}");
        return None;
    }
    let mut rgba = bgra_to_rgba(&pixels);
    // 模糊放在 Rust 侧:页面对伪元素做 backdrop-filter 只会糊掉伪元素自己,
    // 不会处理它带的背景图,所以毛玻璃的 blur 必须在送进页面前完成。
    box_blur(&mut rgba, width as usize, height as usize, BLUR_RADIUS);
    encode_png_data_uri(&rgba, width, height)
}

/// 毛玻璃模糊半径(像素):18 约等于 Apple 桌面 Liquid Glass 的糊度 ——
/// 桌面文字完全读不出,但保留大色块、光带与明暗,玻璃感最强。
const BLUR_RADIUS: usize = 18;

/// 可分离盒式模糊:横竖各一遍滑动窗口均值。半径 14 在胶囊尺寸下够快
/// (400ms 一帧绰绰有余),质量比单遍高斯略低但肉眼看不出。
fn box_blur(rgba: &mut Vec<u8>, width: usize, height: usize, radius: usize) {
    if radius == 0 || width < 3 || height < 3 {
        return;
    }
    let mut line = vec![0u8; rgba.len()];
    // 横向:窗口外的采样点贴边延拓,避免读出黑边
    for y in 0..height {
        let row = y * width * 4;
        for x in 0..width {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for d in 0..=radius {
                let i = row + x.saturating_add(d).min(width - 1) * 4;
                r += rgba[i] as u32;
                g += rgba[i + 1] as u32;
                b += rgba[i + 2] as u32;
                n += 1;
            }
            for d in 1..=radius {
                let i = row + x.saturating_sub(d) * 4;
                r += rgba[i] as u32;
                g += rgba[i + 1] as u32;
                b += rgba[i + 2] as u32;
                n += 1;
            }
            let o = row + x * 4;
            line[o] = (r / n) as u8;
            line[o + 1] = (g / n) as u8;
            line[o + 2] = (b / n) as u8;
            line[o + 3] = 255;
        }
    }
    for x in 0..width {
        for y in 0..height {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for d in 0..=radius {
                let i = (y.saturating_add(d).min(height - 1) * width + x) * 4;
                r += line[i] as u32;
                g += line[i + 1] as u32;
                b += line[i + 2] as u32;
                n += 1;
            }
            for d in 1..=radius {
                let i = (y.saturating_sub(d) * width + x) * 4;
                r += line[i] as u32;
                g += line[i + 1] as u32;
                b += line[i + 2] as u32;
                n += 1;
            }
            let o = (y * width + x) * 4;
            rgba[o] = (r / n) as u8;
            rgba[o + 1] = (g / n) as u8;
            rgba[o + 2] = (b / n) as u8;
            rgba[o + 3] = 255;
        }
    }
}

/// GDI 的 32bpp 是 BGRA,页面要 RGBA;顺带把 alpha 钉成不透明。
fn bgra_to_rgba(src: &[u8]) -> Vec<u8> {
    let mut out = vec![255u8; src.len()];
    for (i, chunk) in src.chunks_exact(4).enumerate() {
        out[i * 4] = chunk[2];
        out[i * 4 + 1] = chunk[1];
        out[i * 4 + 2] = chunk[0];
        out[i * 4 + 3] = 255;
    }
    out
}

fn encode_png_data_uri(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    let mut uri = String::with_capacity(bytes.len() * 4 / 3 + 32);
    uri.push_str("data:image/png;base64,");
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        uri.push(TABLE[(n >> 18) as usize & 63] as char);
        uri.push(TABLE[(n >> 12) as usize & 63] as char);
        uri.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        uri.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    Some(uri)
}