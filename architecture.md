# Mango Chat Architecture

## 1) Purpose and scope

Mango Chat is a lightweight Windows desktop utility for real-time speech-to-text and speech-to-action workflows.

It is designed to:
- Capture microphone audio locally.
- Send speech either to a selected cloud STT provider or to a local Whisper runtime.
- Apply local command rules and text insertion into the active app.
- Keep user configuration and usage telemetry on-device.

It is not designed to:
- Run general local LLM pipelines.
- Store user transcripts in a central backend.
- Operate as a multi-tenant server platform.

## 2) High-level system boundary

```text
User + Windows Desktop
    |
    | (mic audio, hotkeys, UI actions)
    v
Mango Chat (local desktop app)
    |
    +--> Local Whisper.cpp runtime
    |
    +--> Cloud STT Provider (OpenAI / Deepgram / ElevenLabs / AssemblyAI)
            |
            | (transcript events)
            v
       Mango Chat local action engine -> text/commands to focused Windows app
```

Key point: execution is primarily local. In cloud mode, only audio stream and provider protocol traffic leave the machine. In local mode, transcription stays on-device.

## 3) Runtime flow (end-to-end)

1. Audio capture starts from selected/default microphone.
2. Local preprocessing runs (downmix/resample + local VAD gate).
3. Speech chunks are either sent to provider-specific cloud transport or committed to the local Whisper runtime.
4. Transcript events are normalized into internal transcript events.
5. Final text is resolved through command rules or typed into the focused app.
6. Session/usage counters are updated locally.

## 4) Core runtime components

| Component | Responsibility |
|---|---|
| UI shell | Compact always-on-top control surface + settings panels |
| Audio engine | Capture, resample, VAD gating, FFT visualizer feed |
| Provider session engine | Connect/send/receive/reconnect/keepalive lifecycle for cloud STT |
| Local STT engine | In-process Whisper runtime for local transcription |
| Action engine | Command matching and keyboard/text dispatch |
| Screenshot module | Screen snip capture, clipboard/editor actions |
| Settings/secrets store | Persist settings + encrypted provider keys |
| Update module | Check releases, optional installer download/launch |

## 5) Provider abstraction model

Mango Chat uses a transcription-mode split:

- `Cloud`: provider abstraction for hosted STT backends
- `Local`: in-process Whisper.cpp path

Within cloud mode, Mango Chat uses a provider abstraction so one UI/app flow can target multiple STT backends.

Provider integrations vary by:
- URL and auth headers
- Audio wire format (binary vs JSON base64)
- Commit/finalization semantics
- Transcript event shape

Normalization happens inside the app before dispatch, so product behavior remains consistent across providers.

The local Whisper path reuses the same downstream transcript dispatch and command handling flow.

## 6) Data and security model

### Local persistence
- Settings, usage, and local metadata are stored under local app-data paths.
- API keys are not kept as plaintext in the normal settings file.

### Secret handling
- Provider keys are stored separately and encrypted at rest (Windows DPAPI path).
- Legacy settings migration paths are supported for backward compatibility.

### Network surface
- Outbound traffic is provider WebSocket/API traffic and update-check traffic in cloud mode.
- Local Whisper mode does not require provider traffic for transcription.
- No inbound listening service is required for normal operation.

## 7) Reliability and resilience

- Single-instance lock prevents duplicate app instances.
- Provider session includes reconnect/backoff behavior for transient failures.
- Local Whisper runtime is loaded lazily and unloaded when local recording stops or when the app switches away from local mode.
- Mic/device loss is detected and surfaced to UI state.
- Session boundaries are explicit (start/stop), with usage counters maintained.

Operationally, this favors desktop reliability over complex distributed recovery patterns.

## 8) Performance profile (design intent)

The app is built in Rust with a native desktop stack (eframe/egui) to keep runtime overhead low.

Design choices that support low footprint:
- Local VAD gate to reduce unnecessary uplink audio.
- Lightweight local state and file-based persistence.
- No embedded browser runtime.

Tradeoff:
- Local Whisper keeps transcription on-device, but its current path is still batch-per-utterance and can feel slower than cloud streaming providers.

## 9) Deployment and distribution model

Target platform: Windows desktop.

Packaging model:
- Native executable build (`mangochat.exe`).
- Installer packaging for user deployment.
- Release artifacts published through GitHub Releases.

Update model:
- In-app check against GitHub release feed.
- Optional user-driven installer download/launch from app.

## 10) Governance and extension points

### Good extension points
- New STT provider implementations under the provider abstraction.
- Improvements to the local Whisper path, such as model management or lower-latency decode strategy.
- Additional command rules and enterprise presets.
- Additional diagnostics/telemetry exports (local-first).

### Review focus for enterprise adoption
- Provider policy/compliance fit (data residency, retention, contracts).
- Endpoint management policy for hotkeys/input automation.
- Code-signing + release governance process.
- Privacy posture around local logs and screenshot workflows.

## 11) Current architectural constraints

- Desktop-first model (not a web/mobile control plane).
- Provider availability and quality are external dependencies in cloud mode.
- Local Whisper quality and latency depend on local CPU performance and bundled model choice.
- Input automation behavior can vary by target Windows application.
- Bluetooth headset behavior may vary by driver/firmware policy.

---

For engineering implementation details, see source modules under `src/` and setup/release operational docs in this repository.
