# Moonshine Offline STT Spike

This branch evaluates Moonshine as the first offline speech-to-text path for
Mango Chat.

## Goals

- offline speech-to-text
- English only
- low latency suitable for dictation
- lightweight model/runtime footprint
- preserve the app's existing local VAD-driven utterance boundaries

## Current App Reality

- Mango Chat already has local WebRTC VAD and utterance commit behavior.
- Mango Chat does not have any offline recognizer today.
- The current provider abstraction is WebSocket-oriented, so the first Moonshine
  spike should avoid forcing the local recognizer into the cloud-provider path.

## Research Summary

The strongest current Moonshine candidate for the stated constraints is the
Moonshine tiny English int8 packaging distributed through sherpa-onnx.

Official sources reviewed:

- Moonshine upstream GitHub: https://github.com/moonshine-ai/moonshine
- Moonshine tiny model card: https://huggingface.co/UsefulSensors/moonshine-tiny
- sherpa-onnx Moonshine model packaging:
  https://k2-fsa.github.io/sherpa/onnx/moonshine/models.html

Important points from those sources:

- Moonshine upstream positions itself for low-latency, on-device speech
  interfaces and supports Windows.
- The original Moonshine v1 model family is English-only.
- sherpa-onnx publishes 8-bit quantized Moonshine tiny English models.
- The listed Moonshine tiny int8 package is roughly 117 MB across ONNX model
  files, which keeps it within the current model-size target.

Observed branch-local setup using the official `moonshine-voice` Windows wheel:

- downloaded `tiny-en` assets: about 74 MB
- downloaded `tiny-streaming-en` assets: about 84 MB
- downloaded `medium-streaming-en` assets: about 449 MB

For this spike, `tiny-en` and `tiny-streaming-en` fit the current storage target.
`medium-streaming-en` does not.

## Environment Notes

Current machine status when this branch was created:

- Python is available on PATH.
- CMake is not available on PATH.
- Visual Studio Build Tools are installed under `C:\VSBuildTools\Installed`.

The official Moonshine Windows quickstart expects:

- Python package install for model downloading
- native build tooling
- Visual Studio / MSBuild on Windows

## Branch Prep

This branch reserves the following local-only directories:

- `.models/`
- `.native-build/`

They are ignored in Git and intended for:

- downloaded Moonshine / ONNX model artifacts
- native build outputs
- temporary runtime experiments

## First Implementation Target

Phase 1 should prove this loop only:

1. microphone audio capture
2. local VAD boundaries from existing Mango Chat logic
3. offline Moonshine transcription
4. final text flowing into the existing typing/action pipeline

Non-goals for the first spike:

- full provider settings UI redesign
- replacing existing cloud providers
- reworking updater/release packaging
- multi-turn diarization
- swapping WebRTC VAD for Silero VAD

## Intended Integration Shape

Preferred direction for the first spike:

- add a local recognizer path alongside the current WebSocket provider path
- keep existing `typing::process_transcript()` flow
- keep existing VAD-driven commit semantics
- defer any provider-abstraction redesign until after local transcription works

## Immediate Next Steps

1. Run `.\scripts\setup-moonshine.ps1`
2. Run `.\scripts\smoke-test-moonshine.ps1`
3. Add minimal local recognizer scaffolding behind a branch-local feature path
4. Build a narrow microphone-to-transcript proof of concept
