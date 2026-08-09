Status: M1 and M2 implemented 2026-08-09; M3–M5 not started (written
2026-08-09). The knob it fixes — `config.ron: smart_actors.extra_ambient_npcs`,
0..=20000 — shipped the same day; see `crates/cathedral-sim/src/crowd.rs` and
the AGENTS.md section "The crowd knob".

# Give the crowd somewhere to be

`extra_ambient_npcs` fills the city with generated ambient citizens. It works,
and what it produces is wrong in a specific and fixable way: **the crowd is a
traffic jam, not a population.** Everybody is on a through-route, everybody is
walking to one of twenty-three places, and the lines at the wells never clear.
Nobody is leaning on a wall in a back lane, nobody is sitting in a yard, and
nobody goes home, because nobody has one.

Evidence: `logs/session_745_2026-08-09_14_42_00/screenshots/` at 20,000
(`20000_wickmarket.png` — a queue threading a lane; `20000_above.png` — the
same queue from 30 m up, with empty yards on either side of it) and
`logs/session_747_2026-08-09_15_01_34/` at 2,000.

## What the crowd is today, measured

Every number below is from the shipped data, not from memory.

**Placement is on the street graph, not the ground.**
`crowd.rs:53 spread_over_walkable` picks from the navigation graph's 3,972
nodes and jitters by `SPREAD_JITTER_M = 1.6` (`crowd.rs:44`). But the graph is
the *welded through-route skeleton*: the quarter-metre walkable bitset covers
5,162,715 cells — **322,670 m² of walkable ground, i.e. 81 m² per graph
node**. The 23 named sites (`Coswald's Yard`, `Burnt Court`, `Skinners'
Court`, `Seven Lofts`, `The Shambles`, the parish reserves…), the widenings,
the alley dead ends and the strips beside walls are all walkable and all
outside the 1.6 m band. Placing on nodes means placing on the roads people
route down, by construction.

**There are 23 workplaces for the entire crowd.**
`round.rs:5423 build_legs` gives each trade the *nearest* of its candidate
places in `assets/world/rounds.json: workplaces`. The whole file names 36
distinct places; 31 of the city's 65 occupations have exactly **one**
candidate. The 32 trades in `crowd.rs:295 TRADES` reach 23 of them:

> Cinder Row · Coswald's Yard · Doctor Ferrant's house · Gaunt Passage ·
> Maren's Green · Skinners' Court · Tanners' Slip · Tenterhook Lane · The
> Alder Moorings · The Bellstand · The Cut · The Cut ropewalk · The Draper's
> Reach · The Gradine · The Harne Gate · The Hungry Ox · The Lanthorn · The
> Needle · The Old Sluice · The Tallage · The Wickmarket · The Wool Gate ·
> The masons' lodge

At `extra_ambient_npcs: 1000` that is ~43 people per anchor; at 20,000, ~870.
They stop within `ROUND_ARRIVE_RADIUS_M = 6.0` m of the point and mill inside
`DEFAULT_ROUND_LEASH_M = 10.0` m of it. **19 of the 32 trades share the
`day_worker` archetype**, so they are also on the same schedule.

**The lines at the wells are real queues.**
Three occupations draw water (`round.rs:64-79`: `domestic_servant` with a
household vessel, `cloth_worker` and `garment_worker` with a trade one) — 3 of
32 trades, so ~94 drawers at 1,000 and ~1,900 at 20,000, each bound to the
nearest *staffed* of 9 sources. `WaterSource.queue` has no cap and a keeper
serves one at a time. Those queues cannot clear.

**Nobody has a bed.** `homes.json` is baked per authored id, so a generated
citizen gets `home: None` (`crowd.rs:117 ambient_sheet`). Three consequences,
none of them obvious from the field:

1. `build_legs` skips the `"home"` leg outright — *"the homeless have no bed to
   walk to"* (`round.rs:5471`). They never walk into a residential lane and
   never sleep on a step.
2. The nightly **ambient evening roll** — the code that sends ambient people to
   taverns and their ward's chalk sign — opens with
   `if person.curfew_exempt || person.home.is_none() { continue }`. The crowd
   has no evenings at all.
