Status: IMPLEMENTED — M0–M5 complete and reviewed (2026-09-06). M3 Q4 passed on both providers; M4 C1 under-fired on both (accepted). UI verified with software rendering; real-GPU checks unavailable in this environment. Final results: `m5_measurements.md`; independent review: `plan/m5_review_2026-09-06.md`.

# Knowledge and rumour

*What a person knows, where they got it, and how wrong it has become on the way.*

One feature in two halves, shipped together:

| Half | Owns | Chapter |
|---|---|---|
| **Facts** | The proposition. Who holds it at first hand. What a holder says, what a non-holder says instead. How it renders on a sheet. What the player has learned. | `01_facts.md` |
| **Pollen** | How a fact travels to people who were not there: the ward's air, carriers, stage-local hops, heat, decay, garbling, provenance. | `02_rumor_pollen.md` |

## Files

| File | What it is |
|---|---|
| `README.md` | This: the shape, the decisions, the seams, the milestones, the risks |
| `01_facts.md` | The `Fact` type, `holds()`, the merge rule, the `what_you_know` block, the ignorance rule, player receipts, the journal |
| `02_rumor_pollen.md` | The transport half — the ward's air, stage-local hops, **salience** (topics, affinity, damping), the cadence band, garbling and the chain |

## Schedule decision (2026-08-30; reconciled at M5)

This core mechanic was built end-to-end before resuming quest work. The three quest specifications
(`quest_the_bale_that_gained_forty_pounds`, `quest_ring_a_dead_womans_name_at_marenstide`, and
`quest_secure_votes_for_a_drainage_funding_plan_before_the_rain`) now consume the shared API; they
own neither a knowledge type nor a player receipt store. Their knowledge prerequisite is satisfied
by M5. The quest mechanics themselves remain separate work.

The base game's own arrests, notices, knells and drawn/scrubbed marks supply news. Quests add JSON
packs to an already measured propagation system. `plan/` preserves the serial implementation
contracts, corrections and reviews; the three chapters here describe the resulting API.

## Why one feature and not two

`Fact` and `Pollen` were separable on paper, and an earlier draft proposed shipping facts first with
a deterministic gossip roll standing in for propagation, upgrading to real hops later. Rejected: it
buys a smaller first milestone at the price of an interim system that has to be reasoned about,
tested and then taken out again.

## The one idea

**A fact is the noun. Pollen is the transport.**

A proposition has one id and one identity wherever it goes. *How you came by it* is separate from
*what it is*:

| Holding | hops | Garbles | Decays | Where it comes from |
|---|---|---|---|---|
| You did it, saw it, or were authored to know | 0 | no | event heat cools; authored standing facts persist | the fact's `seeded` set, or actual earshot at a successful mint hook |
| You were told, *n* hops out | *n* | yes | cools, but is not forgotten | the ward's air, or a mouth beside you |

That split is what lets one system carry both *"I promised her bolt a place on that cart"* (authored,
sealed to three people, never drifts) and *"they say Ede was taken at the Wickmarket, two days past"*
(minted from an arrest, four hops out, the day already wrong).

```rust
/// None means they have never heard of it at all.
pub fn holds(world: &World, actor: &ActorId, fact: &FactId) -> Option<Held>;
```

`knowledge::holds` / `holds_key` resolve a stored override before the seeded default. The prompt,
journal and quest readers use that API; disabling knowledge makes it return `None`. Heat is derived
from learning time, so an event seed can cool while retaining pristine first-hand knowledge.

## The third axis: not every fact spreads alike

`hops` says how far a telling has come and `heat` says how fresh it is. Neither says whether the
thing was **worth repeating in the first place**, and without that every fact crosses the city at one
speed — an adultery and a quarrel over a stall pitch arriving in the Weigh Ward on the same
afternoon, which is the one thing about gossip everybody already knows to be false.

So a fact carries a `topic` from a closed set of nine (`Bed`, `Blood`, `Law`, `Omen`, `Stranger`,
`Coin`, `Bread`, `Craft`, `Talk`), and the pickup roll gains one term:

```
hash(fact, actor, stir)  <  ( curiosity_of(actor) × air.heat × salience(fact, actor) ).clamp(0.0, 1.0)
```

Two things about that, both load-bearing, both in `02_rumor_pollen.md`:

- **Salience is not heat.** Heat answers *is this current* and decays; salience answers *is this worth
  repeating at all* and never does. They multiply, and what falls out of the multiplication is
  **a cold scandal out-travels a fresh squabble** — which is a test, not a feeling.
- **The number is never on the fact.** A float per fact is five hundred floats nobody can reason
  about. The topic is on the fact; the number is in one designer-owned table keyed by topic and by
  the listener's trade. Base `1.00` is *defined* as the cadence target this feature already had, so
  the top band is not a new quantity, and setting every band to `1.0` reproduces the pre-salience
  model exactly.

Salience is a **spread rate, never an importance ranking.** `bale.promise` — the hinge of a whole
quest — is a `Talk` fact and travels almost nowhere, and that is why that quest is hard. A dull fact
reaches the player by being *asked for*, which relevance selection already handles.

## Where facts come from, and which of them an LLM writes

Three routes: **authored** JSON, **minted** at explicit successful-operation hooks, and — new — **coined by
a mouth** (`raise_word`, M4). The first two get their topic from an author or from a constant per
event kind. Only the third has to decide, and the division of labour there is the answer to the
obvious question about letting a model set how juicy its own gossip is:

> **The mouth says what kind of thing it is. The city decides how far that kind of thing travels.**

A model rating the importance of its own utterance inflates — everything it just chose to say feels
worth saying. A model *classifying* it does not: the list is closed, the tag has an external check,
an unrecognised tag falls back to the dullest band, and the number stays with the designer. And
everything a mouth coins carries `FactSource::claimed(speaker)`, which is the whole safety argument in one
field: **a model can mint claims; it can never mint truths.** Full guardrails in `01_facts.md`,
"Where facts come from".

**And the model is never asked to judge when to use it.** `raise_word` is in an actor's verb list
only when the sim has put an occasion there — somebody asserted something they do not hold, or a
percept reached them that minted no fact — capped at one per actor per office and refused on a
`(topic, subject, place, day)` collision. That is the same discipline `draw_mark` and `raise_notice`
already have, and it is what makes "how does the model know?" a lookup rather than a hope. It will
under-fire; coded mints are the staple, and a quiet verb is a far cheaper failure than a loud one.

**Repetition gets no verb at all.** A warm carrier already deposits on every due pollen poll, and a cold
fact re-heats when somebody asks about it — a sim-side rule, because warming is a consequence rather
than a decision. That rule quietly hands the player the best verb in the feature: **asking about a
dead story is what revives it.**

One consequence is worth having on purpose: **the player cannot set the salience of their own lie.**
They get whatever the mouth they told makes of it — so to spread something you make it sound like a
scandal to the person in front of you, and to bury it you tell it to someone who will hear it as a
trade matter.

## The one idea's mirror, which is just as load-bearing

**A knowledge system that only adds knowledge cannot be perceived.** If a stallholder answers
"who carried that bale to the gate?" with a confident invented name whether or not she holds the
fact, then holding the fact changed nothing observable, and the whole feature is invisible to the
player no matter how good the propagation is.

So the same feature owns the negative:

- an actor who holds nothing on a subject **says so**, and
- says it **directionally** — not "I don't know" but "I don't know; the porters were at the gate,
  ask them."

Ignorance that names the next mouth is a lead. Ignorance that dead-ends is a wall, and a city of
walls is not investigable. M0/M0b measured that prompt discipline before the store existed; M1
installed it unconditionally immediately before “Use ONLY the verbs listed below” in `turn.j2`. The full rule is in `01_facts.md` under "The ignorance rule".

## What this is not

- **Not `notices.rs`.** A notice is an accusation with a legal ladder (Hearsay → Word → Summoned → Warranted; an individual hearsay notice can be summoned directly)
  and a settlement path. A fact is a proposition. They share an *idiom* — `notices::carries`
  (`notices.rs:419`) is the proven deterministic-roll pattern this borrows, and `word_in_the_ward` is
  the proven prompt block this sits beside — but they are not the same type and must not be merged.
  A reviewer will ask; that is the answer.
- **Not memory.** `stored_memories` is durable, LLM-owned and LLM-erasable — the turn prompt actively
  instructs every model to `forget` what is stale, and `actions.rs:2603` is the only production
  writer. Nothing quest-critical may live there. A fact is re-derived into the sheet every turn and
  cannot be forgotten.
- **Not a clue log.** The player's side records what they *heard or caused*, in the sentence they
  heard, attributed. It never lists objectives.
