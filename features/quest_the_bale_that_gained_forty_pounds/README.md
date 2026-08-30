Status: SPEC ONLY — unimplemented (2026-08-30)

# Quest: the bale that gained forty pounds

Working title: **Forty Pounds Over**

Promoted from `features/systemic_quest_suggestions.md` §02. That entry was an idea; this is the spec.
Companion files:

| File | What it is |
|---|---|
| `PLAYTHROUGHS.md` | Ten worked routes from the gate stop to the aftermath, with storyboards — what goes wrong on each and whether the player can come back from it |
| `storyboards/` | Ten four-panel boards, one per route |
| `forty_pounds_over_implementation.docx` / `.pdf` | Four-page implementation summary: only the part that has to be built |
| `generate_storyboards.py` | Regenerates the boards (`gpt-image-2`, `--only NN`, `--force`) |
| `generate_implementation_summary.py` | Regenerates the docx (`python-docx`) |

---

## One-sentence pitch

A corded bale weighs forty pounds more at the Wool Gate than it did on the Tallage beam two hours
earlier, an innocent road trader is turned around in front of a crowd for it, and the player has
until the next Dayspring's public opening to decide what the truth is worth — and to whom.

## Player-facing promise

You meet a blocked cart, frightened oxen, and two porters pulling the same rope in opposite
directions. Nobody hands you a clue list. Forty pounds plainly came from somewhere, and the only way
to find out where is to walk the cart backwards through the day it just had: the Draper's Reach where
the cloth was counted, the beam where it was called aloud, the bonded warehouse it passed, the gate
that stopped it. Everything you need is an object somebody is holding, a place somebody has a key to,
or a sentence somebody will say for a penny or not at all.

You can clear Hugh Crake lawfully, quietly, publicly, or not at all. You can clear him and ruin two
other people doing it. You can clear him by making the forty pounds cease to exist, which is itself a
crime and leaves you carrying it. You can decide his innocence is worth less to you than what the
truth is worth to the man who would pay for it.

The city does not tell you which of those you did. It just weighs differently afterwards.

## Why this belongs in this game

**The discrepancy is arithmetic, not a flag.** This is the whole design. The bale's weight is the sum
of what is physically inside it; the manifest is a separate written claim; the seal records what the
beam called at sealing. Nothing anywhere stores "this bale is suspicious". Remove the extra bolt and
the bale really does weigh what it says. Add something else to it and you have made it worse. Swap in
a lighter grade and the numbers land somewhere nobody authored. Every route through the quest is the
player editing one side of a subtraction the city performs in public at Dayspring.

That is what stops this being a dialogue tree with a stage. It also means the quest cannot be
completed by talking to the right person, which is the failure mode the idea bank's selection rules
exist to prevent.

**The cast already contains it.** Not one character has to be invented, and barely a line has to be
written that the sheets do not already imply. The weigher is going blind and has his daughter read
the beam back to him aloud in front of the whole square. The carter who moved the bale knows every
weight at the Tallage by the note the beam makes as it settles, cannot read one word of the bill he
carries, and is owed money by the broker. The notary holds that an unwitnessed word is only weather,
and is at this moment lying awake about a seal he failed to look twice at two winters ago. The
weaver's own daughter walks the ropewalk and counts strands for a living. The broker has already
pledged his mother's warehouse keys to the money-dealer across the square. Nobody needed a quest
hook: these people were authored into a shape that has exactly one hole in it, and the hole is bale-
shaped.

**It leaves a tool behind.** Weighed, sealed, custody-tracked consignments are the instrument the
Tallage runs on. Building it here makes suggestions 13 (the fish cart), 14 (the locked grain store)
and 16 (the disputed standard weight) into content problems rather than engineering problems.

## Selection rules, answered

The idea bank promotes a suggestion only when it answers all six. Answers, in order:

1. **What forces movement?** The evidence is five places and seven people, none of whom will come to
   you, and all of whom are on their own schedules. The bale itself is locked in the bonded warehouse
   the moment it is impounded, and the opening happens at a fixed bell whether you are there or not.
2. **Which existing systems support three-plus approaches?** Rounds and schedules, speech and
   earshot, items and offers, the notice ladder (`notices.rs`), custody (`custody.rs`), marks
   (`marks.rs`), places and wayfinding, the road-party trip machinery, and the market's purchase
   path. The four authored lanes — lawful, social, covert, predatory — are each built from a
   different subset, and none of them is a quest-only verb.
3. **What can go wrong without a reload-shaped failure?** Everything in §"Failure matrix". The
   opening is not a fail state; it is the second act. Being seized is not a fail state; the Stone
   House has a door and a posted fee.
4. **Who can witness, misunderstand, exploit or publicize?** §"The evidence surface" names seven, and
   two of them (Warin Underbridge, Odo Trask) are specifically the people who catch a player's own
   forgery *later*.
5. **What changes in ordinary life?** §"Outcomes": who stands at the beam, whether the Brede cart
   runs, whether six loom households are paid, what the Draper's Reach pays by the ell, and whether
   there is now a real smuggling route into the city with a copied key on it.
6. **What new capability matters later?** Weighed sealed lots with a custody chain, and weight on the
   item catalog. Both are general.

---

## Canon boundary

### Two people are called Clemence, and the quest turns on both

This is the single most dangerous fact in the feature and it must be handled at the string level.

- **Clemence Hobbe** (`e5hob`), 46, weaver mistress, four looms in a jetty over the Needle, Cinder
  ward. Works wool put out by Ewart Skell. Has since autumn been buying raw fleece straight off Ansel
  of Brede — her own wool, her own profit, "and the ruin of you if Skell hears of it before you have
  work enough to live without him". Every parcel fixed by word of mouth, the girl Ede of the Needle
  carrying the message for a penny, **nothing written down, ever**.
- **Clemence Crake** (`fp6ck`), 52, wholesale merchant, the Crake counter on the Tallage for nineteen
  years, Weigh ward. Renn's mother. Hugh's mother's first cousin. Keeps the notary Odo Trask on
  retainer because "a woman who made her own name signs nothing without a witness". Has begun, without
  saying why, to look twice at her son's figures.

**Rule:** no prompt string, HUD line, notice `about` field, receipt or piece of quest prose ever
renders a bare "Clemence". It is always "Clemence Hobbe the weaver" or "Clemence Crake of the
Tallage" on first mention in any rendered block, and the two must never appear in the same sentence
without their bynames. A test asserts this over every authored string the feature adds.

The collision is not a bug to design around — it is the quest's best piece of misdirection, and an
officer, a gossip or an LLM confusing them mid-investigation is a legitimate and desirable outcome.
It just must never be the *author* who is confused.

### The road party already exists

`assets/world/rounds.json → road_parties[0]`:

