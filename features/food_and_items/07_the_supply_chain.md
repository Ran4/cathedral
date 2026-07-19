# The supply chain: merchants, grain, flour, and the death of the restock

M3 shipped a market that works and confessed its magic: `Round::restock` (`round.rs:1575`) conjures
bread into a baker's hands each Kindling, and `Round::close_books` (`round.rs:1620`) resets every
wallet each Watch. This document kills both — not by adding a simulation of agriculture, but by
**replacing each conjuring with one purchase**, until the last conjuring stands outside the walls
where the playable world stops anyway.

The chain, end to end:

```text
  beyond the walls        the gate                 the city
  ────────────────        ────────                 ────────
  Brede, the Combs  ->  Wool Gate   ->  a merchant's cart, sells grain
  northern farms    ->  Stone Gate      |                          ^
  Salorge, the Serle -> River Gate      |                          |
                                        v                          |
                          miller buys grain, mills flour, sells it  |
                                        |                          |
                                        v                          |
                          baker buys flour, bakes at the bakehouse  |
                                        |                          |
                                        v                          |
                          baker sells loaves at the Wickmarket      |   <- M3, unchanged
                                        |                          |
                                        v                          |
                          Ilse eats                                 |   <- M4, unchanged
                                                                    |
                          the same cart, loaded with Ombreval cloth,
                          pays the city on its way back out  ───────┘
```

Every arrow inside the walls is `try_purchase` (`round.rs:2648`), the machinery M3 already built.
The two new ideas are **the merchants who come in through the gates**, and **the fact that their
carts are full in both directions** — which is what makes §7's economy close without printing money.

---

## 1. The rule that shapes everything: a fixed cast

The near-countryside brief asks for *"actors arriving through a gate [with] grounded destinations,
kin, cargo, and news instead of being generic travelers from nowhere"*
(`features/the_near_countryside__aka_add_market_stalls.md:5`). That is a design constraint, not a
flourish:

> **No procedural people. Ever.** The road traders are a fixed, hand-authored roster with names, kin,
> homes-beyond-the-walls and memories, exactly like the other 514. The player must be able to learn
> that Ansel of Brede comes in on Highmarket with rye, recognise him next week, and ask him how the
> road was. A spawner would break that, and would break the "unknown people" rule
> (`crates/cathedral-sim/AGENTS.md`) that makes strangers legible.

### 1.1 They are merchants, not peasants

The people who come through the gate are **businesspeople** — capital, stock, credit, a name on a
contract, and opinions about the road. Not a ploughman with a sack. This matters mechanically as
much as tonally: a merchant *buys as well as sells*, and §7's whole economy rests on that.

The `merchant` occupation is already written for exactly this — `lore_locations`: "The five
squares", "The Tallage", "Outer wharves", "River roads", **"Lantern Road"**; `alternative_titles`
include **"Foreign merchant"** and **"Road trader"**; the `lore_example` is *"Merchants know Ombreval
as a toll town with honest weights, fair tolls and crowds that turn the window into custom."*

And a **trading dynasty already exists in the cast**, fully written, currently doing nothing because
nothing in the sim ever asked where goods come from:

| who | id | trade | role in the chain |
|---|---|---|---|
| **Clemence Crake** | `fp6ck` | Wholesale merchant, the Crake counter on the Tallage, 19 years | *"Brede wool **bought** down to the last fleece, Ombreval cloth **sold out** along the river and the Lantern Road, credit given open-handed and called in to the very day."* **She is the two-way valve, already authored.** |
| **Renn Crake** | `fr9ck` | Cargo broker — **her son** (`mother: fp6ck`) | moves what she buys. She has *"begun to look twice at his figures, because his promises have got larger and his sleep has got worse"* — a supply chain gives that a place to actually go wrong |
| **Ewart Skell** | `e1skl` | Draper, "Cloth merchant" | the outbound cargo: Ombreval cloth |
| **Dunstan Skell** | `fb3sk` | Money broker | the credit behind a cart |
| **Ansel of Salorge** | `fa4sg` | Foreign merchant — came up the Serle at nineteen with salt and southern iron | the River Gate road's city end |
| **Gile of Brede** | `fg4br` | Hired writer, from Brede | the Wool Gate road's paperwork |

