# Architecture: Speech-to-Action Pipeline


## End-to-end flow

```
Microphone → Audio Capture & Resample → Local VAD Gate → WebSocket (provider) → Transcript Events → Command / Typing Dispatch
src/audio.rs                            src/audio.rs      src/provider/session.rs  provider/*.rs       src/typing.rs
```

1. **Mic capture** (`src/audio.rs`): Audio is captured via `cpal`, downmixed to mono, and resampled to the provider's required sample rate.
2. **Local VAD** (`src/audio.rs`): A threshold + hangover gate suppresses silence. A preroll buffer preserves utterance onsets.
3. **Provider transport** (`src/provider/session.rs`): PCM chunks are encoded per-provider (base64 JSON or raw binary) and streamed over a WebSocket. On speech end, a commit signal is sent. The session auto-reconnects on disconnect (800ms backoff) and stops only when the audio channel is closed.
4. **Transcript parsing** (`src/provider/*.rs`): Each provider's WebSocket messages are parsed into normalized `TranscriptDelta` (interim) and `TranscriptFinal` (committed) events.
5. **Action dispatch** (`src/typing.rs`): Final transcripts are handed to `process_transcript` on a blocking thread. The text is either executed as a voice command or typed into the currently focused window.

---

## Audio capture and local VAD

File: `src/audio.rs`

- Audio is captured and resampled to the provider-required sample rate (`config.sample_rate` from `src/provider/session.rs`).
- Local VAD modes:
  - `strict`: higher threshold (150/32768), shorter hangover (300ms), shorter preroll (100ms).
  - `lenient`: lower threshold (100/32768), longer hangover (700ms), longer preroll (300ms).
  - `off`: always send audio (no gating).
- While below threshold (and outside hangover), chunks are suppressed and not sent upstream.
- A preroll buffer is kept; when speech starts, preroll audio is sent first so utterance onsets are not clipped.
- During speech, PCM chunks are continuously sent to the provider send loop.
- When speech ends, audio thread sends an empty chunk (`Vec::new()`) as an internal commit signal.
- The session send loop converts that signal into provider-specific commit behavior:
  - OpenAI: sends `input_audio_buffer.commit`.
  - Deepgram: sends `Finalize`.
  - ElevenLabs: sends manual commit JSON (`"commit": true`).
- After commit, the session starts a 700ms flush timer. If no provider-side final arrives in that window, the session forces a local flush to avoid stale buffered segments.
- Audio thread also computes a 50-bar FFT spectrum for the UI waveform visualizer.

---

## Provider transport and session management

File: `src/provider/session.rs`, `src/provider/mod.rs`

- All providers implement `SttProvider` and return a `ConnectionConfig` (`src/provider/mod.rs`).
- `run_session` manages the full lifecycle: WebSocket connect, optional init message, audio send loop, event receive loop, keepalive, commit signals, and reconnect.
- Audio sample rate is provider-driven (`config.sample_rate`), so capture/resampling adapts to each provider's requirement.
- Reconnect strategy: on any disconnect, retries automatically after 800ms. Stops only when the audio channel is closed (i.e. the session was intentionally stopped).

### Provider sample rates and encoding

| Provider | Sample Rate | Wire Encoding | Model |
|----------|-------------|---------------|-------|
| OpenAI Realtime | 24 kHz | Base64 JSON (`input_audio_buffer.append`) | configurable (default `gpt-4o-realtime-preview`) |
| Deepgram | 16 kHz | Raw binary (linear16) | `nova-3` (hardcoded) |
| ElevenLabs Realtime | 16 kHz | Base64 JSON (`input_audio_chunk`) | `scribe_v2_realtime` (hardcoded) |

---

## OpenAI Realtime

File: `src/provider/openai.rs`

- Quirk: Requires an explicit session setup message after connect.
  - Solution: Sends `session.update` init payload with input format (PCM 24kHz), noise reduction, transcription model/language, and server VAD config.
