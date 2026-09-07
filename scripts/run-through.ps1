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
# Two more are listed rather than pressed:
#
#   768      NEW KEY   throws the current key away, so every device that had it
#            has to be told the new one
#   773      CHANGE MODE  opens a MODAL Yes/No, so the next SendMessageW never
#            returns and the script hangs rather than failing. The four role
#            buttons it replaced (760-763) no longer exist: the mode is answered
#            by the launch knob and shown by the badge, 772.
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
    // For the unlisted-control sweep. Sibling walking rather than
    // `EnumChildWindows`, which would need a delegate marshalled out of an
    // inline `Add-Type` -- more moving parts than the walk is worth.
    [DllImport("user32.dll")]
    public static extern IntPtr GetWindow(IntPtr hwnd, uint cmd);
    [DllImport("user32.dll")]
    public static extern int GetDlgCtrlID(IntPtr hwnd);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder buf, int max);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, StringBuilder buf);
    // **`GetWindowText` reads a CAPTION, and cross-process it reads only that.**
    // Called from another process it does not send `WM_GETTEXT` -- by design, so
    // a hung target cannot hang the caller -- so an EDIT owned by the app comes
    // back as the empty string however much text is in it. A whole defect was
    // reported against this app on the strength of that empty string: the CHAOS
    // page looked blank from outside while the app had filled it correctly.
    // `WM_GETTEXT` is the honest read; the caption is the fallback, because a
    // BUTTON's label really is its caption.
    public static string TextOf(IntPtr h) {
        var sb = new StringBuilder(4096);
        SendMessageW(h, 0x000D /* WM_GETTEXT */, (IntPtr)4096, sb);
        if (sb.Length > 0) { return sb.ToString(); }
        var cap = new StringBuilder(512); GetWindowTextW(h, cap, 512); return cap.ToString();
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
    768 = 'DESTRUCTIVE: throws the key away, so every device must be told again'
    773 = 'BLOCKS: CHANGE MODE opens a MODAL confirmation, which stops the message loop'
}
if (-not $Slow) {
    $skip[204] = 'slow: starts a model, minutes. Pass -Slow to include it'
    $skip[704] = 'slow: starts a render, hours. Pass -Slow to include it'
}
if (-not $Brand) {
    $skip[770] = 'opens a browser: pass -Brand to include it'
    # Opens a modal folder dialog to ask which project to work in, and a modal
    # dialog stops the message loop -- the same reason BROWSE... (312) is
    # skipped. Pressing it here would hang the transcript, not measure it.
    $skip[774] = 'BLOCKS: opens a modal folder dialog, which stops the message loop'
    # The strip's STOP, on every page. Pressing it with nothing running is
    # harmless and proves nothing; pressing it with something running would
    # stop the thing this transcript is measuring.
    $skip[405] = 'stops whatever is running -- nothing is, so it would prove nothing'
}

$pages = @(
    @{ Id = 401; Name = 'CHAT';     Controls = @(101, 102, 104, 103) }
    @{ Id = 402; Name = 'MODELS';   Controls = @(201, 202, 208, 210, 211, 212, 203, 204, 205, 206, 207, 209) }
    @{ Id = 403; Name = 'MONITOR';  Controls = @() }
    @{ Id = 404; Name = 'SETTINGS'; Controls = @(301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312) }
    @{ Id = 406; Name = 'IMAGE';    Controls = @(708, 701, 702, 703, 709, 706, 705, 707, 704) }
    # **The page the run-through never covered**, which is where the mode lives
    # and where the two brand buttons were added and never clicked.
    # **CHAOS has no rail entry any more** -- 407 is still its id and still
    # opens it, which is what the mode badge does. Reached that way here so the
    # transcript covers the page a person can still get to.
    @{ Id = 407; Name = 'CHAOS';    Controls = @(760, 764, 765, 766, 767, 768, 770, 774, 769) }
)

