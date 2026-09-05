# 02 — The numbers

Every constant, the model it comes out of, and the arithmetic that gets it there. Nothing here is a
feeling; nothing here is a number somebody liked the look of.

Read this before writing M2. The whole point of it is that a missed cadence target is a **division**,
not a search. Reconciled 2026-09-04 against the arithmetic critique: `VOLUNTEER_HEAT` is re-solved
(D53), both ends are restated as what the city does at its bells (D54), and the self-subject rule
left the salience product (D51). Where a figure below differs from an earlier draft, this file wins.

---

## 1. The measured inputs

All measured off this tree (`develop`, `f56a2c3`), not estimated. Reproduce them with the commands in
§8.

| Input | Value | Where from |
|---|---|---|
| authored cast | **519** sheets | `lore/characters/**/*.json` |
| sheets authoring their own `curiosity` | **0** — so `curiosity_of` is always `derived_curiosity` in production | `attention.rs:686` |
| **c̄ = mean `curiosity_of`** | **0.1193** (median 0.082, p10 0.062, p90 **0.202** by nearest rank, min 0.030, max 0.392 — `p00au`, no trade; σ 0.068) | `attention.rs:707-766` walked over all 519 |
| **standing** ward populations at spawn (Voronoi over 321 doors — where people *are*, not what their sheet says) | BellAndSluice **192**, Weigh **66**, Reed **62**, Wick **53**, Cloth **46**, Fabric **45**, Wallwright **29**, Cinder **26** (mean 64.9, median 53) | `homes.json` + each sheet's `spawn_location` |
| lore ward populations (what a sheet *says* — **not** the pollen ward) | bell_and_sluice 169, weigh 79, reed 66, cloth 47, fabric 44, wick 42, cinder 39, wallwright 33 | `LoreProfile.planning_ward` |
| baked doors | 413, all distinct, **321** with a ward the map knows (92 are "Outer wards"); minimum separation between any two **1.2748 m** | `assets/world/homes.json`, `crowd::ward_of_district` |
| door hulls | the 413 doors: x ∈ [-338.4, 329.1], z ∈ [-457.1, 327.6]; the 321 ward-labelled marks: x ∈ [-323.6, 329.1], z ∈ [-457.1, 324.9]. The grid uses neither | `homes.json` |
| walkable city box | x ∈ [-365.0, 361.8], z ∈ [-480.5, 347.8] → **726.75 × 828.25 m** | `navigation.json: grid` |
| named areas | **71** | `assets/world/areas.json` |
| named nav places | **70**; the Wickmarket at (-17.375, 248.375), standing ward **Wick** | `navigation.json: places[node]` |
| **X = ward crossings per person per game day, city mean** | **2.368** (70.9% of the cast sleep in a different standing ward from the one they work in; 4 legs/day for `day_worker`+`wharf`, 2 for most others, 0 for `stationary`) — a **leg** count; the tally counts boundary changes (a Wick → Weigh walk through Reed is 2), which is why the measured city figure is printed and compared like with like | `rounds.json: archetypes/occupations/workplaces` × `homes.json` × the Voronoi |
| **X_W = the mint ward's own exit rate** | exits of Wick ÷ person-game-hours spent in Wick, × 24 — the quantity every expectation in §4 uses; measured by the tally, not derived, because the standing wards are 7:1 lopsided and a city mean hides both ends | `CrossingTally` (M2 step 17) |
| **the commute is columns, not a stream** | the authored cast has **zero leg lag** (`crowd_leg_lag_share` is written only in `Round::seed`'s generated-enrolment branch, `round.rs:3286`), and a leg fires on the first ladder decision after the bell — `LADDER_DECISION_MIN/MAX_SECONDS` = 1..6 sim-s (`lib.rs:203-204`) = 0.4–2.4 game minutes at 3600 s/day. So all crossings of a day happen in **seven columns**, one per bell | `round.rs`, `lib.rs` |
| **walking is in sim-seconds** | `WALK_SPEED_MPS = 2.1` (`lib.rs:175`); `tick_movement` advances movers by `MOVEMENT_TICK_SECONDS = 0.05` slices, at most `MAX_MOVEMENT_CATCHUP_SLICES = 8` per poll → **0.4 s of walking per poll** (`engine.rs:2714-2725`); a `go_to` is budgeted `2.5 × metres / 2.1` sim-s (`round.rs:4842`). The `T` key scales only the clock (`clock.rs:390`) | `lib.rs:173-175, 318, 370` |
| the pack's mouths at the Wickmarket, and their boundaries | seeded: `dv8ll` Osanne Vell (chandler, stays in Wick), `p003t` Tib Stott (pilgrim), `bn5jk` Jonet Kett (civic_officer, works at The Tallage in **Weigh**, home in Wick), `p006f` Betriss Sedge (garment_worker, works at The Draper's Reach in Wick, home in Fabric); subject `p0043` Ede Kett (revenue_worker). Jonet Kett's Dayspring leg is 270.3 m and crosses the Wick/Weigh boundary **95.4 m** in (0.30 gh); the **nearest foreign-ward boundary from the Wickmarket is ≈ 82 m** (0.26 gh); the other three foreign-ward legs from it cross at 82–98 m | `rounds.json` × `homes.json` × the Voronoi (reproduced by the arithmetic critique) |
| offices, in game hours | Watch 3, Kindling 2, Dayspring 5, HighWick 3, Waning 3, Lamplight 3, Snuffing 5 → mean 24/7 = 3.4286 gh; the bells stand at 2:00, 5:00, 7:00, 12:00, 15:00, 18:00, 21:00 | `clock.rs:65-93` `start_fraction` |
| shipped clock | `seconds_per_day: 3600.0`, `start_office: "dayspring"`, `start_day: 2` → 1 gh = 150 real s, one office = 5–12.5 real min | `config.ron:49-54` |
| shipped crowd | `extra_ambient_npcs: 1000`, knob to 20,000 | `config.ron:67` |
| affinity-ear sizes in the shipped cast | Bed 77 (`domestic_servant` 45, `tavern_worker` 9, `sex_worker` 8, `water_and_bath_worker` 8, `laundress` 7); Law 63; Coin/Bread 42; no-trade 10 | counted per `occupation_id` |
| `knows` set size | mean **3.2**, median 2, max 14, six with none — and **seed-only for LLM actors** (`actions.rs:553`), empty for the whole generated crowd (`crowd.rs:597`, asserted `:1070`) | `lore/characters/**` |
| golden fixtures | **22** `.txt` | `crates/cathedral-sim/tests/fixtures/prompts/` |
| the sheet's own budget | a real sheet ≈ 13.6 KB against a 64 KiB assertion (`world_data.rs:422`); the snapshot is at 137 KB of 160 KiB (`:444`) | measured by the canaries |

