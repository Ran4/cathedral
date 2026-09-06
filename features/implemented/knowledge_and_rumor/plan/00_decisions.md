# 00 — Decisions

Every design decision this feature needs, resolved. The specs (`../README.md`, `../01_facts.md`,
`../02_rumor_pollen.md`) say *what*; this says *which way*, and no milestone re-opens any of it.

Repo: `/home/ran/src/rust/cathedralbevy`, branch `develop`, base commit `f56a2c3` (plan written
against `6910fdf`, re-verified on `f56a2c3` on 2026-09-04 by the reconciliation pass that added
D51–D62 below). Every anchor below was re-verified against that tree. Anchors are cited
`path:line (marker)`; line numbers drift, the marker does not.

**Corrections to anchors quoted in the specs and in earlier planning** — these are wrong on disk
and are fixed here:

| Quoted | Actually |
|---|---|
| `attention::on_stage` at `attention.rs:242` | `attention.rs:219` (`pub fn on_stage`) |
| `round::tick`'s `decay_needs` call at `round.rs:5744` | `round.rs:5741`; 5744 is `resolve_food_arrivals` |
| the ignorance paragraph "was measured at `turn.j2:241`" (an earlier draft of this table) | **wrong, reversed.** It goes immediately **before `turn.j2:194`** (`Use ONLY the verbs listed below, spelled exactly as shown (lowercase English).`), which is the only position M0b fired (110 calls, both providers, `../m0_evidence/ignorance_rule.txt` header, `NOTES.md` § "M0b — measured repairs"). Round 1's `sheets/v2_structural/*.txt` rendered it lower down and are the record of round 1, not the shipping position |
| `domestic_servant` — "46 of them, the commonest occupation" | 45; still the commonest |
| `announce_commitment` at `engine.rs:3814` | `engine.rs:3748`; 3815 is the `confiscate_the_taking` call the mint hook goes above |
| door extents | two hulls, both right: the 413 `homes.json` doors span x ∈ [-338.4, 329.1], z ∈ [-457.1, 327.6]; the **321 ward-labelled marks** span x ∈ [-323.6, 329.1], z ∈ [-457.1, 324.9]. The ward grid uses **neither** — it uses the nav box (D23) |
| lore-ward mean population 65 as the mint ward's size | the **standing** ward is a Voronoi over 321 doors: BellAndSluice 192, Weigh 66, Reed 62, Wick 53, Cloth 46, Fabric 45, Wallwright 29, Cinder 26. The Wickmarket stands in **Wick, N = 53** |
| `unknown_person_role` = `"a {trade}, of {ward}"` | no comma, and `%s` not braces: `"a %s of %s (you don't know their name)"` (`../m0_evidence/strings_draft.toml`, two ordered `%s`) — the comma form was tried and rejected in M0 |
| the hedge ladder = six flat `hedge_*` keys with a band-shift table (an earlier draft of `03_assets.md`) | **24 keys, 21 distinct measured rungs** (`know_hedge_{default,top,low}_{hops0_own,hops0,hops1,hops2,hops3,hops4,cold}`), every one fired on both providers; `strings_draft.toml` says in as many words that a band-shift layout "is not a transcription of the measured text" |
| `ids` re-export at `lib.rs:141` | `lib.rs:85`; 143 is `HEARING_RADIUS_M` |
| `local_engine::translate` at `:914`; `BridgeCommand` at `bridge.rs:223` | `local_engine.rs:733`; `bridge.rs:69` |
| drive.rs `Action`/`describe`/`parse_statement`/`Directive` at 227/295/399/675 | 183 / 275 / 340 / 655 |
| `rg` as the acceptance-check tool | **not a binary on this machine** — `rg` is a shell function wrapping the Claude bundle, absent in a non-interactive subprocess. Every check in `plan/` uses `grep -rnE` |
| `scripts/m0/` and `scripts/m0b/` "to be deleted" / "to be extended" | both **already gone** (`ls scripts/`); nothing to delete, nothing to run. M0b was scored in `NOTES.md` |
| `EngineConfig.start_office` | no such field: `WorldClock::new(seconds_per_day, start_office: Office, start_day, night_brightness)` (`clock.rs:349`); the headless binary resolves `--start-office` with `Office::from_config_name` (`cathedral_headless.rs:406`) |
| `Engine::apply_player_action` callable from a test | it is a **private** `fn` (`engine.rs:2074`). Tests drive `EngineCommand::PlayerSay { … }` (`engine.rs:569`) through `Engine::poll` and read `Engine::world_mut()` (`engine.rs:1579`, `pub`) |
| `kimi-k2.5` is the shipped moonshot default and needs `LLM_MODEL=kimi-k3` | fixed at `f56a2c3`: `cathedral_headless.rs:1393` and `llm.rs:94` read `kimi-k3`. No override anywhere in `plan/` |
| the headless watch loop's "catch-up budget (3.2 s)" comment (`cathedral_headless.rs:630`, `:634`) | stale: `MAX_MOVEMENT_CATCHUP_SLICES` (8, `lib.rs:370`) × `MOVEMENT_TICK_SECONDS` (0.05, `lib.rs:173`) = **0.4 s** of walking per poll (`engine.rs:2714-2725`). M2 fixes the comment |
| `Provenance::ItemAt(ItemId, [f64; 3])` | items have no position (`item.rs:132`); `Provenance::ItemWith { item, holder }` (M1 step 4) |

---

## A. Types, names and vocabulary

### D1 — The topic tag for a promise is `talk`, not `word`
`../01_facts.md`'s example JSON writes `"topic": "word"` twice and `../02_rumor_pollen.md`'s table
names the variant `Talk`. **`Topic::Talk`, authored as `"talk"`.** `Topic` derives
`#[serde(rename_all = "snake_case")]`, so the nine authored spellings are `bed blood law omen
stranger coin bread craft talk`. The spec's two `"word"` occurrences are typos; the shipped
`facts.json` writes `talk`. Reason: `word` is the *city's prose register* (`word_in_the_ward`,
`raise_word`) and must not double as a machine tag — reusing it would make a grep for the verb hit
the tag.

### D2 — There is no `talkativeness`
`../02_rumor_pollen.md`'s deposit formula multiplies by `talkativeness`, which does not exist
anywhere in the crate. **Delete the term.** Deposit is `air.heat = max(air.heat, held.heat)`; the
per-hop heat loss is charged **once, at pickup** (`heat_at_learn = air.heat × HOP_LOSS`). A person's
talkativeness already enters twice and in the right places: as `curiosity_of` (`attention.rs:678`)
in the pickup roll, and as the `heat × salience > VOLUNTEER_HEAT` deposit gate that decides whether
they repeat it at all. Keeping it would multiply the air's heat by ~0.12 on every deposit (measured
mean curiosity), so the air would never get hot and nothing would ever cross a ward.

### D3 — `GameDays` is a plain `f64`
Not a newtype. `Option<f64>` wherever a clock may be absent, exactly as
`WardNotice::raised_game_days`, `CustodyRecord::sentence_due_game_days` and
`Marks::last_sweep_game_days` already are. A newtype would not be arithmetic-compatible with the
forty existing `f64` game-day call sites and buys nothing here.

### D4 — `GarbleMask` is three bools with a string serde form
```rust
pub struct GarbleMask { pub subject: bool, pub place: bool, pub day: bool }
```
Authored as a comma-joined string: `"none"`, `"place,day"`, `"subject,place,day"`. Parsed by
`GarbleMask::parse` (an unknown token is a load error naming the three legal ones), rendered by
`as_authored` in the fixed order `subject,place,day` so a round-trip is byte-stable. A struct rather
than bitflags: three named fields read at the call sites (`if fact.garble.place`) and no dependency.

### D5 — `AreaId` at the boundary, `AreaKey(u16)` inside
`id_newtype!(AreaId, "a named area")` in `ids.rs`, re-exported at `lib.rs:85` — the JSON/API
boundary type. `AreaKey(u16)` — the index into `AreaMap::areas` (71 areas today,
`assets/world/areas.json`) — is what the store and `FactView` hold. Interning is what makes six
holdings cost ~850 B instead of ~1.6 KB and makes the air key `(PlanningWard, FactKey)` `Copy`, so
one ward's air is a `BTreeMap::range` and not a filter.
**`AreaId::new()` returns `Result<Self, InvalidId>`** (`ids.rs:51`); at the `AreaMap` boundary use
`AreaId::from_raw(area.id.clone())` (`ids.rs:61`), which is the seed-data constructor.
`FactKey(u32)` likewise: dense, never authored, never rendered, never serialised.

