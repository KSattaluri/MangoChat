# Debugging

## Purpose

This document captures the current development-only debugging workflow for offline Whisper work on `feature/moonshine`.

The goal is to keep instrumentation available during development without shipping it in customer builds.

## Production vs Dev Builds

The repository now uses a compile-time feature flag for session-level debugging:

- default build: no dev session capture
- `dev-session-capture` build: enables per-recording session logs and Whisper native log routing

Default customer build:

```powershell
cargo run
```

Development build with session capture:

```powershell
$env:LIBCLANG_PATH='C:\Program Files\LLVM\bin'
$env:CMAKE='C:\Program Files\CMake\bin\cmake.exe'
cargo run --features dev-session-capture
```

## Why The Feature Flag Exists

This instrumentation is development-only.

It should not be included in the normal customer-facing build because it:

- records additional diagnostic detail
- may capture sensitive session behavior
- increases logging surface area
- is intended for evaluation and debugging, not normal product usage

## What The `dev-session-capture` Feature Enables

When `dev-session-capture` is enabled:

- a per-recording session log file is created when recording starts
- that session log stops when recording stops or the app exits
- Whisper native runtime logs are routed into Rust logging and can land in the session log
- the normal global app log still exists

When `dev-session-capture` is disabled:

- no per-recording session logs are created
- Whisper native log redirection is not enabled
- the build remains closer to the intended customer path

## Log Locations

Global app logs:

- `%LOCALAPPDATA%\MangoChat\logs\app.log`

Per-recording session logs when `dev-session-capture` is enabled:

- `%LOCALAPPDATA%\MangoChat\logs\sessions`

Session log file naming pattern:

- `session-YYYYMMDD-HHMMSS-<provider>-<session_id>.log`

## Session Boundaries

For the current implementation, a session log begins when the user starts recording and ends when:

- the user stops recording
- recording stops due to inactivity timeout
- recording stops due to max session duration
- the app exits

This is intentionally aligned with the same user-visible recording session concept used by the app.

## Current Offline Whisper Behavior

Current local transcription on this branch is:

- native and in-process via Rust
- Whisper.cpp only
- not using `whisper-server.exe`
- still batch-per-utterance
- driven by Mango Chat VAD commit boundaries

That means:

- Mango Chat decides when an utterance is committed
- Whisper transcribes the committed chunk afterward
- perceived slowness is now more about decode latency than server overhead

## Current VAD Timing

In the current strict path, logs have shown:

- `hangover_ms = 260`
- `stop_silence_ms = 50`
- `post_roll_ms = 40`

So commits are already fairly aggressive. Remaining latency is primarily the decode step after commit.

## Current Whisper Runtime Notes

Current native Whisper improvements already implemented:

- in-process `whisper-rs` integration
- reusable `WhisperRuntime`
- reusable `WhisperState`
- preload path when `Offline + Whisper.cpp` is selected

This means the app has already moved past:

- `whisper-server.exe`
- localhost HTTP inference
- one-time-per-utterance model loading

## Windows Build Requirements For Native Whisper

The native Whisper build on Windows requires:

- LLVM installed so `libclang.dll` is available for bindgen
- CMake available

Known working paths on this machine:

- `C:\Program Files\LLVM\bin\libclang.dll`
- `C:\Program Files\CMake\bin\cmake.exe`

If the shell does not pick these up automatically, set:

```powershell
$env:LIBCLANG_PATH='C:\Program Files\LLVM\bin'
$env:CMAKE='C:\Program Files\CMake\bin\cmake.exe'
```

before running `cargo run` or `cargo check`.

## Recommended Debug Workflow

1. Run the app with `--features dev-session-capture`
2. Reproduce the dictation behavior you care about
3. Stop the recording session
4. Open `%LOCALAPPDATA%\MangoChat\logs\sessions`
5. Inspect the matching session log
6. Compare:
   - VAD boundaries
   - Whisper decode timing
   - final transcript text
   - any command execution logs

## Next Likely Dev-Only Additions

These are not implemented yet, but fit the same dev-only feature flag:

- full session audio capture to WAV
- post-session upload to an online transcription provider for comparison
- sidecar JSON with timestamps and comparison metadata

If added, they should remain behind `dev-session-capture` or another development-only feature.
