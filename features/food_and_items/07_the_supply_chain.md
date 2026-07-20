# The supply chain: Seven Lofts, road merchants, grain, flour, and cloth

Status: **scoped for M5; not started.**

M3 deliberately cheats twice: `Round::restock` creates food at Kindling and
`Round::close_books` resets wallets at the Watch. M5 replaces the bread half of those cheats with a
visible, buffered supply chain. It does not simulate farms, harvests, every wage, or every workshop
outside the walls.

The corrected chain takes **two days**, not one morning:

```text
Day N
  beyond the walls
      │ fixed road-party manifest (the declared item source)
      ▼
  gate ── visible cart ──> Seven Lofts
                              │
                              │ Betriss Skep buys and stores grain
                              │ Bertran Hobbe later buys grain
                              ▼
                         Wool Gate mill face
                              │ grain becomes flour
                              │ Averil Quern buys flour
                              ▼
                      Ansel Quern's bakehouse
                         Watch → Kindling bake

Day N+1
  bakehouse ──> Wickmarket bread stall ──> a funded city buyer

M5d adds the other direction:
  Brede wool ──> Ewart Skell ──> Ombreval cloth ──> the same road cart leaves
```

Seven Lofts is what makes this possible. Grain is delivered to a store, not teleported through
three trades before breakfast. Its inventory persists across the three weekdays with no scheduled
arrival.

M5 has one price rule:

> **Every sale uses the catalog's fixed `price_sparks` value. M5 has no road-quality simulation. A
> later event may delay a party or reduce what arrives; it must never modify a price.**

There is no scarcity multiplier, road-quality multiplier, bargaining price, or dynamic-price hook
in this milestone.

---

## 1. Binding decisions

These decisions are requirements, not suggestions left to implementation:

1. **The cast is fixed.** Road parties are five hand-authored people, not procedural travellers.
2. **Only those five people live beyond the walls.** Existing farmers, millers, merchants, and
   cargo workers remain residents with their current homes and rounds.
3. **The gate is the simulation boundary.** Nav still ends at the gate nodes.
4. **Seven Lofts is the persistent grain buffer.** It is not replaced by a generic gate stall.
5. **The chain is asynchronous.** An arriving grain batch can become bread no earlier than the next
   Dayspring.
6. **Stock procurement and hunger are different intents.** A baker buying flour does not eat it.
7. **A stall sells matching items its vendor actually holds.** Transform outputs do not require a
   second registration step.
8. **Only named producers transform stock.** This feature does not make every miller, baker, or
   cloth worker run the whole chain.
9. **The road boundary is the only new recurring item source/sink.** Inside the walls, grain, flour,
   wool, cloth, and bread appear only through a seed, purchase, or declared transform.
10. **Prices stay fixed.** Supply trouble is represented by empty shelves and late carts.

Fish and the tavern pot remain separate, explicitly named cheats. M5c removes all magically
restocked loaves, including the four rye loaves currently hidden in the `provisions` template.

---

## 2. The lore anchor: keep and deepen Seven Lofts

### 2.1 What is already canonical

`lore/places/03_new_places_and_infrastructure.md` already gives the feature its best location:

- Seven Lofts is a defended compound between the Wool and Stone gate routes;
- grain is dried, sampled, turned, guarded, and released there;
- the Chapter rents bays but does not own the entire food supply;
- the delayed opening of the Chapter's reserve in F.183 gives stored grain political weight;
- the place has hoists, sealed doors, grain dust, cats, fire rules, repairs, and contested memories.

`assets/world/places.json` already makes **Seven Lofts** a named navigable place. M5 uses that place;
it does not introduce “the grain warehouse” under another name.

The cast is equally specific:

| actor | id | existing lore used by M5 |
|---|---:|---|
| **Betriss Skep** | `p008s` | grain dealer; buys in bulk, sells by small measure, extends short credit, carries a tally stick, and is already trying to recover sacks sent to the wrong loft |
| **Bertran Hobbe** | `e7mil` | master miller; carts his own flour and already supplies the Quern bakehouse |
| **Ansel Quern** | `danqn` | owns and runs the quarter's common oven |
| **Averil Quern** | `davqn` | performs the night bake: in at Watch, dough set by Kindling, first loaves at Dayspring |
| **Ewart Skell** | `e1skl` | master draper at Draper's Reach; puts out Brede wool and takes cloth back |
| **Clemence Crake** | `fp6ck` | resident wholesale merchant whose lore explicitly joins Brede wool to outbound Ombreval cloth |
| **Renn Crake** | `fr9ck` | resident cargo broker and the natural city-side organizer |

