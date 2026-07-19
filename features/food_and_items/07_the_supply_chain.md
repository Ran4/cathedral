# The supply chain: carriers, grain, flour, and the death of the restock

M3 shipped a market that works and confessed its magic: `Round::restock` (`round.rs:1575`) conjures
bread into a baker's hands each Kindling, and `Round::close_books` (`round.rs:1620`) resets every
wallet each Watch. This document kills both — not by adding a simulation of agriculture, but by
**replacing each conjuring with one purchase**, until the last conjuring stands outside the walls
where the playable world stops anyway.

The chain, end to end:

```text
  beyond the walls          the gate            the city
  ────────────────          ────────            ────────
  Brede, the Combs   ->   Wool Gate    ->   carrier sells grain
  northern farms     ->   Stone Gate         |
                                             v
                                          miller buys grain, mills flour, sells flour
                                             |
                                             v
                                          baker buys flour, bakes at the bakehouse
                                             |
                                             v
                                          baker sells loaves at the Wickmarket   <- M3, unchanged
                                             |
                                             v
                                          Ilse eats                              <- M4, unchanged
```

Every arrow but the first is `try_purchase` (`round.rs:2648`), the machinery M3 already built. The
first arrow is the one new idea in this document: **the country carriers**.

---

## 1. The rule that shapes everything: a fixed cast

The near-countryside brief asks for *"actors arriving through a gate [with] grounded destinations,
kin, cargo, and news instead of being generic travelers from nowhere"*
(`features/the_near_countryside__aka_add_market_stalls.md:5`). That is a design constraint, not a
flourish:

> **No procedural people. Ever.** The carriers are a fixed, hand-authored roster with names, kin,
> homes-beyond-the-walls and memories, exactly like the other 514. The player must be able to learn
> that Ansel of Brede comes in on Highmarket with rye, recognise him next week, and ask him how the
> road was. A spawner would break that, and would break the "unknown people" rule
> (`crates/cathedral-sim/AGENTS.md`) that makes strangers legible.

The consequence for scope: this feature adds **at most three new authored characters**, because the
lore already staffed the road. Three occupations are canonically *outsiders* and the sim already
routes all ten of them to a gate:

| occupation | cast | `lore_locations` | current workplace |
|---|---|---|---|
| `farmer` | 7 | "The Combs", "Nearby farms", **"Villages beyond the walls"** | `["The Wool Gate"]` |
| `miller` | 3 | **"Mills beyond the walls"**, "Nearby villages", "Grain routes" | `["The Wool Gate"]` |
| `cargo_worker` | 16 | alternative titles include **"Carrier"**, **"Carter"**, "Wool carrier" | wharves / River Gate |

`ecbrd` **Ansel of Brede** is a farmer *named for the village at the top of the Wool Gate road*.
`p0026` **Noll Quern** and `p0027` **Corin Kett** are Wick Ward farmers; `danqn` **Ansel Quern** and
`davqn` **Averil Quern** are two of the eight bakers. A quern is a hand-mill. The grain family is
already written; nobody noticed because nothing in the sim yet asked where flour comes from.

## 2. Presence: the one genuinely new concept

Today every character in `world.characters` is unconditionally in the city. A carrier must be able
to *not be here* — six carters loitering at a gate all night is worse fiction than no carters at all.

```rust
/// Whether the character is inside the walls right now. `BeyondTheWalls`
/// people keep their state — wallet, holds, memories, kin — but are not in
/// the world the player or the other actors can perceive.
pub enum Presence { InCity, BeyondTheWalls }
```

A `BeyondTheWalls` character is:

- **omitted from `WorldSnapshot.actors`** (`snapshot.rs`) — and the Bevy host already handles this
  for free: `actors.rs:120-131` despawns any root whose id left the projection and spawns any id
  that arrived. No host change, no pop-in work, no new component;
- **skipped by the attention gate** (`attention.rs`) and therefore never spends an idle LLM turn;
- **not a percept recipient, not in `you_see`, not a valid `go_to`/`offer_item` target**;
- **still in `world.characters`** — the wallet they left with, the grain they didn't sell, and the
  memory of yesterday's road all persist, so the same person comes back next week having had a week.

Presence flips in exactly two places, both on the carrier's own round:

- **arrival**: at their arrival office, at the gate node, `BeyondTheWalls -> InCity`;
- **departure**: when they reach the gate node on their closing leg, `InCity -> BeyondTheWalls`.

Everyone else is `InCity` forever, so this is inert for the other 514. The seed sets carriers to
`BeyondTheWalls` and the world starts with the gates empty.

