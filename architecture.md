# Architecture: Speech-to-Action Pipeline

## End-to-end flow

```text
Microphone -> Audio Capture & Resample -> Local VAD Gate -> WebSocket (provider) -> Transcript Events -> Command/Typing Dispatch
src/audio.rs                            src/audio.rs      src/provider/session.rs  src/provider/*.rs    src/typing.rs
```

1. **Mic capture** (`src/audio.rs`): Captured via `cpal`, downmixed to mono, and resampled to provider-required sample rate.
2. **Local VAD** (`src/audio.rs`): Threshold + hangover gate suppresses silence; preroll preserves utterance starts.
3. **Provider transport** (`src/provider/session.rs`): Streams PCM chunks over WebSocket with provider-specific encoding and commit signals.
4. **Transcript parsing** (`src/provider/*.rs`): Normalizes provider messages into `TranscriptDelta` and `TranscriptFinal`.
5. **Action dispatch** (`src/typing.rs`): Final transcript is interpreted as command or typed into the focused app.

---

## Audio capture and local VAD

File: `src/audio.rs`

- Capture is resampled to provider sample rate from `ConnectionConfig`.
- Local VAD uses the WebRTC VAD algorithm via the `webrtc-vad` crate.
- User-facing VAD modes:
  - `strict`: higher threshold, shorter hangover/preroll.
  - `lenient`: lower threshold, longer hangover/preroll.
- Internal legacy `off` mode value still exists for compatibility, but UI only exposes strict/lenient.
- Suppressed chunks are not sent while below threshold (outside hangover window).
- Preroll is sent when speech starts to avoid clipped onsets.
- On speech stop, audio sends an empty chunk as internal commit marker.
- Session layer maps commit marker to provider protocol:
  - OpenAI: `input_audio_buffer.commit`
  - Deepgram: finalize message
  - ElevenLabs: manual `"commit": true`
- Audio thread also computes 50-bin FFT magnitudes for the compact visualizer.

---

## Provider transport and session management

Files: `src/provider/session.rs`, `src/provider/mod.rs`

- Providers implement `SttProvider` and produce `ConnectionConfig`.
- `run_session` handles:
  - connect/init
  - send loop
  - receive loop
  - keepalive
  - commit/flush behavior
  - reconnect
- Reconnect backoff is automatic and session ends only when audio channel closes intentionally.

### Provider sample rates and encoding

| Provider | Sample Rate | Wire Encoding | Model |
|----------|-------------|---------------|-------|
| OpenAI Realtime | 24 kHz | Base64 JSON (`input_audio_buffer.append`) | configurable (`gpt-4o-realtime-preview` default) |
| Deepgram | 16 kHz | Raw binary (linear16) | `nova-3` |
| ElevenLabs Realtime | 16 kHz | Base64 JSON (`input_audio_chunk`) | `scribe_v2_realtime` |
| AssemblyAI | provider-specific | provider-specific | configurable in provider impl |

---

## Provider-specific notes

- **OpenAI** (`src/provider/openai.rs`):
  - Sends `session.update` init.
  - Uses local VAD for commit timing.
  - Sends `conversation.item.delete` cleanup after completed transcription.
- **Deepgram** (`src/provider/deepgram.rs`):
  - Uses raw binary audio.
  - Accumulates segment finals and emits committed result on endpoint conditions.
  - Keepalive is sent periodically.
- **ElevenLabs** (`src/provider/elevenlabs.rs`):
  - Uses `xi-api-key` + custom JSON envelopes.
  - Uses manual commit semantics and explicit close.
- **AssemblyAI** (`src/provider/assemblyai.rs`):
  - Integrated as first-class provider in provider factory and UI provider selection.

---

## Transcript dispatch: commands and typing

File: `src/typing.rs`

`process_transcript` normalizes text (lowercase, punctuation stripped, whitespace collapsed) and applies this order:

1. **Dynamic URL commands** from settings (`url_commands`).
2. **App launch commands** (`chrome`, `paint`).
3. **Dynamic alias commands** from settings (`alias_commands`).
4. **Static keyboard commands** with wake-word prefix support.
5. **Standalone exact command matches** (no wake word).
6. **Fallback typing** via `enigo` into the focused window.

Current wake-word variants in code:
- `mangochat`, `mango`, `jarvis`, `jarvi`, `jarbi`

---

## UI, settings, and persistence

Files:
- UI root: `src/ui/mod.rs`
- UI tabs: `src/ui/tabs/*`
- Form model: `src/ui/form_state.rs`
- Settings: `src/settings.rs`
- Secrets: `src/secrets.rs`

Key points:
- UI is modular (no monolithic `src/ui.rs`).
- Provider API keys are stored per provider (`settings.api_keys`) and encrypted at rest via DPAPI on Windows (`src/secrets.rs`).
- Settings persist under:
  - `%LOCALAPPDATA%\\MangoChat\\settings.json`
  - fallback read migration from legacy `Jarvis` paths.
- Usage/session/provider counters persist under `%LOCALAPPDATA%\\MangoChat\\...` (`src/usage.rs`).
- Snips are stored under `Pictures\\MangoChat` (`src/snip.rs`).
