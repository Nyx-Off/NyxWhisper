# installer\build-all.ps1
# Pipeline complet : compile les 2 backends (CUDA + CPU) puis l'installeur.
# Total : ~30 min selon la machine.
#
# Note : la variante Vulkan est desactivee pour cette version (bug de compilation
# whisper.cpp + vulkan-shaders-gen sur Windows). A reactiver si besoin.

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

function Step($title) {
    Write-Host ""
    Write-Host "============================================" -ForegroundColor Cyan
    Write-Host "  $title" -ForegroundColor Cyan
    Write-Host "============================================" -ForegroundColor Cyan
}

# 1) Build CUDA (target/release/)
Step "1/3  Build CUDA (target/release/)"
& "$repo\scripts\build-cuda.ps1"
if ($LASTEXITCODE -ne 0) { Write-Host "Build CUDA echoue" -ForegroundColor Red; exit 1 }
if (Test-Path "$repo\dist-cuda") { Remove-Item -Recurse -Force "$repo\dist-cuda" }
New-Item -ItemType Directory -Path "$repo\dist-cuda" | Out-Null
Copy-Item "$repo\target\release\NyxWhisper.exe" "$repo\dist-cuda\"
Get-ChildItem "$repo\target\release\*.dll" -ErrorAction SilentlyContinue | ForEach-Object {
    Copy-Item $_.FullName "$repo\dist-cuda\"
}

# 2) Build CPU (target-cpu/)
Step "2/3  Build CPU (target-cpu/)"
$env:CARGO_TARGET_DIR = "$repo\target-cpu"
$env:PATH = "$env:ProgramFiles\CMake\bin;$env:ProgramFiles\LLVM\bin;$env:PATH"
if (-not $env:LIBCLANG_PATH) { $env:LIBCLANG_PATH = "$env:ProgramFiles\LLVM\bin" }
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Host "Build CPU echoue" -ForegroundColor Red; exit 1 }
Remove-Item Env:CARGO_TARGET_DIR
if (Test-Path "$repo\dist-cpu") { Remove-Item -Recurse -Force "$repo\dist-cpu" }
New-Item -ItemType Directory -Path "$repo\dist-cpu" | Out-Null
Copy-Item "$repo\target-cpu\release\NyxWhisper.exe" "$repo\dist-cpu\"

# 3) Build installer
Step "3/3  Compilation de l'installeur"
& "$repo\installer\build-installer.ps1"
if ($LASTEXITCODE -ne 0) { Write-Host "Build installeur echoue" -ForegroundColor Red; exit 1 }

Write-Host ""
Write-Host "TERMINE" -ForegroundColor Green
Write-Host "Installeur : installer\out\NyxWhisper-Setup-*.exe"
