# Click every control in the Chaos window, and write down what happened.
#
# Atur: *"you must run app and check every function and work and waite check it
# is really work"*. Fair, and earned: several things shipped in one week that a
# single run would have caught. This is that run, as a script, so it is a
# transcript rather than a memory and so it can be repeated before every
# release.
#
#   .\scripts\run-through.ps1
#
# # What it will not press
#
# **Four controls are destructive and are listed rather than clicked.** A
# run-through that deleted a 144 GB model, or wiped the user's settings, would
# be a worse bug than anything it could find:
#
#   207  DELETE   removes a model's files from disk
#   311  RESET    discards the saved settings
#   312  BROWSE   opens a MODAL folder dialog -- it blocks the window's message
#                 loop, so the very next SendMessageW never returns and the
#                 script hangs rather than failing
#   310  SAVE     writes settings; harmless in itself, but only meaningful
#                 after a change, and a run-through should not leave one
#
# Two more are *slow* rather than dangerous and are opt-in with -Slow:
#
#   204  LOAD     starts a model; minutes
#   704  DRAW     starts a render; hours at a useful size
#
# Everything else is pressed. `SendMessageW` is synchronous, so the time each
# takes is the time the UI thread was blocked -- anything over 200 ms is a
# window that looks frozen to the person using it.

param(
    [switch] $Slow,
    [int] $SettleMs = 350
)

$ErrorActionPreference = 'Stop'

Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Run {
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
    public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder buf, int max);
    public static string TextOf(IntPtr h) {
        var sb = new StringBuilder(512); GetWindowTextW(h, sb, 512); return sb.ToString();
    }
}
'@

$hwnd = [Run]::FindWindowW('ChaosAppWindow', [IntPtr]::Zero)
if ($hwnd -eq [IntPtr]::Zero) {
    Write-Error 'No Chaos window. Start target\release\chaos-app.exe first.'
    exit 1
}

$WM_COMMAND = 0x0111
$BN_CLICKED = 0
$CB_GETCOUNT = 0x0146
$CB_GETCURSEL = 0x0147
$LB_GETCOUNT = 0x018B

# id -> why it is not pressed.
$skip = @{
    207 = 'DESTRUCTIVE: deletes a model from disk'
    310 = 'skipped: writes settings, and there is no change to write'
    311 = 'DESTRUCTIVE: discards the saved settings'
    312 = 'BLOCKS: opens a modal folder dialog, which stops the message loop'
}
if (-not $Slow) {
    $skip[204] = 'slow: starts a model, minutes. Pass -Slow to include it'
    $skip[704] = 'slow: starts a render, hours. Pass -Slow to include it'
}

$pages = @(
    @{ Id = 401; Name = 'CHAT';     Controls = @(104, 103) }
    @{ Id = 402; Name = 'MODELS';   Controls = @(201, 202, 208, 210, 211, 212, 203, 204, 205, 206, 207, 209) }
    @{ Id = 403; Name = 'MONITOR';  Controls = @() }
    @{ Id = 404; Name = 'SETTINGS'; Controls = @(301, 302, 303, 305, 306, 308, 309, 310, 311, 312) }
    @{ Id = 406; Name = 'IMAGE';    Controls = @(708, 702, 703, 709, 706, 705, 704) }
)

$worst = 0.0
$pressed = 0
$skipped = 0

function Press($id, $label) {
    $c = [Run]::GetDlgItem($hwnd, $id)
    if ($c -eq [IntPtr]::Zero) { return $null }
    $wp = [IntPtr](($BN_CLICKED -shl 16) -bor ($id -band 0xFFFF))
    $t = Measure-Command { [Run]::SendMessageW($hwnd, $WM_COMMAND, $wp, $c) | Out-Null }
    Start-Sleep -Milliseconds $SettleMs
    return $t.TotalMilliseconds
}

"Chaos run-through  --  window $hwnd"
"".PadRight(78, '=')

foreach ($page in $pages) {
    ''
    "## $($page.Name)"
    $ms = Press $page.Id $page.Name
    "  {0,4}  {1,-22} opened  {2,8:N1} ms" -f $page.Id, $page.Name, $ms
    if ($ms -gt $worst) { $worst = $ms }

    foreach ($id in $page.Controls) {
        $c = [Run]::GetDlgItem($hwnd, $id)
        if ($c -eq [IntPtr]::Zero) {
            "  {0,4}  -- no such control" -f $id
            continue
        }
        $label = [Run]::TextOf($c)
        $vis = [Run]::IsWindowVisible($c)
        $en = [Run]::IsWindowEnabled($c)

        if (-not $vis) {
            "  {0,4}  {1,-22} HIDDEN on this page" -f $id, $label
            continue
        }
        if ($skip.ContainsKey($id)) {
            "  {0,4}  {1,-22} {2}" -f $id, $label, $skip[$id]
            $skipped++
            continue
        }
        if (-not $en) {
            # Greyed out is an answer, not a gap: it is what the window says
            # about the current state, and pressing it would prove nothing.
            "  {0,4}  {1,-22} greyed out (correct for this state)" -f $id, $label
            continue
        }

        # A drop-down or a list is read rather than clicked: pressing a
        # ComboBox does not open it through a message, and what matters is
        # whether it holds anything.
        $n = [int][Run]::SendMessageW($c, $CB_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
        if ($n -gt 0) {
            $sel = [int][Run]::SendMessageW($c, $CB_GETCURSEL, [IntPtr]::Zero, [IntPtr]::Zero)
            $note = if ($sel -lt 0) { '  <-- NOTHING SELECTED' } else { '' }
            "  {0,4}  {1,-22} {2} options, showing #{3}{4}" -f $id, $label, $n, $sel, $note
            $pressed++
            continue
        }
        $rows = [int][Run]::SendMessageW($c, $LB_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
        if ($rows -gt 0) {
            "  {0,4}  {1,-22} {2} rows" -f $id, $label, $rows
            $pressed++
            continue
        }

        $ms = Press $id $label
        if ($ms -gt $worst) { $worst = $ms }
        $flag = if ($ms -gt 200) { '  <-- STALL' } else { '' }

        # **A checkbox is pressed twice.** These two are toggles, and a
        # run-through that left "allow unverified architectures" flipped would
        # have changed the thing it was inspecting. Nothing is saved either
        # way, but a second press puts the window back where it was found.
        $note = ''
        if ($id -eq 308 -or $id -eq 309) {
            $back = Press $id $label
            if ($back -gt $worst) { $worst = $back }
            $note = ' (toggled and restored)'
        }
        "  {0,4}  {1,-22} pressed {2,8:N1} ms{3}{4}" -f $id, $label, $ms, $flag, $note
        $pressed++
    }
}

''
"".PadRight(78, '=')
"{0} controls exercised, {1} skipped by policy" -f $pressed, $skipped
"worst blocking call: {0:N1} ms" -f $worst
if ($worst -gt 200) { 'A click over 200 ms is a window that looks frozen.' }
else { 'Nothing blocked the window for longer than a fifth of a second.' }
