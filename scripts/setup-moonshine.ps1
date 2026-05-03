param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$python = Join-Path $root ".venv\Scripts\python.exe"
if (-not (Test-Path $python)) {
    Write-Host "Creating virtual environment..." -ForegroundColor Cyan
    python -m venv .venv
}

Write-Host "Installing moonshine-voice into .venv..." -ForegroundColor Cyan
& $python -m pip install --upgrade pip moonshine-voice

$modelsRoot = Join-Path $root ".models"
New-Item -ItemType Directory -Force -Path $modelsRoot | Out-Null

$downloads = @(
    @{ Name = "tiny-en"; Args = @("--language", "en", "--model-arch", "0", "--root", ".models") },
    @{ Name = "tiny-streaming-en"; Args = @("--language", "en", "--model-arch", "2", "--root", ".models") }
)

foreach ($download in $downloads) {
    Write-Host ("Downloading {0}..." -f $download.Name) -ForegroundColor Cyan
    & $python -m moonshine_voice.download @($download.Args)
}

Write-Host ""
Write-Host "Moonshine setup complete." -ForegroundColor Green
Write-Host "Run .\\scripts\\smoke-test-moonshine.ps1 to validate the local runtime." -ForegroundColor Green