```json
{ "id": "brede_wool_gate", "leader": "rbrde",
  "members": ["rbrde", "cbred", "dbred"],
  "gate": "The Wool Gate", "only_on": ["highmarket", "fourth"],
  "stage_at": "kindling", "enter_at": "dayspring", "return_at": "lamplight",
  "commercial_cargo": [{"kind":"grain"},{"kind":"wool"},
                       {"kind":"cloth","metadata":{"grade":"broadcloth"}}],
  "manifest": [{"kind":"grain","quantity":4},{"kind":"wool","quantity":4}],
  "legs": [ {"from":"dayspring","at":"Seven Lofts","doing":"trade"},
            {"from":"waning","at":"The Draper's Reach","doing":"trade"},
            {"from":"lamplight","at":"The Wool Gate","doing":"stand"} ] }
```

Hugh Crake, Colin Barley and Douce Fenn, the Wool Gate, broadcloth as declared return cargo, and a
manifest that is already a first-class object in `round.rs` (items are minted against
`road:{party}:{trip}:manifest:{slot}` at the boundary exchange). The quest hijacks this; it does not
add a cart.

**One authored deviation.** The shipped legs put the cart at the Wool Gate at *Lamplight*, which
would leave the player only the night to work in. On a quest day the party runs a shifted schedule —
Seven Lofts at Dayspring, the Draper's Reach and the Tallage beam before noon, the Wool Gate at
**High Wick** — because Hugh has moved his own departure up to catch the Brede road, which is
precisely the thing he then owes for missing. This is a `quest_legs` override on the party spec, off
when the quest is off, so the base city's schedule is byte-identical.

### The places already exist

Every location is a registered place with a baked nav pin: `pl_35z0` The Tallage, `pl_8so5` Tallage
toll-house, `pl_5vy5` Bonded warehouse, `pl_6gvy` Bonded weighing yard, `pl_kjqa` The Tally Bridge,
`pl_9ecu` The Wool Gate, `pl_u20m` The Draper's Reach, `pl_z2mq` Tenterhook Lane, `pl_kq1k` The
Needle, `pl_rxyx` Seven Lofts, `pl_t7rf` The Wickmarket, `pl_kcdh` The Stone House. No nav rebake.

### The lore already fixes the procedure

- Measures are proved against the **Tallage stone** and sealed brass **Gaudry weights**, kept chained
  in the customs square (`lore/core_lore/trade_and_daily_life.md`).
- Cloth is measured by the **ell of Ombreval**.
- Lowmarket (day six) is the Tallage's market day. Highmarket (day three) is the Wickmarket's.
- The Tally Bridge is a *dry* overhead connection between toll-house and bonded warehouse. There is
  no water under it, ever.
- **Bertran Hobbe** already hauls his own sealed weights to the Tallage every Lowmarket and has Ewart
  Tarn prove them against the standards with the whole square looking on. The quest's "neutral
  reweigh" is not a new procedure; it is a man's existing weekly habit, borrowed.

### What the quest may not do

- Not invent a character, not even a porter. (`no-procedural-characters`: the cast is fixed.)
- Not randomize the culprit. Renn Crake did it, in every playthrough, forever.
- Not make Hugh secretly guilty in some branch. The accusation against him is false. That is the
  premise, not a twist.
- Not resolve into a completed-quest entry. §"Outcomes" is the deliverable.
- Not put water in the Cut.

---

## The authored truth

Not a mystery to be randomized. This happened, in this order, and every piece of evidence in the
quest is a physical consequence of one of these lines.

| When | What |
|---|---|
| Autumn, ongoing | Clemence Hobbe buys raw fleece direct from Ansel of Brede, outside Ewart Skell's putting-out system. Nothing written. Ede of the Needle carries every arrangement for a penny. |
| Recent weeks | Clemence Hobbe weaves one bolt of broadcloth from that fleece. Her own wool, her own cloth, her own name on it — the first she has ever had to sell. |
| Spring | A Salorge salt contract goes bad on Renn Crake. Rather than go to his mother he pledges **her** warehouse keys to Dunstan Skell against the forfeit. Eleven weeks to mend it. |
| Days before | Renn promises Clemence Hobbe he will put her bolt on Hugh's returning cart. He is very good at the half of the trade that is talking. He has not yet worked out how. |
| Quest day, Dayspring | Hugh's party enters at the Wool Gate, sells grain at Seven Lofts. |
| Quest day, mid-morning | At the Draper's Reach, Ewart Skell hands over the bolts owed to the Brede principal. Rohese Skell measures them by the ell and enters the count. Douce Fenn cords the bale. |
| Quest day, ~two hours before noon | At the Tallage beam, Ewart Tarn weighs the bale aloud — the sack named, the weight called, the standard cited, the oath said — Averil Tarn reads the figure back for him, and the bale is sealed. Warin Underbridge is on the porter stand and hears the beam settle. |
| Quest day, between beam and gate | Renn finds the bale already sealed. He has one promise, eleven weeks, and his mother's warehouse keys in another man's strongbox. He diverts the cart through the bonded warehouse, cuts the cord, puts Clemence Hobbe's bolt in, re-lays the cord and presses an imitation of Tarn's seal. Warin Underbridge carries the bale back out and says nothing, because Renn owes him for loads he keeps promising. |
| Quest day, High Wick | The Wool Gate beam says forty pounds over. Officers call it smuggling, turn the cart around before a gathering crowd, and impound the load into the bonded warehouse until a public opening at the next Dayspring. |

**Hugh did not authorise it and does not know.** The accusation is false. A crime did occur. Clearing
him exposes either a weaver's bid for independence or a broker whose family's keys are already in
debt, and most routes expose both.

The forty pounds is one bolt of broadcloth. `assets/world/items.json` already prices a broadcloth
bolt at **40 sparks**. It will weigh **40 lb**. The pun is free and the game should never point at it.

---

## Cast

Everyone below is a shipped, authored character. The "already on the sheet" column is what makes them
load-bearing — none of this is written for the quest.

