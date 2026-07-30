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

## Card thumbnails

Each card shows a small gpt-image-2 illustration (illuminated-miniature
style), generated **lazily while the board is polled** — a backend that is
never opened generates nothing. At most 5 generate in parallel; a poll never
waits on generation. Thumbnails live in `images/` as content-addressed
`<hash>.png` files (128×128, downscaled from a 1024×1024 low-quality render,
~$0.01 each) beside a `manifest.json` mapping feature → current image; both
are committed to git. When a feature's *text* changes (byte hash — a re-save
with identical bytes does nothing) the old image keeps being served until the
replacement lands, then the orphan is deleted. Needs `OPENAI_API_KEY` (env or
repo-root `.env`); without it the board simply shows letter placeholders.
Failed generations back off for 15 minutes.

This directory is excluded from the backlog scan, so the board never lists
itself.
