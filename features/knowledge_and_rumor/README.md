Status: M0 measured (2026-09-03, GO — see m0_evidence/NOTES.md); M1 implemented (2026-09-05). M2–M5 pending.

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

## Schedule decision (2026-08-30)

**This is a core mechanic and it is built end-to-end first.** All six milestones below ship before
any quest work resumes, and the three quest specs
(`quest_the_bale_that_gained_forty_pounds`, `quest_ring_a_dead_womans_name_at_marenstide`,
`quest_secure_votes_for_a_drainage_funding_plan_before_the_rain`) are **rewritten against what
actually shipped**, afterwards. Where a quest spec currently says otherwise — that it starts after
M4, that it owns a casebook, that it defines its own knowledge enum — that text is stale and this
document wins. Do not reconcile them by editing the quests now; reconcile them once, at the end,
against the real API.

The reason is not scheduling convenience. A knowledge layer designed against three quests at once
becomes the union of three quests' needs. Designed and tuned on its own, with the base game's own
arrests, knells and market talk as the content, it becomes a city that gossips — and the quests then
get authored into a mechanic that already works.

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
| You did it, saw it, or were authored to know | 0 | no | no | the fact's `seeded` set, or an event's `recipient_ids` at mint |
| You were told, *n* hops out | *n* | yes | cools, but is not forgotten | the ward's air, or a mouth beside you |

That split is what lets one system carry both *"I promised her bolt a place on that cart"* (authored,
sealed to three people, never drifts) and *"they say Ede was taken at the Wickmarket, two days past"*
(minted from an arrest, four hops out, the day already wrong).

```rust
/// None means they have never heard of it at all.
pub fn holds(world: &World, actor: &ActorId, fact: &FactId) -> Option<Held>;
```

`holds` checks the fact's `seeded` set first, then the carrier store. Everything else — the prompt
block, the journal, a quest — goes through that one call.

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

Three routes: **authored** JSON, **minted** from the `World::emit` whitelist, and — new — **coined by
a mouth** (`raise_word`, M4). The first two get their topic from an author or from a constant per
event kind. Only the third has to decide, and the division of labour there is the answer to the
obvious question about letting a model set how juicy its own gossip is:

> **The mouth says what kind of thing it is. The city decides how far that kind of thing travels.**

A model rating the importance of its own utterance inflates — everything it just chose to say feels
worth saying. A model *classifying* it does not: the list is closed, the tag has an external check,
an unrecognised tag falls back to the dullest band, and the number stays with the designer. And
everything a mouth coins carries `FactSource::Claimed`, which is the whole safety argument in one
field: **a model can mint claims; it can never mint truths.** Full guardrails in `01_facts.md`,
"Where facts come from".

**And the model is never asked to judge when to use it.** `raise_word` is in an actor's verb list
only when the sim has put an occasion there — somebody asserted something they do not hold, or a
percept reached them that minted no fact — capped at one per actor per office and refused on a
`(topic, subject, place, day)` collision. That is the same discipline `draw_mark` and `raise_notice`
already have, and it is what makes "how does the model know?" a lookup rather than a hope. It will
under-fire; coded mints are the staple, and a quiet verb is a far cheaper failure than a loud one.

**Repetition gets no verb at all.** A warm carrier already deposits on every ladder poll, and a cold
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
walls is not investigable. `turn.j2` has no such discipline today (its only "do not invent" line is
about verbs, `turn.j2:196`), which is why M0 exists and why it is a **prompt** milestone with no
store behind it. The full rule is in `01_facts.md` under "The ignorance rule".

## What this is not

- **Not `notices.rs`.** A notice is an accusation with a legal ladder (Word → Summoned → Warranted)
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

