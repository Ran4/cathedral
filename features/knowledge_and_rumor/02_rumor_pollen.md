# Rumor Pollen — the transport half

*News that travels at walking speed for zero marginal LLM calls.*

Was `features/rumors.md` until 2026-08-30, when it became the transport half of
`features/knowledge_and_rumor/`. `01_facts.md` is the other half: the proposition a token carries.
**Everything in this file is the design that ships.** The original pitch is not reproduced here —
what it got right is the design below, what it got wrong is listed immediately, and its actual text
is in git (`git show 9e61fc6:features/rumors.md`), where it cannot be mistaken for a plan.

## What the original pitch got wrong

1. **A token carries a `FactId` and a garble seed, not an inline `{kind, subject, place, day}`.**
   That is what lets authored quest knowledge — sealed, non-decaying, never garbled — ride the same
   rails as gossip without being gossip.
2. **Propagation is not general person-to-person proximity.** The pitch put pickup and hops in "the
   per-poll distance pass the scheduler already does". There is no such pass:
   `scheduler.rs` is the LLM turn scheduler, and the only proximity primitive the sim has —
   `World::neighbours_by_distance` (`world.rs:478`) — is a **linear scan over every character**.
   "Every carrier scans everyone within 25 m every poll" is O(carriers × N), and the shipped config
   is `extra_ambient_npcs: 1000` with the knob going to 20,000, where the pump is already 179 ms of a
   204 ms frame. The replacement is two layers, below.
3. **Cold tokens are not dropped**, they fade. See `01_facts.md`, "Cold is not forgotten".
4. **Walk the chain is not a child**, it is M3. Garbling without a way to check is the game lying to
   the player, and players correctly read that as a bug.

## Two layers, and why there are two

The wave has to be **cheap everywhere** and **granular where the player is standing**. Those are
different problems, so they get different mechanisms — the same split `attention.rs` already makes
for cognition, for the same reason.

### Layer 1 — the ward's air (city-wide, O(1) per person per poll)

The word sits in a ward, not on a person's skin. Eight wards tile the city
(`lore::PlanningWard::ALL`; position → ward is `crowd.rs:370` `nearest_ward` over
`homes::ward_marks()` at `homes.rs:94` — both crate-private today, and to be lifted rather than
reimplemented), so the whole city's weather is a few hundred entries.

```rust
/// The word in one ward's air: what is being said there, how loudly, at how
/// few removes, and by whose mouth it last got in.
pub struct Drift {
    pub heat: f32,
    /// The fewest hops any depositing carrier holds it at. A pickup lands at
    /// `hops + 1`.
    pub hops: u8,
    /// The last mouth to deposit at that hop count — the chain link a pickup
    /// records as its `from`, which is what keeps walk-the-chain walkable.
    pub via: Option<ActorId>,
    /// Bumped whenever the air is re-heated. One pickup roll per person per
    /// fact per stir — never a fresh draw per poll.
    pub stir: u32,
}

air: BTreeMap<(PlanningWard, FactId), Drift>
```

**Deposit and pickup both ride the round's ladder poll** (`round.rs:7400` `run_ladder`, gated per
person by `next_decision`, jittered `LADDER_DECISION_MIN_SECONDS..=MAX` — 1 to 6 s). That poll
already exists, is already per-actor throttled, and is already staggered by `leg_lag_share`, so hops
arrive scattered across an office instead of frame-synchronised. Per person per poll the work is one
ward lookup and a walk over the handful of facts in that ward's air:

- **Deposit.** A carrier warm enough to be saying it contributes their telling:
  `heat = max(heat, held.heat × talkativeness)`, `hops = min(hops, held.hops)`, and `via` becomes
  them if they lowered it. `stir += 1` when the heat actually rose.