Those are not flavor references. Betriss, Bertran, Averil, and Ewart are the preferred vendors or
producers in data, by actor id.

### 2.2 Proposed lore patch when M5 is implemented

Keep the Seven Lofts section and its history. Make four narrow updates alongside the milestone that
needs them:

1. In `lore/places/03_new_places_and_infrastructure.md`, change “scarcity and price stories” to
   **“scarcity, rationing, and release-order stories.”** The famine lever changes who can get grain
   and when, not the catalog price.
2. Deepen Betriss Skep's sheet so her household remains in Bell-and-Sluice streets but her working
   counter and rented bay are at Seven Lofts. Preserve the tally stick, short credit, and
   wrong-loft problem; those details are ideal for the mechanic.
3. Materialize the bakehouse as **Ansel Quern's common oven**, rather than authoring a generic new
   bakehouse. Averil's existing night-bake paragraph becomes the schedule.
4. Replace Ansel's hypothetical “price climbs / loaf goes to three sparks” line with a shortage
   consequence, for example: if Bertran's delivery fails, the bake is cut short and the quarter
   shouts at Ansel's door. This keeps the conflict while respecting fixed catalog prices.

The two new road merchants should know the appropriate resident counterparties and Betriss. Their
sheets should mention a particular family relationship, route, cargo obligation, and opinion about
the road. No broad lore rewrite is needed.

A future famine feature can reserve or release a physical Seven Lofts bay. M5 only establishes the
inventory state that feature would act on.

---

## 3. Fixed road parties

M5 ships two parties and five new characters:

| party | members | scheduled arrivals | city relationships | incoming cargo | intended return load |
|---|---:|---|---|---|---|
| **Brede / Wool Gate** | one minor merchant + two ambient carters | Highmarket and Fourth | merchant is kin or factor to Clemence Crake; Renn brokers the cart | rye, wheat; raw wool from M5d | broadcloth |
| **Lantern Road / Stone Gate** | one minor merchant + one ambient carter | Second and Fifth | merchant has an authored tie to Ewart Skell or a Crake factor | rye and wheat | kersey |

There is no scheduled arrival on Bellday, Lowmarket, or Seventh. That gap is deliberate; Seven
Lofts covers it.

The River Gate road, Harne Gate, Reed Postern, salt, charcoal, honey, and small farm sellers are
future extensions. Do not enroll the existing seven farmers or three millers as off-map actors just
because their occupation lore mentions villages or mills beyond the walls. In M5 they remain city
residents. Do not rebake their homes.

Each new character requires a normal authored sheet and stable id before M5a can ship. A
`merchant` or `cargo_worker` occupation may describe the person, but **party membership**, not
occupation mapping or an alternative title, gives them the road schedule. This avoids changing the
rounds of every resident merchant or cargo worker.

---

## 4. The road boundary and the visible cart

### 4.1 Deterministic manifests

Immediately before a scheduled arrival, while the party is still `BeyondTheWalls`, a
`boundary_exchange` does two things:

1. consumes cloth and any unsold incoming cargo carried out on the previous trip;
2. creates that trip's fixed incoming manifest in the leader's holds.

This is the declared recurring item source and sink. Item ids are deterministic from
`party_id + trip_number + manifest_slot`. The event is traced. Loading and unloading do **not**
reset the merchant's wallet; the off-map principal's accounts are outside this simulation.

Starting manifests:

| party | M5a manifest | M5d addition |
|---|---|---|
| Brede | 3 measures rye grain, 1 measure wheat grain | 4 bundles raw wool |
| Lantern Road | 1 measure rye grain, 2 measures wheat grain | none |

The manifests are tuning inputs, not prices. If the 14-day acceptance run starves or floods the
chain, adjust manifest quantities, storage targets, transform yields, or purchase caps.

Seven Lofts receives one historical day-zero seed of **4 rye + 2 wheat measures**. It is created
once by world seeding, never restored at Kindling or Watch. That buffer bootstraps a save whose
first day has no road arrival and makes the compound matter immediately.

