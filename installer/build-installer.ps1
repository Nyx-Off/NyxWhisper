# scripts\..\installer\build-installer.ps1
# Compile l'installeur NyxWhisper avec Inno Setup.
# Prerequis : avoir compile au prealable les 2 variantes :
#   .\scripts\build-cuda.ps1
#   .\scripts\build-cpu.ps1
# Puis avoir copie chaque NyxWhisper.exe (+ DLLs CUDA) dans :
#   dist-cuda\ , dist-cpu\

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

# Detection Inno Setup (machine-wide ou per-user)
$iscc = $null
foreach ($p in @(
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
)) {
    if (Test-Path $p) { $iscc = $p; break }
}
if (-not $iscc) {
    Write-Host "ERREUR : Inno Setup 6 introuvable. Installe-le :" -ForegroundColor Red
    Write-Host "  winget install --id JRSoftware.InnoSetup"
    exit 1
}
Write-Host "[installer] Inno Setup : $iscc" -ForegroundColor Green

# Verifications des dist
$missing = @()
foreach ($d in @("dist-cuda", "dist-cpu")) {
    if (-not (Test-Path "$repo\$d\NyxWhisper.exe")) { $missing += $d }
}
if ($missing.Count -gt 0) {
    Write-Host "ATTENTION : binaires manquants pour les variantes : $($missing -join ', ')" -ForegroundColor Yellow
    Write-Host "L'installeur sera incomplet." -ForegroundColor Yellow
}

if (-not (Test-Path "$repo\installer\out")) {
    New-Item -ItemType Directory -Path "$repo\installer\out" | Out-Null
}

Write-Host "[installer] Compilation..." -ForegroundColor Cyan
& $iscc "$repo\installer\NyxWhisper.iss"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$out = Get-ChildItem "$repo\installer\out\NyxWhisper-Setup-*.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($out) {
    $sizeMB = [math]::Round($out.Length / 1MB, 1)
    Write-Host ""
    Write-Host "OK -> $($out.FullName) ($sizeMB Mo)" -ForegroundColor Green
}
