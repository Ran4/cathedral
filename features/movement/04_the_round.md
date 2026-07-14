# L3 — The Round: where people are supposed to be

The ladder ([03](03_the_ladder.md)) decides what a person does *now*. The Round is what they do when
nothing is wrong: their day. It is rung 9 of the ladder — the default, the thing that runs when nobody
is thirsty, tired or afraid.

And most of it is already written.

---

## 1. The schema was discovered, not invented

`lore/second_sun/05_dramatis_personae.md` gives each of the twenty most important characters a
`route:` line. Not one of them was written with a movement system in mind, and all twenty have the
same shape:

> **Praelucent Havise Dorn** — *prime office in the nave at dawn; chapter-house audiences till noon;
> the crossing at the strong hour*
>
> **Warden Hamel Stott** — *the Fabric yard at first light; the lodge after*
>
> **Mistress Jonet Sparr** — *furnace lit before dawn; workshop all day; the Lanthorn only when the
> Fabric summons, never alone*
>
> **Verger Dunstan Pike** — *unbars the west doors for the dawn-showing; rounds all day; locks at
> dusk; then the Bell and Ladle*
>
> **Crier Jos Brant** — *cries at set hours; posts edicts; the Bell and Ladle by evening*
>
> **Renna Tapster** — *taps open when Brant first cries; counter until the last argument goes home*
>
> **Wyn Alder** — *the Moorings yard before dawn; the fish market; walking pilgrims to the Old Sluice
> for a penny; the nave on Bellday*
>
> **Mother Gude** — *simples from the south-wall herb plots at first light; the stall through market
> hours; never inside the Lanthorn since she looked*
>
> **Dame Aldith** — *none; the city routes itself past her squint*

Two to four legs. Each leg pegged to an **office** (*at dawn*, *till noon*, *at the strong hour*, *by
evening*) or to a **market day** (*stall on Highmarket*, *salt at Lowmarket*, *the nave on Bellday*).
Each leg naming a **place that already exists in `areas.json`**.

That is the data structure:

```rust
/// One person's day. Small, cheap, and authored or derived — never planned.
pub struct Round {
    pub home: Option<Anchor>,       // None for the 18 `unhoused`
    pub workplace: Option<Anchor>,
    pub legs: Vec<Leg>,             // 2–4, ordered
    /// How far they will drift from the current leg's anchor when idle.
    pub leash_m: f32,
}

pub struct Leg {
    /// The office at which this leg begins. The leg runs until the next one.
    pub from: Office,
    pub place: AreaId,
    pub doing: Arrival,             // Work | Trade | Pray | Sleep | …
    /// `None` = every day. Otherwise only on these.
    pub only_on: Option<Vec<Weekday>>,
}

/// Where "at the Wickmarket" actually puts your feet.
pub enum Anchor {
    Area(AreaId),          // stand anywhere in it
    Door(BuildingId),      // a house — go in
    Point(Vec3),           // a stall, a workbench, a curb
}
```

Dame Aldith is an anchoress with `route: none` — she is bricked into a wall. **Her Round has zero
legs and it works**, which is a good sign that the schema is the right shape: the degenerate case is
degenerate rather than special.

---

## 2. Where the Round comes from — three sources, in order

### (a) Authored, for the 20

Transcribe the twenty `route:` lines by hand into `assets/world/rounds.json`. It is an afternoon's
work and it is the highest-value content in the whole feature, because these are the twenty people the
player will actually talk to.

One join to be careful of: `05_dramatis_personae.md` identifies people by **slug** (`havise-dorn`),
while the character sheets use a **5-char id** (`ak3vd`, in
`lore/characters/candor_cleric/ak3vd_havise_dorn.json`). Join by name, once, and assert that all
twenty resolve — a silent mis-join would give the Praelucent a fuller's day.

### (b) Derived, for everyone else