---

## 2. The model, in eight lines

```
λ      = 0.5^(1/AIR_HALF_LIFE_GAME_HOURS)                 the air's cooling factor per game hour
air.heat(t)      = λ^t                                    mint at 1.0; deposits raise, cooling lowers
held.heat(t)     = HOP_LOSS^n · λ^t                       n = hops; the loss is charged once, at the hop
may_carry        = !fact.subject.contains(listener)       a rule, outside every product below (D51)
pickup roll      hash(fact, actor, stir) < (curiosity_of(actor) · air.heat · salience(fact, actor)).clamp(0,1)
deposit gate     fact.decays && held.heat(t) · salience(fact, self) > VOLUNTEER_HEAT      (D52)
rolls per person = STIRS_PER_GAME_HOUR per game hour      one roll per fact per stir, never per poll
transport        a holder walks their round; a deposit lands in the ward they are standing in
```

`held.heat` depends only on `t`, not on when the holder picked up — because the air's own heat is
`λ^t` and the hop loss is a single multiplication. That is what makes every closed form below
possible. `salience` is the **pure table product** `base × affinity × household` (D51); the subject
is excluded by `may_carry`, and a standing fact (`decays: false`) never auto-deposits (D52).

**There is no gain term.** The roll is exactly the spec's product. Two spec-named quantities carry no
number in the spec, and they are the two free parameters — one per end of the band:

| Parameter | Ends it pins | Retune rule |
|---|---|---|
| `VOLUNTEER_HEAT` | the **slow** end | `VH_new = 0.12 / 2^(t_b / τ)`, `t_b = d_b / (WALK_SPEED_MPS × seconds_per_day / 24)` game hours, `d_b` the nearest foreign-ward boundary from the mint place (§3, D53) |
| `STIRS_PER_GAME_HOUR` | the **fast** end's *density* | `S_new = S_old × target / measured` (then re-check the poll-gap invariant) |

Both retunes are one division. **Never retune a salience band** — the flat-table identity is asserted
against them, and moving one silently invalidates M2's baseline.

### Warm life — the quantity everything turns on

A carrier repeats a thing while `held.heat(t) · s > VH`, so their **warm life** from mint is

```
t_warm(s, n) = τ · log2( HOP_LOSS^n · s / VH )        clamped at 0; ∞ never — a standing fact has none (D52)
```

with `n = 0` for a witness. This is the deposit gate's own clock, and it is what makes "may never
leave its ward" arithmetically true rather than hopeful. Any implementer who reads `VOLUNTEER_HEAT`
as a cosmetic sheet threshold will not hit the band.

### The fast end is the city's commute, and the commute is a column at each bell

The word crosses in mouths, and the mouths move when the bells ring (§1): the authored cast sets off
within 1–6 sim-seconds of a bell, so crossings out of a ward come in **seven columns a day**, not as
a uniform stream. A closed form with a uniform rate `X_gh` is therefore right only when integrated
over **whole days** (F2, S1, the daily crossing count) and wrong for "one office after a mint" — the
first office after a mint at the Dayspring bell has **no bell in it** until HighWick, five game hours
later, so the pickup term contributes nothing before then and the seeded term is exactly the number
of seeded mouths whose Dayspring leg leaves the ward (one, Jonet Kett). So:

> **The fast end is a property of the city's existing commute, stated in bells and asserted as
> deposits. The slow end is what the constants are solved from, and it is solved against a distance
> — the nearest ward boundary — not against an office.**

---

## 3. The constants, derived

`τ = AIR_HALF_LIFE_GAME_HOURS = 12.0`; `λ = 0.5^(1/12) = 0.943874`; `k = ln(1/λ) = 0.057762`;
`Σ(T) = ∫₀^T λ^u du = (1 − λ^T)/k` → **Σ(5) = 4.342**, **Σ(6.5) = 5.419**, **Σ(11) = 8.1414**,
**Σ(24) = 12.9843**, `Σ(∞) = 1/k = 17.31`.

