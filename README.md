# Mango Chat

Free, open-source, lightweight voice dictation for Windows.

Mango Chat is a native Rust desktop app for Windows dictation and speech-driven commands. It supports both cloud transcription providers and a local Whisper.cpp path.

## Highlights

- Native Windows app (Rust + egui), low memory footprint
- Cloud speech-to-text providers:
  - OpenAI Realtime
  - Deepgram
  - ElevenLabs Realtime
  - AssemblyAI
- Local speech-to-text option:
  - Whisper.cpp
- Local VAD (voice activity detection) to suppress silence before upload
- Built-in + custom voice commands
- Screenshot/snip workflow with clipboard modes
- Per-provider API keys encrypted at rest (Windows DPAPI)
- No built-in telemetry

## Download

Download the latest Windows installer from Releases:

- https://github.com/KSattaluri/MangoChat/releases/latest

## Installation

1. Download `MangoChat-Setup-<version>.exe` from the latest release.
2. Run the installer (no admin rights required).
3. Complete setup and launch Mango Chat.

Install location is per-user under `%LOCALAPPDATA%\Programs\MangoChat`.

## Quick Configuration

1. Open Settings (gear icon).
2. Go to `Provider`.
3. Choose `Cloud` or `Local`.
4. If using `Cloud`, select a provider, paste your API key, and click `Verify`.
5. Click `Save`.

API keys are encrypted with Windows DPAPI and stored locally. Local Whisper mode does not require a provider key.

## Provider Cost Notes

Mango Chat is free. You only pay your speech provider.

Deepgram and AssemblyAI often provide trial credits (commonly up to a combined $250) that can be used without a credit card at signup, depending on current provider policies.

## FAQ

See the full FAQ here:

- [`FAQ.md`](FAQ.md)

## Development

### Prerequisites

- Windows 10/11
- Rust stable toolchain
- Visual Studio C++ build tools
- Inno Setup for installer builds

If you want to build the native local Whisper path from source, also install:

- LLVM (`libclang.dll`)
- CMake

### Run locally

```powershell
cargo run
```

For the native local Whisper path on Windows, this shell may also need:

```powershell
$env:LIBCLANG_PATH='C:\Program Files\LLVM\bin'
$env:CMAKE='C:\Program Files\CMake\bin\cmake.exe'
cargo run
```

### Build release binary

```powershell
cargo build --release
```

### Build installer

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-installer.ps1
```

## License

MIT. See [`LICENSE`](LICENSE).
