<#
.SYNOPSIS
tok/s against how many layers are on the device.

.DESCRIPTION
`research/ngl-partial-offload-2026-08-16.md` closed the accuracy question — the
device path picks the same token as the CPU on 8 of 8 parity prompts — and left
this open in writing:

> **A speed number for `-ngl`.** Nothing here is a performance claim. The
> partial-offload tok/s ladder against resident VRAM is the interesting
> measurement and it has not been run.

This runs it. One model, `-ngl` from 0 to all of it, prefill and generation
reported separately because **they are two different questions**: prefill is
compute-bound and a GPU should win it outright, generation is bandwidth-bound
and only wins if the weights it reads every token are on the far side of the
PCIe bus already.

**Requires a Vulkan build.** `GGML_LIB_DIR` must point at
`build-vulkan/ggml/src`, which is the directory with `ggml-vulkan/ggml-vulkan.a`
in it. Against a CPU-only build every row is the same number and the script says
so rather than reporting a flat ladder as a finding.

**Read the rules before believing a row.** `Get-Process` first — an orphaned
benchmark holding 9 GiB has looked like a 10x regression here before — and
compare only within one run of this script. The alternation and the repeat count
exist because a first GPU run compiles shader pipelines inside the timed region;
that once produced a published 0.42x that was really 1.7x.

.PARAMETER Model
Any unique part of a model name, as `chaos-run` accepts.

.PARAMETER Layers
Which `-ngl` values to walk. Default 0 (all CPU) through 99 (all device).

.PARAMETER Device
Device index from `chaos-run --list-devices`. Default 1, the discrete card on
this machine; 0 is the integrated one and `the-igpu-is-not-a-tier` says what
that is worth.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File scripts/ngl-ladder.ps1 -Model Qwen3-4B
#>
[CmdletBinding()]
param(
    [string]$Model = 'Qwen3-4B',
    [int[]]$Layers = @(0, 8, 16, 24, 32, 99),
    [int]$Device = 1,
    [int]$Tokens = 32,
    [int]$Repeat = 2,
    [string]$Prompt = 'The capital of France is',
    # **A five-token prompt does not measure prefill.** The first run of this
    # reported a prefill figure from 0.2 s of work, and the rows moved by a
    # factor of two for no reason a bus could explain. Point this at a file of a
    # few hundred tokens and the prefill column becomes a number rather than a
    # coin toss.
    [string]$PromptFile = ''
)

$ErrorActionPreference = 'Stop'
$run = Join-Path $PSScriptRoot '..\target\release\chaos-run.exe'
if (-not (Test-Path $run)) { throw "no chaos-run at $run" }

# **A layer count is a small number.** Passing `-Layers 0,8,16,24,32,99` through
# a nested `powershell -File` collapsed the list into the single integer
# 816243299 -- the digits concatenated -- and the ladder ran one row, labelled
# with that number, and printed a plausible prefill and generation figure beside
# it. Nothing about the output said anything was wrong.
#
# 99 already means "all of them" to chaos-run, so anything past a few hundred is
# a parsing accident rather than a request.
foreach ($n in $Layers) {
    if ($n -lt 0 -or $n -gt 999) {
        throw ("-Layers got $n, which is not a layer count. Passing a list through " +
               "``powershell -File`` can concatenate it into one integer; pass the " +
               "values one call at a time, or use the default.")
    }
}

# **Never `2>&1` on a native command in Windows PowerShell.** ggml writes its
# device banner to stderr; redirecting it wraps every line in an ErrorRecord,
# and with `$ErrorActionPreference = 'Stop'` above that is a terminating error
# raised by a program that succeeded. The first run of this script died on
# `ggml_vulkan: Found 2 Vulkan devices:` -- an informational line.
#
# So: stdout only here, and `--log-file` below, which is why chaos-run has it.
Write-Host ''
Write-Host '=== what this build can see ================================'
$devices = & $run --list-devices
$devices | ForEach-Object { Write-Host "  $_" }

