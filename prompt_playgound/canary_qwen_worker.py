#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = [
#   "llvmlite==0.43.0",
#   "nemo_toolkit[asr]==2.7.3",
#   "numba==0.60.0",
#   "numpy==1.26.4",
#   "torch==2.6.0",
# ]
# ///
"""Private persistent FP16 Canary-Qwen transcription worker.

stdout is a machine-readable JSON-lines channel owned by speech_client.py.
NeMo diagnostics are redirected to stderr so they cannot corrupt it.
"""

from __future__ import annotations

import json
import os
import sys
from contextlib import redirect_stdout
from pathlib import Path
from typing import Any

MODEL = os.environ.get("LOCAL_STT_MODEL", "").strip() or "nvidia/canary-qwen-2.5b"


def send(payload: dict[str, object]) -> None:
    sys.stdout.write(json.dumps(payload, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def load_model() -> Any:
    # Some NeMo dependencies log to stdout during import and checkpoint load.
    # Keep the parent protocol strictly one JSON object per line.
    with redirect_stdout(sys.stderr):
        import torch
        import nemo.collections.speechlm2 as speechlm2

        if not torch.cuda.is_available():
            raise RuntimeError("CUDA is not available")
        model = speechlm2.models.SALM.from_pretrained(MODEL)
        model = model.to(device="cuda", dtype=torch.float16).eval()
    return model


def transcribe(model: Any, wav_path: Path) -> str:
    import torch

    prompt = [
        {
            "role": "user",
            "content": f"Transcribe the following: {model.audio_locator_tag}",
            "audio": [str(wav_path)],
        }
    ]
    with redirect_stdout(sys.stderr), torch.inference_mode():
        answer_ids = model.generate(prompts=[prompt], max_new_tokens=256)
        return model.tokenizer.ids_to_text(answer_ids[0].cpu())


def main() -> int:
    try:
        model = load_model()
    except Exception as error:
        print(
            f"[canary-qwen] model startup failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        send(
            {
                "type": "fatal",
                "error": "local Canary-Qwen failed to load; check CUDA, dependencies, and available VRAM",
            }
        )
        return 1
    send({"type": "ready", "model": MODEL, "precision": "fp16"})

    for line in sys.stdin:
        request: object = None
        try:
            request = json.loads(line)
            request_id = request["request_id"]
            wav_path = Path(request["wav_path"])
            if (
                isinstance(request_id, bool)
                or not isinstance(request_id, int)
                or wav_path.suffix.lower() != ".wav"
                or not wav_path.is_file()
            ):
                raise ValueError("invalid transcription request")
            text = transcribe(model, wav_path)
            if not isinstance(text, str):
                raise RuntimeError("model returned no text")
            send({"type": "result", "request_id": request_id, "text": text})
        except Exception as error:
            print(
                f"[canary-qwen] transcription failed: {type(error).__name__}: {error}",
                file=sys.stderr,
            )
            send(
                {
                    "type": "error",
                    "request_id": request.get("request_id")
                    if isinstance(request, dict)
                    else None,
                    "error": "local Canary-Qwen transcription failed; press Z to use cloud transcription",
                }
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