3. At curfew the housed are sent home; the homeless simply stand where their
   last leg dropped them, which is a workplace.

Structurally, what shipped is *N homeless day-labourers who all work at one of
twenty-three places*. That is exactly the picture on screen.

## Design stance

Five milestones, in increasing order of invasiveness and payoff. **M1 and M2
together change the whole picture for very little code**; M4 is the one that
buys actual residential life and is deliberately last.

Numbered M1–M5 rather than the usual M0–M4 so they map one-to-one onto the
five fixes from the 2026-08-09 review that produced this file.

Three rules hold across all of them:

- **Nothing may change at `extra_ambient_npcs: 0`.** The authored cast's day is
  pinned by golden prompt fixtures and by the daily-round tests. Every branch
  below is gated on `LoreProfile.generated`, the flag `crowd.rs` already sets
  and which no character file can set (`lore.rs`, `LoreCharacterSheet` has no
  such field).
- **No new authored content.** Widening `rounds.json` is real authoring work
  and is out of scope; see "What this does not fix".
- **No performance work.** At 20,000 the measured p50 frame is 204 ms with
  179 ms of it inside the engine pump. None of these five milestones improves
  that and none is allowed to make it materially worse. Making a crowd
  *playable* above ~2,000 is a separate feature.

---

## M1 — Spread on the ground, not along the graph

**Implemented 2026-08-09** in `crates/cathedral-sim/src/crowd.rs`, as written
below with two things worth recording:

- The eight draws are salted apart by adding the attempt index to each of the
  two existing salts, rather than by mixing it into the hashed index — mixing
  it in would have made citizen *n*'s second draw identical to citizen *n+1*'s
  first. The offset is now built by a private `spread_point`, so
  `spread_over_walkable` still only owns *which* node a citizen belongs to.
- The fallback rate is **1.4%**, not the "well under 1%" estimated above (28 of
  2,000 against the shipped graph). The estimate treated the draws as
  independent samples of a 53.6%-walkable grid; they are not — a node walled in
  on several sides rejects all eight together. 1.4% still stand on their node,
  exactly as the whole crowd used to.

Measured over 2,000 points against the shipped graph: every point walkable,
1,843 of 2,000 more than 3 m off their anchor node (before: **0**, since 1.6 m
is never more than 3), none beyond 12 m. Test:
`crowd::tests::the_crowd_stands_off_its_nodes_and_all_of_it_on_walkable_ground`,
which loads the committed `navigation.json`/`.bin` the way `round/tests.rs`
does. Nothing is gated on `generated` because nothing had to be:
`spread_over_walkable` has exactly two call sites
(`src/smart_actors/local_engine.rs`, `cathedral_headless.rs`) and both return
before it at `extra_ambient_npcs: 0`.

Drive evidence at 20,000 — `logs/session_753_2026-08-09_20_44_50/screenshots/`
(before) against `logs/session_754_2026-08-09_20_45_41/screenshots/`
(`*_coswalds_above.png` is the pair to look at: the yard's paving goes from
empty-with-a-chain-of-bodies-down-the-road to occupied). The narrow-lane pair
(`*_cinder_row.png`) barely moves, which is correct — a 4.6 m lane has no width
to spread into, so there the 12 m only pushes people *along* it.

Replace the 1.6 m jitter with a real radius and a walkability rejection test —
the idiom `round.rs:8270 wander_target` already uses for the same problem.

```
const SPREAD_RADIUS_M: f64 = 12.0;   // was SPREAD_JITTER_M = 1.6
const SPREAD_ATTEMPTS: usize = 8;
```

For each citizen: keep the coprime node stride (it is what guarantees even
coverage of the *reachable* city — `crowd.rs:83 coprime_stride`, tested), then
draw up to `SPREAD_ATTEMPTS` polar offsets inside `SPREAD_RADIUS_M` of that
node, take the first that `nav.is_walkable`, and fall back to the node point.
At 53.6% of the grid walkable, eight attempts fail on well under 1% of
citizens, and those simply stand on the node as they do today.

