#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["fastapi", "uvicorn[standard]", "openai", "pillow"]
# ///
"""Backlog board for features/ — two lanes (backlog, finished) + a reader pane.

Run:   uv run features/ui/server.py [--port 8123]
Open:  http://localhost:8123/

Edits to this file hot-reload the server (uvicorn --reload).

Card thumbnails are generated lazily by gpt-image-2 while the board is being
polled — never at startup, at most THUMB_WORKERS at a time — and stored
content-addressed in images/ next to a manifest.json. A feature whose text
changed keeps its old image until the replacement has been generated.
"""
import argparse
import base64
import hashlib
import io
import json
import os
import re
import sys
import threading
import time
from datetime import datetime
from pathlib import Path

from fastapi import FastAPI, HTTPException
from fastapi.responses import FileResponse

UI_DIR = Path(__file__).resolve().parent
FEATURES_DIR = UI_DIR.parent
REPO_DIR = FEATURES_DIR.parent
BACKLOG_EXCLUDE = {"implemented", "ui", "AGENTS.md", "CLAUDE.md", "order.json"}
META_EXCLUDE = {"AGENTS.md", "CLAUDE.md"}

IMAGES_DIR = UI_DIR / "images"
MANIFEST_PATH = IMAGES_DIR / "manifest.json"
IMAGE_MODEL = "gpt-image-2"
IMAGE_SIZE = "1024x1024"
IMAGE_QUALITY = "low"
THUMB_PX = 128
THUMB_WORKERS = 5
FAILURE_COOLDOWN_S = 15 * 60
IMAGE_NAME_RE = re.compile(r"^[0-9a-f]{16}\.png$")


def rel(p: Path) -> str:
    return p.relative_to(REPO_DIR).as_posix()


def load_order() -> tuple[list, list]:
    try:
        data = json.loads((FEATURES_DIR / "order.json").read_text())
    except (OSError, json.JSONDecodeError):
        data = {}

    def entries(field):
        out = []
        for e in data.get(field, []):
            path = (e.get("path") or "").rstrip("/")
            if path:
                out.append((path, e.get("when")))
        return out

    return entries("order"), entries("finished")


def scan(directory: Path, exclude: set[str]) -> list[dict]:
    items = []
    if not directory.is_dir():
        return items
    for p in sorted(directory.iterdir()):
        if p.name in exclude or p.name.startswith("."):
            continue
        if p.is_dir():
            items.append({"name": p.name, "path": rel(p), "kind": "folder"})
        elif p.suffix == ".md":
            items.append({"name": p.stem, "path": rel(p), "kind": "file"})
    return items


def ordered(items: list[dict], order_entries: list) -> list[dict]:
    pos = {path: i for i, (path, _) in enumerate(order_entries)}
    when = {path: w for path, w in order_entries if w}
    for it in items:
        it["when"] = when.get(it["path"])
    return sorted(items, key=lambda it: (pos.get(it["path"], len(pos)), it["name"].lower()))


def md_index() -> list[str]:
    files = []
    for f in FEATURES_DIR.rglob("*.md"):
        inner = f.relative_to(FEATURES_DIR).parts
        if inner[0] == "ui" or any(part.startswith(".") for part in inner) or f.name in META_EXCLUDE:
            continue
        files.append(rel(f))
    return sorted(files)


# ---------------------------------------------------------------- thumbnails

_lock = threading.Lock()          # guards manifest writes, _in_flight
_in_flight: set[str] = set()      # feature paths currently generating
_retry_at: dict[str, float] = {}  # feature path -> monotonic time before which we won't retry
_hash_cache: dict[str, tuple[tuple, str]] = {}  # feature path -> (stat key, hash)
_api_key: str | None = None
_key_missing_logged = False


def _feature_files(p: Path) -> list[Path]:
    if p.is_dir():
        return sorted(
            f for f in p.rglob("*.md")
            if f.name not in META_EXCLUDE and not any(part.startswith(".") for part in f.relative_to(p).parts)
        )
    return [p]


def content_hash(p: Path) -> str | None:
    """16-hex digest of a feature's text; a byte-identical re-save changes nothing."""
    try:
        files = _feature_files(p)
        if not files:
            return None
        stat_key = tuple((rel(f), f.stat().st_mtime_ns, f.stat().st_size) for f in files)
    except OSError:
        return None
    cached = _hash_cache.get(str(p))
    if cached and cached[0] == stat_key:
        return cached[1]
    digest = hashlib.sha256()
    try:
        for f in files:
            digest.update(rel(f).encode())
            digest.update(b"\0")
            digest.update(f.read_bytes())
            digest.update(b"\0")
    except OSError:
        return None
    hash16 = digest.hexdigest()[:16]
    _hash_cache[str(p)] = (stat_key, hash16)
    return hash16


