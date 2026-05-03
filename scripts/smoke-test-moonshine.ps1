param(
    [ValidateSet("tiny-en", "tiny-streaming-en")]
    [string]$Model = "tiny-en"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$python = Join-Path $root ".venv\Scripts\python.exe"
if (-not (Test-Path $python)) {
    throw "Missing .venv. Run .\scripts\setup-moonshine.ps1 first."
}

switch ($Model) {
    "tiny-en" {
        $modelPath = ".models\download.moonshine.ai\model\tiny-en\quantized\tiny-en"
        $modelArch = "0"
    }
    "tiny-streaming-en" {
        $modelPath = ".models\download.moonshine.ai\model\tiny-streaming-en\quantized"
        $modelArch = "2"
    }
}

if (-not (Test-Path $modelPath)) {
    throw "Missing model path: $modelPath"
}

Write-Host ("Running Moonshine smoke test for {0}..." -f $Model) -ForegroundColor Cyan
& $python -m moonshine_voice.transcriber --model-path $modelPath --model-arch $modelArch --quiet