if (-not (($devices -join "`n") -match 'Vulkan|CUDA|Metal')) {
    Write-Host ''
    Write-Host '  This build has no GPU backend linked, so every row below would be' -ForegroundColor Yellow
    Write-Host '  the same CPU number. Rebuild with GGML_LIB_DIR pointing at' -ForegroundColor Yellow
    Write-Host '  build-vulkan/ggml/src and run again.' -ForegroundColor Yellow
    exit 1
}

# **One number per row is not a measurement.** Each -ngl is run $Repeat times
# and the best generation figure is kept: the first run of a device path
# compiles shader pipelines inside the timed region, so a single cold run
# understates the GPU and a single warm one is not comparable with it.
function Measure-One($ngl) {
    $best = $null
    for ($i = 0; $i -lt $Repeat; $i++) {
        # **Delete it first.** The log path is derived from the row, so a run
        # that writes nothing leaves the PREVIOUS run's file in place and
        # `Get-Content` reports last time's numbers as this time's. That is how
        # the long-prompt ladder came back with a row identical to the
        # short-prompt ladder's, to the second decimal, and it would have been
        # published as "prompt length makes no difference".
        $log = Join-Path $env:TEMP "ngl-ladder-$ngl-$i.log"
        Remove-Item $log -ErrorAction SilentlyContinue
        $cmd = if ($PromptFile) {
            @($Model, '-f', $PromptFile, '-n', "$Tokens", '--log-file', $log)
        } else {
            @($Model, $Prompt, '-n', "$Tokens", '--log-file', $log)
        }
        if ($ngl -gt 0) { $cmd += @('--device', "$Device", '-ngl', "$ngl") }
        & $run @cmd | Out-Null
        # chaos-run's own timing summary is the source -- parsing it rather than
        # timing the process keeps model load out of the number -- and it goes
        # to a log file rather than through a pipe, for the reason above.
        if (-not (Test-Path $log)) {
            return @{ Failed = $true; Why = 'chaos-run wrote no log -- the run did not start' }
        }
        $out = Get-Content $log -Raw
        $gen = [regex]::Match($out, 'generate[^\n]*?([\d.]+)\s*tok/s')
        $pre = [regex]::Match($out, 'prefill[^\n]*?([\d.]+)\s*tok/s')
        if (-not $gen.Success) {
            $err = ($out -split "`n" | Where-Object { $_ -match '\S' } | Select-Object -Last 3) -join ' / '
            return @{ Failed = $true; Why = $err }
        }
        $g = [double]$gen.Groups[1].Value
        $p = if ($pre.Success) { [double]$pre.Groups[1].Value } else { [double]::NaN }
        if ($null -eq $best -or $g -gt $best.Gen) { $best = @{ Gen = $g; Prefill = $p } }
    }
    return $best
}

Write-Host ''
$what = if ($PromptFile) { "prompt from $(Split-Path $PromptFile -Leaf)" } else { "short prompt" }
Write-Host "=== $Model, $what, -n $Tokens, best of $Repeat ==="
Write-Host ''
Write-Host ('{0,6}  {1,12}  {2,12}  {3,10}' -f 'ngl', 'prefill tok/s', 'gen tok/s', 'vs ngl 0')
$baseline = $null
foreach ($n in $Layers) {
    $r = Measure-One $n
    if ($r.Failed) {
        Write-Host ('{0,6}  {1}' -f $n, "FAILED: $($r.Why)") -ForegroundColor Red
        continue
    }
    if ($null -eq $baseline) { $baseline = $r.Gen }
    $ratio = if ($baseline -gt 0) { '{0:N2}x' -f ($r.Gen / $baseline) } else { '-' }
    Write-Host ('{0,6}  {1,12:N2}  {2,12:N2}  {3,10}' -f $n, $r.Prefill, $r.Gen, $ratio)
}
Write-Host ''
Write-Host 'Rows are comparable to each other and to nothing else. A number from'
Write-Host 'another session, another machine or another build is not a comparison.'
