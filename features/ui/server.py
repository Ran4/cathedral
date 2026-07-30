#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["fastapi", "uvicorn[standard]"]
# ///
"""Backlog board for features/ — two lanes (backlog, finished) + a reader pane.

Run:   uv run features/ui/server.py [--port 8123]
Open:  http://localhost:8123/

Edits to this file hot-reload the server (uvicorn --reload).
"""
import argparse
import json
from pathlib import Path

from fastapi import FastAPI, HTTPException
from fastapi.responses import FileResponse

UI_DIR = Path(__file__).resolve().parent
FEATURES_DIR = UI_DIR.parent
REPO_DIR = FEATURES_DIR.parent
BACKLOG_EXCLUDE = {"implemented", "ui", "AGENTS.md", "CLAUDE.md", "order.json"}
META_EXCLUDE = {"AGENTS.md", "CLAUDE.md"}


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


def feature_lists() -> dict:
    order, finished = load_order()
    return {
        "backlog": ordered(scan(FEATURES_DIR, BACKLOG_EXCLUDE), order),
        "finished": ordered(scan(FEATURES_DIR / "implemented", META_EXCLUDE), finished),
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
async def no_store(request, call_next):
    response = await call_next(request)
    response.headers["Cache-Control"] = "no-store"
    return response


@app.get("/")
def index():
    return FileResponse(UI_DIR / "index.html")


@app.get("/marked.min.js")
def marked_js():
    return FileResponse(UI_DIR / "marked.min.js")


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