**The gate is the edge of the world.** The nav graph stops dead at the gate nodes — 5 (Wool), 17
(Stone), 50 (Harne), 36 (River), 51 (Reed Postern) are the extreme nodes of the graph in all four
directions, and there is nothing walkable beyond them. Ground geometry *does* run further out
(`GROUND_MIN/MAX_*`, `src/city/mod.rs:32-35` — 165 m north of the Wool Gate, 205 m west of the River
Gate), so a carrier standing at a gate stands on ground, not void; the appearing/vanishing happens
at the gate node itself, which is where a wall and a shut gate make it read correctly. Extending nav
outside the walls is deliberately **not** part of this feature.

## 3. The roster and the two roads

Two gate→market pairings ship. The geography picks them: these are the two shortest gate-to-square
runs in the city, and both are grain roads in lore.

### 3.1 The Wool Gate road — Brede and the Combs → The Wickmarket (130 m)

*"The upstream road toward Brede and the Combs enters here with wool, hides, honey, and pilgrims"*
(`lore/places/00_city_plan.md:170`). The tightest gate-to-market pairing in the city, and the bread
chain's whole length is visible from one rooftop.

| who | id | status | cargo | comes in on |
|---|---|---|---|---|
| Ansel of Brede | `ecbrd` | **existing** (farmer, minor) | rye grain, honey | Highmarket, Fourth |
| Noll Quern | `p0026` | **existing** (farmer, ambient) | rye grain | Highmarket |
| *a Combs wool-and-wheat carrier* | *new* | **to author** (minor) | wheat grain | Lowmarket, Seventh |

### 3.2 The Stone Gate road — northern farms and the Lantern Road → Coswald's Yard (241 m)

*"Quarry stone, lime, scaffold timber, charcoal, grain from northern farms, and the land road toward
Ostrelle use it. Its inner road descends directly to Coswald's Yard"*
(`lore/places/03_new_places_and_infrastructure.md:38-42`).

| who | id | status | cargo | comes in on |
|---|---|---|---|---|
| Osanne Crake | `p0024` | **existing** (farmer, ambient, Wallwright Ward — the Stone Gate's own ward) | wheat grain | Highmarket |
| *a Lantern Road trader* | *new* | **to author** (merchant, minor) | wheat grain, charcoal | Second, Fifth |

**Not shipped, data-only later:** River Gate → The Tallage (301 m, salt on the Salorge route —
`salt_trader` lore already names Grigor Ashe there), Reed Postern → Maren's Green (227 m,
handbarrows from the fish wharves), Harne Gate → The Bellstand (411 m, the dry road). Each is a
`food.json` + `rounds.json` edit once §4 exists.

**The weekday gating is the point.** Different carriers on different days means a market morning has
a *cast*, Belldays are quiet, and grain supply varies — which is what makes prices and scarcity
mean anything later. It also keeps the present-in-city headcount to 2–4 on a market day and 0–1
otherwise.

