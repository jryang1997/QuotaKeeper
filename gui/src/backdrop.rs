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
    // 模糊放在 Rust 侧:页面对伪元素做 backdrop-filter 只会糊掉伪元素自己,
    // 不会处理它带的背景图,所以毛玻璃的 blur 必须在送进页面前完成。
    // 先 2×2 均值降采样(重度模糊下无视觉损失,PNG 编码与 base64 小 4 倍),
    // 再提饱和(iOS 玻璃的关键质感),最后三遍滑窗盒式模糊逼近高斯。
    let (small, sw, sh) = downscale_2x(&pixels, width, height);
    let mut rgba = bgra_to_rgba(&small);
    saturate(&mut rgba, SATURATE);
    let hash = frame_hash(&rgba);
    if !FIRST_FRAME.load(Ordering::Relaxed) && LAST_HASH.swap(hash, Ordering::Relaxed) == hash {
        return None; // 桌面没变:跳过编码与脚本求值,调用方保持上一帧
    }
    FIRST_FRAME.store(false, Ordering::Relaxed);
    box_blur(&mut rgba, sw, sh, BLUR_RADIUS);
    encode_png_data_uri(&rgba, sw as u32, sh as u32)
}

/// 降采样倍率:2×2 均值池化。
const DOWNSCALE: usize = 2;
/// 毛玻璃模糊半径(降采样后像素):三遍盒式模糊的等效高斯远比单遍 18px 柔,
/// 桌面文字完全读不出,但保留大色块、光带与明暗。
const BLUR_RADIUS: usize = 9;
/// 饱和度提升:iOS 玻璃材料的标志 —— 糊掉细节的同时把底下的颜色"喂"回来。
const SATURATE: f32 = 1.38;

