# scripts\build-vulkan.ps1
# Compile NyxWhisper avec acceleration GPU via Vulkan.
# Marche sur NVIDIA / AMD / Intel ; legerement moins rapide que CUDA sur NVIDIA
# mais bien plus universel et leger a installer.
#
# Prerequis :
#   - GPU avec drivers Vulkan (deja le cas sur tout PC moderne)
#   - Vulkan SDK : winget install --id KhronosGroup.VulkanSDK
#   - LLVM, CMake, MSVC Build Tools (voir scripts\setup.ps1)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

# Detection Vulkan SDK
$vkPath = $env:VULKAN_SDK
if (-not $vkPath) {
    $vkCandidates = Get-ChildItem "C:\VulkanSDK" -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending
    if ($vkCandidates) { $vkPath = $vkCandidates[0].FullName }
}
if (-not $vkPath -or -not (Test-Path "$vkPath\Bin\glslangValidator.exe")) {
    Write-Host "ERREUR : Vulkan SDK introuvable." -ForegroundColor Red
    Write-Host "Installe-le : winget install --id KhronosGroup.VulkanSDK"
    exit 1
}
Write-Host "[build-vulkan] Vulkan SDK detecte : $vkPath" -ForegroundColor Green

if (-not $env:LIBCLANG_PATH -and (Test-Path "$env:ProgramFiles\LLVM\bin\libclang.dll")) {
    $env:LIBCLANG_PATH = "$env:ProgramFiles\LLVM\bin"
}
$env:VULKAN_SDK = $vkPath
$env:PATH = "$vkPath\Bin;$env:ProgramFiles\CMake\bin;$env:ProgramFiles\LLVM\bin;$env:PATH"

Write-Host "[build-vulkan] cargo build --release --features vulkan" -ForegroundColor Cyan
Write-Host "(1ere compilation : 10-20 min, patience...)"
cargo build --release --features vulkan
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "OK -> target\release\NyxWhisper.exe (Vulkan)" -ForegroundColor Green
