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

M5 has one rule for **mechanical market transactions**:

> **Every code-driven `sale` uses the catalog's posted `price_sparks` value. M5 has no
> road-quality or scarcity price multiplier. A later event may delay a party or reduce what
> arrives; it must never silently rewrite a posted price.**

That rule does **not** ban haggling. Player/NPC conversation may produce a counter-offer, gift,
short credit, deliberate overpayment, or a cheat, and those outcomes remain part of the fun. An
accepted conversational exchange uses the existing offer/transfer path; its one or more completed
steps are traced as `item_transfer`, not as a mechanical `sale`. It may therefore differ from the
posted price. The
distinction is deliberately visible in observability: `sale` means deterministic ladder
procurement at the catalog price; negotiated transfers retain the terms the participants agreed.
M5 does not add an enforceable debt ledger: short credit remains a conversational promise until
later item/spark transfers fulfill it.

There is no automatic scarcity multiplier, road-quality multiplier, or dynamic-price hook in this
milestone.

---

## 1. Binding decisions

These decisions are requirements, not suggestions left to implementation:

1. **The cast is fixed.** Road parties are five hand-authored people, not procedural travellers.
2. **Only those five people live beyond the walls.** Existing farmers, millers, merchants, and
   cargo workers remain residents with their current homes and rounds.
3. **The gate is the simulation boundary.** Nav still ends at the gate nodes.
4. **Seven Lofts is the persistent grain buffer.** It is not replaced by a generic gate stall.
5. **The chain is asynchronous.** An arriving grain batch may finish baking overnight, but cannot
   be sold from the bread stall earlier than the next Dayspring.
6. **Stock procurement and hunger are different intents.** A baker buying flour does not eat it.
7. **A stall sells matching items its vendor actually holds.** Transform outputs do not require a
   second registration step.
8. **Only named producers transform stock.** This feature does not make every miller, baker, or
   cloth worker run the whole chain.
9. **The road boundary is the only new recurring item source/sink.** Global quantities of grain,
   flour, wool, cloth, and loaves change only through the one-time seed, the boundary, consumption,
   or a declared transform; purchases and gifts only move existing quantities. Institutional
   payroll and boundary cash settlement are the only new spark source/sink paths.
10. **Posted prices stay fixed; bargaining stays possible.** Supply trouble is represented by
    empty shelves and late carts. Conversational exchanges may still settle on other terms.

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
| **Ewart Skell** | `e1skl` | master draper at The Draper's Reach; puts out Brede wool and takes cloth back |
| **Clemence Crake** | `fp6ck` | resident wholesale merchant whose lore explicitly joins Brede wool to outbound Ombreval cloth |
| **Renn Crake** | `fr9ck` | resident cargo broker and the natural city-side organizer |

Those are not flavor references. Betriss, Bertran, Averil, and Ewart are the preferred vendors or
producers in data, by actor id.

### 2.2 Proposed lore patch when M5 is implemented

Keep the Seven Lofts section and its history. Make four narrow updates alongside the milestone that
needs them:

1. In `lore/places/03_new_places_and_infrastructure.md`, change “scarcity and price stories” to
   **“scarcity, rationing, release-order, and bargaining stories.”** The famine lever changes who
   can get grain and when; it does not automatically rewrite the catalog's posted price.
2. Deepen Betriss Skep's sheet so her household remains in Bell-and-Sluice streets but her working
   counter and rented bay are at Seven Lofts. Preserve the tally stick, short credit, and
   wrong-loft problem; those details are ideal for the mechanic.
3. Materialize the bakehouse as **Ansel Quern's common oven**, rather than authoring a generic new
   bakehouse. Averil's existing night-bake paragraph becomes the schedule.
4. Replace Ansel's hypothetical automatic “price climbs / loaf goes to three sparks” line with a
   shortage consequence, for example: if Bertran's delivery fails, the bake is cut short and the
   quarter shouts and haggles at Ansel's door. Ansel can still demand three sparks in conversation;
   the simulation simply does not mutate the catalog row for everyone.

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

### 4.1 Deterministic manifests and trip float

At a scheduled Kindling, while a party is still `BeyondTheWalls`, its `boundary_exchange` runs in
this exact order:

1. consume all configured commercial cargo still held by the leader from the previous trip;
2. settle the leader's wallet to the party's configured `trip_float_sparks`;
3. create the next trip's fixed incoming manifest in the leader's holds.

The boundary spec lists the commercial cargo matchers; M5 uses grain, raw wool, and cloth. These
are business goods, so any matching quantity carried out is delivered to the off-map principal
regardless of whether it came from the manifest, an ordinary sale, or a negotiated exchange. The
boundary never consumes unrelated personal items.

Wallet settlement represents the principal taking receipts and advancing purchasing money. A
surplus is removed and traced as `road_cash_out`; a deficit is created and traced as
`road_cash_in`. The starting trip float is **25 sparks per leader**. This is neither the old nightly
wallet reset nor an attempt to model off-map accounts: it occurs only at a successful trip
boundary, is symmetric, and is included in the coin-conservation equation.

The boundary is the declared recurring item and road-cash source/sink. Manifest item ids are
deterministic from `party_id + trip_number + manifest_slot`; trip numbers advance only when a new
trip is successfully staged. Every consume, cash adjustment, and load is traced.

The party's manifest, commercial-cargo matchers, and trip float live with its topology/schedule in
the `road_parties` row in `assets/world/rounds.json`; they reuse the exact `StockSpec`/`ItemMatcher`
shape validated by the food document. M5d extends those same rows rather than adding a second road
manifest registry.