### 4.2 The cart is part of M5a

A road arrival without a cart does not satisfy the visible feature. M5a adds a presentation-only
`RoadCart` record to the public snapshot:

- one cart per present road party, keyed by party id;
- follows the leader with a fixed offset;
- has no collision and is not independently targetable;
- uses sacks/bales when the leader holds grain or wool, bolts when the leader holds cloth, and an
  empty load otherwise;
- appears and disappears atomically with the party.

Cargo remains in the merchant leader's normal `holds`. The cart is a view of that stock, not a
second container. This does require a small Bevy host change to spawn, update, and despawn the cart
presentation.

---

## 5. Presence is a world rule, not a snapshot trick

`Presence` is persistent character state:

```rust
pub enum Presence {
    InCity,
    BeyondTheWalls,
}
```

All existing characters default to `InCity`. The five road-party members seed as
`BeyondTheWalls` and retain wallets, holds, relationships, and memories while absent.

Add a central `World::is_present(actor_id)` predicate and use it at every world-facing seam:

- public actors, owned items, pending offers, and item references in `WorldSnapshot`;
- `characters_within`, percept recipients, `you_see`, and sound/speech recipients where physical
  presence is required;
- action targets, gestures, `tell_way`, offers, conversations, and direct `go_to` targets;
- attention eligibility, idle/priority scheduler lanes, and queued cognition;
- movement, rounds, hunger/meal logic, vendor/keeper binding, queues, and census totals.

An absent actor cannot be found indirectly through an item or offer they own. Filtering only
`WorldSnapshot.actors` is insufficient.

When a party departs, clear or cancel its movement targets, market queues, meal/stock intents,
gestures, pending offers, scheduler priority, and other transient city state. Departure waits until
every member is at the gate, no member is in a conversation, and no member has an in-flight action.
Then all members and the cart transition atomically. Arrival is also atomic.

The party controller is the only code allowed to move a `BeyondTheWalls` actor to a gate. Normal
actions reject absent targets.

This changes public-world semantics, so bump the world/snapshot revision. Tests must cover both
actor and owned-item leakage.

---

## 6. Road schedules bypass occupation routing

Do not add a `road_trader` occupation-to-archetype mapping. The current route builder selects by
explicit actor route or occupation; an alternative title such as “Road trader” has no routing
effect, and mapping `merchant` or `cargo_worker` would catch residents.

Add explicit `road_parties` data to `assets/world/rounds.json`. The party controller synthesizes
the same ordinary leg shape used by rounds, but from party membership:

```json
{
  "id": "brede_wool_gate",
  "leader": "<authored merchant id>",
  "members": ["<merchant>", "<carter 1>", "<carter 2>"],
  "gate": "The Wool Gate",
  "only_on": ["highmarket", "fourth"],
  "legs": [
    {"from": "kindling",  "at": "gate",            "doing": "arrive"},
    {"from": "dayspring", "at": "Seven Lofts",     "doing": "trade"},
    {"from": "lamplight", "at": "gate",            "doing": "depart"}
  ]
}
```

The Stone Gate party uses the same offices and its own gate. Before M5d it remains at Seven Lofts
until its return-to-gate leg. M5d inserts a High Wick trade leg at Draper's Reach into both party
routes.

At Kindling, the controller performs the boundary exchange and places the whole party at its gate.
At Dayspring the leader opens the cart pitch at Seven Lofts. At Lamplight the party walks to its
gate and departs under the safety conditions in Section 5.

Followers share the leader's destination without becoming vendors or stock owners. Party presence
and schedule state persist across save/load.

---

## 7. Sell inventory and stock procurement

### 7.1 Separate listings from magical restock

`TradeSpec.stock` currently means both “things this trade may sell” and “things to conjure.” Split
it:

```rust
struct TradeSpec {
    listings: Vec<ItemMatcher>,
    restock: Vec<StockSpec>,       // only explicitly unchained stock
    conjure_per_serving: Option<ItemKind>,
}
```

A stall's sellable inventory is a live scan of its current vendor's holds matching `listings`.
Remove `FoodStall.stock_ids`, or keep it only as a derived cache that is invalidated on every
transfer/transform. A transform output is sellable immediately because its producer holds it.