- **Pickup.** For each fact in the air they do not already hold at `≤ air.hops + 1`, one
  deterministic roll — the `notices::carries` idiom (`notices.rs:419`), hashed over
  `(fact, actor, stir)` against `attention::curiosity_of` (`attention.rs:678`, the same curiosity
  `notices::carries` rolls against) scaled by `air.heat`. Hashed over `stir`, not over the clock, is
  what stops a 1-in-20 chance from becoming a certainty within a few seconds of polling; that is the
  lesson `attention.rs` learned the hard way. On success they take it at `air.hops + 1`,
  `from: air.via`, garbled per hop, merged by `01_facts.md`'s merge rule.
- **Cooling.** The air cools on the clock's game-hour edge — eight wards by a few facts, once an
  hour, not a per-person cost at all. A carrier's own heat cools on their own poll.

The word therefore crosses the city **in mouths**: somebody who holds it walks their round into the
next ward and deposits it there. That is the fiction, it is free, and it is why the cast's existing
commuting tide *is* the propagation model. Ward-adjacency seep exists as a knob and ships **off**:
air does not blow through walls, people carry it.

### Layer 2 — mouth to mouth, on stage (granular, only where it can be seen)

Inside the scan `attention::on_stage` (`attention.rs:242`) already runs around the player each poll,
carriers within speaking distance hop directly, one at a time, at `hops + 1`. This costs nothing new
— the scan is happening anyway — and it is what makes the wave legible as a wave: you can watch a
thing you said move down a street, person to person, instead of merely finding it already known one
ward over.

Expensive where it is visible, cheap where it is not. Same trade the whole engine makes.

### What ward granularity costs, and why it is still right

You cannot get one *street* ahead of the word — only one *ward*. Over an 840 × 700 m city with eight
wards, one ward is roughly the scale a player actually plays at, and Layer 2 restores the fine grain
in the only place a player could ever perceive it. The alternative buys street-level fidelity in the
empty half of the city, where nobody is looking, at a cost that scales with the crowd knob.

## Salience — what a fact is worth repeating

Layer 1's roll, as written above, is

```
hash(fact, actor, stir)  <  curiosity_of(actor) × air.heat
```

Two of those three terms vary. `actor` varies — some people gossip more, and `curiosity_of`
(`attention.rs:678`) is a real authored number. `heat` varies — some news is fresher. The `fact` term
supplies **hash entropy only**: it decorrelates the rolls so the same mouths do not pick up
everything, and it contributes nothing whatever to the probability. An adultery and a quarrel over a
stall pitch therefore cross the city at exactly the same speed, which is the one thing about gossip
that everybody already knows to be false.

So the roll gains a third term, and it is the only change to Layer 1:

```
hash(fact, actor, stir)  <  ( curiosity_of(actor) × air.heat × salience(fact, actor) ).clamp(0.0, 1.0)
```

### Salience is not heat, and the difference is the whole design

The cheap version of this is "mint the juicy one hotter, add nothing". That is wrong structurally,
not just in degree:

| | Answers | Decays | Varies with |
|---|---|---|---|
| `heat` | *Is this current?* | yes — per game hour, and per hop | time |
| `salience` | *Is this worth repeating at all?* | **never** | the fact's topic, and who is hearing it |

Mint a scandal at heat 1.0 and a squabble at 0.4 and the scandal spreads faster for one afternoon;
then they converge, and by "Cold is not forgotten" (`01_facts.md`) both settle to the same floor and
spread identically ever after. Real gossip does the opposite. A scandal is still worth repeating when
it is stale; a stall-pitch quarrel was not worth repeating when it was fresh. Two axes, multiplied,
and the sentence that falls out of the multiplication is the whole feature:

> **A cold scandal out-travels a fresh squabble.**

That is one line and it is a test (`01_facts.md`, test contract), not a feeling.

### The topic vocabulary

Salience is **not a number on a fact**. It is a closed set of topics, and the number lives in a
designer-owned table keyed by topic. That is deliberate, and it is the difference between a knob and
a tuning surface: a float per fact is five hundred floats nobody can reason about, whereas a topic is
a classification with an external check — you can read a fact and say whether it got tagged right.

