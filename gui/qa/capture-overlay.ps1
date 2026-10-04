# 悬浮窗桌面区域截图:验证 DWM Acrylic 背板(CDP 截不到窗外的合成结果)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class Win32Enum {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lparam);
  [DllImport("user32.dll")] public static extern int GetWindowTextW(IntPtr hwnd, [MarshalAs(UnmanagedType.LPWStr)] StringBuilder sb, int max);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L, T, R, B; }
}
'@
[Win32Enum]::SetProcessDPIAware() | Out-Null
$found = [IntPtr]::Zero
$rect = New-Object Win32Enum+RECT
$cb = [Win32Enum+EnumProc]{
  param($hwnd, $lparam)
  $sb = New-Object System.Text.StringBuilder 256
  [Win32Enum]::GetWindowTextW($hwnd, $sb, 256) | Out-Null
  if ($sb.ToString() -eq 'Token Monitor 悬浮窗' -and [Win32Enum]::IsWindowVisible($hwnd)) {
    $script:found = $hwnd
    [Win32Enum]::GetWindowRect($hwnd, [ref]$script:rect) | Out-Null
    return $false
  }
  return $true
}
[Win32Enum]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($found -eq [IntPtr]::Zero) { Write-Output 'WINDOW-NOT-FOUND'; exit 1 }
$L = $rect.L; $T = $rect.T; $W = $rect.R - $rect.L; $H = $rect.B - $rect.T
Write-Output "RECT $L $T $W $H"
$bmp = New-Object System.Drawing.Bitmap($W, $H)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($L, $T, 0, 0, $bmp.Size)
$out = 'C:/Users/Jerry/Desktop/tui_token_moniter/gui/qa/shots/glass/ov-desktop.png'
$bmp.Save($out)
Write-Output "SAVED $out"