Track ids created by `restock` separately as `conjured_ids`. The next legacy restock may sweep only
those ids; it must never delete bought, seeded, boundary-loaded, or transformed stock.

At M5c:

- `bread.listings` contains rye and wheat loaves; `bread.restock` is empty;
- remove loaves from the `provisions` restock and listing until a provisioner distribution route is
  designed;
- fish still restocks herring and eel, explicitly marked as an unbuilt wharf chain;
- tavern stew remains `conjure_per_serving`, licensed by the never-empty-pot lore.

`food.json` is embedded with `include_str!`, so changes require a rebuild. The spec and developer
notes must not call it runtime-tunable.

### 7.2 Split buying from eating

Refactor the current one-unit `try_purchase` into a generic market transaction. The transaction
moves items and sparks atomically and returns a receipt; it does not decide why the buyer wanted the
item.

```rust
enum PurchasePurpose {
    Meal,
    Stock { targets: Vec<StockTarget> },
}

struct StockTarget {
    matcher: ItemMatcher,          // exact kind + required metadata
    desired_quantity: u32,
    max_spend_sparks: u32,
}
```

- `Meal` chooses one affordable edible unit. The meal ladder consumes it only after a successful
  purchase.
- `Stock` buys the target's deficit, cheapest matching stack first, up to quantity, budget, and
  wallet limits. It stores the result and never calls eating.
- Matching includes metadata: rye grain cannot silently satisfy a wheat target.
- The atomic cross-check still asserts that the buyer's debit equals the seller's credit.
- A vendor may buy from another trade, but not from itself or its own stall.

A `MarketErrand` is a resumable ladder intent: choose the configured source, walk there, wait for it
to open, buy, then clear or retry. Player-directed actions, conversations, and explicit `go_to`
retain precedence; stock errands run before the ordinary work round and hunger idle behavior.
Conversation pauses an errand rather than cancelling it.

Starting target caps:

| buyer | source | target |
|---|---|---|
| Betriss | an arriving cart at Seven Lofts | store up to 8 rye and 4 wheat measures |
| Bertran | Betriss's Seven Lofts counter | hold up to 2 rye and 1 wheat measures awaiting milling |
| Averil | Bertran's Wool Gate counter | hold up to 3 rye and 1 wheat flour sacks awaiting the night bake |
| Ewart *(M5d)* | Brede cart at Draper's Reach | hold up to 4 raw-wool bundles |
| Brede merchant *(M5d)* | Ewart's cloth counter | acquire 1 broadcloth bolt for departure |
| Lantern Road merchant *(M5d)* | Ewart's cloth counter | acquire 1 kersey bolt for departure |

Targets are initial tuning values. They must live in data and use exact metadata.

### 7.3 Named counters and binding

| counter or worksite | purpose | preferred actor | active window |
|---|---|---:|---|
| arriving road cart, Seven Lofts | sells the leader's incoming grain | road-party leader | Dayspring–High Wick on its weekdays |
| Betriss's grain counter, Seven Lofts | sells persistent stored grain | `p008s` | Dayspring–High Wick |
| Bertran's mill counter, Wool Gate | sells flour he actually milled | `e7mil` | Waning–Lamplight |
| Ansel Quern's bakehouse | baking worksite, not a shop | producer `davqn`; keeper `danqn` | Watch–Kindling |
| Wickmarket bread stall | sells Averil's loaves | `davqn` | Dayspring–Waning |
| Ewart's counter, Draper's Reach *(M5d)* | raw-wool receipt, production, cloth sale | `e1skl` | High Wick–Waning |

Binding requires all of the following: the preferred actor is present, today is allowed, the
counter is open, and the actor's **currently active** leg is `Trade` at that site. Merely having
some matching leg elsewhere in the weekly round is not enough.

Using preferred actor ids prevents output from becoming stranded on one producer while a different
occupation-matched actor binds as vendor.

---

## 8. Transforms and the two-day timetable

A transform is a resumable timed job, not an instant side effect of buying:

```rust
struct TransformSpec {
    id: String,
    site: String,
    producer: ActorId,
    consumes: Vec<StockSpec>,
    produces: Vec<StockSpec>,
    allowed_offices: Vec<Office>,
    seconds: f64,
}

struct TransformJob {
    spec_id: String,
    inputs: Vec<ReservedInput>,    // item id + exact reserved quantity
    progress_seconds: f64,
}

struct ReservedInput {
    item_id: ItemId,
    quantity: u32,
}
```