```rust
/// What a fact is *about*. Fixed, small, and the only thing that decides how far
/// it will travel. A property of the proposition, so it is invariant across every
/// mouth that carries it: garbling moves the subject, the place and the day, and
/// never moves the topic.
pub enum Topic {
    Bed,      // who is with whom, and whose child is whose
    Blood,    // a death, a birth, a sickness, a beating
    Law,      // a seizure, a notice, a summons, the Stone House
    Omen,     // a sign, a bell rung wrong, a mark on a door, the rats
    Stranger, // the player, and anyone else the city has no place for
    Coin,     // a debt, a short measure, a refused credit, a beam that lied
    Bread,    // what there is to eat, and what it costs
    Craft,    // a trade dispute, a spoiled batch, a stall pitch, a bad joint
    Talk,     // a promise, a boast, a denial — who said what to whom
}
```

**The base band is derived, not chosen.** `1.00` is *defined* as the cadence target already stated
below — the number this feature was always going to be tuned to — so the top band is not a new
quantity and M2's measurement is not redone. Every other band is a stated fraction of it:

| Topic | Base | Why that fraction |
|---|---|---|
| `Bed` | 1.00 | The reference. Everyone has an opinion, nobody needs context, and the telling is its own reward. |
| `Blood` | 1.00 | A death concerns the whole ward practically — a post falls vacant, a debt is owed, a knell is counted. |
| `Law` | 0.80 | Travels hard, but it is *frightening* rather than delicious: some mouths shut on it, and the law already carries every notice unconditionally (`notices::carries`). |
| `Omen` | 0.80 | In this city, cheap to say and expensive to ignore. |
| `Stranger` | 0.80 | High and free of affinity: nobody's trade makes you more or less interesting. |
| `Coin` | 0.45 | Repeatable only to people it can happen to. |
| `Bread` | 0.35 | The **ordinary** case: a price is dull. Scarcity is not a hotter topic, it is a hotter *fact* — it mints at high heat and cools slowly, which is exactly what heat is for. |
| `Craft` | 0.20 | Almost all of this topic's reach is affinity, below: nothing to anyone but a cooper, everything to a cooper. |
| `Talk` | 0.15 | A promise is the dullest thing in the city and the hardest to chase. |

`Talk` at 0.15 is the band worth arguing about, so: **`bale.promise` — the hinge of an entire quest —
is a `Talk` fact, and it barely spreads at all.** That is not a flaw to be corrected by giving quest
facts a high band; it is *why that quest is hard*. Salience is a spread rate, never an importance
ranking, and the moment a designer reaches for a high band to make a quest fact reachable, the thing
they actually want is relevance selection (`01_facts.md`, "What reaches the sheet"), which already
seats a cold, dull, four-hop fact the instant somebody asks about it.

### Affinity — the same fact, a different ear

The base band is what the city thinks of a topic. `affinity` is what *this listener* thinks of it,
and it is one multiplier from named occupation sets living beside the bands — the idiom
`notices::LAW_OCCUPATIONS` (`notices.rs:71`), `attention::RESERVED_TRADES` and
`round::TRADE_OCCUPATIONS` (`round.rs:69`) already establish, so this is a fourth of a kind and not a
new pattern.

| Topic | Ear | × | Why these trades |
|---|---|---|---|
| `Bed` | `domestic_servant`, `laundress`, `tavern_worker`, `sex_worker`, `water_and_bath_worker` | 1.6 | Not a joke: these are the trades that are *inside other people's rooms*, and `domestic_servant` is the single commonest occupation in the cast (46 of them). The people who change the sheets know. |
| `Law` | `LAW_OCCUPATIONS` | 1.6 | The same instinct `notices::carries` already encodes absolutely, here in a weaker form — a fact is not a notice. |
| `Coin`, `Bread` | `market_seller`, `merchant`, `grocer_and_spicer`, `baker`, `fish_trader`, `revenue_worker` | 1.5 | It is their day. |
| `Craft` | the subject's **own** occupation | 2.0 | A spoiled batch. |
| `Craft` | every other occupation | 0.6 | Everyone else has their own batch to worry about. |
| *any* | **no occupation at all** | 1.4 | The no-trade quarter. |