Starting manifests:

| party | M5a manifest | M5d addition |
|---|---|---|
| Brede | 3 measures rye grain, 1 measure wheat grain | 4 bundles raw wool |
| Lantern Road | 1 measure rye grain, 2 measures wheat grain | none |

The manifests are tuning inputs, not prices. If the acceptance run starves or floods the chain,
adjust manifest quantities, storage/production targets, transform yields, or purchase budgets.

Betriss's holds receive one historical day-zero seed of **4 rye + 2 wheat measures**, reported as
Seven Lofts stock while she owns it. M5 does not invent an unowned place container. The seed is
created once by world seeding, never restored at Kindling or Watch. That buffer bootstraps a world
whose first day has no road arrival and makes the compound matter immediately.

### 4.2 The cart is part of M5a

A road arrival without a cart does not satisfy the visible feature. M5a adds a presentation-only
record to the public snapshot:

```rust
struct RoadCart {
    party_id: PartyId,
    leader_id: ActorId,
    load: Vec<CartLoadKind>,
}
```

- There is one cart per present road party, keyed by party id.
- It follows the leader's host-side transform at a fixed offset; no cart position is authoritative
  sim state.
- It has no collision and is not independently targetable.
- `load` is a sorted set, so a mixed load can show sacks, bales, and bolts at once. It is derived
  from the leader's held grain, wool, and cloth; an empty list shows an empty cart.
- It appears and disappears in the same world transition as the party.

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
`BeyondTheWalls` and retain wallets, holds, relationships, and memories while absent. Presence is
stored in `World`, not inferred from the current round or hidden only while making a snapshot.

Add a central `World::is_present(actor_id)` predicate and use it at every world-facing seam:

- public actors, owned items, pending offers, and item references in `WorldSnapshot`;
- `characters_within`, percept recipients, `you_see`, and sound/speech recipients where physical
  presence is required;
- action targets, gestures, `tell_way`, offers, conversations, and direct `go_to` targets;
- attention eligibility, idle/priority scheduler lanes, and queued cognition;
- movement, rounds, hunger/meal logic, vendor/keeper binding, queues, and census totals.

An absent actor cannot be found indirectly through an item or offer they own. Filtering only
`WorldSnapshot.actors` is insufficient.

The party controller owns an explicit phase per party:

```rust
enum PartyPhase {
    BeyondTheWalls,
    StagedOutsideGate { trip_number: u64 },
    InCity,
    Returning,
    DeparturePending,
}
```

The phase lives in `Round`; physical `Presence` and the public `RoadCart` records live in `World`.
`World::enter_party` and `World::leave_party` perform one atomic mutation and increment the public
`world_revision` change counter once. Here `world_revision` means the existing monotonic snapshot
revision, not a serialized schema version.

All members of one party always share presence. `BeyondTheWalls` and `StagedOutsideGate` require
every member to be absent and no cart; the other three phases require every member to be present and
exactly one cart. Seed/config validation rejects duplicate membership or a leader outside `members`.

The lifecycle is binding:

1. At a scheduled Kindling, a `BeyondTheWalls` party performs the boundary exchange and becomes
   `StagedOutsideGate`. It remains absent and invisible, and its needs remain frozen like those of
   every absent actor.
2. At Dayspring, the gate opens and `enter_party` places every member at the gate, sets
   `Presence::InCity`, creates the cart, and changes the phase to `InCity` atomically. Because
   off-map breakfast and water are not item-simulated, this traced entry also sets the members to
   `HUNGER_MAX` and `THIRST_MAX`. The party then walks its route; office changes never teleport it
   between sites.
3. At Lamplight, the route changes to the gate and the phase becomes `Returning`.
4. Once every member is at the gate, no member is in conversation, and no member has an in-flight
   action, the phase becomes `DeparturePending`. On the first safe engine tick,
   `World::leave_party` removes all members and the cart atomically, even if the clock has passed
   Snuffing.

While returning or departure-pending, a member receives no ordinary round, meal, or stock errand.
The return-to-gate intent outranks a new explicit `go_to`, which is rejected with a leaving-city
reason; nearby conversation and item offers remain allowed and can delay the safe departure.
Departure cleanup cancels movement targets, market queues, meal/stock intents, gestures, pending
offers, and other transient city state. The engine clears scheduler priority, novelty, queued
cognition, and any conversation bookkeeping in the same transition. A cognition completion for an
actor who is no longer present is discarded rather than applied.

If a party has not departed by its next scheduled Kindling, trace `road_trip_missed`; do not run a
second boundary exchange, advance the trip number, or stage another arrival. Its next eligible
trip is the first scheduled weekday after actual departure.

The party controller is the only code allowed to move a `BeyondTheWalls` actor to a gate. Every
action rejects an absent initiator as well as an absent target. The engine supplies the controller
with conversation and in-flight-action status; the round layer requests transitions but does not
guess at engine state.

