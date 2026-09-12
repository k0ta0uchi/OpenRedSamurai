param([string]$OutPath = "ui_midnight_linear.png")

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class WinUtil {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBkgnd, uint nFlags);
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$proc = Get-Process -Name "redsamurai-config" -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Output "PROCESS_NOT_FOUND"; exit 1 }

$targetHwnd = [IntPtr]::Zero
$targetRect = New-Object WinUtil+RECT

[WinUtil]::EnumWindows({
    param($hwnd, $lparam)
    $wpid = [uint32]0
    [WinUtil]::GetWindowThreadProcessId($hwnd, [ref]$wpid) | Out-Null
    if ($wpid -eq $proc.Id -and [WinUtil]::IsWindowVisible($hwnd)) {
        $r = New-Object WinUtil+RECT
        [WinUtil]::GetWindowRect($hwnd, [ref]$r) | Out-Null
        $w = $r.Right - $r.Left
        $h = $r.Bottom - $r.Top
        if ($w -gt 400 -and $h -gt 300) {
            $script:targetHwnd = $hwnd
            $script:targetRect = $r
            return $false
        }
    }
    return $true
}, [IntPtr]::Zero) | Out-Null

if ($targetHwnd -eq [IntPtr]::Zero) {
    Write-Output "MAIN_WINDOW_NOT_FOUND"
    exit 1
}

[WinUtil]::SetForegroundWindow($targetHwnd) | Out-Null
Start-Sleep -Milliseconds 700

[WinUtil]::GetWindowRect($targetHwnd, [ref]$targetRect) | Out-Null
$w = $targetRect.Right - $targetRect.Left
$h = $targetRect.Bottom - $targetRect.Top
Write-Output ("Window found: {0}x{1} at ({2},{3})" -f $w, $h, $targetRect.Left, $targetRect.Top)

$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($targetRect.Left, $targetRect.Top, 0, 0, $bmp.Size)
$bmp.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Output "saved: $OutPath"
