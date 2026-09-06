# Known-place estimates: verification, 2026-09-06

The source is Colm Threefinger's 11:53:10 turn in session 799. The original
prompt is preserved in `colm_original_prompt.txt`, so reproduction does not
depend on retaining the session's logs.

`colm_replay_prompt.txt` uses the current turn instructions, including the
footer, and replaces only the original sheet's known-place section. The rest
of the archived sheet is unchanged: rough speech transcription, Hamel's later
interjection, the daily round, memories, and recent history all remain.
Estimates use the archived, rounded position `(323, 117)` and the shipped graph.
The 1,000 generated citizens are seeded together to reproduce home assignment.

## Live replies

Provider/model: `openai`, `gpt-5.6-luna` (the recorded model). Each reply below
was requested through the existing `cathedral-headless --one-shot` command.
These are samples of model behavior, not a guarantee of every future reply.

| Case | Result | Evidence |
| --- | --- | --- |
| Original follow-up, run 1 | Harne Gate, 1,330 m, ten and a half minutes | [Reply](colm_replay_1_answer.txt) |
| Original follow-up, run 2 | Harne Gate, 1,330 m, ten and a half minutes | [Reply](colm_replay_2_answer.txt) |
| Original follow-up, run 3 | Harne Gate, 1,330 m, ten and a half minutes | [Reply](colm_replay_3_answer.txt) |
| Direct workplace distance question | Harne Gate, 1,330 m | [Prompt](colm_distance_prompt.txt), [reply](colm_distance_answer.txt) |
| Direct workplace walking-time question | Harne Gate, ten and a half minutes | [Prompt](colm_walking_time_prompt.txt), [reply](colm_walking_time_answer.txt) |
| Explicit switch to Coswald's Yard | About seven minutes, Coswald's Yard | [Prompt](colm_explicit_switch_prompt.txt), [reply](colm_explicit_switch_answer.txt) |
| Unknown Glass Orchard | Does not know it or its route; no invented estimate | [Prompt](colm_unknown_prompt.txt), [reply](colm_unknown_answer.txt) |

Earlier prompt variants sometimes answered the old naming question, switched
to the bystander's destination, or asked for a place already established. The
final instructions explicitly resolve the question from the exchange before
it, prioritize the latest unanswered question, and repeat the essential rule
after the sheet. No speaker routing or conversation-state changes were needed.

## Cost

Recorded in [metrics.json](metrics.json), on a world containing 1,520 characters
and 3,972 navigation nodes, with Colm holding 20 place handles:

- First full prompt with cold destination caches: **3.68 ms**.
- Warm full-prompt rendering, 300 samples: **25.9 µs median**, **28.7 µs p95**.
- Isolated warm distance-table lookup, one million varying origins: **2.16 ns
  average**. This excludes nearest-node lookup, endpoint offsets and rendering.
- Known-place section: **528 → 1,157 UTF-8 bytes**. Whole replay prompt,
  including the additional instructions: **16,861 → 19,183 bytes**. These are
  byte counts, not token counts.
- Each populated destination row stores 3,972 four-byte distances, about
  **15.5 KiB**. Colm's first 20 destinations need about **310 KiB** of row data,
  plus shared row metadata. Full coverage of the 78 named/ward destinations
  and 1,101 doors is at most **17.86 MiB**; even querying every possible node
  as a destination caps row data at **60.18 MiB**. Duplicate nodes share rows.

Distances describe the existing simulation routes. This change does not
improve the graph's topology: its routes can contain substantial detours.
Endpoint offsets, avoidance and individual walking lanes also make the spoken
values estimates. The accelerated day/night clock does not scale them.

## Automated checks

- `cargo test -p cathedral-sim`: **1,025 passed**, one fixture-regeneration
  test intentionally ignored during the normal run.
- `cargo test -p cathedral-backends --lib`: **148 passed**.
- `cargo check -p cathedralbevy` and `cargo build -p cathedralbevy`: passed,
  with the existing unused
  `Weather`/`Hud`/`Interaction` probe warning in `src/perf.rs`.
- Golden fixtures regenerated through the documented ignored test and their
  diff reviewed. New Rust files pass rustfmt; `git diff --check` passes.
- Cache tests compare cached values against real routes, check shared table
  reuse, and ensure a separately loaded graph never reads stale values.
- Prompt tests cover detours, movement, short walks, arrival, home offsets,
  blocked/disconnected destinations, missing navigation, learning a place,
  and keeping the same handles usable by `go_to`.

To reconstruct prompts and repeat the offline measurement from the repo root:

```sh
cargo test -p cathedral-backends --test place_estimates_evidence -- --ignored --nocapture
```

The output defaults to `/tmp/cathedral-wayfinding`; set
`CATHEDRAL_WAYFINDING_EVIDENCE_DIR` to choose another directory. For a live reply:

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- \
  --provider openai --model gpt-5.6-luna \
  --one-shot /tmp/cathedral-wayfinding/colm_replay_prompt.txt
```

The offline preparation makes no provider calls. The second command uses the
configured provider credentials. Neither command opens a game window or plays
speech. Tests and replaying did not replace `logs/latest_session`.
