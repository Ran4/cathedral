# Facts — the held half

The proposition, who holds it at first hand, how it renders, and what the player has learned.
`02_rumor_pollen.md` is how it travels to everyone else.

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

    /// Why it is true. **Never rendered anywhere** — not in a prompt, a
    /// projection, a log line or the journal. This is the anti-leak field:
    /// it is how a fact gets invalidated when the world changes under it,
    /// and it is where a quest keeps the thing the player is trying to find out.
    pub source: FactSource,
}

/// What one person currently has.
pub struct Held {
    pub hops: u8,
    pub heat: f32,
    /// The fact as *this* person has it, after per-hop garbling.
    pub view: FactView,
}

/// None means they have never heard of it at all.
pub fn holds(world: &World, actor: &ActorId, fact: &FactId) -> Option<Held>;
```

`holds` checks `seeded` first (hops 0, heat 1, no garble), then the pollen store. Every consumer —
the prompt block, the journal, a quest — goes through that one call and nothing else.

### Why `own` and `said` are separate

It is the cheapest available answer to this feature's principal risk. If a fact has one string, every
holder says the same sentence and a ward of parrots is guaranteed by construction. With the split,
Renn holds *"I promised her bolt would go on Hugh's cart"*, Ede holds *"I carried that word to the
bridge for a penny"*, and a Weigh-ward gossip four hops out holds *"they say some weaver's cloth was
in that bale"* — three different sentences from one authored fact, before garbling has done anything
at all.

### Why `source` is never rendered

A fact says *what*. `FactSource` says *why it is true*, and in a quest that is usually the answer the
player is looking for. Keeping them in one struct but rendering only one of them is what stops a
knowledge system from being a spoiler pipe. The rule is absolute and testable: `FactSource` appears
in no prompt, no `EngineMessage`, no snapshot, no log line and no journal entry.

## Authoring: data, not code

Facts are data, following the crate's standing rule (`cathedral-sim/AGENTS.md`, "Data, not code").
A quest authors JSON and owns no Rust type:

```json
{ "facts": [
  { "id": "bale.promise",
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
    "said": "a cart was turned at the Wool Gate; the beam called forty pounds over",
    "minted_by": "quest.seizure",
    "garble": "place,day" }
]}
```

`bale.promise` is sealed to three people and never drifts. `bale.stop` is minted from the seizure
event, seeded with whoever was actually within earshot, and spreads as ordinary pollen with its
place and day free to go wrong.

The base game authors facts the same way, and most of its facts are minted rather than authored.

## Minting from events

The whitelist in `02_rumor_pollen.md` (custody commit, `raise_notice`, the knell, a big accepted
sale, a memorable stranger deed) is intercepted at `World::emit`. A mint needs nothing the event does
not already carry:

| `Fact` field | Comes from |
|---|---|
| `seeded` | `DomainEvent::recipient_ids` — everyone actually within radius, players included |
| `place` | `position_m`, resolved through `areas.rs` |
| `subject` | `actor_id` / `target_id` |
| `day` | the world clock at emit |
| `said` | a per-kind template in `strings.toml` |

Which means the "who was there" question is already answered correctly by the existing hearing
calculation, and is not re-implemented.

## Invalidation

A fact can stop being true. `FactSource` is what lets the sim notice:

- a fact sourced on an item's location dies when the item moves;
- a fact sourced on a custody record dies on release;
- a fact sourced on a quest phase dies when the phase advances.

Dead facts are dropped from the store, which removes them from every sheet on the next turn — with
no `forget` verb, no LLM cooperation and no drift. Carriers who were holding it simply stop saying
it. (A deliberately *stale* rumour outliving its truth is a legitimate authored choice: set
`decays: true` and leave the source unbound, and the ward goes on saying a thing that is no longer
so until it cools.)

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
}

player_learned: BTreeMap<FactId, LearnedHow>
```

`from` and `hops` are the journal entry *and* the "who told you that?" chain — the same field serves
provenance display and `02_rumor_pollen.md`'s **walk the chain** child. A player who was told
something wrong can trace it back to the mouth it went wrong in.

### The journal (J)

An overlay on the inventory overlay's pattern (`src/smart_actors/inventory_ui.rs`), because that
interaction already works and players already know it.

It renders `player_learned`, newest first, **as the sentence the player heard, attributed**:

> *Warin Underbridge, at the porter stand, this morning:*
> "she went heavy, and I said nothing."

Rules, which are the difference between a journal and a hint system:

- **Only what the player heard or caused.** Never the authored truth, never `FactSource`, never a
  fact they hold at zero hops because a designer wanted them to.
- **No objectives, ever.** Open threads render as the *questions they are* — "Whose cord is on that
  bale?" — never as instructions.
- **Provenance is shown.** A fourth-hand line is labelled as one. Being able to see that you are
  working from a garbled report is the point.
- **Two standing lines at the top**, supplied by whatever is live — a quest supplies a clock ("the
  bale opens at Dayspring — one bell away") and a stake ("Hugh Crake is summoned to answer for
  it"). The journal knows nothing about quests; it renders what it is given.

The last rule follows the game's own principle, already stated in `src/smart_actors/hud.rs:373` for
the law-standing line: *it must always name what would clear it — a brand with a visible door is a
story, a brand with no door is a bug.*

### The HUD

While a clock is live, one standing HUD line, not a toast — same reasoning, same precedent. A
deadline the player cannot see is not a deadline.

## Test contract

- `holds()` is pure and deterministic; every roll is a hash of stable inputs.
- A `seeded` holder's view is byte-identical across runs and never garbled.
- `FactSource` appears in no rendered string: asserted by a test that walks every projection.
- Dropping a fact removes it from every sheet on the next turn with no actor cooperation.
- With no facts in the world, golden prompts are byte-identical to the M0 bless.
- Facts never enter `PublicSnapshot` (the size canary still passes).