This changes public-world semantics but does not imply a disk-schema version. Tests assert that the
existing monotonic `world_revision` advances exactly once per atomic party transition and cover
actor, owned-item, offer, target, and stale-cognition leakage.

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
  "stage_at": "kindling",
  "enter_at": "dayspring",
  "return_at": "lamplight",
  "legs": [
    {"from": "dayspring", "at": "Seven Lofts",   "doing": "trade"},
    {"from": "lamplight", "at": "The Wool Gate", "doing": "stand"}
  ]
}
```

The three phase fields are party-controller triggers; `legs` retain the existing ordinary round
shape and valid `doing` values. The first leg is the physical walk from the entry gate. The Stone
Gate party uses the same offices and its own gate. Before M5d it remains at Seven Lofts until its
return-to-gate leg. M5d inserts a High Wick `trade` leg at **The Draper's Reach** into both party
routes.

Kindling stages an absent party outside the gate as described in Section 5. At Dayspring the party
appears at the gate, walks to Seven Lofts, and only opens its cart pitch when the leader reaches the
configured counter radius. At Lamplight it walks back and departs under the safety conditions in
Section 5.

The controller processes every crossed office boundary in chronological order, so a coarse clock
pump that crosses Kindling and Dayspring stages before it enters. The shipped game initializes on
Highmarket at Dayspring; on a **fresh world initialized exactly at an eligible `enter_at`**, the
controller performs that day's not-yet-run stage and entry once before the first snapshot. A fresh
world initialized later in the day does not retroactively teleport a party in; it waits for the next
scheduled trip.

Followers share the leader's destination without becoming vendors or stock owners.

M5 does **not** add disk save/load; the current game has no such system. Party phase, trip number,
jobs, and reservations are ordinary authoritative sim state and survive engine polls, clock pauses,
and cloning `World` and `Round` together in tests. If persistence is added later, its versioned save
format must include them, but that is not an M5 acceptance requirement.

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

A stall's sellable inventory is a live scan of the current vendor's **uncommitted** held quantity
matching `listings`. Remove `FoodStall.stock_ids`; if profiling later proves a cache is needed, the
`World` inventory mutator, rather than callers in `Round`, must invalidate it. A transform output is
sellable immediately because its producer holds it.

`ItemMatcher` means kind equality **and entire metadata-map equality**, not subset matching or a
wildcard. Rye grain cannot satisfy wheat grain, kersey cannot satisfy broadcloth, and an unexpected
extra key is a content error rather than a new invisible stock class.

Track legacy stock created by `restock` in private operational state, not in catalog metadata:

```rust
struct LegacyRestockLot {
    item_id: ItemId,
    original_vendor: ActorId,
    source_id: String,             // the exact legacy stall/restock source
}
```

`World` owns this map keyed by item id. `Round` registers a lot when it conjures legacy stock and
asks `World` to sweep eligible lots for that same `source_id`; action callers do not send
invalidation messages back to `Round`.

The inventory helper owns the lifecycle of each marker. On a partial transfer, consumption, or
transform, the original-vendor remainder retains the marker but the moved quantity or output does
not. Before a whole stack leaves, merges, or is consumed, the marker is cleared. A legacy restock
sweep may remove a marked id only if the item is still held by `original_vendor` and has no pending
offer; an item gifted back later is real stock and is not re-marked. This closes the case where an
LLM-negotiated whole-stack transfer would otherwise be deleted at the next Kindling.

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
item. It is the only path that emits `sale`.

```rust
enum PurchasePurpose {
    Meal,
    Stock { plan_id: String },
}

struct StockPlan {
    id: String,
    buyer: ActorId,
    source_trade: String,
    targets: Vec<StockTarget>,
    max_spend_sparks: u32,
}

