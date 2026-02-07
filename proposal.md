# Proposal: Diction Desktop (Tauri)

## Summary
Diction Desktop is a Windows-first, cross-platform dictation app that types directly into the focused OS text field. It uses low-latency speech-to-text (STT) and supports a minimal command set (starting with "enter"). The product goal is a lightweight, always-available dictation tool with reliable OS-level typing, hotkey control, and configurable API keys/models.

## Background
A web-based POC (FastAPI + browser) was built first to validate:
- OpenAI Realtime WebSocket connection and protocol.
- Audio capture at 24kHz mono PCM via AudioWorklet.
- Final transcript latency and STT quality.
- Observer/parser pipeline feasibility.

These were confirmed working. The web app serves as a reference implementation. The desktop app reuses the same audio capture and WebSocket logic inside a Tauri WebView, replacing the transcript pane with OS-level text injection.

## Goals
- Dictate into any focused field (Word, terminal, browser address bar, search bars, etc.).
- Start/stop via hotkey (default: `Ctrl+Shift+D`) and optional UI toggle.
- Low latency with final-utterance typing (type only after a pause/VAD).
- Configurable API key and model settings (no env files for end users).
- Optional tray icon and auto-start.
- Clean upgrade path to multi-platform support (macOS/Linux).

## Non-Goals (for v1)
- Real-time partial typing (streaming keystrokes per partial delta).
- Multi-speaker diarization.
- Advanced editor commands (undo, backspace, delete, select). Only `enter` command in v1.
- Offline STT.

## High-Level Requirements
1. **OS-Level Text Injection**
   - Must type into the active/focused window, not a browser UI.
   - Must be reliable across common apps (Office, Chrome, terminals).

2. **Low-Latency STT**
   - Use OpenAI Realtime transcription over WebSocket.
   - Type only after final transcript (pause/VAD) to avoid jitter.

3. **Command Recognition (Minimal)**
   - Spoken `enter` should emit an Enter keypress.
   - Command matching is literal-only: the entire utterance must be exactly "enter" (case-insensitive, trimmed). If "enter" appears within a sentence, it is typed as text.
   - Command processing happens locally before typing.

4. **User Controls**
   - Global hotkey: start/stop dictation.
   - UI button: start/stop.
   - Status indicator: idle/listening/error.

5. **Configuration & Storage**
   - API key stored in local JSON file via `tauri-plugin-store`.
   - Keychain/Credential Manager integration deferred (not needed for personal use in v1).
   - Configurable models and language.

6. **Telemetry (Local)**
   - Minimal local metrics: dictation on/off, latency to final transcript.
   - No remote logging by default.

## Decisions Made

### Tauri over native Win32/C++
Tauri is confirmed as the shell. Rationale:
- The POC's WebAudio + WebSocket JavaScript ports directly into the Tauri WebView.
- `tauri build` produces MSI + NSIS installers out of the box — no manual packaging.
- Memory (~30-60 MB) is acceptable for a background dictation tool.
- The UI is minimal (button, status dot, settings form) — HTML/CSS handles this trivially.
- The hard parts (SendInput, hotkeys, tray) are Rust calls regardless of framework.
- Going pure Rust (egui/iced) or C++/Win32 adds significant dev time for no functional gain in v1.

### WebAudio capture (permanent — JS)
Audio capture stays in JavaScript via AudioWorklet in the Tauri WebView. This is a permanent decision. Rationale:
- Already proven in the POC — same code, same 24kHz mono PCM path.
- WebAudio is a stable W3C standard baked into Chromium/WebView2. No longevity risk.
- Tauri's WebView2 on Windows supports the full WebAudio API.
- Moving to `cpal` in Rust would mean rewriting audio capture for zero functional benefit.

### WebSocket in JavaScript for v1 — migrate to Rust post-v1
The OpenAI Realtime WebSocket lives in JavaScript for v1 to reuse POC code and ship faster. **Post-v1, the WebSocket will migrate to Rust** (`tokio-tungstenite`). Rationale:
- **v1 (JS):** POC code ports directly. Gets us to a working desktop app fast.
- **Post-v1 (Rust):** A dictation app is a background tool. Users will minimize it, work in other apps, and hotkey to dictate. WebView2 could throttle or suspend the WebSocket when the window is hidden. Moving the WebSocket to Rust makes the connection independent of WebView lifecycle — it survives minimization, tray-only mode, and any future WebView2 throttling changes.
- **Migration path:** Audio flows from JS AudioWorklet → Tauri IPC → Rust WebSocket → OpenAI. Audio capture stays in JS; only the network connection moves to Rust. ~150-200 lines of Rust to implement.

