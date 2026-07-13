#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["requests", "python-dotenv"]
# ///
"""Generate the sound assets the catalog (prompt_playgound/sounds.py) defines.

Idempotent by design: reads the catalog, diffs against assets/sounds/, and
generates only what is missing via ElevenLabs sound generation. Don't like a
sound? Delete the file and re-run — nothing else regenerates, nothing else
costs credits.

One-shots land as mp3 (what ElevenLabs returns on every tier). Ambient loops
must be wav — rodio does not honour LAME gapless tags, so a looped mp3 clicks
at the wrap point — and are converted locally with ffmpeg.

`ELEVENLABS_API_KEY` lives in the repo-root `.env` (gitignored).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import requests
from dotenv import load_dotenv

REPO_ROOT = Path(__file__).resolve().parents[1]
ASSET_DIR = REPO_ROOT / "assets" / "sounds"
API_URL = "https://api.elevenlabs.io/v1/sound-generation"

sys.path.insert(0, str(REPO_ROOT / "prompt_playgound"))
from sounds import AMBIENT, SOUNDS  # noqa: E402


def generate(api_key: str, prompt: str, duration_seconds: float) -> bytes:
    response = requests.post(
        API_URL,
        headers={"xi-api-key": api_key},
        json={
            "text": prompt,
            # The API accepts 0.5-22 s.
            "duration_seconds": min(max(duration_seconds, 0.5), 22.0),
            "prompt_influence": 0.3,
        },
        timeout=120,
    )
    response.raise_for_status()
    return response.content


def convert_to_wav(mp3_bytes: bytes, target: Path) -> bool:
    ffmpeg = shutil.which("ffmpeg")
    if ffmpeg is None:
        return False
    with tempfile.NamedTemporaryFile(suffix=".mp3") as source:
        source.write(mp3_bytes)
        source.flush()
        subprocess.run(
            [ffmpeg, "-y", "-loglevel", "error", "-i", source.name, str(target)],
            check=True,
        )
    return True


def main() -> int:
    load_dotenv(REPO_ROOT / ".env")
    api_key = os.environ.get("ELEVENLABS_API_KEY", "").strip()
    if not api_key:
        print("ELEVENLABS_API_KEY is not set (repo-root .env)", file=sys.stderr)
        return 2
    ASSET_DIR.mkdir(parents=True, exist_ok=True)

    generated = 0
    for sound in SOUNDS.values():
        target = ASSET_DIR / f"{sound.sound_id}.mp3"
        if target.exists():
            print(f"  keep     {target.relative_to(REPO_ROOT)}")
            continue
        print(f"  generate {target.relative_to(REPO_ROOT)} ({sound.duration_seconds}s)")
        target.write_bytes(generate(api_key, sound.sfx_prompt, sound.duration_seconds))
        generated += 1

    for ambient in AMBIENT.values():
        target = ASSET_DIR / f"{ambient.sound_id}.wav"
        if target.exists():
            print(f"  keep     {target.relative_to(REPO_ROOT)}")
            continue
        print(
            f"  generate {target.relative_to(REPO_ROOT)} ({ambient.duration_seconds}s)"
        )
        mp3_bytes = generate(api_key, ambient.sfx_prompt, ambient.duration_seconds)
        if not convert_to_wav(mp3_bytes, target):
            fallback = target.with_suffix(".mp3")
            fallback.write_bytes(mp3_bytes)
            print(
                f"  warning: ffmpeg not found; wrote {fallback.name} instead "
                "(a looped mp3 clicks at the wrap point)",
                file=sys.stderr,
            )
        generated += 1

    print(f"done: {generated} generated, catalog satisfied")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
