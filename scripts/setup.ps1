# scripts\setup.ps1
# Installe les prerequis pour compiler NyxWhisper sur Windows :
#   - Rust (rustup)
#   - LLVM (libclang.dll, requis par bindgen pour whisper-rs-sys)
#   - CMake (requis pour compiler whisper.cpp)
#   - Visual Studio Build Tools 2022 avec workload C++ (requis par MSVC)
#
# Usage : powershell -ExecutionPolicy Bypass -File .\scripts\setup.ps1

$ErrorActionPreference = "Stop"

function Test-Cmd($name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

function Install-WingetPackage($id, $displayName) {
    Write-Host "[setup] $displayName : installation via winget ($id)..." -ForegroundColor Cyan
    winget install --id $id --silent --accept-package-agreements --accept-source-agreements
}

if (-not (Test-Cmd "winget")) {
    Write-Host "winget est requis (livré avec App Installer). Installe-le depuis le Microsoft Store." -ForegroundColor Red
    exit 1
}

# ---- Rust ----
if (Test-Cmd "cargo") {
    Write-Host "[setup] Rust deja installe : $(cargo --version)" -ForegroundColor Green
} else {
    Install-WingetPackage "Rustlang.Rustup" "Rustup (Rust)"
    Write-Host "Pense a redemarrer le terminal pour avoir cargo dans le PATH." -ForegroundColor Yellow
}

# ---- Visual Studio Build Tools (workload C++) ----
$vsBuildToolsDetected = $false
$vsWherePath = "$env:ProgramFiles(x86)\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vsWherePath) {
    $vsList = & $vsWherePath -products * -requires Microsoft.VisualStudio.Workload.VCTools -format value -property installationPath
    if ($vsList) { $vsBuildToolsDetected = $true }
}
if ($vsBuildToolsDetected) {
    Write-Host "[setup] VS Build Tools (C++) deja installes." -ForegroundColor Green
} else {
    Install-WingetPackage "Microsoft.VisualStudio.2022.BuildTools" "Visual Studio Build Tools 2022"
    Write-Host "IMPORTANT : ouvre 'Visual Studio Installer' et coche le workload" -ForegroundColor Yellow
    Write-Host "  'Desktop development with C++' s'il n'est pas deja active." -ForegroundColor Yellow
}

# ---- CMake ----
if (Test-Cmd "cmake") {
    Write-Host "[setup] CMake deja installe : $((cmake --version | Select-Object -First 1))" -ForegroundColor Green
} else {
    Install-WingetPackage "Kitware.CMake" "CMake"
}

# ---- LLVM (libclang.dll) ----
$hasLibClang = $false
foreach ($p in @(
    "$env:ProgramFiles\LLVM\bin\libclang.dll",
    "${env:ProgramFiles(x86)}\LLVM\bin\libclang.dll"
)) {
    if (Test-Path $p) { $hasLibClang = $true; $env:LIBCLANG_PATH = (Split-Path $p); break }
}
if ($hasLibClang) {
    Write-Host "[setup] LLVM/libclang deja present : $env:LIBCLANG_PATH" -ForegroundColor Green
} else {
    Install-WingetPackage "LLVM.LLVM" "LLVM"
    $defaultLLVM = "$env:ProgramFiles\LLVM\bin"
    if (Test-Path "$defaultLLVM\libclang.dll") {
        [Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $defaultLLVM, "User")
        Write-Host "[setup] LIBCLANG_PATH defini sur $defaultLLVM (variable utilisateur)." -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "=========================================================" -ForegroundColor Green
Write-Host "  Setup termine. FERME ET ROUVRE TON TERMINAL pour que" -ForegroundColor Green
Write-Host "  les variables d'environnement soient prises en compte." -ForegroundColor Green
Write-Host "=========================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Etapes suivantes :" -ForegroundColor Cyan
Write-Host "  1) .\scripts\download-model.ps1 -Model small"
Write-Host "  2) .\scripts\build-cpu.ps1"
Write-Host "  3) target\release\NyxWhisper.exe"
