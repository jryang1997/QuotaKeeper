@echo off
chcp 65001 >nul
cd /d "%~dp0"

rem 一键启动入口:桌面快捷方式、直接双击本文件都可以。
rem 不在 Windows Terminal 里就先交给 WT 重开(只有 WT 会套用 Sarasa Term SC 字体),
rem nowt 参数是防套娃标记。
if defined WT_SESSION goto run
if /i "%~1"=="nowt" goto run
set "WT=%LOCALAPPDATA%\Microsoft\WindowsApps\wt.exe"
if not exist "%WT%" set "WT=wt.exe"
start "" "%WT%" -d "%CD%" cmd /c ""%~f0" nowt"
exit /b 0

:run
rem 正常退出(q)= 窗口自动关闭;异常退出 = 暂停,方便看清报错。
set "EXE=target\x86_64-pc-windows-gnu\debug\tui-token-monitor.exe"
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
