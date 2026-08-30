# Facts — the held half

The proposition, who holds it at first hand, what a holder says, **what a non-holder says instead**,
and what the player has learned. `02_rumor_pollen.md` is how it travels to everyone else.

## The type

```rust
/// One proposition about the world. Authored, or minted from an event.
/// Its identity is the id: the same fact is the same fact however many
/// mouths it has been through and however wrong it has got.
pub struct Fact {
    pub id: FactId,                        // "bale.promise", "arrest.ede.wickmarket"
    /// Who it is about. Drives the "don't tell them about themselves" rule
    /// and any standing effects.
    pub subject: Vec<ActorId>,
    pub place: Option<AreaId>,
    pub day: Option<i64>,

    /// Third person — what somebody who merely heard it would say.
    pub said: String,
    /// First-person overrides for the people who did it or were there.
    /// The same fact, in their own mouth.
    pub own: BTreeMap<ActorId, String>,

    /// hops-0 holders: authored, or an event's `recipient_ids` at mint.
    pub seeded: BTreeSet<ActorId>,

    /// Which fields may drift on a hop; the rest are load-bearing truth.
    pub garble: GarbleMask,
    /// Authored standing facts do not cool. Event-minted news does.
    pub decays: bool,
    /// What it is *about* — and therefore how far it travels, how its
    /// hedges erode, and whose ear it catches. See `02_rumor_pollen.md`,
    /// "Salience". Invariant across every mouth: garbling moves the
    /// subject, the place and the day, and never moves the topic.
    pub topic: Topic,

    /// Why it is true. **Never rendered anywhere** — not in a prompt, a
    /// projection, a log line or the journal. This is the anti-leak field:
    /// it is how a fact gets invalidated when the world changes under it,
    /// and it is where a quest keeps the thing the player is trying to find out.
    pub source: FactSource,
}

/// What one person currently has of it.
pub struct Held {
    pub hops: u8,
    pub heat: f32,
    /// Who they had it from. `None` for a seeded holder — they had it from
    /// being there. This is the walk-the-chain link (M3).
    pub from: Option<ActorId>,
    pub learned_on: GameDays,
    /// The fact as *this* person has it, after per-hop garbling.
    pub view: FactView,
}

/// A carrier's version, as **deltas from the fact, never as text**. At 20,000
/// people a rendered String per holding is a megabyte of garbled sentences that
/// the sheet renderer would rebuild from the fact anyway — and deltas are what
/// makes the transmission chain reconstructible instead of merely logged.
pub struct FactView {
    /// The roll swapped the subject for this person.
    pub subject: Option<ActorId>,
    /// The roll swapped the place for this one.
    pub place: Option<AreaId>,
    /// The roll moved the day, ±1 per garbled hop, clamped to a small band.
    pub day_offset: i8,
}

/// None means they have never heard of it at all.
pub fn holds(world: &World, actor: &ActorId, fact: &FactId) -> Option<Held>;
```

`holds` checks `seeded` first (hops 0, heat 1, no garble, `from: None`), then the carrier store.
Every consumer — the prompt block, the journal, a quest, a systemic reading — goes through that one
call and nothing else.

### Why `own` and `said` are separate

It is the cheapest available answer to the parroting risk. If a fact has one string, every holder
says the same sentence and a ward of parrots is guaranteed by construction. With the split, Renn
holds *"I promised her bolt would go on Hugh's cart"*, Ede holds *"I carried that word to the bridge
for a penny"*, and a Weigh-ward gossip four hops out holds *"they say some weaver's cloth was in that
bale"* — three different sentences from one authored fact, before garbling has done anything at all.

### Why `source` is never rendered

A fact says *what*. `FactSource` says *why it is true*, and in a quest that is usually the answer the
player is looking for. Keeping them in one struct but rendering only one of them is what stops a
knowledge system from being a spoiler pipe. The rule is absolute and testable: `FactSource` appears
in no prompt, no `EngineMessage`, no snapshot, no log line and no journal entry.

## The ignorance rule

**This is the half that makes the other half perceptible, and it is prose, not code.**