That last row deletes a child. "The poor carry it furthest" was a someday-item at the bottom of this
file; the no-trade quarter already has no round, loiters where it was stood, and is twice as likely
to speak to you first (`AGENTS.md`, the crowd knob), and one table row now makes them hear everything
as well. *The beggars know everything before anyone* stops being a mechanism to build and becomes a
number in a file.

### Damping — a fact is quietest nearest the person it is about

The mirror of salience, and it costs one comparison:

- **The subject never picks it up.** They hold it at hops 0 because they were there, or they do not
  hold it at all. `01_facts.md` already rules that nobody is told about themselves in the third
  person; the store should not carry it either, rather than carrying it and hiding it.
- **Anyone sharing the subject's household door** (`homes.json`, the same door that is their
  `Townsperson.home`) picks up at **×0.15**.

The result is worth stating as the thing to go and look at, because it is the most human behaviour
in the feature and nobody wrote it directly:

> **The last people to hear a scandal are the ones who live with it.**

Which also makes *telling them* a scene — a thing the player can choose to do, or be the fourth
person that week to fail to.

### Where the numbers live

`assets/world/salience.json`: the nine base bands, and the affinity sets as lists of occupation ids.
Data, not code, per `cathedral-sim/AGENTS.md`. The topic *tag* is not in that file — it is on the
fact, and it arrives by one of the three routes in `01_facts.md`, "Where facts come from".

**The flat-table guarantee.** Set every band and every affinity to `1.0` and this whole section is
arithmetically the identity: the roll is `curiosity × heat` again and M2's measured cadence numbers
must reproduce byte-for-byte. Salience is therefore a provable refinement of a shipped model rather
than a replacement for one, and that identity is a test.

## The cadence band

Every knob above — pickup probability, deposit threshold, cooling rate, hop damping — is **derived
from one number**, not tuned toward a feeling. Salience turns that number into a band, because one
speed for all news was the flaw, and a target that cannot express "and the dull one never arrives"
cannot be the target of a system whose point is that some things travel and some do not.

> **The fast end.** A `Bed` or `Blood` fact minted at the Wickmarket is being said in the Weigh Ward
> within about one office, and has reached every ward inside a game day.
>
> **The slow end.** A `Craft` fact minted beside it, at the same hour, by the same mouth, is still in
> its own ward at nightfall — and may never leave it at all.

The fast end is the original target, unchanged and un-retuned: base `1.00` is *defined* as that
number. The slow end is what `0.20 × 0.6` does to it, and it is a measurement rather than a hope —
"may never leave" means the pickup roll's expected crossings over a game day is below one, which is a
computation, not an observation.

At the shipped `seconds_per_day: 3600` an office is 2–5 game hours, so the fast end is 5 to 12 real
minutes. Faster and nothing can ever be outrun; much slower and nothing perceptibly moves in a
session. The measurement is the headless carriers-per-ward-per-game-hour print, run at
`--extra-ambient 0`, `1000` and `20000`, **now printed per topic** — a test, not an eyeball. The
flat-table run (every band `1.0`) reproduces the pre-salience numbers exactly and is the regression
guard on all of it.

## Garbling and the chain

Deterministic, seeded per `(fact sequence, carrier id, hops)`, bounded to a fixed vocabulary and
never inventing a person (`no-procedural-characters` holds):

- **subject** → another named actor of the same ward or trade;
- **place** → an adjacent area;
- **day** → ±1.

Which fields may move is the fact's own `GarbleMask`; the rest are load-bearing truth. Because the
roll is a pure function of the seed and the view is stored as deltas (`01_facts.md`, `FactView`), the
transmission chain is **reconstructible rather than logged**: follow `Held::from` back through
`Drift::via` and each link's garble can be recomputed to show exactly where the story turned. That is
"who told you that?" for the player, for a sergeant, and for a test — an implementation choice made
for determinism, spent as investigation gameplay.

### Hedge erosion — salience drifts the *telling*, not only the facts

