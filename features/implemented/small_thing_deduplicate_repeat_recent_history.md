# Coalesce repeated entries in recent_history

`CharacterState.recent_history` (crates/cathedral-sim/src/character.rs) holds sound percepts
as well as speech, and the window is only `RECENT_HISTORY_MAX_ENTRIES` = 32
entries. Consecutive identical percepts could therefore evict real dialogue: a
player fart barrage at the 2 s rate limit flushes the whole window, and nothing
rate-limits world sounds like the town bell at all.

Fix: coalesce **consecutive** duplicates into one entry with a count,
e.g. `[You heard a big fart!] (3 times now)`. That protects the buffer and
delivers the escalation signal for free — the third fart genuinely reads as a
third fart instead of three identical lines.

## As implemented

- `Character::remember_percept` coalesces when the new line equals the last
  entry modulo an existing ` (N times now)` suffix (parsed by
  `split_repeat_count`); everything downstream already treats the window as
  opaque strings, so nothing else changed.
- Only *consecutive* duplicates: `fart, "hello", fart` stays three entries —
  the interleaving is itself information.
- The inbox delta (`since_your_last_turn`) is intentionally **not** coalesced:
  it is drained per turn, its rate limits already bound it, and per-event
  granularity feeds the scheduler nudge. Duplicates collapse when the
  presented lines graduate into `recent_history`
  (`absorb_presented_history` → `remember_percept`), so a run continues its
  count across turns.
- Tests: `consecutive_duplicate_percepts_coalesce_with_a_count`,
  `graduated_percepts_coalesce_but_the_inbox_delta_does_not`, and
  `split_repeat_count_only_matches_the_counted_suffix` in `character.rs`.