| Seam | Where | Used for |
|---|---|---|
| `World::emit` | `world.rs:510` | Minting facts from a whitelist of event kinds |
| `DomainEvent` | `event.rs:25` | Already carries `sequence`, `kind`, `actor_id`, `position_m`, `recipient_ids` — everything a mint needs |
| `notices::carries` | `notices.rs:419` | The deterministic hash-roll idiom (never a fresh draw), copied for pickup and carrier selection |
| **The round's ladder poll** | `round.rs:7400` `run_ladder`, gated per person by `next_decision` (jittered `LADDER_DECISION_MIN_SECONDS..=MAX`, 1–6 s) | **Where pickup, deposit and cooling run.** Already per-actor throttled and already staggered by `leg_lag_share`, so hops arrive scattered instead of frame-synchronised |
| `attention::on_stage` | `attention.rs:242` | The scan that already exists around the player — where the *visible* mouth-to-mouth hop rides, and nowhere else |
| `crowd::nearest_ward` (private today) | `crowd.rs:370`, over `homes::ward_marks()` | Position → `PlanningWard`. The eight wards tile the city; `areas.json`'s 71 named areas do not |
| `AreaMap::containing_area` / `location_description` | `areas.rs:290`, `areas.rs:330` | The *named* place a fact says it happened at |
| `Character::notify_percept` | `character.rs:755` | The existing sim→inbox path |
| `word_in_the_ward` block | `prompt/mod.rs:1555`, `strings.toml:43` | The block `what_you_know` sits beside, and the shape to copy |
| `notices::LAW_OCCUPATIONS` (`notices.rs:71`), `attention::RESERVED_TRADES`, `round::TRADE_OCCUPATIONS` (`round.rs:69`) | Three existing hand-named occupation sets | The precedent the salience **affinity** table follows — a fourth of a kind, not a new pattern |
| `homes.json` / `Townsperson.home` | The door a person sleeps behind | Subject-side damping: a fact is quietest among the people who share the subject's door |

**Correction to the earlier draft:** it named "the scheduler's per-poll distance pass" as the
propagation seam. `scheduler.rs` has no distance pass — it is the LLM turn scheduler (lanes,
budgets, turn order). The per-actor proximity work is in `run_ladder` and in `attention.rs`. This
matters: it is the difference between a design that costs one throttled O(1) lookup per person and
one that costs a linear scan of the whole city per carrier per tick. See risk 3.

## `arm_actor`: narrowed to the goal

`state.memories` and `state.goal` are **seed-only plus LLM-editable**: seeded from `sheet.memories` /
`sheet.goal` at world creation (`character.rs:527`), and after that written only by
`remember`/`forget`/`set_goal` (`actions.rs:2325`, `2603`). The sim has no setter, and quests need
one for a character who must privately know what they themselves *did* off-screen.

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
`recent_history` is seated first, regardless of heat; heat fills what is left.

**Phrasing is by hop count**, all of it in `assets/prompts/strings.toml`, none of it in Rust:

| hops | Renders as |
|---|---|
| 0, with an `own` line | first person — "I promised her bolt would go on Hugh's cart" |
| 0, without | "you saw" / "you were there when" |
| 1 | "they say" |
| 2+ | "you had it third-hand" |
| cold (any hops) | "you heard something of the sort, a while back" |

**Salience shortens that ladder** (`02_rumor_pollen.md`, "Hedge erosion"). Real gossip loses its
hedges as it travels: at four hops from a stall quarrel a person says "I had it from someone who had
it from Ilse", and at four hops from a scandal they say it flat, as a thing that happened. So the
table gains a band column rather than a mechanism — a top-band fact at hops 4 still renders as "they
say", a bottom-band one at hops 2 is already "third-hand". No new state, and it is the cheapest
answer there is to risk 2: the player *hears* the difference between two kinds of news without a
system being explained to them.

The block's instruction paragraph borrows the notice block's discipline, which exists for exactly
this reason — *treat it as something you know, not as something you must announce* — and adds the
ignorance rule, which is new and is the harder half.

## Milestones

Built end-to-end. M0 is a throwaway spike and it is the go/no-go.