A knowledge system that only adds knowledge cannot be seen from the player's chair. Ask a
stallholder who carried the bale to the gate and today's prompt gives her every reason to produce a
confident name — the sheet is full of people, places and trades to build one out of, and nothing
anywhere tells her not to. If she answers the same way whether or not she holds the fact, then
holding it changed nothing observable and the propagation model is invisible however good it is.

So the block's instruction paragraph carries three sentences, and they are as load-bearing as the
store:

> These are things you know. Treat them as knowledge, not as announcements — say one when it bears
> on what is in front of you, or when somebody asks, not because it is written here. If you are
> asked something that is not here and you were not there, **you do not know it**: say so plainly,
> and if you can think who would — by their trade, their post, or where they were — name them. Never
> supply a name, a day, a place or a number you were not given. A guess said aloud becomes what the
> ward believes.

The last sentence is not decoration. From M4 it is literally true: player and NPC speech both mint
pollen, so a guess said aloud really does become what the ward believes, and the model is being told
the actual rule of the world it lives in.

**Directional ignorance is the point.** "I don't know" is a wall; "I don't know — the porters were at
the gate, ask them" is a lead. A city of walls is not investigable, and three quests are made of
investigation. The referral is the model's own, made from `you_see`, `places_you_know` and its lore —
the sim cannot hand a non-holder a pointer at a fact without leaking the fact's existence, which is
the whole reason `source` is sealed.

**M0 measures whether that works**, against a live provider, before any store exists: are referrals
*useful* (do they point at people who actually hold something) or noise? If they are noise, the M5
fallback is a sim-side `who_keeps_that_word` line — the people this actor knows whose post or trade
covers a subject just named nearby, which leaks nothing because it is about roles, not about facts.
That is a fallback, not the plan.

## The merge rule

A person can hear the same fact twice: at four hops and garbled from a gossip, then at zero hops from
someone who was there. `Held` is one per `(actor, fact)`, so what happens on the second arrival has to
be decided, and it is the player's main verb:

| Arrival | Effect |
|---|---|
| Fewer hops than held | **Replaces** `view`, `from`, `hops`. `heat` takes the maximum. The story is corrected. |
| Equal hops | Keeps the held `view` — a person does not flip-flop between two equally distant versions. `heat` takes the maximum. |
| More hops | `heat` takes the maximum and nothing else changes. Corroboration warms a story; it never rewrites it. |
| Any arrival, when held at hops 0 | Ignored entirely. A witness cannot be talked out of what they saw. |

That table is why walking to a ward ahead of the pollen is worth doing. It is also what the STRANGER
child (`02_rumor_pollen.md`) stands on: you can beat your own garbled story to a ward and put the
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
  not a decision, so it is a sim-side rule and no verb. It lifts the fact to just above
  `VOLUNTEER_HEAT` and no further, so a revived story circulates without ever being as loud as fresh
  news. The approximation (the actor may have been asked and dodged) errs harmlessly: asking about
  something in public is itself how it gets going again. **This is what lets the player re-heat a
  cold rumour by asking about it** — a real verb with no verb attached, no prompt surface and no
  tokens, and exactly the poking-at-a-dead-story that the three quests are made of.
- A fact leaves the store **only by invalidation**, never by cooling.
- The store is bounded per actor (a low cap — a person is not a newspaper), evicting coldest first
  and highest-hops next. Views are deltas, so the bound is bytes, not sentences.

## Authoring: data, not code

Facts are data, following the crate's standing rule (`cathedral-sim/AGENTS.md`, "Data, not code").
The base game authors `assets/world/facts.json`; a quest authors its own pack and owns no Rust type:

