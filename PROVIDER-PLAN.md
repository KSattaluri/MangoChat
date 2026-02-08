# Plan: Multi-Provider STT Abstraction

## Context
Jarvis currently hardcodes OpenAI Realtime API as the sole STT provider. The goal is to abstract the provider layer so users can choose from multiple streaming STT services. Research of 22 providers from the screenshot identified which ones share the same WebSocket+PCM+JSON pattern and can be integrated with minimal custom code.

---

## Provider Compatibility Report

### Tier 1 — "Standard Club" (WebSocket + PCM + JSON, drop-in compatible)

| Provider | Protocol | Audio Format | Auth | Interim Transcripts | Notes |
|----------|----------|-------------|------|-------------------|-------|
| **OpenAI** | wss, base64 JSON | PCM 24kHz | Bearer header | Yes | Already integrated |
| **Deepgram** | wss, raw binary | PCM 24kHz | `Token` header | Yes (`is_final`) | Community Rust crate exists |
| **AssemblyAI** | wss, raw binary | PCM 24kHz | Bearer header | Yes (`end_of_turn`) | ~300ms latency |
| **ElevenLabs** | wss | PCM 8-48kHz | API key header | Yes | ~150ms latency, Scribe v2 |
| **Speechmatics** | wss | PCM | API key | Yes | 55+ languages, <1s latency |
| **Soniox** | wss | PCM | Bearer token | Yes (`is_final`) | Per-word token streaming |
| **Cartesia** | wss | PCM | API key | Yes (interim/final) | ink-whisper model |
| **Sarvam** | wss | PCM 16kHz | API key | Yes | Indian languages focus |
| **Spitch** | wss | PCM | API key | Yes | Speaker diarization |

### Tier 2 — Feasible but different protocol

| Provider | Issue |
|----------|-------|
| **Azure OpenAI** | Near-identical to OpenAI but ephemeral token auth |
| **Mistral Voxtral** | New (Feb 2026), REST + WebSocket |
| **Google Cloud** | gRPC only (no WebSocket) |
| **Amazon Transcribe** | Complex AWS SDK auth |
| **Azure Speech** | Different WebSocket protocol |
| **Clova** | gRPC only |
| **Gladia** | REST-based live API |
| **Baseten** | Requires model deployment |

### Tier 3 — Not suitable

| Provider | Reason |
|----------|--------|
| **Groq** | Batch Whisper only, no streaming |
| **fal** | Batch Whisper only |
| **OVHCloud** | Generic inference platform |
| **Simplismart** | Pseudo-streaming, not real WebSocket |
| **Nvidia** | Self-hosted only, not managed API |

---

## Architecture

### Core Trait

```rust
// src-ui/src/provider/mod.rs

pub trait SttProvider: Send + Sync {
    fn name(&self) -> &str;
    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig;
    fn parse_event(&self, text: &str) -> Vec<ProviderEvent>;
}
```

**Key types:**
- `ConnectionConfig` — URL, headers, init message, audio encoding mode, commit message, sample rate
- `AudioEncoding` — enum: `Base64Json { type_field, type_value, audio_field }` | `RawBinary`
- `CommitMessage` — enum: `Json(Value)` | `None`
- `ProviderEvent` — enum: `TranscriptDelta(String)` | `TranscriptFinal(String)` | `Error(String)` | `SendControl(Value)` | `Status(String)` | `Ignore`
- `ProviderSettings` — `api_key`, `model`, `transcription_model`, `language`, `extra: HashMap`

### Shared Session Runner

`provider/session.rs` — generic `run_session(provider: Arc<dyn SttProvider>, config, event_tx, state, audio_rx)`. Handles the entire WebSocket lifecycle:
1. Build request with provider headers
2. Connect, send init message
3. Send task: audio_rx → encode per `AudioEncoding` → WebSocket
4. Recv task: WebSocket → `provider.parse_event()` → emit `AppEvent`
5. Transcript final → `typing::process_transcript` + any `SendControl` messages

This is extracted from the current `openai.rs` (~200 lines) and made generic.

### Provider Registry

```rust
pub const PROVIDERS: &[(&str, &str)] = &[
    ("openai", "OpenAI Realtime"),
    ("deepgram", "Deepgram"),
    ("assemblyai", "AssemblyAI"),
];

pub fn create_provider(id: &str) -> Arc<dyn SttProvider> { ... }
```

---

## File Layout

```
src-ui/src/
  main.rs              — replace `mod openai` with `mod provider`
  provider/
    mod.rs             — trait, types, registry, create_provider()
    session.rs         — generic run_session() (shared WebSocket loop)
    openai.rs          — OpenAiProvider (~80 lines)
    deepgram.rs        — DeepgramProvider (~60 lines)
    assemblyai.rs      — AssemblyAiProvider (~60 lines)
  settings.rs          — add `provider: String` field (default "openai")
  ui.rs                — use provider::create_provider + dropdown in settings panel
```

**Files to modify:** `main.rs`, `settings.rs`, `ui.rs`
**Files to create:** `provider/mod.rs`, `provider/session.rs`, `provider/openai.rs`, `provider/deepgram.rs`, `provider/assemblyai.rs`
**Files to delete:** `openai.rs` (after refactor verified)
**Files unchanged:** `audio.rs`, `state.rs`, `typing.rs`, `hotkey.rs`, `usage.rs`, `snip.rs`

---

## Implementation Phases

### Phase 1: Extract & Refactor (OpenAI only)
1. Create `provider/` module with trait definitions in `mod.rs`
2. Write `provider/session.rs` — extract generic WebSocket loop from `openai.rs`
3. Write `provider/openai.rs` — implement `SttProvider` for OpenAI
4. Add `provider` field to `Settings` (serde default for backward compat)
5. Update `ui.rs::start_recording()` to use `provider::create_provider` + `provider::run_session`
6. Replace `mod openai` with `mod provider` in `main.rs`
7. Delete old `openai.rs`
8. **Verify:** OpenAI still works identically

### Phase 2: Add Deepgram
1. Write `provider/deepgram.rs` (~60 lines, `RawBinary` encoding, no init message)
2. Add to `PROVIDERS` and `create_provider`
3. Add provider dropdown to settings UI panel

### Phase 3: Add AssemblyAI
1. Write `provider/assemblyai.rs` (~60 lines)
2. Add to registry

### Phase 4 (future): More providers
Each additional provider is ~50-80 lines. Priority order: ElevenLabs, Speechmatics, Soniox.

---

## Settings UI Changes
- Add "Provider" dropdown above API Key field (using `PROVIDERS` const for options)
- Add `form_provider: String` field to `JarvisApp`
- Save/load `provider` in settings.json

---

## Verification
1. Build: `cargo build` from native Windows terminal
2. Test OpenAI: set provider to "openai", verify dictation works as before
3. Test Deepgram: set provider to "deepgram" with a Deepgram API key, verify transcription
4. Test AssemblyAI: same pattern
5. Test backward compat: delete `provider` from settings.json, verify defaults to OpenAI
