Status: SPEC ONLY — unimplemented (2026-08-30). Load-bearing for all three quest specs.

# Knowledge and rumour

*What a person knows, where they got it, and how wrong it has become on the way.*

One feature in two halves, shipped together:

| Half | Owns | Chapter |
|---|---|---|
| **Facts** | The proposition. Who holds it at first hand. How it renders on a sheet. What the player has learned. | `01_facts.md` |
| **Pollen** | How a fact travels between people who were not there: proximity hops, heat, decay, garbling, provenance hedging. | `02_rumor_pollen.md` |

## Files

| File | What it is |
|---|---|
| `README.md` | This: the shape, the seams, the milestones, the risks |
| `01_facts.md` | The `Fact` type, the `holds()` predicate, the prompt block, player receipts, the journal |
| `02_rumor_pollen.md` | The original Rumor Pollen pitch (2026, unchanged) — the transport half and its children |

`02_rumor_pollen.md` was `features/rumors.md` until 2026-08-30. It moved here rather than being
rewritten: its design is intact and its "Children" section is still the roadmap. What changed is its
**status** — it is no longer an independent nice-to-have. Three quest specs need a knowledge layer,
and this is it.

## Why one feature and not two

`Fact` and `Pollen` were separable on paper, and an earlier draft of this proposed shipping facts
first with a deterministic gossip roll standing in for propagation, upgrading to real hops later.
That was rejected deliberately: it buys a smaller first milestone at the price of an interim
system that has to be reasoned about, tested and then taken out again. The two halves are shipped
as one.

The consequence is honest and should be stated where the schedule is decided: **this feature is now
on the critical path for `quest_the_bale_that_gained_forty_pounds`,
`quest_ring_a_dead_womans_name_at_marenstide` and
`quest_secure_votes_for_a_drainage_funding_plan_before_the_rain`.** None of them should start before
it lands. In exchange, each of them gets materially smaller — the bale quest alone drops a bespoke
knowledge enum, its own prompt block, its own casebook and its own player-receipt store.

## The one idea

**A fact is the noun. Pollen is the transport.**

A proposition has one id and one identity wherever it goes. *How you came by it* is separate from
*what it is*:

| Holding | hops | Garbles | Decays | Where it comes from |
|---|---|---|---|---|
| You did it, saw it, or were authored to know | 0 | no | no | the fact's `seeded` set, or an event's `recipient_ids` at mint |
| You were told, *n* hops out | *n* | yes | yes | a pollen token in the store |

That split is what lets one system carry both *"I promised her bolt a place on that cart"* (authored,
sealed to three people, never drifts) and *"they say Ede was taken at the Wickmarket, two days past"*
(minted from an arrest, four hops out, the day already wrong).

```rust
/// None means they have never heard of it at all.
pub fn holds(world: &World, actor: &ActorId, fact: &FactId) -> Option<Held>;
```

`holds` checks the fact's `seeded` set first, then the pollen store. Everything else — the prompt
block, the journal, the quests — goes through that one call.

## What this is not

- **Not `notices.rs`.** A notice is an accusation with a legal ladder (Word → Summoned → Warranted)
  and a settlement path. A fact is a proposition. They share an *idiom* — `notices::carries` is the
  proven pattern this borrows, and `word_in_the_ward` is the proven prompt block this sits beside —
  but they are not the same type and must not be merged. A reviewer will ask; that is the answer.
- **Not memory.** `stored_memories` is durable, LLM-owned and LLM-erasable — the turn prompt actively
  instructs every model to `forget` what is stale, and `actions.rs:2603` is the only production
  writer. Nothing quest-critical may live there. A fact is re-derived into the sheet every turn and
  cannot be forgotten.
- **Not a clue log.** The player's side records what they *heard or caused*, in the sentence they
  heard, attributed. It never lists objectives.

## Seams (all verified present)

| Seam | Where | Used for |
|---|---|---|
| `World::emit` | `world.rs:510` | Minting pollen from a whitelist of event kinds |
| `DomainEvent` | `event.rs:24` | Already carries `sequence`, `kind`, `actor_id`, `position_m`, `recipient_ids` — everything a mint needs |
| `notices::carries` | `notices.rs:419` | The deterministic hash-roll idiom (never a fresh draw), copied for carrier selection |
| `Character::notify_percept` | `character.rs:755` | The existing sim→inbox path |
| The scheduler's per-poll distance pass | `scheduler.rs` | Where hops and decay run, in pure Rust |
| `word_in_the_ward` block | `turn.j2`, `prompt/` | The prompt block `what_you_know` sits beside |

## The two fields nothing can currently write

`state.memories` and `state.goal` are **seed-only plus LLM-editable**: seeded from
`sheet.memories` / `sheet.goal` at world creation (`character.rs:527`), and after that written only
by the `remember`/`forget`/`set_goal` verbs (`actions.rs:2325`, `2603`). The sim has no setter.

