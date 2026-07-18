# Handle the Snuffing — clock-aware chimney smoke

The chimney smoke that landed with `features/implemented/smokey_town.md` is a static hash pick:
`stable_hash("smoke-<building>-<stack>") % 4 == 0` lights 709 of 2,858 flues at city build and
they burn forever. At the seventh office — while the Scold rings curfew and every honest fire in
Ombreval is being covered — the same 709 chimneys keep smoking like it's High Wick. Curfew is
literally *couvre-feu*, cover-fire; the skyline should go out with it, and wake with the
Kindling, whose whole name is fires being lit.

The static hash stays as the *census* (which hearths exist and their character); this feature
adds *when each of them burns*.

## The day of a hearth

All times are sim-time. Offices ring at: Watch 02:00, Kindling 05:00, Dayspring 07:00,
High Wick 12:00, Waning 15:00, Lamplight 18:00, Snuffing 21:00
(`crates/cathedral-sim/src/clock.rs`, `Office::start_fraction`).

| window | ordinary hearths (~92% of the smoking subset) | early hearths (~8%) |
|---|---|---|
| Watch 02:00 + jitter | cold | **light** — bakehouses and brewhouses fire before anyone is up |
| Kindling 05:00 + jitter | **light** (ramp up over ~20 sim-min) | burning |
| through Lamplight | full burn — supper fires blaze, no dip | full burn |
| Snuffing 21:00 + jitter | **douse** (ramp down over ~15 sim-min) | douse |
| after ~21:40 until next light | cold — mesh emits nothing | cold |

Spec decisions:

- **Jitter** [spec decision]: light jitter 0–90 sim-min after the Kindling (the city wakes
  house by house, not on the stroke); douse jitter 0–40 sim-min after the Snuffing — the first
  minutes are the "dusk grace" between Evenblow's seventh office and the Scold's legal ring
  (`lore/second_sun/design/06_the_sound_of_the_city.md` §the Scold), the tail is stragglers.
  Watch-lighting jitter 0–45 sim-min. All jitter is drawn from the plume's existing seed
  (`unit(seed, shift)` with fresh shifts), so the schedule is deterministic per stack forever.
- **Early hearths** [spec decision]: the plan has no `bakery` use, so pick them by hash from
  the smoking subset — `unit(seed, 19) < 0.08`, doubled to `< 0.16` when the building's use is
  `workshop` or `industrial` (requires carrying `use_name` — or just a `bool early` — on
  `ChimneyAnchor`). Between the Watch and the Kindling the skyline shows a scattered handful of
  plumes, which is *more* correct than total darkness.
- **No midday dip**: hearths bank but don't die during the working day; keeping the curve flat
  from light to douse keeps it one readable story (lit → burning → covered).

## Hearth heat

One pure function, the heart of the feature:

```rust
/// 0.0 cold .. 1.0 full burn, for this plume at this day fraction.
fn hearth_heat(day_fraction: f64, seed: u32, early: bool) -> f32
```

Piecewise from the table: smoothstep up over the light ramp, 1.0 through the day, smoothstep
down over the douse ramp, 0.0 through the night, handling the midnight wrap (douse ends ~21:40,
the next light is 02:00+ or 05:00+; comparisons are on the wrapped `[0,1)` fraction). Pure and
`#[cfg(test)]`-friendly — unit-test it without an App.

Heat multiplies the puff's alpha, and a plume whose heat is 0 emits no quads at all — the night
mesh shrinks to (nearly) nothing for free. Keep the `Vec` capacities at the daytime maximum so
dawn doesn't reallocate.

## Sample heat at the puff's *birth*, not at now

A doused fire doesn't delete its airborne smoke; the plume runs out from the flue upward as the
last puffs finish their 9-second lives. That falls out naturally if each puff's alpha uses the
heat *when it left the flue*:

```
birth_day_fraction = day_fraction - age_wall_seconds * (scale / seconds_per_day)
alpha = PUFF_PEAK_ALPHA * fade_in * fade_out * hearth_heat(birth_day_fraction, seed, early)
```