- **Not an oracle.** `holds()` answers what one person has. It is never asked "what is true" by
  anything the player can see.

## Seams

| Seam | Shipped use |
|---|---|
| `Engine::announce_commitment`, `actions::raise_ward_notice_for`, player mark actions | Successful-operation mint hooks. They compute their own earshot: `DomainEvent::recipient_ids` is empty for commits and can be delivery-gated. |
| `EngineCommand::Knell` / `CivicPeal` and accepted soundscape cue/curfew paths | The host sends an audible cause to the sim. Knell mints anonymous Blood news; both ropes amplify matching air. |
| `round::tick_pollen`, immediately after `decay_needs` | Whole-cast due queue, independent of attention/round activity; game-day deadlines, jittered ten-game-minute polls. |
| `knowledge::pollen::hop_on_stage` | Own two-real-second O(N) scan, eight nearby carriers and pairwise 20 m reach; `attention::on_stage` remains unchanged. |
| `World::ward_at` / `pollen::ward_grid` / `crowd::ward_map` | One exact 8 m accelerator with a nearest-mark fallback, shared with crowd placement. |
| `AreaMap::containing_area` / `location_description` | Named event/hearing places, separate from the eight planning wards. |
| `knowledge::holds`, `relevance_seated`, `reheat` | Held-version queries, three relevant/warm sheet seats, and sim-side revival after a speaking turn. |
| `round::try_purchase` / stall selection | Warm Coin refusal, half-game-day backoff and another vendor after refusal. |
| engine/attention admission, `knowledge::raise_hearsay_words`, prompt relevance | Own-home idle door gate with knock/All-mode escape, lowest-rung hearsay summons, and greeting register. |
| `EngineMessage::Journal` / `WardHeat` | Separate bounded player projections, outside `PublicSnapshot`; J overlay, standing HUD, fullscreen-map dots. |
| frozen lore kin and `Townsperson.home` | Household damping computed at mint; a future house move must recompute it. |

No propagation rides an LLM turn or the scheduler. Layer 1 pays a bounded lookup for each due
person; Layer 2 pays its own gated scan where the player can see the result. The cost measurements
include that scan rather than treating it as free.

## `arm_actor`: narrowed to the goal

`state.memories` and `state.goal` are **seed-only plus LLM-editable**: seeded from `sheet.memories` /
`sheet.goal` at world creation (`character.rs:527`), and after that written only by
`remember`/`forget`/`set_goal`. M1 added a narrow sim goal seed for quest arming. Durable private
knowledge about something done off-screen belongs in a fact, rather than an erasable memory.

An earlier draft had `arm_actor` seed both. That is a trapdoor: a seeded memory is erasable by
`forget` on the actor's very first turn, and a quest whose hinge is a memory can be made unwinnable
with no error raised anywhere. So:

```rust
/// Seed a character's standing intention the way world creation does. A
/// **seed**, not an override: the actor's own set_goal must win afterwards, or
/// they stop being a character.
pub fn arm_actor(&mut self, id: &ActorId, goal: Option<String>);
```

**Private knowledge is a fact with a one-person `seeded` set and an `own` string** — re-derived into
the sheet every turn, un-`forget`-able, invalidatable by the sim. That is what facts are *for*. The
rule is enforceable, not aspirational: nothing may pass quest-critical propositions through
`arm_actor`, and there is no memories parameter to tempt it.

## Prompt surface

One new block, `what_you_know`, rendered beside `word_in_the_ward` and bounded like it
(`NOTICES_SHEET_MAX` is 4; facts get their own, smaller cap). Two things decide what a sheet shows:

**Selection is by relevance first, heat second.** "The hottest thing this actor carries" is a gossip
rule, and it is the wrong rule for the interrogation all three quests are made of: ask about the
bale while the ward is loud about an arrest, and the one fact you came for is not on the sheet. So a
fact whose subject, place or a distinctive noun appears in `since_your_last_turn` or
`recent_history` is seated first, regardless of heat; so is a held story about someone within
inclusive 20 m. Heat fills what is left, up to three bullets.

**Phrasing is the measured three-band, seven-rung lookup** in `assets/prompts/strings.toml`.
First-hand holders use an `own` template where supplied; more distant and cold tellings use the
frozen hedges. Salience's authored hedge band shortens the telling's distance without changing its
topic. Unknown subjects render through the reader's known names or occupational roles. M3's real
garbled replies passed the distinctness threshold on both providers; partial hedge erosion and one
misread garbled subject remain documented in the raw evidence.

