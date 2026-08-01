# Chalking the Walls

**Status:** M0 and M1 done (2026-08-02), M2–M4 not started. Branch `slunga/chalking_the_walls`.
§2.7's snapshot measurement is done and recorded below; §0 records sixteen corrections to this spec
found by re-checking every seam against the tree. Milestone state is tracked in §4 — each
milestone's heading carries its own status and an "As built" note where the build diverged.

Every file, function and line reference below was checked against `develop` on 2026-08-01; where the
reference is approximate the text says so. Coordinates, if any appear, are current (post-shrink)
world coordinates.

Stigmergy made touchable. Rule-driven hands chalk marks on the city — a cross on a debtor's door, a
tally at a well, a ward-sign at a shrine — and cheap rules read them back: a chalked buyer is
refused at the stall, a heavily-notched well loses its queue to the next one, a chalked shrine
draws tomorrow's ambient evening crowd. **The environment becomes the database, and the player can
tamper with the medium instead of the agents.** Scrub a cross at night and that household can buy
bread again tomorrow. Forge one on somebody you dislike and the ward's own rules turn on them,
because nothing that reads a mark ever asks who drew it.

Everything is chalk. One medium, many signs; the vocabulary lives in the mark *kinds*, not in a
spread of materials. The whole loop is zero-token: marks are written by code, read by code, and
reach the LLM only as one already-paid-for line on a turn that was going to happen anyway.

---

## 0. Corrections to this spec (verified 2026-08-02)

§1 caught one wrong assumption before implementation started; a full re-check of every seam caught
sixteen more. **Read this section before §1** — several of these change the design, not just a line
number. Each was verified against the tree at `1bb4881`.

### Design-changing

| # | The spec says | The truth | Consequence |
|---|---|---|---|
| C1 | `marks.json`'s "loader takes `&str`, never a path (the host reads the file) … exactly as `items.json` owns item kinds" | **`items.json` has no host call site.** It is `include_str!`-embedded in the sim (`item.rs:51`), as are `food.json`, `rounds.json`, `homes.json`, `places.json`. The read-a-path convention belongs to `shelters.json`/`areas.json`/`seed.json`, and even that is inconsistent (the *game* host embeds shelters, the *headless* host reads it). | `marks.json` is `include_str!`-embedded in `marks.rs`. One line, both hosts, no chance of a headless/game divergence. |
| C2 | `MarkId(pub u64)` "allocated from a counter, as `Notices` does" | Right about the counter, but there **is no `NoticeId`** — notice ids are bare `u64`. And `ids.rs`'s `id_newtype!` macro makes **`String`** newtypes, so it cannot be used here. | `MarkId` copies `RequestId(pub u64)` (`ids.rs:107`) and gets `Display` + serde by hand. |
| C3 | a restitution notice "not settled" | There is **no `settled` field**. `Notices::settle` *removes* the notice from the vec. | "not settled" = "still returned by `Notices::live()`". The spec's "settling stops the re-chalk" then costs **zero code** — a settled notice is simply gone from the iteration. |
| C4 | the cross's writer goes "in `notices.rs`, at the existing age check" | The age check is `Notices::expire`, whose `&mut self` is on **`Notices`, not `World`** — no path to `World.marks`, `.places` or `.characters`. It also runs every poll (~60 Hz), not daily. | The writer is a new free fn in `notices.rs` taking `&mut World`, called from `engine.rs:1157` beside the existing `notices::confront(&mut self.world)`. |
| C5 | a "household door" anchor "resolves via `homes.rs`" | `homes.rs` exposes **no per-actor lookup** — just a raw map and a bulk `place_descriptions()`. | The anchor resolves through `PlaceRegistry::home_of(&ActorId) -> Option<&PlaceEntry>` (`places.rs:236`). `PlaceEntry` carries `point` *and* `ward`. |
| C6 | `WardNotice.place` is "a place" the cross can use | It is **free prose** ("the tenter-frames"), sometimes straight from an LLM's `where` argument. It cannot be resolved to a registered place or a nav point. | Never anchor on it. The accused's *home* is the anchor. |
| C7 | the tally reader is "a term added inside [`nearest_staffed_source`'s] comparison. One function." | **Fatal as written.** There are only **two** callers, both enrolment-time, and `Townsperson.source` is written once for an actor's whole life and never reassigned. The per-trip site (`Decision::ApproachWell`, `round.rs:7633`) reads the *already-bound* index and never re-picks. A penalty inside the function is evaluated at world-seed t=0, when no chalk exists — **it would move nobody, ever.** | M4's tally must re-pick at the per-trip site. See §4 M4 as amended. |
| C8 | `chalk_ward_sign {"place": "<shrine>"}`, "naming a shrine in that ward" | **There are no shrines.** `places.json` has no `kind: "shrine"` and no place whose name contains "shrine"; `PlaceEntry` has no `kind` field at all. Three of the eight wards have nothing devotional to name. | The ward-sign's vocabulary is an authored allow-list in `marks.json` — which is where the spec already says the vocabulary belongs. **Do not re-bake `places.json`**: the bake renumbers nodes and silently rots every pinned index. |
| C9 | put the refusal stamp on `try_purchase` / guard `nearest_open_stall` | Neither `try_purchase` nor `nearest_open_stall` nor its caller `decide` has a `now` or a clock to compare a deadline against. | Copy the existing `lightning_reflex_until` idiom: prune the stamp set in `round::tick` (which has `now`), and membership-test with `contains_key` at the guard. |
| C10 | `MARKS_MAX` "start at 256" | Measured: **26,815 bytes of snapshot headroom** (§2.7), and a quantized wire record of **100.1 B/mark**. 256 marks is 25,610 B — which *does* fit, by 1,205 bytes. The spec guessed it would overflow; it does not. What it does is spend 96% of the headroom the project has left. | **`MARKS_MAX = 100`** → 10,010 B added, 16,805 B still free for the `chalk_pen` item wave and whatever comes after. Chosen because it leaves room, not because 256 overflows. |
| C11 | the 160 KiB assertion is the guard on `MARKS_MAX` | **It will never fire.** That test builds its world from `seed.json` and never runs a round, so it contains zero marks; the printed number stays 137025 whatever `MARKS_MAX` is. | A second assertion in the same test plants `MARKS_MAX` *worst-case* marks and re-encodes. Without it the cap is decorative. |

