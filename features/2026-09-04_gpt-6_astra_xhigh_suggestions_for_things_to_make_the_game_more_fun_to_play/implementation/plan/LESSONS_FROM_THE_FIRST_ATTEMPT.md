# Lessons from the first implementation attempt (removed 2026-10-03)

GPT-6 Astra implemented M1–M6 of this plan between 2026-09-07 and 2026-10-02:
64 commits, ~140k lines of game code and 343 MB of committed "evidence", with no
quest content and save/load never switched on. The developer had all of it
**removed from history** (local and GitHub) on 2026-10-03; `develop` restarts from
the M0 commit. A git bundle of the removed history may still exist at
`/home/ran/cathedralbevy-astra-backup-2026-10-03.bundle` (the developer may delete it).
This note is what independent reviews of that code found, so it doesn't happen again.

## What went wrong

- **A self-invented memory-proof requirement.** `RUNTIME_BUDGETS.md` sets a shared
  1 GiB "resident allocation ceiling" for save/load cohorts, a 4,096-entry replay
  window and ≤2 ms p99 per frame for the whole save/load. None of these came from
  the developer (whose actual decisions are D05 "never pause" and D06 "proper
  whole-world save/load" in `DECISIONS.md`). Treating them as things to *prove*
  led to byte accounting on every message, a `Reservation` threaded through every
  API, and finally forks of `bevy_ecs`, `bevy_render`, `tokio`, `hyper`,
  `hyper-util`, `reqwest` and `serde_json` to count their internal allocations. The
  last days went to GDB sessions attributing a 110-byte discrepancy.
- **Process instead of product.** "Independent gates", "sealed releases", hash-pinned
  source maps and multi-thousand-check audits around every build; a 730-line status
  log in `CLAUDE.md`; evidence copied into git (including test binaries).
- **Systems built ahead of any use.** M1d's operation kernel had zero production
  callers; M5's perception code had no non-test callers; M4/M6 were small refactors.

## Per-milestone findings worth keeping in mind

**M1 (live time / receipts / generations / operations) — ~12k lines, ~300 worth it.**
- "The city must keep running while menus, chat or reading are open" was **already
  true**: nothing paused `Time<Virtual>` before M1. Check the current code before
  building a solution to D05.
- Astra replaced the controller's `Time::<Virtual>::from_max_delta(MAX_FRAME_CATCHUP)`
  hitch clamp (`src/controller.rs`) with unbounded "time debt", so after any stall
  the world fast-forwarded at up to ~6×. Keep the clamp.
- Genuinely good: bell/office consumers remembering their position as a calendar
  day, not a stale `now`, which fixes a double ring when the time scale changes (T key).
- Genuinely good: making seizure/route edits validate *before* mutating anything.
- Genuinely good: byte limits on LLM HTTP response bodies and prompts.
- Not needed: an exactly-once command ledger (commands travel over an in-process
  channel that never redelivers; nothing consumed the receipts); runtime-generation
  fences (only matter once save/load replaces the engine, where a single `u64`
  generation check at the bridge suffices); a generic operation/duty kernel.

**M2 (simulation checkpoints) — correct analysis, ~10× too much code.**
- The state analysis was right and is worth re-deriving: `PERSISTENCE_INVENTORY.md`,
  exhaustive `Engine { … }` / `World { … }` construction so a new field breaks the
  build, and the rules for in-flight LLM work (a result already received applies
  exactly once; otherwise the original prompt is retried once; late callbacks from
  the replaced engine are ignored).
- The best evidence was ~15 **lockstep tests**: run an uninterrupted engine and a
  save→restore engine side by side with the same commands, and compare emitted
  events (and periodically the re-encoded state). Rebuild that harness.
- The sim state is plain `BTreeMap`/`Vec`/`f64` with immutable assets behind `Arc`
  and services as `Box<dyn …>`. `derive(Serialize, Deserialize)` on the real types,
  `#[serde(skip)]` + a `rebind(assets, services)` step, a version number, and
  ordinary errors should do it in a few thousand lines. Astra instead wrote each
  owner's fields four times (DTO, `serde(remote)` wire twin, view, builder; 128
  mirror structs) plus a 903-line allocation meter around serde.
- **Do not bind saves to the exact executable.** Astra hashed `/proc/self/exe`, the
  source tree and rustc into the save, so every rebuild invalidated every save. Use a
  format version plus content fingerprints that actually matter (e.g. the world map).
- **Do not hash the source tree in a build script**: it made any edit under `src/`
  recompile `cathedral-sim` and everything downstream.
- Measured costs were fine for an explicit save: capture p99 76/225 ms, restore p99
  40/119 ms, save size 3.1/12.2 MB (authored / +2,000 residents).

**M3 (save/load in the application)** reached a working save→load→continue on a
stripped test app (no renderer/HUD/weather), but never in the real game. The open
risk is not memory, it's that a load must swap ECS state the whole game depends on
(player, camera, HUD, map, weather, puppets, speech). Test that in the real app early
(headless drive runs) rather than last.