| Id | Who | Role in the quest | Already on the sheet |
|---|---|---|---|
| `rbrde` | Hugh Crake of Brede | The accused. Spare, courteous, hard on quantities. | "more willing to forgive a late gate than a vague tally"; obligation is exact; every bundle answered for before he sleeps |
| `fr9ck` | Renn Crake | Did it. Charming, warm, generous with drink he cannot afford. | Pledged his mother's warehouse keys to Dunstan Skell; eleven weeks left; "never quite where you said you would be"; would sooner be ruined than watched |
| `e5hob` | Clemence Hobbe | The bolt is hers. Does not know how it travelled. | The off-book fleece; "the ruin of you if Skell hears of it"; nothing written down, ever |
| `e1skl` | Ewart Skell | The draper whose system she broke. Can legalise it — or retaliate. | Lays every bolt against the standard ell himself; "about a finger short of kind" |
| `fp6ck` | Clemence Crake | Renn's mother. Owns the warehouse. Already suspicious. | "look twice at your son Renn's figures"; keeps Odo Trask on retainer |
| `fb3sk` | Dunstan Skell | Holds the pledged keys. Fears one unpaid season more than the grave. | Wants the freight brokers lending through him, not around him |
| `fe2tn` | Ewart Tarn | Official weigher. Sealed the bale. **Going blind.** | Does the whole work aloud; "you can see a sack, you cannot see a figure"; orders Averil to read the beam back in front of the square |
| `fa8tn` | Averil Tarn | Toll clerk. Reads the beam for her father. | "how to get those two men into one room without the whole square hearing it" — her own want *is* the confrontation mechanic |
| `fw7ub` | Warin Underbridge | Carried the bale into the warehouse and back out. Heard it settle heavy. Said nothing. | Knows every weight at the Tallage by the note the beam makes; cannot read; loves Averil; wants Renn's word made good |
| `fo6gl` | Odo Trask | The one man who can place a seal — and who once did not look twice. | "an unwitnessed word is only weather"; the old seal "comes to you now at the Kindling bell" |
| `e7mil` | Bertran Hobbe | Owns proved weights and a weekly public proving. | Hauls his own sealed weights to the Tallage every Lowmarket |
| `he3nd` | Ede of the Needle | Carried the message. Eleven. | "never give away a word you were not paid for"; "goes quiet and still the moment anyone reaches for you" |
| `e6ptr` | Petronel Roper | Can read the cord. **Clemence Hobbe's daughter.** | Walks the ropewalk her father laid; "you count everything — strands, fathoms, strokes of a bell" |
| `e2rhs` | Rohese Skell | Measured and counted the bolts out at the Reach. | Cloth measurer at the Draper's Reach |
| `cbred` | Colin Barley | Counts the return cloth twice. | "Hugh's obligation becomes your back if a bolt goes missing" |
| `dbred` | Douce Fenn | Corded the bale. Would know her own cord. | "Every returning bolt of broadcloth is your cargo obligation until Hugh's principal cuts the cord" |
| `fg4br` | Gile of Brede | Hired writer at the Tally Bridge; writes and reads bills for the illiterate. Ansel's father. | Reads Warin's bills back to him for a spark |
| `ecbrd` | Ansel of Brede | Sold Clemence Hobbe the fleece. | "Clemence Hobbe the weaver buys from you fairly"; wants his name clean in Brede |

Six unpaid loom households: `e5hob`, `em3rl`, `et7rd`, `p002l`, `p002n`, `p002q` — weavers with
lawful bolts in the impounded bale who will not be paid while it is evidence.

---

## Scope boundary: quest versus base game

### This feature owns

- The `brede_wool_gate` quest-day leg override and the seizure at the gate.
- `BaleQuest` state, its phase machine, its clock, its receipts and its outcome packages.
- The authored contents of *this* bale, this seal, this cord and this manifest.
- The public opening at the bonded warehouse and its resolution function.
- The four outcome packages and their aftermath edits.
- The quest's authored **facts** (`assets/world/quests/bale.json`) — data only; the quest owns no
  knowledge type, no predicate, no prompt block and no casebook.
- The `the_lot` sheet block: the bale's *physical* state (where it is, whether impounded, what the
  paper claims) for characters with a live custody or ownership relationship to it. This is a
  projection of an object, like `you_hold` — not knowledge, and not a fact.

### This feature owns and gives to the base game

These are general, live on `World`, and are not quest-gated:

- **`weight_lb` on the item catalog** (`assets/world/items.json`), including per-metadata variants,
  exactly like `price_sparks`.
- **`lots.rs`** — sealed, weighed consignments with an append-only custody chain (below).
- **The public weighing** — a spoken, witnessed weigh-beam procedure at the Tallage.

### This feature consumes but does not own

- `notices.rs` — the Word → Summoned → Warranted ladder, `raise_notice` / `settle_notice`.
- `custody.rs` — seizure, the escort tether, the Stone House, the posted fee.
- `marks.rs` — chalk on doors and named places.
- `round.rs` — schedules, road parties, the boundary exchange, `try_purchase`.
- `places.rs` / nav — wayfinding and `go_to`.
- `speech_router.rs` and the 20 m hearing calculation — witnesses are *whoever was actually within
  earshot*, computed the same way as everything else.
- `attention.rs` — the stage. The quest never widens the LLM stage or raises the actor cap.
- **`features/knowledge_and_rumor/`** — `Fact`, `holds()`, the `what_you_know` block, pollen
  propagation and garbling, player receipts, the journal overlay and `World::arm_actor`. The quest
  authors JSON facts and consumes all of it. See *Knowledge, memory and entry* below.

### Foundation dependencies

| Dependency | State | Handling |
|---|---|---|
| Item weight | Does not exist | **Owned by this feature.** M0. |
| Sealed lots + custody chain | Does not exist | **Owned by this feature.** M0. |
| Keys and locked places | `features/keys_and_locked_places.md`, SPEC ONLY | **Scoped substitute.** M2 ships a single authored `key` item (kind already exists) that opens exactly one door — the bonded warehouse — with a hard-coded reach check. If the keys feature lands first, the substitute is deleted and the door registers with it. The quest must not block on it. |
| **Knowledge and rumour** | `features/knowledge_and_rumor/`, SPEC ONLY | **Hard prerequisite — this quest does not start until it lands (M4 there).** It owns `Fact`/`holds()`, the `what_you_know` prompt block, pollen, garbling, player receipts, the journal and `World::arm_actor`. Decided 2026-08-30 to ship both halves as one feature rather than an interim fact-only layer. |
| Quest scaffolding | Does not exist; two other specs each propose their own | **Shared.** M0 lands `quest.rs` as a small common host (id, phase, data gate, outcome application) that `KnellQuest` and `DrainQuest` can adopt. Receipts and the casebook are *not* here — they belong to the knowledge layer, which all three quests share. |
| Player purchase/payment | Only `round::try_purchase`; no general credit path | Duty payment and surety go through a narrow quest-owned settlement that calls the same spark-moving code. Noted in `crowd-knob`/`chalking` findings as a recurring gap; not fixed here. |
| Persistence | No save | The quest is a single-session arc by design (~1.5 game days). Outcomes apply to the live world and are lost on quit, as everything else is. |

### Explicit non-goals

- No courtroom. The bench, the surety hearing and the inquest are speech among authored people in a
  place at a time, not a UI.
- No detective mode, no highlighted clues, no evidence inventory screen. The journal is the
  knowledge layer's, it records only what the player heard or caused, and it renders open threads as
  questions rather than objectives (`knowledge_and_rumor/01_facts.md`).
- No quest-owned knowledge type. An earlier draft of this spec invented a `BaleKnowledge` enum and a
  `BaleFact` list; both are deleted. Three quests inventing three of those was the problem the
  knowledge layer exists to solve.
- No new LLM verb for lying. Deception is what a character says; the sim does not model intent.
- No global smuggling economy. A cover-up creates *one* route, with *one* copied key, and names it.
- No skill checks, no rolls. Every gate is a physical fact, a schedule, an item, or a person's
  authored disposition.

---

## The new primitive: weighed, sealed lots

### 1. Weight on the item catalog

`assets/world/items.json` kinds gain an optional `weight_lb`, keyed exactly like `price_sparks` so
metadata variants can differ:

```json
{ "kind": "cloth", "display": "bolt of cloth",
  "metadata": {"grade": ["kersey", "broadcloth"]},
  "price_sparks": {"grade=kersey": 14, "grade=broadcloth": 40},
  "weight_lb":    {"grade=kersey": 24, "grade=broadcloth": 40} }
```