def _load_manifest() -> dict:
    try:
        return json.loads(MANIFEST_PATH.read_text())
    except (OSError, json.JSONDecodeError):
        return {}


def _save_manifest(manifest: dict) -> None:
    IMAGES_DIR.mkdir(exist_ok=True)
    tmp = MANIFEST_PATH.with_name(".manifest.json.tmp")
    tmp.write_text(json.dumps(manifest, indent=1, sort_keys=True))
    tmp.replace(MANIFEST_PATH)


def _get_api_key() -> str | None:
    global _api_key, _key_missing_logged
    if _api_key:
        return _api_key
    key = os.environ.get("OPENAI_API_KEY", "").strip()
    if not key:
        for envfile in (REPO_DIR / ".env", REPO_DIR / "prompt_playgound" / ".env"):
            try:
                lines = envfile.read_text().splitlines()
            except OSError:
                continue
            for line in lines:
                if line.strip().startswith("OPENAI_API_KEY="):
                    key = line.split("=", 1)[1].strip().strip('"').strip("'")
                    break
            if key:
                break
    if not key:
        if not _key_missing_logged:
            print("[thumbs] OPENAI_API_KEY not set (env or .env) — thumbnails disabled", file=sys.stderr, flush=True)
            _key_missing_logged = True
        return None
    _api_key = key
    return key


def _excerpt(p: Path) -> str:
    if p.is_dir():
        by_name = {f.name.lower(): f for f in p.glob("*.md")}
        files = _feature_files(p)
        src = by_name.get("index.md") or by_name.get("readme.md") or (files[0] if files else None)
    else:
        src = p
    if src is None:
        return ""
    try:
        return src.read_text(errors="replace")[:900]
    except OSError:
        return ""


def _build_prompt(path: str, name: str, safe: bool = False) -> str:
    pretty = name.replace("_", " ").replace("-", " ").strip()
    base = (
        f'A small square illuminated-manuscript miniature icon representing the game feature "{pretty}" '
        "for a simulation game set in a dense late-medieval cathedral city. "
        "Style: medieval illuminated manuscript miniature, rich gouache colors with gold leaf accents, "
        "one bold simple central motif that stays readable at thumbnail size, flat decorative background, "
        "aged parchment tones. No text, no lettering, no words, no border, no frame."
    )
    if safe:
        # fallback when the spec text trips the provider's moderation
        return base + "\n\nDepict the concept indirectly as a tasteful, symbolic, family-friendly emblem."
    return base + f"\n\nThe feature the icon must represent:\n{_excerpt(REPO_DIR / path)}"


def _call_gpt_image(prompt: str) -> bytes:
    from openai import OpenAI

    client = OpenAI(api_key=_get_api_key(), timeout=600.0, max_retries=2)
    response = client.images.generate(
        model=IMAGE_MODEL, prompt=prompt, n=1, size=IMAGE_SIZE, quality=IMAGE_QUALITY, output_format="png"
    )
    if not response.data or not response.data[0].b64_json:
        raise RuntimeError("image API response did not contain base64 image data")
    png = base64.b64decode(response.data[0].b64_json, validate=True)
    if not png.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError("image API response was not a PNG")
    return png


def _downscale(png: bytes) -> bytes:
    from PIL import Image

    img = Image.open(io.BytesIO(png)).convert("RGB")
    img = img.resize((THUMB_PX, THUMB_PX), Image.LANCZOS)
    out = io.BytesIO()
    img.save(out, format="PNG", optimize=True)
    return out.getvalue()


def _set_manifest_entry(path: str, hash16: str) -> None:
    """Point path at <hash16>.png; drop the old file if nothing else uses it. Lock held."""
    manifest = _load_manifest()
    old = manifest.get(path, {}).get("file")
    manifest[path] = {"hash": hash16, "file": f"{hash16}.png", "when": datetime.now().isoformat(timespec="seconds")}
    _save_manifest(manifest)
    if old and old != f"{hash16}.png" and not any(e.get("file") == old for e in manifest.values()):
        (IMAGES_DIR / old).unlink(missing_ok=True)


def _generate_thumbnail(path: str, name: str, hash16: str) -> None:
    try:
        try:
            png = _call_gpt_image(_build_prompt(path, name))
        except Exception as err:
            if "moderation_blocked" not in str(err):
                raise
            print(f"[thumbs] moderation-blocked, retrying name-only: {path}", file=sys.stderr, flush=True)
            png = _call_gpt_image(_build_prompt(path, name, safe=True))
        thumb = _downscale(png)
        IMAGES_DIR.mkdir(exist_ok=True)
        target = IMAGES_DIR / f"{hash16}.png"
        tmp = target.with_name(f".{target.name}.tmp")
        tmp.write_bytes(thumb)
        tmp.replace(target)
        with _lock:
            _set_manifest_entry(path, hash16)
        print(f"[thumbs] generated {path} -> {target.name}", file=sys.stderr, flush=True)
    except Exception as err:
        _retry_at[path] = time.monotonic() + FAILURE_COOLDOWN_S
        print(f"[thumbs] failed {path}: {err}", file=sys.stderr, flush=True)
    finally:
        with _lock:
            _in_flight.discard(path)


