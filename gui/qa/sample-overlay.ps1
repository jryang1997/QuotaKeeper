# 悬浮窗客户区边缘像素取样:定位白色矩形来自哪一层(父窗表面/webview/页面)。
# 用法: powershell -File sample-overlay.ps1 [-Strip] [-Out <path>]
param([switch]$Strip, [string]$Out = '')
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
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  public struct RECT { public int L, T, R, B; }
  public struct POINT { public int X, Y; }
}
'@
[W]::SetProcessDPIAware() | Out-Null
$found = [IntPtr]::Zero
$rect = New-Object W+RECT
$crect = New-Object W+RECT
$cb = [W+EnumProc]{
  param($hwnd, $lparam)
  $sb = New-Object System.Text.StringBuilder 256
  [W]::GetWindowTextW($hwnd, $sb, 256) | Out-Null
  if ($sb.ToString() -eq 'Token Monitor 悬浮窗' -and [W]::IsWindowVisible($hwnd)) {
    $script:found = $hwnd
    [W]::GetWindowRect($hwnd, [ref]$script:rect) | Out-Null
    [W]::GetClientRect($hwnd, [ref]$crect) | Out-Null
    return $false
  }
  return $true
}
[W]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($found -eq [IntPtr]::Zero) { Write-Output 'WINDOW-NOT-FOUND'; exit 1 }
$pt = New-Object W+POINT
$pt.X = 0; $pt.Y = 0
[W]::ClientToScreen($found, [ref]$pt) | Out-Null
$ww = $rect.R - $rect.L; $wh = $rect.B - $rect.T
$cw = $crect.R - $crect.L; $ch = $crect.B - $crect.T
Write-Output ("win {0}x{1} client {2}x{3} clientOrigin ({4},{5})" -f $ww, $wh, $cw, $ch, $pt.X, $pt.Y)
$m = 10
$L = $rect.L - $m; $T = $rect.T - $m
$bw = $ww + (2 * $m); $bh = $wh + (2 * $m)
$bmp = New-Object System.Drawing.Bitmap($bw, $bh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($L, $T, 0, 0, $bmp.Size)
if ($Out -ne '') { $bmp.Save($Out); Write-Output "SAVED $Out" }
$clx = $pt.X - $L; $cly = $pt.Y - $T
$midy = $cly + [int]($ch / 2)
if ($Strip) {
  $row = @()
  for ($x = $clx - 6; $x -le $clx + 36; $x += 3) {
    $c = $bmp.GetPixel($x, $midy)
    $row += ("{0}:({1},{2},{3})" -f ($x - $clx), $c.R, $c.G, $c.B)
  }
  Write-Output ('strip-from-clientLeft ' + ($row -join ' '))
}
Write-Output ("client-corner(+3,+3): " + $bmp.GetPixel($clx + 3, $cly + 3))
Write-Output ("client-mid-left-band(+2,mid): " + $bmp.GetPixel($clx + 2, $midy))
$bmp.Dispose()