```json
{ "facts": [
  { "id": "bale.promise",
    "topic": "word",
    "said": "Renn Crake promised a weaver a place on the Brede cart",
    "own": {
      "fr9ck": "I promised her bolt would go on Hugh's cart",
      "e5hob": "Renn Crake promised my bolt a place on that cart",
      "he3nd": "I carried that word to the bridge for a penny"
    },
    "subject": ["fr9ck", "e5hob"],
    "seeded":  ["fr9ck", "e5hob", "he3nd"],
    "decays": false,
    "garble": "none" },

  { "id": "bale.stop",
    "topic": "law",
    "said": "a cart was turned at the Wool Gate; the beam called forty pounds over",
    "minted_by": "quest.seizure",
    "garble": "place,day" }
]}
```

`bale.promise` is sealed to three people and never drifts. `bale.stop` is minted from the seizure
event, seeded with whoever was actually within earshot, and spreads as ordinary pollen with its place
and day free to go wrong.

Their topics are worth reading against each other, because they are the feature's own worked example.
`bale.stop` is `law` and crosses the city; `bale.promise` is `word` — base 0.15 — and goes almost
nowhere, which is precisely the quest: the city loudly knows the *accusation* and three people quietly
know the *arrangement*, and closing that gap is the player's job. The temptation to give
`bale.promise` a high band so it "reaches the player" is the one mistake this vocabulary exists to
make visible. A dull fact reaches the player by being **asked for** — relevance selection seats a
cold, dull, four-hop fact the instant its subject is mentioned nearby — not by being loud.

**Most of the base game's facts are minted, not authored.** That is deliberate: the city gossiping
about its own arrests, knells and market days is the content this feature is tuned on, and it costs
no authoring at all.

## Minting from events

The whitelist in `02_rumor_pollen.md` (custody commit, `raise_notice`, the knell, a big accepted
sale, a memorable stranger deed) is intercepted at `World::emit` (`world.rs:510`). A mint needs
nothing the event does not already carry:

| `Fact` field | Comes from |
|---|---|
| `seeded` | `DomainEvent::recipient_ids` — everyone actually within radius, players included |
| `place` | `position_m`, resolved through `areas.rs` |
| `subject` | `actor_id` / `target_id` |
| `day` | the world clock at emit |
| `said` | a per-kind template in `strings.toml` |
| `topic` | a constant per event kind, from the whitelist table itself |

Which means "who was there" is already answered correctly by the existing hearing calculation and is
not re-implemented.

## Where facts come from

Three routes, and only three. Every one of them ends at the same `Fact`, and the difference between
them is entirely in `source` and in how the `topic` is arrived at.

| | Route | Topic comes from | `source` |
|---|---|---|---|
| 1 | **Authored** — `assets/world/facts.json` and per-quest packs | the author | whatever the author binds it to |
| 2 | **Minted from an event** — the whitelist at `World::emit` | a constant per event kind | the event |
| 3 | **Coined by a mouth** — `raise_word`, M4 | **the speaker's own classification** | `FactSource::Claimed` |

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
enough to be saying a thing deposits their telling on every ladder poll, lowering the ward's `hops`
and becoming its `via` if they are closer to the source than the air is (`02_rumor_pollen.md`). A
verb for that would be a model declaring a side effect the sim performs for it every one to six
seconds anyway, and it would fire constantly for no observable result.

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

- **`source` is always `FactSource::Claimed(speaker)`.** Unforgeable, not a parameter, and it is what
  makes the thing invalidatable, walk-back-able and — since `source` is never rendered — never a leak.
  **A model can mint claims; it can never mint truths.** One invariant, one test that walks every
  action-reachable mint.
- **`seeded` is the speaker alone.** A claim cannot seat knowledge in other people's heads.
- **`decays: true`, always.** A claim is news and news cools. Nothing said aloud becomes a standing
  fact of the world.
- **`garble` is the default mask for its topic.** A claim cannot seal itself against drift; it goes
  wrong on the way round like everything else, including in the mouth of the person who made it up.
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
claim is just route 3 with the player as `via`, and telling them something they do not hold is
precisely the condition that puts the verb in front of them. And it acquires a rule that is far
better than the one it replaces:

> **The player cannot set the salience of their own lie.** They get whatever the mouth they told
> makes of it.

To spread something quickly, you must make it *sound like a scandal to the person you are telling* —
and to bury something, tell it to someone who will hear it as a trade matter. That is a real verb, it
is legible without a tutorial, and it falls straight out of putting the classifier in the listener's
head instead of in the speaker's.

