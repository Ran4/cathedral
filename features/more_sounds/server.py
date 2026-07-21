#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["requests", "python-dotenv"]
# ///
"""Serve the More Sounds workbench and persist its choices directly to JSON.

Run from anywhere in the repository:

    uv run --script features/more_sounds/server.py --open

The server binds to loopback by default. It owns the shortlist JSON, invokes
``generate_sound.py`` for one ElevenLabs sound at a time, and serves generated
audio with byte-range support so browser audio controls can seek normally.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import threading
import webbrowser
from dataclasses import dataclass
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import quote, unquote, urlsplit


HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
CATALOG_PATH = HERE / "more_sounds.json"
SELECTION_PATH = HERE / "sounds_to_implement.json"
SHOWCASE_PATH = HERE / "showcase_more_sounds.html"
GENERATED_DIR = HERE / "generated"
GENERATOR_SCRIPT = HERE / "generate_sound.py"
SOUND_ID_RE = re.compile(r"[a-z0-9_]+\Z")
AUDIO_SUFFIXES = {".mp3": "audio/mpeg", ".wav": "audio/wav", ".ogg": "audio/ogg"}
MAX_JSON_BODY = 64 * 1024


class WorkspaceError(RuntimeError):
    """A safe error that may be returned to the local frontend."""


class NotFound(WorkspaceError):
    pass


class AlreadyGenerating(WorkspaceError):
    pass


def read_json(path: Path, default: Any) -> Any:
    if not path.exists():
        return default
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise WorkspaceError(f"Could not read {path.name}: {error}") from error


def atomic_write_json(path: Path, value: Any) -> None:
    """Replace a JSON file atomically so an interrupted write cannot truncate it."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            temporary = Path(handle.name)
            json.dump(value, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except OSError as error:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        raise WorkspaceError(f"Could not write {path.name}: {error}") from error


def env_file_has_key(path: Path, key: str) -> bool:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return False
    for raw_line in lines:
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        name, value = line.removeprefix("export ").split("=", 1)
        if name.strip() == key and value.strip().strip("'\""):
            return True
    return False


@dataclass(frozen=True)
class GeneratedAudio:
    sound_id: str
    filename: str
    url: str
    media_type: str
    size_bytes: int
    modified_ns: int

    def as_json(self) -> dict[str, Any]:
        return {
            "sound_id": self.sound_id,
            "filename": self.filename,
            "url": self.url,
            "media_type": self.media_type,
            "size_bytes": self.size_bytes,
            "modified_ns": self.modified_ns,
        }


class SoundWorkspace:
    def __init__(self, root: Path = HERE) -> None:
        self.root = root.resolve()
        self.catalog_path = self.root / "more_sounds.json"
        self.selection_path = self.root / "sounds_to_implement.json"
        self.generated_dir = self.root / "generated"
        self.generator_script = self.root / "generate_sound.py"
        self.repo_root = self.root.parents[1]
        self._lock = threading.RLock()
        self._generating: set[str] = set()

    def _catalog_unlocked(self) -> dict[str, Any]:
        catalog = read_json(self.catalog_path, {})
        if not isinstance(catalog, dict) or not isinstance(catalog.get("sounds"), list):
            raise WorkspaceError("more_sounds.json does not contain a sounds array")

        changed = False
        seen: set[str] = set()
        for sound in catalog["sounds"]:
            if not isinstance(sound, dict):
                raise WorkspaceError("more_sounds.json contains a non-object sound")
            sound_id = sound.get("id")
            if not isinstance(sound_id, str) or not SOUND_ID_RE.fullmatch(sound_id):
                raise WorkspaceError(f"Invalid sound id in more_sounds.json: {sound_id!r}")
            if sound_id in seen:
                raise WorkspaceError(f"Duplicate sound id in more_sounds.json: {sound_id}")
            seen.add(sound_id)
            if not isinstance(sound.get("implemented_in_game"), bool):
                sound["implemented_in_game"] = False
                changed = True
        if changed:
            atomic_write_json(self.catalog_path, catalog)
        return catalog

    @staticmethod
    def _sound_index(catalog: dict[str, Any]) -> dict[str, dict[str, Any]]:
        return {sound["id"]: sound for sound in catalog["sounds"]}

    def _reconcile_unlocked(
        self,
        catalog: dict[str, Any],
        raw_selection: Any,
    ) -> tuple[dict[str, Any], bool]:
        """Normalize the shortlist while preserving future LLM-added metadata.

        The implemented flag is monotonic when files are edited by hand: a true
        value in either JSON propagates to the other. The explicit backend
        toggle updates both sides and can therefore still set it false.
        """
        index = self._sound_index(catalog)
        raw = raw_selection if isinstance(raw_selection, dict) else {}
        raw_sounds = raw.get("sounds") if isinstance(raw.get("sounds"), list) else []
        raw_by_id = {
            row.get("id"): row
            for row in raw_sounds
            if isinstance(row, dict) and isinstance(row.get("id"), str)
        }

        ids: list[str] = []
        candidates = raw.get("selected_sound_ids")
        if isinstance(candidates, list):
            for sound_id in candidates:
                if isinstance(sound_id, str) and sound_id in index and sound_id not in ids:
                    ids.append(sound_id)
        for sound_id in raw_by_id:
            if sound_id in index and sound_id not in ids:
                ids.append(sound_id)

        catalog_changed = False
        selected_sounds: list[dict[str, Any]] = []
        for sound_id in ids:
            canonical = index[sound_id]
            prior = raw_by_id.get(sound_id, {})
            implemented = bool(canonical.get("implemented_in_game")) or bool(
                prior.get("implemented_in_game")
            )
            if canonical.get("implemented_in_game") != implemented:
                canonical["implemented_in_game"] = implemented
                catalog_changed = True

            # Preserve fields a future implementation pass may add (notes,
            # code locations, commit ids), while catalog design fields remain
            # authoritative.
            merged = dict(canonical)
            for key, value in prior.items():
                if key not in canonical:
                    merged[key] = value
            merged["implemented_in_game"] = implemented
            selected_sounds.append(merged)

        normalized = dict(raw)
        normalized.update(
            {
                "schema_version": 1,
                "source_catalog": "more_sounds.json",
                "selected_count": len(ids),
                "implemented_count": sum(
                    1 for sound in selected_sounds if sound["implemented_in_game"]
                ),
                "selected_sound_ids": ids,
                "sounds": selected_sounds,
            }
        )
        return normalized, catalog_changed

    def _sync_generated_metadata_unlocked(
        self,
        catalog: dict[str, Any],
        selection: dict[str, Any],
    ) -> None:
        generated = self._scan_generated_unlocked(set(self._sound_index(catalog)))
        generated_count = 0
        for row in selection["sounds"]:
            audio = generated.get(row["id"])
            if audio is None:
                row.pop("generated_audio", None)
                continue
            metadata = audio.as_json()
            metadata["path"] = f"generated/{audio.filename}"
            row["generated_audio"] = metadata
            generated_count += 1
        selection["generated_count"] = generated_count

    def _load_unlocked(self) -> tuple[dict[str, Any], dict[str, Any]]:
        catalog = self._catalog_unlocked()
        raw_selection = read_json(self.selection_path, {})
        selection, catalog_changed = self._reconcile_unlocked(catalog, raw_selection)
        self._sync_generated_metadata_unlocked(catalog, selection)
        if catalog_changed:
            atomic_write_json(self.catalog_path, catalog)
        if selection != raw_selection:
            atomic_write_json(self.selection_path, selection)
        return catalog, selection

    def state(self) -> dict[str, Any]:
        with self._lock:
            catalog, selection = self._load_unlocked()
            ids = set(self._sound_index(catalog))
            generated = self._scan_generated_unlocked(ids)
            generating = sorted(self._generating)
        return {
            "catalog": catalog,
            "selection": selection,
            "generated": {sound_id: audio.as_json() for sound_id, audio in generated.items()},
            "generating_sound_ids": generating,
            "generator": {
                "provider": "ElevenLabs Sound Effects API",
                "available": self.generator_available(),
            },
        }

    def catalog(self) -> dict[str, Any]:
        with self._lock:
            catalog, _ = self._load_unlocked()
            return catalog

    def selection(self) -> dict[str, Any]:
        with self._lock:
            _, selection = self._load_unlocked()
            return selection

    def set_selected(self, sound_id: str, selected: bool) -> dict[str, Any]:
        with self._lock:
            catalog, selection = self._load_unlocked()
            index = self._sound_index(catalog)
            if sound_id not in index:
                raise NotFound(f"Unknown sound id: {sound_id}")
            ids = list(selection["selected_sound_ids"])
            if selected and sound_id not in ids:
                ids.append(sound_id)
            if not selected:
                ids = [candidate for candidate in ids if candidate != sound_id]
            selection["selected_sound_ids"] = ids
            selection["sounds"] = [row for row in selection["sounds"] if row["id"] in ids]
            normalized, catalog_changed = self._reconcile_unlocked(catalog, selection)
            self._sync_generated_metadata_unlocked(catalog, normalized)
            if catalog_changed:
                atomic_write_json(self.catalog_path, catalog)
            atomic_write_json(self.selection_path, normalized)
            return normalized

    def clear_selection(self) -> dict[str, Any]:
        with self._lock:
            catalog, selection = self._load_unlocked()
            selection["selected_sound_ids"] = []
            selection["sounds"] = []
            normalized, _ = self._reconcile_unlocked(catalog, selection)
            self._sync_generated_metadata_unlocked(catalog, normalized)
            atomic_write_json(self.selection_path, normalized)
            return normalized

    def set_implemented(self, sound_id: str, implemented: bool) -> tuple[dict[str, Any], dict[str, Any]]:
        with self._lock:
            catalog, selection = self._load_unlocked()
            index = self._sound_index(catalog)
            if sound_id not in index:
                raise NotFound(f"Unknown sound id: {sound_id}")
            index[sound_id]["implemented_in_game"] = implemented
            for row in selection["sounds"]:
                if row["id"] == sound_id:
                    row["implemented_in_game"] = implemented
            normalized, _ = self._reconcile_unlocked(catalog, selection)
            self._sync_generated_metadata_unlocked(catalog, normalized)
            atomic_write_json(self.catalog_path, catalog)
            atomic_write_json(self.selection_path, normalized)
            return index[sound_id], normalized

    def generator_available(self) -> bool:
        if os.environ.get("ELEVENLABS_API_KEY", "").strip():
            return True
        return any(
            env_file_has_key(path, "ELEVENLABS_API_KEY")
            for path in (self.repo_root / ".env", self.repo_root / "prompt_playgound" / ".env")
        )

    def _scan_generated_unlocked(self, valid_ids: set[str]) -> dict[str, GeneratedAudio]:
        generated: dict[str, GeneratedAudio] = {}
        if not self.generated_dir.is_dir():
            return generated
        for path in self.generated_dir.iterdir():
            if not path.is_file() or path.suffix.lower() not in AUDIO_SUFFIXES or path.stem not in valid_ids:
                continue
            stat = path.stat()
            candidate = GeneratedAudio(
                sound_id=path.stem,
                filename=path.name,
                url=f"/generated/{quote(path.name)}?v={stat.st_mtime_ns}",
                media_type=AUDIO_SUFFIXES[path.suffix.lower()],
                size_bytes=stat.st_size,
                modified_ns=stat.st_mtime_ns,
            )
            prior = generated.get(path.stem)
            if prior is None or candidate.modified_ns > prior.modified_ns:
                generated[path.stem] = candidate
        return generated

    def generated_audio(self, sound_id: str) -> GeneratedAudio | None:
        with self._lock:
            catalog, _ = self._load_unlocked()
            return self._scan_generated_unlocked(set(self._sound_index(catalog))).get(sound_id)

    def audio_path(self, filename: str) -> Path:
        if Path(filename).name != filename or Path(filename).suffix.lower() not in AUDIO_SUFFIXES:
            raise NotFound("Unknown generated audio file")
        with self._lock:
            catalog, _ = self._load_unlocked()
            if Path(filename).stem not in self._sound_index(catalog):
                raise NotFound("Unknown generated audio file")
        path = self.generated_dir / filename
        if not path.is_file():
            raise NotFound("Generated audio file does not exist")
        return path

    def generate(self, sound_id: str) -> GeneratedAudio:
        with self._lock:
            catalog, _ = self._load_unlocked()
            if sound_id not in self._sound_index(catalog):
                raise NotFound(f"Unknown sound id: {sound_id}")
            if sound_id in self._generating:
                raise AlreadyGenerating(f"{sound_id} is already being generated")
            self._generating.add(sound_id)

        try:
            if not self.generator_script.is_file():
                raise WorkspaceError(f"Missing generator script: {self.generator_script.name}")
            try:
                result = subprocess.run(
                    [sys.executable, str(self.generator_script), "--sound-id", sound_id],
                    cwd=self.repo_root,
                    text=True,
                    capture_output=True,
                    timeout=180,
                    check=False,
                )
            except subprocess.TimeoutExpired as error:
                raise WorkspaceError("Sound generation timed out after 180 seconds") from error
            except OSError as error:
                raise WorkspaceError(f"Could not start the sound generator: {error}") from error
            if result.returncode != 0:
                detail = (result.stderr or result.stdout or "generator exited without an error message").strip()
                if len(detail) > 1600:
                    detail = detail[-1600:]
                raise WorkspaceError(f"Sound generation failed: {detail}")

            audio = self.generated_audio(sound_id)
            if audio is None:
                raise WorkspaceError("Generator finished but did not create an audio file")
            return audio
        finally:
            with self._lock:
                self._generating.discard(sound_id)


class SoundServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, address: tuple[str, int], workspace: SoundWorkspace) -> None:
        self.workspace = workspace
        super().__init__(address, SoundRequestHandler)


