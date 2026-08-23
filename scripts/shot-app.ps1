# Photograph the running Chaos window.
#
# Atur: *"you must run app and check every function and work and waite check it
# is really work"*. Clicking a control proves it responds; only a picture proves
# the page is not two labels on top of each other. This writes a PNG so a
# layout can be looked at rather than reasoned about.
#
#   .\scripts\shot-app.ps1 -Out shot.png
#
# **A screen grab, not `PrintWindow`.** `PrintWindow` asks a window to render
# itself into a bitmap, which would be tidier -- it works behind other windows
# and photographs nothing else. It returns a solid black rectangle here, and
# reports success while doing it. This window answers `WM_ERASEBKGND` itself and
# does all its drawing in `WM_PAINT` through a memory DC; it handles no
# `WM_PRINTCLIENT`, so there is nothing for `PrintWindow` to collect. Rather
# than add a second paint path to the app for the benefit of a script, this
# brings the window to the front and photographs the screen -- which is also
# closer to what the question "does this page look right" actually means.

param(
    [string] $Out = 'chaos-window.png'
)

# **This needs a real display.** In a session with no composited output -- a
# disconnected RDP session, or an agent running with a window station but no
# screen -- the grab succeeds and every pixel is black. If the result has one
# distinct colour, that is what happened; use `poke-app.ps1 -Layout` instead,
# which reads control rectangles and finds overlaps without needing pixels.

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Shot {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern IntPtr FindWindowW(string cls, IntPtr title);
    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hwnd, int cmd);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

$hwnd = [Shot]::FindWindowW('ChaosAppWindow', [IntPtr]::Zero)
if ($hwnd -eq [IntPtr]::Zero) {
    Write-Error 'No Chaos window found. Start target\release\chaos-app.exe first.'
    exit 1
}

# SW_SHOW, then foreground: a window hidden in the notification area has no
# client area to render, and a capture of it is a blank rectangle that looks
# exactly like a broken page.
[Shot]::ShowWindow($hwnd, 5) | Out-Null
[Shot]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 400

$r = New-Object Shot+RECT
[Shot]::GetWindowRect($hwnd, [ref]$r) | Out-Null
$w = $r.Right - $r.Left
$h = $r.Bottom - $r.Top
if ($w -le 0 -or $h -le 0) { Write-Error "window has no size ($w x $h)"; exit 1 }

$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
$g.Dispose()
if ([System.IO.Path]::IsPathRooted($Out)) {
    $full = $Out
} else {
    $full = Join-Path (Get-Location).Path $Out
}
$bmp.Save($full, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"wrote $full -- ${w}x${h}" | Write-Host