### Local JSON storage (not keychain)
API key stored via `tauri-plugin-store` in a local JSON file. Rationale:
- The key is already sent over the network to OpenAI — protecting it from the machine owner adds no real security.
- Keychain integration (Windows Credential Manager, macOS Keychain, libsecret) adds platform-specific complexity.
- Can upgrade to keychain storage later if distributing to a wider audience.

### Literal-only command matching
The "enter" command is matched only when the entire trimmed utterance is "enter" (case-insensitive). If "enter" appears within a sentence (e.g., "press enter to continue"), it is typed as text. This avoids false positives without needing an LLM classifier in v1.

## Technology Stack
- **Shell:** Tauri v2 (Rust backend + WebView2 frontend)
- **UI:** HTML/CSS/JS (minimal — status, toggle, settings)
- **Audio Capture:** WebAudio AudioWorklet in Tauri WebView (24kHz mono PCM)
- **STT:** OpenAI Realtime API (WebSocket — JS in v1, Rust `tokio-tungstenite` post-v1)
- **Typing / OS Automation:** Rust `SendInput` via `windows` crate
  - `KEYEVENTF_UNICODE` for text input
  - Fallback to clipboard paste (`Ctrl+V`) for apps that don't handle Unicode input
  - Inter-keystroke delay (~2-5ms) to avoid overwhelming target apps
- **Hotkeys:** Tauri global shortcut plugin
- **Tray:** Tauri system tray plugin
- **Storage:** `tauri-plugin-store` (local JSON)

## Architecture

### v1 (JS WebSocket)
```
Mic Audio (WebAudio AudioWorklet, 24kHz PCM)
  -> JS WebSocket to OpenAI Realtime (in WebView)
  -> Final transcript event (JS)
  -> Command filter (JS: "enter" -> Rust command, else -> Rust type text)
  -> Tauri invoke: Rust SendInput -> focused OS window
```

### Post-v1 (Rust WebSocket)
```
Mic Audio (WebAudio AudioWorklet, 24kHz PCM)
  -> Tauri IPC -> Rust backend
  -> Rust tokio-tungstenite WebSocket to OpenAI Realtime
  -> Final transcript event (Rust)
  -> Command filter (Rust)
  -> Rust SendInput -> focused OS window
```
Audio capture remains in JS. Only the WebSocket and downstream logic move to Rust.

### Key Data Flow (v1)
1. User presses hotkey (`Ctrl+Shift+D`) or clicks Start.
2. WebView requests mic access, starts AudioWorklet at 24kHz.
3. PCM audio streamed to OpenAI Realtime via JS WebSocket.
4. On `conversation.item.input_audio_transcription.completed`:
   - Trim transcript, check if entire utterance is "enter" (case-insensitive).
   - If yes -> invoke Rust command to send Enter keypress via `SendInput`.
   - If no -> invoke Rust command to type transcript text via `SendInput` + trailing space.
5. User presses hotkey or clicks Stop to end session.

## Installation & Distribution

### What Tauri Produces
`cargo tauri build` generates:
- **NSIS `.exe` installer** — per-user install, no admin required.
- **MSI installer** — system-wide install, requires admin.
- Both are self-contained. No runtime dependencies (no Python, no Node, no JRE).
- Typical binary size: 3-8 MB.

### Windows SmartScreen
Unsigned installers trigger "Windows protected your PC" warning on first run. Solutions:
- **Short-term:** Users click "More info" → "Run anyway." Acceptable for personal use / early testing.
- **Long-term:** Purchase a code signing certificate (~$200-400/year) to suppress the warning.