## Invalidation

A fact can stop being true. `FactSource` is what lets the sim notice:

- a fact sourced on an item's location dies when the item moves;
- a fact sourced on a custody record dies on release;
- a fact sourced on a quest phase dies when the phase advances.

Dead facts are dropped from the store, which removes them from every sheet on the next turn — with no
`forget` verb, no LLM cooperation and no drift. Carriers who were holding it simply stop saying it.
(A deliberately *stale* rumour outliving its truth is a legitimate authored choice: set
`decays: true`, leave the source unbound, and the ward goes on saying a thing that is no longer so
until it cools.)

## What reaches the sheet

`what_you_know` is bounded — a small cap, smaller than `NOTICES_SHEET_MAX`'s 4 — and two rules fill
it, in this order:

1. **Relevance.** A fact whose subject, place or a distinctive noun appears in `since_your_last_turn`
   or `recent_history` is seated first, whatever its heat, including a faded one. Somebody asked; the
   answer must be on the sheet. "The hottest thing this actor carries" is a gossip rule and it is the
   wrong rule for an interrogation, which is what every quest here is made of.
2. **Heat.** What is left goes to the warmest, then fewest hops, then `FactId` — a total order, so
   the sheet is stable across runs and goldens.

A fact whose `subject` contains the actor is never rendered *to* them as news. They are not told
about themselves in the third person; if they hold it, they hold their `own` line or nothing.

## Facts the code reads (M5)

A held fact that only ever becomes dialogue dies the first time the model declines to voice it. A few
**systemic readings** make holding one real even on a turn nobody mentions it, and they are named here
so M5 is a list and not an open question:

- a bound vendor who holds a fact about the buyer refuses credit (`round.rs:6768` `try_purchase` is the only
  in-code credit path there is);
- a householder who holds one about the person at their door does not open it;
- a law officer carrying a garbled arrest fact may `raise_notice` on it at a new, explicitly lower
  hearsay rung (`notices.rs`) — so a swapped subject produces a wrongful summons the player can watch
  get raised, and settle;
- a greeting register: someone whose only knowledge of you is four hops out greets you as the story,
  not as the person.

Each is one `holds()` call at a decision the sim already makes. None of them needs a turn.

## The player's side

The player is a carrier like anyone else, and a receipt is what a carried fact looks like from the
outside:

```rust
pub struct LearnedHow {
    pub at: GameDays,
    pub place: Option<AreaId>,
    /// Who said it. `None` when the player witnessed it themselves.
    pub from: Option<ActorId>,
    pub hops: u8,
    /// How many separate mouths have now told the player this, and in how many
    /// wards. The cheapest possible way to make a wave visible.
    pub tellings: u16,
    pub wards: u8,
}

player_learned: BTreeMap<FactId, LearnedHow>
```

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
  fact they hold at zero hops because a designer wanted them to.
- **No objectives, ever.** Open threads render as the *questions they are* — "Whose cord is on that
  bale?" — never as instructions.
- **Provenance is shown.** A fourth-hand line is labelled as one. Being able to see that you are
  working from a garbled report is the point of the whole feature.
- **The count is shown.** "Three others since, in two wards" is what turns an invisible propagation
  model into something the player can play against — and it costs two integers.
- **Two standing lines at the top**, supplied by whatever is live — a quest supplies a clock ("the
  bale opens at Dayspring — one bell away") and a stake ("Hugh Crake is summoned to answer for it").
  The journal knows nothing about quests; it renders what it is given.

The last rule follows the game's own principle, already stated in `src/smart_actors/hud.rs` for the
law-standing line: *it must always name what would clear it — a brand with a visible door is a story,
a brand with no door is a bug.*

### The HUD

While a clock is live, one standing HUD line, not a toast — same reasoning, same precedent. A
deadline the player cannot see is not a deadline.

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
- **`raise_word` yields `FactSource::Claimed` and nothing else**, asserted by walking every
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