Seed values: broadcloth bolt 40, kersey bolt 24, raw wool bundle 28 (a tod), grain sack 56, salt cask
84. Everything else may omit it and weigh nothing; an absent weight is zero, and only lots and beams
ever ask. `ItemCatalog::weight_lb(&Item) -> u32`, multiplied by stack quantity.

This is deliberately a *general* catalog field and not quest data. It is the thing suggestions 13, 14
and 16 all silently assume.

### 2. `crates/cathedral-sim/src/lots.rs`

```rust
/// A consignment closed under a seal at a weigh-beam: what is actually inside,
/// what the paper claims, what the beam called when it was closed, and every
/// hand it has passed through since. The city's ordinary freight instrument.
pub struct Lot {
    pub id: LotId,
    /// Prose, for percepts and the sheet: "a corded bale of broadcloth".
    pub label: String,
    /// Whose obligation it is. Not who is carrying it.
    pub owner: ActorId,
    /// The real contents. Item ids, so the bolts are ordinary items that can be
    /// taken out, sold, offered and eaten by moths.
    pub contents: BTreeSet<ItemId>,
    /// What the written manifest claims is inside. The crime lives in the gap.
    pub declared: Vec<StockSpec>,
    /// What the beam called when the seal went on.
    pub sealed_weight_lb: u32,
    pub seal: Option<SealImpression>,
    pub cord: CordState,
    /// Append-only. The investigation surface.
    pub custody: Vec<CustodyLeg>,
    pub site: LotSite,
    pub impound: Option<Impound>,
}

pub struct SealImpression {
    /// Whose seal it purports to be. Rendered.
    pub purports: ActorId,
    /// Who actually pressed it. **Never rendered anywhere.** Only
    /// `Lot::examined_by` may consult it, and only for an actor with the
    /// authored competence to place a seal.
    pub pressed_by: ActorId,
    pub authenticity: Authenticity, // Genuine | Imitated
}

pub enum CordState {
    Original { laid_by: ActorId },
    Cut,
    Relaid { by: ActorId },
}

pub struct CustodyLeg {
    pub holder: ActorId,
    pub from: GameDays,
    pub to: Option<GameDays>,
    pub place: Option<PlaceId>,
    /// False when the leg is not in the written copy — a diverted hour.
    pub recorded: bool,
}

pub enum LotSite { Carried(ActorId), AtPlace(PlaceId), OnBeam(BeamId), Impounded(PlaceId) }
```

**The load-bearing method:**

```rust
impl Lot {
    /// Sum of the contents' catalog weight. Never stored, always computed.
    pub fn true_weight_lb(&self, world: &World) -> u32 { … }

    /// What the opening will find. Positive means heavier than sealed.
    pub fn discrepancy_lb(&self, world: &World) -> i64 {
        self.true_weight_lb(world) as i64 - self.sealed_weight_lb as i64
    }
}
```

There is no `is_tampered` boolean and there must never be one. Forty pounds is a subtraction the
world performs, which is why removing the bolt genuinely fixes it and why a player who improvises
something the designer did not anticipate gets a coherent answer instead of a scripted one.

### 3. The public weighing

Ewart Tarn's procedure, from his own sheet, as a sim event:

```rust
pub struct Weighing {
    pub lot: LotId,
    pub beam: BeamId,
    pub weigher: ActorId,        // fe2tn
    /// Who read the figure aloud. Tarn cannot see a figure; someone always does.
    pub read_by: ActorId,        // fa8tn by default
    /// The standard cited: the chained Gaudry weights, or another set.
    pub standard: StandardId,
    /// The figure as spoken. Not necessarily the true weight.
    pub called_lb: u32,
    pub at: GameDays,
    /// Everyone within the ordinary 20 m hearing radius when it was called.
    pub heard_by: Vec<ActorId>,
}
```

The called figure is a function of `(true weight, standard, reader)`. With the chained Gaudry weights
and Averil reading, `called_lb == true_weight_lb`. With Bertran Hobbe's proved weights it is also
true, and *provably* so, because his proving is public and weekly. With a reader who has a reason to
misread it, it is not. Tarn's blindness is not a puzzle to solve; it is a documented, authored
vulnerability in a public institution, and the player may use it, close it, or expose it.

`heard_by` reuses the existing single authoritative recipient calculation — witnesses are literally
whoever was standing there.

### 4. Reading a seal, a cord and a beam

Three examinations, each gated on authored competence, each producing a *statement by a person* and
never a fact in a UI:

| Examination | Who can | What they can say |
|---|---|---|
| Place a seal | `fo6gl` Odo Trask (and, grudgingly, `fe2tn`) | Whether the impression is Tarn's hand or an imitation. Trask's own two-winters-ago failure is why he now looks twice. |
| Read a cord | `e6ptr` Petronel Roper, `dbred` Douce Fenn | Whether it is the cord that was laid, or a cord re-laid by another hand. Douce laid it. Petronel counts strands for a living — and is Clemence Hobbe's daughter. |
| Know a beam's note | `fw7ub` Warin Underbridge | What the bale weighed when he carried it, before any beam said so. He already knows. He has to be made to say it. |

These are dispositions in the prompt, not verbs. The player asks; the character answers or does not,
according to who is standing there and what they owe.

---

## The evidence surface

Seven physical traces. Every one is an object in a place or a sentence in a person, and none of them
is a "clue".

1. **The cut cord.** Douce Fenn corded the bale at the Reach; it is now re-laid by another hand.
   Douce would know her own work. Petronel Roper can read any cord. The bale is in the bonded
   warehouse, so reading the cord means getting to it, or getting to the opening early.
2. **The copied wax.** Renn imitated Tarn's seal. Odo Trask can place it. Trask is the man who two
   winters back witnessed a seal he could not place and chose not to look twice, and it "comes to you
   now at the Kindling bell". He will look twice at this one.
3. **The beam's note.** Warin Underbridge heard the bale settle heavy on the way *out* of the
   warehouse and said nothing, because Renn owes him for loads. He is the single fastest route to the
   truth and the single easiest person to frighten off it: he is cheerful, easily flattered, and the
   only thing he truly fears is being taken for a fool.
4. **The bolt count at the Reach.** Ewart Skell lays every bolt against the standard ell himself;
   Rohese Skell measured them out. Skell's stock minus what he entered is a number, and it will not
   have moved. The extra bolt is not his — which is exactly the fact that tells him somebody is
   weaving off his wool.
5. **The spoken figure.** Tarn called the weight aloud with the square listening; Averil read it back.
   Anyone within 20 m at the time is a witness to what the bale weighed two hours before the gate.
   `Weighing::heard_by` is a real list of real people who were really there.
6. **Ede's penny.** Ede of the Needle carried every word between Clemence Hobbe and Renn Crake. She
   is eleven, she knows every lane and which grown men are slow, she never gives away a word she was
   not paid for — and she goes quiet and still the moment anyone reaches for her. A penny opens her.
   A raised voice closes her for the rest of the quest.
7. **The warehouse hour.** The cart's custody chain has a leg that is not in the written copy. Gile of
   Brede writes and reads the bills at the Tally Bridge and can tell the player what the copy says;
   the gap between the copy and where the cart actually was is the crime's shape.

