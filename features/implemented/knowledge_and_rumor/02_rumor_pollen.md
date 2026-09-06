# Rumor Pollen — the transport half

*News that travels at walking speed for zero marginal LLM calls.*

Was `features/rumors.md` until 2026-08-30, when it became the transport half of
`features/implemented/knowledge_and_rumor/`. `01_facts.md` is the other half: the proposition a token carries.
**Everything in this file is the design that ships.** The original pitch is not reproduced here —
what it got right is the design below, what it got wrong is listed immediately, and its actual text
is in git (`git show 9e61fc6:features/rumors.md`), where it cannot be mistaken for a plan.

## What the original pitch got wrong

1. **A telling carries an interned `FactKey` and compact held deltas, rather than a copy of the proposition.**
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

### Layer 1 — the ward's air (city-wide, bounded work per due person)

Eight `PlanningWard`s tile the city. `World::ward_at` delegates to the shared
`knowledge::pollen::ward_grid()`: 8 m cells over the walkable box, with an exact nearest-mark fallback
for ambiguous cells and points outside it. `crowd::ward_map` supplies the same marks used by crowd
placement. There is no second `nearest_ward` implementation. Named event places still come from
`AreaMap`, whose irregular named areas do not tile the city.

`Knowledge` keeps an `Arc<BTreeMap<(PlanningWard, FactKey), Drift>>`; `Drift` carries heat, fewest
hops, the depositing mouth (`via`) and the stable stir seed. The Arc preserves cheap transactional
world clones; mutations use copy-on-write. There are at most 24 rows per ward and 256 live facts.

**The whole cast runs `round::tick_pollen` immediately after `decay_needs` in `round::tick`.** It does
not ride `run_ladder`, whose attention and activity gates would freeze knowledge away from the
player. Per-person deadlines live in game days, jittered around ten game minutes with a fifteen
minute ceiling. A due queue visits due people, rather than walking every deadline every tick. A
listener is resolved once per poll; the inner air-row loop does not reload their lore each time.

- **Deposit:** a warm carrier contributes `max(air.heat, held.heat(now))`; a closer holder lowers
  air hops and supplies its upstream mouth. Hop loss is charged once when a telling is learned,
  never again as a talkativeness multiplier on deposit. Quantized heat comparisons and the stir
  schedule keep unchanged deposits from offering fresh random chances.
- **Pickup:** for an eligible held distance, the deterministic hash of `(fact sequence, actor,
  stir)` is compared with `curiosity × air.heat × salience`, clamped to 0…1. Success learns
  `air.hops + 1`, `from = air.via`, heat multiplied once by `HOP_LOSS`, and a deterministically
  garbled view. The same stir produces the same roll on every intervening poll.
- **Cooling:** air is swept at the stir edge. A holding's heat is derived from its learning time,
  so cooling costs no per-carrier mutation pass. Stale source invalidation runs on this sweep too.
- **Standing facts:** authored non-decaying seeds never volunteer or auto-deposit. A relevant
  speaking turn can lift a local air row to `REHEAT_TO`; that air and any carried override then
  cool normally. The seed's durable knowledge remains answerable.

People carry the word across ward boundaries on their ordinary rounds. **Adjacency seep is
declined entirely**: there is no dormant seep knob. The authored-seed lookup is bounded by the
256-fact cap; it is not literally constant independent of store size. The M2 measured cost did not
justify the proposed `seeded_by_actor` reverse index.

### Layer 2 — mouth to mouth, on stage

Layer 2 owns a separate `characters_within` scan around the player, gated to once every **two real
seconds** in both Stage and All attention modes. `attention::on_stage` is left alone: it excludes
occupied people and is not a valid roster for speech transport. This scan costs O(N); at 20,000
bodies its gate allows about 10,000 distance tests per second.

The nearest eight carriers, plus the player, exchange eligible held words. A carrier and listener
must also be within **inclusive 20 m of each other in 3D**; being separately close to the player is
insufficient. The M3 review fixed the case where two people 19 m on opposite sides of the player
could otherwise exchange over 38 m. Pair checks use the already selected positions, with no second
city scan. The carried-only bound is 9 × 8 × 6 = 432 candidates per beat, but authored seeds are
exempt from six holdings: the absolute bound is 9 × 8 × 256 = 18,432. It adds no probability or
cadence retune. Repeated successful player pairs are receipt-deduplicated within their game stir.

This restores a visible chain of mouths where the player stands. Everywhere else the ward-air
model carries the wave without another LLM call.

### What ward granularity costs, and why it is still right

You cannot get one *street* ahead of the word — only one *ward*. Over an 840 × 700 m city with eight
wards, one ward is roughly the scale a player actually plays at, and Layer 2 restores the fine grain
in the only place a player could ever perceive it. The alternative buys street-level fidelity in the
empty half of the city, where nobody is looking, at a cost that scales with the crowd knob.

## Salience — what a fact is worth repeating

Without salience the deterministic hash would compare against `curiosity × air.heat`, and an
adultery and a quarrel over a stall pitch would travel at one speed. The shipped roll is:

