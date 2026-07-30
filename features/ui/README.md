# features/ board

A tiny local web UI over the backlog: two lanes (backlog left, finished right,
each sorted by `features/order.json`) and a reader pane that renders the
clicked feature's markdown. Folder features get a chip per `.md` inside.

```sh
uv run features/ui/server.py          # http://127.0.0.1:8123/
uv run features/ui/server.py --port 9000
```

Single-file FastAPI backend (deps pulled by `uv` from the inline script
header; `marked.min.js` is vendored) with hot reload — edits to `server.py`
restart the server automatically. The page polls the backend every 3 s while
the window is focused, every 30 s otherwise.

This directory is excluded from the backlog scan, so the board never lists
itself.