**Homes beyond the walls.** All ten outsiders currently hold city homes or `bedless` entries
(`ecbrd` is housed at the Draper's Reach; `p0024`–`p0028` are bedless), which contradicts their own
lore. This feature adds a `beyond_the_walls` circumstance to `scripts/bake_homes.py` and its guard
test (`round/tests.rs:128` — the `bedless_circumstances` list), giving them a home *string* naming
their village ("a holding above Brede, a day's cart from the Wool Gate") with no `door_node`. The
prompt path already tolerates a doorless home (`prompt/mod.rs:236` — the bedless framing), so
`your_home` answers "Where do you live?" correctly with no renderer change.

## 4. The carrier round

A new `carrier` archetype in `assets/world/rounds.json`, and a new `Arrival` variant. The legs are
the same `LegSpec` shape as everything else (`round.rs:212`), resolved by `build_legs`
(`round.rs:1874`) and selected by `active_leg` (`round.rs:1949`) — no new movement code:

```json
"carrier": {
  "leash_m": 12.0,
  "curfew_exempt": false,
  "legs": [
    {"from": "kindling",  "at": "gate",      "doing": "arrive"},
    {"from": "dayspring", "at": "workplace", "doing": "trade"},
    {"from": "waning",    "at": "gate",      "doing": "depart"}
  ]
}
```

- `"at": "gate"` is a third magic anchor beside `"home"` and `"workplace"`, resolved per-actor from a
  new `gates` map in `rounds.json` (actor id → gate place name). `build_legs` learns one more arm.
- `Arrival::Arrive` flips `Presence` to `InCity` on the tick the leg becomes active, placing the body
  at the gate node. `Arrival::Depart` flips it to `BeyondTheWalls` **on arrival at the gate**, not on
  the office boundary — so the walk out is watchable and a carrier caught in conversation finishes it.
- `only_on` carries the weekday roster from §3. A carrier with no leg today never becomes `InCity`.

Their prose timetable (`leg_line`, `round.rs:3763`) writes itself into `daily_round`, so a carrier
asked about their day answers from the sheet like everyone else.

## 5. Stalls that sell things you cannot eat

Three `try_purchase` assumptions are wrong once grain exists. All three are small and all three are
already flagged at the seam:

| assumption | where | fix |
|---|---|---|
| stock must be edible | `round.rs:2667` — filters on `is_edible` | filter on **the trade's stock kinds** instead; the trade already declares them (`TradeSpec.stock`) |
| a bound vendor never buys | `nearest_open_stall`, `round.rs:2821` | a vendor may buy from a stall of a **different trade** — a baker at the bread pitch may still queue at the mill. Guard on `stall.trade != my_stall.trade`, not on vendorship |
| the buyer wants the cheapest affordable | `round.rs:2667` | correct for hunger, wrong for supply. A trade gains `intent: Eat \| Stock`; `Stock` buyers take **as much as the wallet allows**, cheapest-first, up to a per-trade cap |

Two new catalog kinds in `assets/world/items.json` (joining the six that exist):

| kind | display | stackable | edible | metadata | price_sparks |
|---|---|---|---|---|---|
| `grain` | measure of rye / wheat | yes | **no** | `grain: [rye, wheat]` | `grain=rye` 3, `grain=wheat` 6 |
| `flour` | sack of rye flour / wheat flour | yes | **no** | `grain: [rye, wheat]` | `grain=rye` 5, `grain=wheat` 9 |

Prices ladder deliberately: grain 3 → flour 5 → the 2-spark loaf that
[02](02_the_spark_standard.md) fixed and this feature must not move. One measure of grain must
therefore yield enough loaves to clear the miller's and baker's margins — §6 sets the yields so it
does, and the headless conservation check in §9 is what proves it.

Two new stalls in `assets/world/food.json`, using the existing `site` + `pitch_offset` +
walkability-fallback resolution (`seed_food`, `round.rs:1348-1400`) — no new nav places:

| stall | site | trade | vendor occupations | open |
|---|---|---|---|---|
| The Wool Gate grain pitch | The Wool Gate | `grain` | `farmer`, `merchant` (the carriers) | dayspring→waning, per carrier weekdays |
| The Stone Gate grain pitch | Coswald's Yard | `grain` | `farmer`, `merchant` | ditto |
| The Wool Gate mill | The Wool Gate | `flour` | `miller` | kindling→waning |

The mill sits at the Wool Gate because that is already the millers' workplace in `rounds.json` and
because *"Mills beyond the walls"* means the mill proper is off-map; the pitch is the mill's city
face. Vendor binding (`bind_vendors`, `round.rs:1406`) is unchanged — it already requires that the
candidate's round delivers them to the site, which for millers and carriers it now does.

## 6. Milling and baking: the transform that replaces the conjuring

`restock` conjures because nothing in the sim can turn one item into another. That is the actual
missing verb, and it is small:

```rust
/// A timed, ladder-driven conversion of held inputs into held outputs at a
/// work site. Conserves nothing by design — that is what a mill *is* — but is
/// declared in data, so the yields are auditable and the trace can prove them.
struct Transform {
    at: String,              // work site, resolved like a stall pitch
    occupations: Vec<String>,
    consumes: Vec<StockSpec>,
    produces: Vec<StockSpec>,
    seconds: f64,
}
```

Declared in `assets/world/food.json` alongside the trades:

| transform | site | who | consumes | produces | when |
|---|---|---|---|---|---|
| milling | The Wool Gate | `miller` | 1× `grain {rye}` | 3× `flour {rye}` | kindling→waning, whenever grain is held |
| milling | The Wool Gate | `miller` | 1× `grain {wheat}` | 3× `flour {wheat}` | ditto |
| baking | **the bakehouse** | `baker` | 1× `flour {rye}` | 5× `loaf {flour: rye}` | kindling, before the market leg |
| baking | the bakehouse | `baker` | 1× `flour {wheat}` | 4× `loaf {flour: wheat}` | ditto |

Yields chosen so the ladder pays: 3 sparks of rye grain → 3 sacks worth 15 → 15 loaves worth 30.
The miller's margin is 12 sparks a measure, the baker's 15 a sack, and the 2-spark loaf is untouched.

**The bakehouse.** *"Communal bakehouses cluster near grain routes and ward edges"*
(`lore/places/03_new_places_and_infrastructure.md:254`). The Wool Gate → Wickmarket road **is** the
grain route, so the bakehouse goes on it: a `pitch_offset` off the Wickmarket node, ~30 m up the
gate road, with the same walkability fallback every stall pitch uses. If a prop pass wants an oven
and a smoking flue later, `src/city/smoke.rs` already knows how to put smoke over a hearth on the
sim clock (the graphics overhaul's chimney work).

The bakers' Kindling leg changes from `{at: workplace, doing: work}` (which resolves to the
Wickmarket) to `{at: bakehouse, doing: work}`, and their Dayspring leg still takes them to the pitch.
**The restock becomes a walk you can watch**: at Kindling a baker buys flour, walks to the bakehouse,
bakes, and carries loaves to the Wickmarket in time for Dayspring — which is precisely what the
morning of a medieval city looked like, and precisely what `Round::restock` was faking.

At that point `restock` has nothing left to conjure for the `bread`, `grain` and `flour` trades. It
survives only for `provisions` and `fish` (herring, eel — the wharves are a chain this feature does
not build) and for the pot, which keeps its licensed infinity. **The dead code is deleted, not
left**: `restock` becomes `restock_unchained_trades`, and its doc comment names the two chains still
owed.

## 7. Money: what replaces the Watch ledger

This is the hard half, and the one that must not be hand-waved. Today `close_books` is the *only*
mint and burn in the system, which is why `debug_assert_eq!` conservation
(`round.rs:2688-2697`) can hold everywhere else. Deleting it without a replacement makes the city's
spark supply a closed pool that drains into whoever sells most, and by the fourth day nobody can buy
bread.

The carrier is the natural boundary, and gives an honest model:

- **the sink**: a carrier's takings **leave with them** through the gate. Sparks paid for grain exit
  the city. This is not a cheat; it is a trade balance.
- **the source**: wages, paid at the Watch by the four institutions that in lore actually hold
  coin — the Chapter, the Watch, the Tallage customs, and the harbour brokers. `close_books` shrinks
  from *"every wallet resets to seed"* to *"these four payers credit their own people"*, and the
  cheat shrinks with it: institutions that print money are a smaller lie than 514 self-refilling
  purses, and it is the lie every real city tells.
- **the balance**: the source must roughly match the sink or the city inflates/deflates. `food_summary()`
  gains `sparks in / sparks out` per day so the drift is a number you can watch, not a surprise.

The seed wallets ([02](02_the_spark_standard.md) §4) stay as the day-zero condition. What dies is
the nightly reset of *buyers*; what remains, renamed and honest, is a payroll.

**This is deliberately last (M5d).** M5a–c can ship with `close_books` intact — the chain works fine
with resetting wallets, it is just not yet an economy. Do not couple them.

## 8. Milestones

Ordered so the visible thing lands first, per the brief. Each is shippable and each leaves the game
in a playable state.

### M5a — The carriers *(the visible one)*

**Ships:** `Presence` + the snapshot/attention/percept filtering; the `carrier` archetype, the
`"gate"` anchor and `Arrival::Arrive`/`Depart`; the `beyond_the_walls` home framing + `bake_homes.py`
change; the three new authored characters; the `grain` catalog kind; the two grain pitches;
`try_purchase`'s three generalisations (§5). Millers buy grain and hold it — they cannot mill yet.

**How you know:**

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- --fake --trace-food --watch-clock 1
```

`[food]` shows the gates empty at the Watch, Ansel of Brede arriving at the Wool Gate at Kindling on
a Highmarket, a grain pitch opening at Dayspring, millers queueing and buying, and the pitch walking
back out at the Waning. Headcount `in city` returns to its baseline overnight. On a Bellday the
gates stay shut and nothing changes.

In-game, on a Highmarket morning:

```sh
CATHEDRAL_DRIVE='wait-online; tp -35 18 470 180 -25; shot woolgate_kindling; sleep 30; shot woolgate_trade; quit' cargo run
```

The first shot is an empty gate; the second has a named carrier with a queue of millers. Walk up and
ask him where he came from — the answer is on his sheet, and it is Brede.

### M5b — Flour

**Ships:** the `flour` kind; the `Transform` machinery and its `food.json` table; the Wool Gate mill
stall; millers milling and selling. Bakers buy flour and hold it; `restock` still conjures their bread.

**How you know:** the trace shows a measure of rye becoming three sacks at the mill and the miller's
`you_sell` quoting flour off the template. A unit test walks one measure of grain from carrier to
miller to baker's hands, asserting item ids are minted and consumed exactly once and that no stack
ever reaches quantity 0.

### M5c — The bakehouse *(the restock dies)*

**Ships:** the bakehouse site; the bakers' Kindling leg repointed; baking; `restock` reduced to
`restock_unchained_trades` for `provisions`/`fish`/the pot.

**How you know:** grep for the bread conjuring and find nothing. The trace shows, in order on one
Highmarket morning: carrier in → miller buys grain → miller mills → baker buys flour → baker walks to
the bakehouse → baker bakes → baker walks to the Wickmarket → Ilse buys a loaf. **Eight steps, one
morning, no magic.** Then the M4 acceptance test
([06_milestones.md](06_milestones.md) §M4) is re-run unchanged and still passes — that is the proof
the M3 shapes were right.

### M5d — Wages *(the ledger dies)*

**Ships:** §7 — takings leaving with carriers, institutional payroll, `sparks in/out` in the census,
`close_books` reduced to a payroll.

**How you know:** a seven-day `--watch-clock` run where the city's total spark holding stays inside
a stated band, no vendor goes bankrupt, and no buyer is priced out of a loaf for more than a day.
This is a *tuning* milestone as much as a coding one; expect to move the wage constants twice.

## 9. Observability and invariants

`--trace-food` extends rather than forks. New `[food]` lines: `carrier in`/`carrier out` with gate
and cargo, `milled`/`baked` with inputs and outputs, and the existing sale line unchanged.

`Round::food_summary()` (`round.rs:739`) gains a chain block:

```text
food:  … (unchanged) …
chain: in city 3 carriers | grain 14, flour 9, loaves 31 | milled 6, baked 20 today
coin:  412 held | in 96, out 84 today | drift +12
```

Standing assertions, checked under `--trace-food` and in tests:

- **items are conserved except at declared transforms** — a property test over any sequence of
  arrivals, purchases, transforms and eats: every item id is minted once and destroyed once, and the
  only quantity changes not accounted for by a purchase or an eat are `Transform`-shaped;
- **sparks are conserved except at the gate and the payroll** — the M3 cross-check, now with two
  named exits;
- `world.assert_invariants()` after every transform, as `restock` already does (`round.rs:1610`);
- **no `BeyondTheWalls` character appears in a snapshot, a percept, or a `you_see`** — one test per
  seam, because a leak here is invisible in the trace and obvious in the game.

## 10. Fixtures and blast radius

Smaller than M0's. Fixture worlds pass `nav: None`, so nobody is enrolled, no round runs, and no
carrier ever arrives — the 22 existing prompts should be **byte-stable**. Verify that before
assuming it; new `items.json` rows are inert unless a fixture holds one.

Two new fixtures, both for the new sheet shapes M5 introduces:

- `carrier_at_the_gate.txt` — a `BeyondTheWalls`-authored character rendered `InCity`, with the
  village home framing and a `you_sell` of grain;
- `miller_with_flour.txt` — `you_sell` quoting a non-edible trade, proving §5's generalisation
  reaches the sheet.

`turn.j2` needs **no** new prose. Grain and flour are ordinary stackable items, the mill and the
bakehouse are ordinary places, and buying is the same `offer_item` the model already does. If a
model needs to be told how to buy flour, §5 was implemented wrong.

## 11. Risk ledger

| risk | mitigation |
|---|---|
| `Presence` leaks — a `BeyondTheWalls` carrier is targeted, perceived, or spends an LLM turn | one test per seam (§9); the filter goes in `snapshot`/`attention`/percept-recipient construction, not at call sites, so there is one place to get right |
| the chain starves — a day with no carrier means no grain means no bread three days later | the weekday roster guarantees ≥1 grain carrier on 5 of 7 days; millers and bakers hold buffer stock across days (their `holds` persist — only stall *pitch* stock was ever swept). Watch `chain:` counts for a downward trend across a 7-day run |
| the margins are wrong and the miller or baker goes broke | §6's yields are declared in data, not code — retune without a rebuild. M5d's tuning milestone exists for exactly this |
| the player never sees any of it, because it happens at Kindling while they sleep | the Wool Gate → Wickmarket road is 130 m and the whole chain runs along it. This is why that pairing ships first. If it still reads as invisible, move baking later rather than adding narration |
| a carrier despawns mid-conversation | `Depart` flips presence **on arrival at the gate**, and the conversation floor (`features/implemented/conversation_floor.md`) already holds an actor in place while talking — verify it outranks the round leg, and if not, gate the flip on "not in a conversation" |
| deleting `restock` for bread while `provisions`/`fish` still conjure looks half-finished | it *is* half-finished, honestly and deliberately. The doc comment on `restock_unchained_trades` names the two chains still owed so the next person knows it is a queue, not an oversight |
