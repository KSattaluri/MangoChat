# Jarvis App Notes

## Goal
Windows-native dictation app (system tray) with two modes:
- Strict: verbatim transcript only.
- Assisted: transcript + LLM-driven edit commands via voice.

## Product Scope (Current)
- Windows-first native app built with Tauri.
- Runs in system tray, always-on and listening when enabled.
- Injects text and keystrokes into the currently focused text field in any app
  (e.g., VS Code, Codex, browsers, editors).
- Supports voice commands like "enter" to emit keystrokes.
- Web app is a POC to validate OpenAI STT/LLM/WebSocket/auth flows.

## Direction (New)
- Tauri desktop app with Rust backend (app core in Rust).
- Short-term: WebSocket client can remain JS-based.
- Longer-term: move WebSocket client to Rust.
- Make cross-platform (Mac/Linux) after Windows scope is complete.
- Keep API keys configurable via UI (no hardcoded env for end users).
- Add transcript persistence (save raw + revised, with timestamps).

## Windows v1 Scope + Success Criteria
### Scope
- Tray app with a visible enable/disable control (armed state).
- Hotkey is press-and-hold (push-to-talk). App only listens while held.
- Works with any focused text field in any app.
- If no focused field when dictation ends, transcript is copied to clipboard.
- Validation targets: VS Code, Cursor, terminal input, and a browser text field.

### Success Criteria
1. Start/stop dictation with press-and-hold hotkey while app is armed.
2. Transcript inserts into focused field in VS Code, Cursor, and browser input.
3. If no focused field, transcript is available via clipboard without loss.
4. No obvious UI lag or missed text in typical dictation sessions.

## Analysis (Initial)
### Risks
- WebView background throttling could disrupt JS WebSocket sessions.
- Reliable text/keystroke injection across all apps may require platform-specific handling.
- Microphone permission failures or device changes can break capture mid-session.

### Dependencies
- Windows WebView2 runtime availability/updates.
- Global hotkey + input injection APIs on Windows.
- OpenAI Realtime and Responses API availability.

### Unknowns
- Best Windows injection mechanism for broad app compatibility.
- Behavior when focus changes mid-dictation.
- Long-running stability (reconnect strategy, tray lifecycle).

### Out of Scope (v1)
- Mac/Linux support.
- No-WebView “headless” tray mode.
- Offline STT or local models.

### Acceptance Tests (v1)
1. Press-and-hold hotkey inserts text into VS Code, Cursor, and a browser input.
2. If no focused field, transcript is copied to clipboard.
3. Tray enable/disable reliably arms/disarms dictation.

### Telemetry / Diagnostics
- Minimal local logging for connection state, errors, and session duration.
- Optional user-facing debug panel for WebSocket state (v1 if needed).

## Design Decisions (Audio + WebSocket)

### Audio Capture: WebAudio (WebView) vs `cpal` (Rust)
- **Decision**: Use WebAudio long-term.
- **Why**: WebAudio is a W3C standard (stable since 2014) and shipped in Chromium,
  which is the engine behind WebView2. Microsoft updates WebView2 via Windows Update,
  so this API is stable and widely deployed. AudioWorklet is also mature; the PCM
  capture code should remain valid for years.
- **`cpal` tradeoff**: More control via WASAPI, but requires manual device
  enumeration, sample-rate conversion, buffer sizing, and format negotiation.
- **When `cpal` wins**: If the app must run with *no* WebView loaded at all
  (pure tray, no window). In v1, Tauri keeps the WebView alive even when minimized,
  so this is not a blocker.

### WebSocket: JS (WebView) vs Rust
- **Risk**: Dictation is a background workflow. If WebView throttling changes for
  hidden/minimized windows, the JS WebSocket could be disrupted mid-session.
