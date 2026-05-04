# Mango Chat (Windows Desktop, Rust)

## Prerequisites (Windows)

Run in PowerShell (preferably elevated for installs):

```powershell
winget install -e --id Rustlang.Rustup
# Windows 11 Build Tools + SDK:
winget install Microsoft.VisualStudio.2022.BuildTools --force --override "--wait --passive --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"
winget install -e --id JRSoftware.InnoSetup
winget install LLVM.LLVM
winget install Kitware.CMake
```

Notes:
- This app is Rust + `eframe/egui`.
- The Visual Studio command above is for Windows 11 SDK. Windows 10 uses a different Build Tools SDK selection.
- LLVM and CMake are required for native local Whisper builds.

## Verify toolchain

```powershell
rustc --version
cargo --version
# Visual Studio C++ tools installation path:
& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
# Inno Setup compiler:
& "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" /?
# Native local Whisper build helpers:
& "C:\Program Files\LLVM\bin\clang.exe" --version
& "C:\Program Files\CMake\bin\cmake.exe" --version
```

## Run locally

From repo root:

```powershell
cargo check
cargo run
```

If the native local Whisper build does not pick up LLVM or CMake automatically in your shell, set:

```powershell
$env:LIBCLANG_PATH='C:\Program Files\LLVM\bin'
$env:CMAKE='C:\Program Files\CMake\bin\cmake.exe'
cargo check
cargo run
```

## Local Whisper assets

The local transcription mode expects the bundled Whisper model file at:

- `.models\whispercpp\ggml-base.en-q5_1.bin`

Cloud mode does not depend on that local model file.

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

## Optional Development Diagnostics

There is a development-only feature flag for session-level logging:

```powershell
cargo run --features dev-session-capture
```

That feature is intended for development and evaluation only, not normal customer builds.

