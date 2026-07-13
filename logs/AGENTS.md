# Session log format

One directory per game run; `latest_session` is a symlink to the newest one.

```
logs/
    latest_session -> session_46_2026-07-13_10_28_52
    session_<n>_<YYYY-MM-DD_HH_MM_SS>/   # run counter, local start time
        logs.jsonl
        screenshots/
        prompts/
```

## logs.jsonl

One JSON object per line, chronological. Fields:

- `ts` — local time, `YYYY-MM-DD HH:MM:SS.mmm`
- `ts_ms` — Unix epoch milliseconds
- `source` — who produced the record:
  - `"session"` — the single session-start marker
  - `"rust"` — the game's log stream (same events as the console output)
  - `"engine"` — the in-process actor engine's diagnostics (the `[smart actors]
    …` lines) and its failures
  - `"stt"` / `"tts"` — a local speech worker's stderr (Canary-Qwen, Pocket
    TTS), one record per line; only present when a local backend is in use
  - `"drive"` — `CATHEDRAL_DRIVE` script evidence lines, e.g. `[drive] 1.0s key F5`
- `level` — `INFO` / `WARN` / `ERROR` (non-rust sources are `INFO` unless failing)
- `message` — the log text
- `target` — rust only: module path of the log site
- `fields` — rust only, optional: structured key/values attached to the event

## screenshots/

- `cathedral_screenshot_<YYYY-MM-DD_HH_MM_SS>__<nn>.png` — F5 captures. `nn`
  starts at `00` each second and increments when several captures land in the
  same second.
- `<name>.png` — named captures from drive-mode `shot <name>` actions.

## prompts/

Every LLM exchange (one NPC turn), including failed provider calls, as a pair
sharing one base name:

```
<YYYY-MM-DD_HH_MM_SS>__<nn>__<actor id>__<actor name>_prompt.md
<YYYY-MM-DD_HH_MM_SS>__<nn>__<actor id>__<actor name>_prompt.json
```

The timestamp is when the answer arrived; `nn` disambiguates within a second
as above. The `.md` is for human (or agent) reading, with `# Prompt`, `# Answer`, and `# Meta`
sections (`# Answer` holds `*(no answer)*` on failure). The `.json` carries
the same data for tooling (or agents if you're doing analysis etc):

```json
{"prompt": "...", "answer": "... or null", "meta": {...}}
```

`meta` keys: `actor_id`, `actor_name`, `model` (`"fake"` in offline fake
mode), `duration_seconds` (provider call time), `timestamp` (ISO 8601, local),
and `error` (only present when the call failed; `answer` is then `null`).