### Fact-changing but not design-changing

| # | The spec says | The truth |
|---|---|---|
| C12 | a `{{ dogs_nearby }}`-style placeholder in `turn.j2` | `turn.j2` has exactly three interpolations (`gaol_fee`, `emittable_sounds`, `sheet_md`). Every section is built in **Rust** by `sheet_markdown()` and arrives as one opaque blob; the template only gates prose paragraphs and verb lines. A `marks_here` section is seven coordinated edits in `prompt/mod.rs`, none in the `.j2`. |
| C13 | "unknown-people rules apply through `perception::identify`" | The sheet does **not** use `perception::identify` — it uses a separate duplicated rule in `prompt::person()` (`prompt/mod.rs:471`) rendering `"a stranger (you don't know their name)"`. Two stranger spellings exist by design; the sheet one is the right one here. |
| C14 | — | `strings.toml` is `deny_unknown_fields`. A new string and its `PromptStrings` field must move together or startup hard-errors. |
| C15 | the look-at HUD line hooks into what exists | A full crosshair→HUD pipeline exists (`src/smart_actors/targeting.rs`) but `update_actor_focus` only ever hits entities with an `ActorId`. A mark has none, and faking one would poison the offer path, gaze and the actor sheet. M3 needs a **second** focus system. Separately, **no press-and-hold input pattern exists** anywhere in the tree. |
| C16 | `puddle_mesh` at `weather/render.rs:184`; drive parsing at `drive.rs:364–411`; thirst gate at `round.rs:1044`/`:5958` | `render.rs:184` is the *call site*; the fn is at `:1013`. Drive parsing is `drive.rs:322–413`. The delta gate is `round.rs:5951–5957`. For a *discrete* once-per-game-minute tick the repo's idiom is the **crossing** form (`tick_production`, `round.rs:4805`), not the delta form. |

Two further notes that cost nothing but save an hour: the dogs' authored data is a Rust `const` array,
not JSON at all (legitimate precedent if a catalog is small); and dogs reach the world through
`EngineConfig`, **not** `WorldConfig`.

---

## 1. What already exists — and the one thing that does not

Verified seams. Read these before writing anything; most of this feature is wiring, not invention.

| What | Where | Why it matters here |
|---|---|---|
| Restitution notices with an accused, a place and an age | `crates/cathedral-sim/src/notices.rs:137` `place: Option<String>`, `:144` `raised_game_days: Option<f64>`, `:148` `accused: Option<ActorId>` | The cross's writer needs exactly these three fields. They are already there. |
| A **code-driven, zero-token stall purchase** | `crates/cathedral-sim/src/round.rs:6398` `fn try_purchase(…) -> Option<Sale>`; its caller's graceful `None` arm at `:6337` | This is the credit gate. A refusal here costs nothing and shows up in the round trace. |
| Stall selection as a `continue` chain with an early-return guard | `crates/cathedral-sim/src/round.rs:6561` `fn nearest_open_stall(…)` — note the bound-vendor guard at its top | The shape to copy for M1's anti-re-queue stamp. |
| `nearest_staffed_source` — the well the thirsty walk to | `crates/cathedral-sim/src/round.rs:2184`, a `min_by` over staffed sources by distance, tie-broken by source index | The tally reader is a term added inside this comparison. One function. |
| `reroll_ambient_evenings` — the ambient cast's zero-token evening | `crates/cathedral-sim/src/round.rs:1283`, picks between `TAVERNS` (`round.rs:167`) by `hash01("night_tavern", id, day)` | The ward-sign reader extends this destination list. |
| Authoritative weather **inside the sim** | `crates/cathedral-sim/src/weather.rs:140` `WeatherSample { precipitation, surface_wetness, … }`, held as `World.current_weather` (`world.rs:117`) | Weather-coupled decay needs no Bevy round-trip and works headless. |
| Authoritative **shelter** inside the sim | `weather.rs:1469` `Shelter { polygon_xz, cover, … }`, `ShelterMap::is_sheltered(position) -> bool` (`weather.rs:1589`), held as `World.shelters` (`world.rs:120`). `assets/world/shelters.json` is loaded by **both** hosts: `cathedral_headless.rs:1001` and `src/smart_actors/local_engine.rs:341` | "A sheltered mark holds for days" is a one-call query, not a new data set — and it works headless. A hand-built test `World` has an empty `ShelterMap`, so a decay unit test must construct one explicitly. |
| A sim-owned, non-cognitive layer with its own prompt section | `crates/cathedral-sim/src/dogs.rs` + `World.dogs` (`world.rs:156`) + `**dogs_nearby**` (`prompt/mod.rs:1418`) + `has_dogs` (`prompt/mod.rs:522`) | **This is the module to copy.** `marks.rs` is `dogs.rs` with persistence. |
| A conditional verb sheet | `assets/prompts/turn.j2` — `has_pockets`, `has_law_verbs`, `has_settle_verb`, `has_custody`, `sounds_enabled` all gate both the instructions and the verb line | Precedent for showing `draw_mark` only to someone holding a pen. |
| The witness seam | `actions.rs:1394` `fn world_event_witnessed(…, witnesses: Vec<ActorId>)`, paired with `Engine::nudge_pocket_witness` (see `actions.rs:1392`) | Scrubbing and forging in front of people goes down this exact path, unchanged. |
| Homes with a point and a description | `crates/cathedral-sim/src/homes.rs:29` `homes: HashMap<String, HomeEntry>`, `:38` `point: [f64; 2]`, `:40` `place_description` | A "household door" anchor resolves here. |
| Wayfinding places | `World.places` (`world.rs:130`), `crates/cathedral-sim/src/places.rs`, `PlaceRegistry::named(name) -> entry { name, point }` | Well and shrine anchors resolve here. |
| Named geography | `World.area_map` (`world.rs:79`), `assets/world/areas.json` | Lane anchors, if a later child wants them. |
| The golden prompt fixtures | `crates/cathedral-sim/tests/golden_prompts.rs:225` `fn regenerate_golden_fixtures()`, `#[ignore]` | Any sheet change goes through this, deliberately. |
| Drive-mode action parsing | `src/drive.rs:364–411` — the `"sound"` / `"bell"` / `"status"` / `"seize"` / `"frame"` arms | Where `chalk` and `scrub` actions get added. |

