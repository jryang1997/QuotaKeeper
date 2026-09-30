//! GNU builds link WebView2Loader.dll dynamically. Keep the loader beside the exe.

use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=frontend/dist");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu")
    {
        return Ok(());
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("Cargo OUT_DIR"));
    let profile_dir = out.ancestors().nth(3).expect("Cargo profile directory");
    let build_dir = profile_dir.join("build");
    let loader = std::fs::read_dir(&build_dir)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("webview2-com-sys-")
        })
        .map(|entry| entry.path().join("out/x64/WebView2Loader.dll"))
        .find(|path| path.is_file())
        .expect("webview2-com-sys x64 loader; only x86_64-pc-windows-gnu is supported here");
    println!("cargo:rerun-if-changed={}", loader.display());
    std::fs::copy(loader, profile_dir.join("WebView2Loader.dll"))?;
    Ok(())
}
