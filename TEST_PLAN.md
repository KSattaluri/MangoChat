# Jarvis Windows Test Plan (Draft v1)

## 1. Goals
- Validate core dictation reliability on Windows.
- Catch regressions in transcription flow, usage tracking, and settings behavior.
- Separate fast automated checks from slower/manual OS-integration checks.

## 2. Scope
- In scope:
  - Core Rust logic (parsing, usage logging, provider event handling).
  - Session behavior (commit boundaries, counters, reconnect handling).
  - Windows app smoke behavior (launch, hotkey record cycle, usage updates).
- Out of scope for strict automation (initially):
  - Real microphone/audio quality validation across hardware.
  - Visual/tray rendering fidelity across all Windows themes/scales.

## 3. Test Layers

### Layer A: Unit Tests (Rust, fastest)
- Target modules:
  - `src/typing.rs`
  - `src/usage.rs`
  - `src/provider/*.rs` parse-event behavior
- Run on every change.
- Goal: deterministic logic correctness.

### Layer B: Integration Tests (Rust + mocks/fakes)
- Target modules:
  - `src/provider/session.rs` with mocked websocket/provider events.
  - VAD/send-counter logic with synthetic audio where possible.
- Run on PR and nightly.
- Goal: session correctness and edge-case handling.

### Layer C: Windows UI Automation (slower)
- Recommended tool options:
  - Preferred: FlaUI (.NET) for robust desktop UI automation.
  - Alternate: `pywinauto` (Python) for faster scripting.
  - Optional helper: AutoHotkey for global hotkey scenarios.
- Run nightly or pre-release in clean VM snapshot.
- Goal: real app workflow validation on Windows.

### Layer D: Manual Regression Checklist
- Small release checklist for hardware-dependent behavior.
- Run before release or after major provider/input changes.

## 4. Environment Strategy
- Use disposable, resettable environment:
  - Hyper-V VM snapshot (preferred) or Windows Sandbox image.
- Test runs should start from clean state:
  - Clear `%LOCALAPPDATA%\Jarvis\` test artifacts.
  - Reset test config and usage logs.
  - Start app in test mode (to be added) using deterministic fake audio.

## 5. Recommended Testability Improvements
- Add `--test-mode` (or env flag) to:
  - Disable real mic capture and accept fixture audio input.
  - Redirect settings/usage paths to temp test directory.
  - Emit structured log markers for assertions.
- Abstract OS integrations behind interfaces where possible:
  - Typing output sink.
  - Hotkey trigger source.
  - Clipboard/explorer launch actions.

## 6. Automated Test Matrix (Initial)

### A. Unit tests (automated now)
1. `typing::normalize`:
   - punctuation/case/whitespace normalization.
   - wake-word variants handling.
2. Command matching:
   - longest command wins (`back back` vs `back`).
   - standalone commands vs wake-word prefixed commands.
3. Usage logs (`usage.jsonl`, `usage-session.jsonl`):
   - append and load latest entry.
   - truncation retains max lines.
   - reset totals file behavior.
4. Provider parser tests:
   - OpenAI: delta/completed/error payloads.
   - Deepgram: interim/final/speech_final/utterance end.
   - AssemblyAI: turn + end_of_turn behavior.
   - ElevenLabs: partial/committed transcript behavior.

### B. Integration tests (automate next)
1. Session send/commit counters:
   - bytes/ms increment after successful send.
   - commit increments only when provider commit message exists.
2. VAD boundary behavior:
   - silence -> speech -> silence causes expected commit signal.
3. Reconnect and shutdown behavior:
   - transient websocket failure reconnects.
   - closed audio channel exits cleanly.

### C. Windows UI automation (automate in VM)
1. App launch smoke:
   - process starts, window/tray present, no crash.
2. Recording cycle:
   - simulate Right Ctrl hold/release.
   - verify status transitions and usage counters update.
3. Settings persistence:
   - update provider/model/mic setting.
   - restart app and verify persisted values.
4. Usage view:
   - totals visible and session history row created after session stop.

## 7. Manual Regression Checklist
1. Dictate in Notepad and verify text appears correctly.
2. Verify global hotkey behavior while focus is in another app.
3. Confirm tray actions (arm/disarm/show/quit) work as expected.
4. Validate snip flow and clipboard behavior.
5. Network interruption during dictation surfaces clear recovery behavior.

## 8. Pass/Fail Criteria
- Unit tests: 100% pass required.
- Integration tests: 100% pass required for merge.
- Windows UI suite: all critical scenarios pass in clean VM snapshot.
- Manual checklist: no critical functional failures.

## 9. CI/CD Proposal
- PR pipeline:
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`
  - `cargo test` (unit + integration excluding UI)
- Nightly pipeline:
  - PR checks plus Windows UI automation in reset VM.
- Release gate:
  - latest nightly green + manual regression checklist complete.

## 10. Suggested First Milestone (1 week)
1. Add/expand unit tests for `typing`, `usage`, and provider parsing.
2. Add basic integration tests for session counter increments.
3. Draft `--test-mode` requirements (no full implementation yet).
4. Create minimal Windows UI smoke script (launch + quit + artifact capture).

## 11. Open Questions for Review
1. Which UI automation framework do you want as primary (`FlaUI` or `pywinauto`)?
2. Do you want `--test-mode` as CLI flag, env var, or both?
3. Should nightly UI tests run on every branch or only main/release branches?
