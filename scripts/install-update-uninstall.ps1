<#
.SYNOPSIS
Install a real downloaded release, update it, uninstall it — counting the
models directory before and after.

.DESCRIPTION
The plan's last unchecked item under "Actually run the thing": *install →
update → uninstall from a real downloaded setup, on the machine, with the
models directory counted before and after.*

**Why it needs a script rather than a session of clicking.** The thing being
checked is not that the installer opens. It is that a **downloaded** artefact —
the one a stranger gets, not the one `cargo build` leaves in `target` — puts
files where it says, that a version already installed can reach a newer one on
its own, that removal removes what it added, and that **none of it touches the
models**. Three of those four are only visible by counting something before and
after, which is exactly what a person watching a progress bar does not do.

**The models directory is never written by this script.** It is measured, twice,
and compared. The counts are file count and total bytes: a truncated file keeps
the count and changes the bytes, and a deleted one changes both. If the two
measurements disagree, that is the headline and everything else is a footnote —
`~/.chaos/models` here holds 144 GB that must never be downloaded again.

.PARAMETER From
The version to install first. Default 0.0.16, the release before this one, so
the update step has somewhere to go.

.PARAMETER To
The version the update is expected to reach. Default 0.0.17.

.PARAMETER KeepInstalled
Re-install `To` at the end, so the machine is left with a working Chaos rather
than none. On by default — this runs on Atur's own machine, and the honest
end state of a test is not "your app is gone".

.EXAMPLE
powershell -ExecutionPolicy Bypass -File scripts/install-update-uninstall.ps1
#>
[CmdletBinding()]
param(
    [string]$From = '0.0.16',
    [string]$To = '0.0.17',
    [bool]$KeepInstalled = $true
)

$ErrorActionPreference = 'Stop'
$prefix = Join-Path $env:LOCALAPPDATA 'Chaos'
$models = Join-Path $env:USERPROFILE '.chaos\models'
$work = Join-Path $env:TEMP 'chaos-install-check'
$regkey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Chaos'
$failures = New-Object System.Collections.ArrayList

function Say($text) { Write-Host $text }
function Head($text) {
    Write-Host ''
    Write-Host ("=== {0} {1}" -f $text, ('=' * [Math]::Max(0, 58 - $text.Length)))
}

# A check that records rather than throws: one failure should not hide the
# five checks after it, and the models comparison at the end matters most.
function Check($name, $ok, $detail) {
    if ($ok) { Write-Host ("  [ok]   {0}" -f $name) }
    else {
        Write-Host ("  [FAIL] {0}" -f $name) -ForegroundColor Red
        [void]$failures.Add($name)
    }
    if ($detail) { Write-Host ("         {0}" -f $detail) }
}

# **Count, do not trust a name.** Recursive, because a model is often several
# shards in a subdirectory.
function Measure-Models {
    if (-not (Test-Path $models)) { return @{ Files = 0; Bytes = 0; Missing = $true } }
    $f = Get-ChildItem -Path $models -Recurse -File -ErrorAction SilentlyContinue
    $bytes = 0
    foreach ($x in $f) { $bytes += $x.Length }
    return @{ Files = $f.Count; Bytes = $bytes; Missing = $false }
}

function Installed-Version {
    $v = Join-Path $prefix 'version.txt'
    if (Test-Path $v) { return (Get-Content $v -Raw).Trim() }
    return $null
}

# **Never `cmd /c "<a command line with quotes in it>"`.** The registry's
# QuietUninstallString is one string containing quoted paths; handing it to
# cmd.exe through PowerShell re-quotes it once more and cmd's own rules eat the
# result. Run three times here, it silently did nothing at all -- and the script
# reported that as "the uninstaller does not remove its files", which would have
# been a serious and entirely false bug report against the installer.
#
# The exe and its arguments as an array have no quoting layer to get wrong.
function Invoke-Uninstall($prefix) {
    $exe = Join-Path $prefix 'bin\chaos-setup.exe'
    if (-not (Test-Path $exe)) { return $false }
    Start-Process -FilePath $exe -ArgumentList '/S', '--uninstall', '--prefix', $prefix -Wait
    # The uninstaller re-launches itself from the temp directory so it can
    # delete the copy inside the prefix, so the process that was waited on
    # exits before the work is finished.
    return (Wait-Gone (Join-Path $prefix 'bin') 60)
}

