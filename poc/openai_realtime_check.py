import asyncio
import os
import time
from pathlib import Path

import websockets
from dotenv import load_dotenv


async def main() -> None:
    load_dotenv(Path(__file__).resolve().parent / ".env.local")
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("OPENAI_API_KEY is missing from environment.")
        return

    model = os.getenv("OPENAI_REALTIME_MODEL", "gpt-realtime")
    url = f"wss://api.openai.com/v1/realtime?model={model}"
    headers = {"Authorization": f"Bearer {api_key}"}

    print(f"Connecting to {url} ...")
    start = time.perf_counter()
    try:
        async with websockets.connect(url, additional_headers=headers) as ws:
            elapsed = time.perf_counter() - start
            print(f"Handshake OK in {elapsed:.2f}s")
            await ws.send('{"type":"ping"}')
    except Exception as exc:
        elapsed = time.perf_counter() - start
        print(f"Handshake failed after {elapsed:.2f}s: {exc}")


if __name__ == "__main__":
    asyncio.run(main())
