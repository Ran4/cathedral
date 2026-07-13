#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["requests", "python-dotenv"]
# ///
"""Generate the sound assets the catalog (assets/sounds/catalog.toml) defines.

The catalog is the single source of truth: the sim emits sound events and
renders their percepts from it, Bevy resolves playback by convention from the
id alone, and this script synthesizes each row's asset from its `sfx_prompt`.
It used to import `prompt_playgound/sounds.py`; that module went with the rest
of the Python sidecar, and the TOML it became says exactly the same thing.

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
import tomllib
from pathlib import Path

import requests
from dotenv import load_dotenv

REPO_ROOT = Path(__file__).resolve().parents[1]
ASSET_DIR = REPO_ROOT / "assets" / "sounds"
CATALOG_PATH = ASSET_DIR / "catalog.toml"
API_URL = "https://api.elevenlabs.io/v1/sound-generation"


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

    catalog = tomllib.loads(CATALOG_PATH.read_text(encoding="utf-8"))
    ASSET_DIR.mkdir(parents=True, exist_ok=True)

    generated = 0
    for sound in catalog.get("sounds", []):
        target = ASSET_DIR / f"{sound['sound_id']}.mp3"
        if target.exists():
            print(f"  keep     {target.relative_to(REPO_ROOT)}")
            continue
        duration = sound["duration_seconds"]
        print(f"  generate {target.relative_to(REPO_ROOT)} ({duration}s)")
        target.write_bytes(generate(api_key, sound["sfx_prompt"], duration))
        generated += 1

    for ambient in catalog.get("ambients", []):
        target = ASSET_DIR / f"{ambient['sound_id']}.wav"
        if target.exists():
            print(f"  keep     {target.relative_to(REPO_ROOT)}")
            continue
        duration = ambient["duration_seconds"]
        print(f"  generate {target.relative_to(REPO_ROOT)} ({duration}s)")
        mp3_bytes = generate(api_key, ambient["sfx_prompt"], duration)
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
