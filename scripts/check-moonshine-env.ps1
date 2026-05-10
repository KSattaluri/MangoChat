param()

$ErrorActionPreference = "Stop"

function Write-Check($label, $ok, $detail) {
    $status = if ($ok) { "OK" } else { "MISSING" }
    $color = if ($ok) { "Green" } else { "Yellow" }
    Write-Host ("[{0}] {1} - {2}" -f $status, $label, $detail) -ForegroundColor $color
}

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$python = Get-Command python -ErrorAction SilentlyContinue
Write-Check "Python" ($null -ne $python) ($(if ($python) { (& python --version) } else { "python not found on PATH" }))

$cmake = Get-Command cmake -ErrorAction SilentlyContinue
$cmakePath = if ($cmake) {
    $cmake.Source
} else {
    $default = "C:\Program Files\CMake\bin\cmake.exe"
    if (Test-Path $default) { $default } else { $null }
}
$cmakeVersion = if ($cmakePath) { (& $cmakePath --version | Select-Object -First 1) } else { "cmake not found on PATH or default install location" }
Write-Check "CMake" (![string]::IsNullOrWhiteSpace($cmakePath)) $cmakeVersion

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$vsPath = $null
if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -property installationPath
}
Write-Check "VS Build Tools" (![string]::IsNullOrWhiteSpace($vsPath)) ($(if ($vsPath) { $vsPath } else { "Visual Studio Build Tools not found via vswhere" }))

$msbuild = Get-Command msbuild -ErrorAction SilentlyContinue
$msbuildPath = if ($msbuild) {
    $msbuild.Source
} elseif ($vsPath) {
    $candidates = Get-ChildItem $vsPath -Recurse -Filter msbuild.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($candidates) { $candidates.FullName } else { $null }
} else {
    $null
}
Write-Check "MSBuild" (![string]::IsNullOrWhiteSpace($msbuildPath)) ($(if ($msbuildPath) { $msbuildPath } else { "msbuild not found on PATH or under Visual Studio Build Tools" }))

$modelDir = Join-Path $root ".models"
$nativeBuildDir = Join-Path $root ".native-build"
New-Item -ItemType Directory -Force -Path $modelDir | Out-Null
New-Item -ItemType Directory -Force -Path $nativeBuildDir | Out-Null

Write-Host ""
Write-Host "Reserved local directories:" -ForegroundColor Cyan
Write-Host "  $modelDir"
Write-Host "  $nativeBuildDir"

Write-Host ""
Write-Host "Moonshine branch prep status:" -ForegroundColor Cyan
if ($python -and $vsPath) {
    Write-Host "  Usable for research and model download." -ForegroundColor Green
} else {
    Write-Host "  Missing required tooling for a native Moonshine build." -ForegroundColor Yellow
}

if (-not $cmake) {
    Write-Host "  CMake is installed but this terminal may need to be restarted before PATH picks it up." -ForegroundColor Yellow
}
