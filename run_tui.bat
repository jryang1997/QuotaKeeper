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
rem Register the dedicated profile (Sarasa Term SC) via a WT fragment,
rem best effort. WT reads fragments at startup, so the very first run
rem may need one WT restart before the profile is registered.
set "FRAGDIR=%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\tui-token-monitor"
if exist "terminal\tui-token-monitor.json" (
  mkdir "%FRAGDIR%" >nul 2>&1
  copy /Y "terminal\tui-token-monitor.json" "%FRAGDIR%\" >nul 2>&1
)
set "PARG="
if exist "%FRAGDIR%\tui-token-monitor.json" set "PARG=-p tui-token-monitor"
start "" "%WT%" -d "%CD%" %PARG% cmd /c ""%~f0" nowt"
timeout /t 5 /nobreak >nul
tasklist | findstr /i "tui-token-monitor.exe" >nul
if not errorlevel 1 exit /b 0
rem Profile not registered yet (WT was already running): retry plain.
if defined PARG (
  start "" "%WT%" -d "%CD%" cmd /c ""%~f0" nowt"
  timeout /t 5 /nobreak >nul
  tasklist | findstr /i "tui-token-monitor.exe" >nul
  if not errorlevel 1 exit /b 0
)
goto run

:run
rem Normal exit (q) closes the window; non-zero exit pauses to show the error.
rem Debug builds win so the dev loop (cargo build) is always what launches;
rem release is the fallback for README users who ran cargo build --release.
set "EXE=target\x86_64-pc-windows-gnu\debug\tui-token-monitor.exe"
if not exist "%EXE%" set "EXE=target\debug\tui-token-monitor.exe"
if not exist "%EXE%" set "EXE=target\x86_64-pc-windows-gnu\release\tui-token-monitor.exe"
if not exist "%EXE%" set "EXE=target\release\tui-token-monitor.exe"
if not exist "%EXE%" (
  echo [!] %EXE% not found. Run: cargo build  (or: cargo build --release)
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
