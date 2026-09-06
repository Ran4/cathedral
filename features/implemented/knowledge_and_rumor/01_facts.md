# Facts — the held half

The proposition, who holds it at first hand, what a holder says, **what a non-holder says instead**,
and what the player has learned. `02_rumor_pollen.md` is how it travels to everyone else.

## The type

The public API lives under `cathedral_sim::knowledge`; calls use `knowledge::holds` fully qualified
(the world also has inventory holdings). `FactId` is an authored string; `FactKey` and `AreaKey` are
interned handles. The following is the shipped shape, with derives and methods omitted:

```rust
pub struct Fact {
    pub id: FactId,
    pub key: FactKey,
    pub sequence: i64,
    pub subject: Vec<ActorId>,
    pub place: Option<AreaKey>,
    pub day: Option<i64>,
    pub said: String,
    pub own: BTreeMap<ActorId, String>,
    pub seeded: BTreeSet<ActorId>,
    pub garble: GarbleMask,
    pub decays: bool,
    pub topic: Topic,
    pub minted_game_days: Option<f64>,
    pub quiet_among: BTreeSet<ActorId>,
    pub craft_ear: Option<String>,
    source: FactSource,
}

pub struct Held {
    pub hops: u8,
    pub from: Option<ActorId>,
    pub learned_on: Option<f64>,
    pub view: FactView,
    heat_at_learn: f32,
}

pub struct FactView {
    pub subject: Option<ActorId>,
    pub place: Option<AreaKey>,
    pub day_offset: i8,
}

pub struct Telling {
    pub hops: u8,
    pub from: Option<ActorId>,
    pub heat: f32,
    pub view: FactView,
}

pub fn holds(world: &World, actor: &ActorId, fact: &FactId) -> Option<Held>;
```

`Held::heat(game_days)` derives current heat from `heat_at_learn` and `learned_on`; changing the poll
rate cannot change aging. `None` game time does not age a clockless world. An authored or coded-event seed has hops 0,
a pristine view and no upstream mouth; a decaying seed ages from the fact's mint time. Standing seeds
stay fresh but never volunteer or deposit without a relevance reheat. Carried rows, including a
reheated standing fact's override, cool normally.

`holds` / `holds_key` resolve a stored override, then the fact's seeded default. Disabling knowledge
makes both return `None` and `holdings_of` return an empty list, without destroying the store.
A raised claim can retain its upstream speaker in a stored first-hand override, so its chain can
reach the player who supplied it. `Telling` is the incoming value for `learn`; `round::Arrival` already names a different concept.
`FactView` stores deltas, never a sentence, and has no topic field: topic drift is unrepresentable.
`sequence`, `minted_game_days`, `quiet_among` and `craft_ear` are frozen when the fact is born.

`said` and `own` are templates using only `{subject}`, `{place}` and `{day}`. Rendering resolves
names for the reader through `person_word`: an unknown person becomes their role, including the
stranger, and a garbled subject can therefore be a real person whose name the listener does not know.
A self-subject receives their `own` line or nothing, and cannot pick up or carry their own story as
news. Seeded witnesses are exempt from the six carried-row cap.

### Why `own` and `said` are separate

It is the cheapest available answer to the parroting risk. If a fact has one string, every holder
says the same sentence and a ward of parrots is guaranteed by construction. With the split, Renn
holds *"I promised her bolt would go on Hugh's cart"*, Ede holds *"I carried that word to the bridge
for a penny"*, and a Weigh-ward gossip four hops out holds *"they say some weaver's cloth was in that
bale"* — three different sentences from one authored fact, before garbling has done anything at all.

### Why `source` is never rendered

A fact says *what*. `FactSource` says *why it is true*, and in a quest that is usually the answer the
player is looking for. Keeping them in one struct but rendering only one of them is what stops a
knowledge system from being a spoiler pipe. The payload is private; `FactSource` has no serialization or display implementation, and its custom
`Debug` prints only `FactSource(<sealed>)`. `Fact` itself also keeps the source field private.
Projection tests cover prompts, journal entries, snapshots, ward heat, census and diagnostics.
Only the claim bit and claimant link are exposed; the source payload reaches none of these surfaces.

