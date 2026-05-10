# Whisper Quality Integration Plan

## Purpose

This document captures the next planned changes for the offline Whisper path on `feature/moonshine`.
It is a planning note only. No native in-process Whisper integration has been implemented yet.

## Current State

- Offline mode already exists behind the existing settings switch:
  - `Cloud`
  - `Offline -> Moonshine`
  - `Offline -> Whisper.cpp`
- The current Whisper path is functional, but it is still out-of-process:
  - `mangochat.exe`
  - `whisper-server.exe`
  - local HTTP request to `/inference` per committed utterance
- Current user-observed behavior:
  - Accuracy is better than Moonshine
  - Latency is acceptable but still feels slow
  - Memory is acceptable for now
  - The main remaining issue is product feel, not basic feasibility

## Primary Goal

Keep Whisper as the stronger offline ASR option, but remove unnecessary latency and integration overhead without breaking the existing cloud path.

## Constraints

- Do not regress the current cloud providers
- Do not remove the existing offline switch
- Keep the Whisper path English-only for now
- Preserve the ability to compare Moonshine and Whisper on the same branch
- Prefer an implementation that can be packaged cleanly for Windows release builds

## Planned Next Changes

### 1. Replace `whisper-server.exe` with in-process Rust integration

Current design:

- Rust app spawns `whisper-server.exe`
- Rust wraps each utterance as WAV
- Rust posts multipart HTTP to `http://127.0.0.1:18183/inference`
- Rust parses JSON response text

Planned design:

- Keep using `whisper.cpp`
- Stop using it as a separate server process
- Load the Whisper model inside `mangochat.exe`
- Call the Whisper C API from Rust directly
- Return final transcript text to the existing typing path

Expected benefits:

- Remove HTTP overhead
- Remove multipart form overhead
- Remove localhost server coordination
- Simplify deployment and Task Manager footprint
- Reduce end-of-utterance latency

### 2. Preserve the current UI/settings seam

The following settings behavior should remain unchanged:

- `Transcription mode = Cloud`
- `Transcription mode = Offline`
- `Offline engine = Moonshine`
- `Offline engine = Whisper.cpp`

The refactor should change only the Whisper runtime implementation, not the user-facing switch.

### 3. Keep Whisper model warm for the entire session

Instead of spinning up external inference handling per utterance, the in-process Whisper integration should:

- load the model once
- keep the context alive across utterances
- run inference on a dedicated worker thread
- send transcript results back to the app through the existing event flow

This should improve perceived responsiveness even if VAD still commits on pauses.

### 4. Separate offline endpointing from cloud endpointing

Current VAD timing changes affect both offline and cloud modes because they live in the shared audio path.

Planned change:

- keep shared audio capture
- introduce mode-aware endpointing settings
- allow offline Whisper to use more aggressive commit timing than cloud providers

Reason:

- offline Whisper benefits from earlier commit after a pause
- cloud provider behavior should not be changed accidentally while tuning local ASR

### 5. Keep transcript sanitation on the offline Whisper path

The current Whisper path needed filtering for non-speech markers such as:

- `[BLANK_AUDIO]`
- `[Silence]`
- `[MUSIC]`

That sanitation should remain in the native integration so the focused app never receives those markers as typed text.

## Proposed Implementation Order

1. Add a dedicated in-process Whisper runtime module
2. Move the current Whisper branch in `local_stt.rs` behind that runtime
3. Remove HTTP/multipart inference for Whisper
4. Keep Moonshine unchanged for comparison
5. Split offline VAD tuning from cloud VAD tuning
6. Re-test accuracy, latency, and resource usage

## Likely Code Areas

- `Cargo.toml`
- `src/local_stt.rs`
- `src/audio.rs`
- new Whisper runtime module if the code is split out cleanly

## Acceptance Criteria

The native Whisper refactor should be considered successful if:

- `Offline + Whisper.cpp` still works end to end
- `Cloud` mode behavior is unchanged
- there is no separate `whisper-server.exe` in Task Manager
- Whisper memory shows under `mangochat.exe`
- end-of-utterance latency is lower than the current server-based path
- transcript quality is not worse than the current server-based path
- `[BLANK_AUDIO]`-style markers do not appear in typed output

## Open Questions

- Whether to use a Rust binding crate for `whisper.cpp` or call the C API directly
- Whether model loading should happen lazily on first offline use or at app startup
- Whether to add pseudo-streaming later, or keep batch-per-utterance if latency is good enough after in-process integration

## Non-Goals For The Next Step

- No multilingual support
- No GPU support work
- No replacement of Moonshine
- No redesign of the cloud provider architecture
- No streaming partial-text Whisper UX yet unless latency remains unacceptable after the native refactor

## Summary

The next planned step is not to change which model we use. It is to keep Whisper as the better-quality offline option and make its integration tighter, faster, and cleaner by moving from `whisper-server.exe` to in-process Rust integration while preserving the current offline/cloud switch.