Required quantities are split into job-reserved stacks when the job starts, consumed only when it
completes, and returned to the producer if the job is cancelled. Completion creates output in that
same named producer's holds. Save/load preserves the job and its reserved inputs. Only the actor
named by `producer` may run it.

Catalog additions and fixed prices:

| kind | metadata | price_sparks |
|---|---|---:|
| `grain` | `grain=rye` / `grain=wheat` | 3 / 6 |
| `flour` | `grain=rye` / `grain=wheat` | 5 / 9 |
| `wool` | — | 8 |
| `cloth` | `grade=kersey` / `grade=broadcloth` | 14 / 40 |

Existing loaf prices remain **2 sparks for rye and 4 for wheat**.

Recipes:

| transform | producer and site | consumes | produces |
|---|---|---|---|
| rye milling | Bertran, Wool Gate mill face | 1 rye grain | 3 rye flour |
| wheat milling | Bertran, Wool Gate mill face | 1 wheat grain | 3 wheat flour |
| rye night bake | Averil, Ansel Quern's bakehouse | 1 rye flour | 5 rye loaves |
| wheat night bake | Averil, Ansel Quern's bakehouse | 1 wheat flour | 4 wheat loaves |
| kersey *(M5d)* | Ewart, Draper's Reach | 1 raw wool | 1 kersey |
| broadcloth *(M5d)* | Ewart, Draper's Reach | 3 raw wool | 1 broadcloth |

The gross margins are internally possible at fixed prices:

- one rye flour sack costs Averil 5 and yields five 2-spark loaves: 5 sparks gross margin;
- one wheat flour sack costs 9 and yields four 4-spark loaves: 7 sparks gross margin;
- one rye grain measure costs Bertran 3 and yields three 5-spark flour sacks: 12 sparks gross margin;
- one wheat grain measure costs 6 and yields three 9-spark sacks: 21 sparks gross margin.

These are simplified gross margins, not claims about labor, fuel, tolls, or rent.

The canonical schedule is:

| time | event |
|---|---|
| Day N Kindling | scheduled road party loads outside, appears at its gate, and walks in |
| Dayspring | cart reaches Seven Lofts; Betriss buys grain into persistent storage |
| High Wick | Bertran buys available grain from Betriss; the cart may continue to Draper's Reach in M5d |
| Waning | Bertran mills at the Wool Gate face and opens his flour counter |
| Lamplight | Averil buys flour; road party returns to its gate and departs |
| Watch → Kindling | Averil works the night bake at Ansel's common oven |
| Day N+1 Dayspring | Averil carries finished loaves to her Wickmarket stall |

The one-time Seven Lofts seed lets Bertran work even on days without a delivery. There is no
requirement that each morning's loaf use that morning's grain.

For end-to-end testing, transform trace records input and output item ids so a provenance graph can
follow one grain batch through destroyed and newly created stacks without adding provenance keys to
catalog metadata.

---

## 9. Cloth is produced, not conjured

The old sketch introduced cloth only as a return load, with no source. M5d builds the minimum
workshop side needed to make it real:

1. the Brede manifest brings raw wool;
2. at Draper's Reach, Ewart buys that wool from the road merchant;
3. Ewart runs the exact kersey or broadcloth recipe needed to restore his configured output target;
4. the road merchant buys an existing bolt from Ewart;
5. the bolt remains in the leader's holds and is visible on the departing cart;
6. the next `boundary_exchange` consumes it outside the walls.

The Lantern Road party can buy kersey made from wool left by an earlier Brede trip. If its target is
unavailable, it leaves without cloth; no fallback bolt is minted.

Clemence and Renn remain important resident counterparties in lore and conversation, but M5 does
not silently transfer stock through them. A later wholesale-contract feature can add that layer.

---

## 10. Money after the nightly reset

### 10.1 Transitional M5a–c behavior

M5a splits legacy ledger bookkeeping from chain inventory. Until M5d, the existing resident wallet
refill may remain for non-chain actors, but it must never:

- reset a road trader's wallet;
- delete Seven Lofts stock;
- delete grain, flour, or transformed loaves;
- reset Betriss, Bertran, or Averil to a vendor template.