## The ignorance rule

**This is the half that makes the other half perceptible.** A holder and a non-holder answering with
the same confident invention would make the entire store invisible from the player's chair.

The shipped instruction is the measured `m0_evidence/ignorance_rule.txt` / `prose/v6_both/` paragraph, installed
**unconditionally immediately before “Use ONLY the verbs listed below” in `turn.j2`**. It applies
even when the actor has no knowledge block. It tells an actor to acknowledge what they do not know
and refer the question by trade, post or place, without supplying an invented person, day or number.
The knowledge block separately says to use a held word when relevant, not announce every bullet.

M0/M0b measured that wording and position on both providers. M1 shipped the 24 frozen string keys and
22-line paragraph; M3 measured garbled replies, and M4 measured the occasion interaction. The full
record and limitations remain in `m0_evidence/NOTES.md`. The role-referral fallback was not invoked.
M4 C1 under-fired on both providers (accepted): coded mints supply ordinary city news, while a
qualified assertion can offer `raise_word` without forcing the model to use it.

## The merge rule

A person can hear the same fact twice: at four hops and garbled from a gossip, then at zero hops from
someone who was there. `Held` is one per `(actor, fact)`, so what happens on the second arrival has to
be decided, and it is the player's main verb:

| Incoming `Telling` | Effect |
|---|---|
| Fewer hops than held | **Replaces** `view`, `from`, `hops`. `heat` takes the maximum. The story is corrected. |
| Equal hops | Keeps the held `view` — a person does not flip-flop between two equally distant versions. `heat` takes the maximum. |
| More hops | `heat` takes the maximum and nothing else changes. Corroboration warms a story; it never rewrites it. |
| Any arrival, when held at hops 0 | Ignored entirely. A witness cannot be talked out of what they saw. |

That table is why walking to a ward ahead of the pollen is worth doing. It is also what the shipped stranger
reading (`02_rumor_pollen.md`) stands on: you can beat your own garbled story to a ward and put the
zero-hop version in first, and everyone who then hears the four-hop version keeps yours.

## Cold is not forgotten

An earlier draft dropped cold tokens. That means someone who heard about an arrest three days ago
returns `None` from `holds()` and genuinely cannot answer a direct question about it — which will
read as amnesia, and will be blamed on the LLM rather than on the store.

- `heat` ∈ 0..=1, cooling per game hour and per hop. It gates **volunteering**, not knowing.
- The gate is `heat × salience(fact, holder) > VOLUNTEER_HEAT`, not `heat` alone — the same product
  that drives pickup (`02_rumor_pollen.md`), so a person repeats a thing on the same terms they
  caught it. A scandal therefore stays on the sheet long after a squabble of equal age has dropped
  off it, a laundress goes on volunteering bed-talk that a mason has stopped mentioning, and someone
  of the subject's own household barely volunteers it at all. One expression, no new selection rule.
- Above that threshold, a fact may be seated on the sheet on its own.
- Below it, the fact leaves the sheet but stays in the store. Relevance selection can still seat it —
  somebody asked — and it renders in the faded register: *"you heard something of the sort, a while
  back."*
- **A relevance-seated fact re-heats on a speaking turn.** Relevance seating means somebody just
  asked, and an old thing that gets talked about starts going round again — that is a consequence,
  not a decision, so it is a sim-side rule and no verb. It lifts heat to the absolute `REHEAT_TO = 0.1309`, never divided by salience, so a revived scandal can circulate while a revived trade matter stays answerable.
  Standing facts get a local air row only through this path, and that row cools normally. The approximation (the actor may have been asked and dodged) errs harmlessly: asking about
  something in public is itself how it gets going again. **This is what lets the player re-heat a
  cold rumour by asking about it** — a real verb with no verb attached, no prompt surface and no
  tokens, and exactly the poking-at-a-dead-story that the three quests are made of.
- Cooling removes spent air, not a person’s knowledge. Explicit invalidation removes the fact;
  bounded live-fact and carried-row eviction can also remove it.