class SoundRequestHandler(BaseHTTPRequestHandler):
    server: SoundServer
    protocol_version = "HTTP/1.1"

    def _send_headers(self, status: int, media_type: str, length: int, **headers: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", media_type)
        self.send_header("Content-Length", str(length))
        self.send_header("X-Content-Type-Options", "nosniff")
        for name, value in headers.items():
            self.send_header(name.replace("_", "-"), value)
        self.end_headers()

    def send_bytes(
        self,
        payload: bytes,
        media_type: str,
        *,
        status: int = HTTPStatus.OK,
        body: bool = True,
        cache_control: str = "no-store",
    ) -> None:
        self._send_headers(status, media_type, len(payload), Cache_Control=cache_control)
        if body:
            self.wfile.write(payload)

    def send_json(self, value: Any, *, status: int = HTTPStatus.OK, body: bool = True) -> None:
        payload = (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
        self.send_bytes(payload, "application/json; charset=utf-8", status=status, body=body)

    def send_error_json(self, status: int, message: str) -> None:
        self.send_json({"ok": False, "error": message}, status=status)

    def read_json_body(self) -> dict[str, Any]:
        if self.headers.get_content_type() != "application/json":
            raise WorkspaceError("POST requests must use Content-Type: application/json")
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError as error:
            raise WorkspaceError("Invalid Content-Length") from error
        if length <= 0 or length > MAX_JSON_BODY:
            raise WorkspaceError(f"JSON body must be between 1 and {MAX_JSON_BODY} bytes")
        try:
            body = json.loads(self.rfile.read(length))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise WorkspaceError(f"Invalid JSON body: {error}") from error
        if not isinstance(body, dict):
            raise WorkspaceError("JSON body must be an object")
        return body

    @staticmethod
    def require_sound_id(body: dict[str, Any]) -> str:
        sound_id = body.get("sound_id")
        if not isinstance(sound_id, str) or not SOUND_ID_RE.fullmatch(sound_id):
            raise WorkspaceError("sound_id must contain only lowercase letters, digits, and underscores")
        return sound_id

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        self._handle_get(body=True)

    def do_HEAD(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        self._handle_get(body=False)

    def _handle_get(self, *, body: bool) -> None:
        path = unquote(urlsplit(self.path).path)
        try:
            if path in {"/", "/showcase_more_sounds.html"}:
                self.send_bytes(
                    SHOWCASE_PATH.read_bytes(),
                    "text/html; charset=utf-8",
                    body=body,
                    cache_control="no-cache",
                )
                return
            if path == "/api/state":
                self.send_json(self.server.workspace.state(), body=body)
                return
            if path == "/api/health":
                self.send_json({"ok": True}, body=body)
                return
            if path == "/more_sounds.json":
                self.send_json(self.server.workspace.catalog(), body=body)
                return
            if path == "/sounds_to_implement.json":
                self.send_json(self.server.workspace.selection(), body=body)
                return
            if path.startswith("/generated/"):
                filename = path.removeprefix("/generated/")
                self.send_audio(self.server.workspace.audio_path(filename), body=body)
                return
            if path == "/favicon.ico":
                self.send_bytes(b"", "image/x-icon", status=HTTPStatus.NO_CONTENT, body=body)
                return
            self.send_error_json(HTTPStatus.NOT_FOUND, "Not found")
        except FileNotFoundError:
            self.send_error_json(HTTPStatus.NOT_FOUND, "Not found")
        except NotFound as error:
            self.send_error_json(HTTPStatus.NOT_FOUND, str(error))
        except WorkspaceError as error:
            self.send_error_json(HTTPStatus.INTERNAL_SERVER_ERROR, str(error))
        except OSError as error:
            self.send_error_json(HTTPStatus.INTERNAL_SERVER_ERROR, f"Could not serve the requested file: {error}")

    def send_audio(self, path: Path, *, body: bool) -> None:
        size = path.stat().st_size
        start, end = 0, max(size - 1, 0)
        status = HTTPStatus.OK
        range_header = self.headers.get("Range")
        if range_header:
            match = re.fullmatch(r"bytes=(\d*)-(\d*)", range_header.strip())
            if not match or not size:
                self.send_error_json(HTTPStatus.REQUESTED_RANGE_NOT_SATISFIABLE, "Invalid byte range")
                return
            first, last = match.groups()
            if first:
                start = int(first)
                end = int(last) if last else size - 1
            elif last:
                count = int(last)
                start = max(size - count, 0)
                end = size - 1
            if start >= size or end < start:
                self.send_error_json(HTTPStatus.REQUESTED_RANGE_NOT_SATISFIABLE, "Invalid byte range")
                return
            end = min(end, size - 1)
            status = HTTPStatus.PARTIAL_CONTENT

        length = end - start + 1 if size else 0
        headers = {"Accept_Ranges": "bytes", "Cache_Control": "public, max-age=31536000, immutable"}
        if status == HTTPStatus.PARTIAL_CONTENT:
            headers["Content_Range"] = f"bytes {start}-{end}/{size}"
        self._send_headers(status, AUDIO_SUFFIXES[path.suffix.lower()], length, **headers)
        if not body or not length:
            return
        with path.open("rb") as handle:
            handle.seek(start)
            remaining = length
            while remaining:
                chunk = handle.read(min(64 * 1024, remaining))
                if not chunk:
                    break
                self.wfile.write(chunk)
                remaining -= len(chunk)

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        path = urlsplit(self.path).path
        try:
            body = self.read_json_body()
            if path == "/api/selection":
                sound_id = self.require_sound_id(body)
                selected = body.get("selected")
                if not isinstance(selected, bool):
                    raise WorkspaceError("selected must be a boolean")
                selection = self.server.workspace.set_selected(sound_id, selected)
                self.send_json({"ok": True, "selection": selection})
                return
            if path == "/api/selection/clear":
                selection = self.server.workspace.clear_selection()
                self.send_json({"ok": True, "selection": selection})
                return
            if path == "/api/implemented":
                sound_id = self.require_sound_id(body)
                implemented = body.get("implemented_in_game")
                if not isinstance(implemented, bool):
                    raise WorkspaceError("implemented_in_game must be a boolean")
                sound, selection = self.server.workspace.set_implemented(sound_id, implemented)
                self.send_json({"ok": True, "sound": sound, "selection": selection})
                return
            if path == "/api/generate":
                sound_id = self.require_sound_id(body)
                generated = self.server.workspace.generate(sound_id)
                self.send_json({"ok": True, "generated": generated.as_json()})
                return
            self.send_error_json(HTTPStatus.NOT_FOUND, "Not found")
        except NotFound as error:
            self.send_error_json(HTTPStatus.NOT_FOUND, str(error))
        except AlreadyGenerating as error:
            self.send_error_json(HTTPStatus.CONFLICT, str(error))
        except WorkspaceError as error:
            self.send_error_json(HTTPStatus.BAD_REQUEST, str(error))
        except (BrokenPipeError, ConnectionResetError):
            pass
        except Exception as error:
            self.log_error("Unhandled backend error: %s", error)
            self.send_error_json(HTTPStatus.INTERNAL_SERVER_ERROR, "Unexpected backend error; see the server terminal")

    def log_message(self, template: str, *args: Any) -> None:
        print(f"[{self.log_date_time_string()}] {self.address_string()} {template % args}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1", help="bind address (default: loopback only)")
    parser.add_argument("--port", type=int, default=8798, help="port (default: 8798; use 0 for any free port)")
    parser.add_argument("--open", action="store_true", help="open the workbench in the default browser")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    workspace = SoundWorkspace()
    # Validate and normalize durable state before announcing the server.
    workspace.state()
    try:
        server = SoundServer((args.host, args.port), workspace)
    except OSError as error:
        print(f"Could not start server on {args.host}:{args.port}: {error}", file=sys.stderr)
        return 2
    host, port = server.server_address[:2]
    display_host = "127.0.0.1" if host in {"0.0.0.0", "::"} else host
    url = f"http://{display_host}:{port}/"
    print(f"More Sounds workbench: {url}", flush=True)
    print(f"Shortlist JSON: {SELECTION_PATH}", flush=True)
    if not workspace.generator_available():
        print("Generation disabled until ELEVENLABS_API_KEY is configured.", flush=True)
    if args.open:
        webbrowser.open(url)
    try:
        server.serve_forever(poll_interval=0.25)
    except KeyboardInterrupt:
        print("\nStopping More Sounds workbench.", flush=True)
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