# **This list is hardcoded, and that is a hole in the instrument itself.**
# `USE WITH CLAUDE CODE` (774) was added to the CHAOS page, laid out, wired and
# on screen -- and this transcript did not mention it, because 774 was not in
# the array above. An instrument whose whole purpose is finding controls a
# person cannot reach was blind to a new one.
#
# The check below closes it: after each page, every control that is VISIBLE and
# has a rectangle inside the window is compared against the list, and anything
# unlisted is reported. A new control can then be un-pressed but not unseen.

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

# **There is no launch screen any more**, and RETURN is no longer needed to get
# past one. The mode knob owned the window until it was answered; without a
# RETURN here every control below read HIDDEN and the transcript said nothing.
# One mode now, so the rail is up the moment the window is.
#
# The check below stays, because it is the honest one: `layout` parks controls
# it does not want at -3200, and `IsWindowVisible` is true for a parked button
# as well as a shown one, so a coordinate test is what "on screen" means here.
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
if ($onScreen -lt 6) {
    Write-Error "Only $onScreen of the 6 rail buttons are on screen. Every page below would read HIDDEN, so nothing is reported."
    exit 1
}

"Chaos run-through  --  window $hwnd"
"rail: $onScreen buttons on screen, no launch screen to get past"
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

    # **The sweep that makes a new control impossible to miss.** The list above
    # is written by hand, and `USE WITH CLAUDE CODE` proved what that costs: it
    # was declared, created, laid out, wired and on screen, and this transcript
    # said nothing about it because its id was not in the array.
    #
    # So: walk every child of the window, keep the ones that are visible with a
    # rectangle inside the window -- which on this page is this page's own
    # controls plus the shell -- and report any whose id the list does not know.
    # Unlisted is not the same as broken; it means nobody decided about it.
    $known = @{}
    foreach ($p2 in $pages) { $known[$p2.Id] = $true; foreach ($cid in $p2.Controls) { $known[$cid] = $true } }
    # The page ids in `$pages` are the rail buttons themselves, so they are
    # already known. These are the rest of the shell, which lives on every page
    # and belongs to no page: the mode badge, its CHANGE MODE, and the strip's
    # STOP. **The sweep found all three on its first run**, along with the
    # image prompt and its log, which are now in the IMAGE list above -- the
    # prompt field had never been exercised by this script at all.
    foreach ($shell in 405) { $known[$shell] = $true }
    $wr = New-Object Run+RECT
    [void][Run]::GetWindowRect($hwnd, [ref]$wr)
    $unlisted = @()
    $child = [Run]::GetWindow($hwnd, 5)          # GW_CHILD
    while ($child -ne [IntPtr]::Zero) {
        if ([Run]::IsWindowVisible($child)) {
            $cr = New-Object Run+RECT
            [void][Run]::GetWindowRect($child, [ref]$cr)
            $inside = ($cr.left -ge $wr.left) -and ($cr.right -le $wr.right) -and
                      ($cr.top -ge $wr.top) -and ($cr.bottom -le $wr.bottom)
            $cid = [Run]::GetDlgCtrlID($child)
            if ($inside -and $cid -gt 0 -and -not $known.ContainsKey($cid)) {
                $unlisted += "$cid '$([Run]::TextOf($child))'"
            }
        }
        $child = [Run]::GetWindow($child, 2)     # GW_HWNDNEXT
    }
    if ($unlisted.Count -gt 0) {
        "  ON SCREEN AND NOT IN THIS SCRIPT'S LIST: {0}" -f ($unlisted -join ', ')
        "  Add them to the page above, with a skip reason if pressing them blocks."
    }
}

''
"".PadRight(78, '=')
"{0} controls exercised, {1} skipped by policy" -f $pressed, $skipped
"worst blocking call: {0:N1} ms" -f $worst
if ($worst -gt 200) { 'A click over 200 ms is a window that looks frozen.' }
else { 'Nothing blocked the window for longer than a fifth of a second.' }