| | Milestone | Contents |
|---|---|---|
| **M0** | **The mouth test** (spike, thrown away) | No store, no propagation. Hand-write the `what_you_know` block, its instruction paragraph and the ignorance rule; hand-author ~20 sheets — holders and non-holders, asked and unasked, alone and in a ward — and fire them at a **live provider** with `cathedral-headless --one-shot FILE`. Four questions: does a holder volunteer it when it is relevant? does a holder answer when asked straight? does a **non-holder refuse to invent, and name someone instead**? do eight holders in one ward produce eight different sentences? And, fifth: **given an occasion, does a model reach for `raise_word` — and given none, does it stay quiet?** Iterate on the *prose*, which is the only unknown in this feature, then throw the harness away and keep the strings. |
| **M1** | The fact store and the block | `knowledge.rs`: `Fact`, `FactId`, `Held`, `FactView`, `holds()` with `seeded` holders only. JSON loading (`assets/world/facts.json`, plus per-quest packs). The block for real, with M0's strings, relevance selection and the bounded render. Golden prompts re-blessed **once, here**, before any content exists. Headless: two characters hold an authored fact and discuss it; a third is asked and says who to ask instead. |
| **M2** | Minting, the air, and the **band** | Mint at `World::emit` for two kinds (the custody commit, the knell). The ward air (`02_rumor_pollen.md`): deposit, pickup and cooling on the ladder poll; `heat × salience` gates volunteering, holding persists. **Salience base bands** in the pickup roll and `assets/world/salience.json` — they belong here, not later, because they change the roll the cadence is measured on. Carriers-per-ward per game hour printed headlessly **per topic**, the **cadence band** measured at both ends, and the flat-table identity asserted against the pre-salience run. |
| **M3** | Garbling, provenance, the chain | Deterministic garble seeded per `(fact sequence, carrier id, hops)`, bounded to the fixed vocabulary — subject → another named actor of the same ward or trade, place → adjacent area, day ±1 — never inventing a person (`no-procedural-characters` holds). Hop-keyed hedging, **eroded by salience** (a scandal loses its hedges, a stall quarrel keeps them) — one column on a table that is being written here anyway. The **merge rule** (`01_facts.md`): fewer hops wins, and hearing it closer to the source corrects the view. **"Who told you that?"** — the `from` chain, answerable by an NPC and walkable by the player. Stage-local mouth-to-mouth hops on `attention.rs`'s existing scan, so the wave is visibly a wave where the player can see it. |
| **M4** | The player's side | The player as a carrier; `player_learned: BTreeMap<FactId, LearnedHow>`; the **journal overlay** (J) on the inventory overlay's pattern; the standing HUD line while a clock is live. And the player as a **source**: `raise_word` (`01_facts.md`), so a hearer of player speech can coin a claim out of what you told them — including things that are not true — with the player at the head of a chain that can be walked back to them. This is the LLM mint path, its **occasion gate** (the verb is offered only when somebody asserted something you do not hold, or a percept minted nothing), its caps, and its guardrails: `FactSource::Claimed`, speaker-only `seeded`, forced `decays`, topic from the closed list with `Talk` as the fallback. Plus the sim-side **relevance re-heat**, which is how a cold fact comes back round without a verb. |
| **M5** | Consequence, legibility and tuning | The full event whitelist; the per-sheet budget; **systemic readings** of held facts (a stallholder refuses credit, a household does not open, the hearsay rung in `notices.rs` raises a wrongful summons off a garbled subject); **bells re-heat** matching pollen within earshot; the wave made visible (ward heat on the map, "four mouths, two wards" in the journal). **Salience affinity** (topic × the listener's trade, including the no-trade quarter's flat ×1.4 — which is where "the poor carry it furthest" now lives) and **subject-side damping** (the subject's household hears it last). Tune against M2's cadence band. |

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
nothing already agreed gets re-tuned. At the shipped `seconds_per_day: 3600`, an office is 2–5 game
hours — 5 to 12 real minutes. Faster than that and the player can never get anywhere ahead of the
word, which kills every "beat your own story to the ward" fantasy in this design. Much slower and
nothing perceptibly moves inside a play session. The slow end is a computation, not an observation:
"may never leave" means the pickup roll's expected ward crossings over a game day is below one.

The headless carriers-per-ward-per-game-hour print, broken out per topic, is the measurement; it is a
test, not an eyeball. The flat-table run — every band `1.0`, reproducing the pre-salience numbers
exactly — is the regression guard on the whole of it.

## Risks

1. **Confabulation — the load-bearing one.** The failure that kills this feature is not a ward of
   parrots; it is a ward of confident inventors, in which holding a fact and not holding one look
   identical from the player's chair. Nothing in the current prompt discourages it. Mitigations
   designed in: the ignorance rule with its directional "ask X" form, the block's *know, don't
   announce* paragraph, and **M0, which tests exactly this against a live provider before a line of
   store code is written.** If M0 cannot get reliable non-invention out of the prompt, the feature's
   shape changes (a `what_you_do_not_know` counter-block, a narrower verb, a refusal template) —
   and that is far cheaper to learn in M0 than in M5.
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
   is one O(1) lookup on a poll each person already pays, and the expensive mouth-to-mouth version
   runs only inside the scan `attention.rs` is doing anyway. Guard it with a headless measurement at
   `--extra-ambient 20000`, in M2, before M3 builds on it.
4. **Topic mis-tagging, and the reason the model never sees a number.** Route 3 (`raise_word`) lets
   an LLM create a fact, and the thing that could go wrong is not that it lies — lying is the feature
   — but that everything it coins arrives in a high band and the city becomes uniformly loud, which
   is the flat model this design replaces, reached by a longer road. Mitigations, all structural
   rather than hopeful: the model picks a **topic from a closed list of nine**, never a number; an
   unrecognised tag falls back to `Talk`, so the failure direction is *down*; the salience number
   stays in a designer-owned table; and everything coined carries `FactSource::Claimed`, so a
   runaway is identifiable, invalidatable and walkable back to the mouth it came from. Measure it the
   same way as everything else: the per-topic cadence print will show a claim-heavy run skewing, and
   that is a number, not an impression.
5. **Prompt budget.** A real sheet is already ~13.6 KB. Facts compete with `word_in_the_ward`,
   `the_ward_says`, `your_round`, `you_hold`, `marks_here` and `dogs_nearby` for it. The cap is not
   optional, and relevance selection is what makes a small cap survivable.
6. **Snapshot budget.** Facts are per-actor prompt state and must **not** enter `PublicSnapshot`,
   whose 160 KiB bound already has little headroom (`lore-items-wave`). The journal projects
   separately, like the quest casebook.
7. **Golden churn.** Adding a sheet block re-blesses every golden prompt. Do it once, in M1, before
   any content exists.
8. **Determinism.** Every roll is a hash of stable inputs, never a fresh draw — the engine polls at
   60 Hz and a re-drawn probability is a certainty within a frame (`attention.rs` learned this the
   hard way). Pickup, carrier selection, garbling and hop order all follow that rule, and every
   collection that reaches a prompt or a golden is stably ordered.
9. **Legibility.** A simulation the player cannot see is indistinguishable from no simulation. The
   journal's provenance line, the "four mouths, two wards" count and ward heat on the map are not
   polish; they are what turns the propagation model into something a player can *play against*.
   Same principle the law-standing HUD line already states: a brand with no visible door is a bug.

## What it unblocks

- **The three quest specs**, each of which stops owning a knowledge system, and each of which gets
  rewritten against this once it ships.
- **`02_rumor_pollen.md`'s own children**: the hearsay rung in `notices.rs`, the STRANGER token
  (reputation with no reputation system), Night Office settlement (pollen graduating into canon on
  prompts already paid for), bells as amplifiers, and **walk the chain** — which is no longer a
  child. It is M3, because garbling without a way to check is the game lying to the player, and
  players correctly read that as a bug.

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