**Nothing glows.** The casebook records a lead only once the player has heard it said or seen it, and
records it as the sentence they heard.

---

## Knowledge, memory and entry

Everything here rides `features/knowledge_and_rumor/`. The quest authors facts as JSON and owns no
knowledge code.

### Why the quest does not use memories

`stored_memories` is durable but **LLM-owned and LLM-erasable**: `turn.j2` instructs every model to
re-read it each turn and `forget` whatever is stale, and `actions.rs:2603` is the only production
writer. Anything quest-critical parked there is on a timer. `recent_history` is 32 entries and
evaporates. (`Character::remember_percept` writes to `recent_history`, not memories, despite the
name.)

So quest knowledge is a **fact**, re-derived into the sheet every turn by `holds()`, exactly as
`word_in_the_ward` re-derives notices. It cannot be forgotten, cannot drift, and is gated by a
predicate rather than by a model's cooperation.

### What people already know before the quest exists

Two different problems, and only one needs building.

**The standing situation is free.** `core_character_description` is in every prompt, always. On day
one of a fresh game Clemence Hobbe will already tell you about the off-book fleece if you press her;
Renn is already evasive about money; Tarn is already going blind; Trask is already lying awake about
a seal he did not look twice at. A player who talks to any of them long before the quest fires gets a
coherent person with a coherent problem. Nothing to author — this is what made the suggestion worth
promoting.

**The episodic setup needs arming.** "Renn promised me on Fourth" is *dated*, so it cannot be a seed
memory when the quest may fire on day 3 or day 40.

### `BalePhase::Arming`

One game day before the seizure, and the phase machine gains a rung:

```text
Dormant -> Arming -> Seized -> Investigating -> OpeningDue -> Opened -> Resolved
```

Arming is **played, not narrated**. It puts things in the world rather than in anyone's head:

| Armed | How |
|---|---|
| Clemence Hobbe's bolt | Minted as a real `cloth`/`grade=broadcloth` item in her workshop |
| `bale.promise` | An authored fact, `seeded` to Renn, Clemence Hobbe and Ede, `decays: false`, `garble: none` |
| Renn's intent | `World::arm_actor` sets his goal: *get her bolt onto Hugh's cart before the Tallage seals it, and do not let my mother hear of it* |
| Renn's day | The extra Tally Bridge leg on his round |
| Ede's errand | A real Needle→bridge run, walked, for a real penny |

A player in the Wickmarket on arming day can watch Ede run. Nobody needs to remember it, because it
happened. And `current_goal` is the strongest lever in the prompt — durable, rendered every turn, and
the model is already told to keep it current. A Renn carrying that goal behaves correctly with no
knowledge injection at all.

`arm_actor` is a **seed, not an override**: once armed, the actor's own `set_goal` and `forget` win.
Expect it for two characters here — Renn's goal, and the two private recollections with no physical
proof (his diverted hour, Warin hearing the beam settle heavy). Everything else is a fact.

### How knowledge spreads through the quest

Entirely through the shared layer; no quest-specific gating.

| When | Who holds what |
|---|---|
| Arming | Three people hold `bale.promise` at hops 0. Nobody else has heard of it. |
| The gate stop | `bale.stop` is minted from the seizure event, `seeded` from its `recipient_ids` — whoever was actually within earshot — and spreads as ordinary pollen, place and day free to garble. |
| Lamplight | The notice against Hugh climbs to `Summoned` and travels on `notices.rs`'s own carrier roll, which is not this feature's problem. |
| The opening | Minted public, high heat, `seeded` from the crowd. |

The seven evidence traces are facts with sealed `seeded` sets and authored `own` phrasings, so Warin
holds *"she went heavy, and I said nothing"* while a gossip four hops out holds *"they say that bale
was wrong before it reached the gate"*. Whether Warin will *say* his is not a knowledge question —
it is a disposition question, and the answer is on his sheet.

**The anti-leak rule matters especially here.** Who pressed the wax is `Lot::seal.pressed_by` and the
`FactSource` behind the seal facts. Neither is ever rendered. A character who holds "the seal is an
imitation" knows the seal is wrong; they do not know whose hand it was, and no projection can leak it.

### How the quest starts: no offer scene

The two sibling quests are handed to the player by a named NPC with a writ. This one must not be.
Hugh Crake does not know he needs help and would not ask a stranger, and the whole pitch is *a
blocked cart, frightened oxen and porters pulling the same rope* rather than a merchant explaining a
fee rule.

The quest is **walked into**, through three channels that already exist:

1. **Sight** — the player is near the Wool Gate at High Wick on a quest day. The stop happens whether
   they are there or not.
2. **Earshot** — a turned cart, a crowd and frightened oxen are loud. Ordinary sound propagation
   carries it a couple of streets; a player who hears it can come and look.
3. **Word** — `bale.stop` spreads as pollen and the notice against Hugh enters the ward's tongues.
   Anyone the player talks to in the Weigh ward may raise it unprompted, already slightly wrong.

There is **no accept, no decline and no `AcceptQuest`**. What activates is the journal, on the first
fact the player learns. Availability predicates: it is a Highmarket, the party runs that day, the
quest is unresolved, and the player has been in the city long enough to have somewhere to start.

**The cost, stated plainly:** a player can miss the whole thing. That is playthrough 10, and it is
acceptable for *a* quest but would not be for a *first* one. The mitigation is channel 3 — the city
brings it up on its own, in a garbled form, which is also the most in-fiction possible nudge.

### The journal

Owned by the knowledge layer (`01_facts.md`), not by the quest. The quest supplies only its two
standing lines:

- **the clock** — "the bale opens at Dayspring — one bell away"
- **the stake** — "Hugh Crake is summoned to answer for it"

Both follow the rule `src/smart_actors/hud.rs:373` already states for the law-standing line: *it must
always name what would clear it — a brand with a visible door is a story, a brand with no door is a
bug.* While a clock is live it is a standing HUD line, not a toast; a deadline the player cannot see
is not a deadline.

---

## The clock

One and a half game days. The quest fires on a Highmarket (day three), which is one of the party's
own `only_on` days.

| Bell | What happens whether or not the player is there |
|---|---|
| **Day 1, High Wick** | The gate stop. Officers turn the cart, impound the bale into the bonded warehouse, and raise a ward notice against Hugh Crake. Quest becomes offerable. |
| Day 1, Waning | Hugh must choose: abandon the cargo and take the road, or stay with it and owe for the missed Brede departure. If the player has not spoken to him he stays, because his obligation is exact. |
| Day 1, Lamplight | Officers summon Hugh to answer at the opening — the notice climbs to `Rung::Summoned` with `office: Dayspring`. Ewart Skell hears at the Reach that a bolt he did not enter is in a bonded bale. Renn starts avoiding the Tally Bridge. |
| Day 1, Snuffing | Gates shut. The warehouse is locked. Whoever is going to run, runs tonight. |
| **Day 2, Dayspring** | **The public opening.** Cord cut before a crowd, contents counted against the declared manifest, weight called on the chained standards. Resolution function runs. |
| Day 2, onward | Aftermath. Notices settle or climb to warrant; custody, surety or flight; the outcome package applies. |