The block's instruction says to use a held word when relevant. The ignorance paragraph is separate
and unconditional, including on an empty sheet. `raise_word` is conditionally offered only on a
sim-stamped occasion; M4 freezes the offered source/subject until that exchange finishes, so a newer
percept cannot silently change who an in-flight answer attributes the story to.

## Milestones

Implemented serially, with review and the workspace gate between milestones:

| Milestone | Status | Date | What landed |
|---|---|---|---|
| M0 / M0b | Measured | 2026-09-03 / 2026-09-04 | Live-provider prose measurements; `v6_both` frozen, raw calls retained. |
| M1 | Implemented and reviewed | 2026-09-05 | Fact store/catalog, derived heat, sealed provenance, three-seat prompt block, goal-only `arm_actor`, and the single golden re-bless. |
| M2 | Implemented and reviewed | 2026-09-05 | Explicit custody/notice mints, whole-cast pollen, ward grid, salience and household damping, baseline band and cost measurements. |
| M3 | Implemented and reviewed | 2026-09-05 | Stable garbles, provenance chain, own gated stage scan; review fixed pair distance. Q4 remeasurement: 8/8 distinct replies on both providers. |
| M4 | Implemented and reviewed | 2026-09-06 | Player carrier/receipts, scrolling journal and standing HUD, occasion-gated claims, speaker provenance and relevance reheat. C1 under-fired on both providers (accepted). |
| M5 | Implemented and reviewed | 2026-09-06 | Knell/stranger mints, bell amplifiers, vendor/door/hearsay/greeting readings, stale invalidation, ward heat and final bounds/cadence/cost gate. |

The final M5 measurement and review files carry the verification results. Screenshots use the real
UI and sim with an explicitly documented software-rendering fallback: this environment exposes no
NVIDIA device, so a real-GPU/full-textured visual check could not be performed here.

### The cadence band

M4 in the earlier draft said "tuning", which is not a target and cannot be passed or failed. The
numbers this feature is built to hit, stated up front so hop rate, heat, cooling and salience are
derived from them rather than fiddled toward them. It is a **band** rather than a number because one
speed for all news was the flaw:

> **The fast end.** A `Bed` or `Blood` fact minted at the Wickmarket is being said in the Weigh Ward
> within about one office, and has reached every ward inside a game day.
>
> **The slow end.** A `Craft` fact minted beside it, at the same hour, by the same mouth, is still in
> its own ward at nightfall — and may never leave it at all.

The fast end is the original single target, unchanged: salience base `1.00` is *defined* as it, so
the bands and affinities stay fixed. At the shipped `seconds_per_day: 3600`, an office is 2–5 game
hours. M2 measured the Dayspring commute carrying the top band to all eight wards in 2 game hours,
faster than the draft predicted; a two-sided guard now also requires fewer than eight at 1 hour.
Walking and the mint’s bell phase explain that correction, rather than a retuned probability. The slow end is a computation, not an observation:
"may never leave" means the pickup roll's expected ward crossings over a game day is below one.

The headless carriers-per-ward-per-game-hour print, broken out per topic, is the measurement; it is a
test, not an eyeball. The flat-table run — every band `1.0`, reproducing the pre-salience numbers
exactly — is the regression guard on the whole of it.

## Numbers

Final M5 gate: **1,711 tests passed**, zero failed; the unchanged ignored 20,000-person guard also
passed when run explicitly. Bed reaches all eight wards at 2.01 game hours and 446 carriers at the
last sample; off-affinity Craft remains in one ward throughout. Flat/deleted-salience traces are
625 lines byte-identical. No constant or affinity moved.

At 20,000 extra people over a full day, the observed ON/OFF user-CPU increase is **6.18%** and peak
RSS increase **16 MiB**. The store peaks at 15.70 MiB; the separate synthetic store-plus-cache bound
is **25,869,693 bytes**, below 32 MiB. The snapshot stays exactly **137,179 bytes** and the saturated
longest prompt **16,993 bytes**. Single-pair coarse-step timing and software-rendering limitations
are detailed in the measurement report.

Derivations and the measured corrections to the original predictions are in `plan/02_numbers.md`.
`m2_measurements.md` is the fixed baseline, `m4_evidence/cost.json` is the player-side comparison,
and `m5_measurements.md` records the final band, identity and saturated crowd runs. `plan/` retains
all implementation contracts and independent reviews.

