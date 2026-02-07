# Jarvis (Strict Dictation)

## What it is
Minimal local web UI that streams microphone audio to OpenAI Realtime and renders a live transcript.
Strict mode: it appends text exactly as spoken.

## Setup (from repo root)
1) Install deps:
```bash
uv sync
```

2) Ensure `.env.local` in repo root includes:
```
OPENAI_API_KEY=...
# Optional:
# OPENAI_REALTIME_MODEL=gpt-realtime
# OPENAI_TRANSCRIPTION_MODEL=gpt-4o-mini-transcribe
# OPENAI_REVISE_MODEL=gpt-4.1-mini
```

3) Run the app:
```bash
uv run python jarvis/app.py
```

4) Open:
```
http://127.0.0.1:8000
```

## Notes
- Audio is captured in the browser and streamed as 24kHz mono PCM.
- Final transcripts are appended; interim text appears in gray.
- Say `command ... end command` to mark edits, then click **Revise** to apply.