```
hash(fact sequence, actor, stir) < (curiosity_of(actor) × air.heat × salience(fact, actor)).clamp(0.0, 1.0)
```

The subject exemption is outside that product in `may_carry`, so multiplying by zero can never
accidentally admit a subject. `salience` is base band × listener affinity × frozen household
damping. The player has curiosity 0.35 and affinity exactly 1.0; no missing-lore trade bonus applies.

### Salience is not heat, and the difference is the whole design

The cheap version of this is "mint the juicy one hotter, add nothing". That is wrong structurally,
not just in degree:

| | Answers | Decays | Varies with |
|---|---|---|---|
| `heat` | *Is this current?* | yes — per game hour, and per hop | time |
| `salience` | *Is this worth repeating at all?* | **never** | the fact's topic, and who is hearing it |

Mint a scandal at heat 1.0 and a squabble at 0.4 and the scandal spreads faster for one afternoon;
then their relative heat alone still cannot express a permanent difference in what a listener cares
about. Cold holdings remain answerable, but there is no permanent warm floor that makes them spread
forever. Real gossip does the opposite. A scandal is still worth repeating when
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
| `Bed` | `domestic_servant`, `laundress`, `tavern_worker`, `sex_worker`, `water_and_bath_worker` | 1.6 | Not a joke: these are the trades that are *inside other people's rooms*, and `domestic_servant` is the single commonest occupation in the measured cast (45 of them). The people who change the sheets know. |
| `Law` | `LAW_OCCUPATIONS` | 1.6 | The same instinct `notices::carries` already encodes absolutely, here in a weaker form — a fact is not a notice. |
| `Coin`, `Bread` | `market_seller`, `merchant`, `grocer_and_spicer`, `baker`, `fish_trader`, `revenue_worker` | 1.5 | It is their day. |
| `Craft` | the subject's **own** occupation | 2.0 | A spoiled batch. |
| `Craft` | every other occupation | 0.6 | Everyone else has their own batch to worry about. |
| *any* | **no occupation at all** | 1.4 | The no-trade quarter. |

The measured ear populations are 77 for `Bed`, 63 for `Law`, 42 for `Coin`/`Bread`, and 10 without a
trade. The no-trade and domestic-servant curiosity increases already exist in `derived_curiosity`;
the additional salience affinity deliberately stacks with them and is documented in
`assets/world/salience.json`. `no_fixed_trade` is a directory, not a valid occupation ID.

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
- **The subject’s kin and anyone sharing their household door** pick up at **×0.15**.
  `quiet_among` is frozen at mint: lore father/mother/children and `Townsperson.home` are seed-time
  inputs, never runtime writes. The 0.5 m equality tolerance is below the 1.2748 m minimum between
  distinct authored doors; door sharing therefore mainly matters for generated citizens. A future
  moving-house feature must recompute frozen household membership on that move.

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
must reproduce field for field. `flat()` leaves authored hedge bands unchanged, so the identity
comparison moves numbers without changing prose. Salience is therefore a provable refinement of a shipped model rather
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

The band is defined at the shipped **`seconds_per_day: 3600`** and measured at a **0.4 s walking
step**; a faster clock or the 3 s cost-guard step changes how much of a commute is realised. Its
assertions apply to the authored cast at `--extra-ambient 0`. At 1,000 and 20,000 extra bodies the
census is reported and cost/caps are guarded; the generated cast changes the measured input.

`VOLUNTEER_HEAT = 0.119` was solved from the nearest foreign-ward boundary, 82 m from the Wickmarket:
`0.12 / 2^(0.26/12) = 0.1182`, rounded upward. An off-affinity Craft witness therefore cools below
volunteering after 0.145 game hours, before reaching that boundary. The realised one-ward result is
also asserted, so the arithmetic has a walking/deposit backstop. `STIRS_PER_GAME_HOUR = 2` is the
fast-end free parameter; the bands and affinities are fixed.

M2 corrected the original office-timed prediction: the mint is at the Dayspring bell, so coin-zero
pickups join the commute immediately. Its Bed wave reached all eight wards at **2 game hours**;
a two-sided guard additionally requires fewer than eight at 1 hour. Craft remained in one ward all
day, with measured expected crossings 0.136/day and nightfall confinement probability 0.799, rather
than the draft's 0.057/0.945. `plan/02_numbers.md`'s appended corrections and `m2_measurements.md`
retain the derivations. Final M5 measurements passed without a retune and are recorded in
`m5_measurements.md` and the README's Numbers section.

Every census retains per-ward figures: standing populations are BellAndSluice 193, Weigh 63, Reed 63,
Wick 53, Fabric 45, Cloth 45, Wallwright 27 and Cinder 26. A city mean would hide that 7.4:1 range.

## Garbling and the chain

Deterministic, seeded per `(fact sequence, carrier id, hops)`, bounded to a fixed vocabulary and
never inventing a person (`no-procedural-characters` holds):

- **subject** → a real authored actor of the same lore ward or trade, from a stable 24-person
  candidate pool; the listener need not know their name, so `person_word` may render their role;