At the default pace a 9-wall-second puff life is ~10 sim-minutes — the same order as the ramps —
so birth-sampling visibly matters (bottom-up run-out at the douse, flue-first ignition at the
light). No per-puff state is needed; birth time is derived, exactly like everything else in
`animate_chimney_smoke`.

`EngineMessage::Clock` already carries `seconds_per_day` (`crates/cathedral-sim/src/engine.rs`
§Clock); the projection at `src/smart_actors/mod.rs:742` just drops it. Add
`pub seconds_per_day: f64` to `WorldClockState` (`src/smart_actors/clock.rs` — NOTE: this file
has uncommitted local changes; coordinate) and copy it through in the drain.

## Wiring

- `animate_chimney_smoke` (`src/city/smoke.rs`) takes
  `clock: Option<Res<crate::smart_actors::clock::WorldClockState>>`. `Option` because the city
  builds headlessly in tests without `SmartActorsPlugin`; the import is a read-only projection,
  the same direction `city` already reads `controller::CollisionWorld`.
- **Fallback = full burn** [spec decision]: when the resource is absent or `present == false`
  (engine not yet online), every plume burns at heat 1.0 — exactly today's behaviour. Existing
  tests, `fake_backend`-less runs and early frames stay deterministic; the sky is only ever
  *dimmed* by information, never by its absence.
- `ChimneyAnchor` gains the `early` flag (computed in `add_chimneys` or in
  `build_chimney_smoke` from `building.use_name` + seed); `Plume` stores it.

Optional, cheap, while in there [not required]: exclude `use == "storage"` buildings from the
smoking census — 529 warehouses with lit hearths is its own small lie. This shrinks the subset
(~-20%); rebalance `SMOKE_GATE` from 4 to 3 if the skyline thins too much.

## Tests

- Unit-test `hearth_heat` directly: cold at midnight for an ordinary hearth, full at High Wick,
  monotonic ramp inside the light window, zero after douse-end, early hearth warm at 03:30
  while an ordinary one is cold, midnight wrap, determinism (same seed → same curve).
- App test, mirroring `puffs_fill_one_batched_mesh_of_camera_facing_quads`: insert a
  `WorldClockState` at High Wick → every plume emits (vertex count = plumes × puffs × 4);
  swap in 23:00 → the mesh is empty or near-empty (only puffs born before douse-end survive,
  and only within 9 wall-seconds of the swap); clock absent → full emission (fallback).
- The two existing contracts (`batched_city_keeps_render_entity_count_bounded`,
  `no_walkable_cell_is_solid`) are untouched — this feature adds no entities and no colliders.

## Drive-mode verification

`config.ron: smart_actors.clock.start_office` accepts any office name. Back up config.ron,
set `fake_backend: true`, then two runs with the same `tp` vantage
(e.g. `tp -200 35 0 -90 -8`):

1. `start_office: "high_wick"` → shot shows the familiar ~709-plume skyline;
2. `start_office: "snuffing"`, `sleep` past the douse window (or `T` at 60×) → shot shows the
   plumes gone; repeat at `"watch"` → only the scattered early hearths.

Restore config.ron after.

## Implementation notes (2026-07-18)

Implemented as specified, with three small deltas:

- `WorldClockState` is re-exported as `crate::smart_actors::WorldClockState` (the `clock`
  module is private; the file's idiom is selective `pub use`).
- The optional storage exclusion is in: warehouses carry a `cold` flag on `ChimneyAnchor`
  and the census skips them — 561 of 2,858 flues smoke (down from 709, the predicted ~-20%).
  The High Wick skyline still reads clearly lived-in, so `SMOKE_GATE` stays 4.
- Jitter shifts: watch-light 2, kindling-light 6, douse 10, early pick 19 (all fresh; kept
  ≤ 19 so `unit`'s `% 997` still sees enough bits to cover [0, 1)).

Drive-verified with `fake_backend: true` at `tp -200 35 0 -90 -8`: High Wick shows the
plumed skyline, the Snuffing douses it (empty by 23:44 after riding 60× past the jitter
window and letting the airborne nine-second puffs run out), the Watch shows the scattered
early hearths.