function Wait-Gone($path, $seconds) {
    $limit = (Get-Date).AddSeconds($seconds)
    while ((Test-Path $path) -and (Get-Date) -lt $limit) { Start-Sleep -Milliseconds 250 }
    return -not (Test-Path $path)
}

Head 'before'
$before = Measure-Models
if ($before.Missing) {
    Say "  no models directory at $models"
} else {
    Say ("  {0}: {1} files, {2:N0} bytes ({3:N1} GiB)" -f $models, $before.Files, $before.Bytes, ($before.Bytes / 1GB))
}
$was = Installed-Version
Say ("  installed now: {0}" -f $(if ($was) { "v$was" } else { 'nothing' }))

# ---------------------------------------------------------------------------
# 1. Remove whatever is there, so "install" is an install and not an upgrade.
# ---------------------------------------------------------------------------
if ($was) {
    Head "clearing v$was first"
    [void](Invoke-Uninstall $prefix)
    Check 'the old install is gone' (-not (Test-Path (Join-Path $prefix 'bin'))) $prefix
}

# ---------------------------------------------------------------------------
# 2. Download the real artefact.
# ---------------------------------------------------------------------------
Head "downloading v$From"
New-Item -ItemType Directory -Force -Path $work | Out-Null
$setup = Join-Path $work "Chaos-v$From-windows-x86_64-Setup.exe"
$url = "https://github.com/aturzone/Chaos/releases/download/v$From/Chaos-v$From-windows-x86_64-Setup.exe"
Say "  $url"
& curl.exe -sSL --fail -o $setup $url
if (-not (Test-Path $setup)) { throw "download failed: $url" }
$size = (Get-Item $setup).Length
# **A 9-byte "Not Found" is also a file.** Two releases have been debugged from
# an artefact that downloaded successfully and was not the artefact.
$magic = [System.IO.File]::ReadAllBytes($setup)[0..1]
Check 'it is a Windows executable, not an error page' (($magic[0] -eq 0x4D) -and ($magic[1] -eq 0x5A)) ("{0:N0} bytes" -f $size)
Check 'it is a plausible size (> 5 MB)' ($size -gt 5MB) ("{0:N1} MB" -f ($size / 1MB))

# ---------------------------------------------------------------------------
# 3. Install it silently.
# ---------------------------------------------------------------------------
Head "installing v$From"
$t0 = Get-Date
Start-Process -FilePath $setup -ArgumentList '/S' -Wait
$took = ((Get-Date) - $t0).TotalSeconds
Say ("  took {0:N1} s" -f $took)

# **Returning is not finishing.** The installer stages work and can hand back
# before the last file is written; a check that runs immediately reads whatever
# was true a moment ago, which is how a passing test hides a broken install.
$limit = (Get-Date).AddSeconds(120)
while ((Installed-Version) -ne $From -and (Get-Date) -lt $limit) { Start-Sleep -Milliseconds 500 }

Check 'version.txt says what was installed' ((Installed-Version) -eq $From) ("version.txt = {0}" -f (Installed-Version))
$bin = Join-Path $prefix 'bin'
$exes = @(Get-ChildItem $bin -Filter *.exe -ErrorAction SilentlyContinue)
Check 'the binaries are there' ($exes.Count -ge 12) ("{0} .exe in {1}" -f $exes.Count, $bin)
$reg = Get-ItemProperty $regkey -ErrorAction SilentlyContinue
Check 'Windows knows how to remove it' ($null -ne $reg -and $reg.DisplayVersion -eq $From) ("DisplayVersion = {0}" -f $reg.DisplayVersion)
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Chaos.lnk'
Check 'there is a Start Menu entry' (Test-Path $lnk) $lnk

