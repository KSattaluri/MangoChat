# Mango Chat

Free, open-source, lightweight voice dictation for Windows.

Mango Chat is a native Rust desktop app that streams microphone audio to your selected speech-to-text provider for low-latency transcription.

## Highlights

- Native Windows app (Rust + egui), low memory footprint
- Multi-provider speech-to-text support:
  - OpenAI Realtime
  - Deepgram
  - ElevenLabs Realtime
  - AssemblyAI
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
3. Select a provider.
4. Paste your API key.
5. Click `Verify`.
6. Click `Save`.

API keys are encrypted with Windows DPAPI and stored locally.

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

### Run locally

```powershell
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