| Constant | Value | Derivation |
|---|---|---|
| `AIR_HALF_LIFE_GAME_HOURS` | **12.0** | Chosen first, as the legible statement *the ward's air halves every half game day*. It sets the top band's news life: `τ·log₂(1.00/VH) = 36.9 gh = 1.54 game days`. The bracket is `τ ∈ [7.7, 15.4]` for a news life of one to two game days; 12 is the round number in it. |
| **`VOLUNTEER_HEAT`** | **0.119** | **The slow-end solve, against the boundary walk (D53).** An off-affinity `Craft` fact has `s = 0.20 × 0.60 = 0.12`. The leak is the leg that fires at the mint bell: a warm witness who walks out of the ward deposits at their next poll (≤ 15 game min) into the ward they are then standing in. So require the witness cold **before any leg reaches a boundary**: `t_warm(0.12, 0) < d_b / WALK_SPEED_MPS`, with `d_b ≈ 82 m` the nearest foreign-ward boundary from the Wickmarket → `82 / 2.1 = 39 s = 0.26 gh` at 3600 s/day → `τ·log₂(0.12/VH) < 0.26` → `VH > 0.12 / 2^(0.26/12) = 0.1182`. Rounded up to three places → **0.119**, giving `t_warm = 12·log₂(0.12/0.119) = 0.145 gh ≈ 9 game minutes` — about one poll. (An earlier draft solved against "under one game hour" and got 0.115; at that value Jonet Kett was still warm 0.30 gh in, when she crosses into Weigh, and the slow-end fact was being said there inside forty game minutes.) |
| `STIRS_PER_GAME_HOUR` | **2.0** | A stir edge every 30 game minutes = 75 real s at the shipped clock. Must be ≫ the poll jitter (so no stir is skipped) and ≪ an office (so arrivals trickle instead of arriving in a column). Gives `P(a mint-ward person holds a fresh top-band fact)` at the next bell (5 gh) of `1 − exp(−2 × 0.1193 × 4.342) = 0.645` at c̄ — **≈ 0.60 as a population mean** (Jensen: c̄ sits inside a concave function; over the 519 real curiosities the mean of `1 − e^{−x}` is 0.486 at 3.43 gh and 0.893 at 24 gh, against 0.524 and 0.955 at c̄) — and `0.893` after a game day. |
| `POLLEN_POLL_GAME_MINUTES` | **10.0**, jittered ×0.5..1.5 → 5..15 | 6 polls/gh. **Max gap 15 gm < 30 gm = 60/S**, so no person can skip a stir window, so the effective roll rate is exactly `S = 2.00` and not a function of jitter. Composed with the host step: `15 + 0.16 = 15.16 gm` at the 0.4 s cadence step (3600 s/day), `15 + 7.2 = 22.2 gm` at `--watch-clock`'s default 18 s step, `15 + 1.2 = 16.2 gm` under the `T` key's 60× (one 0.05 s round tick = 1.2 gm) — all `< 30`. Asserted as an invariant with a failure message that says why. |
| `HOP_LOSS` | **0.85** | Bracketed by two inequalities, both asserted: `HOP_LOSS⁴ × 1.00 > VH` → `HOP_LOSS > 0.119^(1/4) = 0.5873` (a fourth-hand top-band story is still above the gate, so the ladder's far rungs are reachable and M3's chain is walkable), and `HOP_LOSS × 0.12 < VH` → `HOP_LOSS < 0.119/0.12 = 0.9917` (an off-affinity trade matter travels **one hop and stops**, which is what "nothing to anyone but a cooper" means arithmetically). 0.85 makes a hops-2 telling 72% as warm as a witness's and hops-4 52% — quieter, not dead. |
| `REHEAT_TO` | **`VOLUNTEER_HEAT × 1.10` = 0.1309** | An **absolute** heat, salience-free. `VH/salience` would re-heat a *dull* fact to 0.99 and a scandal to 0.119 — exactly inverted, and a category error besides: heat and salience are the two axes the spec spends a section separating. The 1.10 is **one rung above the gate**: a re-heated top-band fact stays above it for `12·log₂(1.10) = 1.65 gh` (about half an office), a re-asked `Bed` fact volunteers again (`0.1309 × 1.00 > 0.119`) and a re-asked off-trade `Craft` fact does not (`0.1309 × 0.12 = 0.0157 < 0.119`) — *asked for, not loud*. A re-heat deposit into the Wickmarket's ward yields `≈ 49 × c̄ × 0.1309 × 1.0 ≈ 0.77` expected new `Bed` carriers on the first stir. |
| `HEAT_GONE_BELOW` | **0.01** | Never 0.0: exponential decay underflows to exactly zero and `0.0 < 0.0` is false forever, making the row immortal and holding a cap slot nothing can reclaim (`marks.rs:505-521` rejects the same bug in `marks.json`). A row minted at 1.0 lives `12·log₂(100) = 79.7 gh` in the air. |
| `GARBLE_CHANCE_PER_HOP` | **0.35** | A rate is unavoidable — the spec requires per-hop garbling and gives no number. Both readings of `P(a masked field is still right) = (1−g)^n` the spec invites land on the same value: one hop right two times in three gives `g = 0.333`, four hops wrong four times in five gives `g = 0.331`. **0.35 is the rounder number just above ⅓**, chosen so a chain worth walking is a little more wrong than the coincidence. At 0.35: right 0.650 / 0.423 / 0.275 / 0.179 at hops 1–4. |
| `DAY_OFFSET_MAX` | **3** | The spec's day drift is ±1 per garbled hop "clamped to a small band". A **guard**, not a shape: three same-direction day garbles by hops 3 is `(0.35/2)³ ≈ 0.5%`, so the clamp almost never binds; it exists so a deep chain cannot wander a week and fall off `WHEN_DAYS_MAX`'s vocabulary. |
| `GARBLE_AREA_RADIUS_M` | **120.0** | `AreaMap::nearest_areas`' search radius. Over 71 areas in a 727 × 828 m city, 120 m returns 2–5 neighbours for a typical area — enough that the swap is not always the same place, small enough that "an adjacent area" stays adjacent. |
| `HOLDINGS_MAX` | **6** | A person is not a newspaper, and 6 × ~120 B + `Vec` 24 B + a `BTreeMap` entry's ~106 B ≈ **850 B/actor at the cap → 17.4 MB at 20,000**. See §6. |
| `KNOWN_SHEET_MAX` | **3** | Smaller than `notices::NOTICES_SHEET_MAX = 4` (`notices.rs:100`), as the spec requires. 3 bullets × ~110 B + a 136 B header + a 671 B note ≈ **1.14 KB** on a 13.6 KB sheet against a 64 KiB bound; M3's `known_from` clause and the role phrase take a saturated sheet to ≈ 1.4 KB, which M5 step 12 measures. |
| `AIR_PER_WARD_MAX` | **24** | 8 wards × 24 = 192 `Drift`s × ~104 B ≈ **20 KB** total. Comfortably more than any one ward's ear can hold at once, and a hard bound on an LLM spamming `raise_word`. |
| `FACTS_MAX_LIVE` | **256** | 256 × ~700 B ≈ **180 KB**. Well past the base game's authored handful plus a game day of mints plus a quest pack. |
| `CHAIN_MAX_LINKS` | **8** | Bounds `chain()` against a `from` cycle: a merge bug that points two holdings at each other would otherwise be an infinite loop inside a prompt render. Heat alone caps hops at `⌊ln(VH/(s·λ^t)) / ln(HOP_LOSS)⌋` = **13** for a fresh top-band fact (9 at t = 12 gh); the practical bound is the pickup rate — about one hop per `1/(S·c̄·s)` = 4.2 stirs = 2.1 gh at c̄ — so eight is past any chain a game day produces. |
| `PLAYER_CURIOSITY` | **0.35** | Unavoidable: the player has no lore sheet (`assets/world/seed.json`), so `curiosity_of` returns `CURIOSITY_WITHOUT_LORE = 1.0` (`attention.rs:561`) and every roll would clamp to certainty. ≈ 3× the cast mean (0.1193 × 3 = 0.358 → 0.35): an attentive outsider. **And the player's affinity is 1.0 on every topic** (D26) — without that rule their `occupation: None` reads as the no-trade quarter and the effective chance is `0.35 × 1.4 = 0.49`. At 0.35 the player picks up a fresh top-band fact in **≈ 3 stirs** (1/0.35; 1.5 gh) of standing in a ward that is talking about it, and a fresh off-trade `Craft` fact **ever** with probability `1 − exp(−0.042/(1 − λ^½)) = 1 − e^{−1.47} = 77%` (0.042 per stir on an air row cooling under the roll). |
| `PLAYER_POLL_GAME_MINUTES` | **10.0** | Same cadence as the cast's, unjittered — there is one player. |
| `HOUSEHOLD_EPSILON_M` | **0.5** | Below the **measured 1.2748 m** minimum separation between any two of the 413 baked doors, so a false positive is impossible and door equality fires only for generated citizens sharing one `nav::Door` under the occupancy cap. |
| `HOUSEHOLD_DAMPING` (data: `salience.json: household`) | **0.15** | Spec-given (`../02_rumor_pollen.md`, "Damping"). Consequence: a housemate's warm life at hops 1 is `τ·log₂(0.85×0.15/0.119) = 1.19 gh` — they hear it, they barely pass it on. *The last people to hear a scandal are the ones who live with it.* |
| `OCCASION_LIFE_GAME_HOURS` | **1.0** | Shorter than the shortest office (2 gh), so an un-offered occasion cannot survive a bell. It is **not** "longer than any inter-turn gap": off-stage the idle rotation is neighbourhood-gated, and under the `T` key's 60× an hour is 2.5 real seconds — which is why an occasion rendered onto a sheet is `offered` and lives until that exchange's reply lands or fails (D34). |
| `OCCASION_MIN_ASSERTION_CHARS` | **24** | A pre-filter, **not** the gate (the gate is the `holds()` lookup, D34). 24 bytes is past "Aye." and "What of it?" and short of any real assertion. |
| `RAISES_PER_OFFICE` | **1** | Spec-given. |
| `STAGE_HOP_SECONDS` | **2.0** real | Layer 2's scan is O(N) (`world.rs:478`, no spatial index). 0.5 scans/s × 20,000 = **10,000 distance tests/s** against a pump already at 179 ms/frame. Real seconds and not game time on purpose: it is a *legibility* cadence for what the player can watch, not a sim cadence; the roll inside it is keyed on the game-timed stir. |
| `STAGE_HOP_MAX_PAIRS` | **8** | The nearest eight carriers, so a busy square hops a handful of times rather than N². |
| `WARD_CELL_M` | **8.0** | 91 × 104 = **9,464 `u8` cells = 9.2 KB**; **508 cells (5.37%) ambiguous** (measured), so 94.6% of queries are one array index and the rest fall through to the exact 321-mark search — which makes the grid *exact*, not approximate (D23). At 16 m the ambiguity is 10.7% and at 4 m it is 2.8% for 37 KB and 4× the bake; 8 m is the knee. |
| `DOOR_SHUT_REACH_M` | **10.0** | The authored idle leash (`rounds.json` `leash_m: 10.0`, every authored archetype): a person "at home" mills within it, so a smaller threshold finds them "at their door" only by luck (D59). Not a probability. |
| `KNELL_CARRY_M` | **300.0** | The clip's own radius for `SmallvoiceStroke` (`src/soundscape.rs:951-957`); pinned to the clip by a host-side test (M5 T19). |

