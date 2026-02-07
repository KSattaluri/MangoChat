import asyncio
import base64
import json
import os
from pathlib import Path

from dotenv import load_dotenv
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles
from openai import OpenAI
from pydantic import BaseModel
import websockets


ROOT = Path(__file__).resolve().parent
load_dotenv(ROOT / ".env.local")

OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")
OPENAI_REALTIME_MODEL = os.getenv("OPENAI_REALTIME_MODEL", "gpt-realtime")
OPENAI_TRANSCRIPTION_MODEL = os.getenv(
    "OPENAI_TRANSCRIPTION_MODEL", "gpt-4o-mini-transcribe"
)
OPENAI_REVISE_MODEL = os.getenv("OPENAI_REVISE_MODEL", "gpt-4.1-mini")

app = FastAPI()
static_dir = Path(__file__).resolve().parent / "static"
app.mount("/static", StaticFiles(directory=static_dir), name="static")

openai_client = OpenAI()


def _append_text(base: str, text: str) -> str:
    if not base:
        return text
    if base.endswith("\n"):
        return base + text
    return base + " " + text


class ReviseRequest(BaseModel):
    raw_text: str
    commands: list[str] = []


def _revise_call(raw_text: str, commands: list[str]) -> str:
    response = openai_client.responses.create(
        model=OPENAI_REVISE_MODEL,
        input=[
            {
                "role": "system",
                "content": (
                    "You are an editor. Apply the user's command blocks to the raw "
                    "transcript and return the revised text only. The raw transcript "
                    "includes inline markers like 'command' and 'end command'. "
                    "Remove all command markers and command-only content from the output. "
                    "Preserve dictation content and punctuation unless a command changes it."
                ),
            },
            {
                "role": "user",
                "content": json.dumps(
                    {"raw_text": raw_text, "commands": commands},
                    ensure_ascii=True,
                ),
            },
        ],
    )
    return response.output_text or ""


@app.get("/")
def index() -> FileResponse:
    return FileResponse(static_dir / "index.html")


@app.post("/revise")
def revise(request: ReviseRequest):
    if not OPENAI_API_KEY:
        return {"error": "OPENAI_API_KEY is missing in .env.local"}
    if not request.raw_text.strip():
        return {"error": "No text to revise"}
    try:
        revised = _revise_call(request.raw_text, request.commands)
        return {"revised_text": revised}
    except Exception as exc:
        return {"error": f"Revision failed: {exc}"}




@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    if not OPENAI_API_KEY:
        await websocket.send_text(
            json.dumps(
                {
                    "type": "error",
                    "message": "OPENAI_API_KEY is missing in .env.local",
                }
            )
        )
        await websocket.close()
        return

    url = f"wss://api.openai.com/v1/realtime?model={OPENAI_REALTIME_MODEL}"
    headers = {"Authorization": f"Bearer {OPENAI_API_KEY}"}
    session_update = {
        "type": "session.update",
        "session": {
            "type": "realtime",
            "audio": {
                "input": {
                    "format": {"type": "audio/pcm", "rate": 24000},
                    "noise_reduction": {"type": "near_field"},
                    "transcription": {
                        "model": OPENAI_TRANSCRIPTION_MODEL,
                        "language": "en",
                    },
                    "turn_detection": {
                        "type": "server_vad",
                        "threshold": 0.5,
                        "prefix_padding_ms": 300,
                        "silence_duration_ms": 500,
                    },
                }
            },
        },
    }

    raw_text = ""

    try:
        async with websockets.connect(url, additional_headers=headers) as openai_ws:
            await openai_ws.send(json.dumps(session_update))

            async def forward_client_audio() -> None:
                try:
                    while True:
                        message = await websocket.receive()
                        if "bytes" in message and message["bytes"]:
                            audio_b64 = base64.b64encode(message["bytes"]).decode(
                                "ascii"
                            )
                            await openai_ws.send(
                                json.dumps(
                                    {
                                        "type": "input_audio_buffer.append",
                                        "audio": audio_b64,
                                    }
                                )
                            )
                        elif message.get("text") == "stop":
                            await openai_ws.send(
                                json.dumps({"type": "input_audio_buffer.commit"})
                            )
                            break
                except WebSocketDisconnect:
                    return
                except RuntimeError:
                    return

            async def forward_openai_events() -> None:
                nonlocal raw_text
                async for raw in openai_ws:
                    event = json.loads(raw)
                    event_type = event.get("type")
                    if event_type == "conversation.item.input_audio_transcription.delta":
                        await websocket.send_text(
                            json.dumps(
                                {
                                    "type": "transcript",
                                    "text": event.get("delta", ""),
                                    "is_final": False,
                                    "speech_final": False,
                                }
                            )
                        )
                    elif (
                        event_type
                        == "conversation.item.input_audio_transcription.completed"
                    ):
                        transcript = (event.get("transcript") or "").strip()
                        if not transcript:
                            continue

                        await websocket.send_text(
                            json.dumps(
                                {
                                    "type": "transcript",
                                    "text": transcript,
                                    "is_final": True,
                                    "speech_final": True,
                                }
                            )
                        )

                        raw_text = _append_text(raw_text, transcript)
                    elif event_type == "error":
                        message = event.get("error", {}).get(
                            "message", "OpenAI error"
                        )
                        await websocket.send_text(
                            json.dumps({"type": "error", "message": message})
                        )

            client_task = asyncio.create_task(forward_client_audio())
            openai_task = asyncio.create_task(forward_openai_events())
            done, pending = await asyncio.wait(
                [client_task, openai_task], return_when=asyncio.FIRST_COMPLETED
            )
            for task in pending:
                task.cancel()
    except Exception as exc:
        await websocket.send_text(
            json.dumps({"type": "error", "message": f"OpenAI connection failed: {exc}"})
        )
    finally:
        try:
            await websocket.close()
        except RuntimeError:
            pass


if __name__ == "__main__":
    import uvicorn

    uvicorn.run("app:app", host="127.0.0.1", port=8000, reload=False)
