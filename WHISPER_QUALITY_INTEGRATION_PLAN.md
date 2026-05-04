# Whisper Quality Integration Plan

## Purpose

This document captures the next planned changes for the offline Whisper path on `feature/moonshine`.
It is a planning note only.

## Current State

- Offline mode already exists behind the existing settings switch:
  - `Cloud`
  - `Offline -> Whisper.cpp`
- The current Whisper path is native and in-process:
  - `mangochat.exe`
  - embedded `whisper.cpp` runtime through Rust
- Current user-observed behavior:
  - Accuracy is acceptable and clearly better than the removed Moonshine path
  - Latency still feels slow at about ~2s from commit to final on this machine
  - Memory is acceptable for now
  - The main remaining issue is product feel, not basic feasibility

## Primary Goal

Keep Whisper as the single offline ASR option, but remove unnecessary latency and product friction without breaking the existing cloud path.

## Constraints

- Do not regress the current cloud providers
- Do not remove the existing offline switch
- Keep the Whisper path English-only for now
- Prefer an implementation that can be packaged cleanly for Windows release builds

## Planned Next Changes

### 1. Preserve the current UI/settings seam

The following settings behavior should remain unchanged:

- `Transcription mode = Cloud`
- `Transcription mode = Offline`

The app should stay simple: one cloud mode, one local Whisper mode.

### 2. Keep Whisper model warm only while local mode is active

The in-process Whisper integration should:

- load the model once
- keep the context alive across utterances
- run inference on a dedicated worker thread
- send transcript results back to the app through the existing event flow

This is largely implemented already. Remaining work is around responsiveness and decode cadence, not basic runtime ownership.

### 3. Separate offline endpointing from cloud endpointing

Current VAD timing changes affect both offline and cloud modes because they live in the shared audio path.

Planned change:

- keep shared audio capture
- introduce mode-aware endpointing settings
- allow offline Whisper to use more aggressive commit timing than cloud providers

Reason:

- offline Whisper benefits from earlier commit after a pause
- cloud provider behavior should not be changed accidentally while tuning local ASR

### 4. Consider pseudo-streaming partials only if the CPU tradeoff is acceptable

Current Whisper behavior is still batch-per-utterance.

Possible next step:

- run periodic rolling-window decodes while speech is active
- surface only stabilized partial text
- keep final decode on utterance commit

Reason:

- users care about earlier visible feedback
- final latency can remain similar while the app feels much faster

Tradeoff:

- materially higher CPU usage
- more transcript instability unless stabilized carefully

### 5. Keep transcript sanitation on the offline Whisper path

The current Whisper path needed filtering for non-speech markers such as:

- `[BLANK_AUDIO]`
- `[Silence]`
- `[MUSIC]`

That sanitation should remain in the native integration so the focused app never receives those markers as typed text.

## Proposed Implementation Order

1. Split offline VAD tuning from cloud VAD tuning
2. Measure current commit-to-final latency on longer sessions
3. Decide whether pseudo-streaming partials are worth the CPU cost
4. If yes, implement rolling partial decodes with stabilization
5. Re-test latency, CPU, and usability

## Likely Code Areas

- `src/local_stt.rs`
- `src/audio.rs`
- `src/whisper_runtime.rs`
- `src/ui/mod.rs`

## Acceptance Criteria

The next Whisper iteration should be considered successful if:

- `Offline + Whisper.cpp` still works end to end
- `Cloud` mode behavior is unchanged
- Whisper remains unloaded when the app is in cloud mode
- transcript quality is not worse than the current baseline
- perceived responsiveness improves
- `[BLANK_AUDIO]`-style markers do not appear in typed output

## Open Questions

- Whether pseudo-streaming partials are worth the CPU cost on target hardware
- Whether a smaller Whisper model variant is worth exposing for faster fallback testing
- Whether the first-utterance experience needs special warmup handling

## Non-Goals For The Next Step

- No multilingual support
- No GPU support work
- No redesign of the cloud provider architecture
- No second local engine path

## Summary

The next planned step is not another engine comparison. It is to keep Whisper as the single local path and improve its responsiveness without regressing quality or cloud behavior.
