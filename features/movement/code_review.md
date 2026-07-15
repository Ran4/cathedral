### Findings of potential M0 bugs

Verify these findings before fixing them.

1. High — Bell timing breaks at the required 60× debug speed.
 Bell strokes are appended to a FIFO even though their due times can overlap between offices (crates/cathedral-sim/src/engine.rs:1245). With a 60-second day, High Wick’s final stroke is due at 21.5 s, while the Waning’s first is due at 20 s but is
 appended behind it. The drain only examines the front (crates/cathedral-sim/src/engine.rs:1256), causing delayed and simultaneous strokes rather than the specified three-second countable rhythm (features/movement/01_the_clock.md:126). Existing tests
 cover only the first High Wick stroke, not overlapping complete ordinals.

2. Medium — The new inbox bound is not actually an invariant.
 Normal notification paths cap the buffers (crates/cathedral-sim/src/character.rs:202), but scheduler retry/error paths mutate them directly. A provider failure can restore 64 entries and immediately push a 65th (crates/cathedral-sim/src/
 scheduler.rs:479); repeated failures grow indefinitely. Mid-flight percepts can similarly be prepended to an already-full pending_history. This leaves the explicit “bound it” requirement only partially fixed.

3. Medium — The Scold/legal-curfew boundary from the detailed clock specification is missing.
 The specification requires the Scold to ring minutes after the Snuffing office, with the interval representing dusk grace (features/movement/01_the_clock.md:132). The implementation has only the Lanthorn town_bell path (crates/cathedral-sim/src/
 engine.rs:1285) and no second curfew boundary or Scold sound. Consequently, later schedule/curfew code cannot distinguish liturgical Snuffing from legal curfew. The short M0 checklist omits this, but the detailed M0 clock document explicitly
 includes it.

4. Medium — night_brightness: NaN reaches Bevy unchanged.
 RON accepts NaN; night_brightness.clamp(0.0, 1.0) preserves it (crates/cathedral-sim/src/clock.rs:271). It then becomes a NaN sun illuminance and ambient brightness (src/smart_actors/clock.rs:98). Clock configuration should reject or safely default
 non-finite brightness values.

5. Low — The documented headless acceptance command does not test two days.
 -t 120 means 120 NPC turns, not 120 seconds (crates/cathedral-backends/src/bin/cathedral_headless.rs:79). Running the exact documented command produced 13 office crossings and stopped at the Kindling on day 2, rather than completing two days. The
 new --watch-clock 2 path is the deterministic clock test, but the milestone still documents -t 120 (features/movement/07_milestones.md:56). The movement README also still opens with “Status: proposed. No code written.”