struct StockTarget {
    matcher: ItemMatcher,          // exact kind + entire metadata map
    desired_quantity: u32,
}
```

- `Meal` chooses one affordable edible unit. The meal ladder consumes it only after a successful
  purchase.
- `Stock` considers targets in data order and buys each deficit, then matching source items in
  `(catalog unit price, item id)` order, up to quantity, visit budget, and the buyer's uncommitted
  spark quantity. It stores the result and never calls eating.
- A target's held quantity includes quantities currently reserved by a transform. This prevents a
  producer from duplicating its procurement while a job is in progress.
- Matching includes metadata: rye grain cannot silently satisfy a wheat target.
- The complete plan is validated before mutation. Each receipt line records source item id,
  destination item id after split/merge, quantity, catalog unit price, and line total. The buyer's
  debit and seller's credit equal the receipt total; a failed validation moves neither items nor
  sparks.
- A matching stack without an exact catalog price is not mechanically sellable and produces a
  traced content error; the transaction never guesses a default.
- A vendor may buy from another trade, but not from itself or its own stall.
- Negotiated offers do not call this routine. They use the same central item/spark transfer helpers
  and emit `item_transfer` with the agreed terms, so haggling and gifts remain possible without
  masquerading as catalog-price sales.

A `MarketErrand` is a resumable ladder intent: choose the configured source, walk there, wait for it
to open, buy, then clear or retry. It is created only when the source has an eligible scheduled
window; an absent road cart does not make a buyer stand at Seven Lofts for days. There is at most
one failed attempt per source, office, and unchanged source-stock fingerprint (matching uncommitted
item ids and quantities). `source_absent`, `closed`, `no_matching_stock`, `unpriced_stock`, and
`insufficient_funds` are distinct traced results. A `no_matching_stock` errand may retry in the
same office only after a transfer or transform changes that fingerprint—this is how a road merchant
waiting for Ewart's active job buys its finished bolt without polling every tick. Other failures
record the next eligible office/day and yield.

Player-directed actions, conversations, explicit `go_to`, pressing needs, and curfew retain
precedence. Stock errands run before the ordinary round and non-pressing hunger idle behavior.
Conversation pauses an errand rather than cancelling it.

Starting target caps and whole-visit budgets:

| buyer | source | target, in declaration order | max spend per visit |
|---|---|---|---:|
| Betriss | an arriving cart at Seven Lofts | 8 rye grain; 4 wheat grain | 30 |
| Bertran | Betriss's Seven Lofts counter | 2 rye grain; 1 wheat grain | 12 |
| Averil | Bertran's Wool Gate counter | 3 rye flour; 1 wheat flour | 24 |
| Ewart *(M5d)* | Brede cart at The Draper's Reach | 4 raw wool | 32 |
| Brede merchant *(M5d)* | Ewart's cloth counter | 1 broadcloth | 40 |
| Lantern Road merchant *(M5d)* | Ewart's cloth counter | 1 kersey | 14 |

Targets are initial tuning values. They must live in data and use exact metadata.
`stock_plans`, listing/counter specs, budgets, offices, and site radii belong in the embedded
`assets/world/food.json`; actor, trade, and place references are validated when the round seeds.

### 7.3 Named counters and binding

| counter or worksite | purpose | preferred actor | exact offices | required active leg |
|---|---|---:|---|---|
| arriving road cart, Seven Lofts | sells the leader's incoming grain | road-party leader | `dayspring`, `high_wick` on its weekdays | `trade` at Seven Lofts |
| Brede road cart, The Draper's Reach *(M5d)* | sells raw wool | Brede leader | `high_wick`, `waning` on Brede weekdays | `trade` at The Draper's Reach |
| Betriss's grain counter, Seven Lofts | sells persistent stored grain | `p008s` | `dayspring`, `high_wick` | `trade` at Seven Lofts |
| Bertran's mill counter, The Wool Gate | sells flour he actually milled | `e7mil` | `waning` | `work` at The Wool Gate |
| Ansel Quern's common oven | baking worksite, not a shop | producer `davqn`; keeper `danqn` | `watch`, `kindling` | `work` at the common oven |
| The Wickmarket bread stall | sells Averil's loaves | `davqn` | `dayspring`, `high_wick`, `waning` | `trade` at The Wickmarket |
| Ewart's counter, The Draper's Reach *(M5d)* | cloth sale | `e1skl` | `high_wick`, `waning` | `work` at The Draper's Reach |

Counter binding requires the preferred actor to be present, the weekday and exact office to be
allowed, the actor to be within the configured site radius, and the actor's **current** leg to match
the row. A worksite uses its own activation rule and does not become a shop merely because work is
in progress. Merely having a matching leg elsewhere in the weekly round is insufficient.

Using preferred actor ids prevents output from becoming stranded on one producer while a different
occupation-matched actor binds as vendor.

M5 authors explicit routes for Betriss (M5a), Bertran (M5b), Averil and Ansel (M5c), and Ewart
(M5d). In particular, Betriss's current `food_provisioner` route points at The Wickmarket and cannot
be reused as proof that she reaches Seven Lofts.

The two road-cart pitches use distinct offsets inside Seven Lofts. The Wool Gate mill face uses the
existing Wool Gate node plus a data offset. **Ansel Quern's common oven** is a named worksite at the
Querns' existing authored home/door node; M5 does not rebake city geometry. All data uses the exact
existing display names **The Wickmarket**, **The Wool Gate**, and **The Draper's Reach**.

---

## 8. Transforms and the two-day timetable

A transform is a resumable timed job, not an instant side effect of buying:

```rust
struct ProductionPlan {
    producer: ActorId,
    transforms: Vec<TransformSpec>, // deterministic preference order
    max_jobs_per_day: u32,
}

struct TransformSpec {
    id: String,
    site: String,
    consumes: Vec<StockSpec>,
    produces: Vec<StockSpec>,
    allowed_offices: Vec<Office>,
    work_minutes: u32,
    desired_output_quantity: u32,
}

struct TransformJob {
    job_id: String,
    spec_id: String,
    production_day: i64,
    start_slot: u32,
    inputs: Vec<ReservedInput>,    // item id + exact reserved quantity
    progress_work_minutes: f64,
}

struct ReservedInput {
    item_id: ItemId,
    quantity: u32,
}
```

Production specs and per-day start counters live in `Round`; active jobs live in private
`World::transform_jobs`, keyed by producer. This placement is required because action verbs receive
`&mut World` without `Round` and must still see reservations. `Round` starts, advances, finishes, or
cancels a job only through `World` inventory methods; jobs are authoritative but are not exposed in
the public snapshot.

The `ProductionPlan`/`TransformSpec` rows also live in `assets/world/food.json`; durations, targets,
and caps are embedded content requiring a rebuild, just like listings and stock plans.

Starting a job **does not split or transfer an input stack**. The item remains in the producer's
holds, preserving the existing “every item has one owner” and “no two same-stuff stackable stacks
per holder” invariants. Pending offers are commitments too, so mechanical procurement cannot sell
an item out from under an active negotiation:

```text
uncommitted(item)
  = item.quantity
  - sum(active transform reservations for item.id)
  - pending offer quantity for item.id
