# Setup (Windows - Tauri App)

This project is a Windows-first Tauri app. Follow these steps to build and run it.

## Prerequisites
1. **Rust toolchain** (stable)
2. **Visual Studio Build Tools** — "Desktop development with C++" and Windows 10/11 SDK
3. **WebView2 Runtime**

## Install (Windows PowerShell)

Run these in an **elevated PowerShell** window.

### 1) Rust (rustup)
```powershell
winget install -e --id Rustlang.Rustup
```

### 2) Visual Studio Build Tools (C++ + Windows SDK)

Windows 11:
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --force --override "--wait --passive --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"
```

Windows 10:
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --force --override "--wait --passive --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows10SDK"
```

### 3) WebView2 Runtime
```powershell
winget install -e --id Microsoft.EdgeWebView2Runtime
```

### 4) Tauri CLI
```powershell
cargo install tauri-cli
```

## Verify Installs
```powershell
rustc --version
cargo --version
cargo tauri --version
```

## Run the App (dev)
```powershell
cargo tauri dev
```

## Configuration

1. Launch the app
2. Click **Settings**
3. Enter your **OpenAI API key**

The app uses OpenAI's Realtime API for speech-to-text. You'll need an API key with access to the `gpt-4o-realtime-preview` model.

## Important Notes

- **WSL2 users**: You MUST run `cargo tauri dev` from a native Windows terminal (PowerShell or cmd), not from the WSL shell. Tauri needs direct access to Windows APIs.
- **Microphone access**: Windows will prompt for microphone permission on first use. Grant access when prompted.
- The UI is served from `src/`.
- Rust backend lives in `src-ui/`.

## Troubleshooting

**"MSVC not found" or linker errors**
- Ensure Visual Studio Build Tools installed correctly
- Try running from "x64 Native Tools Command Prompt for VS 2022"

**WebView2 errors**
- Reinstall: `winget install -e --id Microsoft.EdgeWebView2Runtime --force`

**Microphone not working**
- Check Windows Settings → Privacy → Microphone → allow desktop apps

**"API key invalid" or connection errors**
- Verify your OpenAI key in Settings
- Ensure your key has access to the Realtime API