Garbling as described above only ever **misremembers**. Real gossip does something else on the way
round, and it is the single most recognisable thing about the saucy kind: it **loses its hedges**. At
four hops from a stall quarrel a person says "I had it from someone who had it from Ilse"; at four
hops from a scandal they say it flat, as a thing that happened.

The hop-keyed phrasing ladder (`README.md`, "Prompt surface") is a `strings.toml` lookup on hop
count. Salience shortens the ladder, and that is the entire mechanism:

| | hops 1 | hops 2 | hops 4 |
|---|---|---|---|
| `Talk`, `Craft` (low band) | "they say" | "third-hand" | "third-hand" |
| `Bed`, `Blood` (top band) | plain assertion | "they say" | "they say" |

No new state, no new field, one table widened by a column, and it applies to **every** fact — unlike
magnitude drift (below), which needs a fact to carry a number and most do not. It is also the
cheapest available answer to risk 2 (perceptibility): the player meets someone four hops from a
scandal stating it as fact and someone one hop from a squabble hedging it, and *hears* the difference
between the two kinds of news without a single system being explained to them.

The failure direction is right, too. Over-eroded, a dull fact is stated too confidently — a small
lie. Under-eroded, a scandal is hedged — merely stilted. Neither is a bug the player reads as one.

## The player as a source (M4)

The player becomes a carrier by hearing, like anyone else. They become a **source** by talking: a
hearer of player speech may **`raise_word { topic, said }`** on their reply turn (`01_facts.md`,
"Where facts come from"), coining a claim out of what the player just said, at `hops = 0` at their
feet with the player as `via`.

Telling somebody a thing they do not hold *is* the precondition that puts that verb in front of them,
so the toy below is not a hope about model behaviour — it is the gate firing. Repeating something
already in the air needs no verb from anybody: carriers deposit automatically on their own ladder
poll, and a cold thing warms again when somebody asks about it.

This is also the design's best toy, and it should be built as one rather than left implicit: **what
the player says need not be true.** Say a false thing to a credulous mouth and it enters the air with
you at the head of its chain, garbles on its way around the city, and comes back to you three days
later with your name filed off — and anyone who thinks to ask "who told you that?" can walk it back
to you. It is the same mint path as everything else; the only thing needed is not to prevent it.

And because the hearer supplies the topic, **the player cannot set the salience of their own lie** —
they get whatever the mouth they told makes of it. Tell a laundress something that sounds like `Bed`
and it is round the city by nightfall; tell a cooper the same words and they hear a `Craft` matter
and it dies in the lane. That is a genuine verb, learnable without being taught, and it exists only
because the classifier sits in the listener's head rather than the speaker's.

It is also what makes the ignorance rule's last sentence literally true rather than rhetorical: *a
guess said aloud becomes what the ward believes.*

## The whitelist

Intercepted at `World::emit` (`world.rs:510`). Two kinds in M2, the rest in M5:

| Event | The fact it mints | Topic | Garbles |
|---|---|---|---|
| custody commit | "X was taken at Y, on day D" | `Law` | subject, place, day |
| the knell | "someone was buried out of Saint Maren's, N years old" | `Blood` | subject, day |
| `raise_notice` | the notice's own words, as hearsay | `Law` | subject, place |
| a large accepted sale | "X bought/sold Y at Z" | `Coin` | subject, place |
| a memorable stranger deed | "the stranger did X at Y" — the STRANGER token | `Stranger` | place, day |
| a civic peal | re-heats matching air within earshot rather than minting | — | — |

**A coded mint needs no classifier.** The topic is a constant per event kind, in this table and
nowhere else: a seizure is `Law` because seizures are, not because anything looked at one. The only
mint that has to *decide* a topic is the one a mouth makes (`01_facts.md`, `raise_word`), and that is
the whole reason the vocabulary is closed.

## Children

- **Hearsay rung** — a sergeant carrying arrest or wrong pollen may `raise_notice` on its strength at
  a new, explicitly lower rung in `notices.rs`, so a garbled subject produces a wrongful summons the
  player can watch get raised, and settle. Makes garbling consequential to the law system rather than
  cosmetic dialogue colour. **Scheduled: M5.**
