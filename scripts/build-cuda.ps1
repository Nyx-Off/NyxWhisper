# scripts\build-cuda.ps1
# Compile NyxWhisper avec accélération GPU NVIDIA (CUDA).
# C'est le build le plus rapide a l'execution sur RTX / GTX recentes.
#
# Prerequis :
#   - GPU NVIDIA + drivers a jour
#   - CUDA Toolkit 12.x : winget install --id Nvidia.CUDA
#   - LLVM, CMake, MSVC Build Tools (voir scripts\setup.ps1)
#
# La 1ere compilation prend 10-30 minutes (whisper.cpp avec backend CUDA).

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

# Detection CUDA
$cudaPath = $env:CUDA_PATH
if (-not $cudaPath) {
    $cudaCandidates = Get-ChildItem "$env:ProgramFiles\NVIDIA GPU Computing Toolkit\CUDA" -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending
    if ($cudaCandidates) { $cudaPath = $cudaCandidates[0].FullName }
}
if (-not $cudaPath -or -not (Test-Path "$cudaPath\bin\nvcc.exe")) {
    Write-Host "ERREUR : CUDA Toolkit introuvable." -ForegroundColor Red
    Write-Host "Installe-le : winget install --id Nvidia.CUDA"
    exit 1
}
Write-Host "[build-cuda] CUDA detecte : $cudaPath" -ForegroundColor Green

if (-not $env:LIBCLANG_PATH -and (Test-Path "$env:ProgramFiles\LLVM\bin\libclang.dll")) {
    $env:LIBCLANG_PATH = "$env:ProgramFiles\LLVM\bin"
}
$env:CUDA_PATH = $cudaPath
$env:PATH = "$cudaPath\bin;$env:ProgramFiles\CMake\bin;$env:ProgramFiles\LLVM\bin;$env:PATH"

Write-Host "[build-cuda] cargo build --release --features cuda" -ForegroundColor Cyan
Write-Host "(1ere compilation : 10-30 min, patience...)"
cargo build --release --features cuda
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Copie des DLLs runtime CUDA a cote du .exe (CUDA 13.x les place dans bin\x64).
$dllSrc = "$cudaPath\bin\x64"
if (-not (Test-Path "$dllSrc\cudart64_13.dll")) { $dllSrc = "$cudaPath\bin" }
$exeDir = Join-Path $repo "target\release"
$dlls = @("cudart64_13.dll", "cublas64_13.dll", "cublasLt64_13.dll")
foreach ($dll in $dlls) {
    $src = Join-Path $dllSrc $dll
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination $exeDir -Force
        Write-Host "[build-cuda] copie $dll" -ForegroundColor DarkGray
    } else {
        Write-Host "[build-cuda] ATTENTION : $dll introuvable dans $dllSrc" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "OK -> target\release\NyxWhisper.exe (CUDA)" -ForegroundColor Green
Write-Host "Les DLLs CUDA ont ete copiees a cote du .exe pour le rendre portable."
