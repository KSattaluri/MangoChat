param(
    [string]$Version,
    [string]$BuildName
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $Version -or $Version.Trim().Length -eq 0) {
    $cargoToml = Get-Content "$root\Cargo.toml" -Raw
    $m = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $m.Success) {
        throw "Could not determine version from Cargo.toml"
    }
    $Version = $m.Groups[1].Value
}

if (-not $BuildName -or $BuildName.Trim().Length -eq 0) {
    $BuildName = "local-" + (Get-Date -Format "yyyyMMdd-HHmmss")
}

# Inno preprocessor macro safe value
$BuildName = ($BuildName -replace '[^A-Za-z0-9._-]', '-')

Write-Host "Building jarvis.exe (release)..." -ForegroundColor Cyan
cargo build --release

$iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $iscc)) {
    throw "Inno Setup not found at '$iscc'. Install Inno Setup 6 first."
}

$exePath = Join-Path $root "target\release\jarvis.exe"
if (-not (Test-Path $exePath)) {
    throw "Missing $exePath"
}

Write-Host "Packaging installer v$Version ($BuildName)..." -ForegroundColor Cyan
& $iscc "$root\installer\Jarvis.iss" "/DMyAppVersion=$Version" "/DBuildName=$BuildName" "/DMyAppExe=$exePath"
if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed with exit code $LASTEXITCODE" }

Write-Host "Done. Installer output is in $root\dist" -ForegroundColor Green