Every character has an `occupation_id` (65 of them), a `planning_ward` (8 machine-readable values,
which `lore/characters/AGENTS.md` notes *"is authoring/spatial metadata and is not injected into the
NPC prompt"* — so it is ours to use), and a `spawn_location`.

```
Round = template(occupation_id)
      ∘ bound to ward(planning_ward)
      ∘ home = nearest unclaimed residential building in that ward to spawn
      ∘ well = nearest public source in that ward
```

**A template per occupation**, not per character. Sixty-five templates, each 2–4 legs. The baker's:

```ron
baker: (
    legs: [
        (from: Kindling,  place: "workplace", doing: Work),   // the oven, before light
        (from: Dayspring, place: "workplace", doing: Trade),  // selling
        (from: Waning,    place: "workplace", doing: Work),
        (from: Lamplight, place: "home",      doing: Sleep),
    ],
    leash_m: 12.0,
),
```

That is the entire behavioural content for one trade, and it will be right about 90% of the time.

### (c) The Night Office, for the Majors

The LLM may edit its own Round overnight — swap a leg, add an errand, change where it takes its meal.
See [05_the_llm_seam.md](05_the_llm_seam.md) §4. The Round is authored *content*; the Night Office is
how a character's day drifts because of what happened in it.

---

## 3. Homes: a bake, not an authoring task

No character has a home. But: 670 buildings have `use == "residential"`, 2,566 buildings carry a
`district`, and every character carries a `planning_ward`. Wards and districts are the same eight
things under two spellings (`bell_and_sluice` ↔ `"Bell and Sluice Wards"`), so one normalisation table
joins them.

```
for each character, in a stable order (sorted by id — determinism):
    candidates = residential buildings in my ward, unclaimed
    home       = nearest candidate to my authored spawn_location
    claim it
```

Deterministic, one-time, checked in as `assets/world/homes.json`, and a test asserts every character
with a home has a **reachable door**.

That test does not pass today. 106 buildings have a front door with no walkable ground within six
metres — `stable_hash(id) % polygon.len()` picks the door edge without checking what is on the other
side of the wall ([02_navigation.md](02_navigation.md) §1). **Fix the door rule in M1, before you bind
anybody's home to a house they cannot get into.**

500 people into 670 houses is comfortable. And the ones who get no house are the ones who should not
have one:

> **100 `pauper`. 18 `unhoused`. 14 `insecure_lodging`. 20 `retired`. 21 `widow`.**

The lore hands us their concern directly: *"fear of losing a sleeping place."* **Do not invent beds
for them.** An NPC with no door is an NPC who is still in the street at the Snuffing — and that is
exactly the person the watch stops, exactly the person who sleeps in the Bell and Ladle's woodstore
(Ede of the Needle's authored route says so), and exactly the kind of life the city is supposed to
contain. The absence of a home is content.

---

## 4. Workplaces: the one real content gap

`occupations.json` gives each of the 65 trades a `lore_locations` array — but they are **free prose**,
not area ids:

```json
{
  "occupation_id": "candor_cleric",
  "lore_locations": ["The Lanthorn", "Chapter house", "Parish churches",
                     "Saint Maren's", "Ostrelle"]
}
```

About 40 of 65 trades have at least one `lore_location` that maps cleanly onto an existing area — *The
Tallage* → `tallage`, *Coswald's Yard* → `coswalds_yard`, *The Wickmarket* → `wickmarket`, *Cinder
Row* → `cinder_row`, *The Hungry Ox* → `hungry_ox`, *Ward wells* → the nine water areas. That mapping
must be **authored once, by hand**, into `assets/world/workplaces.json`. It is ~150 lines and it is the
only genuinely new content this feature needs.

The rest are honest gaps, and they need decisions rather than code:

| Gap | Trades | What to do |
|---|---|---|
| **"not yet fixed"** | `baker` (*"Bakery site not fixed"*), `smith`, `brewer`, `bellfounder`, `executioner` | Pick a building. There are 434 `workshop` and 56 `industrial` footprints; assign one per ward and be done. Or leave them working from home, which for a medieval baker is *correct*. |
| **diffuse** | `domestic_servant` (*"Households throughout Ombreval"*), `scavenger` (*"City streets"*), `sanitation_worker`, `messenger` | Their workplace **is** the street. Their Round is a *circuit*, not a post: a loop of anchors within the ward. This is a different `Leg` shape and it is worth having — a servant's morning is house → well → market → house, and that is the most-walked route in the city. |
| **outside the wall** | `outer_wharves` — the **second most common** workplace in the lore (8 trades) | Not a gap. A commute. See §7. |
| **The Bell and Ladle** | named in **4 of the 20 authored routes** — and **it is not in `areas.json`** | Add it. One area, at the Bellstand. |

---

## 5. Market days move the crowd — the cheapest life in the plan

From `lore/core_lore/trade_and_daily_life.md`:

> **Highmarket** is the third day, chiefly at the **Wickmarket and Coswald's Yard**. **Lowmarket** is
> the sixth, chiefly at the **Tallage and Maren's Green**.

So `market_seller`, `food_provisioner`, `grocer_and_spicer`, `draper`, `salt_trader`, `fish_trader`,
`butcher` and the rest get a leg with `only_on: [Highmarket]` or `only_on: [Lowmarket]`, pointing at
the right square. And the authored routes already say so — Osanne Vell: *"stall on Highmarket"*.
Grigor Ashe: *"salt at Lowmarket"*. Jonet Sparr: *"the Wickmarket on Highmarket days"*.

**One `match` in the agenda builder, and the city has a week.** Walk into the Wickmarket on a
Highmarket and it is packed; on a Fourth it is a square with some empty stalls and a woman selling
candles. That is an enormous amount of felt life for approximately no work, and it is the thing I would
build immediately after the water round.

Bellday closes the trades and fills the nave. Also one `match`.

**And the crowding is a feature.** `lore/characters/AGENTS.md` has a dispersion rule — *"no 20 m
neighbourhood may contain more than three NPCs"* — but that is a **spawn** rule, written so the cast is
scattered across the city at load. Steady state is allowed, and required, to violate it. The gazetteer
is explicit:

> On Lowmarket, a laden cart approaching the bridge stair can halt pedestrian movement for a moment.
> **This is a designed piece of congestion, not a collision bug.** People wait, trade remarks, read
> the chalk across the yard, or take a longer route.

Do not "fix" the market being crowded.

---

## 6. Curfew empties the streets

Ladder rung 5. The Scold rings the legal Snuffing at the Bellstand — *"the office is prayer, the Scold
is law, and the minutes between them are the city's dusk grace"* — and everyone who has a bed goes to
it.

What is left in the street is the whole point:

- **the watch** (`watchman_and_keeper`, 15 `militia_and_soldier`) — who are *at work*, on their rounds;
- **the taverns** (`tavern_worker`, `cook`, `brewer`, `entertainer`) — Renna Tapster's *"counter until
  the last argument goes home"*;
- **the lamplighters**, finishing the round;
- **`sex_worker`**, `scavenger`, and the night trades;
- **the hundred and thirty-two people with nowhere to go.**

That last line is the game. A city that empties at curfew, and the people who are still in it, is a
better piece of atmosphere than most things you could build deliberately — and here it falls out of one
ladder rung plus content that already exists.

`features/lore_ward_politics.md` even names the lever: election results change *"gate hours, stall
licences, **watch routes**, shoring orders, well repairs"*. The curfew is designed to be politically
adjustable. Not now, but do not build it in a way that makes that hard.

---

## 7. The gate-caught boatmen — a failure mode of the schedule, and it is canon

`outer_wharves` is the second most common workplace in the lore, and it lies **outside the wall**
(x −580…−560). The boat families live in the Reed Ward (64 characters, `planning_ward: reed`) and work
at the wharves. Their day is a **commute out through the River Gate or the Reed Postern**.

And `lore/the_dry_boatmen.md` has already written what happens when the schedule fails:

> ## Gate-time
>
> The river has no hours. The gate does.
>
> A boat-family's day is [...] ruled by the **River Gate** and the **Reed Postern**, which **open in
> the morning and shut at the Snuffing** [...]
>
> - **Gate-caught.** A crew that misses the Snuffing sleeps outside the wall. This happens
>   constantly: bad water, a broken pole, a late tally at the wharf, a clerk who went to eat. A
>   gate-caught man is not in danger. He is simply *not home*, and his household eats without him,
>   again.

**This falls out of the system for free.** Give the gates an `open: Dayspring, shut: Snuffing`, give
the boatmen a Round that ends outside the wall, and some nights some of them will not make it back.
Their households will eat without them. Nobody has to write that; it happens, because the walk takes
as long as it takes and the gate shuts when it shuts.

It is also the best possible test of the whole stack: **if a boatman can be gate-caught, everything
below him works.**

The lore even gives us the word for it, and it is a word about more than gates:

> **"Gate-caught."** Shut out at the Snuffing. Also, more generally: **late, through no fault of one's
> own, in a system that does not care.**

---

## 8. What the Round is *not*

**Not a planner.** No goals, no preconditions, no search. A Round is a list of two to four places and
the office at which you walk to each. If that sounds too simple to produce a living city, re-read
seagame's seventeen `if`s and then go look at the crew.

**Not a guarantee.** Rungs 0–8 all preempt it. A thirsty man goes to the well even if his Round says
he should be at the tenter-yards, and he arrives late, and that is *correct*.

**Not per-character content, except for twenty people.** Sixty-five occupation templates, eight wards,
one home-binding bake. Everything else is derived. If you find yourself authoring a Round for the 400th
ambient, something has gone wrong in the templates.

**Not the LLM's job.** The LLM never plans a day. It may *edit* one, once, at night
([05](05_the_llm_seam.md) §4), and it may *interrupt* one with `go_to`. It does not build one, because
building one is a code problem and 500 of them is a budget problem.