Quests need one, for the narrow case of a character who must privately know what they themselves
*did* off-screen, where no object and no projection can carry it. This feature adds it:

```rust
/// Seed a character's durable state the way world creation does. A **seed**,
/// not an override: the actor's own set_goal/forget must win afterwards, or
/// they stop being a character.
pub fn arm_actor(&mut self, id: &ActorId, goal: Option<String>, memories: Vec<String>);
```

Expect this to be used for two or three characters per quest, not twenty. If a fact can be a fact,
it should be one.

## Prompt surface

One new block, `what_you_know`, rendered beside `word_in_the_ward`, bounded like it
(`NOTICES_SHEET_MAX` is 4; facts get their own cap). Phrasing is chosen by hop count:

| hops | Renders as |
|---|---|
| 0, with an `own` line | first person — "I promised her bolt would go on Hugh's cart" |
| 0, without | "you saw" / "you were there when" |
| 1 | "they say" |
| 2+ | "you had it third-hand" |

All hedging vocabulary lives in `assets/prompts/strings.toml`. Nothing about the format goes in Rust.

The block's instruction paragraph borrows the notice block's discipline, which exists for exactly
this reason: *treat it as something you know, not as something you must announce.* Without that
line, every mouth in a ward parrots the same sentence — which is this feature's principal risk, below.

## Milestones

Shipped as one feature; the quests start after M4.

| | Milestone | Contents |
|---|---|---|
| **M0** | The fact store | `knowledge.rs`: `Fact`, `FactId`, `Held`, `holds()` with `seeded` holders only. JSON loading. The `what_you_know` block and its strings. Bounded render. Headless: two characters hold an authored fact and discuss it; nobody else has heard of it. |
| **M1** | Pollen | `rumor.rs`: deposits at `World::emit` for two event kinds (the custody commit, the knell), proximity pickup and hops in the scheduler's existing distance pass, heat decay per game hour and per hop, cold tokens dropped. Carriers-per-ward per game hour printed headlessly. **Propagation lives entirely in the pure-Rust poll pass** — see the risk below. |
| **M2** | Garbling and provenance | Deterministic garble seeded per `(fact sequence, carrier id)` at each hop, bounded to the fixed vocabulary (subject → another named actor of the same ward or trade; place → adjacent area; day ±1). Hop-keyed hedging. Never invents a person (`no-procedural-characters` holds). |
| **M3** | The player's side | The player as a carrier; `player_learned: BTreeMap<FactId, LearnedHow>`; the **journal overlay** (J), built on the inventory overlay's pattern. Receipts carry `from` and `hops`, which is also the "who told you that?" chain. |
| **M4** | The whitelist, budgets and tuning | The full event whitelist; per-sheet fact budget; the perceptibility pass — how often an injected line actually gets voiced, and how much variety there is across a ward. This is the milestone that decides whether the feature works. |

## Risks

1. **Perceptibility — the load-bearing one, carried over from `02_rumor_pollen.md` and now on the
   critical path.** Once spread is code-driven, the LLM must voice the injected line often enough
   for the player to feel the wave, *without every mouth in a ward parroting the identical
   sentence.* This is a tuning problem, which means an open-ended one, and three quests are now
   behind it. Mitigations designed in: the `own`/`said` split gives holders different words by
   construction; garbling diverges the sentences further; the block's instruction paragraph says
   *know, don't announce*; and the per-sheet budget caps how much competes for a turn. **M4 exists
   to prove this and should be treated as the feature's go/no-go, not as polish.**
2. **Prompt budget.** Facts compete with `word_in_the_ward`, `the_ward_says`, `your_round`,
   `you_hold`, `marks` and `dogs_nearby` for the same sheet. A cap is not optional.
3. **Snapshot budget.** Facts are per-actor prompt state and must **not** enter `PublicSnapshot`,
   whose 160 KiB bound already has little headroom (`lore-items-wave`). The journal projects
   separately, like the quest casebook.
4. **Golden churn.** Adding a sheet block re-blesses every golden prompt. Do it once, in M0, before
   any quest content exists.
5. **Determinism.** Every roll is a hash of stable inputs, never a fresh draw — the engine polls at
   60 Hz and a re-drawn probability is a certainty within a frame (`attention.rs` learned this the
   hard way). Carrier selection, garbling and hop order all follow that rule, and all collections
   that reach a prompt or a golden are stably ordered.

## What it unblocks

- **The three quest specs**, each of which stops owning a knowledge system.
- **`02_rumor_pollen.md`'s own children**: the hearsay rung in `notices.rs` (a wrongful summons
  raised off a garbled subject, which the player can watch happen and then settle), the STRANGER
  token (reputation with no reputation system), Night Office settlement (pollen graduating into
  canon on prompts already paid for), bells as rumour amplifiers, and **walk the chain** — tracing a
  slander back to its garble point. That last one is investigation gameplay falling out of a
  testability decision, and it is the mechanism the bale quest's whole second act is made of.