def _kick_generator(items: list[dict]) -> None:
    """Top the pool up to THUMB_WORKERS one-shot threads. Never blocks the poll."""
    if _get_api_key() is None:
        return
    now = time.monotonic()
    with _lock:
        free = THUMB_WORKERS - len(_in_flight)
        if free <= 0:
            return
        manifest = _load_manifest()
        for it in items:
            if free <= 0:
                break
            path = it["path"]
            if path in _in_flight or _retry_at.get(path, 0.0) > now:
                continue
            cur = content_hash(REPO_DIR / path)
            if cur is None:
                continue
            entry = manifest.get(path)
            if entry and entry.get("hash") == cur and (IMAGES_DIR / entry.get("file", "")).is_file():
                continue
            if (IMAGES_DIR / f"{cur}.png").is_file():
                # identical content already has an image (e.g. the feature moved lanes)
                _set_manifest_entry(path, cur)
                manifest = _load_manifest()
                continue
            _in_flight.add(path)
            threading.Thread(target=_generate_thumbnail, args=(path, it["name"], cur), daemon=True).start()
            free -= 1


# ------------------------------------------------------------------- listing

def feature_lists() -> dict:
    order, finished = load_order()
    backlog = ordered(scan(FEATURES_DIR, BACKLOG_EXCLUDE), order)
    done = ordered(scan(FEATURES_DIR / "implemented", META_EXCLUDE), finished)
    manifest = _load_manifest()
    for it in backlog + done:
        entry = manifest.get(it["path"])
        has_file = entry and (IMAGES_DIR / entry.get("file", "")).is_file()
        it["image"] = f"/images/{entry['file']}" if has_file else None
    _kick_generator(backlog + done)
    return {
        "backlog": backlog,
        "finished": done,
        "md_files": md_index(),
    }


def feature_detail(path_str: str) -> dict | None:
    if not path_str:
        return None
    p = (REPO_DIR / path_str).resolve()
    if not p.is_relative_to(FEATURES_DIR) or p == FEATURES_DIR:
        return None
    inner = p.relative_to(FEATURES_DIR).parts
    if inner[0] == "ui" or any(part.startswith(".") for part in inner):
        return None
    if p.is_dir():
        files = sorted(p.rglob("*.md"), key=lambda f: (f.name.lower() != "index.md", f.name != "README.md", rel(f)))
        files = [f for f in files if f.name not in META_EXCLUDE]
        return {
            "kind": "folder",
            "name": p.name,
            "path": rel(p),
            "files": [{"name": f.relative_to(p).as_posix(), "path": rel(f)} for f in files],
        }
    if p.is_file() and p.suffix == ".md":
        return {"kind": "file", "name": p.stem, "path": rel(p), "content": p.read_text(errors="replace")}
    return None


app = FastAPI(title="features/ board")


@app.middleware("http")
async def cache_headers(request, call_next):
    response = await call_next(request)
    if request.url.path.startswith("/images/"):
        # content-addressed: a changed image is a different URL
        response.headers["Cache-Control"] = "public, max-age=31536000, immutable"
    else:
        response.headers["Cache-Control"] = "no-store"
    return response


@app.get("/")
def index():
    return FileResponse(UI_DIR / "index.html")


@app.get("/marked.min.js")
def marked_js():
    return FileResponse(UI_DIR / "marked.min.js")


@app.get("/images/{name}")
def image_file(name: str):
    if not IMAGE_NAME_RE.fullmatch(name) or not (IMAGES_DIR / name).is_file():
        raise HTTPException(status_code=404, detail="no such image")
    return FileResponse(IMAGES_DIR / name)


@app.get("/api/features")
def api_features():
    return feature_lists()


@app.get("/api/feature")
def api_feature(path: str = ""):
    detail = feature_detail(path)
    if detail is None:
        raise HTTPException(status_code=404, detail="no such feature")
    return detail


def main():
    import uvicorn

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8123)
    parser.add_argument("--host", default="127.0.0.1")
    args = parser.parse_args()
    uvicorn.run(
        "server:app",
        host=args.host,
        port=args.port,
        reload=True,
        app_dir=str(UI_DIR),
        reload_dirs=[str(UI_DIR)],
        log_level="warning",
    )


if __name__ == "__main__":
    main()