**Why anchored to a node and not uniform over the bitset.** A uniform sample
over 322,670 m² is the obvious move and it is a trap: it can land somebody in a
walkable pocket the graph never enters. `route_between` snaps both endpoints to
the *nearest node*, so a body sealed in a court would walk through a wall on its
first errand. Anchoring at 12 m bounds the connection to something the graph
plausibly reaches, and leaves genuinely unreachable ground empty — which is the
correct outcome, not a limitation.

A 12 m disc is 452 m² against 81 m² of walkable ground per node, so the discs
overlap heavily and the coverage is continuous; what changes is that the crowd
now occupies the *width* of the city's open ground instead of a 3.2 m ribbon
down the middle of it.

**Verify.** A unit test in `crowd.rs`: over 2,000 points against the shipped
graph, every point is walkable, and a clear majority are further than 3 m from
their anchor node (today: none are). Then a drive shot standing in a
residential lane and one in `Coswald's Yard` — both empty today.

---

## M2 — A cohort with no trade at all

**Implemented 2026-08-09** in `crates/cathedral-sim/src/crowd.rs`
(`NO_TRADE_SHARE = 0.25`, one branch in `ambient_sheet`), with five things
worth recording:

- **The support circumstances and the prose are one bank, not two.** `SUPPORTS`
  pairs each set of circumstances with the sentence that says the same thing in
  the second person, because two banks drawn independently produce a citizen
  whose sheet says `alms_dependent` and whose description says they live off
  piece-work — the "prompt content drifts from behaviour" row of the risk
  ledger, arrived at by construction. Ten entries; `retired` and `widow` carry
  their own minimum age (58 and 34), because nobody is retired at sixteen.
  `widow`/`widower` is written once and re-spelled for men.
- **The validator's own list is now a named constant.** `lore.rs` had
  `"alms_dependent" | "dependent" | "pauper" | "prisoner"` inline in
  `LoreCharacterSheet::validate`; it is `SUPPORT_CIRCUMSTANCES` now, and the
  crowd's test asserts against *that* rather than a copy, so the generated
  no-trade shape cannot drift from the authored one. `prisoner` is deliberately
  never minted: `custody.rs` seeds anybody carrying it into the Stone House.
- **Three banks, not the two budgeted.** `NO_TRADE_OPENINGS` and the support
  lines were the estimate; `NO_TRADE_CONCERNS` is the third, because two of
  `CONCERNS`' twelve entries presume a trade ("your employer is late", "a strap
  on your load has frayed"). Its ten goals are all textually distinct from the
  twelve, so `a_crowd_is_not_one_person_repeated` still counts a clean sum. Two
  existing lines were also reworded where they collided with having no work
  ("…and then get back to work"; "Your working years are behind you" against
  the concern that wants a day's work).
- **A fourth consequence the spec did not claim: they draw no water.**
  `vessel_of(None)` is `None`, so the no-trade quarter is bound to no well. At
  `--extra-ambient 400` the drawer count is 97 against 69 for the cast alone —
  28 generated drawers where the pre-M2 crowd would have produced ~37 (3 of 32
  trades draw). It does not fix the unbounded queues under "What this does not
  fix", but it takes a quarter of every future crowd out of them.
- **The `[crowd]` diagnostic now counts the cohort out loud** — the
  `cathedral-headless` line reports the share and, deliberately, the number of
  no-trade citizens *without* a support circumstance, which is a zero that
  deserves printing rather than assuming.

**Measured.** `cathedral-headless --fake --extra-ambient 400`:
`[crowd] 400 generated ambient citizens; 95 with no trade at all (23.8%), 0 of
those with no support circumstance` — 1.2 points under target at n=400, and
25.0% ± <2 points over 4,000 in the unit test.

Curiosity, measured over 2,000 rather than assumed: the no-trade cohort spans
**0.162..=0.322, mean 0.232**; the trades **0.082..=0.192, mean 0.118**. The
spec's ≈0.30 is the *top* of the no-trade band and not its middle (0.082 + 0.10
+ 0.10 + 0.02 is the entry carrying begs *and* unhoused; under 25 the age term
takes it to 0.322). The floor, 0.162, is the retired pauper. The two bands
**overlap** — a young milk seller out-talks a retired pauper, which is correct —
so the test pins the means and the floors, not a separation.