| Parameter | Shipped value | Meaning |
|---|---:|---|
| `VOLUNTEER_HEAT` | 0.119 | Solved from the 82 m foreign-ward boundary: `0.12 / 2^(0.26/12) = 0.1182`, rounded upward. |
| `STIRS_PER_GAME_HOUR` | 2 | One stable pickup roll window per 30 game minutes. |
| `POLLEN_POLL_GAME_MINUTES` / maximum | 10 / 15 | Jittered game-time carrier cadence; maximum remains below a stir window. |
| `AIR_HALF_LIFE_GAME_HOURS` | 12 | Derived cooling, independent of poll rate. |
| `HOP_LOSS` | 0.85 | Charged once when learned; fourth-hop heat 0.522 before time decay. |
| `REHEAT_TO` | 0.1309 | Absolute heat, `VOLUNTEER_HEAT × 1.10`, never divided by salience. |
| Garble chance / day bound | 0.35 / ±3 | Stable per-field hop corruption; topic never changes. |
| Live facts / carried rows / ward air | 256 / 6 / 24 | Seeded witnesses are exempt from the six carried rows. |
| Sheet / receipt / projected journal rows | 3 / 64 / 24 | Distinct receipt mouths also capped at 64, plus one unattributed observation. |
| Stage gate / carriers / pair reach | 2 real s / 8 / 20 m | Own scan, inclusive pairwise distance; 432 carried-only or 18,432 absolute candidates. |
| Ward grid / knell / door radius | 8 m / 300 m / 10 m | Exact ward lookup accelerator, clip-pinned bell carry, day-worker home gate. |

The nine bases remain Bed 1.00, Blood 1.00, Law 0.80, Omen 0.80, Stranger 0.80, Coin 0.45, Bread 0.35,
Craft 0.20 and Talk 0.15; affinities remain the M2 table. M5 adds deterministic consequences and
amplification, without another probability.

- **`GARBLE_SUBJECT_POOL_MAX = 24`** (M3) — how many candidates a garbled subject is drawn from. The
  smallest lore ward is **Wallwright at 33 people**, so every subject with a lore ward has at least 32
  cohort members and this cap **always binds**: the walk over `World::roster` stops inside the
  authored prefix and never reaches the generated tail, whatever the crowd knob says. It is also a
  design choice and not only a bound — a given subject is confused with a stable handful of people
  rather than with the whole city, which is what makes a walked chain legible instead of merely wrong.
  Measured on the shipped cast: 20 real subjects, 24 candidates each, byte-identical with 1,000
  generated citizens in the world (`pollen_cadence.rs::the_subject_pool_holds_only_the_authored_cast`).

## Departures recorded at landing

| Earlier assumption | Shipped behavior and reason |
|---|---|
| Event interception; event recipient lists supply witnesses | Explicit successful-operation hooks compute earshot. Commit recipients can be empty or presentation-gated. |
| Stored heat, inline proposition per carrier, public provenance | Derived heat, interned `FactKey` plus `Held` deltas, and sealed `FactSource`; topic cannot be garbled. |
| A block-gated ignorance rule | The measured rule is unconditional, before the verb fence; absence of knowledge is part of its instruction. |
| A claim is a literal sentence | Claims replace the resolved subject with `{subject}` and garble only that slot. Notice mints assert an accusation exists; their deed text can originate with an LLM. |
| Standing facts have an ordinary warm life | They are never volunteered automatically; relevance can reheat their ward air at the fixed absolute target. |
| Ward adjacency seep, or free use of the attention scan | No seep knob. Whole-cast due queue and a separate two-second, eight-carrier stage scan with actual pairwise 20 m reach. |
| Fast news needs most of an office to cross the city | The measured Dayspring commute reaches eight wards in two game hours; the unmodified cadence test pins that correction. |
| Every home idle leash is 10 m | Several shipped trades wander farther. The fixed door predicate applies only while both bodies are within 10 m; Stage idle is gated, All/reaction/priority/inbox paths remain available. |
| Refusing credit is a separate transaction | The round has only a sale path. Coin knowledge refuses that sale, pauses shopping for half a game day, then permits other eligible vendors and the existing household fallback. |
| A garbled accusation can use ordinary witness authority | Hearsay is the lowest rung, summonable and settleable, with no immediate witnessed-breach seizure and no notice-to-fact feedback. |
| Knell subjects, and a large-sale mint | A knell has no name and garbles only its day. The sale mint is declined: no authoritative price/recipient seam and staged transaction clones make it unsafe. |
| Repeated arrivals count as separate mouths | Journal receipts track bounded distinct sources across stirs; the player's own mark deeds get witnessed receipts without a self-subject carrying exception. |
| The old footprint included every allocation | M5 includes spare vector capacity, sealed source strings, household/craft data and receipt sets; consequence caches are measured separately and bounded with the store. |
| Quest-owned knowledge/casebooks and memory seeding | JSON catalog extension, goal-only `arm_actor`, shared `knowledge::holds` and `EngineMessage::Journal`; a quest host must supply phase invalidation when it exists. |
| Full-textured real-GPU screenshots are available here | Only software rendering is exposed. The accepted UI checks use real fonts/map assets in an unmapped, unfocused window; the rendering limitation is explicit in the evidence. |