Six of the eight names in Clemence Crake's `knows` list are links in this chain. Nobody planned that
for this feature; it fell out of the lore being written by someone thinking about how a toll town
works.

### 1.2 What this feature actually authors

The dynasty above is **resident** — Clemence has held her counter for nineteen years and is not
going anywhere. What does not exist yet is **the road end**: the branch of the family that works
Brede and comes in on market mornings. That is what gets authored, and the near-countryside brief
asked for precisely this shape:

> *"Families might keep one branch inside a gate and another on a holding outside, while seasonal
> workers and sellers cross the walls every day."*
> — `features/the_near_countryside__aka_add_market_stalls.md:4`

So: **five or six new authored characters** — three travelling merchants (one per road) plus their
carters — each *kin or factor to someone already in the city*, so an arrival is a reunion and an
argument rather than a stranger with a cart.

A merchant and their carters travel together, arrive together and leave together. This document
calls that unit a **road party**, and calls its members **road traders** where the merchant and the
hands behave identically (they share one round, one gate and one presence flag). Where the
distinction matters — who owns the stock, who does the talking — it says *merchant* and *carter*.

The existing outsider trades stay in the picture as the **second tier**, and the sim already routes
all ten of them to a gate:

| occupation | cast | `lore_locations` | role |
|---|---|---|---|
| `farmer` | 7 | "The Combs", **"Villages beyond the walls"** | small sellers who walk their own produce in on market days |
| `miller` | 3 | **"Mills beyond the walls"**, "Grain routes" | the grain's first buyer (§6) |
| `cargo_worker` | 16 | alt. titles include **"Carrier"**, **"Carter"** | the hands who push the merchants' carts |

`ecbrd` **Ansel of Brede** is a farmer named for the village at the top of the Wool Gate road.
`p0026` **Noll Quern** and `p0027` **Corin Kett** are Wick Ward farmers; `danqn` **Ansel Quern** and
`davqn` **Averil Quern** are two of the eight bakers. A quern is a hand-mill — the grain family was
written before anything needed one.

## 2. Presence: the one genuinely new concept

Today every character in `world.characters` is unconditionally in the city. A road trader must be able
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

Presence flips in exactly two places, both on the road party's own round:

- **arrival**: at their arrival office, at the gate node, `BeyondTheWalls -> InCity`;
- **departure**: when they reach the gate node on their closing leg, `InCity -> BeyondTheWalls`.

Everyone else is `InCity` forever, so this is inert for the other 514. The seed sets road traders to
`BeyondTheWalls` and the world starts with the gates empty.

**The gate is the edge of the world.** The nav graph stops dead at the gate nodes — 5 (Wool), 17
(Stone), 50 (Harne), 36 (River), 51 (Reed Postern) are the extreme nodes of the graph in all four
directions, and there is nothing walkable beyond them. Ground geometry *does* run further out
(`GROUND_MIN/MAX_*`, `src/city/mod.rs:32-35` — 165 m north of the Wool Gate, 205 m west of the River
Gate), so a road trader standing at a gate stands on ground, not void; the appearing/vanishing happens
at the gate node itself, which is where a wall and a shut gate make it read correctly. Extending nav
outside the walls is deliberately **not** part of this feature.

## 3. The roster and the roads

Two gate→market pairings ship. The geography picks them: these are the two shortest gate-to-square
runs in the city, and both are grain roads in lore.

Each road gets **one named merchant, one or two carters, and a city counterparty** — the person
inside the walls they deal with, who already exists. The counterparty is what makes an arrival
matter: the player can meet Clemence Crake at her counter on the Tallage on a quiet day and then
watch her kinsman's cart come through the Wool Gate on Highmarket.

### 3.1 The Wool Gate road — Brede and the Combs → The Wickmarket (130 m)

