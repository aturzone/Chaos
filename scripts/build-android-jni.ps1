<#
.SYNOPSIS
Cross-compile the JNI bridge and put it where Gradle will package it.

.DESCRIPTION
`android/jni` is a cdylib. Built for an Android ABI it becomes
`libchaos_android.so`, which `Engine.kt` loads with `System.loadLibrary`. The
result is **not committed** — it is 490 KB of build output per ABI, and a binary
in the tree is one nobody can review.

**The app works without it**, as a client. `Engine.available` is false, the
device line falls back to Android's own reading, and nothing crashes.

# Two traps, both paid for

**1. Do not use the NDK's `.cmd` wrapper as the linker for a cdylib.** rustc
passes `--version-script=<path>` when linking a cdylib, to control which symbols
are exported. cmd.exe mangles that argument and the link dies with

    --version-script=...\list"" was unexpected at this time.

which names neither Rust nor the NDK. The executables built earlier were fine
because they never get that flag. So the linker here is the real `clang.exe`
with an explicit `--target=`, and only `CC_*` (used by build scripts, which do
not pass `--version-script`) keeps the wrapper.

**2. ggml must be built for the same ABI**, and `GGML_LIB_DIR` must point at
that build rather than the host one. Pointing it at the host's archives links
x86_64 Windows objects into an Android library and fails with a page of
architecture mismatches.

.PARAMETER Ndk
The NDK root. Default is where this project keeps it.

.PARAMETER Abis
Which ABIs to build. `arm64-v8a` is every phone worth caring about; `x86_64` is
the emulator, which is the only way to test this without a handset.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File scripts/build-android-jni.ps1
#>
[CmdletBinding()]
param(
    [string]$Ndk = 'C:/Projects/android-sdk/ndk/android-ndk-r26d',
    [string]$LlamaCpp = 'C:/Projects/llamacpp-unsloth',
    [ValidateSet('arm64-v8a', 'x86_64')]
    [string[]]$Abis = @('arm64-v8a', 'x86_64'),
    [int]$ApiLevel = 28
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$bin = Join-Path $Ndk 'toolchains/llvm/prebuilt/windows-x86_64/bin'
if (-not (Test-Path (Join-Path $bin 'clang.exe'))) {
    throw "no NDK clang at $bin -- pass -Ndk with the right root"
}

# ABI -> (rust target, ggml build directory, cmake ANDROID_ABI)
$map = @{
    'arm64-v8a' = @{ Target = 'aarch64-linux-android'; Build = 'build-android' }
    'x86_64'    = @{ Target = 'x86_64-linux-android'; Build = 'build-android-x64' }
}

foreach ($abi in $Abis) {
    $m = $map[$abi]
    $target = $m.Target
    $ggml = Join-Path $LlamaCpp "$($m.Build)/ggml/src"

    Write-Host ''
    Write-Host "=== $abi ($target) ===================================="

    if (-not (Test-Path (Join-Path $ggml 'libggml-base.a'))) {
        Write-Host "  ggml is not built for $abi." -ForegroundColor Yellow
        Write-Host "  cmake -S $LlamaCpp -B $LlamaCpp/$($m.Build) -G Ninja ``" -ForegroundColor Yellow
        Write-Host "    -DCMAKE_TOOLCHAIN_FILE=$Ndk/build/cmake/android.toolchain.cmake ``" -ForegroundColor Yellow
        Write-Host "    -DANDROID_ABI=$abi -DANDROID_PLATFORM=android-$ApiLevel ``" -ForegroundColor Yellow
        Write-Host "    -DCMAKE_BUILD_TYPE=Release -DGGML_OPENMP=OFF -DBUILD_SHARED_LIBS=OFF" -ForegroundColor Yellow
        Write-Host "  cmake --build $LlamaCpp/$($m.Build) --target ggml-base ggml-cpu ggml" -ForegroundColor Yellow
        throw "ggml missing for $abi"
    }

    # `build.rs` looks for `ggml-base.a`; cmake writes `libggml-base.a`.
    foreach ($lib in 'ggml-base', 'ggml-cpu', 'ggml') {
        $src = Join-Path $ggml "lib$lib.a"
        $dst = Join-Path $ggml "$lib.a"
        if ((Test-Path $src) -and -not (Test-Path $dst)) { Copy-Item $src $dst }
    }

    $upper = $target.ToUpper().Replace('-', '_')
    $under = $target.Replace('-', '_')
    $env:GGML_LIB_DIR = $ggml
    # The real clang, not the .cmd wrapper -- see the header.
    Set-Item "env:CARGO_TARGET_${upper}_LINKER" (Join-Path $bin 'clang.exe')
    Set-Item "env:CARGO_TARGET_${upper}_RUSTFLAGS" "-Clink-arg=--target=$target$ApiLevel"
    Set-Item "env:CC_$under" (Join-Path $bin "$target$ApiLevel-clang.cmd")
    Set-Item "env:AR_$under" (Join-Path $bin 'llvm-ar.exe')

    Push-Location $root
    try {
        & cargo build --release --target $target -p chaos-android
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed for $abi" }
    } finally {
        Pop-Location
    }

    $so = Join-Path $root "target/$target/release/libchaos_android.so"
    if (-not (Test-Path $so)) { throw "no .so produced for $abi" }
    $out = Join-Path $root "android/app/src/main/jniLibs/$abi"
    New-Item -ItemType Directory -Force $out | Out-Null
    Copy-Item $so (Join-Path $out 'libchaos_android.so') -Force
    $kb = [int]((Get-Item $so).Length / 1KB)
    Write-Host ("  {0,-12} {1} KB -> jniLibs/{2}/" -f $target, $kb, $abi)
}

Write-Host ''
Write-Host 'Now build the APK. The engine line in the app should read'
Write-Host '"engine <version> on this phone: ..." rather than Android''s own'
Write-Host 'description -- that is how you know the library was packaged.'