- **The STRANGER token** — everything memorable the player does mints pollen about the player, so far
  wards greet you with a garbled second-hand version of yourself; and because you walk faster than
  pollen crosses wards, you can beat your own story to a ward and pre-empt it with your account
  (`01_facts.md`, the merge rule, is what makes the pre-emption stick). Emergent reputation with no
  reputation system. **Scheduled: M4–M5.**
- **Night Office settlement** — include the ward's hottest air in the already-running curfew batch
  prompt so it shapes the returned Minor mood, and let a Major's nightly reflection settle a rumour
  into permanent memory — pollen graduating into canon, on prompts already paid for.
- **Bells as amplifiers** — a civic peal re-heats matching air within earshot (the knell re-heats the
  death token; a summons re-mints the newest ward notice as fresh air at the Bellstand), so acoustics
  physically extend rumour range, and the false-bell prank gains a visible epistemic consequence.
  **Scheduled: M5** — it is small, it is already designed, and it is the most *city* thing on this
  list.
- **Walk the chain** — **no longer a child. It is M3.** See above.
- **The poor carry it furthest** — **no longer a child.** It is one row of the affinity table above
  (no occupation at all, ×1.4 on every topic), which is what a someday-item looks like once the
  mechanism it needed exists.
- **Magnitude drift** — the other half of embellishment: forty pounds over becomes a hundred, three
  days becomes a week, a debt grows on the way round. A signed step in `FactView` beside
  `day_offset`, biased **upward** in proportion to salience rather than drifting symmetrically,
  because that is the direction gossip actually errs in. Deliberately *not* M3: it needs a fact to
  carry a number, and most facts do not, so it buys embellishment for a minority of the store where
  hedge erosion buys it for all of it. Worth doing after — the bale quest is made of a number that
  ought to grow in the telling.
- **Sincerity** — `raise_word` is neutral about whether the speaker believes themselves, which is
  right for the verb and leaves something on the table: a claim raised by somebody who knows it is
  false could carry a private flag the sim reads (they do not repeat it themselves, they deny it if
  asked twice, they watch to see whether it took). None of that is dialogue; all of it is `holds()`
  at a decision the sim already makes, in the shape of "Facts the code reads".

---

## Where this came from

`features/rumors.md`, written earlier in 2026 as an independent nice-to-have and moved here on
2026-08-30 (`80a0ea4`) when three quest specs turned out to need a knowledge layer. Its text is at
`git show 9e61fc6:features/rumors.md` if the archaeology is ever wanted; it is deliberately not
quoted here, because a superseded design sitting under a heading in the live spec is read as an
alternative plan sooner or later.

What it got right, and what this file therefore still is:

- **The core bet** — that news can cross the city in pure Rust, on turns that were already going to
  happen, for zero marginal LLM calls. Everything above is still that.
- **Minting from a `DomainEvent` whitelist at `World::emit`**, using `recipient_ids` as the hops-0
  set, so "who was there" is never re-implemented.
- **Deterministic garbling seeded per hop**, bounded to a fixed vocabulary and never inventing a
  person — and the observation that this makes the transmission chain reconstructible, which is now
  M3 rather than a someday-child.
- **Provenance hedging that degrades with hop count**, and the player as a carrier who can outrun
  their own story.
- **Its stated load-bearing risk**: propagation must never ride an LLM turn, because `attention.rs`
  gates idle cognition to the player's neighbourhood and a cognition-driven hop freezes the rumour
  field everywhere the player is not. That still holds and is why both layers above are pure Rust.
- **Its mirror risk** — perceptibility without parroting — also still holds. The README adds a third
  that outranks both: an actor who holds nothing must *say* so, or holding a fact and not holding
  one look the same from the player's chair.

What it got wrong is the four items at the top of this file, and the reason it was wrong is worth
keeping: it was written against a mental model of the sim rather than against the code, so it cited
a distance pass the scheduler does not have and costed proximity hops as if a spatial index existed.