- Quirk: Both local VAD and server-side VAD are active.
  - Solution: Server VAD is configured with `create_response: false` so it does not generate model responses. Local VAD drives commit timing. Server VAD acts as a secondary signal.
- Quirk: Audio must be JSON base64 chunks, not raw binary.
  - Solution: Uses `input_audio_buffer.append` envelope for each chunk.
- Quirk: End-of-utterance must be explicitly committed.
  - Solution: Sends `input_audio_buffer.commit` on local VAD stop.
- Quirk: Realtime sessions accumulate conversation items.
  - Solution: On completed transcription, sends `conversation.item.delete` to keep context clean.
- Quirk: Empty commit can raise benign provider errors.
  - Solution: Ignores `input_audio_buffer_commit_empty`.

## Deepgram

File: `src/provider/deepgram.rs`

- Quirk: Uses raw PCM binary frames (linear16) over websocket.
  - Solution: `AudioEncoding::RawBinary` with 16kHz stream config.
- Quirk: Transcript arrives as multiple final segments before utterance end.
  - Solution: Accumulates `is_final` segments and emits one final transcript only on `speech_final` (or flush).
- Quirk: Can emit `UtteranceEnd` separately from `Results`.
  - Solution: Handles `UtteranceEnd` by forcing a provider `flush()`.
- Quirk: Idle websocket requires keepalive.
  - Solution: Sends Deepgram `KeepAlive` every 5s and `CloseStream` on shutdown.

## ElevenLabs Realtime

File: `src/provider/elevenlabs.rs`

- Quirk: Uses custom JSON envelope and API key header (`xi-api-key`).
  - Solution: Sends `input_audio_chunk` with base64 PCM and sample rate metadata.
- Quirk: Session stability is sensitive when idle.
  - Solution: Sends an initial silence chunk on connect and periodic silence keepalive every 3s.
- Quirk: Commit protocol differs from OpenAI/Deepgram.
  - Solution: Uses manual commit via empty `input_audio_chunk` with `"commit": true`.
- Quirk: Event names differ (`partial_transcript`, `committed_transcript`).
  - Solution: Maps partial to delta and committed to final in provider parser.
- Quirk: Needs explicit close.
  - Solution: Sends `{"message_type": "close"}` on shutdown.

---

## Transcript dispatch: commands and typing

File: `src/typing.rs`

Once a `TranscriptFinal` arrives, `process_transcript` runs on a blocking thread and decides what to do with it. The text is normalized (lowercase, punctuation stripped, whitespace collapsed) and then matched in priority order:

1. **URL commands** (dynamic, from settings): If the phrase matches a configured trigger (e.g. "github", "youtube"), opens that URL in Chrome via `Ctrl+T → Ctrl+L → type URL → Enter`.
2. **App-launch commands**: "chrome" / "open chrome" focuses or launches Chrome. "paint" / "open paint" launches mspaint.
3. **Static keyboard commands** (with wake word "Jarvis"):
   - `enter` / `new line` / `line break` → press Enter
   - `new paragraph` → double Enter
   - `back` → delete previous word (Ctrl+Shift+Left then Backspace)
   - `back back` → delete line (Home+Shift then Backspace)
   - `select all`, `undo`, `redo`, `copy`, `paste`, `cut` → corresponding Ctrl+key
   - If a command is matched but trailing text remains, the command executes and the remainder is typed.
4. **Standalone commands** (no wake word): exact match only — bare "enter", "back", etc.
5. **Fallback**: If nothing matches, the full original transcript is typed into the focused window via `enigo`, with a trailing space appended.

Wake word recognition includes phonetic variants: "jarvis", "jarvi", "jarbi", "jarbis", "jarviss".

---

## UI and settings

Files: `src/settings.rs`, `src/ui.rs`

- API keys are stored per provider in `settings.api_keys` (`HashMap<String, String>`).
- When the user changes provider in the settings UI, the current key is saved back to the map and the new provider's key is loaded into the form field. This prevents cross-provider key mixups.
- Provider, model, transcription model, language, mic device, VAD mode, theme, and text size are all persisted to `settings.json` in the local app data directory.
