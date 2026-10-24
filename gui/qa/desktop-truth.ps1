# 桌面真相截图:主窗挪开、悬浮窗挪到干净位置后 CopyFromScreen。
# 用法: powershell -File desktop-truth.ps1 [-Out <path>]
param([string]$Out = 'C:/Users/<user>/Desktop/tui_token_moniter/gui/shots/desktop-truth.png')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Text; using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lparam);
  [DllImport("user32.dll")] public static extern int GetWindowTextW(IntPtr hwnd, [MarshalAs(UnmanagedType.LPWStr)] StringBuilder sb, int max);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
  public struct RECT { public int L, T, R, B; }
}
'@
[W]::SetProcessDPIAware() | Out-Null
$overlay = [IntPtr]::Zero
$main = [IntPtr]::Zero
$orect = New-Object W+RECT
$cb = [W+EnumProc]{
  param($hwnd, $lparam)
  $sb = New-Object System.Text.StringBuilder 256
  [W]::GetWindowTextW($hwnd, $sb, 256) | Out-Null
  $t = $sb.ToString()
  if ($t -eq 'Token Monitor 悬浮窗' -and [W]::IsWindowVisible($hwnd)) {
    $script:overlay = $hwnd
    [W]::GetWindowRect($hwnd, [ref]$script:orect) | Out-Null
    return $false
  }
  if ($t -eq 'Token Monitor' -and [W]::IsWindowVisible($hwnd)) { $script:main = $hwnd }
  return $true
}
[W]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($overlay -eq [IntPtr]::Zero) { Write-Output 'OVERLAY-NOT-FOUND'; exit 1 }
if ($main -ne [IntPtr]::Zero) { [W]::MoveWindow($main, 60, 60, 1200, 700, $true) | Out-Null; Write-Output 'MAIN-MOVED' }
# 悬浮窗挪到屏幕中上,避开任务栏与主窗
$ow = $orect.R - $orect.L; $oh = $orect.B - $orect.T
[W]::MoveWindow($overlay, 1500, 700, $ow, $oh, $true) | Out-Null
Start-Sleep -Milliseconds 900
$rect = New-Object W+RECT
[W]::GetWindowRect($overlay, [ref]$rect) | Out-Null
$m = 30
$L = $rect.L - $m; $T = $rect.T - $m
$bw = ($rect.R - $rect.L) + (2 * $m); $bh = ($rect.B - $rect.T) + (2 * $m)
$bmp = New-Object System.Drawing.Bitmap($bw, $bh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($L, $T, 0, 0, $bmp.Size)
$bmp.Save($Out)
$bmp.Dispose()
Write-Output "SAVED $Out (overlay at $($rect.L),$($rect.T))"