- The store is bounded per actor (a low cap — a person is not a newspaper), evicting coldest first
  and highest-hops next. Views are deltas, so the bound is bytes, not sentences.

## Authoring: data, not code

Facts are data, following the crate's standing rule (`cathedral-sim/AGENTS.md`, "Data, not code").
The base game authors `assets/world/facts.json`; a quest authors its own pack and owns no Rust type:

```json
{ "schema_version": 1, "facts": [
  { "id": "bale.promise", "topic": "talk",
    "said": "{subject} promised a weaver a place on the Brede cart",
    "own": {
      "fr9ck": "I promised her bolt would go on Hugh's cart",
      "e5hob": "{subject} promised my bolt a place on that cart",
      "he3nd": "I carried that word to the bridge for a penny"
    },
    "subject": ["fr9ck", "e5hob"],
    "seeded": ["fr9ck", "e5hob", "he3nd"],
    "decays": false, "garble": "none" },
  { "id": "bale.stop", "topic": "law",
    "said": "a cart was turned at {place} {day}; the beam called forty pounds over",
    "place": "Wool Gate", "day": 0,
    "decays": true, "garble": "place,day" }
]}
```

This is an authoring example, not a third shipped base-game pack. The host reads the quest JSON and
passes its text to `knowledge::catalog::FactCatalog::extend_from_json`, then seeds the catalog into
the world at the quest's start. The real pack must use the city's actual actor and area IDs and set
the witnessed row's seeds/place/day at the seizure hook. There is no `minted_by` field or automatic
quest-event interpreter. The loader rejects unknown fields and validates template/mask agreement.

`bale.promise` is sealed to three people and never drifts; its self-subjects retain their own lines.
`bale.stop` is seeded with actual witnesses and spreads as ordinary pollen, with place and day free
to go wrong. A fully garblable template uses `"subject,place,day"`; a sealed one uses `"none"`.

Their topics are worth reading against each other, because they are the feature's own worked example.
`bale.stop` is `law` and crosses the city; `bale.promise` is `talk` — base 0.15 — and goes almost
nowhere, which is precisely the quest: the city loudly knows the *accusation* and three people quietly
know the *arrangement*, and closing that gap is the player's job. The temptation to give
`bale.promise` a high band so it "reaches the player" is the one mistake this vocabulary exists to
make visible. A dull fact reaches the player by being **asked for** — relevance selection seats a
cold, dull, four-hop fact the instant its subject is mentioned nearby — not by being loud.

**Most of the base game's facts are minted, not authored.** That is deliberate: the city gossiping
about its own arrests, knells and market days is the content this feature is tuned on, and it costs
no authoring at all.

## Minting from events

Mints are **explicit hooks after successful operations**, not a `World::emit` interception. Each
computes `characters_within(at, HEARING_RADIUS_M, None)` itself, following
`actions::raise_ward_notice_for`: commit events have empty
`recipient_ids`, and gaol delivery gates that set, so it is not a witness oracle.

| Hook | Topic and template source | Witnesses / special rule |
|---|---|---|
| custody commitment | `Law`, fixed `MINT_KINDS` row | actual earshot at the committed event |
| `raise_notice` | `Law`, fixed accusation-exists template | actual earshot; the LLM deed is interpolated inside a fixed accusation-exists template |
| `EngineCommand::Knell { years, at }` | `Blood`, fixed age-filled template | earshot at the tower; empty subject and **day-only** garble; equal age/day collapses |
| player `draw_mark` / `scrub_mark` | `Stranger`, separate fixed rows | earshot, with the player as a first-hand seed; NPC use mints nothing |
| `EngineCommand::CivicPeal` | no mint | amplifies matching `Law` air within the bell's supplied carry |

`place` is interned from the event position, `day` comes from game time, and `topic`, `said` and
`garble` live together in `knowledge/mint.rs::MINT_KINDS`. They add no prompt-string keys. A coded
mint remains event-sourced even if an LLM action triggered it: it establishes that an accusation or
mark happened, not that the model's accusation is true.