If the player never engages, the opening still happens, the extra bolt is found, Hugh is seized, and
the city gets on with it. That is a legitimate ending and it is playable through.

---

## Approaches

Four authored lanes. They are not exclusive and most real playthroughs braid two. Every step below is
an existing system.

### Lawful — make the paper true

Stage a neutral reweigh with Bertran Hobbe's proved weights (his own Lowmarket habit, borrowed a day
early), before Odo Trask as witness, and amend the manifest. Requires: getting Bertran and his weights
to the beam, getting Trask to attend (he cannot be bought; the fee book lies open and a widow pays
what a wool factor pays), getting the bale out of impound for a proving, and **paying the omitted
duty on forty pounds of unentered broadcloth** — which is an admission that it exists, and therefore
names its weaver unless the player has arranged otherwise.

Settles the notice against Hugh by amendment. Does not, by itself, say who put it there.

### Social — put the stories where they cannot all stand

Averil Tarn's own stated want is how to get two men into one room without the whole square hearing;
the quest's confrontation is the player doing exactly that with three. Clemence Hobbe, Renn and Hugh
in one place is not a cutscene: it is three scheduled people, a location, and the 20 m hearing radius.
Their accounts are mutually impossible and the LLM will find that out loud.

Softer variants: make Renn confess without naming Clemence Hobbe (he would sooner be ruined than
watched, so a private confession is *easier* to get than a public one). Persuade Ewart Skell to buy
the independent bolt, which makes it lawful stock retroactively and ends the crime — at the cost of
the weaver's independence, and she will know who sold it. Turn the six unpaid loom households into a
crowd at the warehouse door and let the officers decide whether evidence is worth six households'
wages.

### Covert — change what is in the bale

Get into the bonded warehouse before Dayspring, take the bolt out, re-lay the cord. The opening then
finds a bale that matches its manifest, because it *does*. Requires warehouse access (Renn's pledged
keys are in Dunstan Skell's strongbox; Clemence Crake has her own; Warin Underbridge walks in and out
of it all day), and leaves the player holding forty pounds of bulky broadcloth that belongs to a
weaver who cannot afford to claim it.

Variants: replace the false seal with a better forgery (works on officers, fails on Trask); alter the
custody copy at the Tally Bridge (Gile of Brede writes for hire and does not ask); route the cart out
through another gate entirely.

### Predatory — the truth is an asset

Sell Renn's guilt to Dunstan Skell, who holds the pledged keys and wants the freight brokers lending
through him rather than around him. Sell the weaver's independence to Ewart Skell, who has half the
weavers on the Cut and pays by his own reckoning. Take Renn's warehouse key as the price of silence
and keep the route. Or say nothing at all, let Hugh carry it to Brede, and collect Clemence Hobbe's
loyalty for the one thing you did not say.

---

## Deterministic quest state

Lives in `cathedral-sim`, on `World`, behind an absent/default-off data gate. Stable ordering
throughout; no `HashMap` order may reach a snapshot, prompt or golden.

```rust
struct BaleQuest {
    phase: BalePhase,
    lot: LotId,
    notice_against_hugh: Option<NoticeId>,
    opening_at: GameDays,
    brede_departure_at: GameDays,
    /// Per-character standing toward the player, authored deltas only.
    standing: BTreeMap<ActorId, i8>,
    /// Set when Ede has been frightened, Warin flattered, Trask engaged, etc.
    dispositions: BTreeMap<ActorId, Disposition>,
    outcome: Option<BaleOutcome>,
}

enum BalePhase { Dormant, Arming, Seized, Investigating, OpeningDue, Opened, Resolved }
```

Phase machine:

```text
Dormant -> Arming -> Seized -> Investigating -> OpeningDue -> Opened -> Resolved
                                   \                            ^
                                    \--- (player never engages) -/
```

**What is deliberately *not* here.** Receipts, learned leads and the player's casebook were in an
earlier draft of this struct and are gone: they belong to `knowledge_and_rumor`, because all three
quests need them and because "what the player has learned" is the same question in every one. What
remains is what is genuinely this quest's: a phase, a clock, a lot, a notice, per-character standing
and an outcome.

### Receipts

Append-only, stable ids, and the *only* thing outcomes may branch on. Never the authored truth.

- the lot's contents and declared manifest at the moment the cord was cut at the opening;
- a weighing occurred at beam B with standard S, read by R, calling N pounds, heard by [ids];
- actor A stated X to the player at time T, in place P, within earshot of [ids];
- a seal was examined by A and called genuine/imitated;
- a cord was examined by A and called original/re-laid;
- the player entered place P at time T, seen by [ids];
- duty of N sparks was paid on behalf of A, witnessed by W;
- a notice was raised/summoned/warranted/settled against A by B;
- an item moved from A to B at T within 4 m, seen by [ids].

The casebook projects only receipts the player caused or witnessed.

---

## The public opening

At Dayspring, at the bonded warehouse, before whoever is standing there. Pure function of lot state
at that instant — this is the whole quest's resolution and it reads no quest flags:

```rust
fn open_lot(world: &World, lot: &Lot, witnesses: &[ActorId]) -> OpeningResult {
    let found      = lot.contents_by_kind(world);      // what is actually in it
    let declared   = lot.declared_by_kind();           // what the paper says
    let weight     = lot.true_weight_lb(world);
    let discrepancy= weight as i64 - lot.sealed_weight_lb as i64;
    let seal       = lot.seal.as_ref().map(|s| s.authenticity);
    let cord       = lot.cord;
    …
}
```

Branches, in order of what the crowd sees:

| Found | Consequence |
|---|---|
| Contents match declared, weight matches seal | Hugh cleared publicly. The gate's word was wrong, and the officers wore it. Whoever made it match is now holding the difference. |
| Extra bolt present, seal imitated | Both facts are public. The bolt has no owner in the paper, and the question becomes whose hand pressed the wax — which is now a live matter for the bench, not a settled one. |
| Extra bolt present, seal genuine | Worse for Hugh: the bale was sealed with the bolt in it, which means the beam was wrong or the weigher was. Tarn's thirty unblemished years are suddenly the thing on trial. |
| Contents short of declared | Hugh is cleared of smuggling and owes his principal for missing cloth. Colin Barley counted it twice and will say so. |
| Amended manifest lodged with Trask before the bell | The opening is a formality. Duty paid, the bolt entered, no crime — and a named weaver with a named customer. |

The result is announced aloud, so `heard_by` is again the real hearing calculation, and the ward
notices that follow are ordinary `notices.rs` raises by whichever officer is present.

---

## Outcomes

Not endings. Edits to the running city, all of them visible without opening a menu.

**Lawful amendment.** Hugh's notice settles; the Brede cart runs next Highmarket as normal. The six
households are paid. Clemence Hobbe has a direct customer and a name of her own — and Ewart Skell
knows. Skell's retaliation is authored and immediate: he cuts what he pays by the ell at the Reach,
or he denies her wool, or he moves to absorb her workshop. Whichever, the Cut's weavers notice their
pay change and say so.