static FIRST_FRAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static LAST_HASH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// FNV-1a:同尺寸同内容的帧判等,静止时省掉整条编码链。
fn frame_hash(rgba: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in rgba {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 2×2 均值池化(BGRA 输入),奇数尺寸贴边取样。
fn downscale_2x(bgra: &[u8], width: u32, height: u32) -> (Vec<u8>, usize, usize) {
    let dw = width.div_ceil(DOWNSCALE as u32) as usize;
    let dh = height.div_ceil(DOWNSCALE as u32) as usize;
    let mut out = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        for x in 0..dw {
            let (mut b, mut g, mut r, mut n) = (0u32, 0u32, 0u32, 0u32);
            for dy in 0..DOWNSCALE {
                let sy = (y * DOWNSCALE + dy).min(height as usize - 1);
                for dx in 0..DOWNSCALE {
                    let sx = (x * DOWNSCALE + dx).min(width as usize - 1);
                    let i = (sy * width as usize + sx) * 4;
                    b += bgra[i] as u32;
                    g += bgra[i + 1] as u32;
                    r += bgra[i + 2] as u32;
                    n += 1;
                }
            }
            let o = (y * dw + x) * 4;
            out[o] = (b / n) as u8;
            out[o + 1] = (g / n) as u8;
            out[o + 2] = (r / n) as u8;
        }
    }
    (out, dw, dh)
}

/// 饱和度提升:c' = luma + (c - luma) × factor。
fn saturate(rgba: &mut [u8], factor: f32) {
    for px in rgba.chunks_exact_mut(4) {
        let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let mix = |c: f32| -> u8 { (luma + (c - luma) * factor).round().clamp(0.0, 255.0) as u8 };
        px[0] = mix(r);
        px[1] = mix(g);
        px[2] = mix(b);
    }
}

/// 可分离盒式模糊:横竖一遍 O(1) 滑动窗口均值,重复 BLUR_PASSES 次逼近高斯。
/// 半径 9 降采样像素在胶囊尺寸下亚毫秒级,拉到最大尺寸也只有几毫秒。
fn box_blur(rgba: &mut Vec<u8>, width: usize, height: usize, radius: usize) {
    const BLUR_PASSES: usize = 3;
    if radius == 0 || width < 3 || height < 3 {
        return;
    }
    let mut scratch = vec![0u8; rgba.len()];
    for _ in 0..BLUR_PASSES {
        blur_axis(rgba, &mut scratch, width, height, radius, true);
        blur_axis(&mut scratch, rgba, width, height, radius, false);
    }
}

/// 单方向滑动窗口均值:窗口左右边界各滑一步,均摊 O(1);
/// 窗口截断到图内(不延拓),避免边缘读出黑边或重复计数。
/// 源与目标同为行主序,相邻像素步长 horizontal ? 4 : width*4,两侧一致。
fn blur_axis(src: &[u8], dst: &mut [u8], width: usize, height: usize, radius: usize, horizontal: bool) {
    let (outer, inner, stride, base_step) = if horizontal {
        (height, width, 4usize, width * 4)
    } else {
        (width, height, width * 4, 4usize)
    };
    for o in 0..outer {
        let base = o * base_step;
        let mut lo = 0usize;
        let mut hi = radius.min(inner - 1);
        let (mut sr, mut sg, mut sb) = (0u32, 0u32, 0u32);
        for i in lo..=hi {
            let idx = base + i * stride;
            sr += src[idx] as u32;
            sg += src[idx + 1] as u32;
            sb += src[idx + 2] as u32;
        }
        for i in 0..inner {
            let n = (hi - lo + 1) as u32;
            let out_idx = base + i * stride;
            dst[out_idx] = (sr / n) as u8;
            dst[out_idx + 1] = (sg / n) as u8;
            dst[out_idx + 2] = (sb / n) as u8;
            dst[out_idx + 3] = 255;
            // 滑向 i+1:右缘进入一格(未到图右界时),左缘离开一格(越过 radius 时)
            let next_hi = (i + radius + 1).min(inner - 1);
            if next_hi > hi {
                hi = next_hi;
                let idx = base + hi * stride;
                sr += src[idx] as u32;
                sg += src[idx + 1] as u32;
                sb += src[idx + 2] as u32;
            }
            let next_lo = (i + 1).saturating_sub(radius);
            if next_lo > lo {
                for k in lo..next_lo {
                    let idx = base + k * stride;
                    sr -= src[idx] as u32;
                    sg -= src[idx + 1] as u32;
                    sb -= src[idx + 2] as u32;
                }
                lo = next_lo;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downscale_averages_each_2x2_block() {
        // 4x2 全红 BGRA → 2x1,均值仍是纯红
        let mut src = vec![0u8; 4 * 2 * 4];
        for px in src.chunks_exact_mut(4) {
            px.copy_from_slice(&[0, 0, 200, 255]); // BGRA: B=0 G=0 R=200
        }
        let (out, w, h) = downscale_2x(&src, 4, 2);
        assert_eq!((w, h), (2, 1));
        assert_eq!(out[0], 0);
        assert_eq!(out[2], 200);
    }

    #[test]
    fn box_blur_uniform_field_stays_uniform() {
        // 均匀场模糊后必须恒等(滑动窗口边界不引入暗角/黑边)
        let mut rgba = vec![128u8; 20 * 12 * 4];
        box_blur(&mut rgba, 20, 12, 9);
        assert!(rgba.chunks_exact(4).all(|px| px[0] == 128 && px[3] == 255));
    }

    #[test]
    fn box_blur_keeps_extreme_row_means() {
        // 上半白下半黑:模糊后上半均值应显著高于下半(能量守恒,不许整体漂移)
        let (w, h) = (8usize, 8usize);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h / 2 {
            for x in 0..w {
                let i = (y * w + x) * 4;
                rgba[i] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
            }
        }
        box_blur(&mut rgba, w, h, 3);
        let mean = |start: usize, end: usize| {
            (start..end)
                .map(|y| (0..w).map(|x| rgba[(y * w + x) * 4] as u64).sum::<u64>())
                .sum::<u64>() as f64
                / ((end - start) * w) as f64
        };
        assert!(mean(0, 2) > mean(6, 8) * 1.5); // 上半仍显著亮于下半(半径3×8行的扩散幅度)
    }

    #[test]
    fn saturate_pushes_chroma_without_touching_gray() {
        let mut px = [100, 100, 100, 255, 200, 100, 100, 255];
        saturate(&mut px, 1.38);
        assert_eq!(px[0], 100); // 灰色不动
        assert!(px[4] > 200); // 高于 luma 的通道被推高
        assert!(px[5] < 100); // 低于 luma 的通道被压低
    }

    #[test]
    fn frame_hash_distinguishes_content() {
        assert_ne!(frame_hash(&[1, 2, 3]), frame_hash(&[1, 2, 4]));
        assert_eq!(frame_hash(&[9, 9]), frame_hash(&[9, 9]));
    }
}