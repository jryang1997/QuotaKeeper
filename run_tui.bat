@echo off
chcp 65001 >nul
cd /d "%~dp0"

rem One-click launcher: desktop shortcut or double-click.
rem Re-launch inside Windows Terminal (WT applies the Sarasa Term SC font);
rem "nowt" guards against re-launch loops. If WT fails, fall back to
rem running in-place after 5s (conhost falls back on fonts, still works).
rem NOTE: keep every comment ASCII-only. cmd re-seeks batch files by byte
rem offset; multi-byte UTF-8 here randomly splits lines into bogus commands.

if defined WT_SESSION goto run
if /i "%~1"=="nowt" goto run
set "WT=%LOCALAPPDATA%\Microsoft\WindowsApps\wt.exe"
if not exist "%WT%" set "WT=wt.exe"
start "" "%WT%" -d "%CD%" cmd /c ""%~f0" nowt"
timeout /t 5 /nobreak >nul
tasklist | findstr /i "tui-token-monitor.exe" >nul
if errorlevel 1 goto run
exit /b 0

:run
rem Normal exit (q) closes the window; non-zero exit pauses to show the error.
rem Prefer the pinned GNU build, then the plain (MSVC) default path.
set "EXE=target\x86_64-pc-windows-gnu\debug\tui-token-monitor.exe"
if not exist "%EXE%" set "EXE=target\debug\tui-token-monitor.exe"
if not exist "%EXE%" (
  echo [!] %EXE% not found. Run: cargo build
  pause
  exit /b 1
)
"%EXE%"
set "RC=%errorlevel%"
if not "%RC%"=="0" (
  echo.
  echo [!] Exited with code %RC%. Press any key to close.
  pause >nul
)
exit /b %RC%