**Full exposure.** Hugh is cleared, and Renn loses his trade, his keys or his freedom. Freight
brokerage at the Tally Bridge changes hands — Segwin Sedge takes the loads Renn was matching, and
Warin Underbridge's promised cart never comes. Clemence Crake, who was already looking twice at her
son's figures, now has the answer, and she is a woman who carries a struggling man three seasons and
takes his shop in the fourth. Dunstan Skell forecloses on keys that were never Renn's to pledge, and
that becomes Clemence Crake's fight with him.

**Cover-up.** A real smuggling route now exists: a bonded warehouse with an hour nobody records, and
possibly a copied key in the player's hand. It works. It also means everyone involved is exposed to
the next audit — Gaudry's Audit is a fixed date in the calendar — and Warin Underbridge, who is
cheerful and easily flattered and cannot read, saw whatever the player did.

**Discrediting the Tallage.** If the reweigh is used to prove the beam or the weigher wrong, freight
gets slower and dearer for everyone: longer queues, second provings, Ewart Tarn stood down or
standing on a knee that has been wrong since he was forty while the square watches. Averil is the one
who has to read the figure that ends her father.

**Sacrificing Hugh.** The Brede cart stops. `brede_wool_gate` does not run. Raw wool into Ombreval
constricts, which is felt at the Draper's Reach and by every weaver Skell puts out to — including
Clemence Hobbe, whose independence was the thing the player was protecting.

---

## Failure matrix and recovery

No reload-shaped failures. Each row is a new problem, not an ending.

| What goes wrong | What it becomes |
|---|---|
| Player arrives after the opening | The bolt is found and Hugh is seized. Play continues: surety, a debt bargain, proving who broke custody, or the Stone House's fifth door. |
| Renn runs when accused | A fugitive who knows the warehouse. He can be found, followed, or left; his flight is itself the evidence, and it raises a notice of its own. |
| Ede of the Needle is frightened | She goes quiet and still, and stays closed for the rest of the quest. The message-carrier route is gone; the cord, the wax and the beam remain. |
| Warin Underbridge is made to feel a fool | He denies hearing anything, loudly, in front of the square, and now the player has an enemy who is the biggest man on the Cut. |
| Player forges a seal | It passes officers, because nothing that reads a seal asks who pressed it. It fails before Odo Trask, who looks twice now. And Warin saw the player borrow the wax. The failure lands *later*, in public, at the opening. |
| Player is seized | Custody, the tether, the Stone House, the posted fee and the bell they are told they go at. All existing (`law_and_order` M4/M5). The opening happens without them. |
| Hugh takes the road and abandons the cargo | The accused has left the city. The notice climbs to warrant. The bale opens anyway and the crime is now unattached to anyone, which is worse for Clemence Hobbe. |
| Player pays the duty but names nobody | Lawful and incomplete: the bolt is entered to no weaver, the officers keep asking, and Trask records that the money came from the player. |
| Six households riot at the door | The officers may release the lawful bolts early. The bale is opened in a scrum and the cord evidence is destroyed. |

---

## Prompt surface

Additions are small and mostly reuse existing sheet sections.

- `you_hold` already renders items; a bolt of broadcloth needs no new rendering.
- **New sheet block `the_lot`**, rendered only for characters with a live custody or ownership
  relationship to a lot (Hugh, the carters, the officers, Warin, the warehouse holders): the label,
  where it is, whether it is impounded, and what the paper says is in it. Never the true contents.
- **Notices** already render; the notice against Hugh appears in the ordinary `notices` block with
  its rung, and its summons names the Dayspring bell in the existing sentence.
- **Percepts** for: the gate stop and turnaround; the impound; a weighing called aloud (to everyone
  in `heard_by`); the cord cut at the opening; the count read out.
- **`what_you_know`** is the knowledge layer's block, not this quest's. The quest adds facts to it;
  it adds no rendering.
- Dispositions are *not* prompt fields. Ede's fright, Warin's pride and Trask's second look are
  inbox lines and recent history, so the model reasons about them as things that happened rather
  than as a stat.

Every string lives in `assets/prompts/strings.toml`; nothing about the format goes in Rust.

---

## Data ownership

| File | Adds |
|---|---|
| `assets/world/items.json` | `weight_lb` on kinds (general, not quest data) |
| `assets/world/rounds.json` | `quest_legs` override on `brede_wool_gate` |
| `assets/world/quests/bale.json` | **New.** The lot, its true contents, the declared manifest, the seal, the cord, the custody chain including the unrecorded leg, the six households, the outcome packages — **and the quest's authored `facts`** (`01_facts.md` schema). Absent ⇒ quest off. |
| `assets/prompts/strings.toml` | `the_lot` block strings, weighing and opening percepts. **Not** the `what_you_know` block or the hedging vocabulary — those are the knowledge layer's. |
| `lore/characters/**` | **No edits.** The sheets already carry it. |

---

## Engine and projection seams

New player commands — three, all about acting on the bale:

- `PlayerExamine { lot_id, aspect }` — ask a nearby competent character to read seal/cord/beam
- `PlayerStageWeighing { lot_id, beam_id, standard_id, witnesses }`
- `PlayerLodgeAmendment { lot_id, manifest, witness }`

**There is no `AcceptQuest`.** An earlier draft listed one, copied from the sibling specs. It does
not fit: this quest has no offer scene and nothing to accept (see *How the quest starts* above).
`PinQuestLead` is also gone — pinning is a journal affordance and the journal is the knowledge
layer's.

New messages:

- `EngineMessage::BaleQuest(BaleQuestView)` on activation and on a dedicated monotonic quest
  revision. It carries the clock, the stake and the lot's *player-visible* state — not receipts,
  which arrive through the knowledge layer's own channel.

**Do not** put the journal in the actor/item `PublicSnapshot`, and do not touch the public-state
revision per fact learned — that republishes the whole cast and the configured crowd, and
`PublicSnapshot`'s 160 KiB bound already has little headroom (see `lore-items-wave`). The Bevy side
renders player-safe typed state and never re-derives weights, seal authenticity or guilt.
`Lot::seal.pressed_by`, `FactSource`, the true contents of a sealed lot, and facts the player does
not hold never enter any view.

---

## Milestones

Each is independently playable and testable, headless, with the fake backend.

**Prerequisite: `features/knowledge_and_rumor/` through its own M4.** Nothing below starts before
it. That is a real schedule cost — its M4 is a perceptibility tuning pass with an open-ended shape —
and it was accepted deliberately on 2026-08-30 in exchange for three quests not each building their
own knowledge system.

### M0 — Weight, lots, and one clock
- `weight_lb` in the item catalog with per-metadata variants; `ItemCatalog::weight_lb`.
- `lots.rs`: `Lot`, `SealImpression`, `CordState`, `CustodyLeg`, `true_weight_lb`, `discrepancy_lb`.
- `quest.rs` shared host: id, phase, data gate, outcome application. **Not** receipts or the
  casebook — those are the knowledge layer's.
- `BaleQuest` state, `Arming`, the quest-day leg override, the seizure at the gate, the Dayspring
  opening, and the resolution function with **no** investigation content.
- Headless: start the quest, advance to Dayspring, observe the extra bolt found and Hugh seized.
- **Quest-disabled prompts, snapshots and goldens remain byte-identical.**

