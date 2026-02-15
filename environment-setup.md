# Mango Chat (Windows Desktop, Rust)

## Prerequisites (Windows)

Run in PowerShell (preferably elevated for installs):

```powershell
winget install -e --id Rustlang.Rustup
# Windows 11 Build Tools + SDK:
winget install Microsoft.VisualStudio.2022.BuildTools --force --override "--wait --passive --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"
winget install -e --id JRSoftware.InnoSetup
```

Notes:
- This app is Rust + `eframe/egui`.
- The Visual Studio command above is for Windows 11 SDK. Windows 10 uses a different Build Tools SDK selection.

## Verify toolchain

```powershell
rustc --version
cargo --version
# Visual Studio C++ tools installation path:
& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
# Inno Setup compiler:
& "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" /?
```

## Run locally

From repo root:

```powershell
cargo check
cargo run
```

## Build EXE

```powershell
cargo build --release
```

Output:
- `target\release\mangochat.exe`

## Build installer (Inno Setup)

```powershell
.\scripts\build-installer.ps1 -BuildName local-test1
```

Output:
- `dist\MangoChat-Setup-<version>-<buildname>.exe`

Default install path:
- `%LOCALAPPDATA%\Programs\MangoChat`

Uninstall behavior:
- removes app binaries/shortcuts
- keeps user data

## GitHub Releases

Workflow:
- `.github/workflows/release-windows.yml`

Trigger:
- push a tag like `v0.1.0`

Release assets:
- installer `.exe`
- `SHA256SUMS.txt`