- **Decision (Near-term)**: JS WebSocket is acceptable while WebView remains active.
- **Decision (Longer-term)**: Move WebSocket to Rust for reliability.
  - Rust WebSocket runs independently of the WebView lifecycle.
  - Minimization or window hiding does not affect the connection.
  - Architecture: WebView (AudioWorklet) → Rust (IPC/invoke) → Rust WebSocket → OpenAI.
- **Cost**: Re-implement protocol handling in Rust (session.update, audio buffer
  encoding, transcript event parsing). Roughly 150-200 lines of Rust plus maintenance.

## Current Implementation (POC Web App)

### Tech Stack (POC Web App)
- **Backend**: Python 3.11+, FastAPI, uvicorn, websockets
- **Frontend**: Vanilla HTML/CSS/JS (no framework)
- **Audio**: Browser AudioWorklet capturing 24kHz mono PCM
- **STT**: OpenAI Realtime API (`gpt-realtime` model, `gpt-4o-mini-transcribe` for transcription)
- **Revise**: OpenAI Responses API (`OPENAI_REVISE_MODEL`, default `gpt-4.1-mini`)
- **Config**: `.env.local` for local dev; UI settings planned for end users
- **Package manager**: uv

### Project Structure (POC Web App)
```
jarvis/
|-- app.py                   # FastAPI server, WebSocket proxy, /revise endpoint
|-- main.py                  # Placeholder (unused)
|-- openai_realtime_check.py # Standalone connectivity diagnostic
|-- pyproject.toml           # Dependencies and project metadata
|-- .env.local               # API keys (not committed)
|-- diction.md               # This file
|-- README.md
`-- static/
    |-- index.html           # Single-page UI + Raw/Revised tabs
    |-- styles.css           # Dark theme
    |-- app.js               # Recording, WebSocket client, revise workflow
    `-- audio-worklet.js     # PCM capture worklet
```
### What's Working
- Browser captures mic audio via AudioWorklet at 24kHz mono PCM.
- `app.py` proxies audio over WebSocket to OpenAI Realtime API.
- Streaming partial transcripts displayed in gray; final transcripts appended.
- Command blocks captured inline using `command ... end command`.
- **Revise** button sends raw transcript + command blocks to LLM and displays revised output in a separate tab.
- Copy button for transcript.
### What's Not Yet Implemented
- Strict/Assisted mode toggle (observer pipeline).
- `raw_text` / `display_text` separation (only one text stream on client).
- Redo (undo stack exists, no redo stack).
- Raw/Edited view toggle in UI (only Raw/Revised tab, not two synchronized panes).
- Settings UI for API keys and model selection.
- Transcript persistence / session history.
- Transform output pane (summarize/rewrite).
- Visual indicators for ambiguous state (pulsing/dimmed).
- Edit-applied toast notifications.
- Contextual hints (static footer hint only).
- Desktop packaging.
### Known Issues
- Partial text overwrites on each delta instead of accumulating.
- No error handling if user denies microphone permission.
- WebSocket opens before AudioContext is ready (brief window with no audio flowing).
- No WebSocket keep-alive/ping to OpenAI Realtime connection.

_Below is the target spec (not current state)._

## Architecture: Three-Agent Pipeline

```
User Speech
     |
     +---> STT (OpenAI Realtime API) ---> raw_text (always appending)
     |
     +---> Observer Agent (gpt-4.1-mini, ~50-100 tokens)
                |
                +-- "dictation"  --> do nothing, STT handles it
                +-- "edit intent" --> route to Parser Agent
                +-- "ambiguous"  --> clarify with user
                                       |
                                       +-- "Did you mean to edit, or is
                                            that part of your dictation?"
                                       |
                                       +-- User responds:
                                       |     "dictation" --> append to raw_text
                                       |     "edit"      --> route to Parser
                                       +-- Timeout (3-4s) --> default to dictation
```

### Agent Roles

