# Coalesce repeated entries in recent_history

`Character.recent_history` (prompt_playgound/sim.py) now holds sound percepts
as well as speech, and the window is only `RECENT_HISTORY_MAX_ENTRIES` = 16
entries. Consecutive identical percepts can therefore evict real dialogue: a
player fart barrage at the 2 s rate limit flushes the whole window in ~30 s,
and nothing rate-limits world sounds like the town bell at all.

Deferred fix: coalesce **consecutive** duplicates into one entry with a count,
e.g. `[You heard a big fart!] (3 times now)`. That protects the buffer and
delivers the escalation signal for free — the third fart genuinely reads as a
third fart instead of three identical lines.

Implementation notes for later:

- Coalesce in `_remember_percept` when the new line equals the last entry
  (modulo an existing ` (N times now)` suffix); everything downstream already
  treats the window as opaque strings.
- Only *consecutive* duplicates: `fart, "hello", fart` must stay three entries
  — the interleaving is itself information.
- Decide whether the same coalescing applies to the inbox delta
  (`since_your_last_turn`). Probably not: the delta is drained per turn and
  its rate limits already bound it, and per-event granularity feeds the
  scheduler nudge.
