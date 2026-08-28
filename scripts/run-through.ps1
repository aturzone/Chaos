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
# On the CHAOS page five more are listed rather than pressed, because pressing
# them would reconfigure the machine this script is inspecting:
#
#   760-763  ALONE/CORE/HELPER/CLIENT  change the mode, restart the server and
#            write settings -- the run-through would leave the app in a
#            different mode than it found it
#   768      NEW KEY   throws the current key away, so every device that had it
#            has to be told the new one
#
# Two more open a browser and are opt-in with -Brand:
#
#   770  SHOW THE MARK   shell_open of the node's /qr
#   771  READ A CODE     shell_open of the node's /scan
#
# Two more are *slow* rather than dangerous and are opt-in with -Slow:
#
#   204  LOAD     starts a model; minutes
#   704  DRAW     starts a render; hours at a useful size
#
# Everything else is pressed. `SendMessageW` is synchronous, so the time each
# takes is the time the UI thread was blocked -- anything over 200 ms is a
# window that looks frozen to the person using it.
#
# # It enters a mode first, and that is not optional
#
# **The knob owns the window until a mode is chosen, and it owns the child
# windows too.** This script drives pages with `WM_COMMAND`, which does not go
# through the rail -- so before the guard in `show_page` existed it walked an app
# that had never left its launch screen and reported a clean pass over controls
# that were stacked on top of the knob. Now the same run would report every
# control HIDDEN, which is just as misleading. So it presses RETURN first, the
# knob's own "enter this mode", and stops if that did not take.

param(
    [switch] $Slow,
    [switch] $Brand,
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
    public static extern bool GetWindowRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll")]
    public static extern bool ScreenToClient(IntPtr hwnd, ref POINT p);
    public struct RECT { public int left, top, right, bottom; }
    public struct POINT { public int x, y; }
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
    760 = 'RECONFIGURES: changes the mode and restarts the server'
    761 = 'RECONFIGURES: changes the mode and restarts the server'
    762 = 'RECONFIGURES: changes the mode and restarts the server'
    763 = 'RECONFIGURES: changes the mode and restarts the server'
    768 = 'DESTRUCTIVE: throws the key away, so every device must be told again'
}
if (-not $Slow) {
    $skip[204] = 'slow: starts a model, minutes. Pass -Slow to include it'
    $skip[704] = 'slow: starts a render, hours. Pass -Slow to include it'
}
if (-not $Brand) {
    $skip[770] = 'opens a browser: pass -Brand to include it'
    $skip[771] = 'opens a browser: pass -Brand to include it'
}

$pages = @(
    @{ Id = 401; Name = 'CHAT';     Controls = @(104, 103) }
    @{ Id = 402; Name = 'MODELS';   Controls = @(201, 202, 208, 210, 211, 212, 203, 204, 205, 206, 207, 209) }
    @{ Id = 403; Name = 'MONITOR';  Controls = @() }
    @{ Id = 404; Name = 'SETTINGS'; Controls = @(301, 302, 303, 305, 306, 308, 309, 310, 311, 312) }
    @{ Id = 406; Name = 'IMAGE';    Controls = @(708, 702, 703, 709, 706, 705, 704) }
    # **The page the run-through never covered**, which is where the mode lives
    # and where the two brand buttons were added and never clicked.
    @{ Id = 407; Name = 'CHAOS';    Controls = @(760, 761, 762, 763, 764, 765, 766, 767, 768, 770, 771, 769) }
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

# **Leave the launch screen first.** RETURN is the knob's own "enter this
# mode", and it is a no-op once a mode is entered, so this is safe to send
# either way. Without it every control below reads HIDDEN and the transcript
# says nothing.
$WM_KEYDOWN = 0x0100
$VK_RETURN = 0x0D
[Run]::SendMessageW($hwnd, $WM_KEYDOWN, [IntPtr]$VK_RETURN, [IntPtr]0) | Out-Null
Start-Sleep -Milliseconds 700

# Did it take? A rail button on-screen means the shell is up. `layout` parks the
# pages this mode cannot reach at -3200, so a coordinate test is the honest one:
# `IsWindowVisible` is true for a parked button as well as a shown one.
$onScreen = 0
foreach ($id in 401, 402, 403, 404, 406, 407) {
    $c = [Run]::GetDlgItem($hwnd, $id)
    if ($c -eq [IntPtr]::Zero) { continue }
    if (-not [Run]::IsWindowVisible($c)) { continue }
    $r = New-Object Run+RECT
    [Run]::GetWindowRect($c, [ref]$r) | Out-Null
    $pt = New-Object Run+POINT
    $pt.x = $r.left; $pt.y = $r.top
    [Run]::ScreenToClient($hwnd, [ref]$pt) | Out-Null
    if ($pt.x -gt -1000 -and $pt.y -gt -1000) { $onScreen++ }
}
if ($onScreen -eq 0) {
    Write-Error 'The window is still on its launch screen: no rail button is on screen after RETURN. Everything below would read HIDDEN, so nothing is reported.'
    exit 1
}

"Chaos run-through  --  window $hwnd"
"entered a mode: $onScreen rail buttons on screen"
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