**The leash claim is only two-thirds true, and that is not M2's doing.**
`--fake --extra-ambient 400 --watch-clock 1 --seconds-per-day 300
--trace-positions`, displacement from spawn at the Snuffing:

| | n | median | p90 | max | within 10 m |
|---|---:|---:|---:|---:|---:|
| no trade | 95 | **6.7 m** | 61 m | 122 m | 58 (61%) |
| a trade | 305 | **98.3 m** | 133 m | 164 m | 1 (0.3%) |

The qualitative claim is overwhelming — the median loiterer ends the day inside
the leash and the median tradesman a hundred metres away — but the spec's
"*must* still be within `DEFAULT_ROUND_LEASH_M`" does not hold for 39% of them.
Traced individually, those are a slow monotone creep (~0.7 m per poll, no plateau),
not an errand: `nearest_known_settled` requires `me.knows()` and the crowd knows
nobody, so rung 11 never fires for them, and rung 12's `wander_target` already
`clamp_to_leash`es every target to 10 m of `person.base`. Every *aim* is inside
the leash; the *walking* leaks, because `route_path` snaps both endpoints to the
nearest nav node and M1 now stands people up to 12 m off the graph — so a target
8 m away can route out of the lane and back, and `Decision::Stay` does not
cancel a walk in progress. Identical numbers with `--weather clear`, so the
shelter rung is not involved. **This is pre-existing and belongs to M3**, which
widens the leash and will amplify it; it is exactly the "far half of a wide
leash" row already in the ledger, seen from the other side.

Drive evidence at 2,000: `logs/session_755_2026-08-09_21_08_24/screenshots/`
(`m2_2000_coswalds_ground.png`, `m2_2000_cinder_row.png`) — people standing
about a yard and filling a lane in dark, patched Poor outfits rather than
threading it in a queue.

Tests added: `crowd::tests::a_quarter_of_the_crowd_has_no_trade_and_says_how_it_eats`,
`crowd::tests::the_loiterers_are_the_curious_ones`,
`round::tests::a_generated_citizen_with_no_trade_is_enrolled_with_no_legs`
(the last one pins the `build_legs` claim directly: no legs, `DEFAULT_ROUND_LEASH_M`,
no water source, an empty `daily_round`, against a tradesman neighbour who does
get legs). `cargo test --workspace`: **1469 passed, 4 ignored**, up exactly the
three added from the 1466 baseline. `every_generated_citizen_is_an_ambient_with_a_trade_and_a_goal`
was renamed (it no longer asserts a trade) and now checks the two authored
shapes and nothing between them.

Roughly **a quarter of the crowd gets no occupation**: `occupation_id: None`,
`title: None`, `rank: None`, plus one support circumstance from the loader's own
controlled list (`lore.rs CONTROLLED_CIRCUMSTANCES`) — `pauper`,
`alms_dependent`, `unhoused`, `begs_regularly`, `intermittently_employed`,
`retired`, `widow`/`widower`. This is not a special case bolted on; it is the
`no_fixed_trade/` shape the authored cast already has, and the validator
already requires exactly this pairing.

Everything then follows for free:

- `build_legs` finds no archetype and returns `(&[], DEFAULT_ROUND_LEASH_M,
  false)`. With no active leg the ladder falls through to rung 11 (the social
  pull toward a settled neighbour) and rung 12 (wander, gated at 0.35 per
  decision) around `person.base` — `round.rs:7908-7930`. **That is "hanging
  out", already implemented.**
- `AppearanceSnapshot::compose` reads `None` as `OutfitClass::Poor`, so they
  read as a different class of person at thirty metres without a single new
  mesh.
- `derived_curiosity` (`attention.rs:707`) gives them `CURIOSITY_BASE` 0.082
  + 0.10 (no fixed trade) + 0.10 (begs/alms, counted once) + 0.02 (unhoused)
  ≈ **0.30**, against roughly 0.08–0.17 for a tradesman. The people loitering
  against the wall are the ones who speak to you first. That is the right
  street and it costs nothing to arrange.