*"The upstream road toward Brede and the Combs enters here with wool, hides, honey, and pilgrims"*
(`lore/places/00_city_plan.md:170`). The tightest gate-to-market pairing in the city, and the bread
chain's whole length is visible from one rooftop. **This is the road that ships first.**

| who | id | status | role | comes in on |
|---|---|---|---|---|
| *a Crake of the Brede road* | *new* | **to author** — merchant, minor, kin to `fp6ck`; the family's road branch, resented and indispensable | sells rye + wheat grain, wool, honey; **buys cloth** | Highmarket, Fourth |
| *two carters* | *new* | **to author** — `cargo_worker`, ambient; her hands | push the cart, stand the pitch | with her |
| Ansel of Brede | `ecbrd` | **existing** — farmer, minor, named for the village | small seller: his own rye, on foot | Highmarket |
| **Clemence Crake** | `fp6ck` | **existing, resident** — the counter on the Tallage | *the city counterparty*: buys the wool, sells the cloth | — |
| **Renn Crake** | `fr9ck` | **existing, resident** — cargo broker, her son | brokers the load; his figures are the family's soft spot | — |

### 3.2 The Stone Gate road — northern farms and the Lantern Road → Coswald's Yard (241 m)

*"Quarry stone, lime, scaffold timber, charcoal, grain from northern farms, and the land road toward
Ostrelle use it. Its inner road descends directly to Coswald's Yard"*
(`lore/places/03_new_places_and_infrastructure.md:38-42`).

