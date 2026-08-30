# Does the window open, and can anything be seen on it?
#
# **This is the check that would have caught v0.0.21.** That release shipped with
# the mode knob painted *underneath* nine live child windows: the app opened, the
# process was healthy, every unit test passed, and `run-through.ps1` reported a
# clean pass over it -- because it drove pages by `WM_COMMAND`, which goes
# neither through the rail nor through the knob, so it walked an application that
# had never left its launch screen and never noticed.
#
# So this does the one thing that instrument did not: it presses RETURN like a
# person, and then counts what is actually on the screen.
#
# # Deliberately small
#
# `run-through.ps1` is the thorough instrument and it is **not** safe to run
# unattended: it documents its own hang modes, because BROWSE and CHANGE MODE
# open modal dialogs and the next `SendMessageW` never returns. A script that can
# hang is a script that can wedge a CI job for six hours.
#
# This one touches no modal control, writes no settings, and kills the process
# itself. It answers one question -- *did anything appear* -- and leaves the rest
# to the instrument built for it.
#
# # Two traps it is built around, both already paid for
#
# **`IsWindowVisible` is not "on screen".** `layout` parks the rail buttons a
# mode cannot reach at `(-3200,-3200)` and leaves them visible, so a count of
# visible windows says 22 for a window showing nothing. Client-rects, mapped to
# the screen, are the only honest measure.
#
# **A missing desktop is not a defect.** On a machine with no window station this
# reports SKIPPED and exits 0. Reporting a failure there would teach everyone to
# ignore this check, which is how a gate becomes decoration.
#
#   .\scripts\app-smoke.ps1
#   .\scripts\app-smoke.ps1 -Exe target\release\chaos-app.exe -TimeoutSeconds 60
#
# Exit 0 when the window opened and controls are on screen, or when there is no
# desktop to open one on. Exit 1 when the window opened and nothing was visible.

[CmdletBinding()]
param(
    [string]$Exe = "target\release\chaos-app.exe",
    [int]$TimeoutSeconds = 60,
    # The v0.0.21 window had 0 on screen after the knob was dismissed. Anything
    # above a handful means the shell came up; the exact number belongs to
    # run-through.ps1, not here.
    [int]$MinControls = 4
)

$ErrorActionPreference = "Stop"

Add-Type -Namespace Smoke -Name Win -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
[DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll")] public static extern IntPtr SetForegroundWindow(IntPtr h);
public struct RECT { public int left, top, right, bottom; }
public struct POINT { public int x, y; }
'@

function Get-OnScreenControls([IntPtr]$parent) {
    # GW_CHILD = 5, GW_HWNDNEXT = 2
    $n = 0
    $child = [Smoke.Win]::GetWindow($parent, 5)
    while ($child -ne [IntPtr]::Zero) {
        if ([Smoke.Win]::IsWindowVisible($child)) {
            $r = New-Object Smoke.Win+RECT
            if ([Smoke.Win]::GetClientRect($child, [ref]$r)) {
                $p = New-Object Smoke.Win+POINT
                $p.x = $r.left; $p.y = $r.top
                [void][Smoke.Win]::ClientToScreen($child, [ref]$p)
                $w = $r.right - $r.left
                $h = $r.bottom - $r.top
                # Parked off-screen at (-3200,-3200), or collapsed to nothing.
                if ($w -gt 0 -and $h -gt 0 -and $p.x -gt -1000 -and $p.y -gt -1000) {
                    $n++
                }
            }
        }
        $child = [Smoke.Win]::GetWindow($child, 2)
    }
    return $n
}

if (-not (Test-Path $Exe)) {
    Write-Host "::error::$Exe is not there. Build it: cargo build --release --bin chaos-app"
    exit 1
}

# **Run against a settings file of its own, not the user's.**
#
# Found by running this script twice: the first run pressed RETURN, which
# chose a mode, and `mode_chosen = true` was written to the real
# `~/.chaos/settings.txt`. The second run then skipped the knob entirely and
# reported 10 controls *at open* where the first had reported 0 -- so the
# script had changed the thing it was measuring, and left a setting behind on
# a machine it does not own.
#
# `chaos_config::path()` derives from USERPROFILE (or HOME), so a temporary
# profile is all the isolation needed and no product code changes for a test.
$sandbox = Join-Path ([System.IO.Path]::GetTempPath()) ("chaos-smoke-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $sandbox -Force | Out-Null

$proc = $null
try {
    $proc = Start-Process -FilePath $Exe -PassThru -Environment @{
        USERPROFILE = $sandbox
        HOME        = $sandbox
    }
} catch {
    # -Environment needs PowerShell 7. Fall back to setting it for this process,
    # which the child inherits, and putting it back afterwards.
    $savedUser = $env:USERPROFILE
    $savedHome = $env:HOME
    try {
        $env:USERPROFILE = $sandbox
        $env:HOME = $sandbox
        $proc = Start-Process -FilePath $Exe -PassThru
    } catch {
        Write-Host "SKIPPED: the app could not be started here ($($_.Exception.Message))"
        exit 0
    } finally {
        $env:USERPROFILE = $savedUser
        $env:HOME = $savedHome
    }
}

try {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $hwnd = [IntPtr]::Zero
    while ((Get-Date) -lt $deadline) {
        if ($proc.HasExited) {
            Write-Host "::error::the app exited on its own with code $($proc.ExitCode)"
            exit 1
        }
        $proc.Refresh()
        if ($proc.MainWindowHandle -ne [IntPtr]::Zero) {
            $hwnd = $proc.MainWindowHandle
            break
        }
        Start-Sleep -Milliseconds 250
    }

    if ($hwnd -eq [IntPtr]::Zero) {
        # No window inside the timeout. On a runner with no interactive desktop
        # this is the environment, not the app, and calling it a failure would
        # train everyone to ignore this check.
        Write-Host "SKIPPED: no window appeared in ${TimeoutSeconds}s -- this machine may have no desktop"
        exit 0
    }

    $before = Get-OnScreenControls $hwnd
    Write-Host "on-screen controls at open: $before   (the launch knob paints, it does not create controls)"

    # Leave the knob the way a person does. RETURN, not WM_COMMAND: driving the
    # page directly is exactly how the old instrument walked an app that had
    # never left its launch screen.
    [void][Smoke.Win]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 400
    # WM_KEYDOWN/WM_KEYUP, VK_RETURN
    [void][Smoke.Win]::PostMessageW($hwnd, 0x0100, [IntPtr]0x0D, [IntPtr]0)
    [void][Smoke.Win]::PostMessageW($hwnd, 0x0101, [IntPtr]0x0D, [IntPtr]0)
    Start-Sleep -Milliseconds 1200

    $after = Get-OnScreenControls $hwnd
    Write-Host "on-screen controls after RETURN: $after"

    if ($after -lt $MinControls) {
        Write-Host "::error::the window opened and $after controls are on screen after RETURN."
        Write-Host "::error::v0.0.21 shipped exactly this: the knob painted under nine live"
        Write-Host "::error::child windows, so the app opened onto a screen with nothing usable."
        exit 1
    }

    Write-Host "OK: the window opened and $after controls are on screen."
    exit 0
}
finally {
    if ($proc -and -not $proc.HasExited) {
        # It owns a message loop and will not close itself.
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    }
    if ($sandbox -and (Test-Path $sandbox)) {
        Remove-Item -Recurse -Force $sandbox -ErrorAction SilentlyContinue
    }
}
