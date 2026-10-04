Status: Implemented (2026-10-04). All three bugs were confirmed and fixed. The tests fail on the unfixed code and pass on the fix. Sim+backends suite green, host test green, headless drive run rendered (capped worker). Revised as R1, see [PLAN_REVIEW](PLAN_REVIEW.md).

# M1 — Baseline fixes

The original M1 ("live time and committed actions") set out to make the city keep running while the player reads,
talks or uses a menu. **That was already true.** Nothing in `src/` gates `Time<Virtual>` or the engine pump:
`pump_local_engine` runs unconditionally, and overlays only zero the player's movement input. What the M0 audit
and the plan review did find are three real bugs on the paths later milestones build on. This milestone fixes them
and pins D05 with a regression test. Nothing else.

## 1. Office cursors survive a time-scale change

`Round` and `NightOffice` remember the last office they processed as a host `now` (`last_office_now`) and later
re-read it through the *current* clock rate (`round.rs` ~6650, `night.rs` ~437). After a T-key rate change that old
`now` maps to a different calendar position, so a just-past office can be crossed again (a second Kindling restock
or Watch settlement) or an imminent one skipped.

- In `clock.rs`, add `WorldClock::offices_crossed_since(previous_days, now)`: the current `offices_crossed` body with
  the start taken as a calendar position (game days) instead of a host time. `offices_crossed(previous, now)`
  delegates to it, so existing callers and tests are unchanged.
- `Round` and `NightOffice` store `last_office_days` (a calendar position) instead of `last_office_now`, and advance
  it to the calendar position they just processed. Update the doc comments, including the one in `engine.rs` that
  names `round.last_office_now`.
- Leave the bells' `last_clock_now` (`engine.rs`) alone. `ring_offices` runs before commands, so it does not have
  the bug.

Tests (sim crate, game behaviour). For each, write the test first and confirm it fails without the fix.
- A rate change from 10× to 60× just after Kindling does not re-cross it, and a change from 60× to 1× just before
  Kindling does not skip it.
- A `CycleTimeScale` landing on a skipped 20 Hz round tick produces exactly one Kindling restock in `food_log` and one
  Watch settlement.
- A 60× → 1× change one frame before a Major's bedtime still queues the reflection. Night is day-guarded against
  *double* reflection already, so don't add a test for that.

## 2. `take_into_charge` validates before it mutates

In `actions.rs`, `take_into_charge` calls `world.custody.seize(...)` before the fallible
`route_budget(...)?`. With no route, the verb reports `NoRoute` to the LLM while the custody record stands, and the
witness percepts and the seize world event are skipped (tick_custody later re-lays the walk, so it is not an orphan,
but the report and the state disagree). Resolve the station and route budget first, then seize.

Test: an officer on an unroutable spot returns `Err(NoRoute)`, the target is not in custody, and no seize event is
emitted.

## 3. LLM response bodies are bounded

`crates/cathedral-backends/src/llm.rs` reads the whole HTTP response body into memory, both on error and on success.
Read at most ~1 MiB (a `chunk()` loop):

- **Success body:** an oversize body becomes the existing non-retryable failure.
- **Error body:** truncate it before building the snippet, and keep the existing retryable/non-retryable
  classification by status code.

The request timeout and `MAX_LLM_REPLY_CHARS` stay as they are. Test with the loopback server in `testing.rs`
serving a 2 MiB body.

## 4. D05 regression test

One host test (with `ready_fake_plugin_app` in `src/smart_actors/mod.rs`): open each input-capturing overlay in turn
(journal, chat, inventory, map, Esc settings) and assert that the published simulation clock and an NPC's position
keep advancing.

## Do not build

All of these were the original M1 and are cut (see PLAN_REVIEW):

- an accepted-time / wall-time-debt coordinator or recovery budget (keep the 100 ms `MAX_FRAME_CATCHUP` clamp in
  `src/controller.rs`);
- a logical time origin, runtime generations or fences (save/load in M2/R3 does this with a host `time_offset`
  and a fresh backend channel);
- command identities, a receipt envelope, an idempotency ledger or a replay window;
- an operation/duty kernel or a due-expiry boundary;
- an input watermark or physical-sample protocol;
- a hard prompt-size refusal (it would mute an NPC permanently);
- any RUNTIME_BUDGETS measurement or evidence directory.

## Done when

- The tests above pass in `cargo test -p cathedral-sim -p cathedral-backends` and the host test passes in the game
  crate, all through the capped worker.
- The game builds and a headless drive run starts and renders.
