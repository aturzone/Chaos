# Click things in a running Chaos window, and say how long each click took.
#
# **Why this exists.** Atur: *"you must run app and check every function and
# work and waite check it is really work"*. Several things shipped that a
# single run would have caught, and "I looked at it" is not a record. This
# drives the real window through real Win32 messages and prints what happened,
# so a run-through is a transcript rather than a memory.
#
# It also measures. `SendMessageW` is synchronous: it does not return until the
# window's message loop has finished handling it, so the time it takes *is* the
# time the UI thread was blocked. That is exactly the number behind "installed
# models load with lag".
#
#   .\scripts\poke-app.ps1 -Ids 201,202,201,202     switch tabs four times
#   .\scripts\poke-app.ps1 -List                    every control the window has
#
# Two traps already paid for, recorded here so they are not paid again:
#   * `GetWindowText` cannot read an EDIT's contents across processes. It
#     returns captions only, so an empty result proves nothing about a text box.
#   * Sending to a null handle succeeds silently. A run that reports 25 rounds
#     survived against `hwnd = 0` has tested nothing. This refuses to start
#     without a real handle, and prints it.

param(
    [int[]] $Ids = @(),
    [int]   $Repeat = 1,
    [switch] $List,
    [switch] $Layout,
    [int[]] $Combo = @(),
    [int]   $SettleMs = 250
)

$ErrorActionPreference = 'Stop'

Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Poke {
    // **The title is an IntPtr, not a string.** PowerShell marshals a $null
    // string argument as an empty string, so FindWindowW(class, "") looks for
    // a window whose caption is empty, finds none, and reports "not running"
    // about a window plainly on screen. IntPtr.Zero is the real NULL.
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern IntPtr FindWindowW(string cls, IntPtr title);
    [DllImport("user32.dll")]
    public static extern IntPtr GetDlgItem(IntPtr hwnd, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
    [DllImport("user32.dll")]
    public static extern bool IsWindowEnabled(IntPtr hwnd);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetClassNameW(IntPtr hwnd, StringBuilder buf, int max);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder buf, int max);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll")]
    public static extern bool GetClientRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll")]
    public static extern bool ScreenToClient(IntPtr hwnd, ref POINT p);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int X, Y; }
    public static string ClassOf(IntPtr h) {
        var sb = new StringBuilder(256); GetClassNameW(h, sb, 256); return sb.ToString();
    }
    public static string TextOf(IntPtr h) {
        var sb = new StringBuilder(512); GetWindowTextW(h, sb, 512); return sb.ToString();
    }
}
'@

$hwnd = [Poke]::FindWindowW('ChaosAppWindow', [IntPtr]::Zero)
if ($hwnd -eq [IntPtr]::Zero) {
    Write-Error 'No Chaos window found. Start target\release\chaos-app.exe first.'
    exit 1
}
"window {0} (0x{0:X})" -f [int64]$hwnd | Write-Host

$WM_COMMAND = 0x0111
$BN_CLICKED = 0

if ($List) {
    # Every id the window declares, so a run-through can be exhaustive rather
    # than a list somebody remembered. 100..999 covers nav.rs's whole range.
    foreach ($id in 100..999) {
        $c = [Poke]::GetDlgItem($hwnd, $id)
        if ($c -ne [IntPtr]::Zero) {
            $cls = [Poke]::ClassOf($c)
            $txt = [Poke]::TextOf($c)
            $vis = if ([Poke]::IsWindowVisible($c)) { 'shown' } else { 'HIDDEN' }
            $en  = if ([Poke]::IsWindowEnabled($c)) { 'on ' } else { 'off' }
            "{0,4}  {1,-16} {2,-6} {3}  {4}" -f $id, $cls, $vis, $en, $txt | Write-Host
        }
    }
    exit 0
}

