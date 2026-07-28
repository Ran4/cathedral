# Cathedral-city bug hunt — 2026-07-28

21 finder agents swept ~131k lines of Rust and raised 70 candidates, 66 after dedup. Every candidate went to two independent verifiers — an adversarial refuter told to kill the claim, and a tracer that had to walk a reachable failure path — with an xhigh judge breaking splits.

**46 confirmed, 20 rejected.**

Coordinates quoted below are live post-shrink values read out of the current tree, not
historical pre-shrink records.

Two findings are baked into committed assets and need
`export_collision_footprints` → `bake_navigation.py` re-run after the code fix:
`src/city/mod.rs:7301` (curtain towers) and `src/city/mod.rs:4158` (bridge spine piers).

Seventeen findings sit in the arrest/escort/gaol path (`features/implemented/law_and_order.md`
M4–M5). The five high-severity ones compound: the grip dies to the dead-man timer the poll
after it lands, `grab_reflex` often cannot fire it at all because it measures the officer
from a frozen mirror position, `let_go` is never projected so the visible grip is never
released, and the strain meter is overwritten before it renders. Treat them as one repair,
not five.

## Index

[ ] -> open
[F] -> Fixed

- [F] **high** `crates/cathedral-sim/src/round.rs:5505` — interrupt_for_conversation never stops a walk to a food stall
- [F] **high** `crates/cathedral-sim/src/round.rs:5531` — Road-party members' go_to intents are never ticked, so they never arrive or lapse
- [F] **high** `crates/cathedral-sim/src/engine.rs:3026` — Dead-man timer can drop a grip next poll: `grab` never refreshes officer_last_turn
- [F] **high** `crates/cathedral-sim/src/inventory.rs:1531` — debit_sparks spends tagged sparks the solvency check never counted
- [F] **high** `crates/cathedral-sim/src/custody.rs:384` — Dead-man timer never stamped at seizure or on grab, so a grip dies next poll
- [F] **high** `crates/cathedral-sim/src/weather.rs:748` — Street wetness snaps 0.09 -> 0.77 in one game minute at the daylight cliff
- [F] **high** `crates/cathedral-backends/src/llm.rs:575` — Retry-After clamped after Duration::from_secs_f64, which panics and wedges cognition
- [F] **high** `src/smart_actors/custody.rs:146` — Strain-meter bar is overwritten before it renders, so pulling shows no progress
- [F] **high** `src/smart_actors/custody.rs:285` — grab_reflex measures the officer from the frozen snapshot position, not live
- [F] **high** `src/city/mod.rs:7301` — Curtain towers are planted inside gate openings, walling the arches shut
- [F] **high** `src/city/mod.rs:4158` — Bridge spine pier is sized from the mouth WIDTH, so half of it stands outside the shell
- [F] **high** `src/city/route_boards.rs:54` — Route boards use pre-shrink coordinates; two float outside the world entirely
- [F] **high** `src/smart_actors/mod.rs:900` — Engine disconnect never clears PlayerCustodyState — a held player stays tethered forever
- [F] **medium** `crates/cathedral-sim/src/round.rs:7130` — Curfew rung's Stay-at-home short-circuits rung 3's eat-what-you-hold
- [F] **medium** `crates/cathedral-sim/src/actions.rs:3388` — `release` never tells the person being released: the second-person branch is unreachable
- [F] **medium** `crates/cathedral-sim/src/actions.rs:3455` — `struggle`'s attempt counter saturates, so every repeat attempt replays one frozen die
- [F] **medium** `crates/cathedral-sim/src/actions.rs:3522` — `announce_struggle` excludes the struggler, so an NPC gets no record of their own struggle
- [F] **medium** `crates/cathedral-sim/src/engine.rs:3320` — announce_commitment's hand-release loop is always empty (commit cleared holders)
- [F] **medium** `crates/cathedral-sim/src/engine.rs:3275` — Engine custody endings emit only `let_go`, which nothing clears the grip on
- [F] **medium** `crates/cathedral-sim/src/engine.rs:2742` — ramp_urgency deletes the debug-written `urgency` status in the same poll it is set
- [F] **medium** `crates/cathedral-sim/src/scheduler.rs:812` — An actor queued in both the player-reaction and priority lanes takes two turns
- [F] **medium** `crates/cathedral-sim/src/custody.rs:414` — commit clears holders before the arrival announcement reads them
- [F] **medium** `crates/cathedral-sim/src/weather.rs:1073` — Forced weather's 8-hour elapsed cap freezes the wet aftermath forever
- [F] **medium** `crates/cathedral-sim/src/nav/mod.rs:590` — offset_route validates only vertices, so lane paths run through walls for up to 10 m
- [F] **medium** `crates/cathedral-backends/src/worker.rs:362` — Worker::forget kills the child but never reaps it: a zombie per worker failure
- [F] **medium** `src/smart_actors/mod.rs:1252` — Custody sounds placed from the cold snapshot, which lacks a mover's live position
- [F] **medium** `src/smart_actors/actors.rs:235` — MovementInbox is never pruned, so a stale pose overrides an authoritative reposition
- [F] **medium** `src/smart_actors/body.rs:2102` — Urgency multiplies the absolute accumulated gait phase, snapping the legs
- [F] **medium** `src/smart_actors/body.rs:2649` — Gait-phase interpolation sweeps backwards when the sim resets the phase to 0
- [F] **medium** `src/city/mod.rs:7338` — Wall-tower collider is the AABB of a 45-degree-rotated square: twice the visible footprint
- [F] **medium** `src/scene.rs:425` — Outer aisle wall split does not line up with the transept: 3 m hole in the cathedral shell
- [F] **medium** `src/scene.rs:983` — Baldachin's two front columns float 0.54 m above the altar steps
- [F] **medium** `src/soundscape.rs:1597` — Daily curfew peal never claims the Scold's cooldown, so a summons rings on top of it
- [F] **medium** `src/drive.rs:109` — drive `frame` ignores the camera's 0.65 m eye offset, aiming shots 0.65 m too high
- [F] **medium** `src/smart_actors/mod.rs:1300` — `let_go` world event is never projected — the visible custody grip is never released
- [F] **medium** `crates/cathedral-sim/src/actions.rs:3603` — `announce_grip`'s second-person branch is unreachable, so a prisoner is never told a hand landed on or left their arm *(found while fixing the above; not one of the 46 original findings)*
- [ ] **low** `crates/cathedral-sim/src/round.rs:6058` — service_stalls never checks custody: a marched-off prisoner still completes a sale
- [ ] **low** `crates/cathedral-sim/src/round.rs:2032` — A road member left behind by the law is dropped from the party and never enrolled
- [ ] **low** `crates/cathedral-sim/src/actions.rs:3141` — `seize` with an explicit notice_id discards the second door instead of testing it
- [ ] **low** `crates/cathedral-sim/src/prompt/mod.rs:989` — Notice cap can drop the wronged actor's own notice while settle_notice is offered
- [ ] **low** `crates/cathedral-sim/src/night.rs:744` — A rejected ward set_round still permanently teaches the Minor the way
- [ ] **low** `src/smart_actors/hands.rs:628` — One out-of-range holder takes every other officer's hand off the prisoner
- [ ] **low** `src/city/mod.rs:5067` — Firewood rick stacks its two columns along the log axis instead of across it
- [ ] **low** `src/scene.rs:941` — Apse arcade is missing its final boundary column, leaving the hemicycle asymmetric
- [ ] **low** `src/soundscape.rs:2020` — "Once a day" NPC yawn fires twice per evening: the day rolls inside the Snuffing
- [ ] **low** `src/screenshot.rs:89` — Dead-key screenshot binding bypasses the chat box's keyboard suppression
- [ ] **low** `src/weather/mod.rs:201` — smooth_weather spends its one-time snap on the CLEAR default, not the first sim sample

---


# HIGH severity (13 findings)


## crates/cathedral-sim/src/round.rs:5505

### interrupt_for_conversation never stops a walk to a food stall

`interrupt_for_conversation` only halts a walk when the phase is one of the three walking phases:

```rust
if !matches!(
    person.phase,
    Phase::Approaching | Phase::Travelling | Phase::Returning
) {
    return;
}
```

But a stall errand deliberately leaves the phase at `Idle` while the body walks — `apply_decision` for `Decision::ApproachStall` does `person.phase = Phase::Idle; ... set_route(world, id, path)` (round.rs:7422-7433, "Idle + a food errand: the water phase is left standing"). So a buyer under way to a pitch matches none of the three phases and the function returns without clearing `character.state.movement`. Nothing else stops them either: `run_ladder` skips anyone with `round.people[&id].food.is_some()` (line 6712), and `resolve_food_arrivals` only acts once `is_walking()` is false.

**Failure scenario.** An NPC crosses HUNGER_HUNGRY, rung 7 picks a stall and lays the walk (phase Idle, food = Some(Approaching)). The player walks up and speaks to them; the engine calls `round::interrupt_for_conversation`, which returns immediately. The NPC keeps walking the whole path to the Wickmarket pitch and joins the queue while the player's line is still being answered — leaving the conversation partner behind, the exact drift the function exists to prevent.


## crates/cathedral-sim/src/round.rs:5531

### Road-party members' go_to intents are never ticked, so they never arrive or lapse

`tick_intents` walks only the enrolled cast:

```rust
let ids: Vec<ActorId> = round.people.keys().cloned().collect();
```

Road-party members are explicitly excluded from `people` at seed (`.filter(|id| ... && !self.is_road_member(id))`, line 2480), yet `go_to` accepts them — it refuses only `leaving_city` and custody (actions.rs:2405-2421), and the sight-gated `go_to { person }` branch needs no `places_known`. Nothing else stamps `intent.deadline`, detects arrival, updates `last_seen`, or emits the arrival/lapse percept + nudge. Meanwhile `tick_road_parties` lets that dead intent override the party's trade leg every tick:

```rust
let target = actor.state.intent.as_ref().map_or(target, |intent| match &intent.target {
    IntentTarget::Place { point, .. } => *point,
    IntentTarget::Person { last_seen, .. } => *last_seen,
});
```

(line 1802-1810). Only `begin_road_return` ever clears it.

**Failure scenario.** Rowan of the Lantern (road party `lantern_stone_gate`) is InCity at Dayspring and issues `go_to { person: "..." }` for someone in `you_see`. `tick_road_parties` walks him to the frozen `last_seen` point and parks him there (`movement = None` inside ROUND_ARRIVE_RADIUS_M). He never gets "You have caught up with …", the intent never expires, and for the whole Dayspring→Lamplight trading window he stands away from the Seven Lofts pitch, so `counter_binding` fails its `position_m().distance(counter.pitch) > counter.radius_m` test and Betriss's `betriss_grain` stock errand never opens that trip.


## crates/cathedral-sim/src/engine.rs:3026

### Dead-man timer can drop a grip next poll: `grab` never refreshes officer_last_turn

`tick_custody` clause 1 releases a grip that has gone stale:

```rust
if record.is_held() && now - record.officer_last_turn > custody::CUSTODY_DEAD_MAN_SECONDS
{
    ungripped.push(prisoner.clone());
}
```

