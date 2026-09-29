@echo off
chcp 65001 >nul
cd /d "%~dp0"

rem 一键启动入口:桌面快捷方式、直接双击本文件都可以。
rem 不在 Windows Terminal 里就先交给 WT 重开(只有 WT 会套用 Sarasa Term SC 字体),
rem nowt 参数是防套娃标记;WT 重开失败(别名在部分会话下静默失败)则 5 秒后
rem 兜底前台运行,任何环境都能出界面。
if defined WT_SESSION goto run
if /i "%~1"=="nowt" goto run
set "WT=%LOCALAPPDATA%\Microsoft\WindowsApps\wt.exe"
if not exist "%WT%" set "WT=wt.exe"
start "" "%WT%" -d "%CD%" cmd /c ""%~f0" nowt"
rem 兜底:等 5 秒,TUI 没起来就自己跑(conhost 下字体回退,但功能完整)
timeout /t 5 /nobreak >nul
tasklist | findstr /i "tui-token-monitor.exe" >nul
if errorlevel 1 goto run
exit /b 0

:run
rem 正常退出(q)= 窗口自动关闭;异常退出 = 暂停,方便看清报错。
rem 优先 GNU 固化构建产物,其次 MSVC 默认路径。
set "EXE=target\x86_64-pc-windows-gnu\debug\tui-token-monitor.exe"
if not exist "%EXE%" set "EXE=target\debug\tui-token-monitor.exe"
if not exist "%EXE%" (
  echo [!] 未找到 %EXE%
  echo     先在项目目录执行:cargo build
  pause
  exit /b 1
)
"%EXE%"
set "RC=%errorlevel%"
if not "%RC%"=="0" (
  echo.
  echo [!] 程序异常退出,退出码 %RC%。按任意键关闭窗口。
  pause >nul
)
exit /b %RC%