- **place** → an adjacent area;
- **day** → ±1 per garbled hop, clamped to ±3 days. The per-field per-hop chance is 0.35.

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

The shipped phrasing is the **three-band, seven-rung** ladder measured in M0b and frozen in
`assets/prompts/strings.toml`; it replaced the draft's coarse “third-hand” table. Selection depends
on hops, current heat and the fact's authored hedge band. `SalienceTable::flat()` preserves that
band, so an ablation does not accidentally measure different prose.

M3 measured the actual garbled renderer with both providers: both produced 8/8 distinct ward replies
under the recorded clause-and-slot rule. Top-band hedge erosion remained partial, and one Moonshot
reply accepted the question's person despite an unknown-role garble on its sheet. Those limits and
every raw response remain in `m0_evidence/NOTES.md`; the result is not a promise of perfect model
fidelity. No M5 prompt rewrite or golden re-bless is owed.

## The player as a source (M4)

The player becomes a carrier by hearing, like anyone else. They become a **source** by talking: a
hearer of player speech may **`raise_word { topic, said }`** on their reply turn (`01_facts.md`,
"Where facts come from"), coining a claim out of what the player just said, at `hops = 0` at their
feet. The raiser’s holding links to the player; the new air names the raiser as `via`, so a pickup
walks back through that mouth to the original speaker.

Telling somebody a thing they do not hold *is* the precondition that puts that verb in front of them,
so the toy below is not a hope about model behaviour — it is the gate firing. Repeating something
already in the air needs no verb from anybody: carriers deposit automatically on their own pollen
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

Explicit successful-operation hooks, never a `World::emit` interceptor. Every coded mint computes
its own earshot instead of trusting `DomainEvent::recipient_ids`. Templates and topic/mask choices
live together in `knowledge/mint.rs::MINT_KINDS`.

| Cause | What is established | Topic | Garbles |
|---|---|---|---|
| custody commit | somebody was taken at a place and day | `Law` | subject, place, day |
| `raise_notice` | an accusation was raised about somebody | `Law` | subject, place |
| knell | Saint Maren’s burial count at the rung age | `Blood` | **day only**; subject is empty |
| player `draw_mark` | the stranger chalked a mark | `Stranger` | place, day |
| player `scrub_mark` | the stranger scrubbed a mark | `Stranger` | place, day |
| Scold’s curfew or summons | warms matching Law air within the supplied bell carry | no mint | — |

The notice row uses fixed accusation-exists prose and an event source. The model’s deed is interpolated within that fixed
accusation-exists template; it does not become a fact that the deed was done. Stranger rows are
player-only, use fixed templates and record the player's witnessed act without allowing them to
pick up their own story as news. There is no generic “memorable” free-text classifier.

`EngineCommand::Knell { years, at }` is the missing host→sim seam, with a 300 m carry pinned to the
actual Smallvoice clip by a host test. It mints the new count and amplifies older `Blood` air.
`EngineCommand::CivicPeal { rope, at, radius_m }` only amplifies; both Curfew and Summons map to
`Law` in one exhaustive `peal_topic` function. A second peal that raises no quantized heat grants no
extra stir. Matching wards come from 8 m cell centres inside the bell’s planar carry, plus its own ward;
centroids are used for map placement only, never as the acoustic coverage test.

The large accepted-sale row is **declined**: no price in the emitted event, empty recipients, and a
staged transaction clone that would discard a premature mint on failure (D32). There is no new
salience probability in M5: consequences are predicates, amplification is a guarded maximum.

`EngineMessage::WardHeat` publishes all eight centroid rows, whole-percent heat and bounded word
counts, only when the projection changes. Eight heat dots render **under the player marker on the
fullscreen map only**; zero-heat dots are hidden. The host changes its resource only on WardHeat
messages, so ordinary Clock/Weather traffic does not dirty map layout. Journal mouth/ward counts
and the standing HUD share the separately bounded knowledge projection, outside `PublicSnapshot`.

## Children

Hearsay summons, the player’s drawn/scrubbed stranger deeds, greeting relevance, and bell amplifiers
are shipped in M5. Walking the chain shipped in M3; the no-trade ×1.4 ear shipped in M2. They are no
longer future feature slots.

- **Additional stranger deeds** remain an authoring seam: extend `STRANGER_DEED_KINDS` one
  successful event at a time, with its fixed template and explicit reason. The two shipped mark
  deeds do not classify every memorable act.
- **Night Office settlement** remains a future integration: use the existing curfew batch/reflection
  prompts to let current ward words shape mood or become memory. No such prompt runs in this feature.
- **Magnitude drift** remains future work: a numerical field beside `day_offset` could let forty
  pounds become a hundred. Most current facts have no number slot, and adding one is outside M3/M5.
- **Sincerity** remains future work: a private distinction between believing and knowingly coining a
  false claim could drive denial/repetition behavior. `raise_word` currently makes no such judgement.

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
- **Minting from a small event whitelist**. The shipped hooks use actual local earshot; the old
  `World::emit`/`recipient_ids` proposal was corrected because event delivery is not witnessing.
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