but `officer_last_turn` has exactly three writers (`Custody::seize` at seize time, `seed_inmate` = 0.0, and `Engine::poll`'s `take_submitted()` block at engine.rs:1341). Putting a hand on the arm does not touch it: `Custody::grab` (custody.rs:377-386) only pushes the holder and clears `closing`, and `Engine::player_grabbed` (engine.rs:3487-3505) calls it and then only `prioritize`s the holder — it never stamps `record.officer_last_turn = now`. So the timer measures "time since the *officer of record* was last submitted a prompt", yet it is applied the instant *anyone* takes hold.

**Failure scenario.** Havise Ashe seizes the player at t=0 (`Custody::seize` stamps `officer_last_turn = 0`). The player complies and stands still; under the novelty/curiosity gate Ashe is not handed another turn for 70 s. At t=70 the player bolts, Ashe closes, and the host's grab reflex sends `PlayerGrabbed` → `holders = [Ashe]`, `closing = false`. On the next poll (t≈70.02) `tick_custody` sees `70.02 - 0.0 > 60` and pushes the player into `ungripped`, releasing the grip with "Havise Ashe lets go of your arm" ~16 ms after it landed. The tether never engages and the player simply walks away. The same happens NPC↔NPC when a second officer `grab`s somebody else's prisoner: at submission time `prisoners_of(second_officer)` is empty, so no stamp is made, and the fresh grip is judged against the *first* officer's last turn.


## crates/cathedral-sim/src/inventory.rs:1531

### debit_sparks spends tagged sparks the solvency check never counted

`spendable_sparks`/`wallet_sparks` measure the purse with an *exact* matcher — `ItemMatcher::new("spark")` has an empty metadata map and `ItemMatcher::matches` requires `self.metadata == item.metadata` — so a `spark` stack carrying `{"condition":"wet"}` or `{"condition":"poopstained"}` is worth 0. But `debit_sparks` picks the stacks to consume by **kind only**:

```rust
let ids: Vec<ItemId> = self.characters[owner]
    .holds().iter()
    .filter(|id| self.items.get(*id).is_some_and(|item| item.kind.as_str() == "spark"))
```

and then drains them in `holds` order (`let take = remaining.min(self.uncommitted_quantity(&id)); self.consume_item_quantity(owner, &id, take)?`). The guard above it (`if self.spendable_sparks(owner) < amount`) therefore counts one set of coins and the deduction spends a different, larger set. Tagged sparks are not exotic: `actions.rs:1494` restamps a spark to `condition=wet` when it is put in the mouth, `engine.rs:2702` automatically restamps anything sharing a lower slot with a stool to `condition=poopstained`, `retrieve_item` never removes the stamp, and `assets/world/seed.json` ships `sp001` — Gude Quern's wet cheek coin — from seed. Every payer of sparks is affected: `market_sale_inner` (`funds = spendable_sparks(buyer)` then `debit_sparks(buyer, spend)`), `settle_wallet_exact`, and `round.rs::settle_households`.

**Failure scenario.** Gude Quern (`p004q`) holds only `sp001` (`spark{condition=wet}`, qty 1) and retrieves it from her mouth, so `holds = [sp001]`. She is then paid 3 sparks: `credit_sparks` -> `add_stock` finds no *plain* spark stack to merge into and appends a new one, giving `holds = [sp001(wet,1), spNEW(plain,3)]`. `wallet_sparks`/`spendable_sparks` both report 3. She buys a 3-spark item: the guard passes, then the loop consumes 1 from `sp001` first and only 2 from `spNEW`. She receives goods worth 3 sparks while her purse falls from 3 to 1 — one spark created out of nothing and the authored wet coin silently destroyed. The same path makes `settle_wallet_exact(owner, target)` miss its boundary (a visitor unloaded to 0 walks out still holding sparks) and lets a `settle_households` donor hand over 4 sparks a night while their spendable purse drops by less.


## crates/cathedral-sim/src/custody.rs:384

### Dead-man timer never stamped at seizure or on grab, so a grip dies next poll

`CustodyRecord::officer_last_turn` is documented as "Real seconds at `seize` ... stamped by the engine", and `engine.rs:3026` releases the grip with `if record.is_held() && now - record.officer_last_turn > custody::CUSTODY_DEAD_MAN_SECONDS`. But the only writer of a *new* record is `actions::take_into_charge`, which passes a literal zero:

```rust
world.custody.seize(target_id.clone(), officer_id.clone(), notice_id, station, 0.0);
```

so a record created at engine time `now = 500` claims its officer last thought at app start (`now` is `time.elapsed_secs_f64()`, monotonic since launch). The engine's only refresh (`engine.rs:1339-1343`, `for prisoner in self.world.custody.prisoners_of(&actor_id) { record.officer_last_turn = now; }`) runs at LLM *submission* time, i.e. before the seizing action is applied, so it cannot stamp the record that action creates. And `Custody::grab` — the one place a hand ever lands — resets only the chase latch:

```rust
if !record.holders.contains(&holder) { record.holders.push(holder); }
record.closing = false;
```

leaving `officer_last_turn` stale. `Engine::player_grabbed` (the host reflex, `engine.rs:3498`) does not stamp it either.

**Failure scenario.** Sergeant Ashe seizes the player 5 minutes into a session (`now = 300`); the record gets `officer_last_turn = 0.0`. The player strays past `CUSTODY_LEASH_M`, the latch sets, Ashe walks in and the host reflex fires `player_grabbed` -> `Custody::grab`. On the very next `tick_custody` poll `now - 0.0 = 300 > 60`, so the dead-man branch runs `release_grip` and announces "Ashe lets go of your arm" — the grip lasts one frame. Because `grab` also cleared `closing`, the reflex cannot fire again until the player strays past 8 m afresh, so the tether, the strain meter and the struggle are unreachable for the rest of that custody unless the officer happens to be handed an LLM turn in the seconds between the seizure and the grab.


## crates/cathedral-sim/src/weather.rs:748

### Street wetness snaps 0.09 -> 0.77 in one game minute at the daylight cliff

`accumulated_water` derives ONE instantaneous drying rate from the sample instant's daylight/cloud/wind and then applies it retroactively across the whole elapsed window:

```rust
let hours_since = ((time - episode.rain_end).max(0.0)) * HOURS_PER_DAY;
let daylight = summer_daylight(time.rem_euclid(1.0));
let wind = wind_speed(current.wind_xz_mps);
let drying_per_hour = if daylight < 0.08 {
    0.045
} else {
    0.10 + 0.42 * daylight * (1.0 - current.cloud_cover * 0.78)
        + 0.025 * wind.min(10.0)
};
let contribution = saturated * (-drying_per_hour * hours_since).exp();
```

`summer_daylight` crossing the hard `< 0.08` branch flips `drying_per_hour` from ~0.17 to 0.045 in a single step, and because that rate multiplies `hours_since` (5-15 h by then) the exponential result jumps by a huge factor. Two consequences: surface_wetness is discontinuous at dusk and dawn, and it *rises* steadily through every dry evening (drier air is modelled as if it had always been night), which is backwards. The existing test `semantic_revision_changes_only_at_actual_named_boundaries` only samples schedule boundaries, so the daylight cliff (~20:11 and ~05:15) is never checked.

**Failure scenario.** `WeatherTimeline::default()` sampled every game minute: at day 25, 20:19 — `precipitation == 0.0`, `kind == Clear` — `surface_wetness` goes 0.0862 -> 0.7670 between two consecutive minutes; the mirror image happens at day 2, 05:16 (0.2715 -> 0.0488). Over a 60-day, 1-minute sweep the worst dry-weather rise is +0.68/minute. `wetness_band()`/`prompt_phrase()` therefore flip an NPC's sheet from "the stones are damp" to "the streets are soaked" in one minute with no rain, and the host's wet-street material pops across the whole city at nightfall.


## crates/cathedral-backends/src/llm.rs:575

### Retry-After clamped after Duration::from_secs_f64, which panics and wedges cognition

`parse_retry_after` validates the header but clamps only the *constructed* Duration:

```rust
let seconds: f64 = header?.trim().parse().ok()?;
if !seconds.is_finite() || seconds < 0.0 {
    return None;
}
Some(Duration::from_secs_f64(seconds).min(MAX_RETRY_AFTER))
```

`Duration::from_secs_f64` panics for any *finite, non-negative* value above ~1.8e19 seconds (`cannot convert float seconds to Duration: value is either too big or NaN`). I confirmed this on this toolchain: `"1e20".parse::<f64>()` is finite and non-negative, and `Duration::from_secs_f64(1e20)` panics. The `is_finite()`/`< 0.0` guards do not cover overflow, and `.min(MAX_RETRY_AFTER)` runs too late to help.

The call sits in `LlmClient::attempt`, which runs inside the tokio task spawned by `HttpCognition::submit_on`, whose tail is the only thing that frees the lane and reports the result:

```rust
lane.store(false, Ordering::SeqCst);
events.send(Completion { request_id, result: ..., duration_seconds });
```

A panicking task is caught by tokio and simply dropped, so neither line ever executes.

The same unguarded conversion exists at line 480, `Duration::from_secs_f64(self.settings.timeout_seconds.max(0.001))`, reachable from `LLM_TIMEOUT_SECONDS` (which `Environment::float` only filters for non-finiteness, not magnitude).

**Failure scenario.** A rate-limited provider (or a proxy/CDN in front of it) answers 429 with `Retry-After: 100000000000000000000`. `is_retryable_status(429)` is true, so `parse_retry_after` is called, `Duration::from_secs_f64(1e20)` panics, and the spawned task dies before `lane.store(false, ...)` and before any `Completion` is pushed. `HttpCognition::busy` stays `true` for the rest of the process, so every subsequent `request()` returns `Err(CognitionBusy)`; meanwhile `NpcScheduler.in_flight` is only ever cleared by a matching completion (crates/cathedral-sim/src/scheduler.rs:385-408 — there is no in-flight timeout), so it never clears either. The entire cast stops taking LLM turns for the remainder of the run, with no error line, no degraded status, and no backoff.


## src/smart_actors/custody.rs:146

### Strain-meter bar is overwritten before it renders, so pulling shows no progress

`law_standing_hud` is scheduled in `Update`:

```rust
app.init_resource::<PlayerCustodyState>().add_systems(
    Update,
    (grab_reflex, strain_meter, law_standing_hud).chain(),
);
```

and it is the only writer of the strain bar:

```rust
hud.set_law_standing(... format!(
    "HELD BY {} — pull away to struggle free\n{bar}", ...
))
```

But `apply_law_standing` (this file, line 169: `hud.set_law_standing(standing_text(notices, state.custody.as_ref()))`) is called from `drain_bridge_messages` in `SmartActorSet::DrainBridge` (PostUpdate), and the HUD text is only pushed into the UI by `hud::update_smart_actor_hud` in `SmartActorSet::Present` (PostUpdate, ordered after DrainBridge — mod.rs:534-545, 668-671). So per frame the write order is: Update writes the bar → PostUpdate/DrainBridge overwrites it with the bar-less `standing_text` → PostUpdate/Present renders. Every frame that carries a `LawStanding` message, the bar is discarded before it can be drawn; `standing_text` never renders a bar.

The message arrives constantly while held: `Engine::publish_law_standing` (crates/cathedral-sim/src/engine.rs:3633-3666) sets `anchor_m` from the holder's *live* `position_m()` and republishes whenever the whole message differs. An escorting officer keeps walking (round.rs rung 8 `go_to` is gated only by `!escorting`), and movement samples arrive at 20 Hz, so `anchor_m` changes ~20 times a second.

**Failure scenario.** Havise Ashe grabs the player in the Wickmarket and starts dragging them to the Bellstand. The player holds W away from her; `strain_meter` fills the meter correctly and `law_standing_hud` formats `HELD BY HAVISE ASHE — pull away to struggle free\n[####------]`. Because Havise is walking, `anchor_m` changes on ~20 of every 60 frames, so `LawStanding` is republished and `apply_law_standing` replaces the line with the bar-less `standing_text` before `update_smart_actor_hud` runs. On screen the `[####------]` bar strobes at 20 Hz / is effectively absent for the whole drag — the one situation the meter exists for — so the player gets no feedback that pulling is doing anything.


## src/smart_actors/custody.rs:285

### grab_reflex measures the officer from the frozen snapshot position, not live

The reflex takes the officer's position out of `WorldMirror`:

```rust
let Some(officer) = mirror.actor(officer_id) else {
    return;
};
let here = position.current;
let officer_at: Vec3 = officer.position_m.into();
let separation = here.distance(officer_at);
if separation > CUSTODY_REACH_M as f32 {
    return;
}
```

`WorldMirror` is rebuilt only from `EngineMessage::Snapshot`, and the sim emits a snapshot only when `world_revision` bumps (`Engine::flush`, engine.rs:2775-2778). Walking never bumps it — movement rides the hot `Movement` channel, which mod.rs:1052-1069 documents as something that "must never touch the mirror or bump a revision". So for a *walking* officer, `officer.position_m` is frozen wherever the last unrelated revision bump left it; that is exactly why `hands::hold_the_seized` is ordered after `drive_npc_bodies` to use the hot channel instead (mod.rs:599-603).

This defeats the module's own stated reason for existing ("the sim reads the player at `POSITION_UPDATE_HZ = 10`, which is 1.2 m of travel per sample … so a 3 m radius decided over there would be wrong by most of its own radius") — the officer half of the same 3 m test is read from a source that is *less* fresh, and unboundedly so. The live grip point is already in hand: the sim fills `custody.anchor_m` with the officer's current `position_m()` whenever nobody has hold yet (engine.rs:3635-3640).

**Failure scenario.** Havise Ashe seizes the player outside the Wickmarket and starts walking north to the Bellstand; the player sprints 30 m away, crosses the leash, and the sim latches `closing` and turns her around. Nothing during those two walks bumps `world_revision`, so `mirror.actor("sergeant0").position_m` is still the seizure point. Havise's rendered body walks up and stops at arm's length, but `separation` computed against the frozen point is ~30 m > `CUSTODY_REACH_M`, so `PlayerGrabbed` is never sent: she stands next to the player indefinitely and never takes hold. The mirror case is equally reachable — the player walking back over her stale position gets grabbed while she is visibly 20 m up the street.


## src/city/mod.rs:7301

### Curtain towers are planted inside gate openings, walling the arches shut

`build_fortifications` cuts the curtain for each gate via `wall_ranges_around_gates(start, end, &openings)`, but the tower loop that follows ignores `openings` entirely:

```rust
let mut tower_points = plan.wall_polygon_xz.iter().map(|point| Vec2::from_array(*point)).collect::<Vec<_>>();
for (start, end) in plan.wall_polygon_xz.iter().zip(plan.wall_polygon_xz.iter().cycle().skip(1)) {
    let divisions = (start.distance(end) / 115.0).floor() as usize + 1;
    for step in 1..divisions { tower_points.push(start.lerp(end, step as f32 / divisions as f32)); }
}
```

Every vertex and every 115 m division gets a 12 m tower with a full-height collider, whether or not a gate was just cut there. Two of the five gates get a tower dropped straight into the arch.

**Failure scenario.** Harne Gate: opening `(10.5, -465.5)`, width 18. The wall edge `(231.0,-472.5)→(-311.5,-462.0)` is 542.6 m, so `divisions = 5` and `step = 2` lands a tower at exactly `(14.0, -468.3)` — 4.5 m from the gate centre, inside the 14 m arch between `gate_harne_2` (x ≤ 3.5) and `gate_harne_1` (x ≥ 17.5). Its collider covers x [5.5, 22.5] × z [-476.8, -459.8]. The committed bake (`assets/world/navigation.bin`) leaves only x ∈ [3.75, 4.75] walkable at z = -466 — a 1.25 m slot for a main city gate. Stone Gate: the wall-vertex tower at `(353.5, 91.0)` covers x [345, 362] × z [82.5, 99.5], eating 8.5 m of the ~18 m curtain gap; the nominal gate centre `(346.5, 94.5)` is baked non-walkable and traffic is squeezed into z ∈ [100, 106.5].


## src/city/mod.rs:4158

### Bridge spine pier is sized from the mouth WIDTH, so half of it stands outside the shell

`build_bridge_supports` sizes the spine pier from the mouth width and centres it on the mouth midpoint:

```rust
for end in ends {
    let size = Vec2::new(1.25, (width - 1.0).max(1.2));
    ...
    let center = Vec3::new(end.x, 2.1, end.y);
    spawn_rotated_box_named(..., center, Vec3::new(size.x, 4.2, size.y), angle, ...);
    add_rotated_box_collider_at(collision_world, center, Vec3::new(size.x, 4.2, size.y), angle);
}
```

`width` is the SHORT-edge (mouth) length, but `size.y` is the extent ALONG the passage. Because `center` is the mouth midpoint, exactly `(width - 1)/2` of the pier projects outside the building footprint — and it collides from y 0 to 4.2, i.e. right through the walk band.

**Failure scenario.** `named_malt_house` has a 27 m mouth and a 45 m shell, so each pier is 26 m long and overhangs 13 m. The south pier runs from (153.1, 68.6) to (150.8, 94.5) — 13 m of it outside the shell, standing perpendicular across FABRIC WAY (7 m wide, centreline through (152, 71)). The committed `assets/world/collision_footprints.json` contains the resulting 3.5 × 26.0 footprints at (152.0, 81.6) and (148.0, 126.4), and the baked walkable grid shows Fabric Way severed at x 151–154 for all z ≥ 69, leaving a ~2 m squeeze at z 67–68.6. The north pier's outer half ends up buried inside `named_masons_lodge`.


## src/city/route_boards.rs:54

### Route boards use pre-shrink coordinates; two float outside the world entirely

`PLACEMENTS` was authored in commit c5455a6 ("Add route boards") and was never updated by 68b630c ("Make world smaller (30% less width/height)") — `git log c5455a6..HEAD -- src/city/route_boards.rs` is empty. The constants are still 1/0.7 of the shipped world:

```rust
BoardPlacement { location: "The Wool Gate",  position: [-53.0,  BOARD_CENTER_Y_M, 497.40], ... }
BoardPlacement { location: "The Stone Gate", position: [482.90, BOARD_CENTER_Y_M, 158.0 ], ... }
BoardPlacement { location: "Seven Lofts",    position: [341.02, BOARD_CENTER_Y_M, 336.17], ... }
BoardPlacement { location: "The Draper's Reach", position: [132.15, BOARD_CENTER_Y_M, 256.77], ... }
```

The shipped world is x∈[-409.6, 354.5], z∈[-475.5, 367.0] (assets/world/areas.json + collision_footprints.json), and the ground quad only spans GROUND_MIN/MAX = x∈[-497,385], z∈[-521.5,455] (src/city/mod.rs:45-48). Multiplying each placement by 0.7 lands it on/near its named area, which is the tell. The module doc claims the boards are "mounted a few centimetres proud of existing solid façades"; none of them are. `exactly_the_four_annotated_places_receive_a_board` only checks names and unit normals, never positions.

**Failure scenario.** Wool Gate area box is x∈[-31.5,-17.5], z∈[347,367] but its board is spawned at (-53, 3.25, 497.40) — 130 m past the northern edge of every building and 42 m past the end of the ground mesh, so it is an unlit 5.6×4.2 m panel hanging in empty sky. The Stone Gate board at (482.9, 3.25, 158.0) is likewise 128 m east of the outermost footprint (max x 362) and 98 m past GROUND_MAX_X, while the real Stone Gate is at x 338.5–354.5, z 82.5–106.5. Seven Lofts' board sits ~55 m outside the Seven Lofts area (x 193–311, z 185.5–283.5) and the Draper's Reach board ~37 m outside its area (x 94.4–105.6, z 208.4–219.6), both clipping into unrelated façades.


## src/smart_actors/mod.rs:900

### Engine disconnect never clears PlayerCustodyState — a held player stays tethered forever

The `BridgeEvent::Disconnected` arm resets the runtime, the interaction state, the microphone and the HUD transients, but touches none of the hot-channel resources:

```rust
bridge::BridgeEvent::Disconnected(message) => {
    runtime.connected = false;
    ...
    interaction.clear_pending();
    microphone_input.clear_on_disconnect();
    hud.clear_transients_on_disconnect(truncate_owned(message, 300));
    presentation.clear.write(speech::ClearSpeechPresentation);
    ...
}
```

`hot.law` (`custody::PlayerCustodyState`) is left exactly as the last `LawStanding` set it. `PlayerCustodyState::tether()` returns `Some((custody.anchor_m, CUSTODY_TETHER_M))` for any `custody.held`, and `controller.rs:752` clamps the player's *desired* position against it every fixed step. The very next line then sets `runtime.connected = false`, so `process_engine_message` returns early (`if !runtime.connected { continue; }`) for every subsequent message — there is no path left that can ever set `law.custody = None`. `hud::clear_transients_on_disconnect` likewise never clears `law_standing`, so the "HELD BY …" line stays on screen.

The disconnect path is reachable in a normal run: `LocalEngine::pump` wraps `engine.poll` in `catch_unwind`, and on a sim panic calls `self.fail(...)` → `BridgeEvent::Disconnected` (src/smart_actors/local_engine.rs:531-537).

**Failure scenario.** An officer has hold of the player (`custody.held == true`, anchor at the officer's grip point). On the next pump the sim panics — e.g. `World::assert_invariants` or one of the many `expect(...)` calls in `engine.rs` — so `LocalEngine` drops the engine and emits `Disconnected`. The drain clears the runtime but leaves `PlayerCustodyState.custody = Some(held)`. From that frame on the player is permanently clamped to a 1.5 m sphere around a now-frozen anchor with no engine left to release them, and the HUD keeps saying they are held; the only escape is the developer flying key, which `tether()` special-cases.


# MEDIUM severity (22 findings)


## crates/cathedral-sim/src/round.rs:7130

### Curfew rung's Stay-at-home short-circuits rung 3's eat-what-you-hold

Rung 5 returns before rung 3 is ever reached for a housed, non-exempt actor standing at their door:

```rust
if night && !escorting && !person.curfew_exempt && let Some(home) = person.home {
    return if position.distance(home) <= HOME_ARRIVE_RADIUS_M {
        (Decision::Stay, None)
```

Rung 3 immediately below documents the invariant it can no longer keep: "Eat what you hold, standing — for anyone, the night trades included, **at any hour**: a famished actor with food in hand always eats it." Meanwhile `decay_needs` keeps subtracting `HUNGER_DECAY_PER_GAME_SECOND` all night and the hearth refill is gated on `is_meal_office` (HighWick/Waning/Lamplight only, line 5758-5763), so `night` (Snuffing 21:00 → Watch 05:00) is eight game hours of pure decay with no mechanical way to eat.

**Failure scenario.** A housed mason buys a loaf at Lamplight, `should_carry` sends him home with it, and he reaches his door before the Snuffing. From 21:00 hunger falls with no hearth refill; if he went to bed below ~205 he crosses HUNGER_FAMISHED (15) in the small hours while holding an edible loaf, and `decide` still returns `(Decision::Stay, None)` every cadence. He shows the computed `famished` condition to any player who wakes him, and only eats once the Kindling lifts the curfew rung.


## crates/cathedral-sim/src/actions.rs:3388

### `release` never tells the person being released: the second-person branch is unreachable

`release` builds its percepts from `nearby(world, &target_id, ...)`, and `nearby` passes the id as `characters_within`'s **exclude** argument (`world.characters_within(origin, radius, Some(actor_id))`), so the prisoner is never in the list:

```rust
    let witnesses = nearby(world, &target_id, HEARING_RADIUS_M);
    let lines: Vec<(ActorId, String)> = witnesses
        .iter()
        .map(|witness| {
            ...
            let line = if witness == &target_id {
                format!("{who} lets you go; you are free to walk away")
```

`witness == &target_id` can never be true, so the only line addressed to the released party is dead code. Every *other* release path deliberately does tell them — `custody::forget_departed` pushes `"you are out of the law's hands: …"` straight onto the prisoner, and its doc says it "[m]irrors the freed loop in `tick_custody`". The correct pattern is the one `take_into_charge` uses: fan out from the *officer* (`nearby(world, officer_id, ..)`), which does include the target.

**Failure scenario.** Sergeant Havise Ashe has Ede Clove in charge and, talked round mid-escort, issues `release {"person": "…"}`. Bystanders get "Havise Ashe lets a stranger go"; Ede gets nothing at all. Her `you_are_held` prompt section (`prompt/mod.rs::held_line`) simply vanishes on her next turn with no event and no line explaining it, so the model has no percept of being freed and keeps answering as a prisoner — while every clock-driven release in the same subsystem states the ending and its reason.


## crates/cathedral-sim/src/actions.rs:3455

### `struggle`'s attempt counter saturates, so every repeat attempt replays one frozen die

The "which attempt this is" seed is the length of a bounded buffer:

```rust
    let attempt = world.characters[actor_id].recent_history().len() as u64;
    let broke = crate::custody::struggle_roll(actor_id, &holders, attempt, chance);
```

`remember_percept` ends with `cap_front(&mut self.state.recent_history, RECENT_HISTORY_MAX_ENTRIES)` and `RECENT_HISTORY_MAX_ENTRIES = 32`, so for any actor who has lived past 32 remembered lines — i.e. essentially the whole cast in a real run — `recent_history().len()` is pinned at 32 forever. `struggle_roll` hashes `("custody_struggle", prisoner, holders, attempt)` and compares the fixed result against `chance`, so with the same holders the boolean is identical on every attempt. (It is worse than it looks: because `announce_struggle` never gives the struggler a line of their own, struggling does not even add to their history, so the seed is frozen for under-cap actors too.)

**Failure scenario.** Ede Clove (32-entry history) is grabbed by one sergeant and calls `struggle`; `hash("custody_struggle"|ede|srgnt|32)` lands above `chance`, so she is held fast. She calls `struggle` again on her next turn, and the next, for the whole walk to the station: identical inputs, identical hash, `held fast` every single time. The escort is a hold that cannot ever be broken by any number of tries, instead of an independent draw per attempt.


## crates/cathedral-sim/src/actions.rs:3522

### `announce_struggle` excludes the struggler, so an NPC gets no record of their own struggle

Same exclusion mistake as `release`:

```rust
    let witnesses = nearby(world, struggler_id, HEARING_RADIUS_M);
    let lines: Vec<(ActorId, String)> = witnesses
        .iter()
        .map(|witness| {
            let who = cap_first(&identify_ids(world, witness, struggler_id));
            let theirs = witness == struggler_id;
```

`nearby` excludes `struggler_id`, so `theirs` is always `false` and the three second-person strings ("You pull against the hands on you.", "You tore free of the law's hands.", "You fought to get free and could not.") can never be delivered. Unlike `say`, `go_to`, `gesture`, `pocket_item`, `spit` and `tell_way` — all of which call `remember_percept` explicitly for the actor — `struggle` has no self-percept path at all, so the actor's own line was clearly meant to come from here.

**Failure scenario.** An NPC prisoner issues `struggle {}` and the roll fails. Bystanders and the holder are told "X fought against the hands holding them, and did not get free"; the struggler's inbox and `recent_history` receive nothing whatsoever. On their next turn the model has no evidence it ever tried, sees only the sheet's unchanged "a hand is on your arm, and struggle is the only way out of that", and re-issues the same verb — which (see the frozen-attempt bug above) fails identically forever.


## crates/cathedral-sim/src/engine.rs:3320

### announce_commitment's hand-release loop is always empty (commit cleared holders)

`announce_commitment` intends to say every hand came off on arrival:

```rust
// `Custody::commit` drops every hand — arriving ends the walk. Say so,
// or the presented arm stays reaching at somebody nobody is holding any
// more: a hold that ends in silence looks exactly like one that did not.
for holder in record.holders.clone() {
    actions::announce_grip(&mut self.world, &holder, prisoner_id, false);
}
```

But `record` is read *after* the commit at both call sites, and `Custody::commit` (custody.rs:405-416) does `record.holders.clear()` before returning `true`. Site 1: `custody::follow_escorts` does `arrived.retain(|prisoner| world.custody.commit(prisoner, now));` and only the survivors reach `tick_movement`'s `for prisoner in escort.committed { self.announce_commitment(now, &prisoner); }`. Site 2: `debug_commit` does `if self.world.custody.commit(&target_id, now) { self.announce_commitment(now, &target_id); … }`. In both cases `record.holders` is already empty, so the loop body never runs.

**Failure scenario.** Havise Ashe takes Ede Clove in charge and `grab`s their arm (holders = [Ashe]; the host records a grip via the `grab` world event → `HandoverFeedback::TookHold`). She walks them to the Stone House; `follow_escorts` commits on arrival, clearing `holders`, and `announce_commitment` then iterates an empty vector. Result: neither Ede nor any of the ~20 m of witnesses is ever told the hand came off (no "Havise Ashe lets go of your arm" percept), and no `let_go` world event is emitted at all — so `GripHolds` in the host (src/smart_actors/hands.rs, cleared only by `HandsOff`) keeps Ashe's arm visually clamped to Ede for the rest of the run.


## crates/cathedral-sim/src/engine.rs:3275

### Engine custody endings emit only `let_go`, which nothing clears the grip on

Every ending `tick_custody` owns announces the hand coming off through `announce_grip(.., false)`:

```rust
// Every hand comes off audibly, whichever clock ended the custody.
for holder in &record.holders {
    actions::announce_grip(&mut self.world, holder, &prisoner_id, false);
}
```

…and the same call is used by the `ungripped` (dead-man) loop at engine.rs:3214 and by `custody::forget_departed`. `announce_grip` emits the world-event kind `"let_go"` (actions.rs:3600), but the sim never emits a `"release"` event on these paths — `Custody::release` is a pure data operation, and only the `release` *verb* (actions.rs:3410) emits `"release"`. The host's world-event match (src/smart_actors/mod.rs:1291-1309) handles `"grab"`, `"release"` and `"broke_free"` and drops `"let_go"` into `_ => {}`; `GripHolds::hands_off` is reachable only from `"release"`/`"broke_free"`. For an NPC prisoner there is no other channel (only the *player's* custody rides `LawStanding`).

**Failure scenario.** A sergeant grabs a pickpocket and then, because the LLM lane is saturated, takes no turn for 60 s: clause 1 fires, `release_grip` empties `holders`, and a `let_go` event goes out. The host ignores it, so the sergeant's arm stays drawn reaching for the pickpocket's arm while both walk away in different directions. The same happens on the station-cap release ("the keeper wants the room", 240 s), on the sentence release, and on the `separation > 20 m` "parted" release — none of which emits `release` or `broke_free`.


## crates/cathedral-sim/src/engine.rs:2742

### ramp_urgency deletes the debug-written `urgency` status in the same poll it is set

`digest(now)` runs unconditionally every poll over every character, and its second pass owns the whole `Urgency` key:

```rust
if !carries_a_stool {
    actor.state.urgency_since_game_days = None;
    return actor.state.statuses.remove(&StatusKind::Urgency).is_some();
}
```

It removes `StatusKind::Urgency` from anyone not carrying a `poop` stack in a pocket — including a value just written by `EngineCommand::DebugSetStatus` → `World::debug_set_status`. In `Engine::poll` the commands are applied at line 1204 and `self.digest(now)` runs at line 1362, so the poke is wiped before the poll's final `flush`. This contradicts the documented contract of the poke (`.claude/rules/CATHEDRAL_DRIVE.md`: "Kinds are `drunkenness`, `weariness` and `urgency`") and of the headless flag (cathedral_headless.rs:149-151: "The sim writes `urgency` itself on the poop clock … this only forces it").

**Failure scenario.** `CATHEDRAL_DRIVE='status Ilse urgency 1; frame Ilse'` (or `cathedral-headless --status Ilse=urgency:1`): `debug_set_status` inserts `Urgency → 1.0`, bumps the revision and flushes a snapshot carrying it; ~1 ms later, still inside the same `poll`, `ramp_urgency` finds Ilse carries no stool, removes the key and touches public state again, so the final snapshot of that poll — and every one after — has no urgency at all. The clenched, quickened walk the developer asked to eyeball never renders, and no diagnostic explains why.


## crates/cathedral-sim/src/scheduler.rs:812

### An actor queued in both the player-reaction and priority lanes takes two turns

`select_next_actor` pops from one lane only and never removes the actor from the other:

```rust
if let Some(actor_id) = self.player_reactions.pop_front() {
    return Some((actor_id, true));
}
...
if let Some(actor_id) = self.priority_handoffs.pop_front() {
    return Some((actor_id, false));
}
```

Both `prioritize` and `prioritize_player_reaction` de-duplicate only *within* their own `VecDeque` (`if !self.priority_handoffs.contains(actor_id)` / `if !self.player_reactions.contains(actor_id)`), and nothing in `submit_next_turn` or `apply_reply` clears the other lane. This contradicts the stated invariant on `prioritize`: "An actor already queued is not queued twice: their one turn answers everything that has reached them, because the render drains the whole inbox." The two lanes were split so a background handoff cannot *erase* the player's listener, not so the same actor is scheduled twice.

**Failure scenario.** The player retrieves an item from a pocket in plain sight of Ilse: `Engine::nudge_pocket_witness` calls `scheduler.prioritize(&world, ilse, true, now)`, putting Ilse in `priority_handoffs`. A second later the player speaks ("sorry about that"); the speech router calls `prioritize_player_reaction(&world, ilse, now)`, putting Ilse in `player_reactions` too. Ilse pops off `player_reactions`, `render_prompt_and_drain` empties her whole inbox (both the pocket percept and the player's line) and she answers. On the next selection she pops off `priority_handoffs` and a second full provider call goes out with an empty `since_your_last_turn` — a paid-for, contentless turn that can also make her speak again unprompted.


## crates/cathedral-sim/src/custody.rs:414

### commit clears holders before the arrival announcement reads them

`Custody::commit` drops every hand as part of the state change:

```rust
record.state = Confinement::Committed;
record.committed_at = Some(now);
record.holders.clear();
```

Unlike `Custody::release` and `Custody::forget`, which deliberately hand the record back so "the caller can say what ended and to whom", `commit` returns a bare `bool` and discards the holder list. Both callers commit *first* and read the holders *after*: `follow_escorts` does `arrived.retain(|prisoner| world.custody.commit(prisoner, now));` and the engine then runs `announce_commitment`, whose whole point is

```rust
// `Custody::commit` drops every hand - arriving ends the walk. Say so, or
// the presented arm stays reaching at somebody nobody is holding any more
for holder in record.holders.clone() {
    actions::announce_grip(&mut self.world, &holder, prisoner_id, false);
}
```

(`engine.rs:3317-3322`; `engine.rs:2225` takes the same order for the drive-mode `commit`). By then `record.holders` is always empty, so the loop is dead code.

**Failure scenario.** Havise Ashe grabs Ede Clove after she strays (`holders = [ashe]`) and walks her the last stretch to the Tallage toll-house. `follow_escorts` commits her on arrival, clearing `holders`; `announce_commitment` then iterates an empty list, so no `announce_grip(.., false)` runs: Ede never gets "Havise lets go of your arm", nobody within `HEARING_RADIUS_M` hears "Havise lets go of Ede", and no `let_go` domain event is emitted — a hold that ended in exactly the silence the surrounding code is written to prevent (the `forget_departed` path is pinned against this in `custody_tests.rs:1407-1413`; the commit path is not).


## crates/cathedral-sim/src/weather.rs:1073

### Forced weather's 8-hour elapsed cap freezes the wet aftermath forever

`forced_sample` clamps the elapsed time used for BOTH the new accumulation and the inherited decay:

```rust
let elapsed_hours = forced.began_at_days.map_or(1.0, |began| {
    ((time - began).max(0.0) * HOURS_PER_DAY).min(8.0)
});
...
let inherited_wetness = forced.initial_wetness * (-drying_per_hour * elapsed_hours).exp();
...
let inherited_standing = forced.initial_standing_water * (-0.62 * elapsed_hours).exp();
```

Once 8 game hours have passed, `elapsed_hours` saturates, so the inherited wetness/standing water never decay any further no matter how much game time elapses. Worse, `drying_per_hour` is recomputed from the current clock (same `daylight < 0.08` cliff as `accumulated_water`), so instead of drying, the inherited wetness oscillates with the day/night cycle in perpetuity.

**Failure scenario.** Drive/dev override path: `set_override(Downpour, Some(0.95), day 8 08:00)` then `set_override(Clear, None, day 8 09:00)`. At +9 h `surface_wetness` is 0.0250; at +12 h (21:00) it is back to 0.6975; it is still exactly 0.6975 at +36 h and returns to it every night through +168 h (a full game week of forced Clear sky). `standing_water` stays pinned at 0.0070 forever. One `weather downpour` command therefore leaves the city permanently re-soaking every night until the process restarts.


## crates/cathedral-sim/src/nav/mod.rs:590

### offset_route validates only vertices, so lane paths run through walls for up to 10 m

`offset_route`'s doc promises "Every shifted vertex is validated against the walkable bitset (halving the shift until it lands on ground), so the lane can never put a body inside a wall", but only the polyline *vertices* are tested:

```rust
let mut point = points[i];
for _ in 0..3 {
    let candidate = points[i] + right * offset;
    if self.is_walkable(candidate.x, candidate.z) {
        point = candidate;
        break;
    }
    offset *= 0.5;
}
```

The consumer (`World::advance_movers`, world.rs ~658-767) interpolates linearly between consecutive path points and commits `new_pos = tentative` with no walkability check at all (only the on-stage separation push at world.rs:810 is bitset-clamped). A graph edge can be tens of metres long while `half_width_m` describes only its widest part, so the corridor pinches between the two validated endpoints and the entire shifted segment lies inside a building.

**Failure scenario.** Shipped `assets/world/navigation.{json,bin}`, edge 232->233: 38.75 m long, `half_width_m` 0.60, so `usable` = 0.60 - 0.35 = 0.25 m. Both endpoints pass `is_walkable` at a 0.125 m shift, yet 34 of 157 samples along the shifted segment are off-surface — a ~7.4 m contiguous run inside a wall, while the unshifted centreline is 100% walkable. Sweeping every named-place pair in both directions with the real production lanes (0.1 / 0.4 / 0.7, the range `world::lane_fraction` produces), 12,691 of 96,567 route segments contain off-surface stretches; 3,733 have contiguous runs >= 1 m and the worst is 10.54 m (edge 301->302, lane 0.7). An NPC walking that leg visibly passes straight through a building.


## crates/cathedral-backends/src/worker.rs:362

### Worker::forget kills the child but never reaps it: a zombie per worker failure

`forget` removes the child from `self.child` and SIGTERMs it, then does a single non-blocking poll:

```rust
let child = self.child.lock().expect("worker child lock").take();
if let Some(mut child) = child
    && !matches!(child.try_wait(), Ok(Some(_)))
{
    terminate(&child);
    // Reaped by the OS; we do not wait, because forgetting happens on
    // the request path and a wedged child must not hold a turn.
    let _ = child.try_wait();
}
```

The comment's premise is wrong on unix: a terminated child is reaped by its *parent*, not by the OS, and `std::process::Child` has no `Drop` that waits. `terminate` only sends SIGTERM, so the `try_wait()` on the very next line almost always returns `Ok(None)` — the child has had microseconds to die. The `Child` is then dropped at the end of the `if let`, and because `forget` already `take()`-ed it out of `self.child`, nothing can ever wait on it again: `close()` (the only path that calls `child.wait()`) and `ensure`'s `slot.replace(child)` reaping both look at `self.child`, which is now `None`.

**Failure scenario.** Local Pocket TTS is the selected voice backend but the model cannot load (no CUDA, or `HF_TOKEN` missing so the download fails). Every NPC line goes `TtsEngine` -> `PocketTts::synthesize_stream` -> `Worker::request` -> `ensure`, which spawns `uv`, reads a handshake of `{"type":"fatal","error":"..."}`, and calls `self.forget(io)`. The mismatched-request-id and invalid-JSON poison paths are worse, because there the child is definitely still alive when SIGTERM is sent, so `try_wait()` is guaranteed to return `Ok(None)`. A play session with a few hundred spoken lines leaves a few hundred unreaped `uv` processes parented to the game, consuming pid slots until the game exits.


## src/smart_actors/mod.rs:1252

### Custody sounds placed from the cold snapshot, which lacks a mover's live position

The two custody cues read the actor's position out of `WorldMirror`:

```rust
if kind == "seize"
    && let Some(officer) = mirror.actor(&actor)
{
    presentation.soundscape.write(crate::soundscape::SoundscapeCue::CustodyKeys {
        position: officer.position_m.into(),
    });
}
...
if kind == "commit"
    && let Some(prisoner) = mirror.actor(&actor)
{
    presentation.soundscape.write(crate::soundscape::SoundscapeCue::GaolDoor {
        position: prisoner.position_m.into(),
    });
}
```

`ActorSnapshot::position_m` is the *cold* channel. `World::step_movement` (crates/cathedral-sim/src/world.rs:556-823) writes `character.state.position_m` and never calls `touch_public_state()` — that is the documented hot/cold split, and it means a walker's snapshot position is only refreshed when some *unrelated* change bumps `world_revision`. On top of that, `Engine::flush` pushes world events before `EngineMessage::Snapshot`, so when this arm runs the mirror is still one revision behind the seizure/commit that just happened. The live pose is sitting in `MovementInbox` (written a few lines above in the `Movement` arm) and is not consulted. Both cues are genuinely positional — `soundscape.rs:1520-1541` schedules `GatekeeperKeyRing` / `StoneHouseCellDoor` at exactly this `Vec3`.

**Failure scenario.** Sergeant Havise Ashe walks her beat for ~15 s while nothing else bumps the world revision (2.1 m/s => ~30 m of travel invisible to the snapshot), then seizes the player. The `seize` world event is handled against the previous revision, so the key rattle is scheduled ~30 m up the street instead of at the player's elbow — outside the clip's audible falloff, so the player hears nothing. The drive-mode case is worse: `Engine::debug_seize` (crates/cathedral-sim/src/engine.rs:2116-2127) teleports the officer beside the target inside the same poll, so `CustodyKeys` fires at wherever the officer stood before the teleport — potentially hundreds of metres away. Likewise `commit`: `debug_commit` (engine.rs:2213) sets the prisoner to `gaol.point` in the same poll, so the Stone House door slams at the prisoner's old street position rather than at the gaol.


## src/smart_actors/actors.rs:235

### MovementInbox is never pruned, so a stale pose overrides an authoritative reposition

`drive_npc_bodies` unconditionally re-asserts the last movement sample over whatever `reconcile_actor_views` wrote from the snapshot:

```rust
let Some(sample) = inbox.0.get(actor_id) else {
    continue;
};
...
let t = ((now - motion.t0) / MOVEMENT_TICK_SECONDS).clamp(0.0, 1.0) as f32;
let translation = motion.previous.lerp(motion.current, t);
...
if transform.translation != translation || transform.rotation != rotation {
    transform.translation = translation;
    transform.rotation = rotation;
}
```

Nothing ever removes an entry from `MovementInbox` (`model.rs:70`) — not when the actor leaves the mirror and its `ActorView` root is despawned in `reconcile_actor_views`, and not when the sim moves an actor by a path that emits no `Movement` tick. The comment at line 265 acknowledges "an arrived walker keeps its stale sample forever", but the code then trusts that sample even when the authoritative snapshot has since put the actor somewhere else. Because `t` clamps to `1.0`, the body is pinned at `motion.current` — the pre-teleport position — and because `reconcile_actor_views` is gated on `mirror.is_changed()` it never gets a second chance to correct it.

**Failure scenario.** Run the documented drive script `seize Ede Clove; sleep 2; commit; shot cell`. During the 2 s escort the officer produces movement samples, so her `MovementInbox` entry and `NpcMotion` track the street walk. `Engine::debug_commit` (crates/cathedral-sim/src/engine.rs:2210-2217) then sets `character.state.position_m = gaol.point; movement = None; intent = None` and bumps the revision. The host applies the snapshot, `reconcile_actor_views` places her at the Stone House — and `drive_npc_bodies` immediately writes her back to the last escort sample out on the street, where she stays until the round happens to give her a new errand, so `shot cell` captures an empty cell with no keeper. The same mechanism bites production code: a road-party carrier who departs (`transition_presence` -> BeyondTheWalls) and later re-enters at `party.gate_point` (round.rs:1608-1630) gets a fresh entity whose first `NpcMotion` is built as `previous = gate spawn, current = <stale pre-departure sample>`, so the body slides off the entry point and stands in the wrong place until the sim next moves them.


## src/smart_actors/body.rs:2102

### Urgency multiplies the absolute accumulated gait phase, snapping the legs

`apply_locomotion` scales the sim's running phase accumulator instead of its rate:

```rust
let cycle = (gait_phase * urgent_cadence(carriage.urgency)
    + carriage.drunkenness * drunk_phase_wobble(now, seed))
    * TAU;
```

`gait_phase` is not a 0..1 phase — the sim accumulates it without bound (`world.rs:766` `movement.gait_phase += movement.speed * dt * GAIT_CADENCE`, and `round.rs:7975` deliberately carries it across route legs: "Never reset, so the gait is seamless"). Multiplying an accumulated value by a *changing* factor `k = 1 + 0.4·u` adds a spurious phase offset of `gait_phase · Δk` every time `u` changes, on top of the intended rate change. The drunk term next to it is correct (a bounded additive offset); this one is not. `Carriage::urgency` comes straight from `engine.rs:2753`, which quantizes to sixteenths, so `u` steps by 0.0625 → `Δk = 0.025`, sixteen times as the poop clock ramps.

**Failure scenario.** An NPC carrying a stool has been walking one continuous route for ~150 m, so `gait_phase ≈ 100` cycles. `ramp_urgency` bumps urgency from 0.0625 to 0.125; `urgent_cadence` goes 1.025 → 1.05, so `cycle` jumps by 100 × 0.025 = 2.5 cycles in one frame. Thighs, shins and arms teleport to an unrelated point of the stride (half a cycle swaps which leg is forward) — a visible leg pop, repeated at each of the sixteen urgency steps. The same jump fires instantly for the documented dev tool `CATHEDRAL_DRIVE='status Ilse urgency 1'`, which is the exact command meant to let you eyeball this pose.


## src/smart_actors/body.rs:2649

### Gait-phase interpolation sweeps backwards when the sim resets the phase to 0

The two-sample gait history assumes `gait_phase` only ever increases, and guards a discontinuity purely by elapsed time:

```rust
let stale = now - history.t0 > SAMPLE_STALE_SECONDS;
history.prev_phase = if stale { sample.gait_phase } else { history.cur_phase };
...
gait_phase = history.prev_phase + (history.cur_phase - history.prev_phase) * t;
```

But the sim *does* reset the phase: `round.rs:7975`'s `set_route` reads the old phase with `map_or(0.0, |movement| movement.gait_phase)`, so any route laid while `state.movement` is `None` starts again at 0. Several sites clear `movement = None` and then explicitly re-decide immediately (`round.rs:3641/3647`, followed by `person.next_decision = 0.0` — "Re-decide at once rather than waiting out the cadence"). When the new route lands inside `SAMPLE_STALE_SECONDS` (0.18 s), `stale` is false and the lerp runs from the old large phase down to ~0 instead of snapping.

**Failure scenario.** An NPC abandons a well errand mid-walk after 120 m (`gait_phase ≈ 80`); `movement` is cleared and the ladder lays a new route on the next tick (~16 ms later), so the next 20 Hz sample carries `gait_phase = 0.02` about 50 ms after the last one. `stale` is false, so `prev_phase = 80`, `cur_phase = 0.02`, and over one 50 ms tick window the interpolated phase sweeps −80 cycles: legs and arms whirl through ~1600 cycles/s for several frames before settling — exactly the thrash the `stale` branch was written to prevent.


## src/city/mod.rs:7338

### Wall-tower collider is the AABB of a 45-degree-rotated square: twice the visible footprint

The tower is drawn as a 12 m box rotated 45° (a diamond, half-diagonal 8.485), but the collider is the circumscribing axis-aligned box:

```rust
let half = 12.0 * 2.0_f32.sqrt() * 0.5;
collision_world.add_box(
    Vec3::new(point.x - half, 0.0, point.y - half),
    Vec3::new(point.x + half, height + 5.7, point.y + half),
);
```

`half = 8.485` is the diamond's half-diagonal, so the box is 16.97 × 16.97 = 288 m² against the tower's true 144 m² footprint — exactly double. Buildings in this same file get exact geometry through `add_footprint_colliders` → `CollisionWorld::add_convex_prism`; the towers do not.

**Failure scenario.** For the tower at (14.0, -468.3), the point (7.0, -462.0) has |dx| + |dz| = 13.3 > 8.485, so it is well clear of the visible masonry — open ground to the eye — yet it sits inside the AABB and is solid from y 0 to height+5.7. Walking past any of the 28 towers along a diagonal face, the player stops against nothing; `assets/world/collision_footprints.json` carries 29 such 16.97 × 16.97 squares, so the nav bake also erodes the phantom corners out of the walkable set (this is what reduces the Harne Gate throat from ~4.3 m of genuinely clear arch to 1.25 m).


## src/scene.rs:425

### Outer aisle wall split does not line up with the transept: 3 m hole in the cathedral shell

The aisle walls at x=±44 are split for the transept opening:

```rust
for (center_z, length) in [(34.0, 88.0), (-69.5, 53.0)] {
```

Those spans are z∈[-10, 78] and z∈[-96, -43]. But the transept is z∈[-39, -7] everywhere else: the end walls are `Vec3::new(side * 67.0, 11.0, -23.0)` with length 32, the connecting walls are `for z in [-39.0, -7.0]` (2 m thick, so [-40,-38] and [-8,-6]), and the crossing piers are at `for z in [-39.0, -7.0]`. The opening is therefore mis-centred: it starts 4 m too far south and stops 3 m too far south. Both the mesh and the matching collider (`collision_world.add_box(Vec3::new(side*44.0 - 1.0, 0.0, center_z - length*0.5), ...)`) share the mistake, so nothing plugs z∈(-43,-40) at x=±44.

**Failure scenario.** Walk down the south aisle to (x=43, y=1, z=-41.5) and continue toward +x: no collider covers x∈[43,45] at that z (the aisle wall collider stops at z=-43, the transept south-wall collider starts at z=-40), so the player walks straight out through an 18 m tall, 3 m wide hole in the cathedral's outer shell onto the city ground — and sees daylight through it from inside the aisle. The same hole exists at x=-44. Symmetrically, wall1 covering z∈[-10,-8] leaves a 2 m stub of 18 m-tall aisle wall standing inside the north edge of the transept arch.


## src/scene.rs:983

### Baldachin's two front columns float 0.54 m above the altar steps

All four baldachin columns are footed at y=1.1, the height of the top altar step:

```rust
for x in [-4.4, 4.4] {
    for z in [-85.5, -78.2] {
        spawn_compound_column(commands, mesh, material, Vec3::new(x, 1.1, z), 0.62, 10.0);
```

`spawn_compound_column` treats `foot` as the bottom of the base plinth (`foot + Vec3::Y * 0.35 * scale` with size `Vec3::new(2.7, 0.7, 2.7) * scale` spans y∈[foot.y, foot.y+0.7*scale]). But the steps shrink *and* shift south as they rise: step i has depth `12.0 - i*1.55` centred at `-82.5 - i*0.35`, giving top surfaces of 0.28 over z∈[-88.5,-76.5], 0.56 over [-88.075,-77.625], 0.84 over [-87.65,-78.75] and 1.12 over [-87.225,-79.875]. z=-78.2 is only covered by steps 0 and 1.

**Failure scenario.** The rear pair at z=-85.5 correctly rests on the 1.12 m top platform, but the front pair at (±4.4, -78.2) stands over step 1, whose surface is at y=0.56. Their 1.67 m square stone plinths hang 0.54 m in mid-air; looking at the high altar from the choir you see a visible gap and shadow under both front columns of the baldachin.


## src/soundscape.rs:1597

### Daily curfew peal never claims the Scold's cooldown, so a summons rings on top of it

`schedule_curfew_bell` queues nine `ScoldStroke`s straight into `ScheduledSounds` without ever touching `CueCooldowns`:

```rust
    state.curfew_day = Some(clock.day);
    // Wait out the Lanthorn's seven strokes, then the grace, then the law.
    let plan = schedule_bell_pattern(
        BellPattern::ScoldCurfew,
        now + office_bell_span_seconds(Office::Snuffing) + DUSK_GRACE_SECONDS,
        &format!("curfew:{}", clock.day),
        &mut scheduled,
    );
```

The system's parameter list is `(time, clock, state: ResMut<CivicBellState>, scheduled: ResMut<ScheduledSounds>)` - there is no `CueCooldowns` at all. The *other* path into the same bell does record occupancy, and its comment states the invariant it protects (lines 1550-1556): "One peal per bell at a time: a second summons on top of a ringing one would make the strokes uncountable, which is the one thing these bells may never be." Because the curfew never inserts an entry under `stable_hash("civic-bell:38")`, `cooldowns.allow(key, now, occupies)` in `ingest_soundscape_cues` sees no prior peal and approves. The key is deliberately per-bell (`plan.sound as u8`, the same `ScoldStroke` for both patterns), so the guard was clearly meant to cover both paths.

**Failure scenario.** At 21:00 the office edges into Snuffing; `schedule_curfew_bell` queues nine strokes at `now+35.04 + k*3.0` (last at `now+59.04`) from SCOLD_TOWER and registers nothing. Within that ~59 s an officer takes the `summon` action (law_and_order M4a), so `drain_bridge_messages` writes `SoundscapeCue::CivicBell(BellPattern::ScoldSummons)`; `ingest_soundscape_cues` finds no cooldown entry for `civic-bell:38` and queues five more `ScoldStroke`s at 1.15 s spacing from the same tower. The player anywhere in the eastern city hears one bell ring 14 strokes at two interleaved tempos and cannot read the curfew count - precisely the failure the cooldown exists to prevent.


## src/drive.rs:109

### drive `frame` ignores the camera's 0.65 m eye offset, aiming shots 0.65 m too high

`frame_view` computes a *camera* pose:

```rust
let position = actor.translation + away * distance + Vec3::Y * FRAME_EYE_HEIGHT_M;
let target = actor.translation + Vec3::Y * FRAME_LOOK_AT_HEIGHT_M;
let look = target - position;
let yaw = (-look.x).atan2(-look.z);
let pitch = look.y.atan2(look.xz().length());
```

but the value is handed to `TeleportPlayer { position, .. }` (drive.rs:1026), and `apply_teleports` (controller.rs:601) writes it to the **player body** transform. The camera is a child spawned once at `Transform::from_xyz(0.0, EYE_OFFSET, 0.0)` with `EYE_OFFSET = 0.65` (controller.rs:46, 533), and `apply_teleports` never touches the camera's translation — it only sets `camera.rotation`. So the eye ends up at `actor.y + FRAME_EYE_HEIGHT_M + 0.65`, while the pitch was solved for an eye at `actor.y + FRAME_EYE_HEIGHT_M`. Both documented intents ("Camera height above the actor's root … so the shot is level rather than looking down on the crown", "the sternum, which puts head and feet symmetrically in frame") are violated. The unit test `frame_stands_in_front_of_the_actor_and_looks_back` only exercises `frame_view` in isolation, so it cannot catch this.

**Failure scenario.** `CATHEDRAL_DRIVE='frame Ilse; shot ilse; quit'`. Ilse's root is at WALK_Y = 0.91 and her puppet is 1.71 m tall (sole at world y 0.00, crown at 1.71 — body.rs:44-57). `frame_view` returns position.y = 1.43 and pitch = atan2(1.21 − 1.43, 2.6) = −4.84°. The real camera lands at y = 1.43 + 0.65 = 2.08, i.e. 0.37 m above her crown, still pitched only −4.84°. With the 70° vertical FOV (controller.rs:510), the frame at her plane spans y ≈ [0.03, 3.69] centred at 1.86: her soles are clipped off the bottom edge and the body occupies only the lower ~45 % of the shot, the rest sky — the opposite of the "level, head-and-feet-symmetric" portrait the constants were written for.


## src/smart_actors/mod.rs:1300

### `let_go` world event is never projected — the visible custody grip is never released

The `EngineMessage::WorldEvent` arm only maps three custody kinds onto the grip presentation:

```rust
("grab", Some(prisoner), _) => { ... HandoverFeedback::TookHold { holder: actor, prisoner } }
("release", Some(prisoner), _) => { ... HandoverFeedback::HandsOff { prisoner } }
("broke_free", _, _) => { ... HandoverFeedback::HandsOff { prisoner: actor } }
_ => {}
```

But the sim emits a **fourth** kind for a hand coming off an arm. `actions::announce_grip(world, holder, prisoner, taken)` emits `if taken { "grab" } else { "let_go" }` (crates/cathedral-sim/src/actions.rs:3600), and `"release"` is only ever the LLM verb. Every non-verb grip release goes out as `let_go`: the dead-man timer (`engine.rs:3214`, the `ungripped` loop), the `freed` sweep (`engine.rs:3276`), the station cap/departed-escort release (`custody.rs:650`), and arrival at a station (`engine.rs:3321`, whose own comment says *"Say so, or the presented arm stays reaching at somebody nobody is holding any more"*). `let_go` appears nowhere in `src/` — the host's `_ => {}` swallows it, so `GripHolds` keeps the holder→prisoner entry and `hands::hold_the_seized` keeps drawing the arm.

The only backstop is the distance check `holder_at.distance(held_at) <= GRIP_BREAKS_AT_M` (`CUSTODY_REACH_M + 1.0` = 4.0 m), which does not fire for an escort, because `custody::follow_escorts` teleports an NPC prisoner to `CUSTODY_ESCORT_CONTACT_M` = 1.5 m behind the officer's shoulder every tick.

**Failure scenario.** A sergeant runs `grab` on an NPC thief → `TookHold` → the officer's arm is posed on the thief's upper arm. With one LLM turn in flight across ~500 NPCs the officer is not handed a turn for 60 s, so `tick_custody` clause 1 pushes the pair into `ungripped`, calls `custody.release_grip()` and `announce_grip(..., false)` → a `let_go` world event. The sim now has `record.holders` empty (merely in charge) and keeps escorting. The host never receives a `HandsOff`, and `follow_escorts` holds the two 1.5 m apart, so the 4 m grip-break backstop never trips: the officer's arm stays clamped on the thief's arm for the rest of the escort, and the same happens on arrival at the Stone House (all hands are dropped by `Custody::commit`, announced only as `let_go`).


# LOW severity (11 findings)


## crates/cathedral-sim/src/round.rs:6058

### service_stalls never checks custody: a marched-off prisoner still completes a sale

The stall service loop prunes only on presence and starts serving the queue head unconditionally:

```rust
if round.stalls[s].serving.is_none()
    && let Some(front) = round.stalls[s].queue.first().cloned()
{
    round.stalls[s].serving = Some((front, now + PURCHASE_SECONDS));
}
```

There is no `world.custody.holds(...)` test anywhere in `service_stalls` or `try_purchase`, and `World::market_sale` has no proximity check either (inventory.rs:1261+). Compare the water twin, which does handle it — `service_sources` stands a finished drawer down with `if in_conversation.contains(&drawer) || world.custody.holds(&drawer)` (line 6571). Nothing removes a seized buyer from a stall queue: `abandon_bodily_errands` is only invoked for the *escort* (round.rs:6705, engine.rs:2142), never the prisoner.

**Failure scenario.** An NPC is standing in the Wickmarket bread queue (`FoodPhase::Queued`) when an officer seizes them. `custody::follow_escorts` teleports them a pace behind the officer and walks them toward the Stone House, but their queue slot survives; four seconds later `try_purchase` fires and `world.market_sale` moves a spark from the prisoner to Averil and a loaf back — a completed hand-to-hand sale between two people now hundreds of metres apart — followed by `FoodPhase::Eating` and a `silent_eat` on the road.


## crates/cathedral-sim/src/round.rs:2032

### A road member left behind by the law is dropped from the party and never enrolled

On departure the held members are stripped from the roster:

```rust
party.members.retain(|member| !staying.contains(member));
party.state.phase = PartyPhase::BeyondTheWalls;
```

The comment two lines above asserts "a life in the city is theirs the moment the law is done with them", but nothing gives them one. `Round::seed` is one-shot (`if self.seeded { return }`, line 2429) and deliberately excluded road members from `people`, so the left-behind actor has no legs, no home, no base, no ladder cadence — `run_ladder`, `decay_needs`, `tick_intents` and `census` all iterate `round.people` and skip them. Their `state.economic_class` stays `EconomicClass::RoadParty` (excluded from `settle_households`) and their `state.daily_round` still lists the departed party's legs (set at line 1516).

**Failure scenario.** An officer seizes carter Rowan while his party is Returning; the party departs without him (`road_left_behind` logged) and he is retained out of `party.members`. Custody later releases him at the Stone House. From that moment he stands on the same paving stone for the rest of the run — no round rung, no wander, no curfew walk — while his prompt sheet keeps telling him "at Dayspring: trade at Seven Lofts", a leg no system will ever walk him to.


## crates/cathedral-sim/src/actions.rs:3141

### `seize` with an explicit notice_id discards the second door instead of testing it

The two authority doors are collapsed with `or_else` and only *then* matched against the officer's explicit `notice_id`:

```rust
    let authority = world
        .notices
        .warrant_against(&target_id)
        .or_else(|| {
            world
                .notices
                .fresh_own_notice(actor_id, &target_id, game_days)
        })
        .filter(|notice| named.is_none_or(|named| named == notice.id));
```

`or_else` short-circuits: once `warrant_against` yields a notice, `fresh_own_notice` is never consulted, and the `filter` can then only reject. The named notice is therefore never checked against the second door, even when it passes it perfectly. The filter needs to be applied to each candidate (e.g. filter the warrant, then `or_else` the fresh own notice, then filter that too).

**Failure scenario.** Sergeant Ashe watches Tamrd spit at a neighbour; `raise_ward_notice_for` raises notice 7 naming her as raiser. An older word against Tamrd (notice 3) has already ripened into a warrant. Ashe says her piece and calls `seize {"person": "tamrd", "notice_id": 7}` — the notice she is actually acting on. `warrant_against` returns notice 3, the filter sees 7 != 3 and drops it, and the officer is refused with `no_warrant` ("you may take someone only on a warrant, or on a wrong you yourself put to the ward within the hour") — a refusal that is false on both counts, and one that omitting `notice_id` entirely would not have produced.


## crates/cathedral-sim/src/prompt/mod.rs:989

### Notice cap can drop the wronged actor's own notice while settle_notice is offered

`build_sheet` caps the notice list *after* filtering, keeping only the newest four the actor carries:

```rust
word_in_the_ward: world.notices.live().iter().rev()
    .filter(|notice| crate::notices::carries(world, actor.id(), notice.id))
    .take(crate::notices::NOTICES_SHEET_MAX)
```

but the verb gate does not respect that cap:

```rust
let has_settle_verb = has_law_verbs
    || world.notices.live().iter()
        .any(|notice| notice.wronged.as_ref() == Some(actor_id));
```

The comment above it asserts the invariant being broken: *"They always carry that notice, so the number the verb takes is on their sheet."* `carries()` guarantees they carry it, but `.take(NOTICES_SHEET_MAX)` still cuts it, because the cap keeps the newest by id, not the ones the actor has standing in. `NOTICES_MAX_LIVE` is 8 and `NOTICES_SHEET_MAX` is 4, so four newer carried notices are enough.

**Failure scenario.** A non-law resident is named `wronged` on notice 3 (a spark taken from them). Over the next hours four more notices (ids 5–8) are raised and their curiosity roll makes them a carrier of all four. `word_in_the_ward` then renders notices 8, 7, 6, 5 and drops notice 3, while `has_settle_verb` stays true — so `turn.j2` prints "A notice in word_in_the_ward names a wrong done to you … settle_notice with that notice's number" against a sheet containing no such notice. The model can only guess a number (settling someone else's word, which fails) or ignore the verb, so the wrong done to them can never be forgiven by the one person entitled to forgive it.


## crates/cathedral-sim/src/night.rs:744

### A rejected ward set_round still permanently teaches the Minor the way

`ward_set_round` writes the place into the target's `places_known` *before* the leg number is validated, and does not undo it when the edit is refused:

```rust
world
    .characters
    .get_mut(&person)
    .expect("checked by ward_minors")
    .state
    .places_known
    .insert(place_id.clone());
set_round_leg(world, &person, &leg, &place_id).map_err(|error| error.message)
```

`set_round_leg` can still fail after this point — `"you keep no standing round to change"` when `daily_round` is empty, and `"leg must be one of the N leg numbers in your_round"` for any leg outside `1..=legs` (including a non-integer `leg`, since `Value::as_u64()` returns `None`). The caller only turns that into a diagnostic; the `places_known` insert is never rolled back, so a refused action has permanently mutated world state the whitelist elsewhere is careful to guard (`tell_way` is the only sanctioned way to learn a route).

**Failure scenario.** A ward batch replies `set_round {"person": "mnr01", "leg": 7, "place_id": "pl_bbbb"}` for a Minor whose `daily_round` has one leg (as in `world_with_cast`). `ward_minors` admits mnr01, `world.places` holds `pl_bbbb`, so `pl_bbbb` is inserted into mnr01's `places_known`; `set_round_leg` then errors with "leg must be one of the 1 leg numbers" and only a `[night] weigh ward: set_round failed: …` diagnostic is emitted. `round_edit` is correctly left unset, but from the next morning on mnr01's prompt lists The Hungry Ox under `places_you_know` and they can `go_to pl_bbbb` — a route nobody ever told them.


## src/smart_actors/hands.rs:628

### One out-of-range holder takes every other officer's hand off the prisoner

The break check is per *pair*, but the repair is per *prisoner*:

```rust
// Out of reach, or one of them is no longer rendered: either way
// there is no arm left to draw.
_ => broken.push(prisoner.clone()),
...
for prisoner in broken {
    grips.hands_off(&prisoner);
}
```

`hands_off` is documented as "Every hand off one person" (`retain(|_, held| held != prisoner)`), so it evicts *all* holders of that prisoner, not just the pair that drifted apart. Custody is refcounted per holder sim-side (`CustodyRecord::holders`, and `CustodyView::holder_ids` is a `Vec`), so more than one hand on one person is a modelled state. The entry is also gone for good — `GripHolds` is only ever refilled by a fresh `grab` world event, which the sim will not re-emit for a hold it still considers live.

**Failure scenario.** Two sergeants `grab` the same runner. One is nudged aside by the separation pass and ends up 4.2 m away (past `GRIP_BREAKS_AT_M`) for a single frame; that pair pushes the prisoner onto `broken`, and `hands_off` then deletes the *second* sergeant's entry too even though he is still standing 0.9 m away. Both arms drop to rest and never come back for the rest of the escort, while the sim still has the runner in custody and the HUD still says so.


## src/city/mod.rs:5067

### Firewood rick stacks its two columns along the log axis instead of across it

In `build_street_props`, the firewood variant offsets the second column along `direction` — the same axis the 1.05 m logs are laid on:

```rust
for row in 0..3 {
    for column in 0..2 {
        let log2 = position2
            + direction * (column as f32 * 0.3 - 0.15)
            + normal * 0.05;
        add_log(&mut dark_wood, Vec3::new(log2.x, 0.14 + row as f32 * 0.24, log2.y), 0.115, 1.05, direction);
    }
}
```

`add_log`'s `along` argument is `direction`, so a 0.3 m step along `direction` slides a 1.05 m log along its own length. Rows correctly step 0.24 m vertically (log diameter 0.23) and `normal` is used only as a fixed 0.05 m push off the wall — the depth axis the column offset clearly wants (compare the sack branch, which uses `direction * offset.x + normal * offset.y`).

**Failure scenario.** Any doorway whose `spot_hash % 4 == 3` emits six logs at three positions: each pair overlaps over 0.75 m of its 1.05 m length, so the rick renders as three logs with ~50% of the emitted geometry buried inside its twin, instead of the intended 3-high × 2-deep stack.


## src/scene.rs:941

### Apse arcade is missing its final boundary column, leaving the hemicycle asymmetric

The apse spawns 12 wall segments at `theta = (i as f32 + 0.5) * PI / segment_count as f32` (7.5°…172.5°), which have 13 boundaries, but only 12 boundary columns are spawned:

```rust
let boundary_angle = i as f32 * PI / segment_count as f32;
```

with `i in 0..segment_count`, giving 0°, 15°, … 165° — the closing boundary at 180° is never emitted.

**Failure scenario.** A compound column stands at the θ=0 end of the hemicycle at (x = 23*0.985 = +22.66, z = -82), but the mirror position (-22.66, -82) at θ=π has none. Standing in the choir looking east/west across the apse, the arcade terminates in a pier on one side and in nothing on the other, in an otherwise perfectly symmetric hand-authored apse.


## src/soundscape.rs:2020

### "Once a day" NPC yawn fires twice per evening: the day rolls inside the Snuffing

The yawn is gated on the sim day number while its office window straddles midnight:

```rust
    if now >= state.next_global_at && matches!(office, Some(Office::Lamplight | Office::Snuffing)) {
        ...
            if timer.yawn_day == Some(day) {
                continue;
            }
            ...
                timer.yawn_day = Some(day);
                timer.yawn_due_at = None;
```

`Office::Snuffing` runs 21:00 to 02:00 (`crates/cathedral-sim/src/clock.rs`: `Snuffing => 21.0`, `Watch => 2.0`), so `clock.day` increments while the office does not change. This file already documents that exact hazard for the other day-keyed once-a-day state, `CivicBellState` (lines 1679-1684): "The office alone, never `(day, office)`: the Snuffing runs 21:00 to 02:00, so the day number changes *inside* it, and a day-keyed edge would ring the city's curfew a second time at midnight." The yawn timer makes precisely that mistake, and `timer.yawn_due_at = None` on firing leaves nothing else to block a re-roll.

**Failure scenario.** The player stands near a stationary NPC through the evening. At 21:40 on day 3 the NPC yawns; `yawn_day = Some(3)` and `yawn_due_at = None`. At 00:00 the clock rolls to day 4 while `clock.office` is still `Office::Snuffing`, so `timer.yawn_day == Some(3) != Some(4)` passes the guard, a fresh `yawn_due_at` of `now + 20..150 s` is drawn, and the same NPC audibly yawns a second "end of day" yawn well before the 02:00 edge into the Watch closes the window.


## src/screenshot.rs:89

### Dead-key screenshot binding bypasses the chat box's keyboard suppression

`capture_screenshot_on_key` triggers on logical keys as well as physical ones:

```rust
physical_keys.just_pressed(KeyCode::F5)
    || physical_keys.just_pressed(KeyCode::Equal)
    || physical_keys.just_pressed(KeyCode::Backquote)
    || logical_keys.just_pressed(Key::Dead(Some('\u{b4}')))
    || logical_keys.just_pressed(Key::Dead(Some('\u{301}')))
    || logical_keys.just_pressed(Key::Character("\u{b4}".into()))
```

The typed-chat editor is what stops other bindings firing from a keystroke meant as text, and it does that by resetting only the physical map — `src/smart_actors/chat.rs::collect_chat_input` ends with `keyboard.reset_all()` on `ResMut<ButtonInput<KeyCode>>`, and its module doc explicitly lists F5 among what that hides. It never touches `ButtonInput<Key>`. Bevy's `keyboard_input_system` presses *both* maps from the same event, so the three `logical_keys` arms above still see the keystroke, and the system itself has no chat/menu/map/inventory gate at all. The physical arms (F5, `Equal`, `Backquote`) are correctly suppressed; the logical ones are not — the asymmetry is the bug.

**Failure scenario.** On the sv-SE layout the game is developed for, press Enter to open the chat box and type the Swedish word "idé": the acute dead key arrives as `Key::Dead(Some('´'))`, `logical_keys.just_pressed` is true, and the game writes `logs/latest_session/screenshots/cathedral_screenshot_<ts>__00.png` plus an `INFO Capturing screenshot to …` line — one stray PNG per accented character typed, while the same message typed with F5 pressed correctly produces nothing.


## src/weather/mod.rs:201

### smooth_weather spends its one-time snap on the CLEAR default, not the first sim sample

`smooth_weather` snaps instead of blending exactly once, keyed on its own `initialized` flag:

```rust
let target = authoritative.current;
if !visual.initialized {
    *visual = SmoothedWeather::from_sample(target, true);
    return;
}
```

It never consults `WorldWeatherState::present`, which is the flag that says the sim has actually spoken (`receive` sets `self.present = true`; the resource's `Default` is `present: false` with `current: WeatherSample::CLEAR`). Because `smooth_weather` is registered in `Update` and runs from frame 1 — long before the engine's first `EngineMessage::Weather` reaches the bridge — the snap is consumed by the CLEAR placeholder and `initialized` is already true when the real sample lands. The first authoritative sample is therefore exponentially ramped like any later change. `WorldWeatherState::present` is written but read nowhere in the codebase (only `WorldClockState::present` is), which is the tell that this gate was meant to be here.

**Failure scenario.** Start a session whose weather timeline is mid-downpour (or `config.ron: weather.mode: "downpour"`). Frame 1 snaps the smoothed values to CLEAR (`cloud_cover 0.08`, `precipitation 0.0`, `visibility_m 340.0`). The engine's first sample arrives a few frames later and is blended in at tau 2.4 s (cloud), 0.55 s (rain) and 1.2 s (visibility), so for the opening seconds the sky, fog and rain density present as a clear day. `CATHEDRAL_SHOT=storm cargo run` — which is `sleep 2; shot storm; quit` — captures that half-formed sky rather than the storm the sim is actually running.


---


# Rejected by verification (20)

Raised by a finder, then killed on review. Recorded so they are not re-litigated.

- `crates/cathedral-sim/src/actions.rs:3205` — A seizure whose station has no route leaves a phantom custody record behind
- `crates/cathedral-sim/src/speech_router.rs:611` — Refused recording strands its stream: player_composing() stuck true, cast stops thinking
- `crates/cathedral-backends/src/stt_realtime.rs:533` — A provider `error` event leaks its commit slot, permanently desyncing the ack FIFO
- `crates/cathedral-sim/src/round.rs:3636` — abandon_bodily_errands drops a stall errand but leaves its walk running
- `crates/cathedral-sim/src/prompt/mod.rs:524` — Prisoner paragraph gated on is_held (hand on arm) instead of holds (in custody)
- `crates/cathedral-sim/src/scheduler.rs:523` — Background-turn backoff overrides a pending player reaction's wake-up
- `crates/cathedral-backends/src/stt_realtime.rs:604` — `scrub()` truncates before redacting, so a split API key reaches the HUD and logs
- `src/smart_actors/chat.rs:332` — Chat box's Ctrl mirror is never cleared on focus loss, silently swallowing all typing
- `src/smart_actors/speech.rs:471` — Rejected TTS WAV leaves its audio expectation queued, stalling all speech for 10 minutes
- `src/smart_actors/config_menu.rs:316` — STT pill switches to an unavailable backend the UI greyed out, and persists it
- `src/smart_actors/hud.rs:257` — Disconnect never clears law_standing, pinning a stale custody panel forever
- `crates/cathedral-sim/src/seed.rs:232` — validate() lets an unheld item through, so build_world panics instead of erroring
- `crates/cathedral-backends/src/config.rs:658` — An empty LLM_PROVIDER disables cognition instead of falling back to moonshot
- `src/smart_actors/local_engine.rs:212` — Session-dir fallback: mic writes to /tmp while transcription reads from the CWD
- `src/smart_actors/hands.rs:651` — Custody grip aim double-counts shoulder height, so the arm reaches too high
- `src/smart_actors/speech.rs:590` — Stream end with zero accepted chunks never releases the queued line
- `src/city/water.rs:806` — Three-Curb's 2nd and 3rd windlasses are mirrored: gear points inward, roofs 30° askew
- `src/soundscape.rs:1366` — Cooldown pruning drops bell entries mid-peal (120 s retention vs 358 s cooldown)
- `crates/cathedral-backends/src/stt_realtime.rs:222` — begin() leaves active_key set on a full action queue; socket never idle-closes
- `src/smart_actors/speech.rs:436` — Speech bubble stacks are never despawned; per-speaker UI entities accumulate
