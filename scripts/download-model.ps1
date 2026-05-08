# Telechargement d'un modele Whisper GGML depuis Hugging Face
# Usage : .\scripts\download-model.ps1 -Model small
#
# Tailles disponibles : tiny, base, small, medium, large-v3
# Recommandation FR : "small" (rapide, bonne qualite) ou "medium" (meilleure qualite, plus lent)

param(
    [ValidateSet("tiny", "base", "small", "medium", "large-v3")]
    [string]$Model = "small"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$modelsDir = Join-Path $repoRoot "models"
if (-not (Test-Path $modelsDir)) {
    New-Item -ItemType Directory -Path $modelsDir | Out-Null
}

$fileName = "ggml-$Model.bin"
$destPath = Join-Path $modelsDir $fileName

if (Test-Path $destPath) {
    Write-Host "Le modele existe deja : $destPath" -ForegroundColor Yellow
    Write-Host "Supprime-le manuellement si tu veux le re-telecharger."
    exit 0
}

$url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$fileName"

Write-Host "Telechargement de $fileName depuis Hugging Face..." -ForegroundColor Cyan
Write-Host "URL : $url"
Write-Host "Destination : $destPath"
Write-Host ""

try {
    # BITS = barre de progression robuste sur Windows
    Start-BitsTransfer -Source $url -Destination $destPath -DisplayName "Whisper $Model"
} catch {
    Write-Host "BITS a echoue, repli sur Invoke-WebRequest..." -ForegroundColor Yellow
    $ProgressPreference = "Continue"
    Invoke-WebRequest -Uri $url -OutFile $destPath -UseBasicParsing
}

$sizeMB = [math]::Round((Get-Item $destPath).Length / 1MB, 1)
Write-Host ""
Write-Host "OK - Modele telecharge ($sizeMB Mo) : $destPath" -ForegroundColor Green
Write-Host ""
Write-Host "Tu peux maintenant lancer :" -ForegroundColor Cyan
Write-Host "  cargo run --release -- --model models/$fileName"