### The thing that does not exist

**There is no non-LLM "unpaid sale / credit" path in `actions.rs`.** The earlier sketch of this
feature assumed one. Selling between characters is LLM-mediated: a bound vendor carries a
`you_sell` price list on their sheet (`prompt/mod.rs:266`, `:1371`) and goods move by
`offer_item` / `accept_offered_item`. There is nothing there to gate.

The gate that *does* exist is `round::try_purchase` — the in-code stall sale the bread round runs
for NPC buyers with real wallets and real stock. **That is the cross's hard reader.** Do not invent
a credit system in `actions.rs`; do not try to make the LLM sale path refuse in code. The LLM half
gets the mark as a prompt line and decides for itself, which is correct and free.

A consequence to accept rather than fight: **the player never goes through `try_purchase`**, so a
cross on the player's own door does not mechanically refuse the player anything. The player's side
of this feature is *drawing and scrubbing other people's marks*. Being marked yourself is the
"marks as the mute witness" child in §7, not this task.

---

## 2. Binding decisions

### 2.1 A mark is authoritative, and each behaviour reads exactly one source

This is the feature's whole load-bearing risk. The sim already holds a social database — ward
notices, memories, ward moods. If marks merely *project* it, scrubbing changes nothing and forging
is placebo. So marks are authoritative state, and the partition is absolute:

**Stall refusal reads the chalk and never the notice.** `try_purchase` must not consult
`World.notices`. The notice's only role is to be the *reason a hand chalks*, once, in §4 M1.

The contradiction that remains is deliberate and diegetic: a shopkeeper who remembers you anyway is
characterization; the ward re-chalking a scrubbed cross from a still-live notice (§4 M1) is the
ward repairing its own database, and it is the visible answer to "which source wins."

### 2.2 You cannot chalk a blank wall

A mark anchors to a **handle the city already has** — a household, a registered place, an area —
never to a free coordinate. This is not a limitation to work around; it is what makes the sim clean
(no new geometry ownership), the readers cheap (a reader looks up its own anchor, never sweeps
space), and the Bevy render trivial (the anchor already knows where it is).

Drawing therefore resolves to *the nearest eligible anchor within reach*, and fails with a plain
reason when there is none. "There is nothing here to chalk" is a good failure.

### 2.3 Readers never look at `author`

`Mark.author` is recorded for the mute-witness child and for the trace. **No reader may branch on
it.** A forged cross refuses a stall exactly as hard as a magistrate's, and survives until it
weathers off or somebody scrubs it. If you find yourself writing `if mark.author.is_none()`, you
have broken the feature.

### 2.4 Marks are drawn, never collided, never baked

Same trap as the rats and the Cut kerb: `scripts/bake_navigation.py::build_walkable` erodes every
exported collider footprint, so anything with a collider punches holes in the walkable surface. A
chalk mark has **no entry in `CollisionWorld`, ever**, and `collision_footprints.json` must stay
byte-identical after this feature. Assert that in review.

### 2.5 Everyone over seven can read

No literacy state, no learned-kinds set, no tooltip unlock. A mark's meaning is plain to anyone who
looks at it, player included. The HUD says what it means (§4 M3); the prompt says what it means
(§4 M0). Ward-specific meanings are a child (§7), not a v1 subtlety.

### 2.6 The pen gates the verb, never the rule

`draw_mark` requires a chalk pen in hand. The **system writers do not** — the ward's hand needs no
inventory, exactly as `raise_notice` needs no parchment. Losing the pen mid-scene removes the verb
from the next sheet; it never retroactively erases a mark.

`scrub_mark` needs no pen (a wet sleeve is a wet sleeve) but does need a mark within reach.

### 2.7 Bounded: the count, the sweep, the snapshot

- `MARKS_MAX` (start at 256). At the cap, drawing evicts the **faintest** mark, and logs it. A
  player with a pen and an afternoon must not be able to grow world state without limit.