The large accepted-sale mint is **declined (D32)**: the sale event has empty recipients, carries no
price, and minting inside `World::market_sale`'s staged clone would lose the result on transaction
failure. There is no generic “memorable deed” text-to-truth path.

## Where facts come from

Three routes, and only three. Every one of them ends at the same `Fact`, and the difference between
them is entirely in `source` and in how the `topic` is arrived at.

| | Route | Topic comes from | `source` |
|---|---|---|---|
| 1 | **Authored** — `assets/world/facts.json` and per-quest packs | the author | whatever the author binds it to |
| 2 | **Minted from an event** — explicit successful-operation hooks | a constant per event kind | the event |
| 3 | **Coined by a mouth** — `raise_word`, M4 | **the speaker's own classification** | `FactSource::claimed(speaker)` |

Route 3 is new, and it is the only path by which an LLM creates a fact.

### One verb, and why repetition does not get one

An earlier draft had `pass_word(fact)` doing two jobs — which is why the same page could say it takes
a `FactId` and also say that what the player tells a credulous mouth "enters the air". Only one of
those two jobs needs a verb at all:

```rust
/// Coin a proposition, here, now, in your own words. The only path by which a
/// model creates a fact — and everything it creates is a **claim**.
raise_word { topic: Topic, said: String }
```

**Repeating a fact you already hold needs no verb**, because Layer 1 already does it: a carrier warm
enough to be saying a thing deposits their telling on their own game-time pollen poll, lowering the ward's `hops`
and becoming its `via` if they are closer to the source than the air is (`02_rumor_pollen.md`). A
verb for that would be a model declaring a side effect the sim performs for it on a jittered ten-game-minute cadence, and it would fire constantly for no observable result.

**Re-heating a cold one needs no verb either**, for a better reason: warming is not a decision
anybody makes, it is what *happens* when an old thing gets talked about. So it is a rule, on the sim
side, in "Cold is not forgotten" above — a relevance-seated fact re-heats on a speaking turn, which
also hands the player a real verb with no verb attached (*asking about a dead story is what revives
it*).

The name follows `raise_notice`, and the analogy is load-bearing rather than decorative: both bring a
new, **attributable, contestable** object into the city — a notice can be settled, a raised word can
be walked back to the mouth that raised it. The noun is deliberately `word` and not `fact`: the model
must never be told it is handling facts, because the whole feature turns on the gap between what is
true and what somebody claims, and `raise_fact` asserts truth at exactly the moment truth is the
thing in question. `word` is the city's own register already (`word_in_the_ward`, the word in a
ward's air), and `Fact` stays an internal type name that no prompt ever says.

### When the verb is offered — a precondition, not a judgement

"The prompt tells it when" is a hope. Every comparable verb in this game is **gated by the sim**:
`draw_mark` needs reach and a pen, the ward's cross needs an aged unsettled notice, `raise_notice`
needs a law occupation. So `raise_word` appears in an actor's verb list **only when there is an
occasion for it** — that is, only when, since their last turn, either:

- **somebody asserted something to them that they do not hold** — `since_your_last_turn` carries the
  speech and `holds()` answers the rest, so this is a lookup, not a classifier; or
- **a percept reached them that minted no fact** — they saw something the whitelist does not cover.

That turns "how does the model know?" into a structural answer: *it knows because the verb is not
there otherwise.* It also makes the player-lie path exact rather than incidental — you tell somebody
a thing they do not hold, the condition goes true, the verb appears, and they may raise it. Nothing
else can reach it.

Two backstops on top of the gate:

- **One raise per actor per office.** A person starts a rumour rarely, and a hard cap bounds the
  store however a model behaves.
- **Collision reject.** A raise whose `(topic, subject, place, day)` already exists in that ward's air
  is refused as a no-op — deterministic, structural, no text comparison, and it stops a ward
  re-minting the same event eight times.

**It will still under-fire, and that is correct.** Coded mints are the staple — most of the base
game's facts are minted, not authored — and route 3 is the spice. A quiet verb is a far cheaper
failure than a loud one.

### Why the model picks the topic and never the number