---

## 4. Both ends, computed

The pack (§7) is planted **at the Dayspring bell** (7:00) — `--start-office dayspring`, the mint at
`now = 0` — and everything below is stated from that phase. It is the best case for the fast end
(the first bell after the mint is HighWick, +5 gh, when the day trades go home) and the **worst**
case for the slow one (every seeded mouth's Dayspring leg fires within 1–6 sim-seconds of the mint).

### Fast end — a `Bed`/`Blood` fact minted at the Wickmarket

Mint ward **Wick, N = 53**. Authored seeded set `K = 4` (§7). `P_hold(t) = 1 − exp(−S·c̄·s·Σ(t))`,
quoted at c̄ and, in brackets, as the population mean over the real curiosities.

```
at the mint bell (Dayspring):   Jonet Kett's leg leaves Wick for the Tallage (Weigh); she crosses
                                the boundary at 0.30 gh and deposits at her next poll (≤ 15 gm)
                                → Weigh's air holds the fact within ≈ 0.55 gh, hops 0, heat ≈ 0.98
by the next bell (HighWick, +5 gh):
    P(a Wick person holds it) = 1 − exp(−2 × 0.1193 × 1.00 × 4.342) = 0.645   [≈ 0.60]
    holders standing in Wick  ≈ 3 seeded + 0.60 × 49 ≈ 32
at the HighWick bell:           70.9% of the cast sleep in a different standing ward from where they
                                work → ≈ 21 holders walk out of Wick within 1–6 sim-s of the bell,
                                each depositing ≤ 15 gm after crossing at heat ≥ 0.85 × λ^5.25 = 0.63
by the second bell (Waning, +8 gh) + 1.5 gh:  the home wards of ≈ 21 people — every ward the
                                Wickmarket's crowd sleeps in
after a game day:               P(a Wick person holds it) = 1 − exp(−2 × 0.1193 × 12.9843) = 0.955   [0.893]
                                crossings out of Wick ≈ 4·X_W·24 + 49·X_W·∫₀^24 P_hold ≈ 99 at c̄ (≈ 90)
```

**Crossings are not wards; deposits are.** The spec's sentence — *being said in the Weigh Ward
within about one office* — is a deposit into Weigh's air. Read that way the fast end is over-satisfied
by one seeded mouth at 0.55 gh, and the honest assertion is about the **wave**, not the outlier:

**Asserted (F1):** `wards_reached(bed) ≥ 2` at the first census sample at or after **HighWick + 1.5 gh**
(6.5 gh after the mint — about one office plus a poll gap), and `≥ 4` at the first sample at or after
**Waning + 1.5 gh** (9.5 gh). `wards_reached` counts **air rows** (D54).
**Asserted (F2):** `wards_reached(bed) == 8` at 24 gh. Saturation is certain by the arithmetic above,
but it is a *branching* process over the ward graph with no honest closed form, so it is asserted
**as a measurement**, not derived.
**Printed, not asserted:** `expected_crossings(bed) = warm_mint_ward_game_hours × X_W / 24` at each
sample, with `X_W` the mint ward's own exit rate from the final snapshot (M2 step 17), beside the
realised `holder_exits`. Both are diagnostics of the model; neither passes or fails a build.

Sanity on the "you can outrun it" half: the Wickmarket to the Shambles well (the nearest Weigh-ward
place) is 224 m — the player walks it in 1.15 gh; the wave reaches Weigh at the next bell, 5 gh out.
That is the fantasy the band exists to protect, and Jonet Kett's early deposit is the reminder that a
single witness with a leg to walk beats the player to it: to be first, be first.

### Slow end — a `Craft` fact minted beside it, same hour, same mouth

`s = 0.20 × 0.60 = 0.12` (off-affinity). `t_warm(0.12, 0) = τ·log₂(0.12/0.119) = **0.145 gh**` (≈ 9
game minutes, about one poll); `t_warm(0.12, 1) = τ·log₂(0.85×0.12/0.119) < 0 → **0.0**` — no
hops-1 carrier is ever warm enough to deposit, so the pickup path contributes **zero** crossings.

```
the boundary:   the nearest foreign-ward boundary from the Wickmarket is ≈ 82 m = 0.26 gh at walking
                pace; Jonet Kett's is 95.4 m = 0.30 gh. Every seeded witness is cold (0.145 gh) before
                any leg reaches one → deposits outside Wick = 0 by construction, not by luck
uniform model:  crossings per game day = K·X_W·min(24, 0.145) = 4 × 0.0987 × 0.145 = 0.057
                P(still confined to Wick at nightfall, 11 gh) = exp(−0.057) = 0.945
inside Wick:    the mint-ward air row (seeded at 1.0 by `install_fact`) lives 79.7 gh; a Wick person
                picks it up within a day with P = 1 − exp(−2 × 0.1193 × 0.12 × 12.98) = 0.31, at hops 1,
                heat 0.85 × λ^t × 0.12 < 0.119 — carriers who can never pass it on
```

**Asserted (S1):** `expected_crossings(craft)` over a game day (the uniform model, off the run's own
integrand) **< 1.0** — computed **0.057** — and, as the realised backstop, `wards_reached(craft) == 1`
at 24 gh: the fact is in its own ward's air and no other.
**Asserted (S2):** `exp(−expected_crossings(craft) at 11 gh) > 0.5` — computed **0.945** — and
`wards_reached(craft) == 1` at 11 gh.

These are the *total*, not a split, because the measurement pack's seeded set is authored to contain
**nobody of the fact's `craft_ear` occupation** (§7). That is the honest reading of "may never leave
it at all": a spoiled batch heard by four market people who are not coopers dies in the lane.

**Reported, not asserted:** the same-trade ear. A `Craft` witness of the subject's own occupation has
`s = 0.20 × 2.00 = 0.40` and a warm life of **21.0 gh**, so *one* such witness contributes ≈ 2.1
crossings a game day. Bounding that would be bounding the feature — the spec says of `Craft` that
"almost all of this topic's reach is affinity… nothing to anyone but a cooper, everything to a
cooper." It is printed per topic as `holder_exits_same_trade` and measured in its **own** scenario
(a second pack whose seeded set is three coopers), so a regression in it is legible as an
affinity-table change and never as a model change.

### A standing fact is answerable and never loud (D52)

`bale.promise` is `Talk` (s = 0.15) and `decays: false`. Its three seeded holders sit at heat 1.0
forever, so with the deposit gate alone they would deposit on every poll and the ward's air would sit
at ≈ 1.0 permanently: per-listener pickup mass `S·c̄·1.0·s·24 = 0.859` a game day → **58%** of a
ward held the hinge of the quest within a day of sharing a ward with a holder. So a standing fact
**never auto-deposits**: it enters a ward's air only when relevance seats it on a speaking turn and
`reheat` puts it there at `REHEAT_TO` (`stir_up`, D28). A ward-mate then picks it up at
`c̄ × 0.1309 × 0.15 = 0.0023` per stir on a row that cools like news — ≈ 0.12 expected new carriers
on the first stir and ≈ 4 over the row's whole 44 gh life in a 53-person ward — every one of them at
hops 1 and heat `0.85 × 0.1309 × 0.15 = 0.0167 < 0.119`, unable to pass it on. The word stays in the
ward it was asked in. That is the quest, in one inequality: `1.00 × 0.15 > 0.119` keeps the three
holders answerable forever, and nothing else about it moves.

### Warm-life table at `VOLUNTEER_HEAT = 0.119`

Game hours from mint that a carrier of a **decaying** fact stays above the deposit gate — i.e. how
long they go on saying it, and how long it may sit on their sheet unasked. Every cell is 0.59 gh
below the 0.115 draft's.

| Band | base | affinity | s | witness | hops 1 | hops 4 |
|---|---|---|---|---|---|---|
| `Bed`, `Blood` | 1.00 | 1.00 | 1.00 | 36.9 | 34.0 | 25.6 |
| `Law`, `Omen`, `Stranger` | 0.80 | 1.00 | 0.80 | 33.0 | 30.2 | 21.7 |
| `Coin` | 0.45 | 1.00 | 0.45 | 23.0 | 20.2 | 11.8 |
| `Bread` | 0.35 | 1.00 | 0.35 | 18.7 | 15.9 | 7.4 |
| `Craft`, own trade | 0.20 | 2.00 | 0.40 | 21.0 | 18.2 | 9.7 |
| **`Craft`, any other trade** | 0.20 | 0.60 | 0.12 | **0.145** | **0.00** | 0.00 |
| `Talk` (a decaying claim) | 0.15 | 1.00 | 0.15 | 4.0 | 1.2 | 0.00 |
| `Bed` × laundress/servant | 1.00 | 1.60 | 1.60 | 45.0 | 42.2 | 33.7 |
| `Bed` × the no-trade quarter | 1.00 | 1.40 | 1.40 | 42.7 | 39.9 | 31.4 |
| `Bed` × the subject's household | 1.00 | 0.15 | 0.15 | 4.0 | 1.2 | 0.00 |

### A cold scandal out-travels a fresh squabble

| | pickup chance ∝ | deposit gate |
|---|---|---|
| `Bed` at heat 0.30 | `0.30 × 1.00` = **0.300** | 0.300 > 0.119 ✓, 16.0 gh of warm life left |
| `Craft` (off-trade) at heat 1.00 | `1.00 × 0.12` = **0.120** | 0.120 > 0.119 ✓ (barely), 0.145 gh left |

**2.50× on pickup, and over a hundred times on warm life** (16.0 gh of repeating left, against 0.145
gh). The single assertion that salience is not heat, and it holds with margin rather than by a
whisker.

### The clamp never binds

The worst per-roll chance anywhere in the cast is a max-curiosity no-trade pauper on a fresh
top-band fact: `0.392 × 1.00 × 1.40 = **0.549**`. The ×1.6 Bed ear is occupation-based, so nobody
gets both it and the ×1.4 no-trade multiplier; the highest that path reaches on the cast is
`0.292 × 1.6 = 0.467`. So `.clamp(0.0, 1.0)` in the spec's formula never activates, the roll stays
linear, and every closed form above stays valid — which is exactly what makes a retune a division and
not a search. Asserted: `the_pickup_chance_never_clamps`.

---

## 5. The flat-table identity, in its strongest form

`SalienceTable::flat()` sets every base and every affinity to 1.0 (hedge bands untouched, D19;
`household` too, bypassing the loader's `< 1.0` rule on purpose), so the roll reduces
arithmetically to `curiosity_of × air.heat` — **the model as it would be with the salience term
removed from the roll entirely.**

That is what the identity is asserted against, and the distinction matters: a per-topic baseline
checked in alongside the bands cannot detect a band that was already wrong when the baseline was
taken. So:

- `--pollen-no-salience` runs with the third factor **deleted from the expression** — `salience_for`
  returns a bare `1.0` (one `if` in one function, `knowledge/salience.rs`), and
- `--pollen-flat` runs with `SalienceTable::flat()`,

and `pollen_flat_reproduces_the_no_salience_run` asserts the two runs' `PollenCensus` are **equal
field for field** — same carriers, same crossings, same wards reached, same store bytes, for the same
seed, the same clock and the same fact pack. Not "close": equal. `flat()` is a multiplication by one,
and a multiplication by one that changes a number is a bug in the roll.

**Why the self-subject rule had to leave `salience()` (D51).** As first written, `salience()`
returned 0.0 for the subject and `--pollen-no-salience` returned 1.0 for everyone — so under the
lever a subject picked up their own fact (the shipped `vell.stall.pitch`'s `dv8ll` never leaves
Wick; the pack's subject `p0043` stands in the mint ward) and under `flat()` never did, and the
identity failed **by construction** on the shipped content. `may_carry` sits outside the product and
holds under both levers; `the_subject_never_carries_under_either_lever` pins it.

The deposit gate also has to survive flattening: at `s = 1.0` for every topic, `t_warm` is 36.9 gh
for all nine, so the flat run's cadence is one speed for all news — which is the pre-salience model,
stated as the thing being reproduced.

---

## 6. Store footprint

`Holding` (interned and flattened): `key` u32 4 + `hops` u8 1 + `heat_at_learn` f32 4 +
`learned_on` Option\<f64\> 16 + `from` Option\<ActorId\> 24 + `view` (Option\<ActorId\> 24 +
Option\<AreaKey\> 4 + i8 1 → 32) = 81 → **88 bytes** (the three 8-aligned fields round it **up**, not
down to 80), plus `from`'s 5-char `String` on the heap (≈32 B with allocator overhead) ≈ **120 B**,
and `view.subject` is `None` unless the fact's mask moves the subject (≈+32 B when it does).

```
per actor at the cap : 6 × 120 = 720 B + Vec header 24 + BTreeMap entry (ActorId 24 + heap 32 + node share ~50) ≈ 850 B
at --extra-ambient 20000 (20,519 bodies, all at the cap)  ≈ 17.4 MB
air    : 8 wards × 24 × ~104 B                            ≈ 20 KB
live   : 256 facts × ~700 B                               ≈ 180 KB
ward grid (one static, whole process)                      = 9.2 KB
```

**Asserted:** `size_of::<Holding>() <= 88` (M3 T23); per-actor holdings never exceed `HOLDINGS_MAX`,
eviction order is deterministic (coldest, then most hops, then `FactKey`); and
`Knowledge::footprint_bytes() ≤ 32 MB` at `--extra-ambient 20000` (`the_store_footprint_is_bounded`,
M2, on a synthetic 20,519 bodies; M5 T16 re-runs it on the real crowd). `footprint_bytes()` is
`size_of` arithmetic, so the 32 MB test asserts the formula against itself — which is why M2 step 20e
also records the **peak-RSS delta** with and without `CATHEDRAL_NO_KNOWLEDGE` at 20,000 in
`m2_measurements.md` and checks the 32 MB bound against *that* number in the verification script.

**CPU.** Per person per poll: one ward lookup (94.6% an array index) + at most `AIR_PER_WARD_MAX`
rolls, each a `DefaultHasher` over three small values, two `BTreeMap<String>` lookups
(`holds_key`, `characters`), a lore read and an ear scan — **1–2 µs realistic**, not 200 ns.
At 20,000 bodies, 6 polls/gh and 1 gh = 150 real s: `20,000 × 6 / 150 = 800` person-polls/s × 24 =
**19,200 rolls/s ≈ 20–40 ms/s of CPU** (2–4% of one core), inside the 15% budget. Plus Layer 2's
gated scan: 10,000 distance tests/s. **Measured saturated, not assumed**: every 20,000 guard plants
the pack and `--pollen-saturate`s it (nine rows in every ward's air), because a run with two authored
facts and ≤ 2 rows per ward rolls almost nothing and guards nothing.

**The reverse index, decided here and not in a code comment.** `poll_person`'s deposit phase walks
`live` through `holdings_of` to find the seeded rows: O(facts) per poll — at `FACTS_MAX_LIVE = 256`
and 800 person-polls/s that is ~205,000 `BTreeSet::contains` a second on sets of 2–20 short ids,
≈ 10 ms of one core. **M2 ships without the index.** If step 20e's saturated wall-clock ratio exceeds
15%, the fix is `seeded_by_actor: BTreeMap<ActorId, Vec<FactKey>>` on `Knowledge`, maintained in
`install`/`invalidate` — named here so nobody invents a different one, and not added without that
number.

---

## 7. The measurement scenarios

`--pollen-seed <PLACE>` plants an authored pack — **not shipped content** — of one fact per topic at
one named place, all with the **same** `seeded` set, because the spec's slow end is "minted beside
it, at the same hour, by the same mouth". Planted at the Dayspring bell (`--start-office dayspring`,
minted at `now = 0`). Three packs, in `crates/cathedral-sim/tests/fixtures/pollen/`:

| Pack | Contents | Used for |
|---|---|---|
| `cadence_band.json` | 9 facts, one per topic, `seeded` = the same 4 named Wickmarket people, `craft_ear` set to an occupation **none of the four holds**, subject a Wick stander with that occupation (`p0043`) | F1, F2, S1, S2, the per-topic print |
| `craft_ear.json` | the `Craft` fact only, `seeded` = 3 coopers, `craft_ear = "cooper"` | the reported same-trade term |
| `household.json` | one `Bed` fact whose subject has **≥ 2 kin present** and a door, `seeded` = 4 people outside the household | "the household is last" (asserted on the fraction holding at every sample, M2) |

Why an authored pack and not the coded mints: M2's two mints are both `Law` (D32), and two `Law`
facts cannot measure nine bands. Controlling the topic, the mint ward, the hour and the seeded set is
the only way any of these numbers means one thing.

---

## 8. Reproducing the measured inputs

Every cadence run is at the **shipped clock** (`--seconds-per-day 3600 --start-office dayspring`)
and `--trace-pollen` steps the watch loop at **0.4 s** (the most walking one poll can realise, D22;
`--pollen-step` overrides it). At `--seconds-per-day 120` nobody can finish a commute inside a game
day and the band is unmeasurable — never measure it there.

```sh
# c̄, the curiosity distribution — walk derived_curiosity over lore/characters/**
cargo test -p cathedral-backends --test pollen_cadence the_measured_curiosity_mean -- --nocapture

# standing ward populations, the ward grid's size and its ambiguous-cell count
cargo test -p cathedral-sim --test knowledge_tests the_ward_grid -- --nocapture

# X — city mean and the per-ward exit rates, off the real round (nothing planted)
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --watch-clock 1 --seconds-per-day 3600 --start-office dayspring \
    --trace-pollen --pollen-per-day 48

# both ends of the band, per topic
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --watch-clock 1 --seconds-per-day 3600 --start-office dayspring \
    --trace-pollen --pollen-per-day 48 --pollen-seed "The Wickmarket"

# the identity run, and the baseline it must reproduce — byte-identical commands but for the lever
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --watch-clock 1 --seconds-per-day 3600 --start-office dayspring \
    --trace-pollen --pollen-per-day 48 --pollen-seed "The Wickmarket" --pollen-no-salience \
    | grep '^\[pollen\]' > /tmp/pollen_base.txt
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --watch-clock 1 --seconds-per-day 3600 --start-office dayspring \
    --trace-pollen --pollen-per-day 48 --pollen-seed "The Wickmarket" --pollen-flat \
    | grep '^\[pollen\]' > /tmp/pollen_flat.txt
diff /tmp/pollen_base.txt /tmp/pollen_flat.txt          # must be empty

# cost and footprint at the crowd knob's ceiling — SATURATED, and at the coarse step
# (crossings are not what this measures; 9,000 polls at 179 ms each would be 27 minutes)
for n in 0 1000 20000; do
  /usr/bin/time -v cargo run --release -p cathedral-backends --bin cathedral-headless -- \
      --fake --extra-ambient $n --watch-clock 1 --seconds-per-day 3600 --start-office dayspring \
      --trace-pollen --pollen-step 3 --pollen-seed "The Wickmarket" --pollen-saturate 2>&1 | tail -30
  CATHEDRAL_NO_KNOWLEDGE=1 /usr/bin/time -v cargo run --release -p cathedral-backends \
      --bin cathedral-headless -- --fake --extra-ambient $n --watch-clock 1 \
      --seconds-per-day 3600 --start-office dayspring 2>&1 | tail -5
done
```

---

## 9. Asserted vs reported

**Asserted** — a failing number fails `cargo test --workspace`. The band is **asserted at
`--extra-ambient 0`** only: c̄, X_W and the four mouths are the authored cast's, and the generated
crowd changes all three. At 1000 and 20000 the same census is **printed** into `m2_measurements.md`
and only the cost guard is asserted there.

| # | Assertion | Computed |
|---|---|---|
| F1 | `wards_reached(bed) ≥ 2` at the first sample ≥ HighWick + 1.5 gh (6.5 gh), `≥ 4` at the first sample ≥ Waning + 1.5 gh (9.5 gh) | ≈ 21 holders cross at the HighWick bell |
| F2 | `wards_reached(bed) == 8` at 24 gh | measured |
| S1 | `expected_crossings(craft)` over a game day < 1.0, and `wards_reached(craft) == 1` at 24 gh | 0.057; 1 |
| S2 | `exp(−expected_crossings(craft) at 11 gh) > 0.5`, and `wards_reached(craft) == 1` at 11 gh | 0.945; 1 |
| — | a cold `Bed` at heat 0.30 reaches more wards over an interval than a fresh off-trade `Craft` at heat 1.00 | 2.5× on pickup, >100× on warm life |
| — | `--pollen-flat` equals `--pollen-no-salience`, field for field | identity |
| — | the roll count is invariant under the time scale (`seconds_per_day` 120 vs 3600 over one game-hour span: same `rolls_per_game_hour`, same `stir` on every row) | identity |
| — | `POLLEN_POLL_MAX_GAME_MINUTES < 60 / STIRS_PER_GAME_HOUR`, bare and composed with the host step | 15 < 30; 15.16, 22.2, 16.2 < 30 |
| — | `HOP_LOSS⁴ > VOLUNTEER_HEAT` and `HOP_LOSS × 0.12 < VOLUNTEER_HEAT` | 0.522 > 0.119; 0.102 < 0.119 |
| — | max per-roll chance over the whole cast < 1.0 (the clamp never binds) | 0.549 |
| — | `reheat` never yields heat above `REHEAT_TO`, and `REHEAT_TO ≤ 0.15` | 0.1309 |
| — | a standing fact seeded to three has zero non-seeded holders after 24 gh of ticking, and ≤ 1 after one relevance re-heat's first stir | 0 ; ≈ 0.12 |
| — | `size_of::<Holding>() ≤ 88`; `footprint_bytes() ≤ 32 MB` at 20,519 bodies; cap and eviction order enforced | 88; ≈17.4 MB |
| — | the ward grid agrees with the exact search at 10,000 pseudo-random city points **and** outside the box | exact by construction |
| — | at every census sample, the fraction of the subject's household holding the `Bed` fact ≤ the fraction of the rest of the city holding it, and the subject never holds it | — |
| — | goldens byte-unchanged from the M1 bless, at every milestone from M2 on | — |

**Reported** — printed by `--trace-pollen`, recorded in `m2_measurements.md` / `m5_measurements.md`,
never asserted:

- `expected_crossings` and `holder_exits` per topic at each sample (the model's own numbers beside
  the realised ones), and `X` (city mean) beside every ward's exit rate;
- `Craft` same-trade exits (≈ 2.1/day per cooper witness) — bounding it would bound the affinity table;
- per-ward carrier counts and mean curiosity per ward — the standing wards are 7:1 lopsided, so every
  cadence figure is stated per ward and never as a city mean;
- the census at `--extra-ambient 1000` and `20000`, and the p50/p99 frame time and pump time at 0 / 1000 / 20000;
- the sheet's byte cost with and without the block, against the 64 KiB canary;
- mean hops and max heat per topic at each sample.

---

## 10. If a number misses

1. **Fast end out of band?** `STIRS_PER_GAME_HOUR_new = 2.0 × target / measured`. Then re-check
   `POLLEN_POLL_MAX_GAME_MINUTES < 60/S` and, if it now fails, scale `POLLEN_POLL_GAME_MINUTES` by
   the same factor. Nothing else moves. (F1's bells do not move: they are the city's.)
2. **Slow end out of band?** Re-solve from the boundary: `VOLUNTEER_HEAT_new = 0.12 / 2^(t_b / 12)`
   with `t_b = d_b / (WALK_SPEED_MPS × seconds_per_day / 24)` game hours and `d_b` the nearest
   foreign-ward boundary from the mint place (re-measure `d_b` if the pack moved or the ward marks
   changed). Then re-read the warm-life table: `Talk` must stay above zero (`0.15 > VH`) or a
   decaying `Talk` claim never moves at all, which is a *different* design; and re-check the
   `HOP_LOSS` brackets.
3. **Both out, in the same direction?** `AIR_HALF_LIFE_GAME_HOURS` scales both, since every warm life
   and every `Σ(T)` is linear in `τ`. Move it, then re-solve `VOLUNTEER_HEAT` from the slow end and
   re-check the fast end. Re-derive; do not fiddle.
4. **Never a salience band, and never an affinity.** The flat-table identity is asserted against them,
   and moving one invalidates M2's baseline without failing a test that says so.
5. **Never the clock.** The band is defined at `seconds_per_day: 3600`; a run at another clock
   measures a different city (D22).
6. Whatever moves, the substitution — not just the result — is written into
   `features/knowledge_and_rumor/README.md`'s `## Numbers` section and appended to this file as
   `## Retunes taken`, so nobody re-derives it and nobody quotes a stale figure.