if ($Layout) {
    # Where every visible control actually sits, and whether any two of them
    # are on top of each other.
    #
    # **This is the check a screenshot would have been.** A screen grab is
    # black in a session with no composited display, and a page whose two
    # labels overlap is exactly the failure a picture catches and a click test
    # does not. Rectangles cross the process boundary as integers, so this
    # works where pixels do not.
    $client = New-Object Poke+RECT
    [Poke]::GetClientRect($hwnd, [ref]$client) | Out-Null
    "client area {0} x {1}" -f $client.Right, $client.Bottom | Write-Host
    ''

    $boxes = @()
    foreach ($id in 100..999) {
        $c = [Poke]::GetDlgItem($hwnd, $id)
        if ($c -eq [IntPtr]::Zero) { continue }
        if (-not [Poke]::IsWindowVisible($c)) { continue }
        $r = New-Object Poke+RECT
        [Poke]::GetWindowRect($c, [ref]$r) | Out-Null
        $tl = New-Object Poke+POINT; $tl.X = $r.Left;  $tl.Y = $r.Top
        $br = New-Object Poke+POINT; $br.X = $r.Right; $br.Y = $r.Bottom
        [Poke]::ScreenToClient($hwnd, [ref]$tl) | Out-Null
        [Poke]::ScreenToClient($hwnd, [ref]$br) | Out-Null
        $boxes += [pscustomobject]@{
            Id = $id; Text = [Poke]::TextOf($c)
            L = $tl.X; T = $tl.Y; R = $br.X; B = $br.Y
        }
    }

    foreach ($b in $boxes) {
        $off = ''
        if ($b.L -lt 0 -or $b.T -lt 0 -or $b.R -gt $client.Right -or $b.B -gt $client.Bottom) {
            $off = '  <-- OUTSIDE THE PAGE'
        }
        "{0,4}  ({1,4},{2,4}) {3,4}x{4,-4}  {5}{6}" -f `
            $b.Id, $b.L, $b.T, ($b.R - $b.L), ($b.B - $b.T), $b.Text, $off | Write-Host
    }

    ''
    $clashes = 0
    for ($i = 0; $i -lt $boxes.Count; $i++) {
        for ($j = $i + 1; $j -lt $boxes.Count; $j++) {
            $a = $boxes[$i]; $b = $boxes[$j]
            $w = [Math]::Min($a.R, $b.R) - [Math]::Max($a.L, $b.L)
            $h = [Math]::Min($a.B, $b.B) - [Math]::Max($a.T, $b.T)
            if ($w -gt 0 -and $h -gt 0) {
                $clashes++
                "OVERLAP  {0} and {1}  -- {2}x{3} px" -f $a.Id, $b.Id, $w, $h | Write-Host
            }
        }
    }
    if ($clashes -eq 0) { "{0} visible controls, none overlapping" -f $boxes.Count | Write-Host }
    exit 0
}

if ($Combo.Count -gt 0) {
    # How many rows a drop-down holds, and which is picked.
    #
    # **Counts, not text.** `CB_GETLBTEXT` writes into a buffer in the *asking*
    # process, and the window is another one, so a cross-process read returns
    # nothing whatever the control contains -- the same trap that nearly had
    # the IMAGE log reported as broken. A count and a selection are integers
    # returned in the message result, so they cross the boundary honestly.
    $CB_GETCOUNT  = 0x0146
    $CB_GETCURSEL = 0x0147
    foreach ($id in $Combo) {
        $c = [Poke]::GetDlgItem($hwnd, $id)
        if ($c -eq [IntPtr]::Zero) { "{0,4}  -- no such control" -f $id | Write-Host; continue }
        $n   = [int][Poke]::SendMessageW($c, $CB_GETCOUNT,  [IntPtr]::Zero, [IntPtr]::Zero)
        $sel = [int][Poke]::SendMessageW($c, $CB_GETCURSEL, [IntPtr]::Zero, [IntPtr]::Zero)
        $note = if ($n -le 0) { '  <-- EMPTY' } elseif ($sel -lt 0) { '  <-- nothing selected' } else { '' }
        "{0,4}  {1} rows, selection {2}{3}" -f $id, $n, $sel, $note | Write-Host
    }
    exit 0
}

if ($Ids.Count -eq 0) {
    Write-Host 'Nothing to click. Pass -Ids, or -List to see what there is.'
    exit 0
}

$worst = 0.0
for ($r = 1; $r -le $Repeat; $r++) {
    foreach ($id in $Ids) {
        $c = [Poke]::GetDlgItem($hwnd, $id)
        if ($c -eq [IntPtr]::Zero) {
            "{0,4}  -- no such control" -f $id | Write-Host
            continue
        }
        $label = [Poke]::TextOf($c)
        $wp = [IntPtr](($BN_CLICKED -shl 16) -bor ($id -band 0xFFFF))
        # Synchronous on purpose: what this measures is the UI thread's stall.
        $t = Measure-Command { [Poke]::SendMessageW($hwnd, $WM_COMMAND, $wp, $c) | Out-Null }
        $ms = $t.TotalMilliseconds
        if ($ms -gt $worst) { $worst = $ms }
        $flag = if ($ms -gt 200) { '  <-- STALL' } else { '' }
        "{0,4}  {1,-22} {2,8:N1} ms{3}" -f $id, $label, $ms, $flag | Write-Host
        Start-Sleep -Milliseconds $SettleMs
    }
}
''
"worst blocking call: {0:N1} ms" -f $worst | Write-Host
if ($worst -gt 200) {
    Write-Host 'A click over 200 ms is a window that looks frozen.'
}
