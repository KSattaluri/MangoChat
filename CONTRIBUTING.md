# Contributing to Mango Chat

Thanks for contributing.

## Scope

This repository contains the Windows desktop app (`mangochat.exe`) and installer build flow (`MangoChat-Setup.exe`).

Primary goals:
- Reliable dictation/transcription behavior
- Stable window placement and compact UI behavior
- Safe release and upgrade flow

## Development Setup

Use `README.md` for environment setup and build instructions.

Quick start:

```powershell
cargo check
cargo run
```

## Branching and PRs

- `master` is release-ready.
- Use feature branches for all changes.
- Keep PRs focused (one concern per PR when possible).
- Do not push broken builds to `master`.

Recommended branch naming:
- `feature/<topic>`
- `fix/<topic>`

## Code Guidelines

- Keep changes minimal and targeted.
- Prefer clear and explicit behavior over implicit side effects.
- Preserve existing monitor placement and audio/session behavior unless intentionally changing it.
- Avoid adding global hooks or background polling without a clear need and a user-visible setting.

## UI Changes

- Maintain consistency with existing compact/settings behavior.
- Validate on multi-monitor setups when touching positioning logic.
- Validate compact + expanded modes and screenshot controls.

## Security and Secrets

- Never commit keys, tokens, or credentials.
- API keys are stored via app runtime secret storage (encrypted at rest on Windows).
- Keep operational runbooks local-only unless explicitly intended for public docs.

## Testing Checklist

Before opening a PR:

1. `cargo check` succeeds.
2. `cargo run` starts and UI renders.
3. Recording start/stop works.
4. Screenshot flow works (if enabled).
5. Settings save/load works.
6. No regressions in monitor placement behavior.

If installer/build logic changes:

1. `cargo build --release` succeeds.
2. `.\scripts\build-installer.ps1` succeeds.
3. Installer output launches app correctly.

## Commit Messages

Use short imperative subject lines, for example:
- `Refactor provider tab rendering`
- `Fix compact window anchor on secondary monitor`
- `Add update check state to General tab`

## Release Notes Discipline

For user-visible changes, include:
- What changed
- Why it changed
- Any migration or user action required