# **Does it start?** The one failure that made every earlier release useless
# was a binary that died before `main` with no message on a machine without
# MSYS2. `--version` is the smallest thing that proves the process ran.
$run = Join-Path $bin 'chaos-run.exe'
$out = & $run --version 2>&1 | Out-String
Check 'the installed chaos-run starts and reports its version' ($out -match [regex]::Escape($From)) ($out.Trim())

# ---------------------------------------------------------------------------
# 4. Update, using the app's own path rather than a second download by hand.
# ---------------------------------------------------------------------------
Head "updating v$From -> v$To"
# **This runs the OLD version's updater, which is the point.** Whatever is
# shipped today is what a user updates from, so the code under test here is
# v$From's, not the working tree's.
#
# Two things had to be fixed to get this far, and only the second is testable
# from an old release:
#
#   1. `'y' | & $exe` leaves stdin at EOF. `--update` read an empty line, took
#      it for "no", and the script then reported "the update check found the
#      newer release" -- true -- while nothing had been updated. A file on
#      stdin is what actually answers the question.
#   2. `--update` downloads and then opens the installer's WINDOW. Even
#      answering "y" leaves a script waiting on a button nobody will press.
#      `--update --yes` passes `/S` from the next release on; against every
#      release before it, the honest thing is to say what this step covers and
#      finish the upgrade the way pressing INSTALL does.
$log = Join-Path $work 'update.log'
$yes = Join-Path $work 'yes.txt'
Set-Content -Path $yes -Value 'y' -Encoding ascii
$before_dl = Get-Date
# **Not `-Wait`, and not a pipe either.** `-Wait` waits for the whole process
# tree, and `--update` deliberately spawns an installer and exits -- so it sat
# on a window waiting for a click no script will make. Reading the output
# through a pipe instead deadlocks in the same place, because the installer
# inherits the handle and the pipe does not close until it exits. Files for the
# output, `-PassThru` for the handle, and a bounded wait on that one process.
$proc = Start-Process -FilePath $run -ArgumentList '--update', '--yes' `
    -RedirectStandardInput $yes -RedirectStandardOutput $log `
    -RedirectStandardError (Join-Path $work 'update.err') -NoNewWindow -PassThru
if (-not $proc.WaitForExit(300000)) {
    $proc.Kill()
    Say '  chaos-run --update did not exit within 5 minutes'
}
$updateOut = (Get-Content $log -Raw) + (Get-Content (Join-Path $work 'update.err') -Raw -ErrorAction SilentlyContinue)
Say ($updateOut.Trim() -split "`n" | Select-Object -First 14 | Out-String)
Check 'the update check found the newer release' ($updateOut -match [regex]::Escape($To)) ''
Check 'it got past the question rather than reading EOF' ($updateOut -notmatch 'nothing downloaded') ''

# What it downloaded, in the place the running code chose -- not a path this
# script picked, so a change to that path fails here rather than passing.
$staged = Join-Path $env:TEMP "Chaos-v$To-windows-x86_64-Setup.exe"
$fresh = (Test-Path $staged) -and ((Get-Item $staged).LastWriteTime -gt $before_dl)
Check 'it downloaded the new installer itself' $fresh $staged

# If the old updater opened a window, close it: this step is finished with it,
# and a stray installer window would sit over everything after.
Start-Sleep -Seconds 3
$gui = Get-Process -Name "Chaos-v$To-windows-x86_64-Setup" -ErrorAction SilentlyContinue
$opened_window = $null -ne $gui
if ($opened_window) { $gui | Stop-Process -Force; Start-Sleep -Seconds 1 }

