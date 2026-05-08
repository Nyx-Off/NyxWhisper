# scripts\build-cpu.ps1
# Compile NyxWhisper sans backend GPU (CPU uniquement, AVX2).
# C'est le build le plus rapide a compiler.
#
# Prerequis : LLVM (libclang.dll), CMake, MSVC Build Tools
# Lance scripts\setup.ps1 si manquants.

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

if (-not $env:LIBCLANG_PATH -and (Test-Path "$env:ProgramFiles\LLVM\bin\libclang.dll")) {
    $env:LIBCLANG_PATH = "$env:ProgramFiles\LLVM\bin"
}
$env:PATH = "$env:ProgramFiles\CMake\bin;$env:ProgramFiles\LLVM\bin;$env:PATH"

Write-Host "[build-cpu] cargo build --release (CPU only)" -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "OK -> target\release\NyxWhisper.exe (CPU only)" -ForegroundColor Green
