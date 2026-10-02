# 创建/更新桌面 GUI 直启快捷方式.
#
# 直指 exe(无控制台闪窗,图标取自 exe 内嵌资源);dist 已 include 进 exe,
# 唯一外部依赖 WebView2Loader.dll 由 build.rs 自动复制到 exe 旁.
# 幂等:重复运行即覆盖更新.回退路径与错误提示见根目录 run_gui.bat.
#
# 用法:powershell -NoProfile -ExecutionPolicy Bypass -File scripts\new-gui-shortcut.ps1

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$candidates = @(
    (Join-Path $root 'gui\target\x86_64-pc-windows-gnu\release\tui-token-monitor-gui.exe'),
    (Join-Path $root 'gui\target\release\tui-token-monitor-gui.exe')
)
$exe = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $exe) {
    Write-Error "未找到 GUI exe;先构建: cd gui; cargo build --release"
    exit 1
}
$exe = (Resolve-Path $exe).Path

$desktop = [Environment]::GetFolderPath('Desktop')
$lnkPath = Join-Path $desktop 'TUI Token Monitor.lnk'
$shell = New-Object -ComObject WScript.Shell
$lnk = $shell.CreateShortcut($lnkPath)
$lnk.TargetPath = $exe
$lnk.WorkingDirectory = Split-Path -Parent $exe
$lnk.Description = 'TUI Token Monitor (GUI)'
$lnk.Save()

# 读回验证
$check = $shell.CreateShortcut($lnkPath)
Write-Host ("created " + $lnkPath)
Write-Host ("  target   " + $check.TargetPath)
Write-Host ("  workdir  " + $check.WorkingDirectory)
if ($check.TargetPath -ne $exe -or -not (Test-Path $check.TargetPath)) {
    Write-Error "快捷方式校验失败"
    exit 1
}