The chapters explain these decisions at their actual API sections. Milestone reviews retain the
additional corrections to test fixtures, old anchors and measured predictions, without rewriting
historical provider records or deleting user-owned scripts.

## Risks

1. **Confabulation — the load-bearing one.** The failure that kills this feature is not a ward of
   parrots; it is a ward of confident inventors, in which holding a fact and not holding one look
   identical from the player's chair. The shipped mitigations are: the ignorance rule with its directional "ask X" form, the block's *know, don't
   announce* paragraph, and **M0/M0b’s live-provider gate before store code**. Those calls passed the non-invention gate;
   M3’s later garbled-subject misreading remains recorded rather than hidden by that earlier result.
2. **Perceptibility and parroting.** Once spread is code-driven, the LLM must voice the injected line
   often enough for the player to feel the wave, *without every mouth in the ward saying the same
   sentence.* Mitigations: the `own`/`said` split gives holders different words by construction;
   garbling diverges them further; the instruction paragraph; and the per-sheet budget caps how much
   competes for a turn. Also measured in M0 — eight holders, eight sentences.
3. **Cost at crowd scale.** `World::neighbours_by_distance` (`world.rs:478`) is a **linear scan over
   every character** — the sim has no spatial grid. "Every carrier scans everyone within 25 m every
   poll" is O(carriers × N), and the shipped `config.ron` is `extra_ambient_npcs: 1000`, with the
   knob going to 20,000 where the pump is already 179 ms of a 204 ms frame. This is why propagation
   is **ward air plus stage-local hops** and not general person-to-person proximity: the common case
   is bounded work on a game-time due queue, while mouth-to-mouth transport pays its own gated scan
   once per two real seconds. Saturated 20,000-body measurements cover its cost in M2/M4/M5.
4. **Topic mis-tagging, and the reason the model never sees a number.** Route 3 (`raise_word`) lets
   an LLM create a fact, and the thing that could go wrong is not that it lies — lying is the feature
   — but that everything it coins arrives in a high band and the city becomes uniformly loud, which
   is the flat model this design replaces, reached by a longer road. Mitigations, all structural
   rather than hopeful: the model picks a **topic from a closed list of nine**, never a number; an
   unrecognised tag falls back to `Talk`, so the failure direction is *down*; the salience number
   stays in a designer-owned table; and everything coined carries `FactSource::claimed(speaker)`, so a
   runaway is identifiable, invalidatable and walkable back to the mouth it came from. Measure it the
   same way as everything else: the per-topic cadence print will show a claim-heavy run skewing, and
   that is a number, not an impression.
5. **Prompt budget.** A real sheet is already ~13.6 KB. Facts compete with `word_in_the_ward`,
   `the_ward_says`, `your_round`, `you_hold`, `marks_here` and `dogs_nearby` for it. The cap is not
   optional, and relevance selection is what makes a small cap survivable.
6. **Snapshot budget.** Facts are per-actor prompt state and must **not** enter `PublicSnapshot`,
   whose 160 KiB bound already has little headroom (`lore-items-wave`). The shared journal and ward heat project
   separately; quests must consume that journal rather than build a second casebook.
7. **Golden churn.** The sheet block spent the one golden re-bless in M1. Every subsequent
   milestone checks all 22 frozen fixtures byte for byte.
