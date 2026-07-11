#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.13"
# dependencies = [
#   "kokoro==0.9.4",
#   "en-core-web-sm @ https://github.com/explosion/spacy-models/releases/download/en_core_web_sm-3.8.0/en_core_web_sm-3.8.0-py3-none-any.whl",
#   "soundfile>=0.13,<1",
# ]
# ///
"""Persistent Kokoro-82M JSON-lines synthesis worker.

stdout is reserved for bounded machine messages. Dependency/model diagnostics
remain on stderr where uv and Kokoro naturally write them.
"""

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

MAX_TEXT_CHARS = 500
VOICE_RE = re.compile(r"^[a-z]{2}_[a-z0-9_]{1,61}$")
MODEL_RE = re.compile(r"^[A-Za-z0-9_.-]{1,64}/[A-Za-z0-9_.-]{1,64}$")


def send(message: dict[str, object]) -> None:
    sys.stdout.write(json.dumps(message, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def request_id(value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise ValueError("invalid request ID")
    return value


def bounded_text(value: object) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > MAX_TEXT_CHARS:
        raise ValueError("invalid synthesis text")
    value.encode("utf-8")
    if any(
        (ord(character) < 0x20 and character not in "\n\t")
        or 0x7F <= ord(character) <= 0x9F
        for character in value
    ):
        raise ValueError("invalid synthesis text")
    return value


def safe_basename(value: object) -> str:
    if (
        not isinstance(value, str)
        or len(value) > 128
        or Path(value).name != value
        or not value.endswith(".wav")
    ):
        raise ValueError("invalid WAV basename")
    return value


def safe_voice(value: object) -> str:
    if not isinstance(value, str) or VOICE_RE.fullmatch(value) is None:
        raise ValueError("invalid Kokoro voice")
    return value


def main() -> int:
    runtime_value = os.environ.get("KOKORO_RUNTIME_DIR", "")
    runtime_dir = Path(runtime_value).resolve()
    if not runtime_dir.is_dir():
        send({"type": "error", "error": "local voice runtime is unavailable"})
        return 2

    try:
        from kokoro import KPipeline
        import numpy as np
        import soundfile as sf

        model = os.environ.get("LOCAL_TTS_MODEL", "hexgrad/Kokoro-82M").strip()
        if MODEL_RE.fullmatch(model) is None:
            raise ValueError("invalid local TTS model identifier")
        device = os.environ.get("LOCAL_TTS_DEVICE", "cpu").strip().lower()
        if device not in {"cpu", "cuda"}:
            raise ValueError("LOCAL_TTS_DEVICE must be cpu or cuda")
        # The initial cast speaks English. Provider-qualified environment
        # mappings can choose another verified voice, but language selection
        # deliberately remains pinned for deterministic pronunciation.
        pipeline = KPipeline(lang_code="a", repo_id=model, device=device)
    except Exception as error:
        print(
            f"[kokoro] model loading failed: {type(error).__name__}",
            file=sys.stderr,
            flush=True,
        )
        send({"type": "error", "error": "Kokoro model failed to load"})
        return 2

    send({"type": "ready"})
    for line in sys.stdin:
        rid: int | None = None
        temporary: Path | None = None
        try:
            raw = json.loads(line)
            if not isinstance(raw, dict) or set(raw) != {
                "request_id",
                "text",
                "voice",
                "wav_basename",
            }:
                raise ValueError("invalid synthesis request")
            rid = request_id(raw["request_id"])
            text = bounded_text(raw["text"])
            voice = safe_voice(raw["voice"])
            basename = safe_basename(raw["wav_basename"])
            output = (runtime_dir / basename).resolve(strict=False)
            if output.parent != runtime_dir:
                raise ValueError("invalid WAV path")
            temporary = output.with_suffix(".wav.part")
            temporary.unlink(missing_ok=True)
            chunks = [audio for _, _, audio in pipeline(text, voice=voice)]
            if not chunks:
                raise RuntimeError("Kokoro produced no audio")
            audio = chunks[0] if len(chunks) == 1 else np.concatenate(chunks)
            sf.write(temporary, audio, 24_000, format="WAV")
            temporary.replace(output)
            send(
                {
                    "type": "result",
                    "request_id": rid,
                    "wav_basename": basename,
                }
            )
        except Exception as error:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
            print(
                f"[kokoro] synthesis failed: {type(error).__name__}",
                file=sys.stderr,
                flush=True,
            )
            send(
                {
                    "type": "error",
                    "request_id": rid,
                    "error": "local Kokoro synthesis failed",
                }
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
