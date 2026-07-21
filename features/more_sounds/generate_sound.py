#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["requests", "python-dotenv"]
# ///
"""Generate or regenerate one More Sounds candidate through ElevenLabs.

This is the single-sound adapter used by ``server.py``. It deliberately reuses
the provider and loop-conversion functions in ``scripts/generate_sounds.py`` so
the workbench follows the game's established generation path.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path
from typing import Any

from dotenv import load_dotenv


HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
CATALOG_PATH = HERE / "more_sounds.json"
GENERATED_DIR = HERE / "generated"

# ``scripts`` is intentionally not a Python package. Import its established
# generator as a module without copying its HTTP or ffmpeg behavior here.
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from generate_sounds import convert_to_wav, generate  # noqa: E402


def atomic_write_bytes(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            temporary = Path(handle.name)
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except OSError:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        raise


def load_sound(sound_id: str) -> dict[str, Any]:
    catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))
    for sound in catalog.get("sounds", []):
        if sound.get("id") == sound_id:
            return sound
    raise ValueError(f"Unknown sound id: {sound_id}")


def generate_one(sound: dict[str, Any], api_key: str) -> Path:
    sound_id = sound["id"]
    prompt = sound["generation_prompt"]
    duration = float(sound["suggested_duration_seconds"])
    playback_mode = sound["playback_mode"]
    mp3_bytes = generate(api_key, prompt, duration)
    GENERATED_DIR.mkdir(parents=True, exist_ok=True)

    target: Path
    if playback_mode == "loop":
        wav_target = GENERATED_DIR / f"{sound_id}.wav"
        temporary_wav: Path | None = None
        try:
            with tempfile.NamedTemporaryFile(
                dir=GENERATED_DIR,
                prefix=f".{sound_id}.",
                suffix=".wav",
                delete=False,
            ) as handle:
                temporary_wav = Path(handle.name)
            if convert_to_wav(mp3_bytes, temporary_wav):
                os.replace(temporary_wav, wav_target)
                target = wav_target
            else:
                temporary_wav.unlink(missing_ok=True)
                target = GENERATED_DIR / f"{sound_id}.mp3"
                atomic_write_bytes(target, mp3_bytes)
                print("warning: ffmpeg unavailable; loop saved as mp3", file=sys.stderr)
        except Exception:
            if temporary_wav is not None:
                temporary_wav.unlink(missing_ok=True)
            raise
    else:
        target = GENERATED_DIR / f"{sound_id}.mp3"
        atomic_write_bytes(target, mp3_bytes)

    # Successful regeneration replaces the prior representation only after the
    # new file is safely in place.
    for suffix in (".mp3", ".wav", ".ogg"):
        alternate = GENERATED_DIR / f"{sound_id}{suffix}"
        if alternate != target:
            alternate.unlink(missing_ok=True)
    return target


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sound-id", required=True, help="id from more_sounds.json")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    # Real environment variables win. Keep compatibility with the existing
    # generator's repo-root .env while also honoring the project's documented
    # prompt_playgound/.env secret location.
    load_dotenv(REPO_ROOT / ".env", override=False)
    load_dotenv(REPO_ROOT / "prompt_playgound" / ".env", override=False)
    api_key = os.environ.get("ELEVENLABS_API_KEY", "").strip()
    if not api_key:
        print(
            "ELEVENLABS_API_KEY is not set (environment, repo-root .env, or prompt_playgound/.env)",
            file=sys.stderr,
        )
        return 2

    try:
        sound = load_sound(args.sound_id)
        target = generate_one(sound, api_key)
    except Exception as error:
        print(f"{type(error).__name__}: {error}", file=sys.stderr)
        return 1

    print(
        json.dumps(
            {
                "sound_id": sound["id"],
                "path": str(target.relative_to(REPO_ROOT)),
                "playback_mode": sound["playback_mode"],
                "duration_seconds": sound["suggested_duration_seconds"],
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