- Decay sweeps **at most once per game-minute**, not per 60 Hz poll — gate it on an accumulated
  game-days delta the way `round.rs` gates thirst decay (`round.rs:1044`, `:5958`).
- `MARKS_MAX` is **a number you must measure, not the 256 suggested above.** `World.marks` reaching
  the game grows `PublicSnapshot`, and `full_roster_prompts_and_public_snapshot_remain_bounded`
  (`crates/cathedral-backends/src/world_data.rs:383`, assertion at `:433`) caps it at 160 KiB —
  a bound already *raised* from 128 KiB by the lore-item wave, so the remaining headroom is at most
  32 KiB and probably less. At a plausible ~150 serialized bytes per mark, 256 marks is ~38 KiB and
  **may not fit.**

  So the very first thing to do in M0: make that assertion print the current size (it only prints on
  failure today), run it, and write the measured number into this file. Then size `MARKS_MAX` to the
  real headroom, and prefer shrinking what a mark carries over the wire — the render needs a point,
  a kind, a strength and an orientation, not the author or the prose label — over raising the bound.

  **MEASURED 2026-08-02** (`world_data.rs` now prints this unconditionally, not only on failure):

  ```
  public snapshot: 137025 bytes of the 160 KiB bound (26815 bytes headroom);
  longest prompt 13425 bytes for ar5tl
  ```

  So the headroom is **26,815 bytes**, not the "at most 32 KiB" the paragraph above guessed. The
  suggested `MARKS_MAX = 256` was then measured too, rather than estimated at "~150 B/mark":

  ```
  public snapshot with 100 marks: 147035 bytes (10010 added, 100.1 B/mark)
  public snapshot with 256 marks: 162635 bytes (25610 added, 100.0 B/mark)
  ```

  256 **fits**, by 1,205 bytes — the paragraph above was wrong to predict an overflow. But it
  spends 96% of the headroom the project has left, so `MARKS_MAX` is **100**: chosen because it
  leaves 16,805 bytes for the `chalk_pen` item wave and whatever follows, not because 256 breaks
  anything. A canary in the same test fills `World.marks` to the cap with worst-case records and
  re-asserts the bound, so the number is enforced rather than trusted — without it the assertion
  never sees a mark at all, since its world is built from `seed.json` and never runs a round.

### 2.8 Refusals and percepts do not buy provider calls

The rats M2 lesson, and it applies directly: a percept that takes the priority lane hands somebody
a paid turn. A stall refusal, a weathered-off mark and a re-chalk therefore write an **inbox line
only** — no `nudge`, no priority slot. The refused buyer mentions it on their next natural turn.

The one exception is the witness of a *deliberate act* (§4 M2): somebody scrubbing or forging in
front of you is a social event, and it takes the same nudge `pocket_item` already takes.

### 2.9 Config and ablation

A `marks:` block under `smart_actors` in `config.ron`, defaulting on, with per-kind switches
(`cross`, `tally`, `ward_sign`) and a `decay_scale` for testing. Plus `CATHEDRAL_NO_MARKS=1` to
ablate the whole layer, matching `CATHEDRAL_NO_ACTORS` / `CATHEDRAL_NO_WEATHER`.

---

## 3. The model

New module `crates/cathedral-sim/src/marks.rs`, wired into `world.rs` beside `notices` and `dogs`.

```rust
pub struct MarkId(pub u64);          // allocated from a counter, as Notices does

pub enum MarkKind { ChalkCross, WellTally, WardSign }

pub enum MarkAnchor {
    Household(ActorId),   // that person's home door — resolves via homes.rs
    Place(String),        // a PlaceRegistry entry: a well, a shrine, a stall
}

pub struct Mark {
    pub kind: MarkKind,
    pub anchor: MarkAnchor,
    pub about: Option<ActorId>,   // derived from the anchor at draw time, never passed in
    pub author: Option<ActorId>,  // None = the ward's own hand; never read by a reader (§2.3)
    pub drawn_game_days: f64,
    pub strength: f64,            // 1.0 fresh
    pub strokes: u32,             // tallies accumulate; other kinds stay at 1
}

pub struct Marks { /* BTreeMap<MarkId, Mark> + next_id */ }
```

One resolver, and only one, turns an anchor into a site:

```rust
pub fn anchor_site(world: &World, anchor: &MarkAnchor) -> Option<AnchorSite>
// AnchorSite { point: Vec3, label: String, occupant: Option<ActorId> }
```

`Household` resolves through `homes.rs` (point + `place_description`, occupant = the actor);
`Place` resolves through `World.places.named(...)`. An anchor that resolves to nothing is a mark
that quietly ceases to exist on the next sweep — buildings and bindings change, and a dangling mark
must never panic or leak.

### `assets/world/marks.json`

The catalog owns every string and every number, exactly as `items.json` owns item kinds. Loader
takes `&str`, never a path (the host reads the file — see `crates/cathedral-sim/AGENTS.md`).
Schema version 1, `deny_unknown_fields`. Per kind:

- `label` — what a reader sees: `"a chalk cross at knee height"`
- `meaning` — the plain sentence everyone over seven knows:
  `"this household owes and has not paid"`
- `anchors` — which `MarkAnchor` variants this kind accepts
- `half_life_days_dry`, `half_life_days_wet` — weathering (suggest 9.0 and 0.4)
- `sheltered_multiplier` — suggest 6.0
- `faint_below`, `gone_below` — suggest 0.35 and 0.05
- `drawable_by_hand` — whether `draw_mark` may produce it (all three: yes)

### Decay

Swept at most once per game-minute (§2.7), for every mark:

```
wet   = precipitation, from World.current_weather (0.0 when weather is absent)
if world.shelters.is_sheltered(site.point) { wet *= 1.0 / sheltered_multiplier }
half  = lerp(half_life_days_dry, half_life_days_wet, wet)
strength *= 0.5f64.powf(elapsed_game_days / half)
```

Below `faint_below` the label gains a "half-washed" qualifier everywhere it renders and the reader
effects **stop applying** — a faint mark is a fact about the past, not a rule. Below `gone_below`
the mark is removed. Removal writes nothing to anybody's inbox: chalk washing off is not news.

### The prompt

A new sheet section `**marks_here**`, rendered exactly like `**dogs_nearby**`
(`prompt/mod.rs:1418`): omitted entirely when empty, so an untouched sheet does not move a byte.
One bullet per live mark whose anchor site is within `MARK_NOTICE_RADIUS_M` (suggest 8.0) of the
actor, nearest first:

```
**marks_here** (chalk on the walls about you)
- a chalk cross at knee height, on Ede Clove's door, 3.1 m — this household owes and has not paid
- four tally strokes, half-washed, at Chain Well, 6.4 m — this many have drawn today
```

Unknown-people rules apply through `perception::identify` — a mark on a stranger's door names the
stranger the way everything else does.

---

## 4. Milestones

Each is independently shippable and independently verifiable. **M0–M2 are the spine**; M3 is what
makes it visible; M4 widens the vocabulary.

### M0 — the medium — **DONE 2026-08-02**

`marks.rs`, `World.marks`, `assets/world/marks.json` with all three kinds authored, the anchor
resolver, the decay sweep, the `**marks_here**` sheet section, the config block and
`CATHEDRAL_NO_MARKS`. **No writer, no reader** — a mark can only be created by test code.

**As built.** `marks.json` is `include_str!`-embedded (C1). `MarkId(pub u64)` copies `RequestId`
(C2). The household anchor resolves through `PlaceRegistry::home_of` (C5). `MARKS_MAX = 100`,
measured at **100.1 B/mark** — a fully chalked city adds 10,010 bytes and leaves 16,805 of headroom,
and `full_roster_prompts_and_public_snapshot_remain_bounded` now fills the walls to the cap and
re-asserts the bound (C10, C11). `PublicMark` carries **no orientation**: the sim does not know
which way a door faces, and the host — which owns the city geometry — derives it in M3 rather than
honouring a yaw the sim would have had to invent. The catalog's `ward_sign.places` authors one place
of resort per ward, and the word "shrine" is gone (C8).

Beyond the spec's list: `Marks::iter` is id-ordered, so sheets, snapshots and evictions replay
identically; `AnchorSite::label` is documented as name-leaking (a household entry spells its owner)
and the sheet uses `occupant` + the `knows` check instead; and `draw_or_refresh` is the single
idempotent writer entry point every later milestone uses.

21 tests: 9 unit (`marks.rs`) + 12 integration (`tests/marks_tests.rs`). Golden fixtures **byte
identical**; `collision_footprints.json`, `navigation.bin` and `navigation.json` md5-verified
unchanged.

Done when:
- `cargo test -p cathedral-sim` green, including new unit tests for: exponential decay against a
  known elapsed span; a sheltered anchor outlasting an exposed one under the same rain (build the
  `ShelterMap` in the test — a hand-made `World` has an empty one); a mark whose anchor stops
  resolving disappearing without panicking; the eviction-at-`MARKS_MAX` rule.
- The golden fixtures are **unchanged** (no fixture world has marks), proving §3's "omitted when
  empty" claim. If they moved, the section is rendering unconditionally — fix that, do not bless it.
- A test that plants a mark 3 m from a fixture actor and asserts the rendered sheet contains the
  bullet, with the distance and the meaning.

### M1 — the cross, the gate, and the ward's own hand — **DONE 2026-08-02**

The first writer and the first reader, and the answer to §2.1.

**As built.** The writer is `notices::chalk_the_debtors(&mut World, game_days) -> Vec<String>`, a free
fn called from `engine.rs` beside `notices::confront` — *not* inside `Notices::expire`, which has no
path to `World` (C4). It is gated to one beat a game day by `Marks::take_daily_beat`; without that
gate it would run at ~60 Hz and re-chalk a scrubbed cross before the player straightened up, making
scrubbing pointless. "Not settled" needed no code at all: `settle` removes the notice from `live`,
so the beat simply stops finding it (C3).

The reader is two lines in `try_purchase` calling `marks::binding_mark_about`, which reads
`World.marks` and never `World.notices`. The caller re-asks the same predicate to write the
distinguishable inbox line (no nudge), the `refused_on_chalk;` food-log trace, and the
`chalk_refused_until` stamp — `try_purchase` has neither `now` nor a clock, and both the stamp and
the trace want one (C9). The stamp is pruned in `round::tick` beside `lightning_reflex_until` and
tested with a bare `contains_key` in `nearest_open_stall`.

Beyond the spec: the refusal is **idempotent per stamp period**, not merely per selection. The
`nearest_open_stall` guard stops the ladder re-queueing them, but a buyer already standing in a
queue when the cross went up would otherwise get a second line; §4 M1 asks for "one refusal per
stamp period", and that is now what it is regardless of route.

**`--owe <NAME>`** is a new headless stand-in, on the `--status` / `seize` precedent: a cross needs
an aged unsettled restitution notice, and raising one is an LLM's judgement, so an offline run could
not otherwise reach the door at all. It raises the same notice `raise_notice` raises, back-dated
past the age gate; everything downstream is the real code.