$limit = (Get-Date).AddSeconds(30)
while ((Installed-Version) -ne $To -and (Get-Date) -lt $limit) { Start-Sleep -Seconds 1 }
if ((Installed-Version) -ne $To) {
    # v$From's updater has no silent path. Do what pressing INSTALL does, and
    # say so rather than reporting an unattended update that did not happen.
    Say "  v$From's updater opens a window and waits; finishing it the way INSTALL does."
    Start-Process -FilePath $staged -ArgumentList '/S' -Wait
    $limit = (Get-Date).AddSeconds(120)
    while ((Installed-Version) -ne $To -and (Get-Date) -lt $limit) { Start-Sleep -Milliseconds 500 }
    Check 'the installer it downloaded upgrades in place' ((Installed-Version) -eq $To) ("version.txt = {0}" -f (Installed-Version))
} else {
    Check 'the update completed unattended' $true ("version.txt = {0}" -f (Installed-Version))
}
$reg = Get-ItemProperty $regkey -ErrorAction SilentlyContinue
Check 'the uninstall entry was updated too' ($reg.DisplayVersion -eq $To) ("DisplayVersion = {0}" -f $reg.DisplayVersion)
$out = & $run --version 2>&1 | Out-String
Check 'the updated chaos-run starts' ($out -match [regex]::Escape($To)) ($out.Trim())

# ---------------------------------------------------------------------------
# 5. Uninstall.
# ---------------------------------------------------------------------------
Head 'uninstalling'
$q = (Get-ItemProperty $regkey -ErrorAction SilentlyContinue).QuietUninstallString
Check 'there is a quiet uninstall command' ($null -ne $q) $q
[void](Invoke-Uninstall $prefix)
Check 'bin is gone' (-not (Test-Path $bin)) $bin
Check 'the uninstall entry is gone' ($null -eq (Get-ItemProperty $regkey -ErrorAction SilentlyContinue)) $regkey
Check 'the Start Menu entry is gone' (-not (Test-Path $lnk)) $lnk

# ---------------------------------------------------------------------------
# 6. The measurement the whole script exists for.
# ---------------------------------------------------------------------------
Head 'after'
$after = Measure-Models
if ($after.Missing) {
    Say "  no models directory at $models"
} else {
    Say ("  {0}: {1} files, {2:N0} bytes ({3:N1} GiB)" -f $models, $after.Files, $after.Bytes, ($after.Bytes / 1GB))
}
Check 'the models directory still exists' (-not $after.Missing) $models
Check 'not one model file was added or removed' ($after.Files -eq $before.Files) ("{0} before, {1} after" -f $before.Files, $after.Files)
Check 'not one byte of models changed' ($after.Bytes -eq $before.Bytes) ("{0:N0} before, {1:N0} after" -f $before.Bytes, $after.Bytes)

# ---------------------------------------------------------------------------
# 7. Leave the machine with a working install.
# ---------------------------------------------------------------------------
if ($KeepInstalled) {
    Head "re-installing v$To so the machine is not left empty"
    $keep = Join-Path $work "Chaos-v$To-windows-x86_64-Setup.exe"
    & curl.exe -sSL --fail -o $keep "https://github.com/aturzone/Chaos/releases/download/v$To/Chaos-v$To-windows-x86_64-Setup.exe"
    Start-Process -FilePath $keep -ArgumentList '/S' -Wait
    Check 'Chaos is installed again' ((Installed-Version) -eq $To) ("version.txt = {0}" -f (Installed-Version))
}

Head 'result'
if ($failures.Count -eq 0) {
    Write-Host '  install -> update -> uninstall clean, models untouched.' -ForegroundColor Green
    exit 0
} else {
    Write-Host ("  {0} check(s) failed:" -f $failures.Count) -ForegroundColor Red
    foreach ($f in $failures) { Write-Host "    - $f" -ForegroundColor Red }
    exit 1
}