```

Job start atomically reserves exact quantities from matching uncommitted items in item-id order.
Every inventory operation—mechanical sale, negotiated offer, gift, eating, and another
transform—must go through the central inventory helper. A new operation may use only uncommitted
quantity. Accepting/retracting an offer consumes/releases that offer's own commitment; completing
or cancelling a transform does the same for its reservations. An intentional giver action that
uses an offered quantity first retracts or shrinks the offer under the existing action semantics. A
whole-stack transfer fails while another commitment exists; an explicit partial transfer may move
no more than the uncommitted remainder.

On completion, the helper decrements the original input stacks by the reserved quantities, removes
zero stacks, and inserts or merges output into that same named producer's holds using the normal
stacking rule. Cancelling a job only deletes its reservation; there is no detached stack to “give
back.” A transform receipt records all consumed item ids and quantities and each created-or-merged
destination id and quantity.

When an output does not merge, its legal item id is deterministically derived from producer id,
transform id, production day, start slot, and output slot through the existing id-mint/hash helper;
a collision is an invariant failure. When it merges, the receipt names the pre-existing destination
id instead. `job_id` is derived from the same tuple and makes completion idempotent.

Only the actor named by `ProductionPlan.producer` may run its transforms, and a producer may have at
most one active job. A job starts only when all of these are true:

- the producer is present, within the configured site radius, stationary, not in conversation or a
  conflicting movement/market/meal intent, and on the required `work` leg;
- the current office is in `allowed_offices`;
- all inputs are uncommitted;
- the actor has not spent `max_jobs_per_day` start slots; and
- held output plus active-job output plus one recipe batch is no greater than
  `desired_output_quantity`.

The last condition is the production backpressure. Eligible transforms are checked in their data
order after each completion; no random selection is involved.

The engine supplies the active-conversation set to the production controller on each pump rather
than asking `Round` to infer it from prompt or scheduler state.

Progress uses elapsed **game-clock minutes**, not wall seconds or a real-time deadline. It accrues
only during the overlap with a qualifying office/leg/site interval and pauses on movement,
conversation, interruption, absence, or closure. Clock jumps are processed across crossed office
and day boundaries, so jumping from Waning to Watch cannot credit the whole interval to a closed
mill. No job produces more than once, even if a single pump crosses its completion time.

Catalog additions and posted prices (all four kinds are stackable and non-edible):

| kind | display / plural | visual key | metadata | `price_sparks` selectors |
|---|---|---|---|---|
| `grain` | grain measure / grain measures | `grain_sack` | `grain: [rye, wheat]` | `grain=rye`: 3; `grain=wheat`: 6 |
| `flour` | sack of flour / sacks of flour | `flour_sack` | `grain: [rye, wheat]` | `grain=rye`: 5; `grain=wheat`: 9 |
| `wool` | bundle of raw wool / bundles of raw wool | `wool_bale` | — | default: 8 |
| `cloth` | bolt of cloth / bolts of cloth | `cloth_bolt` | `grade: [kersey, broadcloth]` | `grade=kersey`: 14; `grade=broadcloth`: 40 |

Do not add a default price to metadata-sensitive kinds. The supply-chain seed, manifest, stock-plan,
and transform-spec validators require the listed metadata, so an incorrectly untyped grain, flour,
or cloth item fails content validation rather than acquiring the wrong price. Existing loaf posted
prices remain **2 sparks for rye and 4 for wheat**.

Recipes and initial production backpressure:

| data order | producer and site | consumes | produces | allowed offices | work | target |
|---:|---|---|---|---|---:|---:|
| 1 | Bertran, The Wool Gate mill face | 1 rye grain | 3 rye flour | `waning` | 45 min | 3 rye flour |
| 2 | Bertran, The Wool Gate mill face | 1 wheat grain | 3 wheat flour | `waning` | 45 min | 3 wheat flour |
| 1 | Averil, Ansel Quern's common oven | 1 rye flour | 5 rye loaves | `watch`, `kindling` | 45 min | 15 rye loaves |
| 2 | Averil, Ansel Quern's common oven | 1 wheat flour | 4 wheat loaves | `watch`, `kindling` | 45 min | 4 wheat loaves |
| 1 | Ewart, The Draper's Reach *(M5d)* | 3 raw wool | 1 broadcloth | `high_wick`, `waning` | 45 min | 1 broadcloth |
| 2 | Ewart, The Draper's Reach *(M5d)* | 1 raw wool | 1 kersey | `high_wick`, `waning` | 45 min | 1 kersey |

Here rye/wheat grain and flour use exact `grain=rye|wheat` metadata, cloth uses
`grade=kersey|broadcloth`, and existing loaves use `flour=rye|wheat`.

The producer caps are two job starts per workday for Bertran, four per night for Averil, and two
per workday for Ewart. Starting a job spends the slot; cancellation does not refund it. A job
started before a day boundary retains its original production-day key for the cap.

The gross margins are internally possible at posted prices:

- one rye flour sack costs Averil 5 and yields five 2-spark loaves: 5 sparks gross margin;
- one wheat flour sack costs 9 and yields four 4-spark loaves: 7 sparks gross margin;
- one rye grain measure costs Bertran 3 and yields three 5-spark flour sacks: 12 sparks gross margin;
- one wheat grain measure costs 6 and yields three 9-spark sacks: 21 sparks gross margin.

These are simplified gross margins, not claims about labor, fuel, tolls, or rent.

The canonical schedule is:

| time | event |
|---|---|
| Day N Kindling | scheduled road party exchanges cargo/cash outside and stages invisibly |
| Dayspring | gate opens; party appears, walks to Seven Lofts, and Betriss buys grain into persistent storage |
| High Wick | Bertran buys available grain from Betriss; in M5d the cart continues to The Draper's Reach, where Ewart buys wool and starts work |
| Waning | Bertran mills and Averil buys available flour; in M5d Ewart finishes cloth and the road leader buys its return target |
| Lamplight | road party returns toward its gate; Averil heads to the common oven |
| Watch → Kindling | Averil works the night bake at Ansel's common oven |
| Day N+1 Dayspring | Averil carries finished loaves to her Wickmarket stall |

The one-time Seven Lofts seed lets Bertran work even on days without a delivery. There is no
requirement that each morning's loaf use that morning's grain.

For end-to-end testing, purchase and transform receipts provide quantity edges between source and
destination item ids. If an output merges into an existing stack, the receipt names that existing
destination id. This is a truthful commodity-flow graph, not lot provenance: after stack merging,
M5 does not claim that a particular loaf contains a particular historical grain item.

---

## 9. Cloth is produced, not conjured

The old sketch introduced cloth only as a return load, with no source. M5d builds the minimum
workshop side needed to make it real:

1. the Brede manifest brings raw wool;
2. at The Draper's Reach, Ewart buys that wool from the road merchant;
3. Ewart runs the exact kersey or broadcloth recipe needed to restore his configured output target;
4. the road merchant buys an existing bolt from Ewart at the posted price, unless an authored
   conversation negotiates a different transfer;
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
| each road-party leader | 25 trip float |
| each carter | ordinary personal seed |
| Ewart *(M5d)* | 48 |

These are minimum initial values and explicit data, not nightly targets. For the named resident
chain actors, the same numbers are their initial working reserves during household settlement;
ordinary residents use their day-zero seeded wallet as their reserve unless authored otherwise.

Road leaders have the boundary trip-float settlement from M5a onward. Their wallets may rise or
fall inside the walls, including through haggling, but return to 25 only at the next successful
off-map boundary exchange.

### 10.2 M5d household settlement

M5d adds an economic classification that is sim state but is not rendered into prompts:

```rust
enum EconomicClass {
    Resident,
    Visitor,
    RoadParty,
}
```

The field deserializes to `Resident` for backward-compatible fixtures. The player is always
`Visitor`; character content explicitly marks nonresident pilgrims—including Ilse—as `Visitor`;
and road-party membership forces `RoadParty` at seed validation. Do not re-infer the class from an
occupation on every tick. Visitors keep their actual wallets but neither donate to nor receive
from household settlement. This preserves M4's one-spark Ilse fixture.

M5d deletes `close_books` and replaces it with `settle_households` at Watch:

1. Immediately before settlement, update each resident's zero-wallet Watch streak. This sampling
   point defines the acceptance criterion.
2. Resident actors below a 2-spark household floor become recipients.
3. Resident actors above their effective working reserve contribute only their surplus. The
   effective reserve is at least the 2-spark household floor, so a recipient cannot also be a
   donor.
4. Donors and recipients are processed in actor-id order. Transfers move only the amount recipients
   need and conserve sparks. A donor contributes only uncommitted sparks, so settlement never
   invalidates a pending negotiated offer.
5. If the surplus pool is insufficient, an explicit institutional wages/alms payment creates only
   the shortfall and logs the exact minted amount.
6. Visitors and road-party members are excluded from both redistribution and institutional payroll.
7. No stock is reset or deleted by settlement.

This is an aggregate representation of wages, rents, alms, and shared household pots. It is honest
about the remaining mint instead of pretending every actor has a fully simulated employer.

Every item/spark exchange, including a negotiated one, uses the atomic transfer helper. Mechanical
sales and conversational exchanges conserve sparks; the latter may also be a zero-spark gift. At
every census point the exact global equation is:

```text
resident + visitor + all road-party sparks
  = initial global sparks + institutional payroll + road_cash_in - road_cash_out