| Agent | Model | Role | Token Budget | Latency |
|-------|-------|------|--------------|---------|
| **STT** | OpenAI Realtime (`gpt-realtime` + `gpt-4o-mini-transcribe`) | Always-on transcription, append to raw_text | n/a | streaming |
| **Observer** | gpt-4.1-mini (via Responses API) | Classify every utterance: dictation / edit / ambiguous | ~50-100 tokens | ~150-200ms |
| **Parser** | gpt-4.1-mini (via Responses API) | Only called on "edit intent" - produces edit op JSON | ~100-200 tokens | ~300-500ms |

### Why Three Agents
- No keyword hacking or prefix phrases ("hey editor", "command:") needed.
- User speaks naturally; observer decides intent.
- Eliminates false positives (e.g., user dictates "I want to replace the carpet"
  and "replace" incorrectly triggers edit mode).
- Ambiguous cases get clarified by the observer asking the user directly.

## Pipeline Flow

1. App opens, user clicks "Ready".
2. Audio starts, STT streams live transcription to raw_text.
3. Observer classifies each utterance in parallel (~150-200ms).
4. On "dictation": utterance appended to raw_text, displayed immediately.
5. On "edit intent": utterance routed to Parser, edit op applied to display_text.
6. On "ambiguous": utterance buffered, observer asks user to clarify.
   - Visual indicator (pulsing/dimmed text) shows "processing..."
   - User clarifies or timeout defaults to dictation.
7. On pause (silence), create a paragraph break.
8. User can copy transcript via quick action.

### Buffering During Ambiguity
- STT keeps streaming but the ambiguous chunk is held (not yet committed).
- Observer speaks: "Was that part of your dictation, or did you want to make an edit?"
- Resolution: user responds or 3-4s timeout defaults to dictation.
- Buffering is preferred over optimistic-append-then-retract for cleaner UX.

## Data Model
- raw_text: append-only, verbatim STT.
- display_text: editable view (starts as raw_text).
- ops: list of edit operations for undo/redo (stack-based).

## Use Cases

1) Strict Dictation (verbatim)
   - STT only, observer disabled.
   - No edits applied.

2) Assisted Corrections (observer + parser)
   - Observer classifies utterances.
   - Edit intent -> Parser produces structured edit op.
   - Edits apply to display_text only; raw_text unchanged.

3) Voice Editing Commands (natural language)
   - Natural commands like "replace James Bond with Jim Bond... J I M".
   - Observer detects edit intent, routes to Parser.
   - Parser produces edit op JSON, no heuristics.
   - Ambiguous cases clarified by observer asking user.

4) Transform Output
   - Summarize / rewrite into email / bullet list.
   - Output in a separate pane; raw/display stays intact.

## Edit Op Schema (Parser output)
```json
{
  "op": "replace|delete|insert_after|insert_before|undo|none",
  "target": "string or anchor phrase",
  "with": "string (for replace)",
  "text": "string (for insert)",
  "scope": "full|last_paragraph|last_sentence",
  "occurrence": "first|last|all",
  "confidence": 0.0
}
```

## Observer Contract
- Input: last user utterance only (minimal context).
- Output: one of `dictation`, `edit`, `ambiguous`.
- Must be fast (~150-200ms) - runs on every utterance.
- Does NOT parse edit commands - only classifies intent.

## Parser Contract
- Input: last user utterance + short context snippet (recent display_text).
- Output: JSON only, matching edit op schema.
- If no edit intent (fallback): `{"op":"none"}`.
- Must normalize spelled letters (e.g., "J I M" -> "Jim").
- Only called when observer classifies as "edit intent".

## Latency Targets
- Observer classification: < 200ms.
- Parser response: < 500ms.
- End-to-end (edit intent detected to edit applied): < 1.5s.
- Clarification round-trip: < 4s (including user response or timeout).

## UI Notes
- Live transcript with partial text streaming.
- Highlight edits + show short "edit applied" toast.
- Pulsing/dimmed indicator when utterance is buffered (ambiguous).
- Quick copy button.
- Raw/Edited toggle.
- Contextual hints after extended pure dictation:
  "Say 'replace X with Y' or 'delete last sentence' to edit" (dismissible).



