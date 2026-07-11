#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.15"
# dependencies = [
#   "numpy>=2.5",
#   "pocket-tts==2.1.0",
# ]
# ///
"""Persistent CPU-streaming Pocket TTS worker.

stdout is strictly bounded JSON lines. Each chunk contains mono 24 kHz signed
16-bit PCM and is flushed immediately; diagnostics remain on stderr.
"""

from __future__ import annotations

import base64
import json
import os
import re
import sys
import time

MAX_TEXT_CHARS = 500
VOICE_RE = re.compile(r"^[a-z][a-z0-9_-]{0,63}$")


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


def safe_voice(value: object) -> str:
    if not isinstance(value, str) or VOICE_RE.fullmatch(value) is None:
        raise ValueError("invalid Pocket TTS voice")
    return value


def main() -> int:
    try:
        from pocket_tts import TTSModel

        model = TTSModel.load_model()
        configured = {
            "sven": os.environ.get("TTS_POCKET_VOICE_SVEN", "").strip()
            or "michael",
            "conny": os.environ.get("TTS_POCKET_VOICE_CONNY", "").strip()
            or "george",
            "ilse": os.environ.get("TTS_POCKET_VOICE_ILSE", "").strip() or "alba",
        }
        voices = {
            key: model.get_state_for_audio_prompt(safe_voice(value))
            for key, value in configured.items()
        }
    except Exception as error:
        print(
            f"[pocket-tts] model/voice loading failed: {type(error).__name__}",
            file=sys.stderr,
            flush=True,
        )
        send({"type": "error", "error": "Pocket TTS model or voices failed to load"})
        return 2

    send({"type": "ready", "sample_rate": model.sample_rate})
    for line in sys.stdin:
        rid: int | None = None
        try:
            raw = json.loads(line)
            if not isinstance(raw, dict) or set(raw) != {
                "request_id",
                "text",
                "voice_key",
            }:
                raise ValueError("invalid synthesis request")
            rid = request_id(raw["request_id"])
            text = bounded_text(raw["text"])
            voice_key = safe_voice(raw["voice_key"])
            voice_state = voices.get(voice_key)
            if voice_state is None:
                raise ValueError("unknown logical NPC voice")

            started = time.monotonic()
            chunk_count = 0
            first_chunk_ms: int | None = None
            for chunk in model.generate_audio_stream(voice_state, text):
                audio = chunk.detach().cpu().clamp(-1.0, 1.0).numpy()
                pcm = (audio * 32767.0).astype("<i2", copy=False).tobytes()
                if not pcm:
                    continue
                if first_chunk_ms is None:
                    first_chunk_ms = round((time.monotonic() - started) * 1000)
                send(
                    {
                        "type": "chunk",
                        "request_id": rid,
                        "chunk_seq": chunk_count,
                        "sample_rate": model.sample_rate,
                        "pcm_s16le_base64": base64.b64encode(pcm).decode("ascii"),
                    }
                )
                chunk_count += 1
            if chunk_count == 0:
                raise RuntimeError("Pocket TTS produced no audio")
            send(
                {
                    "type": "result",
                    "request_id": rid,
                    "chunk_count": chunk_count,
                    "first_chunk_ms": first_chunk_ms,
                }
            )
        except Exception as error:
            print(
                f"[pocket-tts] synthesis failed: {type(error).__name__}",
                file=sys.stderr,
                flush=True,
            )
            send(
                {
                    "type": "error",
                    "request_id": rid,
                    "error": "local Pocket TTS synthesis failed",
                }
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