### Antivirus Considerations
Apps using `SendInput` can be flagged as keyloggers by antivirus software. Mitigations:
- Code signing significantly reduces false positives.
- Avoid suspicious patterns (don't capture keystrokes, only send them).
- If flagged, users can whitelist the app. Document this in a FAQ.

### Microphone Permissions
- Tauri WebView2 triggers a Windows-level mic permission prompt on first `getUserMedia` call.
- The app should show a setup/test screen before first use to handle this gracefully.
- Users configure and test their mic in settings before dictation is available.

## Implementation Plan
1. **Tauri app skeleton**
   - `cargo create-tauri-app` scaffold
   - Minimal UI: Start/Stop button, status dot, settings panel
   - Global hotkey toggle (`Ctrl+Shift+D`)
   - Tray icon with start/stop/quit

2. **Audio pipeline (port from POC)**
   - Port `audio-worklet.js` and `floatTo16BitPCM` from web app
   - Mic selection UI via `navigator.mediaDevices.enumerateDevices()`
   - Test/verify screen in settings

3. **OpenAI Realtime WebSocket (port from POC)**
   - Port WebSocket connection and `session.update` config from web app
   - Handle `completed` transcript events
   - Handle errors and connection drops

4. **OS typing (Rust)**
   - Implement `SendInput` with `KEYEVENTF_UNICODE` via `windows` crate
   - Enter keypress for "enter" command
   - Trailing space after text
   - Inter-keystroke delay
   - Clipboard-paste fallback for problematic apps

5. **Settings & storage**
   - UI form: API key, model, language, mic selection
   - `tauri-plugin-store` for persistence
   - Validate API key on save (test connection)

6. **Polish**
   - Status indicator (idle / listening / error)
   - Latency display (optional, dev/debug)
   - Autostart toggle
   - Error messages for common failures (no mic, bad API key, network down)

## Post-v1: Migrate WebSocket to Rust
After v1 is stable and working:
1. Add `tokio-tungstenite` and `base64` crates to the Rust backend.
2. Implement OpenAI Realtime WebSocket connection in Rust (session.update, audio streaming, transcript event parsing).
3. Change JS AudioWorklet to send PCM buffers to Rust via Tauri IPC instead of directly to a JS WebSocket.
4. Rust receives audio, forwards to OpenAI, receives transcripts, runs command filter, calls SendInput.
5. Remove JS WebSocket code. Audio capture remains in JS.

This decouples the network connection from WebView lifecycle, making dictation reliable in tray-only / minimized mode.

## Known Challenges

### Rust Learning Curve
The Rust backend is ~200-400 lines for v1 (SendInput, hotkey, tray, Tauri commands). The borrow checker and ownership model will slow initial development. The amount of Rust required is small and well-scoped.

### SendInput Edge Cases
- **Unicode:** `KEYEVENTF_UNICODE` works for most apps. Some older Win32 apps, certain terminals, and games may not process it correctly. Clipboard-paste fallback covers these.
- **Elevated windows:** `SendInput` is blocked when the target window runs as admin and the app doesn't. Admin terminals and some installers will be unresponsive. No clean fix — document as a known limitation.
- **Focus races:** If the user switches windows between speaking and the final transcript arriving, text types into the newly focused window. This is expected behavior — document it.
- **Typing speed:** Dumping a full sentence at once can overwhelm some apps. A 2-5ms inter-keystroke delay prevents this.

### Microphone Variance
Different machines have different default audio devices (USB mics, Bluetooth headsets, built-in arrays). The setup/test screen in settings handles this — users pick their mic and verify it works before using dictation.

### WebView Mic Permission
Tauri WebView2 handles mic permissions differently than Chrome. The first `getUserMedia` call triggers a system-level prompt. The settings/test screen should guide the user through this on first launch.

## Risks
- WebAudio capture in Tauri WebView2 may behave differently than in Chrome for edge-case audio devices.
- OS-level typing may be blocked by some secure/elevated apps.
- Antivirus false positives for SendInput-based automation.
- Latency spikes on long transcripts or slow network.

## Success Criteria
- Start/stop dictation reliably with hotkey.
- Final transcript appears in any focused text field within 1-2 seconds of pause.
- API key configurable via UI and persisted locally.
- App is stable across common Windows apps (Chrome, Word, VS Code, Notepad).
- Installer runs on a personal Windows laptop without requiring admin or special setup.
