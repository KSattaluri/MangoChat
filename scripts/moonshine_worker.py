import argparse
import array
import base64
import json
import sys

from moonshine_voice import LineCompleted, LineTextChanged, ModelArch, TranscriptEventListener, Transcriber


def pcm16le_to_f32_list(audio_bytes: bytes):
    samples = array.array("h")
    samples.frombytes(audio_bytes)
    if sys.byteorder != "little":
        samples.byteswap()
    return [sample / 32768.0 for sample in samples]


class JsonListener(TranscriptEventListener):
    def __init__(self):
        self.last_delta = ""

    def on_line_text_changed(self, event: LineTextChanged) -> None:
        text = getattr(event.line, "text", "").strip()
        if text and text != self.last_delta:
            self.last_delta = text
            print(json.dumps({"type": "delta", "text": text}), flush=True)

    def on_line_completed(self, event: LineCompleted) -> None:
        text = getattr(event.line, "text", "").strip()
        self.last_delta = ""
        if text:
            print(json.dumps({"type": "final", "text": text}), flush=True)

    def on_error(self, event) -> None:
        print(json.dumps({"type": "error", "message": str(event.error)}), flush=True)


class Worker:
    def __init__(self, model_path: str, model_arch: int, update_interval: float = 0.12):
        self.transcriber = Transcriber(
            model_path,
            model_arch=ModelArch(model_arch),
            update_interval=update_interval,
        )
        self.listener = JsonListener()
        self.stream = None
        self.reset_stream()

    def reset_stream(self):
        if self.stream is not None:
            try:
                self.stream.close()
            except Exception:
                pass
        self.stream = self.transcriber.create_stream(update_interval=0.12)
        self.stream.add_listener(self.listener)
        self.stream.start()

    def add_audio(self, audio_bytes: bytes, sample_rate: int):
        audio_data = pcm16le_to_f32_list(audio_bytes)
        if audio_data:
            self.stream.add_audio(audio_data, sample_rate)

    def commit(self):
        self.stream.stop()
        self.reset_stream()

    def close(self):
        if self.stream is not None:
            try:
                self.stream.stop()
            except Exception:
                pass
            try:
                self.stream.close()
            except Exception:
                pass
            self.stream = None
        self.transcriber.close()


def main():
    parser = argparse.ArgumentParser(description="Persistent Moonshine streaming worker")
    parser.add_argument("--model-path", required=True)
    parser.add_argument("--model-arch", type=int, default=2)
    args = parser.parse_args()

    try:
        worker = Worker(args.model_path, args.model_arch)
    except Exception as exc:
        print(json.dumps({"type": "error", "message": str(exc)}), flush=True)
        return 1

    print(
        json.dumps(
            {
                "type": "ready",
                "model_path": args.model_path,
                "model_arch": args.model_arch,
            }
        ),
        flush=True,
    )

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except Exception as exc:
            print(json.dumps({"type": "error", "message": f"invalid request: {exc}"}), flush=True)
            continue

        req_type = request.get("type")
        try:
            if req_type == "shutdown":
                break
            if req_type == "audio":
                sample_rate = int(request.get("sample_rate", 16000))
                audio_b64 = request.get("audio_b64", "")
                audio_bytes = base64.b64decode(audio_b64)
                worker.add_audio(audio_bytes, sample_rate)
                continue
            if req_type == "commit":
                worker.commit()
                continue
            print(json.dumps({"type": "error", "message": f"unknown request type: {req_type}"}), flush=True)
        except Exception as exc:
            print(json.dumps({"type": "error", "message": str(exc)}), flush=True)

    worker.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