### D6 — "Adjacent area" is `nearest_areas`, precomputed
`../02_rumor_pollen.md` says a garbled place becomes "an adjacent area". `areas.json` carries no
adjacency. **Definition:** `AreaMap::nearest_areas(centroid_of(area), GARBLE_AREA_RADIUS_M, 5)`
(`areas.rs:299`) minus the area itself (index 0 is always the area at distance 0), precomputed once
into `Vec<Vec<AreaKey>>` and picked by `hash % len`. `nearest_areas` already sorts by
`(horizontal_distance, area.id)` (`areas.rs:320-324`), so the table is byte-stable across runs and
goldens — which the garble-determinism test needs. The table lives on `WardGrid`'s sibling
`AreaAdjacency`, built from the **live** `AreaMap` (host-supplied through `WorldConfig`), **not** on
`FactCatalog`: the catalog is an `include_str!` asset with one lifecycle and must not acquire a
world-dependent one. `AreaMap::default()` is empty (`areas.rs:165`), so in every golden and hermetic
world the adjacency table is empty and a place garble is a no-op — stated, not discovered.

### D7 — `knowledge::holds` keeps its name; the collision is handled by convention
`Character::holds` (`character.rs:653`, the hands), `Custody::holds` (`custody.rs:305`, the law) and
`Sheet.you_are_held` all exist. None is a compile-time collision with a free function in a new
module. **`knowledge::holds` is always called fully qualified and never `use`d**, the way
`notices::carries` (`notices.rs:419`) already establishes for exactly this shape of predicate, with
a doc note naming the two other `holds` it is not. No method is added to `Character`. Three quest
specs quote `holds()` and `Held` verbatim; renaming would strand them for readability nobody asked
for.

**One name *is* taken and does get renamed:** `round::Arrival` (`round.rs:252`) is already
re-exported at the crate root, so the merge rule's input type is **`Telling`**, not `Arrival` — which
reads better anyway. Every other type this feature adds was checked repo-wide and is free.

### D8 — `FactView` has no `topic` field
"Topic is invariant under garbling" becomes **unrepresentable** rather than merely asserted. The
test still exists (walk every hop of a chain, assert one topic) as a regression guard on a future
edit that adds the field.

### D9 — `Held::heat` is derived, never stored
```rust
pub struct Held { pub hops: u8, pub from: Option<ActorId>, pub learned_on: Option<f64>,
                  pub view: FactView, heat_at_learn: f32 }
impl Held { pub fn heat(&self, game_days: Option<f64>) -> f32 }
```
`heat = heat_at_learn × λ^((game_days − learned_on) × 24)`, and `heat_at_learn` untouched when
either side is `None`. Deletes a whole-cast per-carrier cooling pass; makes cooling exact under the
`T` key's 60× and `--watch-clock`'s coarse steps; and gives the clock-less golden world the
`raised_game_days` "None means nothing ages" tolerance for free (`notices.rs:400-407`).
**`Drift::heat` stays stored**, because that sweep has a second job a derivation cannot do (bumping
`stir` and enforcing the per-ward cap).

### D10 — `AreaKey`/`FactKey` never reach a prompt, and neither does `FactId`
The sheet bullet is the rendered sentence and nothing else — no id, no number, no hop count. No verb
names a fact, so a handle on the sheet is a handle the model can only misuse. (M0 confirms it:
`{hops}` was available to every band in all three prose variants and deliberately used in none, and
no reply of 66 mentions a count.)

---

## B. Sealing, leaking and the prompt

### D11 — `FactSource` is sealed structurally, three ways
A projection-walking test cannot see `EngineMessage::Diagnostic(format!("{fact:?}"))`, written in
good faith by somebody who has never read this file. So:

1. the payload enum is **private to `knowledge/source.rs`** and `FactSource` is a newtype over it;
2. **no `Serialize`, no `Deserialize`, no `Display`** — the trait is simply absent, so a leak does
   not compile;
3. **`Debug` is hand-written and prints `FactSource(<sealed>)`** — so `{fact:?}` on the whole `Fact`
   is safe, and `Fact` keeps a derived `Debug` that a test can read.

The only things the outside may learn are `is_claimed() -> bool` and `claimant() -> Option<&ActorId>`
(the walk-back-to-the-mouth affordance the spec requires). `Fact::source` is a private field with a
`pub(crate) fn source(&self) -> &FactSource`. The spec's "walks every projection" test stays as a
backstop, not as the mechanism.