**One thing does not follow for free.** The description template in
`ambient_sheet` opens "You are a {title} in {district}", which has no meaning
without a title. This cohort needs its own template answering the ambient
authoring questions (`lore/characters/AGENTS.md`) — in particular *how they
materially survive*, which the no-fixed-trade shape is required to explain.
Budget one extra bank of openings and one of support lines.

**Verify.** `cathedral-headless --fake --extra-ambient 400`: assert the
no-trade share is within a point or two of the target, and that every no-trade
citizen carries at least one support circumstance. Then
`--watch-clock 1 --seconds-per-day 300 --trace-positions`: a no-trade citizen
must still be within `DEFAULT_ROUND_LEASH_M` of their spawn a game day later,
while a tradesman must not be.

---

## M3 — A leash sized for a crowd

`DEFAULT_ROUND_LEASH_M = 10.0` was calibrated for a cast of ~500 spread over
23 anchors, i.e. a handful of people per post. It is the reason a workplace
reads as a scrum rather than a busy corner.

Give a generated citizen a **per-person leash drawn 15–40 m** (deterministic,
off the existing `hash01` idiom) instead of the flat 10. Written once, in
`Round::seed`'s enrolment branch, behind `lore.generated`. A range rather than a
constant so the clump has a soft edge instead of a second, larger hard edge.

Two knock-on effects to handle rather than discover:

- `wander_target` tries **4** offsets before giving up. At 30 m in a narrow
  lane the rejection rate is much higher than at 10 m in a street; raise the
  attempt count (it is a cheap bitset lookup) or the far half of the leash is
  quietly unreachable.
- `CENSUS_POST_RADIUS_M = 9.0` decides who counts as "at their post". A 30 m
  leash puts a milling citizen outside it, so `--trace-round`'s census would
  report a city that never turns up for work. Either widen the census radius
  for generated citizens or read the census as `min(leash, post_radius)`;
  pick one and say so in the code, because the census is how this milestone is
  measured.

**Verify.** `--trace-round --census-by-area` before and after at
`--extra-ambient 1000`: the `at_post` count must not collapse, and a drive shot
of The Wickmarket must show people spread across the square rather than stacked
on its node.

---

## M4 — A door to call home

The payoff milestone, and the invasive one. The city has **1,101 doors**
against 1,032 buildings, and `nav::Door` is exactly the right anchor:

```rust
pub struct Door { pub building: String, pub edge: usize, pub node: usize }
// …the walkable node a metre outside the threshold.
```

Assign each generated citizen a door — deterministically, with a per-door
occupancy cap so they spread rather than piling on whichever door is nearest
the graph's first node — set `Townsperson.home`, and register it with
`PlaceRegistry::add_home` (`places.rs:182`) exactly as a housed authored
character's is. At 20,000 that is ~18 to a house, which is medievally
unremarkable; at 1,000, about one.

What switches on the moment `home` is `Some`, with no further code:

- The archetype's `"home"` leg stops being skipped, so there is a **morning
  tide out of the lanes and an evening tide back into them** — the single
  biggest change to what the city looks like.
- The curfew send-home applies to them.
- The **ambient evening roll** starts choosing their nights: a tavern, or their
  own ward's chalk sign. The crowd acquires evenings.
- `housed` in the round's seed diagnostic becomes truthful (today it reports
  413 regardless of crowd size, which is a real tell that the crowd has no
  domestic life).

Also derive the profile's `home` prose string ("a house in the Cinder Ward,
near the Shambles well") from the chosen door's ward, or "Where do you live?"
will be answered by a citizen who walks home every night and cannot say where
it is.

**Risks specific to M4.**

- `PlaceRegistry::free_id` salts past collisions in a loop. Twenty thousand
  inserts is the first time it has been asked to; check it is not quadratic
  before shipping, not after.
- Each citizen's `places_known` gains their own home. It stays small — the
  crowd knows nobody, so no friends' homes are added — but confirm it, because
  `places_you_know` is rendered into every prompt.