```

“All road-party sparks” includes absent parties. Presence is reported separately, so departure
cannot make money disappear from the accounting view.

For a deterministic 14-day fake-backend, clock-only run, sampled after each Watch settlement:

- resident-held sparks finish within ±10% of their day-zero total;
- institutional payroll mints at most 2% of the initial resident supply over the run;
- no resident's pre-settlement zero streak reaches two Watches;
- no chain vendor fails two consecutive scheduled cycles solely for lack of sparks;
- every `sale` receipt uses the exact static catalog price; and
- the global spark equation balances exactly.

A 56-day clock-only soak additionally asserts that each buyer/output holding stays at or below its
configured target (plus at most the explicitly represented active recipe batch), every producer has
at most one job, and every buyer has at most one errand. A road leader's wallet stays at or below
trip float plus that trip's posted manifest value and is exactly at trip float immediately after
boundary settlement. These bounds apply to the clock-only harness; deliberate conversational gifts
may exceed a target without authorizing automatic deletion.

If this fails, tune manifests, yields, purchase caps, working reserves, or the household floor.
**Do not add an automatic price response to road conditions or inventory.** Authored characters
remain free to haggle about an individual exchange.

---

## 11. Milestones

### M5a — The cart reaches Seven Lofts

Ships:

- central `Presence` semantics and all filters/cleanup in Section 5;
- two explicit road parties and five authored character sheets;
- deterministic boundary manifests and trip-float accounting;
- the `RoadCart` snapshot and Bevy presentation;
- `grain` and its posted-price catalog rows;
- the Seven Lofts historical seed, Betriss's counter, and her targeted lore update;
- `listings` versus `restock`;
- generic `Meal` versus `Stock` purchases and resumable stock errands;
- transfer-safe legacy restock markers; and
- direct party routing and Betriss's explicit route rather than occupation routing.

Betriss buys incoming grain and Bertran can buy and hold it; milling does not exist yet.

Acceptance:

- on Highmarket, exactly the three-person Brede party stages invisibly at Kindling, appears with
  one cart at The Wool Gate at Dayspring, physically reaches Seven Lofts, returns, and disappears
  atomically;
- on Second, exactly the two-person Stone Gate party does the same;
- on Bellday, Lowmarket, and Seventh no party arrives;
- after departure, none of their actors, still-owned items, offers, targets, percepts, queue entries,
  or cart leaks into the public world;
- a conversation delayed past Snuffing delays departure but not forever; the party leaves on the
  first safe tick, and a stale cognition completion cannot bring an actor or action back;
- a party still present at its next scheduled Kindling logs one missed trip and receives neither a
  second manifest nor another trip number;
- each successful boundary exchange sets the leader to 25 sparks, with the exact difference traced
  as `road_cash_in` or `road_cash_out`;
- Seven Lofts stock changes by real purchases and remains unchanged through Watch;
- a whole and a partial negotiated transfer of legacy-restocked stock survive the next restock
  sweep; and
- an asset/prompt test proves that each road merchant's origin, kin, cargo obligation, and road
  opinion are present in their authored context. Asking about them in a live conversation is a
  manual content smoke test, not a nondeterministic automated assertion about LLM wording.

### M5b — Grain becomes flour

Ships:

- `flour`;
- in-place reservations and timed `TransformJob`;
- Bertran's explicit Seven Lofts → Wool Gate route;
- the named Wool Gate mill counter;
- live vendor-hold inventory.

In an isolated no-preexisting-flour fixture, acceptance follows receipt edges for one rye-grain
unit from the road leader to Betriss to Bertran, then **one stack containing three rye-flour units**
from Bertran to Averil. In the full world, where merges may reuse ids, acceptance checks the same
quantities and event chronology without claiming lot ancestry. Input is consumed once, output is
created once, every mechanical sale price comes from the catalog, and no other miller transforms
anything. A reserved input stays owned, cannot be transferred whole, can expose only its
uncommitted remainder, pauses away from the mill, and completes once across a clock jump.

### M5c — The Quern night bake replaces bread restock

Ships:

- Ansel Quern's common bakehouse as a named worksite;
- Averil's Watch-to-Kindling route and baking jobs;
- Wickmarket binding to Averil's real held loaves;
- deletion of every loaf template from both `bread` and `provisions` restock;
- the targeted Ansel/Averil lore alignment in Section 2.2.

Acceptance follows the quantity/receipt chain from grain delivered on Day N through milling and
night baking to a funded resident buyer's loaf purchase no earlier than Dayspring on Day N+1.
Grepping the restock path finds no loaf creation, and the production targets stay bounded when no
buyer comes.

Then rerun M4 unchanged: **Ilse has one spark, cannot buy a two-spark rye loaf, buys the one-spark
herring, and eats it.** M5 must not rewrite that acceptance story into an impossible loaf purchase.

### M5d — Wool returns as cloth and the reset dies

Ships:

- `wool` and `cloth`;
- Ewart's raw-wool purchase and two transforms at The Draper's Reach;
- road merchants' exact cloth stock targets;
- departing cart load presentation;
- boundary consumption of the return load;
- `EconomicClass`, `settle_households`, and deletion of `close_books`;
- 14-day chain/coin acceptance and a 56-day boundedness soak.

Acceptance proves raw wool enters on a Brede manifest, becomes an existing cloth item in Ewart's
holds, is bought by the deterministic ladder at its posted catalog price, leaves on the visible
cart, and is consumed only at the next off-map boundary exchange. The full criteria in Section
10.2 pass. A separate regression negotiates a non-catalog exchange successfully and proves that it
emits transfers but no `sale`.

---

## 12. Observability and invariants

Extend `--trace-food` rather than adding a parallel tracer. Events:

- `boundary_load` / `boundary_unload` with party, trip, item ids, kinds, and quantities, plus
  `road_cash_in` / `road_cash_out` with exact amounts;
- `road_stage`, `road_in`, `road_return`, `road_out`, and `road_trip_missed` with phase, all member
  ids, gate, trip number, and cart state;
- `stock_errand` with plan/source/result and next retry; `sale` with purpose, buyer, seller, every
  source/destination receipt line, exact metadata, quantities, unit prices, and totals;
- `item_transfer` for offers, gifts, and negotiated steps, including transferred item/spark
  quantities but no assertion that they equal a catalog price;
- `transform_start` / `transform_pause` / `transform_cancel` / `transform_finish` with spec,
  reserved or consumed item ids and quantities, output destination ids, producer, and work minutes;
- `cart_load` when the presentation category changes;
- `household_settlement` with donor transfers, recipients, and minted shortfall.

`food_summary()` gains:

- Seven Lofts rye/wheat quantities;
- held, uncommitted, transform-reserved, and offered quantities for Betriss, Bertran, Averil, and
  Ewart;
- every road party's phase, trip number, leader wallet, present/absent state, and cart load;
- active jobs and transforms completed by production-day key;
- resident, visitor, and all-road-party spark totals; boundary cash in/out; redistribution; payroll
  minted; and pre-settlement zero-wallet streaks.

Standing invariants for the completed M5 chain follow. Earlier sub-milestones permit only the
explicit transitional loaf restock and non-chain wallet refill named in Sections 7.1 and 10.1:

- global grain, flour, wool, cloth, and loaf quantities change only through initial seed, boundary
  exchange, consumption, or a declared transform; purchases and negotiated transfers conserve
  their global quantities;
- every item has exactly one owner, no same-stuff stackable duplicates share a holder, and total
  transform-plus-offer commitments for an item never exceed its quantity;
- sparks change only through an atomic conserving transfer, logged institutional payroll, or
  logged boundary cash adjustment, and the equation in Section 10.2 always balances;
- every `sale` uses the exact catalog unit price; `item_transfer` is explicitly exempt because
  bargaining, credit, gifts, and cheats are allowed;
- a transform cannot consume the same quantity twice, complete away from its site, run under the
  wrong actor, exceed the producer's daily cap, or consume/transfer reserved quantity elsewhere;
- a stall cannot sell an item its current vendor does not hold as uncommitted quantity;
- absent actors and their owned state never appear in snapshots, percepts, targets, schedules, or
  present-world totals;
- party transition is atomic, trip numbers advance only at staging, and cart load agrees with the
  leader's held cargo; and
- no production target, market intent, or road wallet grows without a configured bound.

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
- an absent actor whose item, pending offer, target, percept, and queued/stale cognition are all
  filtered or rejected;
- fresh-Dayspring bootstrap, coarse Kindling→Dayspring ordering, delayed departure, missed-trip
  suppression, and an atomic party/cart transition;
- deterministic target order, visit budgets, receipt ids, rollback on validation failure, and
  distinct stock-errand retry reasons;
- a reserved input that remains owned, blocks a whole transfer, permits only the uncommitted partial
  quantity, releases on cancellation, and survives cloning `World` and `Round` together;
- transform pause/resume at movement, conversation, site, office, and crossed-clock boundaries;
- legacy-restocked whole/partial item transfers that cannot be swept from the recipient;
- a pending offer protected from mechanical sale/transform, a non-catalog negotiated exchange, and
  a zero-spark gift, none logged as `sale`;
- `Resident`/`Visitor`/`RoadParty` settlement, including Ilse retaining her authored one spark; and
- exact spark conservation through resident sales, conversational transfers, payroll, road sales,
  and boundary float settlement.

Two pure-sim automated tests are the quantitative gate:

```sh
cargo test -p cathedral-sim supply_chain_14_day -- --nocapture
cargo test -p cathedral-sim supply_chain_56_day_soak -- --nocapture
```

They use the real authored assets, fake/no cognition actions, a deterministic clock starting on day
0 at Watch, and assert Sections 10 and 12 directly rather than parsing prose. The equivalent
developer-readable trace is:

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- \
  --fake --start-day 0 --start-office watch --seconds-per-day 120 \
  --watch-clock 14 --trace-food
```