### D12 — Facts never enter `PublicSnapshot`, and the store is behind `Arc`
`Knowledge` sits on `World` beside `notices` / `custody` / `marks` / `ward_moods`
(`world.rs:140-189`), never calls `touch_public_state()`, and adds no field to `PublicSnapshot`
(`snapshot.rs:21-39`, already 137 KB of a 160 KiB bound). Its two large maps (`holdings`, `air`) are
`Arc<BTreeMap<…>>` mutated through `Arc::make_mut`, because `World::market_sale` does
`let mut staged = self.clone()` on **every catalog sale** (`inventory.rs:1298`, siblings at `:916`
and `:1141`) — without `Arc` that deep-copies the whole fact store per transaction. With it,
`World::clone()` is two refcount bumps and the deep copy happens only on the first knowledge write
after a clone, which on the sale path is never. `World`'s derived `Clone`/`PartialEq` stay intact
(`Arc`'s `PartialEq` compares pointees).

### D13 — Golden churn is paid exactly once, in M1, and it is the *prose* that pays it
The **block** costs zero bytes on a sheet with no facts: `Sheet.what_you_know` is
`#[serde(skip_serializing_if = "Vec::is_empty")]` and `sheet_markdown` guards it with
`if !sheet.what_you_know.is_empty()` — the sheet's universal idiom (`prompt/mod.rs:1545` marks,
`:1555` notices). Its instruction paragraph (`know_discipline`) renders **inside** the block, between
the header and the bullets, exactly as M0 measured it; there is **no `has_known` template flag** —
nothing outside the block is gated on it (M1 step 13f).
The **ignorance rule** is unconditional `turn.j2` prose, inserted immediately **before
`turn.j2:194`** (`Use ONLY the verbs listed below…`) with one blank line after it — because it
exists precisely for the actor whose store is empty, and its own first sentence is about the block's
*absence* ("A sheet with no what_you_know on it means nobody has told you anything… that empty place
is itself an answer"). Gating it would hide it from its only audience. That position is the only one
M0b measured (`../m0_evidence/ignorance_rule.txt`, "PLACEMENT WAS MEASURED, NOT ASSUMED").
So all 22 fixtures in `crates/cathedral-sim/tests/fixtures/prompts/*.txt` move once, in M1, by
**+23 lines / +1387 bytes each and 0 deletions** (22 lines and 1386 bytes of paragraph, plus one
blank line). See D40 for the acceptance check. Every `turn.j2` line number **at or after 194** in
this plan is quoted pre-insertion; after M1 add 23 (`set_goal {"goal": "Eat fish"}` at 231 → 254).

### D14 — Nothing the model reads ever says "fact", "hop", "heat", "band", "salience", "topic",
"rumour" or "store"
Verified absent from all 66 M0 rendered sheets, in every variant, by design. `Fact` is an internal
type name; the verb is `raise_word`; the block is `what_you_know`.
**One carve-out, stated here because the closed list must be named somewhere:** the `raise_word`
fence comment M4 ships (`03_assets.md` §4) ends `topics: bed, blood, law, omen, stranger, coin,
bread, craft, talk` — the measured text (`../m0_evidence/scenarios/q5_raise_word_with_occasion.json:9`,
`verbs.add[0].line`), fired on both providers with 0 misfires. T30 (M1) excludes that one line by
name; **every later milestone extends T30 to the strings it adds** (M3: `known_from`; M4: the three
`raise_word` error messages and the fence comment minus its `topics:` tail; M5: the refusal percept,
the hearsay `line()` clause and `clears_when` text), so the rule is asserted over the whole feature
and not only over M1's keys.

### D15 — The self-subject rule is a filter, not a hope
`what_you_know_lines` **skips any fact whose `subject` contains the reader**, before relevance and
before heat. They are never told about themselves in the third person; if they hold it, they hold
their `own` line and nothing else (`render_line` returns the `own` template when
`fact.own.contains_key(reader)`, whatever the hop count). And the subject **never picks it up at
all** — through `knowledge::may_carry` (D51), a predicate outside the salience product, so the store
does not carry it either. Two tests: `a_subject_is_never_told_about_themselves` (a hops-3 holding
whose subject is the reader renders nothing) and `the_subject_never_carries_under_either_lever`
(`may_carry` is false for the subject with the salience table live, flat, and deleted from the roll).

### D16 — `said` and `own` are **templates**, and one observer-aware renderer substitutes them
`{subject}`, `{place}`, `{day}`. `knowledge::render_line(world, reader, key, held, strings)` is the
only place a fact becomes a sentence. `{subject}` resolves to the **effective** subject
(`held.view.subject` if garbled, else `fact.subject[0]`) and then through the reader: their own name
if it is them, the real name if `reader.knows()` contains them, else
`strings.unknown_person_role` = `"a %s of %s (you don't know their name)"` (trade, then ward),
falling back to `strings.unknown_person_name` when there is no lore.

Three problems, one fix. It makes the unknown-people rule structurally unbreakable (there is no
baked name to leak); it gives garbling something to move (a baked name cannot be swapped); and the
role hint answers the real bite of the rule — **`actions.rs:553` writes `knows` only inside
`if !observer.control().is_llm()`, so no NPC ever learns a name at runtime**, the authored cast's
`knows` averages 3.2 people (measured over 519 sheets, median 2, six with none), and the generated
crowd's is empty by construction (`crowd.rs:558`, asserted `crowd.rs:1070`). Without the role hint,
essentially every third-party subject would render as one undifferentiated "a stranger" — the wall
the ignorance rule exists to avoid, reached from the other side. "A porter of the Weigh Ward" leaks
no name and is a lead.

The two placeholders of `unknown_person_role` are **two ordered `%s`** — the trade, then the ward —
rendered by two sequential `replacen("%s", …, 1)` calls, exactly as `strings_draft.toml` freezes it
(D44). The trade is `LoreProfile.occupation_display` with **only its first character lowercased**
("Baker" → "baker", "Domestic servant" → "domestic servant"); all 65 displays in
`lore/core_lore/occupations.json` are common-noun phrases. The ward is `prompt::ward_label`
(`prompt/mod.rs:806`), which already yields "the Wick Ward". `{day}` renders through
`strings.day_today` / `day_yesterday` / `day_days_past` (`%s`, filled with the count **in words**,
`two`..`seven`) / `day_long_ago` (eight days and beyond, and every clock-less or undated case) — a
digit never reaches a sheet (D10). `{place}` renders `Area::label`, or `strings.place_unknown` when
the key does not resolve.

### D17 — M1 transcribes M0b's frozen strings **byte-for-byte**; nothing prose-shaped is owed later
The freeze is **`v6_both`** (`../m0_evidence/NOTES.md` § "M0b — measured repairs", 2026-09-04),
superseding round 1's `v2_structural`: `strings_draft.toml` (24 keys) and `ignorance_rule.txt`
(22 lines, 1386 B) are its byte-for-byte transcription, verified to round-trip to
`prose/v6_both/`. M0b already **took** the two repairs M0 carried forward — R1 (seven rungs per band)
and R2 (the referral exemplars replaced by descriptions) — and **declined** R3 (the let-it-lie
softening) on evidence. So M1 copies the two artifacts and takes no repair, and **no milestone
schedules a prose re-fire of the M0 strings**: there is nothing left to decide. What is owed:
`known_from` (M3's one unmeasured string) and the Q4 re-measurement `NOTES.md` § "The
re-measurement M3 owes" assigns to M3; `q4_wick_03`'s `own` line is already re-authored. A changed
string is an unmeasured string; M1 must not depend on a live provider.

### D18 — The hedge ladder is the 21 measured rungs, from M1, and one precedence rule
`strings_draft.toml` carries seven rungs per band — `hops0_own, hops0, hops1, hops2, hops3, hops4,
cold` — for the three bands `default`, `top`, `low`: **21 distinct measured values**, not one ladder
shifted by band. M1 ships all 21 under their frozen names and renders by a `(HedgeBand, Rung)` lookup
with no band-shift; M3 adds **no rung** and no rung text (its only new string is `known_from`).
`know_hedge_low_hops3` ships labelled as the weakest rung (n = 1 per provider); M3's re-measurement
gives it its n = 2.

**Rung selection, one rule for the whole feature** (M1 `rung_for`, M3 keeps it):
1. the reader has an `own` line for the fact → `hops0_own`, **whatever the heat** — D15 forces it: a
   cold subject-witness rendered in the `cold` register would be told `said` about themselves. This
   is the one departure from the measured rig, which checked cold first even over `hops0_own`;
2. else if cold (`!knowledge::volunteers(...)`) → `*_cold`, **over every hop count including 0**
   (the measured rig's order; a hops-0 holder of a decaying fact does go cold after M2 step 9);
3. else `hops0` / `hops1` / `hops2` / `hops3` / `hops4` (four removes or more).
M3's "hops 0 wins over cold" reversal is **rejected**: it was unmeasured, and the merge rule's fourth
row (a witness cannot be talked out of what they saw) is about the *store*, not the register.

### D19 — The hedge band is authored per topic, not derived from the base number
`salience.json` carries `"hedge_band": "top" | "default" | "low"` on each topic
(`Bed`/`Blood` → top, `Craft`/`Talk` → low, the other five → default, exactly
`../02_rumor_pollen.md`'s "Hedge erosion" table). **Not** a threshold on the base value: that would
make `SalienceTable::flat()` — which sets every base to 1.0 — silently promote all nine topics to
the top band and change rendered text during the identity run. Flattening must be *purely
arithmetic* (D33).

---

## C. Where the code lives and what it touches

### D20 — Module layout
```
crates/cathedral-sim/src/knowledge/
    mod.rs       Fact, Held, FactView, Topic, GarbleMask, Knowledge, holds(), learn(),
                 render_line(), the constants
    source.rs    FactSource — the sealed field (D11)
    catalog.rs   FactCatalog, facts.json loading and validation, quest packs
    salience.rs  SalienceTable, salience(), salience.json
    pollen.rs    Drift, WardGrid, AreaAdjacency, poll_person, poll_player, sweep,
                 hop_on_stage, PollenCensus
    garble.rs    view_for(), the bounded substitution vocabulary
    mint.rs      MINT_WHITELIST, mint(), the coded mint hooks, raise_word's store side
```
`lib.rs` gains `pub mod knowledge;` (alphabetically between `item` and `lore`) and **re-exports
nothing from `knowledge` at the crate root** (D56): `AreaKey` and `FactKey` join the `pub use ids::{…}`
line at `lib.rs:85`, `JournalEntry` (M4) and `CivicRope` (M5) join `pub use engine::{…}`, and every
other type is reached as `cathedral_sim::knowledge::…`. `mod homes` stays **private** (`lib.rs:28`)
— `homes::ward_marks` is already `pub(crate)` (`homes.rs:94`), so `crate::homes::ward_marks()` is
crate-reachable and no visibility change is needed.

### D21 — Pickup, deposit and cooling run in a **new whole-cast pass inside `round.rs`**, not in
`run_ladder`'s body
`fn tick_pollen(round: &mut Round, world: &mut World, clock: &WorldClock, now: f64)`, a free
function in `round.rs`, called from `round::tick` immediately after
`decay_needs(round, world, clock, now);` (**`round.rs:5741`**).

Two reasons. (a) `run_ladder` (`round.rs:7400`) `continue`s past whole cohorts *before* its
throttle — anyone with a live food errand, anyone the escort abandon touched — and after it, every
non-`Idle`/non-`Travelling` well phase and every held conversation. That is the market crowd, the
well queue and the people standing talking: the most gossip-shaped places in the city, all
invisible. (b) `Round.people` is **private** (`round.rs:1134`) and `Round`'s only `pub(crate)`
method is `abandon_bodily_errands`; a pass living in `knowledge::` would need three new accessors,
and putting the function *in* `round.rs` needs none. `decay_needs` (`round.rs:6259`) is the proven
whole-cast borrow shape (`round: &mut Round` and `world: &mut World` as disjoint params).
`tick_pollen` calls out to `knowledge::pollen::` for everything but the iteration, exactly as
`round.rs` already reaches `crate::marks::binding_mark_about`.

### D22 — Every deadline is in **game-days**, never real seconds; rolls are clock-invariant, transport is not
`Round.next_pollen: BTreeMap<ActorId, f64>` holds **game-days**, shaped like
`chalk_refused_until` (`round.rs:1153`) but on the game clock. A real-seconds deadline silently
changes the roll count under the `T` key's 60× and under `--watch-clock`, so the cadence figures the
spec makes a test would not be the cadence a played run produces — the whole measurement would
calibrate against the wrong run. The same rule binds `Occasion.at_game_days`, the raise cap's
office stamp, `Knowledge::last_sweep_game_days`, `Held::learned_on`, `Round.knowledge_refused_until`
and the hearsay beat. `update_weather_shelter_intents` (`round.rs:5762`) states the same principle
in its own doc comment. Asserted by `the_roll_count_is_invariant_under_the_time_scale`
(M2, `pollen_cadence.rs`): the same game-hour span at `seconds_per_day` 120 and 3600 gives the same
`rolls_per_game_hour` and the same `Drift::stir` on every row.

**What is *not* clock-invariant, and the plan does not pretend it is: walking.** A mouth walks in
sim-seconds — `WALK_SPEED_MPS = 2.1` (`lib.rs:175`), `tick_movement` advances movers by
`MOVEMENT_TICK_SECONDS` slices of `now` (`engine.rs:2697-2725`), a `go_to` is budgeted
`now + GO_TO_BUDGET_FACTOR × metres / WALK_SPEED_MPS` (`round.rs:4842`), and the `T` key scales only
the clock (`clock.rs:390`). So the word's transport across wards is a property of `rounds.json` **at
a given `seconds_per_day`**, and:
- the cadence band is **defined at the shipped `seconds_per_day: 3600`** (`config.ron:50`), stepped
  at **0.4 s** (= `MAX_MOVEMENT_CATCHUP_SLICES` 8 × `MOVEMENT_TICK_SECONDS` 0.05, the most walking
  one poll can realise, `engine.rs:2714-2725`): 9,000 polls a game day, seconds of CPU at
  `--extra-ambient 0`. Every cadence command in `02_numbers.md` §8 and M2/M3/M5 carries
  `--seconds-per-day 3600 --start-office dayspring`, and `--trace-pollen` caps the watch step at 0.4 s
  (its own guard arm, M2 step 19);
- at `--seconds-per-day 120` a game day is 120 sim-seconds and a 270 m leg (129 s) never completes
  inside it — X collapses and both ends are unmeasurable. **Never measure the band there.**
- under the `T` key's 60× nobody crosses a ward inside a session (Jonet Kett's 129 s walk is 51.6
  game hours), so a 60× drive run shows **in-ward** pickups only, and M2/M3's drive checks say so;
- an occasion's one-game-hour life is 2.5 real seconds at 60×, shorter than any provider turn, which
  is why an *offered* occasion survives until its reply lands (D34).

**M2 review addendum (2026-09-05) — one more thing that is not clock-invariant, so no later test
assumes it: the *phase* of a person's polls on the stir grid.** `poll_gap_game_days` is salted on
`Townsperson::epoch` (as this file's M2 steps specify), and the epoch advances on ladder decisions
timed in sim-seconds — about forty a game hour at 1×, about one under the `T` key's 60× — so the
sequence of gaps a body draws, and therefore which coin a given deposit lands before, differs with
the time scale even for a body that never moves. No coin is ever skipped by it (a gap is always under
a stir window, `the_poll_gap_cannot_skip_a_stir`), so the roll *count* and every row's `stir` stay
invariant, which is what `the_roll_count_is_invariant_under_the_time_scale` asserts; carriers and
`rolls_per_game_hour` (a formula over where bodies stand) are not compared, and must not be.

### D23 — Position → ward has **one** definition, and it is exact
`WardGrid` is an **accelerator, not an approximation**. Baked once per process into a
`OnceLock<WardGrid>`: 8 m cells over x ∈ [-365.0, 361.8], z ∈ [-480.5, 347.8] (the walkable grid's
own world box, `assets/world/navigation.json: grid`), **91 × 104 = 9,464 `u8` cells = 9.2 KB**. A
cell stores a ward ordinal `0..=7` when the cell centre *and* all four corners agree under the exact
nearest-mark search, and `0xFF` otherwise. `WardGrid::at(point)` is one array index for the 94.6% of
cells that are settled (measured) and falls through to the **exact** 321-mark search for the 5.4%
that are ambiguous and for every point outside the box. So the grid's answer is *identical to the
exact search everywhere* — provably, because the fallback **is** the exact search — and there is no
hole where a real point answers `None`.

`World::ward_at(point) -> Option<PlanningWard>` is **the** definition of a person's standing ward,
and **`crowd.rs` is routed through it**: the exact search moves from `crowd::nearest_ward`
(`crowd.rs:370`) into `knowledge::pollen` as the grid's baker, `crowd::ward_map` (`crowd.rs:362`)
becomes `pub(crate)`, and `crowd.rs`'s two call sites (`crowd.rs:257`, `crowd.rs:288`) call
`ward_at`. Housing and pollen therefore cannot disagree, and because the grid is exact, **no
generated citizen's ward changes** — `cargo test --workspace` must stay green with no fixture edits.

Do **not** substitute `LoreProfile.planning_ward`: that is the ward a person is *of*, not where they
stand, so nobody would ever carry a word across a boundary and the entire mechanic vanishes.

### D24 — Layer 2 rides its own gated scan, and the reason `on_stage` cannot be reused is a doc
comment
`../02_rumor_pollen.md` says the visible mouth-to-mouth hop rides `attention::on_stage`'s existing
scan "and nowhere else". It cannot: **`night::stage_occupied` (`night.rs:864`) calls `on_stage`
purely as an emptiness question**, so a side-effecting `on_stage` would double-fire; and `on_stage`
is `None` under `IdleCognitionMode::All` (the default, and the headless runner's) and decimated by
`max_actors` = 6.
So `pollen::hop_on_stage(world, player_id, radius_m, game_days)` runs its own
`World::characters_within_refs` (`world.rs:463`) around the player, **self-gated to once every
`STAGE_HOP_SECONDS = 2.0` real seconds** and capped at the nearest `STAGE_HOP_MAX_PAIRS = 8`
carriers. The gate is what bounds the O(N) scan (`world.rs:478` `neighbours_by_distance` has no
spatial index): 0.5 scans/s × 20,000 = 10,000 distance tests a second, against a pump already at
179 ms/frame. The reason goes in `hop_on_stage`'s doc comment so nobody optimises it back onto the
shared scan. Risk 3's guard covers this scan explicitly.

### D25 — A pickup is **not** a percept
`Novelty::admits_idle` short-circuits to `true` on a non-empty inbox (`attention.rs:388`), so one
`notify_percept` per pickup would turn the pollen field into a city-wide prompt firehose at
`extra_ambient_npcs: 1000`. Pickup writes the store and nothing else. Asserted:
`a_pickup_is_not_a_percept` — inbox and pending-history lengths unchanged across a pickup.

### D26 — The player gets a dedicated poll and a dedicated curiosity
The player is not in `round.people` (`Round::seed` filters on `control().is_llm()`,
`round.rs:2816-2827`), and `Character::notify_percept` no-ops for them. So
`pollen::poll_player(world, player_id, game_days)` is called from `Engine::poll` beside the
notices/marks block (`engine.rs:1268-1289`), on its own game-time deadline (**landed in M2 step 15**;
M4 adds only `expire_occasions` after it and writes the receipt inside `learn`). It uses
**`PLAYER_CURIOSITY`**, never `curiosity_of`: the player has no lore sheet
(`assets/world/seed.json`: `{"id":"player","name":"Player","control":"player"}`), so `curiosity_of`
returns `CURIOSITY_WITHOUT_LORE = 1.0` (`attention.rs:561`) and the player would hold every fact in
the ward's air on their first roll — the journal a firehose and "beat your own story to the ward"
dead. **And the player's affinity is 1.0 on every topic**: with no lore they have `occupation: None`,
which the affinity match would otherwise read as the no-trade quarter (×1.4), making their effective
per-stir chance `0.35 × 1.4 = 0.49` and not the 0.35 the derivation reasons from. `salience()` tests
`world.player_id() == Some(listener)` (`world.rs:299`) before the occupation match and returns the
base band unmultiplied, so `PLAYER_CURIOSITY` is the whole of the player's roll (M4 test A-25).

### D27 — `Drift` is not `Copy`, so every air sweep is two-phase
`Drift::via` is `Option<ActorId>` and `ActorId` is `String`-backed. The sweep must collect keys under
`&World` and then loop `get_mut`, exactly as `marks::sweep` does (`marks.rs:755-776`). A single pass
will not compile, and the obvious fix — cloning the air map per poll — is a 20 Hz allocation.

### D28 — `stir` is bumped by the cooling sweep, on a fixed grid — and a deposit never bumps it
`../02_rumor_pollen.md` says `stir` is "bumped whenever the air is re-heated", which makes the
effective roll rate a property of who happens to be warm and therefore not derivable from the
cadence band. **Decided:** `stir` is bumped by the cooling sweep on the half-game-hour grid
(`STIRS_PER_GAME_HOUR = 2`, cooling by `λ^(1/2)` per step so two steps compose to the hourly `λ`),
and by exactly one other path — `Knowledge::stir_up`, the genuine re-heat that `pollen::amplify`
(M5) and `knowledge::reheat` (M4, when the fact is already in that ward's air) call. **A deposit
never bumps it**, whatever heat it carries: `deposit` is `heat = max(heat, held.heat)`,
`hops = min`, `via` on a lowered `hops`, and nothing else (M2 step 10, asserted by
`a_deposit_does_not_bump_the_stir`). A row's `stir` is seeded from `stage_stir(game_days)` at
creation (`seed_air`, the first deposit, `debug_seed_air`), never from 0, so a row evicted and later
re-deposited does not replay the same coins for everyone who was present the first time. The spec's
rule stays true through `stir_up`; the *rate* becomes a constant the fast end can be solved
against. The invariant
`POLLEN_POLL_MAX_GAME_MINUTES (15) < 60 / STIRS_PER_GAME_HOUR (30)` — so no person can skip a stir
window — is asserted by a unit test whose failure message says why.
`sweep` follows `marks::sweep`'s self-gating shape *including* its "keep the clock current on the
empty early-out" clause (`marks.rs:725-730`), so the first fact minted into a long-running world is
not charged for the whole run.

### D29 — The air's heat floor is strictly positive
`HEAT_GONE_BELOW = 0.01`, never `0.0`. Exponential decay underflows to exactly `0.0` and
`0.0 < 0.0` is false forever, so a zero floor makes the row immortal and permanently holds a cap
slot — the bug `MarkCatalog::from_json` rejects in as many words (`marks.rs:505-521`).

### D30 — Heat is quantised before any change test
`knowledge::heat_pct(heat: f32) -> u8` following `marks::published_strength_pct`
(`marks.rs:710`), shared by the sweep's "did anything change" test and by any republish decision.
One cooling step multiplies heat by ~0.943, twelve orders of magnitude above epsilon, so a raw-`f32`
`!=` is true on every sweep and would churn the diagnostic chain forever. `JournalEntry` carries
**no heat at all**, which removes the question from the wire.

---

## D. Minting, the verb, and the occasion

### D31 — Mints are explicit call-site hooks; no mint reads `DomainEvent::recipient_ids`
`../01_facts.md` says minting is "intercepted at `World::emit`" and takes `seeded` from
`recipient_ids`. Neither works:
- `World::emit` (`world.rs:510`) is three lines that assign a sequence and push — it has **no clock
  and no config**, so it cannot stamp a game day or resolve a place;
- `recipient_ids` means something different per kind — carriers for the notice verbs
  (`actions.rs:3145` passes the curiosity-rolled `carriers`), hearers for the law verbs, and
  **empty** for `"commit"` (`engine.rs:3825-3833`), which is additionally gated `if gaol` and so
  never fires for a gate-arch commitment at all.

So each mint is an explicit call at the site that has the facts, and each computes its own earshot
with `world.characters_within(at, HEARING_RADIUS_M, None)` — `None` as `exclude`, so the subject and
the player are both in it. `characters_within` (`world.rs:439`) returns distance-then-id order, so
`seeded` is deterministically ordered for free. The spec's promise that "who was there is already
answered correctly and is not re-implemented" is honoured by reusing the **radius and the scan**,
not the overloaded field. `raise_ward_notice_for` (`actions.rs:2193`) is the precedent: a coded
write with no verb that computes its own earshot.
**This departure is recorded in the spec at close-out** (`../02_rumor_pollen.md`, "The whitelist"),
not left as a private correction.

### D32 — M2's two coded mints are the custody commitment and `raise_notice`
- **`announce_commitment`** (`engine.rs:3748`), immediately above the
  `self.confiscate_the_taking(...)` call at `engine.rs:3815`, where `station`, `officer`,
  `notice_id`, `gaol` and the prisoner's position are all still in scope — and which runs for a gate-arch
  commitment too, the common case the `if gaol` emit misses. Topic `Law`.
- **`raise_notice`** (`actions.rs:3040`, after the `world_event` at `actions.rs:3145`), which already
  carries authored prose, a place from `area_map.location_description` and a clock stamp. Topic
  `Law`.

**The knell moves to M5**, beside bells-as-amplifiers, behind a new
`EngineCommand::Knell { years, at }` — it has no sim seam today. **A large accepted sale is not used
at all**: `inventory.rs` emits `"sale"` with empty recipients and the price never reaches the event,
and a mint inside `market_sale`'s staged clone is silently discarded when the transaction errors.

**The two coded mints prove the seam; they are not the band measurement.** Two `Law` facts cannot
measure nine bands. M2's cadence numbers come from an authored one-fact-per-topic pack planted by
`--pollen-seed`, which is the only way to control the topic under measurement anyway.

### D33 — `MINT_KINDS` is stated once
The whitelist table is one `&[MintKind]` in `mint.rs`, and the occasion gate's second limb
(D34) reads **that same list**, so "a percept that minted nothing" cannot drift out of agreement
with the mints across five call sites.

### D34 — The occasion gate keeps **both** limbs, and limb 1 performs a real `holds()` lookup
One **overwriting** `Occasion` slot per actor (`Knowledge::occasions`, landed in **M4** with the six
methods, A1b), `OCCASION_LIFE_GAME_HOURS = 1.0`, spent on a **successful** raise only (so
`render_prompt_and_drain`'s failure rebound cannot burn it). The prompt-side gate is
`actions::may_raise_word(world, actor)` — O(1) in `render_prompt`, which matters at 20,000.

**Life is game time, but an offered occasion outlives its hour.** One game hour is 150 real seconds
at the shipped clock and **2.5 real seconds at the `T` key's 60×** — shorter than any provider turn —
and off-stage the idle rotation is neighbourhood-gated, so "longer than any inter-turn gap" is false
in both directions. So `Occasion` carries `offered: bool`: `render_prompt_and_drain` sets it when it
renders a sheet with the verb on it (`Knowledge::offer_occasion`), `apply_reply` and `apply_failure`
clear it (`withdraw_offer`), `expire_occasions` drops only **un-offered** entries older than the
hour, and `may_raise_word` treats an offered occasion as live whatever its age. An occasion rendered
onto a sheet therefore survives until that exchange's reply lands or fails, and no more (M4 A1b,
A-13).

**Limb 1 — an assertion they cannot answer.** Stamped inside `actions::say` (`actions.rs:562`,
where the hearer list already exists), for **the addressee** — the explicit `target` when the verb
named one, else the nearest LLM hearer, which is the very person `Engine::player_say` hands the reply
slot to (M4 A12; a bare `target == Some(h)` reading would make the player's untargeted chat line arm
nobody, and the toy would be dead):
1. cheap pre-filter, **not** the gate: the text carries no `'?'` and is at least
   `OCCASION_MIN_ASSERTION_CHARS = 24` bytes — this only skips "Aye.";
2. `named = subjects_named(world, h, text)` — the actors whose display name appears in the text,
   resolved against `h`'s own sheet-name set (`h.state.knows` ∪ everyone within `HEARING_RADIUS_M`
   of `h` ∪ the speaker), minus `h`;
3. **the lookup:** `knowledge::holds(world, h, f.id).is_none()` for **every** live fact whose
   `subject` intersects `named` — or, when `named` is empty, for every live fact whose `subject`
   contains the speaker. If any such fact **is** held, the gate is closed: they already have the
   word, so there is nothing to coin.

Bounded by `named.len()` (≤ ~30) × the actor's ≤ 6 holdings. It is literally "asserted something to
them that they do not hold", and it **fires for the player's lie**, because a novel claim names
nobody the store has a fact about, so step 3 is vacuously true. It does *not* fire when the hearer
already holds the thing — repetition needs no verb (`../01_facts.md`, "One verb").
Note deliberately **not** requiring `knows` membership for the named subject: `knows` is seed-only
for LLM actors (D16) and averages 3.2 people, so that requirement would make the limb
near-unfireable.

**Limb 2 — a witnessed event that minted nothing.** `mint::note_unminted_event(world, event,
game_days)`, called from the one place events are handed out, for every `DomainEvent` whose
`event_type` is `EventType::WorldEvent` (`event.rs:16`) and whose `kind` is **not** in `MINT_KINDS`,
for each LLM actor within `HEARING_RADIUS_M` of `event.position_m`. Restricting to `WorldEvent`
is the narrowing: speech goes through limb 1, item traffic is not news, sounds and gestures are not
events about the world.

### D35 — `raise_word`'s guardrails are unforgeable
`source` is always `FactSource::claimed(speaker)` — not a parameter. `seeded` is the speaker alone.
`decays: true` always. `subject` resolves only against actors on the speaker's own sheet (their
`knows` ∪ everyone within `HEARING_RADIUS_M` ∪ the occasion's subject), never inventing a person.
**A claim is a template too** (D58): `mint_claim` replaces the resolved subject's display name in
`said` with `{subject}` (word-boundary, case-insensitive — `text_mentions_name`'s own matcher), and
`garble = GarbleMask::default_for(topic)` ∩ the placeholders the text now carries — `subject` only if
the substitution happened, `place`/`day` never (a free-text claim names neither). `default_for` is
`GarbleMask::ALL` for all nine topics: a claim cannot seal itself; what it can garble is whatever its
sentence names.
An unrecognised topic tag lands on `Talk` via `Topic::parse_or_talk`, so the failure direction is
downward. One raise per actor per office (`RAISES_PER_OFFICE = 1`), and a raise whose
`(topic, effective subject, place, day)` already exists in that ward's air is refused as a no-op.
Three new `ActionErrorCode` variants (`NoOccasion`, `WordAlreadySaid`, `WordAlreadyInTheAir`) all
**collapse onto `CommandErrorCode::InvalidAction`** in the exhaustive match at `error.rs:378-428`, the
way the five chalk codes do at `error.rs:421-425` — the compiler points at `error.rs`, not at the
verb, so the wrong mapping is easy to "fix" in by accident.

### D36 — M4 re-fires Q5's with-occasion probe with an unrelated example
M0's with-occasion result is **contaminated**: the fence's own `raise_word` example line was
byte-identical to the correct answer for that scenario, on all three sheets. So the round shows
nothing about whether a model picks a sane topic from the closed nine or composes its own claim —
which is the entire safety argument for risk 4. M4 re-fires it with an example whose topic and
`said` are unrelated to the scenario (a `bread` example on a law occasion). The no-occasion control
is uncontaminated and is what threshold 3 rests on.

### D37 — `arm_actor` seeds the goal and nothing else, and it lands in M1
`pub fn arm_actor(&mut self, id: &ActorId, goal: Option<String>)` on `World`, **M1 step 12**, writing
`state.goal` exactly as world creation seeds it (`character.rs:527`, `goal: sheet.goal.clone()`), a
no-op with a diagnostic for an unknown id. No memories parameter, so nothing can pass quest-critical
propositions through it: a seeded memory is erasable by `forget` on the actor's first turn
(`actions.rs:2609`) and a quest whose hinge is a memory becomes unwinnable with no error raised.
Private knowledge is a fact with a one-person `seeded` set and an `own` string — re-derived every
turn, un-`forget`-able, invalidatable by the sim. Pinned by
`arm_actor_seeds_the_goal_and_set_goal_still_wins` (M1 T34): goal set, `stored_memories` unchanged,
and a later `set_goal` verb wins.

### D38 — `quiet_among` is frozen at mint, and that is safe because both inputs are seed-time facts
`quiet_among: BTreeSet<ActorId>` = { the subject } ∪ { `LoreProfile.father`, `mother`, `children` }
∪ { anyone whose household door is within `HOUSEHOLD_EPSILON_M = 0.5` of the subject's }, computed
once at mint or load, so `salience()` is one `contains` in the innermost roll and never a
per-listener scan.

Freezing is safe, and here is why rather than a shrug:
- `LoreProfile` is loaded once from `lore/characters/**` and **never written at runtime** (the only
  assignment to a kin field in the tree is a test fixture at `prompt/mod.rs:2056`);
- `Townsperson.home` is written once, inside `Round::seed`'s enrolment (`round.rs:3107` →
  `round.rs:3172`), and there is no later `.home =` anywhere in `round.rs`.

The door cohort is read off `World::household_doors: Arc<BTreeMap<ActorId, Vec3>>`, which
`Round::seed` publishes once and nothing else writes. In a round-less world (the goldens, most unit
tests) that map is empty and `quiet_among` is subject ∪ kin, which is correct there because nobody
has a door. **A test pins the second claim** (`the_household_door_map_is_written_once`), and the doc
comment names the fix if a future feature ever moves house: recompute `quiet_among` for every live
fact in the same call that moves the door.

The kin graph is the only real "lives with it" relation the authored cast has — all 413 baked doors
are distinct points and the minimum separation between any two is **1.27 m** (measured), so
`HOUSEHOLD_EPSILON_M = 0.5` cannot produce a false positive and door equality fires only for the
generated crowd, which literally shares a `nav::Door` under the occupancy cap. Including both limbs
makes "the last people to hear a scandal are the ones who live with it" testable on the shipped city
and not only at `--extra-ambient`.

### D39 — Invalidation is a sweep, and `still_true` is `pub(crate)` on the sealed type
`FactSource::still_true(&self, world) -> bool` lives inside `source.rs` and is `pub(crate)`, so the
one thing that reads provenance is the sweep and nothing else. A `false` drops the fact from `live`
and from every holding on the next sweep, which removes it from every sheet on the next turn with no
`forget`, no LLM cooperation and no drift.

---

## E. Process

### D40 — The golden bless is one commit, and the acceptance check is the diff's **uniformity**
Command: `rtk proxy cargo test -p cathedral-sim --test golden_prompts regenerate_golden_fixtures -- --include-ignored`
(the rtk hook swallows a bare `-- --ignored`). The commit does nothing else and its message says so.
The check is **not** "the tests pass": `git diff --stat crates/cathedral-sim/tests/fixtures/prompts/`
must show **+23 insertions and 0 deletions on all 22 `.txt` files** (+1387 bytes each), because the
only intended change is the one unconditional ignorance paragraph (1386 bytes, 22 lines, plus one
blank separator), inserted before `turn.j2:194` (D13). A
fixture that moved by a different amount means a section is rendering unconditionally, and
`features/implemented/chalking_the_walls.md:337` is explicit about what to do then: **fix the
rendering, do not bless it.** Every milestone from M2 on carries "goldens byte-unchanged from M1" in
its `done_when`, so a later unconditional render is caught by the milestone that introduced it rather
than by a confused re-bless.

### D41 — M0 wrote no shipped asset, and M1 lands the tree green
M0's output is evidence and prose files under `features/knowledge_and_rumor/m0_evidence/`. Nothing
in `assets/` or `crates/` moved. M1 transcribes and blesses in one milestone, so no boundary ever
hands the next agent a red tree — there is no CI here, and the three commands below are the entire
gate.

### D42 — Verification at every commit boundary, all three commands
```sh
cargo fmt -- --check <touched files only>      # never a bare tree-wide cargo fmt
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```
The `--workspace` form is not optional: the 64 KiB prompt canary and the 160 KiB snapshot canary live
in `crates/cathedral-backends/src/world_data.rs:422` and `:444` and do **not** run under
`cargo test -p cathedral-sim`.

### D43 — Adding a `PromptStrings` key is a **four**-site edit
1. the `pub` field on `PromptStrings` (`prompt/mod.rs:60-133`);
2. the key in `assets/prompts/strings.toml`, appended **flat** — the file has zero `[table]` headers
   and `PromptStrings` is `#[serde(deny_unknown_fields)]`, so one table header would swallow every
   later top-level key;
3. `fn strings()`, the test helper (`prompt/mod.rs:1875-1910`);
4. the inline `\n`-joined TOML literal inside `a_strings_file_without_the_placeholder_is_rejected`
   (`prompt/mod.rs:1922`).

Miss the fourth and the failure surfaces as a missing-field error inside a test asserting a `%s`
error, across ~14 test binaries. This feature adds **29 keys in M1** (the 24 of `strings_draft.toml`
plus `day_today`, `day_yesterday`, `day_days_past`, `day_long_ago`, `place_unknown`) and **one in M3**
(`known_from`); the trap fires thirty times. There are **two** `Sheet { … }` literals to extend
(`prompt/mod.rs:1081` in `build_sheet` and `:1956` in `the_you_line_folds_lore_and_omits_absent_fields`;
the third at `:2065` is struct-update syntax and needs nothing).

### D44 — Placeholder validation follows the `accept_with` idiom, and the placeholder is `%s`
`PromptEnv::new` already rejects a strings file whose `accept_with` / `walking_to` / `following`
lack `%s` (`prompt/mod.rs:182-196`). The new keys use the file's own `%s` — the form
`strings_draft.toml` freezes — and are validated by **counting** `%s` occurrences (`matches("%s").count()`):
exactly **one** in each of the 21 `know_hedge_*` keys, in `day_days_past` and in `known_from` (M3);
exactly **two** in `unknown_person_role`, rendered by two sequential `replacen("%s", …, 1)` calls in
the frozen order (trade, then ward); **zero** in `know_note`, `know_discipline`, `day_today`,
`day_yesterday`, `day_long_ago` and `place_unknown`. Named braces are not used: order is fixed by the
frozen text, and a second placeholder syntax in one file is one more thing to get wrong.

### D45 — Strictly serial milestones; fan out inside them
M1 → M2 → M3 → M4 → M5, no overlap: M2 needs `holds()` and the block, M3 needs a working wave to
garble, M4 needs a chain to walk back, M5 needs M2's checked-in baseline. Inside a milestone, the
split is stated in that milestone's own file. Two rules bind every fan-out: the golden bless is
alone in its own commit, and M5's final tuning pass runs last and alone, after every other M5 change
is in, because it re-measures against the flat-table baseline.

### D46 — M5's credit refusal needs its own deadline map
A vendor who refuses credit on a held fact is **guaranteed** to re-queue forever: the ladder's only
stall filter is affordability, so the refused buyer still has the coin and the stall still has the
bread, and they rejoin the queue on every `next_decision`, one inbox line a lap. It needs a **new**
`knowledge_refused_until: BTreeMap<ActorId, f64>` on `Round` — not a reuse of `chalk_refused_until`
(`round.rs:1153`), so the trace line names one reason — pruned at the top of `round::tick` beside its
sibling (`round.rs:5735`), and the predicate re-asked in the `None =>` arm at the call site because
`try_purchase` (`round.rs:6768`) has no `now`.

### D47 — M5's hearsay rung is declared **first** in `Rung`
`Rung` derives `Ord` by declaration order (`notices.rs:104-112`) and `Notices::against` sorts
worst-first with `right.rung().cmp(&left.rung())`, so a variant added at the **bottom** of the enum
becomes the **highest** rung and would sort a hearsay word above a warrant on the HUD's standing
line. Declare it above `Word`.

### D48 — Host resources cross as `&mut ResMut<_>`, never `&mut T`
`process_engine_message` (`src/smart_actors/mod.rs:1186`) documents this at `:1189-1199`: coercing at
the call site goes through `DerefMut` and stamps the changed tick on **every** message (Clock and
Weather arrive every poll), silently making every downstream `is_changed()` gate permanently true.
This has shipped as a bug once. The journal resource follows suit, with the paired regression test
`an_ordinary_poll_leaves_the_journal_unflagged`.

### D49 — Ablation switch
`World::knowledge_enabled: bool` (`config.ron: smart_actors.knowledge.enabled`,
`CATHEDRAL_NO_KNOWLEDGE=1`), gating **readers and writers both**, following `marks_enabled`
(`world.rs:174`): turning it off stops new facts *and* stops the sheet block and the systemic
readings, so an ablation run is a real ablation.

### D50 — Every drive/shot script runs with `CATHEDRAL_HEADLESS=1`
Somebody is working at that desktop. And drive scripts exercise the real handlers, so a `click` on a
settings pill persists to `config.ron` — back it up before any journal script that touches the Esc
menu.

---

## F. Reconciliation decisions (2026-09-04)

Added when the three critiques (`critique_contract`, `critique_executable`, `critique_arithmetic`,
now deleted; `README.md` records what each changed) were applied. Each names the finding it settles.

### D51 — The self-subject rule lives **outside** the salience product: `knowledge::may_carry`
```rust
/// Whether this person can ever carry this fact as news. The subject cannot:
/// they hold it at hops 0 because they were there, or not at all. A rule and
/// not a number, so it sits outside the salience product and both measurement
/// levers (`--pollen-flat`, `--pollen-no-salience`) leave it standing.
pub(crate) fn may_carry(fact: &Fact, who: &ActorId) -> bool { !fact.subject.contains(who) }
```
Called **first** in `pickup_chance` (returns 0.0), in `knowledge::volunteers` (returns false) and in
`hop_on_stage`'s listener loop. `salience()` is then the **pure table product**
`base(topic) × affinity(listener) × household`, and `salience_for` under `--pollen-no-salience` is a
bare `1.0`. That is what makes the flat-table identity exact: with the rule inside `salience()` the
no-salience run let a subject pick up their own fact (shipped `vell.stall.pitch`'s `dv8ll`, and the
pack's `p0043`) while the flat run never did, and `pollen_flat_reproduces_the_no_salience_run` failed
by construction. `flat()` also sets `household = 1.0`, which the loader rejects — its doc comment says
it bypasses the loader on purpose.

### D52 — A standing fact (`decays: false`) has no warm life
A `decays: false` seeded holder sits at heat 1.0 forever, so with the M2 model as first written they
deposited on every poll and `bale.promise` reached 58% of a ward a day — the opposite of the spec's
"travels almost nowhere". **Decided:** `knowledge::volunteers` returns `false` when `!fact.decays`;
a standing fact is never heat-seated on a non-seeded sheet and never auto-deposited; it enters a
ward's air only through `knowledge::reheat` → `Knowledge::stir_up` at `REHEAT_TO` when relevance
seats it on a speaking turn (M4 A7/A10) — asked for, not loud. **Carried** rows of it are stamped
like news (`learned_on = game_days` always; only `Held::seeded` rows stay unstamped at 1.0), so its
three holders are always answerable through relevance and their own seat on the sheet still renders
(the relevance limb never consults `volunteers`). Consequence: ≈ 0.11 expected new carriers per ask
in a 53-person ward (`02_numbers.md` §4). Test: `a_standing_fact_is_answerable_and_never_loud` (M2).

### D53 — `VOLUNTEER_HEAT` is solved from the boundary walk, not from an office: **0.119**
The slow end leaks at a **distance**, not an hour: the leg that fires at the mint bell carries the
witness across the ward boundary, and at 0.115 an off-affinity `Craft` witness stayed warm for
0.737 gh while Jonet Kett crosses the Wick/Weigh boundary 95 m (0.30 gh) into her Dayspring walk and
polls within 15 game minutes of it — the slow-end fact was being said in Weigh inside forty game
minutes. **Decided:** require `t_warm(0.12, 0) < d_b / WALK_SPEED_MPS` in game hours, with `d_b` the
nearest foreign-ward boundary from the mint place (the Wickmarket: ≈ 82 m → 0.26 gh) →
`VH > 0.12 / 2^(0.26/12) = 0.1182` → **`VOLUNTEER_HEAT = 0.119`**, `t_warm = 0.145 gh ≈ 9 game
minutes`. Every bracket survives (`02_numbers.md` §3); `REHEAT_TO = 0.119 × 1.10 = 0.1309`; every
warm-life cell shrinks by `12·log₂(0.119/0.115) = 0.59 gh`. The general retune rule is in
`02_numbers.md` §10.

### D54 — The band's realised backstops are **deposits**; the fast end is stated in bells
`CrossingTally::out_of_mint` counted every exit of the mint ward by any holder, warm or cold, so the
slow end's backstop (`≤ 2`) failed on a sound model (Jonet Kett alone exits Wick four times a day).
**Decided:** `TopicRow::wards_reached` = **the number of wards whose air holds a row for the fact**
(never holders' wards), and every realised backstop reads it: S1/S2 `wards_reached(craft) == 1` at
11 gh and 24 gh; F1 `wards_reached(bed) ≥ 2` by the first bell after the mint + 1.5 gh and `≥ 4` by
the second bell + 1.5 gh; F2 `== 8` at 24 gh. The authored cast has zero leg lag, so crossings are
seven columns a day at the bells and not a Poisson stream; the closed form's uniform `X` is kept
for **daily** figures only, `X` is the **mint ward's own exit rate** (`exits[W] / person_game_hours[W]`),
and the pack is planted at the Dayspring bell — the best case for the fast end and the worst for the
slow one. Holder exits stay a printed diagnostic (`holder_exits`), never an assertion.

### D55 — Ward-adjacency seep: **declined**, no knob
`02_rumor_pollen.md` says it "exists as a knob and ships off". A config field with no reader is dead
code, and "people carry it" is the fiction the whole transport half is built on. No field, no flag;
recorded in the close-out table (M5 step 17).

### D56 — The crate root re-exports nothing from `knowledge`
M1's rule (step 2) wins over `01_api.md`'s earlier `pub use knowledge::{…}` block: `knowledge::holds`
is one letter from `Character::holds` and `Custody::holds`, and a root re-export invites the `use`
that D7 forbids. Handles that must be reachable as `cathedral_sim::X` are `AreaKey`/`FactKey`
(`pub use ids::{…}`), `JournalEntry` and `CivicRope` (`pub use engine::{…}`). M5 precondition 1
checks `pub mod knowledge;` only.

### D57 — One signature for `what_you_know_lines`, and `relevance_seated` exists from M1
```rust
fn relevance_seated(world: &World, actor: &Character, since: &[&str], recent: &[&str]) -> Vec<FactKey>;
fn what_you_know_lines(world: &World, actor: &Character, since: &[&str], recent: &[&str],
                       strings: &PromptStrings) -> Vec<KnownLine>;                     // M1
fn what_you_know_lines(world: &World, actor: &Character, since: &[&str], recent: &[&str],
                       present: &[ActorId], strings: &PromptStrings) -> Vec<KnownLine>; // M5 adds `present`
```
`since`/`recent` are `build_sheet`'s own post-`fallback` vectors (`prompt/mod.rs:1039-1041`), so the
sheet and the re-heat read one haystack. `relevance_seated` is a named function from M1 (the
relevance limb on its own, ascending `FactKey`); M4's `render_prompt_and_drain` calls it with the same
two vectors built the same way (`fallback(&drained, &strings.nothing)`,
`fallback(actor.recent_history(), &strings.nothing_yet)`), so A-17's "same set" holds by
construction. `01_api.md`'s `(&ActorId, Option<&[String]>, …)` form is withdrawn.

### D58 — A claim is a template: the resolved subject's name becomes `{subject}`
With `said` installed verbatim, "Grigor Ashe gave short measure" reached every reader with the literal
name, `render_line`'s unknown-people ladder never ran, and a garbled `view.subject` was invisible
while `holds_about` acted on it. **Decided** (D35): `mint_claim` substitutes the first resolved
subject's display name with `{subject}` (word boundary, case-insensitive), sets `garble.subject`
only if that substitution happened and `place`/`day` never, and keeps the speaker's `own` line as
the words they actually said. `GarbleMask::default_for(topic)` is `GarbleMask::ALL` for all nine
topics; the loader's own "may garble only what the template names" rule applies to route 3 at mint
time. Tests: `a_claim_names_its_subject_through_the_placeholder`,
`a_claim_garbles_only_what_its_template_names` (M4).

### D59 — `DOOR_SHUT_REACH_M` is the authored idle leash, 10 m
A person "at home" mills within their archetype's `leash_m` (10.0 for every authored archetype,
`assets/world/rounds.json`), so a 4 m threshold (`ITEM_INTERACTION_RADIUS_M`) would find the
householder "at their door" only by luck. `pub const DOOR_SHUT_REACH_M: f64 = 10.0`, with the leash
named in its doc comment and a test that every authored archetype's `leash_m ≤ DOOR_SHUT_REACH_M`
(`the_door_reach_covers_the_idle_leash`, M5), so a widened leash fails loudly instead of silently
switching the reading off.

### D60 — The projection-walking test exists, as the spec's backstop to the structural seal
`fact_source_reaches_no_projection` (M1 T35; M4 extends it with the `Journal` message, M5 with
`WardHeat`, `knowledge_lines`, `PollenCensus::topic_lines` and the diagnostics): seed one fact
sourced `{"quest_phase": {"quest": "zzsentinel", "phase": 1}}` and one `{"custody": "<sentinel id>"}`,
then assert the sentinel strings appear in **none** of: `render_prompt` for every roster actor,
`serde_json::to_string(&world.public_snapshot(&player))`, `format!("{:?}", msg)` over every
`EngineMessage` from ten `Engine::poll`s, and (as each lands) `Engine::knowledge_lines()`,
`PollenCensus::topic_lines()`, every `JournalEntry` and `WardHeatRow`. `claimant()`'s one bit is the
spec's own walk-back affordance and is exempt by name in the test's doc comment.

### D61 — A-1 walks **every** action-reachable mint by name
`raise_word` (the verb), `EngineCommand::DebugRaiseWord`, `raise_notice` → `mint_from_notice`
(an LLM verb reaching a coded mint: `FactSource::event`, and its `{deed}` sits inside a fixed
template, which is the reading — the fact asserts that the accusation *exists*), and from M5
`draw_mark`/`scrub_mark` → `mint_stranger_deed` (`FactSource::event`, the player's hand only).
A-1 asserts each route's source kind and that **`mint_claim` is the only route that installs free
text as `said`**. M5 re-runs A-1 after adding its rows.

### D62 — The player's poll seat lands once (M2), the occasion store lands once (M4)
M2 step 15 puts `pollen::poll_player` and `next_player_pollen_game_days` in `Engine::poll`; M4 A11
adds only `expire_occasions` after it — not a second `poll_player` call. `Occasion`, `LearnedHow`,
`Knowledge::{occasions, raises, occasion, note_occasion, spend_occasion, expire_occasions, note_raise,
raises_left, offer_occasion, withdraw_offer}`, `reheat`'s body, `player_learned` and `seated` are
**M4's** (A1b); `area_adjacency`, `household_doors`, `ward_at`, `pollen.rs`, `mint.rs` and
`take_stir_beat` are **M2's**; `garble.rs`, `chain`, `hop_on_stage` are **M3's**; `arm_actor`,
`invalidate_stale` and `relevance_seated` are **M1's**. `01_api.md` annotates every block with its
milestone so no precondition table can mis-attribute one again.
