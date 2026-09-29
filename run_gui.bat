@echo off
chcp 65001 >nul
cd /d "%~dp0"

rem GUI 入口:桌面快捷方式请指向本文件(与 run_tui.bat 同规矩)。
rem 注意:本文件必须保持 CRLF 行尾、避免在 if/for 块里用带管道的 for /f
rem (msys/conhost 解析差异会挂死),DLL 缺失的兜底放在 Rust 侧完成。
rem 优先 GNU 固化构建产物,其次 MSVC 默认路径。
set "EXE=gui\target\x86_64-pc-windows-gnu\release\tui-token-monitor-gui.exe"
if not exist "%EXE%" set "EXE=gui\target\release\tui-token-monitor-gui.exe"
if not exist "%EXE%" (
  echo [!] 未找到 %EXE%
  echo     先构建:cd gui 再 cargo build --release
  pause
  exit /b 1
)
if not exist "gui\target\x86_64-pc-windows-gnu\release\WebView2Loader.dll" if not exist "gui\target\release\WebView2Loader.dll" (
  echo [!] 缺少 WebView2Loader.dll，请重新运行 GUI Release 构建。
  pause
  exit /b 1
)
start "" "%EXE%"
exit /b 0