The automated tests are the pass/fail authority; the CLI transcript is for diagnosis.

`turn.j2` needs no new market prose. Grain and flour are normal held items; procurement is sim
behavior, not a new LLM verb.

Expected code blast radius includes sim world/snapshot state, the engine/round transition seam,
round ladder, central inventory helpers, food schema and seed, item catalog, authored character
assets, road-party/route/worksite data, headless acceptance fixtures, and the Bevy cart projection.
It does not include a new persistence system. M5a should not be described as a round-only data edit.

---

## 14. Risks and mitigations

| risk | mitigation |
|---|---|
| Seven Lofts empties on non-arrival days | one-time persistent seed, four scheduled arrivals per week, explicit storage and buyer targets, 14-day stock census |
| same-morning ordering becomes flaky | the canonical acceptance spans Day N to Day N+1; buffers decouple each producer |
| resident merchants or cargo workers accidentally leave town | explicit party membership; no occupation-wide `road_trader` mapping |
| transformed output is stranded behind stale `stock_ids` | sell inventory is derived from the active vendor's current holds |
| reserved inputs violate item ownership/stacking | reservations annotate quantities in the producer's existing stacks; no detached input item exists |
| every baker or miller transforms stock, or one producer floods stock | each plan names one producer and has output targets plus daily job caps |
| a road trader disappears mid-conversation or overlaps its next trip | explicit party phases, safe-tick departure, and missed-trip suppression |
| an absent actor leaks through an owned item or offer | central presence predicate plus actor/item/offer/target tests |
| cloth exists only as a return-load prop | raw wool is a boundary manifest; Ewart buys it and performs a declared transform |
| carts are promised but invisible | `RoadCart` presentation is in M5a acceptance, with host work budgeted |
| residents or chain firms go broke | persistent working capital, conservative household redistribution, bounded logged payroll, 14-day thresholds |
| road merchants accumulate cash forever | every successful off-map exchange logs a symmetric settlement to fixed trip float; 56-day soak covers it |
| old restock deletes real or negotiated stock | operational restock markers are cleared on whole transfer/merge and never follow a buyer's partial split |
| stack merging makes false provenance claims | receipts record quantity edges and actual destination ids; M5 claims commodity flow, not lot ancestry |
| implementation accidentally removes haggling | only `sale` is posted-price constrained; negotiated transfer and gift regressions remain green |
| a bad road silently changes posted prices | no multiplier exists; only schedule, manifests, and availability may change, while individual bargaining remains allowed |
| acceptance depends on arbitrary LLM wording | authored facts are asset/prompt assertions; natural conversation is a manual smoke test |
| persistence scope expands M5 | jobs and party phases are authoritative in-memory state; versioned disk persistence is explicitly future work |
| embedded data is mistaken for hot reload | acceptance rebuilds after `food.json` or `items.json` changes |