The obvious design is to let the speaker say how juicy the thing is. It fails in a predictable
direction: a model asked to rate the importance of its own utterance will inflate, because everything
it has just decided to say feels worth saying, and a city where every mint is a scandal is
indistinguishable from the flat city this section exists to replace. Worse, a free float is exactly
the tuning surface `02_rumor_pollen.md` refuses on principle — one derived from a stated target, not
five hundred opinions.

So the division of labour is:

> **The mouth says what kind of thing it is. The city decides how far that kind of thing travels.**

Topic is a *classification*, not a self-assessment. It has an external check — you can read a fact
and see whether "who is in whose bed" got tagged `Bed` — it is drawn from a closed list of nine, and
it is the same tag a coded mint carries, so a claimed fact and a minted one are the same kind of
object from the moment they exist.

### The guardrails, which are the whole of the safety argument

`raise_word` is a model writing into the world's knowledge store, so what it *cannot* do is the
specification:

- **`source` is always `FactSource::claimed(speaker)`.** Unforgeable, not a parameter, and it is what
  makes the thing invalidatable, walk-back-able and — since `source` is never rendered — never a leak.
  **A model can mint claims; it can never mint truths.** Tests also walk the coded action hooks: a
  notice or a drawn mark is recorded with fixed event prose, never the LLM’s free text as established truth.
- **`seeded` is the speaker alone.** A claim cannot seat knowledge in other people's heads.
- **`decays: true`, always.** A claim is news and news cools. Nothing said aloud becomes a standing
  fact of the world.
- **The default mask is `ALL`, narrowed to actual placeholders.** `mint_claim` replaces the first
  recognized visible subject name with `{subject}`. Free text contains no trusted place/day slots,
  so claims garble only that subject; without a recognized subject they have no drifting field. The
  speaker’s first-hand seed remains pristine.
- **`subject` resolves only against actors already on the speaker's own sheet.** A claim cannot invent
  a person to be about — `no-procedural-characters` holds here as everywhere.
- **An unrecognised topic tag falls back to `Talk`**, the dullest band. The failure direction is
  deliberately downward: a mis-tagged fact that under-spreads is a shrug, and a mis-tagged fact that
  becomes a citywide scandal is the bug that would make this verb unshippable.
- **It costs nothing.** `raise_word` rides a turn already being paid for, which is the same bet the
  rest of this feature makes.

### What this hands the player

The M4 toy — say a false thing and watch it come back to you three days later with your name filed
off — stops being a hand-wave and becomes a mechanism, because a hearer of player speech coining a
claim is route 3: the raiser’s holding points back to the player, and its air names the raiser as
`via`. Telling them something they do not hold is precisely the condition that offers the verb. And it acquires a rule that is far
better than the one it replaces:

> **The player cannot set the salience of their own lie.** They get whatever the mouth they told
> makes of it.

To spread something quickly, you must make it *sound like a scandal to the person you are telling* —
and to bury something, tell it to someone who will hear it as a trade matter. That is a real verb, it
is legible without a tutorial, and it falls straight out of putting the classifier in the listener's
head instead of in the speaker's.

## Invalidation

A fact can stop being true. `FactSource` is what lets the sim notice:

- `FactSource::item_with` dies when the item changes hands or leaves `world.items`; items have no
  independent position in this sim;
- a fact sourced on a custody record dies on release;
- a quest-phase source is an explicit future seam: no quest phase engine exists yet, so that arm
  currently stays true. A quest must wire the phase predicate or explicitly invalidate its fact.

`pollen::sweep` calls `invalidate_stale` on its stir beat. Dead facts are dropped from live facts,
all holdings and all wards’ air, which removes them from every sheet on the next turn — with no
`forget` verb, no LLM cooperation and no drift. Carriers who were holding it simply stop saying it.
(A deliberately *stale* rumour outliving its truth is a legitimate authored choice: set
`decays: true`, leave the source unbound, and the ward goes on saying a thing that is no longer so
until it cools.)

## What reaches the sheet

`what_you_know` is bounded to `KNOWN_SHEET_MAX = 3`, smaller than `NOTICES_SHEET_MAX`'s 4 — and two rules fill
it, in this order:

1. **Relevance.** A held fact about somebody within inclusive 20 m is eligible for the greeting
   register. A fact whose subject, place or a distinctive noun appears in `since_your_last_turn`
   or `recent_history` is seated first, whatever its heat, including a faded one. Somebody asked; the
   answer must be on the sheet. "The hottest thing this actor carries" is a gossip rule and it is the
   wrong rule for an interrogation, which is what every quest here is made of.
2. **Heat.** What is left goes to the warmest, then fewest hops, then `FactId` — a total order, so
   the sheet is stable across runs and goldens.

A fact whose `subject` contains the actor is never rendered *to* them as news. They are not told
about themselves in the third person; if they hold it, they hold their `own` line or nothing.

## Facts the code reads (M5)

Four deterministic readings make a held word matter without another LLM call:

- A bound vendor with a warm `Coin` telling about the buyer refuses the sale in `round::try_purchase`.
  “Refuses credit” means that sale is refused: there is no separate loan/cash route. The buyer gets a
  refusal percept without an attention nudge. A half-game-day deadline prevents requeue spam; after
  it expires the buyer can seek another vendor. Heat uses game time even at clockless call sites
  through the world's current clock projection.
- An idle householder within **10 m of their own home**, with the caller at that door and a warm
  telling about them, is gated out of attention/admission. This matches the day-worker’s 10 m leash,
  rather than the 4 m hand-touch reach. Some other archetypes already wander farther (up to 24 m),
  so their door gate applies only while they remain inside the same 10 m home radius. An inbox line is a knock and bypasses the gate; so do the
  existing urgent/in-flight paths and `All` attention mode. It is a cognition gate, not a locked
  physical door object. The live filter is `Engine::poll`’s Stage idle admission, outside `idle_requires_news`; it
  leaves existing attention selection and urgent/reaction admission intact.
- A law officer's second-hand `Law` telling can raise a notice against the **held, possibly garbled
  subject**. `Rung::Hearsay` is declared first, below `Word`, so sorting never treats hearsay as a
  warrant. The existing summons and settlement ladder applies. This path mints no new fact and
  deduplicates its notice: there is no fact → notice → fact feedback loop.
- A fact about the person standing in front of a holder is seated by relevance, even when cold.
  The ordinary hop phrasing then gives the greeting its register. Selection and speaking-turn
  reheat use the same relevant keys; no new greeting probability exists.

## The player's side

The player is a carrier like anyone else, and a receipt is what a carried fact looks like from the
outside:

```rust
pub struct LearnedHow {
    pub word: String,
    pub at: Option<f64>,
    pub place: Option<AreaKey>,
    pub from: Option<ActorId>,
    pub hops: u8,
    pub tellings: u16,
    pub wards: u8,
    // Private bounded mouth bookkeeping and an eight-bit ward set.
}

player_learned: BTreeMap<FactId, LearnedHow>
```

There are at most 64 receipts; the newest 24 are projected as `EngineMessage::Journal { entries,
standing }` at a deduplicated one-real-second cadence. `word` preserves the player's actual rendered
version even after the carrying row is evicted. `place` is **where it was heard**, not the canonical
fact's potentially garbled event place. A closer telling corrects the stored sentence; repeated
mouths do not become new people. Ward counts use a bitset. Player stage receipts also suppress a
repeated successful pair within a game stir, so the two-real-second visibility scan cannot inflate
counts. Witnessing an ordinary coded mint records a receipt even though the pristine held seed
refuses a merge.

`from` and `hops` are the journal entry *and* the "who told you that?" chain. A player who was told
something wrong can trace it back to the mouth it went wrong in — and because garbling is
deterministically seeded per `(fact sequence, carrier id, hops)`, the trace is a reconstruction, not
a stored log.

### The journal (J)

An overlay on the inventory overlay's pattern (`src/smart_actors/inventory_ui.rs`), because that
interaction already works and players already know it.

It renders `player_learned`, newest first, **as the sentence the player heard, attributed**:

> *Warin Underbridge, at the porter stand, this morning — and three others since, in two wards:*
> "she went heavy, and I said nothing."

Rules, which are the difference between a journal and a hint system:

- **Only what the player heard or caused.** Never the authored truth, never `FactSource`, never a
  authored seed alone: a receipt requires hearing or a witnessed/caused event.
- **No objectives, ever.** Open threads render as the *questions they are* — "Whose cord is on that
  bale?" — never as instructions.
- **Provenance is shown.** A fourth-hand line is labelled as one. Being able to see that you are
  working from a garbled report is the point of the whole feature.
- **The count is shown.** "Three others since, in two wards" is what turns an invisible propagation
  model into something the player can play against. The “others since” clause is absent for a
  single mouth, and nouns agree with singular counts.
- **Two standing lines at the top**, supplied by whatever is live — a quest supplies a clock ("the
  bale opens at Dayspring — one bell away") and a stake ("Hugh Crake is summoned to answer for it").
  The journal knows nothing about quests; it renders what it is given. The shipped `standing_lines`
  currently supplies knowledge stakes; future quests add their clock/stake strings at that sim
  producer seam, without another journal or receipt store.

The last rule follows the game's own principle, already stated in `src/smart_actors/hud.rs` for the
law-standing line: *it must always name what would clear it — a brand with a visible door is a story,
a brand with no door is a bug.*

### The HUD

While a clock is live, one standing HUD line, not a toast — same reasoning, same precedent. A
deadline the player cannot see is not a deadline. M4 supplies the standing HUD resource and J
overlay; the journal scrolls inside a bounded panel and excludes chat, map, inventory and movement
input while open. No quest clock is invented when no quest is live.

## Test contract

- `holds()` is pure and deterministic; every roll is a hash of stable inputs.
- A `seeded` holder's view is byte-identical across runs and never garbled.
- **The merge rule**, all four rows: a closer telling corrects, an equal one does not flip, a farther
  one only warms, and a zero-hop holder is immovable.
- **Cold is not forgotten**: a fact cooled below `VOLUNTEER_HEAT` leaves the sheet, stays in the
  store, and is re-seated by relevance in the faded register.
- **`FactView` is deltas**: no rendered sentence is stored per holding, asserted structurally.
- `FactSource` appears in no rendered string: asserted by a test that walks every projection.
- Dropping a fact removes it from every sheet on the next turn with no actor cooperation.
- With no facts in the world, golden prompts are byte-identical to the M1 bless.
- Facts never enter `PublicSnapshot` (the size canary still passes).
- **The store is bounded** at `--extra-ambient 20000`: per-actor cap enforced, eviction order
  deterministic, total footprint measured and asserted.
- **A cold scandal out-travels a fresh squabble**: a `Bed` fact at heat 0.3 reaches more wards over
  the same interval than a `Craft` fact at heat 1.0. The single assertion that salience is not heat.
- **The flat-table identity**: with every band and affinity set to `1.0`, the measured
  carriers-per-ward-per-game-hour numbers reproduce M2's pre-salience run exactly.
- **Topic is invariant under garbling**, asserted over every hop of a walked chain.
- **The household is last**: over a run, the subject's own housemates hold a fact about them later
  than the city mean, and the subject never holds it as news at all.
- **`raise_word` yields `FactSource::claimed(speaker)` and nothing else**, asserted by walking every
  action-reachable mint — the safety property of the LLM mint path.
- **An unrecognised topic tag lands on `Talk`**, never on a high band.
- **`raise_word` is absent from the verb list with no occasion**, present with one, and refused past
  the per-office cap and on a `(topic, subject, place, day)` collision.
- **The relevance re-heat is bounded**: a cold fact asked about repeatedly rises to just above
  `VOLUNTEER_HEAT` and never approaches fresh news.
- **The cadence band, both ends**: the fast end within an office, and the slow end's expected
  crossings over a game day computed to be below one.
- M0's live-provider evidence — the holder/non-holder/asked/unasked sheets and their answers — is
  kept as a fixture next to the strings it produced, because it is the only record of *why* that
  prose is worded the way it is.
