# Cross-compile FerritePLC for Linux (x86_64-unknown-linux-gnu) from Windows.
#
# Prereqs (one-time):
#   rustup target add x86_64-unknown-linux-gnu
#   cargo install cargo-zigbuild
#   zig on PATH (https://ziglang.org/download/), set up below if missing
#
# Usage:  pwsh -File scripts/build-linux.ps1 [-Profile release|debug]
param(
    [ValidateSet('release', 'debug')]
    [string]$Profile = 'release'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# --- locate zig -------------------------------------------------------------
if (-not (Get-Command zig -ErrorAction SilentlyContinue)) {
    $zigDirs = @()
    $zigDirs += Get-ChildItem (Join-Path $env:USERPROFILE '.local\zig') -Directory -Filter 'zig-*-windows-*' -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | ForEach-Object { $_.FullName }
    $zigDirs += 'C:\zig', 'C:\Program Files\zig'

    $zig = $zigDirs | Where-Object { Test-Path (Join-Path $_ 'zig.exe') } | Select-Object -First 1
    if (-not $zig) {
        throw 'zig not found. Download it from https://ziglang.org/download/ and add it to PATH.'
    }
    $env:Path = "$env:Path;$zig"
    Write-Host "using zig: $zig"
}

if (-not (Get-Command cargo-zigbuild -ErrorAction SilentlyContinue)) {
    throw 'cargo-zigbuild not found. Run: cargo install cargo-zigbuild'
}

# --- build ------------------------------------------------------------------
$cargoArgs = @('zigbuild', '--workspace', '--bins', '--target', 'x86_64-unknown-linux-gnu')
if ($Profile -eq 'release') { $cargoArgs += '--release' }

& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { throw "cargo zigbuild failed with exit code $LASTEXITCODE" }

$outDir = Join-Path $root "target\x86_64-unknown-linux-gnu\$Profile"
Get-ChildItem $outDir -File | Where-Object { -not $_.Extension } |
    Select-Object Name, @{n = 'MB'; e = { [math]::Round($_.Length / 1MB, 1) } } |
    Format-Table -AutoSize