**What the headless run does and does not show.** It shows the writer:

```
$ cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 8 --owe "Clemence Skell"
[smart actors] Clemence Skell owes and has not paid; the ward will chalk their door
[marks] the ward chalks a cross on fc7sk's door
```

It does **not** show the refusal, and that is not the feature's fault: in a `--watch-clock` run the
census reports `stalls 1/13 open, queued 0, serving 0` across three game days, so *no* stall sale
happens with or without chalk. The refusal is instead pinned by
`a_chalk_refusal_is_one_scene_and_not_a_loop`, which drives the real `service_stalls` and asserts
the inbox line, the trace line, the `nearest_open_stall` guard, and then hammers twenty more passes
to prove there is no barrage. The spec's "a single headless run shows the whole arc" is therefore
half-met, deliberately, and the other half is met through the same production functions.

An accused with no registered home is skipped with a diagnostic rather than silently — "the cross
never appeared" and "the cross appeared and washed off" look identical from outside and have
completely different causes. That diagnostic found the first test name I picked (`Sibbe Clove`) was
unhoused, in about ten seconds.

9 tests: 5 in `round/tests.rs` (the partition both ways, the faint cross, the forged cross, ablation)
and 9 more in `marks_tests.rs` for the writer (age gate, idempotence, re-chalk after a scrub, the
settle clause, no-door, the per-kind switch, two notices one door), plus 2 for the refusal loop.

**Writer.** In `notices.rs`, at the existing age check: a restitution notice with an `accused`, not
settled, whose `raised_game_days` is more than `CROSS_AFTER_GAME_DAYS` (suggest 2.0) old, puts a
`ChalkCross` on `MarkAnchor::Household(accused)`. Idempotent — one live cross per accused, never a
second. `about` is derived from the anchor, per §3.

**Re-chalk.** Once per game day, while the notice is still live, the writer re-draws a cross that is
missing or faint back to full strength. Diegetically this is the sergeant's beat; mechanically it is
the same idempotent call. This is what makes scrubbing *buy a day*, not amnesty.

**Settling.** `settle_notice` does **not** erase the cross. It stops the re-chalk; the chalk then
weathers off on its own schedule. A settled debt whose mark is still up for two dry days is correct
and is exactly the "database heals slowly" texture this feature is for.

**Reader.** Two sites, one flag, and the order matters because the *drama* is the point.

Refuse **at the counter**, in `round::try_purchase` (`round.rs:6398`): if the buyer is the `about`
of a live, non-faint `ChalkCross`, return `None`. Deliberately not at stall selection — a buyer who
walks across the square, queues, reaches the head and is *then* turned away is a scene the player
can stand next to and watch. A buyer who silently never sets out is nothing.

The caller's existing `None` arm (`round.rs:6337`) already handles a no-sale gracefully — it clears
the food errand and the buyer "leaves and re-evaluates" — so **there is no deadlock to fix and no
queue surgery to do.** What that arm does not do is say why: today `None` means "spent it mid-queue
or nothing affordable." A chalk refusal must therefore write its own distinguishable inbox line
naming the mark and the stall, plus a `push_food_log` trace line beside the sale traces.

The real hazard is the opposite of deadlock: **a refused buyer re-evaluates and queues straight
back up**, refused forever, one inbox line per lap. So the refusal also stamps a
`chalk_refused_until` on that person (suggest half a game day), and `nearest_open_stall`
(`round.rs:6561`) gains one early return on it — modelled exactly on the bound-vendor guard already
sitting at the top of that function, comment and all. Verify in a headless run that a chalked buyer
produces *one* refusal per stamp period, not one per poll.

**Read `World.marks` only. Do not touch `World.notices` in either function.**

Done when a single headless run shows the whole arc, and when a test asserts `try_purchase` refuses
on chalk with the notices collection emptied — i.e. the partition is real, not incidental.

### M2 — the pen and the two verbs

**The pen.** A `chalk_pen` kind in `assets/world/items.json` (small, pocketable, priced). Stocked in
one existing stall's `you_sell` via `assets/world/food.json`, *and* one seeded into the player's
record in `assets/world/seed.json` so testing needs no shopping trip. Expect the item snapshot canary
to move (§2.7).

**`draw_mark {"kind": "chalk_cross", "anchor": "<handle>"}`** — appears on the sheet only when the
actor holds a pen (`has_chalk_verbs` in `turn.j2`, modelled on `has_pockets`). The anchor handle
comes from a new, short `**you_could_chalk**` list on the sheet — the eligible anchors within
`CHALK_REACH_M` (suggest 2.0) — because a verb whose argument the model has to guess is a verb that
fails all day. Errors: `no_pen`, `nothing_to_chalk`, `unknown_kind`, `already_marked`.

**`scrub_mark {"mark_id": "<id>"}`** — appears only when a live mark is within `CHALK_REACH_M`, and
the ids come from the same `marks_here` bullets. No pen needed. Errors: `no_such_mark`, `too_far`.

**Witnesses.** Both verbs go through `world_event_witnessed` (`actions.rs:1394`) with the 4 m
witness set, and take the nudge (§2.8) — this is the one place a paid turn is right, because
somebody chalking a neighbour's door in front of you is the whole drama. The blunt inbox line names
the act plainly and does *not* editorialize; whether it becomes a `raise_notice` is the law's
business, unchanged.

**Drive actions**, in `src/drive.rs` beside `seize`/`status`:
- `chalk <kind> -> <anchor handle>` — the explicit `->` for the same reason `seize` has it: both
  handles may contain spaces.