### M1 — The public weighing
- `Weighing`, standards, the reader seam, `heard_by` off the existing hearing calculation.
- Bertran Hobbe's proved weights as a real item and his Lowmarket proving as a real event.
- `PlayerStageWeighing`; the lawful lane end-to-end: reweigh, amend, pay duty, settle Hugh's notice.
- This is the **go/no-go** for the feature: if a lawful reweigh cannot be staged out of existing
  schedules and speech without a quest-only verb, the design is wrong.

### M2 — The evidence surface
- `assets/world/quests/bale.json`: the seven evidence traces as **authored facts** with sealed
  `seeded` sets and per-holder `own` phrasings. No Rust.
- Arming: the bolt minted, `bale.promise` seeded, Renn's goal and round set via `World::arm_actor`,
  Ede's errand walked.
- The three examinations (seal, cord, beam) as authored dispositions — who *will* say what they know.
- The single scoped warehouse key and its reach check.
- Deterministic fake-backend answers for every fact-holding character.
- Test: `FactSource` and `pressed_by` appear in no rendered string.

### M3 — The four lanes
- Social: the three-in-one-room confrontation; Skell buying the bolt; the six households.
- Covert: removing the bolt, re-laying the cord, forging a seal, altering the custody copy.
- Predatory: selling the truth; taking the key as the price of silence.
- Integration with `notices.rs` escalation and `custody.rs` seizure, including the player being taken.
- Correctness must hold with the Night Office disabled.

### M4 — The opening and aftermath
- The full public opening presentation, bounded, with the crowd and the announced result.
- Five outcome packages applied to the live world: pay-by-the-ell at the Reach, brokerage changing
  hands, the Brede cart stopping, queue length at the Tallage, the smuggling route and its key.
- Follow-up lines that make the new state legible with no ending card.

### M5 — Content, polish, ship
- All six loom households, all failure rows, all recovery routes.
- Prompt-surface tuning and a full golden re-bless.
- Drive scripts for each lane in `.claude/rules/CATHEDRAL_DRIVE.md` style.

---

## Acceptance criteria

### Deterministic sim
- `cargo test -p cathedral-sim` passes; every quest test is offline and deterministic.
- With `assets/world/quests/bale.json` absent, golden prompts are byte-identical to today.
- `true_weight_lb` is never cached; a test removes a bolt from a sealed lot and asserts the
  discrepancy goes to zero with no other state touched.
- No `HashMap` iteration order reaches a snapshot, prompt or golden.
- The opening's resolution function is pure and reads no quest phase.

### Content
- All five opening branches reachable from a scripted headless run.
- At least four materially different routes reach a cleared Hugh, and at least two of them expose
  neither Clemence Hobbe nor Renn.
- Every failure-matrix row has a scripted run that continues to a resolution.

### Player comprehension
- The casebook contains only sentences the player heard or acts they took.
- No rendered string contains a bare "Clemence"; asserted by test over the feature's strings.
- `pressed_by` appears in no projection, prompt, log line or HUD.

### Integration
- Stage cap and the single in-flight cognition slot are unchanged; the quest adds no LLM lane.
- `PublicSnapshot` size is unchanged (the canary still passes).
- Runs with `CATHEDRAL_EXTRA_NPCS=2000` without changing the quest's outcome.

## Vertical-slice acceptance scenario

Headless, fake backend, one command:

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --quest bale --start-office highwick --seconds-per-day 600 --watch-clock 0.5
```

Expected transcript: the gate stop and turnaround; a notice raised against `rbrde`; the player
staging a weighing at the Tallage beam on Bertran Hobbe's proved weights before Odo Trask; an
amendment lodged; forty sparks of duty paid; the notice settled; the Dayspring opening reduced to a
formality; and, three lines later, Ewart Skell at the Draper's Reach saying what he now pays by the
ell.

---

## Risks and open questions

1. **The knowledge layer is now on the critical path — accepted, but it is the schedule risk.**
   Decided 2026-08-30: `features/knowledge_and_rumor/` ships both halves together and all three
   quests wait for it. Its own principal risk is perceptibility tuning ("the LLM must voice the
   injected line often enough for the player to feel the wave, without every mouth parroting the
   same sentence"), which is open-ended by nature. If that milestone stalls, three quests stall.
   The alternative considered and rejected was an interim fact-only layer with a deterministic
   gossip roll standing in for propagation.
2. **`quest.rs` is smaller than it was.** With receipts and the casebook moved to the knowledge
   layer, the shared quest host is a phase, a data gate and outcome application — small enough that
   landing it in M0 no longer commits the sibling specs to much.
3. **The warehouse door.** The scoped one-key substitute is honest but it is a second half-
   implementation of a spec'd feature. If `keys_and_locked_places` is close, do it first.
4. **Duty payment has no general credit path.** Third feature in a row to hit this
   (`chalking-the-walls`, `crowd-knob`). It should be fixed properly rather than worked around a
   fourth time.
5. **LLM confusion between the two Clemences** is desirable in fiction and dangerous in a golden
   test. The string rule handles authored text; it cannot handle a model's own sentence. Accept it.
6. **`settle_notice` on a false accusation.** The existing ladder settles by returning `taken` or by
   the wronged party's say-so. Neither fits "the accused did not do it". M1 must decide whether
   amendment discharges a notice or whether a new discharge reason is needed — this is a change to
   shared law code, not quest code, and it needs care.
7. **Does the player ever learn they were wrong?** If the player clears Hugh by removing the bolt and
   never learns whose it was, they have solved the quest without meeting its subject. That is a
   legitimate and rather good outcome, but it should be checked in playtest that it does not feel
   like content was missed.

## Source references

- `features/systemic_quest_suggestions.md` §02 (the promoted idea)
- `lore/characters/{merchant/rbrde,freight_broker/fr9ck,cloth_worker/e5hob,draper/e1skl,merchant/fp6ck,money_dealer/fb3sk,revenue_worker/fe2tn,revenue_worker/fa8tn,cargo_worker/fw7ub,court_officer/fo6gl,miller/e7mil,messenger/he3nd,roper/e6ptr,scribe_and_clerk/fg4br,farmer/ecbrd}*.json`
- `lore/places/02_canonical_gazetteer.md` (The Tallage, The Tally Bridge)
- `lore/core_lore/trade_and_daily_life.md` (measures, the Tallage stone, Gaudry weights, the week)
- `assets/world/rounds.json` (`road_parties[0]`), `assets/world/items.json`, `assets/world/places.json`
- `crates/cathedral-sim/src/{notices,custody,marks,round,item,inventory}.rs`
- `features/implemented/law_and_order.md`, `features/implemented/chalking_the_walls.md`
- **`features/knowledge_and_rumor/`** (SPEC ONLY — the hard prerequisite; `01_facts.md` is the
  contract this quest authors against)
- `features/keys_and_locked_places.md` (SPEC ONLY — the other real dependency)
- Sibling quest specs: `features/quest_ring_a_dead_womans_name_at_marenstide/`,
  `features/quest_secure_votes_for_a_drainage_funding_plan_before_the_rain/`