8. **Determinism.** Every roll is a hash of stable inputs, never a fresh draw — the engine polls at
   60 Hz and a re-drawn probability is a certainty within a frame (`attention.rs` learned this the
   hard way). Pickup, carrier selection, garbling and hop order all follow that rule, and every
   collection that reaches a prompt or a golden is stably ordered.
9. **Legibility.** A simulation the player cannot see is indistinguishable from no simulation. The
   journal's provenance line, the "four mouths, two wards" count and ward heat on the map are not
   polish; they are what turns the propagation model into something a player can *play against*.
   Same principle the law-standing HUD line already states: a brand with no visible door is a bug.

## What it unblocks

The three quest specifications now describe the shipped catalog, goal setter and shared journal.
They remain quests to implement, without a separate knowledge subsystem or receipt store.
Magnitude drift, sincerity and Night Office settlement remain future work in
`02_rumor_pollen.md`; hearsay, stranger deeds, bells and walking the chain have shipped.

## M0 — the mouth test (2026-09-03)

> **M0b (2026-09-04) supersedes the two frozen artifacts below with `v6_both`** — the hop ladder split into seven rungs and the referral exemplars replaced by descriptions, measured at the shipping position on both providers (110 more calls): threshold 1 held 5/5 on both, threshold 3 held 0, the exemplar leak is gone on openai (3/7 → 0/7), and the "let it lie" softening was tested and **declined**. **Threshold 2 did not improve on openai (3/8 against 4/8) and stays failed** — risk 2 is handed to M3's garbling with a stated re-measurement.
> Record: `m0_evidence/NOTES.md` § "M0b — measured repairs"; `strings_draft.toml` is now 24 keys and `ignorance_rule.txt` carries the repaired paragraph, both re-verified to round-trip to `m0_evidence/prose/v6_both/`.

**GO.** Confabulation — risk 1, the load-bearing one — is answered by evidence. The full record,
including every reply and every rejected wording, is in **`m0_evidence/`**; `NOTES.md` there is the
justification for the strings this feature ships. `scripts/m0/` was the throwaway harness and is gone.

Three candidate wordings of the block header, the block note, the ignorance rule, the hop ladder and
the unknown-subject template were rendered into 22 hand-authored sheets built on the golden prompt
fixtures, and fired at live providers: **88 calls, 88 ok, 0 failed** (`moonshot`/kimi-k3 and
`openai`/gpt-5.6-luna). **`v2_structural` won and is frozen.**

| threshold | risk | result |
|---|---|---|
| A non-holder asked point-blank refuses **and** names a next mouth (≥ 4/5) | 1 — go/no-go | **15 of 15** across two providers and two prompt positions |
| Invented names, days, places or numbers | 1 | **0 in 43 replies** from the frozen wording; 0 invented person names in the whole round |
| Eight holders in one ward produce materially distinct sentences (≥ 6/8) | 2 | **7/8 on moonshot, 4/8 on openai — FAILS on one provider of two** |
| `raise_word` unused when no occasion is given | 4 | **0 uses**, every variant, both providers |

M0's output is two frozen artifacts, both byte-for-byte the measured text:

- **`m0_evidence/strings_draft.toml`** — the `PromptStrings` keys M1 copies into
  `assets/prompts/strings.toml` (18 at M0; **24 after M0b**, see the note at the top of this
  section), values verified to round-trip to the frozen prose directory byte for byte.
- **`m0_evidence/ignorance_rule.txt`** — the **unconditional** `turn.j2` paragraph, which goes
  immediately before "Use ONLY the verbs listed below" (`turn.j2:194`) and is *not* wrapped in an
  `{% if %}`: the sheets it has to work on are the ones with no `what_you_know` block at all. It was
  re-fired in that exact position to measure it there, not assumed.

**Two things M0 changes about the milestones below.** First, **risk 2 is not closed by prose** — the
anti-parroting result held on one provider and failed on the other, so the hop-rung split (give hops
2, 3 and 4 their own rungs; M0's ladder collapsed them into one, and six of eight holders received
one of only two rendered lines) and **M3's garbling** are load-bearing rather than nice-to-have, and
Q4 must be re-measured on both providers after each. Second, **the ignorance rule competes with
`raise_word`**: both providers declined the verb on a live occasion with this paragraph on the sheet,
so M4 owns that interaction as well as the closed-topic check. Neither is a reason to hold M0, and
the spec's `who_keeps_that_word` fallback was **not** invoked.