- The evening roll is per ambient per night; at 20,000 that is new nightly
  work in the pump, which is already the bottleneck. Measure it with
  `--watch-clock 2` before and after.
- `settle_households` already sweeps every `EconomicClass::Resident` daily, so
  the crowd is *already* in the daily settlement — M4 does not add them to it,
  but it is worth a line in the report either way, because it is a once-a-day
  O(roster) pass nobody has looked at with a crowd in the world.

**Verify.** `--watch-clock 2 --seconds-per-day 300 --trace-round`: the census
must show a real `at_home` population at the Snuffing and a real `at_post` one
at High Wick, and the two must swap. Then a drive shot down a residential lane
at the Lamplight, which today is empty and should not be.

---

## M5 — Smear the tide

There is no "the bell fires the legs" event to stagger, which is worth stating
because it is the natural first guess. `active_leg(legs, office, weekday)`
(`round.rs:5502`) is evaluated on each person's *own* ladder poll, jittered
`LADDER_DECISION_MIN_SECONDS` 1.0 to `LADDER_DECISION_MAX_SECONDS` 6.0. So the
tide is already smeared — by up to six seconds. With the default
`seconds_per_day: 3600` an office is ~514 real seconds, so the entire trade
sets off inside **1% of the office**, and 870 people walk the same corridor
shoulder to shoulder.

Give each generated citizen a deterministic **office lag**: after a crossing,
hold the previous leg for `lag(id, office)` seconds, drawn over 0..~25% of an
office. Cleanest as a `leg_lag_seconds` field on `Townsperson`, zero for the
authored cast, consulted at the one `active_leg` call site — so the cast's
timings stay byte-identical and the tests that pin them do not move.

This is the smallest of the five and the one that most changes how a *street*
reads: a trickle of people over minutes instead of a column.

**Verify.** `--watch-clock 1 --census-by-area` at `--extra-ambient 2000`: the
census's `walking` count should show a lower, wider peak after an office bell
rather than a spike. A drive run parked on a main route across an office
crossing, before and after, is the qualitative check.

---

## What this does not fix

- **Twenty-three workplaces is still twenty-three workplaces.** M3 and M5 make
  the pile-up look like a neighbourhood; they do not remove it. The real fix is
  authoring — more candidate places per occupation in `rounds.json`, and/or
  picking a candidate by a spread rather than by nearest-to-base. That is a
  sixth milestone and mostly a content task; it wants its own decision about
  whether generated citizens may work at places the authored cast does not.
- **The well queues.** Nine sources, one keeper each, one draw at a time, no
  cap on the queue. With ~9% of any crowd being drawers this is unbounded by
  construction. Either the generated crowd should not draw water at all (a
  one-line `vessel_of` gate on `generated`), or the queue needs a balk rule —
  "the line is long, come back later" — which is a real behaviour worth having
  and does not belong in this feature.
- **Frame cost.** Unchanged: ~26 ECS entities a head, p50 7.2 ms at 0, 16.8 ms
  at 2,000, 204 ms at 20,000. Above ~2,000 the bottleneck is the engine pump,
  not the puppets.

## Risk ledger

| Risk | Where | Watch |
|---|---|---|
| A change leaks to the authored cast | every milestone | golden prompt fixtures + the daily-round tests must not move at `extra_ambient_npcs: 0` |
| Somebody spawns in an unreachable pocket and walks through a wall | M1 | anchor the sample to a graph node; never sample uniformly over the bitset |
| The far half of a wide leash is unreachable | M3 | `wander_target`'s 4-attempt cap |
| The census stops reporting a working city | M3 | `CENSUS_POST_RADIUS_M` vs the new leash |
| `PlaceRegistry::free_id` goes quadratic at 20k homes | M4 | time `Round::seed` at 20,000 before and after |
| The nightly evening roll lands in the pump, already the bottleneck | M4 | `--watch-clock 2` pump timing before and after |
| Prompt content drifts from behaviour | M2, M4 | a no-trade citizen must be able to say how they eat; a housed one, where they live |