Starting working capital, seeded once:

| actor | sparks |
|---|---:|
| Betriss | 30 |
| Bertran | 24 |
| Averil | 24 |
| each road-party leader | 25 |
| each carter | ordinary personal seed |
| Ewart *(M5d)* | 48 |

These are minimum initial values and explicit data, not nightly targets. For the named resident
chain actors, the same numbers are their initial working reserves during household settlement;
ordinary residents use their day-zero seeded wallet as their reserve unless authored otherwise.

### 10.2 M5d household settlement

M5d deletes `close_books` and replaces it with `settle_households` at Watch:

1. Resident actors below a 2-spark household floor become recipients.
2. Resident actors above their configured working reserve contribute only their surplus.
3. Transfers are deterministic and move only the amount recipients need. They conserve sparks.
4. If the surplus pool is insufficient, an explicit institutional wages/alms payment creates only
   the shortfall and logs the exact minted amount.
5. Road-party members are excluded from both resident redistribution and institutional payroll.
6. No stock is reset or deleted by settlement.

This is an aggregate representation of wages, rents, alms, and shared household pots. It is honest
about the remaining mint instead of pretending every actor has a fully simulated employer.

The census records resident supply separately from present road-party wallets, plus road-trade
sparks entering and leaving through sales. For a deterministic 14-day fake-backend run:

- resident-held sparks finish within ±10% of their day-zero total;
- institutional payroll mints at most 2% of the initial resident supply over the run;
- no resident is at zero at two consecutive Watches;
- no chain vendor fails two consecutive scheduled cycles solely for lack of sparks;
- every sale uses the exact static catalog price.

If this fails, tune manifests, yields, purchase caps, working reserves, or the household floor.
**Do not tune prices in response to road conditions or inventory.**

---

## 11. Milestones

### M5a — The cart reaches Seven Lofts

Ships:

- central `Presence` semantics and all filters/cleanup in Section 5;
- two explicit road parties and five authored character sheets;
- deterministic boundary manifests;
- the `RoadCart` snapshot and Bevy presentation;
- `grain` and its fixed catalog rows;
- the Seven Lofts historical seed, Betriss's counter, and her targeted lore update;
- `listings` versus `restock`;
- generic `Meal` versus `Stock` purchases and resumable stock errands;
- direct party routing rather than occupation routing.

Betriss buys incoming grain and Bertran can buy and hold it; milling does not exist yet.

Acceptance:

- on Highmarket, exactly the three-person Brede party and one cart appear at Wool Gate, trade at
  Seven Lofts, return to the gate, and disappear together;
- on Second, exactly the two-person Stone party does the same;
- on Bellday, Lowmarket, and Seventh no party arrives;
- after departure, none of their actors, still-owned items, offers, targets, percepts, queue entries,
  or cart leaks into the public world;
- Seven Lofts stock changes by real purchases and remains unchanged through Watch;
- asking the road merchant about origin, kin, cargo, and road yields authored answers.

### M5b — Grain becomes flour

Ships:

- `flour`;
- reserved, timed, saveable `TransformJob`;
- Bertran's explicit Seven Lofts → Wool Gate route;
- the named Wool Gate mill counter;
- live vendor-hold inventory.

Acceptance follows one rye item from the road leader to Betriss to Bertran, then three flour items
from Bertran to Averil. Input is consumed once, output is created once, the quoted sale price comes
from the catalog, and no other miller transforms anything.

### M5c — The Quern night bake replaces bread restock

Ships:

- Ansel Quern's common bakehouse as a named worksite;
- Averil's Watch-to-Kindling route and baking jobs;
- Wickmarket binding to Averil's real held loaves;
- deletion of every loaf template from both `bread` and `provisions` restock;
- the targeted Ansel/Averil lore alignment in Section 2.2.

Acceptance follows a grain batch delivered on Day N through milling and night baking to a funded
buyer's loaf purchase on Day N+1. Grepping the restock path finds no loaf creation.

Then rerun M4 unchanged: **Ilse has one spark, cannot buy a two-spark rye loaf, buys the one-spark
herring, and eats it.** M5 must not rewrite that acceptance story into an impossible loaf purchase.

### M5d — Wool returns as cloth and the reset dies

Ships:

- `wool` and `cloth`;
- Ewart's raw-wool purchase and two transforms at Draper's Reach;
- road merchants' exact cloth stock targets;
- departing cart load presentation;
- boundary consumption of the return load;
- `settle_households` and deletion of `close_books`;
- 14-day chain and coin census.

Acceptance proves raw wool enters on a Brede manifest, becomes an existing cloth item in Ewart's
holds, is bought at its fixed catalog price, leaves on the visible cart, and is consumed only at the
next off-map boundary exchange. The full 14-day criteria in Section 10.2 pass.

---

## 12. Observability and invariants

Extend `--trace-food` rather than adding a parallel tracer. Events:

- `boundary_load` / `boundary_unload` with party, trip, item ids, kinds, and quantities;
- `road_in` / `road_out` with all member ids, gate, and cart state;
- `stock_errand` and `sale` with purpose, buyer, seller, exact metadata, quantity, and sparks;
- `transform_start` / `transform_finish` with spec, input ids, output ids, and producer;
- `cart_load` when the presentation category changes;
- `household_settlement` with donor transfers, recipients, and minted shortfall.

`food_summary()` gains:

- Seven Lofts rye/wheat quantities;
- holdings for Betriss, Bertran, Averil, and Ewart;
- present road parties and inbound/outbound cart load;
- transforms completed today;
- resident spark total, road-party spark total, gate in/out, redistribution, payroll minted, and
  zero-wallet streaks.

Standing invariants:

- grain, flour, wool, cloth, and loaf quantities change only through initial seed, boundary
  exchange, atomic purchase, consumption, or a declared transform;
- sparks change only through atomic transfer or logged institutional payroll;
- a transform cannot consume the same quantity twice, complete away from its site, or run under
  the wrong actor;
- a stall cannot sell an item its current vendor does not hold;
- absent actors and their owned state never appear in snapshots, percepts, targets, schedules, or
  census resident totals;
- party transition is atomic and its cart load agrees with the leader's held cargo.

---

## 13. Fixtures and compatibility

With no presence field, actors deserialize as `InCity`. Fixture worlds with `nav: None` enroll no
rounds or road parties. Existing prompt fixtures should therefore remain byte-stable; verify rather
than regenerate them blindly.

Add focused fixtures/tests for:

- a present road merchant with a beyond-the-walls home and grain in `you_sell`;
- Betriss at Seven Lofts with persistent grain;
- Bertran selling non-edible flour from transformed holds;
- Averil selling transformed rye and wheat loaves;
- an absent actor whose owned item and pending offer are both filtered;
- a transform paused and restored through save/load.

`turn.j2` needs no new market prose. Grain and flour are normal held items; procurement is sim
behavior, not a new LLM verb.

Expected code blast radius includes the sim world/snapshot revision, round ladder, food schema and
seed, item catalog, save/load state, authored character assets, road-party data, and the Bevy cart
projection. M5a should not be described as a round-only data edit.

---

## 14. Risks and mitigations

| risk | mitigation |
|---|---|
| Seven Lofts empties on non-arrival days | one-time persistent seed, four scheduled arrivals per week, explicit storage and buyer targets, 14-day stock census |
| same-morning ordering becomes flaky | the canonical acceptance spans Day N to Day N+1; buffers decouple each producer |
| resident merchants or cargo workers accidentally leave town | explicit party membership; no occupation-wide `road_trader` mapping |
| transformed output is stranded behind stale `stock_ids` | sell inventory is derived from the active vendor's current holds |
| every baker or miller transforms stock | each transform names one producer id |
| a road trader disappears mid-conversation | departure waits for the whole party to be at the gate and idle, then transitions atomically |
| an absent actor leaks through an owned item or offer | central presence predicate plus actor/item/offer/target tests |
| cloth exists only as a return-load prop | raw wool is a boundary manifest; Ewart buys it and performs a declared transform |
| carts are promised but invisible | `RoadCart` presentation is in M5a acceptance, with host work budgeted |
| residents or chain firms go broke | persistent working capital, conservative household redistribution, bounded logged payroll, 14-day thresholds |
| old restock deletes real chain stock | only ids recorded in `conjured_ids` may be swept |
| a bad road silently changes prices | no price multiplier exists; only schedule/manifests/availability may change |
| embedded data is mistaken for hot reload | acceptance rebuilds after `food.json` or `items.json` changes |