- `scrub <anchor handle>` — scrubs the nearest live mark at that anchor.

Both resolve handles by display name first, then by id, exactly as `status` and `seize` do. A handle
matching nothing is logged and skipped — not a fault. Each prints an evidence line:
`[smart actors] <who> chalks a cross on <anchor>`.

Done when a headless run and a drive run each show a forged cross refusing a stall, and the golden
fixtures move **only** in the conditional blocks (regenerate deliberately — see §5).

### M3 — chalk you can see

Bevy side. `src/city/marks.rs`, registered in `CityPlugin` beside the vermin and smoke systems
(`src/city/mod.rs`, near line 65).

- One entity, one alpha-tested material, one batched mesh rebuilt when the mark set changes, via
  `write_batch_mesh` (`src/mesh_batch.rs`) — the chimney-smoke / vermin pattern. `NotShadowCaster`.
  An empty batch parks on the idle triangle for free.
- A mark is a **small flat quad laid on the anchor's surface**, not a billboard: chalk seen from an
  overhead bridge must not turn to face you. Offset it a few millimetres off the surface and orient
  it to the anchor's facing (a door's normal; flat on the curb for a well tally). The nearest
  existing thing to copy for a flat surface-hugging quad is `puddle_mesh`
  (`src/weather/render.rs:184`); for the batching and the alpha material, the vermin plugin
  (`src/city/vermin.rs`) and `write_batch_mesh` (`src/mesh_batch.rs:52`).