| who | id | status | role | comes in on |
|---|---|---|---|---|
| *a Lantern Road merchant* | *new* | **to author** — merchant, minor; works the six-week Ostrelle road, so *arrives rarely and matters when he does* | sells wheat grain, charcoal; **buys cloth** | Second, Fifth |
| *one carter* | *new* | **to author** — `cargo_worker`, ambient | his hand | with him |
| Osanne Crake | `p0024` | **existing** — farmer, ambient, Wallwright Ward (the Stone Gate's own ward) | small seller | Highmarket |
| **Ewart Skell** | `e1skl` | **existing, resident** — draper, "Cloth merchant" | *the city counterparty*: the cloth he loads for Ostrelle | — |

### 3.3 The River Gate road — Salorge and the Serle → The Tallage (301 m) *(third, data-only)*

The broadest working gate, *"with paired leaves, a porter wicket, toll shelter, dung ruts, and room
for one cart to wait while another passes"* (`lore/places/00_city_plan.md`). Salt on the Salorge
route; `fa4sg` **Ansel of Salorge** (foreign merchant, came up the Serle at nineteen with salt and
southern iron) is the city end, and `fb3sk` **Dunstan Skell** the money behind it. Ships as a
`food.json` + `rounds.json` edit plus one authored factor, once §4 exists.

**Not shipped:** Reed Postern → Maren's Green (227 m, handbarrows from the fish wharves), Harne Gate
→ The Bellstand (411 m, the dry road).

**The weekday gating is the point.** Different merchants on different days means a market morning has
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

## 4. The road round

A new `road_trader` archetype in `assets/world/rounds.json` (the term is a `merchant`
`alternative_title`, and covers the merchant and their carters alike — they travel the same legs),
and a new `Arrival` variant. The legs are the same `LegSpec` shape as everything else
(`round.rs:212`), resolved by `build_legs` (`round.rs:1874`) and selected by `active_leg`
(`round.rs:1949`) — no new movement code:

```json
"road_trader": {
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
  the office boundary — so the walk out is watchable and a trader caught in conversation finishes it.
- `only_on` carries the weekday roster from §3. A road party with no leg today never becomes `InCity`.

Their prose timetable (`leg_line`, `round.rs:3763`) writes itself into `daily_round`, so a road trader
asked about their day answers from the sheet like everyone else.

## 5. Stalls that sell things you cannot eat

Three `try_purchase` assumptions are wrong once grain exists. All three are small and all three are
already flagged at the seam:

| assumption | where | fix |
|---|---|---|
| stock must be edible | `round.rs:2667` — filters on `is_edible` | filter on **the trade's stock kinds** instead; the trade already declares them (`TradeSpec.stock`) |
| a bound vendor never buys | `nearest_open_stall`, `round.rs:2821` | a vendor may buy from a stall of a **different trade** — a baker at the bread pitch may still queue at the mill. Guard on `stall.trade != my_stall.trade`, not on vendorship |
| the buyer wants the cheapest affordable | `round.rs:2667` | correct for hunger, wrong for supply. A trade gains `intent: Eat \| Stock`; `Stock` buyers take **as much as the wallet allows**, cheapest-first, up to a per-trade cap |

Three new catalog kinds in `assets/world/items.json` (joining the six that exist):

| kind | display | stackable | edible | metadata | price_sparks |
|---|---|---|---|---|---|
| `grain` | measure of rye / wheat | yes | **no** | `grain: [rye, wheat]` | `grain=rye` 3, `grain=wheat` 6 |
| `flour` | sack of rye flour / wheat flour | yes | **no** | `grain: [rye, wheat]` | `grain=rye` 5, `grain=wheat` 9 |
| `cloth` | bolt of kersey / broadcloth | yes | **no** | `grade: [kersey, broadcloth]` | `grade=kersey` 14, `grade=broadcloth` 40 |

`cloth` is the **return load** (§7) and arrives with M5d, not M5a — but it is listed here because it
uses the identical machinery and because its prices are what balance the books. Ombreval's export
being expensive relative to a 2-spark loaf is the point: one bolt of broadcloth pays for a lot of
grain, which is why a merchant bothers with the six-week Ostrelle road.

Prices ladder deliberately: grain 3 → flour 5 → the 2-spark loaf that
[02](02_the_spark_standard.md) fixed and this feature must not move. One measure of grain must
therefore yield enough loaves to clear the miller's and baker's margins — §6 sets the yields so it
does, and the headless conservation check in §9 is what proves it.

Two new stalls in `assets/world/food.json`, using the existing `site` + `pitch_offset` +
walkability-fallback resolution (`seed_food`, `round.rs:1348-1400`) — no new nav places:

| stall | site | trade | vendor occupations | open |
|---|---|---|---|---|
| The Wool Gate grain pitch | The Wool Gate | `grain` | `merchant`, `farmer` (the road traders) | dayspring→waning, per merchant's weekdays |
| The Stone Gate grain pitch | Coswald's Yard | `grain` | `merchant`, `farmer` | ditto |
| The Wool Gate mill | The Wool Gate | `flour` | `miller` | kindling→waning |
| The Draper's Reach cloth counter *(M5d)* | The Draper's Reach | `cloth` | `draper`, `cloth_worker` | dayspring→waning — where the return load is bought |

The mill sits at the Wool Gate because that is already the millers' workplace in `rounds.json` and
because *"Mills beyond the walls"* means the mill proper is off-map; the pitch is the mill's city
face. Vendor binding (`bind_vendors`, `round.rs:1406`) is unchanged — it already requires that the
candidate's round delivers them to the site, which for millers and road traders it now does.

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

**The merchant is the boundary, and a merchant's cart is full in both directions.** That is the
whole model, and Clemence Crake's sheet already states it: *"Brede wool bought down to the last
fleece, Ombreval cloth sold out along the river and the Lantern Road."* A merchant who arrives with
grain and leaves with an empty cart and a purse of sparks is a coin sink that bankrupts the city in
a week. A merchant who arrives with grain and leaves **with cloth** is a trade balance.

```text
   sparks OUT: the city pays the merchant for grain        (§5, the gate pitch)
   sparks IN:  the merchant pays the city for cloth        (the return load)
   net:        the toll, and whatever the terms were that morning
```

- **the outbound cargo is cloth**, because that is what Ombreval actually exports — the Cloth Ward,
  Draper's Reach, the tenterhooks, the cloth halls and 12 `cloth_worker` + 3 `draper` cast all exist
  for it and currently produce nothing. One new catalog kind (`cloth {grade}`), one buying trade
  bound to `draper`/`cloth_worker` vendors, and the same `try_purchase` again. **The second chain
  comes nearly free, and it is the one that pays for the first.**
- **the merchant's `intent` is `Stock` in both directions**: they sell grain until their stock is
  gone, then spend their takings on cloth before their departure leg. A merchant who has sold well
  buys well — so a good grain morning becomes a good cloth afternoon, visibly, in the same square.
- **what is left over is small enough to be honest.** The residual imbalance — the toll, the
  merchant's actual profit, wages for people who make nothing tradeable (the Watch, the Chapter, the
  water-carriers) — is settled by a **payroll**: `close_books` shrinks from *"every wallet resets to
  seed"* to *"these institutions credit their own people"*. That is still a cheat, but it is a
  bounded one, and it is the lie every real city with a mint tells.
- **the balance is a number you watch, not a hope.** `food_summary()` gains `sparks in / out /
  drift` per day (§9). If drift trends, the cloth prices or the payroll are wrong — both live in
  data.

The seed wallets ([02](02_the_spark_standard.md) §4) stay as the day-zero condition. What dies is
the nightly reset of *buyers*.

**Why this is worth the extra chain:** the alternative — four institutions printing money at the
Watch — makes the city's economy a closed accounting trick the player can never see or affect. The
two-way cart makes it a *place*: prices move when a road is bad, the Draper's Reach has customers
because Ostrelle wants cloth, and the famine lever the lore already wrote (*"F.183: the Chapter
opened its granary late"*) has something to act on. It also gives Renn Crake's larger promises and
worse sleep somewhere to lead.

**This is deliberately last (M5d).** M5a–c can ship with `close_books` intact — the chain works fine
with resetting wallets, it is just not yet an economy. Do not couple them.

## 8. Milestones

Ordered so the visible thing lands first, per the brief. Each is shippable and each leaves the game
in a playable state.

### M5a — The merchants *(the visible one)*

**Ships:** `Presence` + the snapshot/attention/percept filtering; the `road_trader` archetype, the
`"gate"` anchor and `Arrival::Arrive`/`Depart`; the `beyond_the_walls` home framing + `bake_homes.py`
change; **the five or six new authored characters of §3, each kinned to a resident** (this is
authoring work, not code — budget it as such, and write them to the standard of the sheets they are
kin to); the `grain` catalog kind; the two grain pitches; `try_purchase`'s three generalisations
(§5). Millers buy grain and hold it — they cannot mill yet.

**How you know:**

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- --fake --trace-food --watch-clock 1
```

`[food]` shows the gates empty at the Watch, the Crake cart arriving at the Wool Gate at Kindling on
a Highmarket, a grain pitch opening at Dayspring, millers queueing and buying, and the party walking
back out at the Waning. Headcount `in city` returns to its baseline overnight. On a Bellday the
gates stay shut and nothing changes.

In-game, on a Highmarket morning:

```sh
CATHEDRAL_DRIVE='wait-online; tp -35 18 470 180 -25; shot woolgate_kindling; sleep 30; shot woolgate_trade; quit' cargo run
```

The first shot is an empty gate; the second has a named merchant, two carters and a queue of millers.

**The acceptance test is a conversation, not a counter.** Walk up and ask her where she came from,
whose cart it is, and how the road was. The answers are on her sheet and they are Brede, the Crake
counter, and an opinion about Renn Crake's figures. Then go find Clemence Crake on the Tallage and
ask *her* about the cart. If both halves of that hold, the fixed-cast rule (§1) is paying for itself
and a procedural spawner would have bought nothing.

### M5b — Flour

**Ships:** the `flour` kind; the `Transform` machinery and its `food.json` table; the Wool Gate mill
stall; millers milling and selling. Bakers buy flour and hold it; `restock` still conjures their bread.

**How you know:** the trace shows a measure of rye becoming three sacks at the mill and the miller's
`you_sell` quoting flour off the template. A unit test walks one measure of grain from merchant to
miller to baker's hands, asserting item ids are minted and consumed exactly once and that no stack
ever reaches quantity 0.

### M5c — The bakehouse *(the restock dies)*

**Ships:** the bakehouse site; the bakers' Kindling leg repointed; baking; `restock` reduced to
`restock_unchained_trades` for `provisions`/`fish`/the pot.

**How you know:** grep for the bread conjuring and find nothing. The trace shows, in order on one
Highmarket morning: merchant in → miller buys grain → miller mills → baker buys flour → baker walks
to the bakehouse → baker bakes → baker walks to the Wickmarket → Ilse buys a loaf. **Eight steps,
one morning, no magic.** Then the M4 acceptance test
([06_milestones.md](06_milestones.md) §M4) is re-run unchanged and still passes — that is the proof
the M3 shapes were right.

### M5d — The return load *(the ledger dies)*

**Ships:** §7 — the `cloth` kind and the Draper's Reach counter; merchants spending their takings on
cloth before departure; takings leaving through the gate; the residual institutional payroll;
`sparks in/out/drift` in the census; `close_books` reduced to that payroll.

**How you know:** a seven-day `--watch-clock` run where the city's total spark holding stays inside
a stated band, no vendor goes bankrupt, and no buyer is priced out of a loaf for more than a day.
Then the reason it works should be *visible*: on a Highmarket the same merchant sells grain at the
Wool Gate in the morning and queues at the Draper's Reach in the afternoon, and the drapers who have
had no customers for four milestones finally have one.

This is a *tuning* milestone as much as a coding one; expect to move the cloth prices and the
payroll constants twice. Both live in data, so that costs a re-run, not a rebuild.

## 9. Observability and invariants

`--trace-food` extends rather than forks. New `[food]` lines: `road in`/`road out` with gate, party
and cargo both ways; `milled`/`baked` with inputs and outputs; the existing sale line unchanged.

`Round::food_summary()` (`round.rs:739`) gains a chain block:

```text
food:  … (unchanged) …
chain: in city 2 merchants + 3 carters | grain 14, flour 9, loaves 31, cloth 4 | milled 6, baked 20 today
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
road party ever arrives — the 22 existing prompts should be **byte-stable**. Verify that before
assuming it; new `items.json` rows are inert unless a fixture holds one.

Two new fixtures, both for the new sheet shapes M5 introduces:

- `merchant_at_the_gate.txt` — a `BeyondTheWalls`-authored merchant rendered `InCity`, with the
  village home framing and a `you_sell` of grain;
- `miller_with_flour.txt` — `you_sell` quoting a non-edible trade, proving §5's generalisation
  reaches the sheet.

`turn.j2` needs **no** new prose. Grain and flour are ordinary stackable items, the mill and the
bakehouse are ordinary places, and buying is the same `offer_item` the model already does. If a
model needs to be told how to buy flour, §5 was implemented wrong.

## 11. Risk ledger

| risk | mitigation |
|---|---|
| `Presence` leaks — a `BeyondTheWalls` trader is targeted, perceived, or spends an LLM turn | one test per seam (§9); the filter goes in `snapshot`/`attention`/percept-recipient construction, not at call sites, so there is one place to get right |
| the chain starves — a day with no merchant means no grain means no bread three days later | the weekday roster guarantees ≥1 grain seller on 5 of 7 days; millers and bakers hold buffer stock across days (their `holds` persist — only stall *pitch* stock was ever swept). Watch `chain:` counts for a downward trend across a 7-day run |
| the margins are wrong and the miller or baker goes broke | §6's yields are declared in data, not code — retune without a rebuild. M5d's tuning milestone exists for exactly this |
| the player never sees any of it, because it happens at Kindling while they sleep | the Wool Gate → Wickmarket road is 130 m and the whole chain runs along it. This is why that pairing ships first. If it still reads as invisible, move baking later rather than adding narration |
| a road trader despawns mid-conversation | `Depart` flips presence **on arrival at the gate**, and the conversation floor (`features/implemented/conversation_floor.md`) already holds an actor in place while talking — verify it outranks the round leg, and if not, gate the flip on "not in a conversation" |
| deleting `restock` for bread while `provisions`/`fish` still conjure looks half-finished | it *is* half-finished, honestly and deliberately. The doc comment on `restock_unchained_trades` names the two chains still owed so the next person knows it is a queue, not an oversight |
