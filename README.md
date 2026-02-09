# Jarvis (Windows Desktop, Rust)

## Prerequisites (Windows)

Run in PowerShell (preferably elevated for installs):

```powershell
winget install -e --id Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --force --override "--wait --passive --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"
```

Notes:
- This app is Rust + `eframe/egui` (not Tauri).
- WebView2 is not required for this app.

## Verify toolchain

```powershell
rustc --version
cargo --version
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
- `target\release\jarvis.exe`

## Build installer (Inno Setup)

Install Inno Setup 6, then:

```powershell
.\scripts\build-installer.ps1 -BuildName local-test1
```

Output:
- `dist\Jarvis-Setup-<version>-<buildname>.exe`

Default install path:
- `%LOCALAPPDATA%\Programs\Jarvis`

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