- Strength drives opacity. A faint mark is visibly half-washed.
- The mark set arrives over the existing engine→ECS projection. **Prefer carrying marks in
  `PublicSnapshot`** (`crates/cathedral-sim/src/snapshot.rs`) and letting a mark change bump
  `world_revision`: marks are slow state, unlike the dogs' per-frame poses, which is exactly why
  the dogs got their own hot channel and marks should not. Mind two things — the 160 KiB bound
  (§2.7), and the existing care about *not* bumping the revision needlessly
  (`crates/cathedral-sim/tests/engine_tests.rs:619`, "movement must not re-trigger the snapshot
  chain"). If a tally notch every few minutes turns out to churn the chain unacceptably, then and
  only then give marks their own `EngineMessage` variant beside `Dogs`.
- **Look-at HUD line.** Aiming within `MARK_READ_RADIUS_M` (suggest 4.0) of a mark shows one line:
  the label, then the meaning. `A chalk cross at knee height — this household owes and has not
  paid.` Faint marks say so.
- **Player draw and scrub** as press-and-hold interactions taking real in-world seconds, issuing the
  same `draw_mark` / `scrub_mark` through the existing key→`EngineCommand` path. Releasing early
  aborts with nothing drawn.

Done when a drive script produces a screenshot of a chalked door that the agent opens and confirms
shows chalk, and a second shot after scrubbing that shows it gone — plus a headless test of the
engine-message→HUD path using the existing pair `ready_fake_plugin_app()`
(`src/smart_actors/mod.rs:2776`) and `LocalEngine::world_mut()`
(`src/smart_actors/local_engine.rs:191`): plant a mark in the world, pump a frame, assert the HUD
read-line says the meaning.

### M4 — the tally and the ward-sign

The two remaining writers, each with its own cheap reader.

**Well tally.** When a draw completes in `round.rs`, notch the `WellTally` on that source's
`MarkAnchor::Place` (create at one stroke, else `strokes += 1`, capped at `TALLY_STROKES_MAX`,
suggest 12). Reader: inside `nearest_staffed_source` (`round.rs:2184`), add
`strokes as f64 * TALLY_METRES_PER_STROKE` (suggest 6.0) to each candidate's distance before the
comparison. A busy well pushes its next drawer to the neighbour; overnight the chalk washes off and
the well recovers. **Keep the tie-break by source index** so an exact tie still binds identically
every run.

Check the other two callers before you touch it — this function is not only the per-trip choice.
`round.rs:2152` binds arrivals at a gate point, and `round.rs:2999` seeds the water-drawers at
startup. Startup is safe (no tallies exist yet, so the penalty is zero), but satisfy yourself that a
mid-day *binding* through `:2152` does not lock somebody to a distant well for the rest of the day
on the strength of a tally that washes off by evening. If binding is sticky, apply the penalty only
at the per-trip site and leave binding on raw distance.

**Ward-sign.** The Night Office ward batch replies in the same `VERB {json}` form the main parser
uses, applied by `Night::apply_ward` (`crates/cathedral-sim/src/night.rs:653`) — a `match` on the
verb name whose one existing arm is `"ward_mood"` (`:672`). So this is **one new match arm**, not a
new key on a struct: `chalk_ward_sign {"place": "<shrine>"}`, naming a shrine in that ward. No new
call, no second prompt, no extra tokens beyond the line itself. Follow `ward_mood`'s error
handling exactly — a missing or unresolvable `place` logs `[night] … ward: …` and is skipped, never
a fault.

Reader: `reroll_ambient_evenings` (`round.rs:1283`) currently picks between `TAVERNS` (`round.rs:167`)
by `hash01("night_tavern", id, day)`; extend that to a weighted destination list where each tavern
weighs 1.0 and each chalked shrine in the walker's own ward weighs `1.0 + 2.0 * strength`. The
walker's ward is already in hand at that point — the loop fetches `character.lore()` to test
`significance`, and the same profile carries `planning_ward` (`crates/cathedral-sim/src/lore.rs:199`),
so no extra lookup is needed. Keep the whole thing a pure hash of `(id, day)` — never a fresh draw
(`attention.rs` explains why: the engine polls at 60 Hz).

Done when a headless night run prints a ward-sign being chalked and shows the ambient evening
distribution shifting toward that shrine versus a run with the sign suppressed, and when a 60×
drive run shows well queues moving between two wells across a day.

---

## 5. Verifying

**Before your first build:** if you are working in a git worktree rather than the main checkout,
export `CARGO_TARGET_DIR` pointing at the **main repo's** `target/`. A fresh in-worktree Bevy build
fills the disk and ENOSPCs the machine. This has happened; do not rediscover it.

Run `cargo test --workspace` at every milestone. **Do not run a bare `cargo fmt` over the tree** —
format only what you touched.

**Golden fixtures.** Any sheet or template change goes through the ignored regenerate test,
deliberately, with the diff reviewed and named in the commit message:

```sh
rtk proxy cargo test -p cathedral-sim --test golden_prompts \
    regenerate_golden_fixtures -- --include-ignored
```

Use exactly that form. The doc-comment on the test suggests `-- --ignored`, but the `rtk` hook
swallows a bare `--ignored`; `--include-ignored` behind `rtk proxy` is the invocation that actually
works here (learned the hard way during the lore-item wave). If the fixtures move in a place you did
not intend, that is a bug in the conditional rendering, not a fixture to bless.

**The headless loop**, which is the fastest way to see the whole feature and needs no Bevy, no
network and no keys:

```sh
# M0/M1: age a notice, chalk the door, refuse the sale
cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 12 -v

# M4: a whole game night, the ward-sign and the ambient roll
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --night-office --start-office waning --seconds-per-day 300 --watch-clock 0.6
```

**The drive loop** (see `.claude/rules/CATHEDRAL_DRIVE.md` — do **not** use xdotool; winit never
sees synthetic core events). `key KeyT` twice puts the clock at 60×, which is how to reach a
re-chalk or a weathering without waiting out real hours. Sketch for M2/M3:

```sh
CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE='wait-online; \
  tp 44.5 1 -207.2 90; key Quote; chalk chalk_cross -> Ede Clove; sleep 1; shot chalked; \
  scrub Ede Clove; sleep 1; shot scrubbed; quit' cargo run
```

Two gotchas already paid for by earlier features: a drive window asks to be non-resizable and a
tiling WM may not honour it, so check PNG dimensions before comparing two shots pixel for pixel;
and drive scripts exercise the **real** handlers, so anything that persists (a settings pill) really
persists to `config.ron`.

**Open the screenshots.** A milestone with a visual component is not done until you have looked at
the PNG and can say what is in it.

---

## 6. Risk ledger

| Risk | Watch for | Answer |
|---|---|---|
| **Authority collision** (§2.1) | `try_purchase` reaching for `World.notices`; a reader consulting both sources | The test that empties `notices` and still refuses. If that test is hard to write, the partition is already broken. |
| A percept barrage buying provider calls | A game day whose prompt count jumps | §2.8 — inbox lines, no nudges, except deliberate acts. Count prompts in `logs/latest_session/prompts/` before and after. |
| **Unbounded world state, and a snapshot bound with little headroom** | The 160 KiB assertion at `world_data.rs:433` | Measure first (§2.7), then pick `MARKS_MAX`. This is the most likely thing to bite in M3, and it bites late — the sim tests pass and the *backend* test fails. |
| A refused buyer re-queueing forever | One refusal inbox line per poll instead of one per stamp | The `chalk_refused_until` stamp plus the `nearest_open_stall` guard (M1). Check the headless transcript, not just that it compiles. |
| Dangling anchors | A panic or a leak when a home rebinds or a place is renamed | `anchor_site` returns `Option`; the sweep drops unresolvable marks silently. |
| Nav corruption | `collision_footprints.json` changing | §2.4 — assert byte-identical in review. |
| Fixture drift | Golden prompts moving on a milestone that should not touch the sheet | The M0 done-criterion explicitly asserts they do **not** move. |
| Decay cost | Frame time or poll time at 256 marks | Sweep gated to once per game-minute, not per poll. |
| The tally starving one well | Everyone piling on a single source, or oscillating between two | Cap the strokes; verify across a full game day, not a single draw. |

---

## 7. Deliberately not in this task

Do not build these. They are listed so you recognise them and leave them alone:

- **Ward dialects** — the same glyph meaning different things per ward, taught by a `teach_mark`
  verb modelled on `tell_way`. Needs the literacy state §2.5 deliberately omits.
- **Marks as the mute witness** — a patrol rung reading scrub-streaks or an off-pattern forgery and
  raising a notice about "a person unknown." This is the player's own consequence channel and the
  natural sequel to M2.
- **The slate** — stall tallies as a physical credit ledger reconciled against memory at a scheduled
  Night Office reckoning.
- **Lane marks and path costs** — "avoid a marked lane" needs path costs the sim does not own;
  check where routing actually happens before scoping it.
- **Shrine steering beyond M4's evening roll** — weighting the soundscape's place beds by chalk.
- **Tallow, ribbons, or any second medium.** Everything is chalk (§ the pitch).

---

## 8. Closing out

Per `features/AGENTS.md`: keep the `Status:` line at the top of this file current as milestones
land, with absolute dates. If the implementation diverges from this spec — and it will, because §1
already caught one wrong assumption — **edit this file to say what was actually built and why**,
rather than leaving a spec that describes a thing that does not exist. When every milestone is done,
`git mv` this file into `features/implemented/` and move its entry in `features/order.json` from
`order` to `finished`.
