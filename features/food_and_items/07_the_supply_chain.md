# The supply chain: Seven Lofts, road merchants, grain, flour, and cloth

Status: **M5a–M5d implemented (2026-07-20).** The pure-sim and Bevy-host acceptance suites pass.
The required M5a screenshot review is still pending: the implementation session ran the exact
drive command below, but its sandbox could not connect to the available X display and winit stopped
with `XOpenDisplayFailed` before creating a session.

M3 deliberately cheated twice: `Round::restock` created food at Kindling and
`Round::close_books` reset wallets at the Watch. M5 replaces loaf restock with a visible, buffered
supply chain and replaces the wallet reset with household settlement. It does not simulate farms,
harvests, every wage, or every workshop outside the walls.

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
  Brede wool ──> Ewart Skell ──> persistent Ombreval cloth stock ──> a later cart leaves
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

M5 also deliberately collapses the earlier varietal split. The mechanically modelled food chain
is exactly **grain → flour → loaf**. Grain, flour, and loaves carry no varietal metadata, and every
loaf has the same posted price. Varietal names may still appear in lore or conversation, but they
do not select recipes, inventory classes, prices, or bread types.

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
6. **Buying has no purpose tag.** Hunger and stock procurement are separate planners around the
   same catalog-sale transaction. Held edible food may satisfy hunger regardless of how it was
   acquired; flour is not eaten because it is inedible, not because of its acquisition history.
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
11. **The food chain has no varietal axis.** Any grain can become the single flour item, and that
    flour can become only the single loaf item.

Fish and the tavern pot remain separate, explicitly named cheats. M5c removes all magically
restocked loaves, including the four loaves currently hidden in the `provisions` template.

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

### 2.2 Targeted lore alignment

Keep the Seven Lofts section and its history. Five narrow updates govern M5; all are aligned in the
implemented lore and asset fixtures:

1. In `lore/places/03_new_places_and_infrastructure.md`, change “scarcity and price stories” to
   **“scarcity, rationing, release-order, and bargaining stories.”** The famine lever changes who
   can get grain and when; it does not automatically rewrite the catalog's posted price.
2. Deepen Betriss Skep's sheet so her household remains in Bell-and-Sluice streets but her working
   counter and rented bay are at Seven Lofts. Preserve the tally stick, short credit, and
   wrong-loft problem; those details are ideal for the mechanic.
3. Keep Bertran Hobbe's now-aligned route: his working mill face is at the Wool Gate and takes
   bread-corn released from Seven Lofts, rather than grinding beyond the south wall. Preserve his
   white-with-flour walk down the Cut, Lowmarket weight ritual, family, and Quern relationship.
4. Materialize the bakehouse as **Ansel Quern's common oven**, rather than authoring a generic new
   bakehouse. Averil's existing night-bake paragraph becomes the schedule.
5. Replace Ansel's hypothetical automatic “price climbs / loaf goes to three sparks” line with a
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
| **Brede / Wool Gate** | one minor merchant + two ambient carters | Highmarket and Fourth | merchant is kin or factor to Clemence Crake; Renn brokers the cart | grain; raw wool from M5d | broadcloth |
| **Lantern Road / Stone Gate** | one minor merchant + one ambient carter | Second and Fifth | merchant has an authored tie to Ewart Skell or a Crake factor | grain | kersey |

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

### 4.1 Deterministic manifests and wallet floats

At a scheduled Kindling, while a party is still `BeyondTheWalls`, its `boundary_exchange` runs in
this logical order:

1. consume all configured commercial cargo still held by **any party member** from the previous trip;
2. settle every member's wallet to that actor's configured `wallet_float_sparks` value;
3. create the next trip's fixed incoming manifest in the leader's holds.

Those three steps are one atomic party-controller transaction over `Round` and `World`. Before
mutating either aggregate, build a `PartyTransitionPlan` containing each cargo removal and its
owner, every member wallet delta and credit destination, every new item destination, and the next
trip number, and validate every matcher, quantity, owner, and arithmetic result. The commit half
contains no fallible lookup, allocation decision, or arithmetic: it applies the planned `World`
writes and the `Round` trip/phase writes back-to-back
during one engine poll, with no snapshot or trace emitted between them. The controller batches the
`World` mutations under exactly one `world_revision` increment.

Resolve each non-merging manifest item id in manifest-slot order from
`party_id + next_trip_number + manifest_slot`, using the existing `mint_item_id` deterministic
probe-until-free loop. A collision with an authored or otherwise existing item id is a normal probe,
not a failed trip. Wallet credit destinations are precomputed by the same rules below; a cash
credit is not allowed to discover or mint an id during commit. Only after the whole plan validates
may the transaction apply the three steps and advance the trip number. A failed preflight emits
`boundary_exchange_failed` and leaves cargo, every member wallet, trip number, party phase,
presence, presence epochs, and therefore the derived cart view unchanged.

The boundary spec lists the commercial cargo matchers; M5 uses grain, raw wool, and cloth. These
are business goods, so any matching quantity carried out by the leader **or a carter** is delivered
to the off-map principal regardless of whether it came from the manifest, an ordinary sale, or a
negotiated exchange. The preflight scans members in configured member order and their items in
item-id order. The boundary never consumes unrelated personal items.

Wallet settlement represents the principal taking receipts and advancing purchasing and personal
money. A surplus is removed and traced as `road_cash_out`; a deficit is created and traced as
`road_cash_in`, in both cases with the member id. The starting floats are **25 sparks per leader**
and **4 sparks per carter**. This is neither the old nightly wallet reset nor an attempt to model
off-map accounts: it occurs only at a successful trip boundary, is symmetric, and is included in
the coin-conservation equation. Seed/config validation requires `wallet_float_sparks` to contain
exactly one nonnegative, `u32`-representable entry for every configured member and no other actor.

The boundary is the declared recurring item and road-cash source/sink. Trip numbers advance only
when a new trip is successfully staged. Every committed consume, cash adjustment, and load is
traced after the atomic apply.

The party's manifest, commercial-cargo matchers, and wallet-float map live with its
topology/schedule in the `road_parties` row in `assets/world/rounds.json`; they reuse the exact
`StockSpec`/`ItemMatcher` shape validated by the food document. M5d extends those same rows rather
than adding a second road manifest registry.

Starting manifests:

| party | M5a manifest | M5d addition |
|---|---|---|
| Brede | 4 measures grain | 4 bundles raw wool |
| Lantern Road | 3 measures grain | none |

The manifests are tuning inputs, not prices. If the acceptance run starves or floods the chain,
adjust manifest quantities, storage/production targets, transform yields, or purchase budgets.

Betriss's holds receive one historical day-zero seed of **6 grain measures**, reported as Seven
Lofts stock while she owns it. M5 does not invent an unowned place container. The seed is created
once by world seeding, never restored at Kindling or Watch. It keeps the chain useful for starts and
user overrides that do not coincide with an arrival, and makes the compound matter immediately even
though the committed M5a default opens on an arrival day.

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
  during snapshot construction from all configured commercial cargo held by **any party member**.
  Each matching presentation kind appears once regardless of quantity or owner. Moving cargo between
  the leader and a carter therefore does not change the visible load; transferring it out of the
  party or consuming it at the boundary does. An empty list shows an empty cart.
- It appears and disappears in the same world transition as the party.

Incoming cargo is created in the merchant leader's normal `holds`; ordinary transfers may later put
it in a carter's holds. The cart is a derived view of the party's inventories, not a second
container and not cached authoritative state. Snapshot construction joins the party topology and
phase in `Round` to the members' current holds in `World`; neither aggregate stores a separate cart
load. This does require a small Bevy host change to spawn, update, and despawn the cart presentation.

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
`BeyondTheWalls` and retain wallets, holds, relationships, durable memories, goals, and existing
`recent_history` while absent. Their unread inbox is city-transient and does not survive departure.
Presence is stored in `World`, not inferred from the current round or hidden only while making a
snapshot.

Each character also owns a monotonic `presence_epoch: u64`, seeded to zero. A successful party
entry or departure increments every affected member's epoch with checked arithmetic in the
transition preflight. Every cognition request is stamped with `(actor_id, presence_epoch)` when it
is dispatched. A completion may mutate state only while the actor is present and the stamped epoch
equals the actor's current epoch; the epoch check prevents a pre-departure request from becoming
valid again after a later re-entry.

Add a central `World::is_present(actor_id)` predicate and use it at every world-facing seam:

- public actors, owned items, pending offers, and item references in `WorldSnapshot`;
- `characters_within`, percept recipients, `you_see`, and sound/speech recipients where physical
  presence is required;
- action targets, gestures, `tell_way`, offers, conversations, and direct `go_to` targets;
- attention eligibility, idle/priority scheduler lanes, and queued cognition;
- movement, rounds, hunger/meal logic, vendor/keeper binding, queues, and census totals.

An absent actor cannot be found indirectly through an item or offer they own. Filtering only
`WorldSnapshot.actors` is insufficient.

The party controller owns one explicit state record per party:

```rust
struct PartyState {
    phase: PartyPhase,
    trip_number: u64,
}

enum PartyPhase {
    BeyondTheWalls,
    StagedOutsideGate,
    InCity,
    Returning,
    DeparturePending,
}
```

`PartyState` is the only storage for phase and trip number and lives in `Round`; the phase does not
duplicate the number. `trip_number` means the most recently successfully staged trip, seeds at
zero, and advances by checked addition exactly when `boundary_exchange` commits the next
`StagedOutsideGate` state. Physical `Presence`, member epochs, and transient city state live in
`World`. The public `RoadCart` is derived during snapshot construction from `PartyState`, party
topology, and the members' current inventories; there is no authoritative cart record or cached load
in either aggregate.

Every party transition—`boundary_exchange`, `enter_party`, `begin_return`,
`mark_departure_pending`, and `leave_party`—uses the same preflight/infallible-commit transaction
shape. It validates all `Round`, `World`, and engine-supplied prerequisites and arithmetic before
applying any write, then applies the writes with no observable intermediate state. A failure leaves
both aggregates and transient engine state unchanged and emits the transition's failure reason.
Each committed transition increments the public `world_revision` change counter exactly once. Here
`world_revision` means the existing monotonic snapshot revision, not a serialized schema version.

All members of one party always share presence. `BeyondTheWalls` and `StagedOutsideGate` require
every member to be absent, so the public snapshot derives no cart; the other three phases require
every member to be present, so it derives exactly one cart. Seed/config validation rejects duplicate
membership or a leader outside `members`.

The lifecycle is binding:

1. At a scheduled Kindling, a `BeyondTheWalls` party performs the boundary exchange and becomes
   `StagedOutsideGate`. It remains absent and invisible, and its needs remain frozen like those of
   every absent actor.
2. At Dayspring, the gate opens and the controller's `enter_party` transaction places every member
   at the gate, sets `Presence::InCity`, advances each member's presence epoch, and changes the phase
   to `InCity` atomically. The next snapshot consequently contains the one derived cart. Because
   off-map breakfast and water are not item-simulated, this traced entry also sets the members to
   `HUNGER_MAX` and `THIRST_MAX`. The party then walks its route; office changes never teleport it
   between sites.
3. At Lamplight, `begin_return` changes the phase to `Returning` and gives the party controller
   exclusive ownership of every member's movement. In the same transition it clears ordinary-round
   destinations, existing movement targets, food/water and market queue membership, meal and stock
   errands, curfew destinations, and gestures. Pressing hunger, thirst, curfew, and ordinary round
   rungs are suppressed until the party leaves; none may redirect a returning member. An active
   conversation or already-dispatched handoff may pause the return, but creates no replacement
   movement intent. New explicit `go_to` and queue/errand requests fail with `leaving_city`.
4. When a member reaches the gate, all still-pending offers involving that member are retracted and
   traced as `road_offer_expired`; a standing offer is never part of the safe-departure predicate.
   Offers may still be made and accepted during a nearby conversation before the gate, but anything
   left unresolved expires there. Once every member is at the gate, no member is in conversation,
   and no member has an in-flight action, the phase becomes `DeparturePending`. For this predicate,
   an in-flight action is an accepted gameplay mutation already executing across the engine/world
   seam, such as a dispatched handoff; an outstanding provider cognition request is not an action
   and does not pin the party in the city. On the first safe engine tick, the controller's
   `leave_party` transaction advances each member's presence epoch, sets every member to
   `BeyondTheWalls`, and removes their remaining transient city state atomically. The members and
   derived cart disappear from the next public snapshot, even if the clock has passed Snuffing.

`Returning` and `DeparturePending` are top-priority party modes, not ordinary ladder intents.
Departure cleanup removes any remaining transient city state. The engine clears scheduler priority,
novelty, queued cognition, conversation bookkeeping, and every departing member's unread inbox in
the same transition. If a provider request already drained inbox percepts into its in-flight request
record, departure discards that percept buffer: the engine retains only enough request identity and
epoch information to classify a later completion. The percepts are neither requeued nor graduated
into `recent_history`. Existing `memories`, goals, relationships, and `recent_history` remain
untouched. A cognition completion whose actor is absent **or whose stamped epoch is stale** is
archived for diagnostics and otherwise discarded: it cannot write memory, requeue its drained
inbox, enqueue an action, restore scheduler state, or affect a newly re-entered incarnation of that
actor.

If a party has not departed by its next scheduled Kindling, trace `road_trip_missed`; do not run a
second boundary exchange, advance the trip number, or stage another arrival. Its next eligible
trip is the first scheduled weekday after actual departure.

The party controller is the only code allowed to move a `BeyondTheWalls` actor to a gate. Every
action rejects an absent initiator as well as an absent target. The engine supplies the controller
with conversation and in-flight-action status; the round layer requests transitions but does not
guess at engine state.

This changes public-world semantics but does not imply a disk-schema version. Tests assert that the
existing monotonic `world_revision` advances exactly once per atomic party transition and cover
actor, owned-item, offer, target, and stale-cognition leakage across departure and re-entry.

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
  "wallet_float_sparks": {
    "<merchant>": 25,
    "<carter 1>": 4,
    "<carter 2>": 4
  },
  "commercial_cargo": [
    {"kind": "grain", "metadata": {}}
  ],
  "manifest": [
    {"kind": "grain", "metadata": {}, "quantity": 4}
  ],
  "legs": [
    {"from": "dayspring", "at": "Seven Lofts",   "doing": "trade"},
    {"from": "lamplight", "at": "The Wool Gate", "doing": "stand"}
  ]
}
```

The three phase fields are party-controller triggers; `legs` retain the existing ordinary round
shape and valid `doing` values. The first leg is the physical walk from the entry gate. The Stone
Gate party uses the same offices and its own gate. Before M5d it remains at Seven Lofts until its
return-to-gate leg. M5d keeps the cart at Seven Lofts through High Wick, then inserts a Waning
`trade` leg at **The Draper's Reach** into both party routes. This preserves the full Dayspring and
High Wick grain-sale window before the cart changes sites.

Kindling stages an absent party outside the gate as described in Section 5. At Dayspring the party
appears at the gate, walks to Seven Lofts, and only opens its cart pitch when the leader reaches the
configured counter radius. At Lamplight it walks back and departs under the safety conditions in
Section 5.

The controller processes every crossed office boundary in chronological order, so a coarse clock
pump that crosses Kindling and Dayspring stages before it enters. Initial construction invokes the
same idempotent trigger path because the engine does not ring the office it starts in:

- on a fresh world initialized exactly at an eligible `stage_at`, stage that party once before the
  first snapshot;
- on a fresh world initialized exactly at an eligible `enter_at`, perform that day's not-yet-run
  stage and entry once before the first snapshot; and
- on a fresh world initialized later in the day, do not retroactively teleport a party in; wait for
  its next scheduled trip.

The trigger key is party + absolute day + phase, so bootstrap and the first ordinary pump cannot
double-run it. M5a intentionally changes committed `default_config.ron` from day 0 (Bellday) to day
2 (Highmarket), retaining `start_office: "dayspring"`, so a fresh installation opens with the Brede
party entering. Tests construct their day and office explicitly and never inherit an ignored local
`config.ron`; a user override may of course start on another day.

Followers share the leader's destination without becoming vendors or stock owners.

M5 does **not** add disk save/load; the current game has no such system. Party state, presence
epochs, jobs, reservations, and completion receipts are ordinary authoritative sim state and
survive engine polls, clock pauses, and cloning `World` and `Round` together in tests. If
persistence is added later, its versioned save format must include them, but that is not an M5
acceptance requirement.

---

## 7. Sell inventory and stock procurement

### 7.1 Separate listings from magical restock

`TradeSpec.stock` currently means both “things this trade may sell” and “things to conjure.” Split
it:

```rust
struct TradeSpec {
    occupations: Vec<String>,
    listings: Vec<ItemMatcher>,
    restock: Vec<StockSpec>,       // only explicitly unchained stock
    conjure_per_serving: Option<ItemKind>,
}

struct ItemMatcher {
    kind: ItemKind,
    metadata: BTreeMap<String, String>,
}
```

`occupations` retains the existing legacy-stall vendor eligibility. M5 adds strict preferred-actor
counter rows alongside it; splitting `stock` must not remove occupation binding from fish,
provisions, or the two tavern pots. The road-party example above shows the M5a fields. M5d extends
Brede's `commercial_cargo` with raw wool and broadcloth matchers and extends its `manifest` with raw
wool; Lantern Road gains its kersey cargo matcher but no additional manifest item.

A stall's sellable inventory is a live scan of the current vendor's **uncommitted** held quantity
matching `listings`. Remove `FoodStall.stock_ids`; if profiling later proves a cache is needed, the
`World` inventory mutator, rather than callers in `Round`, must invalidate it. A transform output is
sellable immediately because its producer holds it.

Removing `stock_ids` must not make a vendor's own bread invisible to hunger. Derive a separate
`commercially_listed(actor, item)` view from exact listings and vendor binding: it is true when the
actor is the configured preferred vendor, or the current daily-bound vendor, for at least one trade
that lists the item. A daily binding remains the association until the next rebind even while its
counter is closed.

The autonomous meal planner scans **all uncommitted held edible quantity** before it creates or
continues a market errand. It prefers a non-listed item over a commercially listed one, then uses
stable item-id order, but listing is only a preference: if the board is the actor's only food, one
unit may be eaten from that real stack. The remaining quantity stays immediately sellable, and the
stock or production planner later observes the one-unit deficit normally. A listed item is not an
inventory commitment, and nothing records whether it originally arrived through meal procurement,
stock procurement, production, a gift, or any other transfer. Multiple matching listings do not
change the ordering, and no transform output needs item-id registration.

`ItemMatcher` means kind equality **and entire metadata-map equality**, not subset matching or a
wildcard. Kersey cannot satisfy broadcloth, and grain, flour, and loaves match only with an empty
metadata map. An unexpected extra key is a content error rather than a new invisible stock class.

Track legacy stock created by `restock` in private operational state, not in catalog metadata. A
stack can contain both real returned stock and newly conjured stock after the mandatory same-stuff
merge, so provenance is a quantity share rather than an all-or-nothing item marker:

```rust
struct LegacyRestockShare {
    original_vendor: ActorId,
    source_id: String,             // the exact legacy stall/restock source
    quantity: u32,                 // conjured units remaining in this stack
}
```

`World` owns a map from item id to a sorted vector of shares, coalesced by
`(source_id, original_vendor)`. `Round` never inserts a detached restock stack directly. It asks
`World::add_legacy_restock` to merge or create the normal inventory quantity and to add exactly the
conjured quantity to the destination's share. This permits one stack to contain unmarked real stock
plus shares from more than one legacy source without violating the one-stack invariant.

The inventory helper owns every share update. When quantity leaves through a transfer,
consumption, or transform, it decrements operational shares in stable
`(source_id, original_vendor)` order before unmarked quantity; provenance never follows the moved
quantity or a transform output. A whole-stack departure clears all shares. A same-holder merge
coalesces both stacks' shares into the surviving destination id. Thus an item transferred away and
later gifted back is unmarked real stock, but a subsequent restock may merge new marked quantity
into that same stack without re-marking the returned units.

Before adding a source's new template, `Round` asks `World` to sweep that `source_id`. For a share
whose stack is still held by `original_vendor`, the sweep removes at most
`min(share.quantity, uncommitted(item))`, decrements the share by the same amount, and removes a
zero stack/share normally. Offered or transform-reserved quantity is left in place; after the new
template merges, the conjured portion is bounded by one template plus outstanding legacy
commitments rather than growing once per day. Unmarked real quantity persists separately. A share
whose item is no longer held by its original vendor is a bug—the central transfer helper should
already have removed it—and produces a traced invariant failure rather than deleting somebody
else's stock.

At M5c:

- `bread.listings` contains the generic loaf matcher; `bread.restock` is empty;
- remove loaves from the `provisions` restock and listing until a provisioner distribution route is
  designed;
- fish still restocks herring and eel, explicitly marked as an unbuilt wharf chain;
- tavern stew remains `conjure_per_serving`, licensed by the never-empty-pot lore.

`food.json` is embedded with `include_str!`, so changes require a rebuild. The spec and developer
notes must not call it runtime-tunable.

### 7.2 One sale primitive, separate planners

Refactor the current one-unit `try_purchase` into a generic market transaction. The transaction
moves items and sparks atomically and returns a receipt; it does not decide why the buyer wanted the
item. It accepts the buyer, concrete counter binding, and requested item lines; it has no
intent discriminator, meal flag, or stock-plan id. It is the only path that emits `sale`.

```rust
enum StockSource {
    Counter { counter_id: String },
    CounterGroup { group_id: String },
}

struct CounterGroupSpec {
    id: String,
    counters: Vec<String>,         // stable counter ids, deterministic preference order
}

struct StockPlan {
    id: String,
    buyer: ActorId,
    source: StockSource,
    targets: Vec<StockTarget>,
    max_spend_sparks: u32,
}

struct StockTarget {
    matcher: ItemMatcher,          // exact kind + entire metadata map
    desired_quantity: u32,
}

struct CounterBindingKey {
    counter_id: String,
    seller: ActorId,
    session: CounterSession,
}

enum CounterSession {
    Daily { absolute_day: i64 },
    RoadTrip { party_id: PartyId, trip_number: u64 },
}

enum MarketErrandPhase {
    Approaching,
    WaitingForOpen,
    AtCounter,
}

enum MarketVisitEnd {
    TargetsSatisfied,
    BudgetExhausted,
    SourceIneligible,
    LastOfficePassed,
    NoRoute,
    TravelExpired,
    ReplacedByGoTo,
    Returning,
    UnpricedStock,
}

struct MarketErrand {
    plan_id: String,
    selected: Option<CounterBindingKey>,
    bindings_seen: Vec<CounterBindingKey>, // deduplicated, in first-selection order
    phase: MarketErrandPhase,
    spent_sparks: u32,
    last_failed_fingerprint: Option<AttemptFingerprint>,
    travel_deadline_real: Option<f64>,
}

struct ClosedMarketVisit {
    plan_id: String,
    bindings_seen: Vec<CounterBindingKey>,
    end_reason: MarketVisitEnd,
}
```

Every selling counter has a stable id. A direct `Counter` source permits purchases only from that
counter. A `CounterGroup` is an explicitly ordered set of interchangeable counters; it permits the
plan to use the first currently bound member in declaration order. The initial
`arriving_grain_carts` group contains the Brede and Lantern Road grain pitches, which never bind on
the same weekday but still have a deterministic order. Seed validation rejects a missing or
duplicate counter id, an empty group, a counter repeated within a group, and a group referenced by
no stock plan.

The counter, not the stock plan, references the seller-side trade and its listings. A plan never
searches every holder of a matching item and never selects a counter merely because its trade lists
the target kind. Thus Betriss may buy grain from `arriving_grain_carts`, while Bertran can buy the
same kind only from `betriss_grain_seven_lofts`.

- The meal planner first selects one uncommitted edible unit already held, preferring non-listed
  food and then stable item-id order. A held unit satisfies meal acquisition and therefore prevents
  or cancels a food-stall errand; the actor never walks away from edible inventory to buy another
  unit. A famished actor eats it immediately. A merely hungry actor also eats it unless its active
  next leg is home during the supper span and it is not yet within the home radius; in that case the
  meal rung starts no errand and falls through to the homeward round leg. Once home, it eats one held
  unit. Only an actor with no usable held edible asks a counter for one affordable unit; after a
  successful sale, the same eat-now/carry-home rule applies.
- This held-food check applies to both hunger rungs and immediately before a queued meal purchase
  commits. It deliberately supersedes M3's famished-only shortcut and hard exclusion of stall stock.
  Cancelling an already-active meal errand also removes the actor from its queue and serving slot so
  a newly acquired or newly uncommitted held unit cannot be followed by a stale sale.
- The stock planner considers targets in data order and buys each deficit, then matching source
  items in `(catalog unit price, item id)` order, up to quantity, visit budget, and the buyer's
  uncommitted spark quantity. It stores the result and never calls eating. A later hunger decision
  may consume an edible unit from those same holds; purchase history does not protect it.
- A target's held quantity includes quantities currently reserved by a transform. This prevents a
  producer from duplicating its procurement while a job is in progress.
- Matching includes the entire metadata map; generic grain, flour, and loaf targets require it to
  be empty.
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

A `MarketErrand` is a resumable ladder intent: resolve the configured `StockSource` to one concrete
counter id, walk there, wait for it to open, buy, then clear or retry. A direct counter keeps its
identity throughout the errand. A counter-group errand records the selected member; if that mobile
counter unbinds before purchase, the errand clears the selection and resolves the group again in its
declared order. A mobile-source errand is created only after a permitted counter actually binds: the
leader is present, on the matching leg, inside the site radius, and in an allowed office.
Merely reaching a scheduled office does not spend an attempt, so an early Dayspring check cannot
make Betriss ignore a cart that reaches Seven Lofts later that same office.

A counter binding has a stable session key. An ordinary fixed counter uses its absolute binding day;
a road counter uses party and trip number. A market visit begins when an errand selects such a
binding. The errand records every binding selected during that visit; `selected`, when present, must
occur in that bounded, deduplicated list. There is at most one active market errand per buyer, and its
`spent_sparks` is cumulative for the whole visit:

- `remaining_budget = plan.max_spend_sparks - errand.spent_sparks`, with checked arithmetic;
- a purchase preflight may plan at most that remainder, and only a committed `sale` increments
  `spent_sparks` by its exact receipt total;
- walking, waiting, conversation, a pressing need, curfew, and a changed attempt fingerprint never
  reset the amount already spent; and
- changing from one selected member of a `CounterGroup` to another during the same still-active
  errand also preserves the amount already spent.

The visit ends with a traced reason when all targets are satisfied, its budget is exhausted, the
configured source can no longer produce an eligible binding for that visit, its last allowed office
passes, pathfinding returns `no_route`, the route-derived travel deadline expires, an explicit
`go_to` replaces the errand, `Returning` takes ownership of the actor, or an unchanged unpriced-stock
content error makes success impossible for the session. A road session remains viable while that
trip can still bind on its required leg and office, so a leader who temporarily steps outside the
counter radius clears `selected` but does not reset or end the visit; rebinding resumes it with the
same spent budget. When a visit ends, `Round` retains one `ClosedMarketVisit` per stock plan; its
binding list is bounded by that source's counter count. A candidate whose full binding key occurs in
that record cannot recreate the errand with a fresh budget. A new daily binding or a new road trip
has a new key, starts a new visit at zero spent, and replaces the older closed record. Conversation
and other temporary higher-priority rungs pause the existing errand while its configured source
remains viable; they do not manufacture a new visit.

There is at most one failed attempt per concrete counter id, office, and unchanged **attempt
fingerprint**. It is the pair of:

- a source fingerprint containing the full `CounterBindingKey`, binding/open state, and the matching
  uncommitted item ids and quantities; and
- a buyer fingerprint containing the buyer's matching held target quantities and uncommitted spark
  quantity, plus the visit's spent and remaining budget.

`source_absent`, `closed`, `no_route`, `travel_expired`, `no_matching_stock`, `unpriced_stock`,
`insufficient_funds`, and `budget_exhausted` are distinct traced results. `source_absent` means a
previously bound source vanished between selection and the atomic purchase; a later binding change
wakes the plan in the same office. Likewise, `no_matching_stock` may retry after a transfer or
transform changes the source fingerprint, and `insufficient_funds` may retry after a sale, transfer,
or other credit changes the buyer fingerprint. This lets a buyer already waiting at an open counter
react to newly available stock or newly available money without attempting a purchase every tick.
An unchanged `unpriced_stock` failure closes the visit until the next counter session because
embedded catalog data cannot repair itself during a run. An absent road cart does not make a buyer
stand at Seven Lofts for days.

For an ordinary in-city actor, player-directed actions, conversations, explicit `go_to`, pressing
needs, and curfew retain precedence. Stock errands run before the ordinary round and non-pressing
hunger idle behavior. Conversation pauses an errand rather than cancelling it. Section 5's
`Returning`/`DeparturePending` party modes are the explicit exception and suppress these rungs.

Starting target caps and whole-visit budgets:

| buyer | source | target, in declaration order | max spend per visit |
|---|---|---|---:|
| Betriss | group `arriving_grain_carts` | 10 grain | 30 |
| Bertran | counter `betriss_grain_seven_lofts` | 3 grain | 9 |
| Averil | counter `bertran_flour_wool_gate` | 4 flour | 20 |
| Ewart *(M5d)* | counter `brede_wool_drapers_reach` | 4 raw wool | 32 |
| Brede merchant *(M5d)* | counter `ewart_cloth_drapers_reach` | 1 broadcloth | 40 |
| Lantern Road merchant *(M5d)* | counter `ewart_cloth_drapers_reach` | 1 kersey | 14 |

Targets are initial tuning values. They must live in data and use exact metadata-map matching;
food-chain targets use empty metadata.
`stock_plans`, counter groups, listing/counter specs, budgets, offices, and site radii belong in the
embedded `assets/world/food.json`; actor, trade, counter, group, and place references are validated
when the round seeds.

### 7.3 Named counters and binding

| stable id | counter or worksite | purpose | preferred actor | exact offices | required active leg |
|---|---|---|---:|---|---|
| `brede_grain_seven_lofts` | Brede road cart, Seven Lofts | sells the leader's incoming grain; member of `arriving_grain_carts` | Brede leader | `dayspring`, `high_wick` on Brede weekdays | `trade` at Seven Lofts |
| `lantern_grain_seven_lofts` | Lantern Road cart, Seven Lofts | sells the leader's incoming grain; member of `arriving_grain_carts` | Lantern Road leader | `dayspring`, `high_wick` on Lantern Road weekdays | `trade` at Seven Lofts |
| `brede_wool_drapers_reach` | Brede road cart, The Draper's Reach *(M5d)* | sells raw wool | Brede leader | `waning` on Brede weekdays | `trade` at The Draper's Reach |
| `betriss_grain_seven_lofts` | Betriss's grain counter, Seven Lofts | sells persistent stored grain | `p008s` | `dayspring`, `high_wick` | `trade` at Seven Lofts |
| `bertran_flour_wool_gate` | Bertran's mill counter, The Wool Gate | sells flour he actually milled | `e7mil` | `waning` | `work` at The Wool Gate |
| `quern_common_oven` | Ansel Quern's common oven | baking worksite, not a shop | producer `davqn`; `danqn` is lore/presentation only | `watch`, `kindling` | `work` at the common oven |
| `averil_bread_wickmarket` | The Wickmarket bread stall | sells Averil's loaves | `davqn` | `dayspring`, `high_wick`, `waning` | `trade` at The Wickmarket |
| `ewart_cloth_drapers_reach` | Ewart's counter, The Draper's Reach *(M5d)* | cloth sale | `e1skl` | `waning` | `work` at The Draper's Reach |

Counter binding requires the preferred actor to be present, the weekday and exact office to be
allowed, the actor to be within the configured site radius, and the actor's **current** leg to match
the row. A worksite uses its own activation rule and does not become a shop merely because work is
in progress. Merely having a matching leg elsewhere in the weekly round is insufficient.

For the common oven, only Averil's producer eligibility activates baking. Ansel's ownership and
keeper role may be shown in lore, his sheet, route, and presentation, but his presence, current leg,
and availability never gate or pause Averil's transform.

Using preferred actor ids prevents output from becoming stranded on one producer while a different
occupation-matched actor binds as vendor.

M5 authors explicit mechanical routes for Betriss (M5a), Bertran (M5b), Averil (M5c), and Ewart
(M5d). M5c may also route Ansel to the oven so the keeper is visibly present, but that route is
presentation/lore only. In particular, Betriss's current `food_provisioner` route points at The
Wickmarket and cannot be reused as proof that she reaches Seven Lofts.

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
    outputs: Vec<StockSpec>,       // exact future kind/metadata/quantity commitments
    progress_work_minutes: f64,
}

struct ReservedInput {
    item_id: ItemId,
    quantity: u32,
}
```

Production specs and per-day start counters live in `Round`; active jobs live in private
`World::transform_jobs`, keyed by producer. This placement is required because action verbs receive
`&mut World` without `Round` and must still see reservations. `Round` starts, advances, and finishes
a job only through `World` inventory methods; jobs are authoritative but are not exposed in the
public snapshot.

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

Replacing an actor's existing pending offer is the one validation special case. Compute the
replacement against availability after provisionally releasing that offer's own item and spark
commitments, while retaining every other commitment. Apply the replacement atomically: if the new
offer is invalid, the old offer and its commitments remain unchanged. This permits re-offering an
entire already-offered stack without treating the old and replacement offers as simultaneous.

Job start atomically reserves exact quantities from matching uncommitted items in item-id order and
records the recipe's exact future outputs on the job. Every inventory operation—mechanical sale,
negotiated offer, gift, eating, and another transform—must go through the central inventory helper.
A new operation may use only uncommitted quantity. Accepting or retracting an offer is allowed to
consume or release that offer's own commitment; completing a transform consumes that job's own
reservations. Re-offer replacement is the sole owner-side provisional-release exception described
above.

Future output is also a capacity commitment. For every exact `(owner, kind, metadata)` stacking
key, `future output quantity` is the checked sum of all matching outputs recorded by that owner's
active jobs, and the helper maintains:

```text
held quantity + future output quantity <= u32::MAX
```

Starting a job preflights the batch against that equation. While the job is active, any gift, sale,
offer acceptance, split transfer, boundary load, or other operation that would add matching stock to
the producer must include the future output and fail atomically with `output_capacity_reserved` if
the equation would overflow. Removing matching held stock remains legal. The commitment survives
pauses and is released only when completion merges or creates the output. Consequently, output
quantity arithmetic cannot fail after inputs have been consumed, even if matching stock arrived
while the job was paused.

No other owner action silently displaces a promise. In particular, `eat` consumes an uncommitted
unit and leaves a still-valid partial offer unchanged; if no uncommitted unit exists it fails with
`item_committed` and tells the actor to retract or replace the offer first. A whole-stack transfer
fails while another commitment exists, and an explicit partial transfer may move no more than the
uncommitted remainder. The current stale-offer behavior where repeated eating can shrink a stack
below its offered quantity is removed.

On completion, the helper decrements the original input stacks by the reserved quantities, removes
zero stacks, and inserts or merges output into that same named producer's holds using the normal
stacking rule. A transform receipt records all consumed item ids and quantities and each
created-or-merged destination id and quantity.

When an output does not merge, its legal item id starts from producer id, transform id, production
day, start slot, and output slot, then calls the existing `mint_item_id` in the same deterministic
probe-until-free loop used by split-item creation. Collisions with authored items or earlier
transform outputs are expected inputs to that loop, not invariant failures. When output merges, the
receipt names the pre-existing destination id instead.

`job_id` is the full logical `(producer, transform, production_day, start_slot)` key, not a
five-character item id. Completion resolves all output destinations, consumes inputs, inserts or
merges outputs, and records the receipt plus `completed_on_day` atomically in a bounded completed-job
record before removing the active job. A repeated completion request for that key while its receipt
is retained returns the recorded receipt and creates nothing. Records are retained for the current
and previous **completion** day, based on the current absolute day, and are pruned deterministically
at crossed day boundaries and before completion lookup; the job's possibly much older
`production_day` never controls retention. A request after pruning finds neither an active job nor
a receipt, returns `no_active_transform_job`, and performs no inventory mutation. Thus an
indefinitely paused job still receives a full replay window when it eventually finishes, while old
logical keys can never recreate output.

Only the actor named by `ProductionPlan.producer` may run its transforms, and a producer may have at
most one active job. A job starts only when all of these are true:

- the producer is present, within the configured site radius, stationary, not in conversation or a
  conflicting movement/market/meal intent, and on the required `work` leg;
- the current office is in `allowed_offices`;
- all inputs are uncommitted;
- the exact future output passes the capacity-commitment preflight;
- the actor has not spent `max_jobs_per_day` start slots; and
- held output plus active-job output plus one recipe batch is no greater than
  `desired_output_quantity`.

The last condition is the production backpressure. Eligible transforms are checked in their data
order after each completion; no random selection is involved.

The engine supplies the active-conversation set to the production controller on each pump rather
than asking `Round` to infer it from prompt or scheduler state.

M5 has no transform-cancellation path. A job that loses eligibility pauses with its inputs reserved
and resumes on a later qualifying interval, including across offices and production days; route and
worksite validation at seed time ensures every producer has a recurring qualifying leg. Movement,
conversation, a missed office, and a day boundary never destroy paid-for inputs or refund a start
slot. A future feature that needs abandonment must add an explicit, traced cancellation command and
its policy rather than guessing from elapsed time.

Progress uses elapsed **game-clock minutes**, not wall seconds or a real-time deadline. It accrues
only during the overlap with a qualifying office/leg/site interval and pauses on movement,
conversation, interruption, absence, or closure. Clock jumps are processed across crossed office
and day boundaries, so jumping from Waning to Watch cannot credit the whole interval to a closed
mill. No job produces more than once, even if a single pump crosses its completion time.

Catalog additions and posted prices (all four kinds are stackable and non-edible):

| kind | display / plural | visual key | metadata | `price_sparks` selectors |
|---|---|---|---|---|
| `grain` | grain measure / grain measures | `grain_sack` | — | default: 3 |
| `flour` | sack of flour / sacks of flour | `flour_sack` | — | default: 5 |
| `wool` | bundle of raw wool / bundles of raw wool | `wool_bale` | — | default: 8 |
| `cloth` | bolt of cloth / bolts of cloth | `cloth_bolt` | `grade: [kersey, broadcloth]` | `grade=kersey`: 14; `grade=broadcloth`: 40 |

Grain, flour, and loaf intentionally have no metadata and use one default posted price each. M5c
removes the former flour-origin selector from the existing loaf catalog row and leaves its posted
price at **2 sparks**. The supply-chain seed, manifest, stock-plan, and transform-spec validators
reject metadata on those three kinds. Cloth remains metadata-sensitive, has no default price, and
requires one listed `grade` value.

Recipes and initial production backpressure:

| data order | producer and site | consumes | produces | allowed offices | work | target |
|---:|---|---|---|---|---:|---:|
| 1 | Bertran, The Wool Gate mill face | 1 grain | 3 flour | `waning` | 45 min | 6 flour |
| 1 | Averil, Ansel Quern's common oven | 1 flour | 5 loaves | `watch`, `kindling` | 45 min | 20 loaves |
| 1 | Ewart, The Draper's Reach *(M5d)* | 3 raw wool | 1 broadcloth | `high_wick`, `waning` | 45 min | 1 broadcloth |
| 2 | Ewart, The Draper's Reach *(M5d)* | 1 raw wool | 1 kersey | `high_wick`, `waning` | 45 min | 1 kersey |

Here grain, flour, and loaf use empty metadata, while cloth uses exact
`grade=kersey|broadcloth` metadata.

The producer caps are two job starts per workday for Bertran, four per night for Averil, and two
per workday for Ewart. Starting a job spends the slot. A job started before a day boundary retains
its original production-day key for the cap even when it pauses and completes later.

The gross margins are internally possible at posted prices:

- one flour sack costs Averil 5 and yields five 2-spark loaves: 5 sparks gross margin;
- one grain measure costs Bertran 3 and yields three 5-spark flour sacks: 12 sparks gross margin.

These are simplified gross margins, not claims about labor, fuel, tolls, or rent.

The canonical schedule is:

| time | event |
|---|---|
| Day N Kindling | scheduled road party exchanges cargo/cash outside and stages invisibly |
| Dayspring | gate opens; party appears, walks to Seven Lofts, and Betriss buys grain into persistent storage |
| High Wick | Bertran buys available grain from Betriss; the road cart remains at Seven Lofts through the office |
| Waning | Bertran mills and Averil buys available flour; in M5d the cart reaches The Draper's Reach, a road leader may buy cloth already in Ewart's buffer, and Ewart buys wool and starts or continues replenishing that buffer for later trips |
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
2. at The Draper's Reach, Ewart buys available raw wool from the Brede road merchant;
3. a road merchant buys any matching bolt already in Ewart's persistent buffer at the posted
   price, unless an authored conversation negotiates a different transfer. If it checked before
   Ewart's payment and lacked funds, the buyer-fingerprint change wakes this attempt in the same
   office;
4. during eligible work offices over this or later days, Ewart runs the exact kersey or broadcloth
   recipe needed to restore his configured output target;
5. a purchased bolt remains in the leader's holds and is visible on the departing cart;
6. the next `boundary_exchange` consumes it outside the walls.

The incoming wool and outgoing cloth are deliberately buffered rather than a same-visit conversion.
A cart may leave with cloth produced from an earlier delivery, while wool bought on this visit
replenishes Ewart's stock over following eligible offices or days. The Lantern Road party can buy
kersey made from wool left by an earlier Brede trip. If a party's target is unavailable, it leaves
without cloth; no fallback bolt is minted, and M5 makes no promise that the same cart or office
completes both directions.

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
| each road-party leader | 25 wallet float |
| each carter | 4 wallet float |
| Ewart *(M5d)* | 48 |

These are minimum initial values and explicit data, not nightly targets. For the named resident
chain actors, the same numbers are their initial **spendable** working reserves during household
settlement; ordinary residents use their day-zero seeded spendable wallet as their reserve unless
authored otherwise. Here and throughout settlement, `spendable_sparks(actor)` means the quantity of
sparks the actor holds minus every spark quantity committed to a pending offer.

All road-party members have boundary wallet-float settlement from M5a onward. Their wallets may rise
or fall inside the walls, including through haggling, but return to 25 for a leader and 4 for a
carter only at the next successful off-map boundary exchange.

### 10.2 Deterministic spark-credit destinations

Every code-driven spark credit is preflighted through the central inventory helper, including a
sale credit, `road_cash_in`, redistribution, and institutional payroll. If the recipient already
holds a spark stack, the plan names that id and validates the addition against overflow. If the
recipient holds no spark stack, the plan resolves a fresh id from the recipient plus the caller's
stable logical operation key—sale receipt, party/trip/member, or settlement day/recipient—and uses
the normal deterministic probe-until-free loop.

An old `w_<actor>` id is not privileged once it has left that actor's hands. If it still exists in
the player's or another character's holds after a whole-purse transfer, it is an ordinary collision
and the credit probes to a different id; the helper must never attach one item id to two owners or
credit its current owner by accident. The resolved destination, amount, prior owner check, and
arithmetic result are part of the enclosing operation's preflight. Commit only merges into the
planned held stack or creates the planned new stack.

This rule also applies when a debit removes a purse at zero and the same actor is credited later.
Boundary and settlement fixtures explicitly transfer a leader's or resident's whole purse to an
actor with no existing sparks, then prove that the subsequent credit creates a stable collision-free
destination and preserves exactly one owner per item.

### 10.3 M5d household settlement

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

M5d deletes `close_books` and replaces it with `settle_households` at Watch. The generic
crossed-office dispatcher—not `settle_households` itself—owns the once-per-absolute-day Watch sample
and the `Round` fields `last_household_watch_day` and `last_household_settlement_day`. At each Watch
it first checks that the previous sampled Watch, if any, has a matching completed-settlement day. A
mismatch emits `household_settlement_missed`; the dispatcher still attempts today's settlement so
one missed day
does not permanently disable relief. It then stores today's Watch day and samples every resident's
**spendable** balance: zero increments `unrelieved_zero_streak`, while a positive balance resets it.
Because this sampling and prior-day check live outside the handler, a handler that is skipped or
returns early is detectable even when every wallet is nonzero.

After that sample, the dispatcher calls `settle_households`:

1. Resident actors with fewer than **4 spendable sparks** become recipients for exactly
   `4 - spendable_sparks(actor)`. Four exceeds M5's most expensive one-unit mechanical meal (the
   3-spark smoked eel), so the newly available balance can fund at least one ordinary meal and still
   remain above zero; already-committed offer money does not masquerade as meal money.
2. A resident's effective working reserve is the greater of their configured reserve and the
   4-spark household floor. A donor may contribute at most
   `spendable_sparks(actor) - effective_reserve`, so pending offers remain committed and a recipient
   cannot simultaneously be a donor.
3. Donors and recipients are processed in actor-id order. Transfers move only what recipients need
   and conserve sparks.
4. If the surplus pool is insufficient, an explicit institutional wages/alms payment creates only
   the residual shortfall and logs the exact minted amount.
5. The complete transfer/payroll plan is preflighted, including every credit id and arithmetic
   result, then committed atomically. `settle_households` returns a day-stamped
   `HouseholdSettlementReceipt` only after that commit, including for a valid empty plan. Only after
   receiving that receipt does the dispatcher set `last_household_settlement_day` to today's
   absolute day; a skipped call, early return without a receipt, or error cannot mark completion.
6. After a successful commit, every relieved resident has at least 4 spendable sparks and their
   `unrelieved_zero_streak` resets to zero. On a skipped or failed settlement the Watch sample and
   missing completion marker remain, so a resident still at zero reaches two on the next Watch and
   the missed-settlement diagnostic fires independently of that streak.
7. Visitors and road-party members are excluded from both redistribution and institutional payroll.
8. No stock is reset or deleted by settlement.

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

Acceptance uses compact deterministic fixtures, not a multiweek full-world or accelerated-clock
run:

- a settlement fixture proves that every eligible resident below 4 spendable sparks finishes with
  4 spendable sparks, donors retain their spendable working reserves and pending-offer commitments,
  visitors and road parties are excluded, stock is untouched, and a successful relief resets the
  zero-wallet streak;
- a dispatcher fixture skips and separately fails one Watch settlement with all wallets positive,
  then proves the next Watch emits `household_settlement_missed`; a successful handler records its
  day only after commit and produces no false positive;
- the fixture calculates the donor pool and residual shortfall independently, then asserts that
  institutional payroll mints exactly that shortfall and no more. Payroll has no arbitrary
  percentage ceiling;
- a transaction fixture combines a resident sale, negotiated transfer, payroll, road sale, and both
  boundary cash directions and checks the global spark equation after every operation; and
- a compact backpressure fixture repeatedly drives market, production, and boundary controllers
  without authored-world pathfinding, asserting target-plus-active-batch limits, one job per
  producer, one errand per buyer, and exact per-member wallet-float settlement.

The 14-day and 56-day full-world runs are not acceptance gates. A developer may still run a long
trace when tuning manifests or routes, but normal verification must not take hours or depend on a
physically impossible accelerated travel clock.

If a focused chain fixture fails, tune manifests, yields, purchase caps, working reserves, or the
household floor.
**Do not add an automatic price response to road conditions or inventory.** Authored characters
remain free to haggle about an individual exchange.

---

## 11. Milestones

### M5a — The cart reaches Seven Lofts

Ships:

- central `Presence` semantics and all filters/cleanup in Section 5;
- two explicit road parties and five authored character sheets;
- deterministic boundary manifests and per-member wallet-float accounting;
- the `RoadCart` snapshot and Bevy presentation;
- `grain` and its posted-price catalog rows;
- the Seven Lofts historical seed, Betriss's counter, and her targeted lore update;
- `listings` versus `restock`;
- live vendor-hold inventory;
- one purpose-neutral catalog-sale transaction, separate meal and stock planners, and resumable
  stock errands;
- quantity-aware, transfer-safe legacy restock shares;
- direct party routing and Betriss's explicit route rather than occupation routing;
- exact-office bootstrap for both Kindling staging and Dayspring entry; and
- the committed default start change to Highmarket/Dayspring.

Betriss buys incoming grain and Bertran can buy and hold it; milling does not exist yet.

Acceptance:

- on Highmarket, exactly the three-person Brede party stages invisibly at Kindling, appears with
  one cart at The Wool Gate at Dayspring, physically reaches Seven Lofts, returns, and disappears
  atomically;
- on Second, exactly the two-person Stone Gate party does the same;
- on Bellday, Lowmarket, and Seventh no party arrives;
- a host integration test loads the committed `default_config.ron` through the production config
  loader (with no local `config.ron` override), asserts day 2/Dayspring, and shows the Brede party
  entering; separate exact-Kindling and exact-Dayspring sim fixtures stage/enter once, never twice;
- an early check before the leader reaches the Seven Lofts counter spends no attempt; binding the
  cart later in the same office wakes Betriss and permits the grain purchase;
- after departure, none of their actors, still-owned items, offers, targets, percepts, queue entries,
  or derived cart leaks into the public world; each departing member's unread inbox is empty, while
  their pre-existing memories, goals, relationships, and `recent_history` are unchanged;
- a conversation delayed past Snuffing delays departure but not forever; an outstanding provider
  cognition request does not delay the first otherwise-safe tick, and its completion is rejected
  both while the actor is absent and after the same party re-enters with a newer presence epoch; its
  drained inbox percepts are neither requeued nor shown after re-entry;
- a returning member with pressing hunger, a water/meal/market queue, a curfew destination, and a
  pending offer still walks to the gate; the competing intents are cleared, the offer expires there,
  and none can deadlock departure;
- a party still present at its next scheduled Kindling logs one missed trip and receives neither a
  second manifest nor another trip number;
- each successful boundary exchange sets the leader to 25 sparks and every carter to 4, with every
  member's exact difference traced as `road_cash_in` or `road_cash_out`; repeated interior earnings
  cannot make any party wallet grow across successful trips;
- commercial cargo transferred from the leader to a carter is consumed at the next boundary, while
  unrelated personal cargo held by either actor survives; the transfer leaves the cart's derived
  presentation load unchanged because the same party still holds the cargo;
- after the leader transfers their whole purse to an actor with no spark stack, the next
  `road_cash_in` probes past the still-owned old wallet id, creates one new purse for the leader,
  and preserves spark conservation and single ownership;
- a forced first-candidate manifest-id collision probes to the same stable free id on replay, while
  a forced preflight failure emits `boundary_exchange_failed` and changes no cargo, member wallet,
  trip, phase, presence, epoch, or derived cart view;
- Seven Lofts stock changes by real purchases and remains unchanged through Watch;
- whole and partial negotiated transfers of legacy-restocked stock survive the next restock sweep;
  a returned unmarked stack preserves its real quantity while newly merged restock quantity is
  swept, and the conjured quantity behind a standing offer stays bounded by one template plus its
  outstanding legacy commitment across repeated restocks; and
- an asset/prompt test proves that each road merchant's origin, kin, cargo obligation, and road
  opinion are present in their authored context. Asking about them in a live conversation is a
  manual content smoke test, not a nondeterministic automated assertion about LLM wording.

### M5b — Grain becomes flour

Ships:

- `flour`;
- in-place reservations and timed `TransformJob`;
- Bertran's explicit Seven Lofts → Wool Gate route;
- the named Wool Gate mill counter; and
- the asset assertion that locks Bertran's preparatory lore alignment from Section 2.2.

In an isolated no-preexisting-flour fixture, acceptance follows receipt edges for one grain unit
from the road leader to Betriss to Bertran, then **one stack containing three flour units**
from Bertran to Averil. In the full world, where merges may reuse ids, acceptance checks the same
quantities and event chronology without claiming lot ancestry. Input is consumed once, output is
created once, every mechanical sale price comes from the catalog, and no other miller transforms
anything. A reserved input stays owned, cannot be transferred whole, can expose only its
uncommitted remainder, pauses away from the mill, and completes once across a clock jump. A forced
collision on the first output-id candidate probes to a stable free id, and replaying completion
returns the same receipt without consuming or producing twice. A capacity regression starts a job
whose future output exactly fills the producer's matching `u32` stack, rejects an intervening inbound
gift with `output_capacity_reserved`, and then completes without losing inputs or overflowing the
stack. A retention regression pauses a job across several production days, completes it, receives
the same receipt on replay during the completion-day window, and receives a mutation-free
`no_active_transform_job` after deterministic pruning. An asset assertion proves Bertran's authored
context places his working mill face at the Wool Gate and its grain source at Seven Lofts.

### M5c — The Quern night bake replaces bread restock

Ships:

- Ansel Quern's common bakehouse as a named worksite, with Ansel's keeper role limited to
  lore/presentation;
- Averil's Watch-to-Kindling route and baking jobs;
- Wickmarket binding to Averil's real held loaves;
- collapse of the existing loaf variants into one metadata-free, 2-spark loaf catalog entry;
- targeted migration and reviewed regeneration of only the fixtures containing old loaf variants;
- deletion of every loaf template from both `bread` and `provisions` restock;
- the targeted Ansel/Averil lore alignment in Section 2.2.

Acceptance follows the quantity/receipt chain from grain delivered on Day N through milling and
night baking to a funded resident buyer's loaf purchase no earlier than Dayspring on Day N+1.
Grepping the restock path finds no loaf creation, and the production targets stay bounded when no
buyer comes. Averil's autonomous hunger prefers any non-listed food she holds, but if her listed
loaves are her only usable food she consumes exactly one instead of leaving to buy another meal.
The remaining stack stays sellable and the production target observes the reduced quantity; an
explicit action may likewise consume or transfer an uncommitted loaf. Baking starts and completes
identically whether Ansel is present, absent, delayed, or on another leg.

Then rerun M4's behavioral acceptance unchanged: **Ilse has one spark, cannot buy a two-spark loaf,
buys the one-spark herring, and eats it.** M5 must not rewrite that acceptance story into an
impossible loaf purchase.

### M5d — Wool returns as cloth and the reset dies

Ships:

- `wool` and `cloth`;
- the road carts' Waning leg at The Draper's Reach, after remaining at Seven Lofts through High Wick;
- Ewart's raw-wool purchase and two transforms at The Draper's Reach;
- road merchants' exact cloth stock targets;
- departing cart load presentation;
- boundary consumption of the return load;
- `EconomicClass`, `settle_households`, and deletion of `close_books`;
- focused chain, settlement, conservation, and bounded-controller fixtures.

Acceptance proves raw wool enters on a Brede manifest and, over later eligible work offices or days,
replenishes existing cloth stock in Ewart's holds. On an eligible visit, the deterministic ladder
may buy a previously produced bolt at its posted catalog price; that bolt leaves on the visible cart
and is consumed only at the next off-map boundary exchange, even if it is transferred from the
leader to a carter before departure. There is no requirement that one cart or one Waning office
deliver wool, wait for its transform, and carry that output away. The focused
criteria in Section 10.3 pass. A separate regression negotiates a non-catalog exchange successfully
and proves that it emits transfers but no `sale`. The cart remains a bound Seven Lofts source
throughout High Wick, then binds only at The Draper's Reach during Waning; it buys buffered matching
stock if available and otherwise departs without cloth.

The funds-order regression deliberately lets the Brede leader fail a 40-spark broadcloth purchase
before Ewart buys wool; Ewart's payment changes the buyer fingerprint, wakes the stock plan during
that same Waning, and the now-funded retry succeeds without depending on actor-id iteration order.

---

## 12. Observability and invariants

Extend `--trace-food` rather than adding a parallel tracer. Events:

- `boundary_load` / `boundary_unload` with party, trip, cargo owner, item ids, kinds, and quantities;
  `road_cash_in` / `road_cash_out` with member id and exact amount; and
  `boundary_exchange_failed` with the preflight reason and unchanged party/trip identity;
- `road_stage`, `road_in`, `road_return`, `road_offer_expired`, `road_out`, and `road_trip_missed`
  with phase, all member ids and presence epochs, gate, trip number, and derived cart view;
- `stock_errand` with plan, configured source, selected counter binding, bindings seen in the visit,
  phase, spent and remaining visit budget, result, the source-and-buyer attempt fingerprint,
  visit-end reason, and next retry;
- purpose-neutral `sale` with buyer, seller, every source/destination receipt line, exact metadata,
  quantities, unit prices, and totals; intent-specific diagnostics remain with the calling planner,
  not the transaction or receipt;
- `item_transfer` for offers, gifts, and negotiated steps, including transferred item/spark
  quantities but no assertion that they equal a catalog price;
- `transform_start` / `transform_pause` / `transform_finish` with spec,
  reserved or consumed item ids and quantities, future-output commitments, output destination ids,
  producer, production day, completion day, and work minutes; rejected inbound inventory operations
  include `output_capacity_reserved` where applicable;
- `cart_load` when the party-wide inventory scan changes the derived presentation category set, with
  the before/after sorted sets but no stored cart inventory;
- `household_settlement` with Watch day, donor transfers, recipients' before/after spendable
  balances, and minted shortfall; plus `household_settlement_missed` with the sampled Watch day and
  last completed day.

`food_summary()` gains:

- Seven Lofts grain quantity;
- held, uncommitted, commercially listed, transform-reserved, and offered quantities for Betriss,
  Bertran, Averil, and Ewart;
- every road party's phase, trip number, every member wallet/float, present/absent state, presence
  epochs, and derived cart load;
- every active market errand's plan, selected counter binding, bindings seen in the visit, phase,
  spent/remaining budget, last failed fingerprint, and next retry;
- active jobs with future outputs and transforms completed by logical job key and completion day;
- resident, visitor, and all-road-party spark totals; boundary cash in/out; redistribution; payroll
  minted; residents' total/spendable balances; last Watch/completed settlement days; and unrelieved
  zero-wallet streaks.

Standing invariants for the completed M5 chain follow. Earlier sub-milestones permit only the
explicit transitional loaf restock and non-chain wallet refill named in Sections 7.1 and 10.1:

- global grain, flour, wool, cloth, and loaf quantities change only through initial seed, boundary
  exchange, consumption, or a declared transform; purchases and negotiated transfers conserve
  their global quantities;
- every item has exactly one owner, no same-stuff stackable duplicates share a holder, and total
  transform-plus-offer commitments for an item never exceed its quantity; held quantity plus active
  future output for every owner/stacking key never exceeds `u32::MAX`;
- sparks change only through an atomic conserving transfer, logged institutional payroll, or
  logged boundary cash adjustment, and the equation in Section 10.3 always balances;
- every code-driven spark credit resolves to a stack currently held by its recipient or to a
  preflighted collision-free id; a purse transferred away is never credited through its old id;
- every `sale` uses the exact catalog unit price; `item_transfer` is explicitly exempt because
  bargaining, credit, gifts, and cheats are allowed;
- neither a catalog-sale request, its receipt, nor the transferred item records a meal/stock purpose;
  both planners call the same transaction and own their subsequent behavior themselves;
- a transform cannot consume the same quantity twice, complete away from its site, run under the
  wrong actor, exceed the producer's daily cap, consume/transfer reserved quantity elsewhere, lose
  its output capacity while paused, or use production day as completion-receipt retention time;
- a stall cannot sell an item its current vendor does not hold as uncommitted quantity;
- autonomous hunger never starts or completes a meal purchase while the actor has a usable
  uncommitted edible unit; held-food selection prefers non-listed inventory but may consume exactly
  one unit from a listed stack when that is the only food available;
- absent actors and their owned state never appear in snapshots, percepts, targets, schedules, or
  present-world totals; departure leaves their memories and existing history intact but clears unread
  and in-flight-drained inbox material, and cognition can mutate an actor only when its stamped
  presence epoch is still current, so departure/re-entry cannot revive an old request;
- `Returning` exclusively owns party movement, standing offers cannot block departure, party
  state has one phase/trip-number source, every phase transition is atomic, trip numbers advance by
  checked addition only at staging, and the derived cart load agrees with all configured commercial
  cargo held by all party members;
- each Watch sample is paired with exactly one successfully committed household settlement or an
  explicit missed-settlement diagnostic, and the 4-spark floor is measured in uncommitted,
  spendable sparks; and
- each buyer has at most one active market errand, its spent budget never decreases or exceeds its
  plan cap, and no binding recorded by the last closed visit can reopen as a fresh visit; no
  production controller or market intent grows without its configured bound, and no successful
  road-trip cycle carries any member's prior-trip cash balance past boundary settlement; every road
  member, including each carter, returns to a configured float there.

---

## 13. Fixtures and compatibility

With no presence field, actors deserialize as `InCity`; with no presence epoch, they deserialize at
epoch zero. Fixture worlds with `nav: None` enroll no rounds or road parties. The M5a/M5b presence,
market, and transform additions should therefore leave existing prompt fixtures byte-stable; verify
them rather than regenerate them blindly. M5c is the deliberate exception: collapsing old loaf
variants into the single metadata-free loaf requires a targeted migration and reviewed regeneration
of fixture manifests and expected snapshots that actually contain those variants. Unrelated
fixtures remain unchanged.

Add focused fixtures/tests for:

- a present road merchant with a beyond-the-walls home and grain in `you_sell`;
- Betriss at Seven Lofts with persistent grain;
- Bertran selling non-edible flour from transformed holds;
- Averil selling transformed loaves;
- an absent actor whose item, pending offer, target, percept, and queued cognition are all filtered
  or rejected; departure discards both an unread inbox line and the percept buffer drained by an
  in-flight request while preserving existing memories and `recent_history`, and that request's
  completion remains stale after the actor re-enters;
- fresh-Kindling and fresh-Dayspring bootstrap, coarse Kindling→Dayspring ordering, delayed
  departure, missed-trip suppression, one authoritative party-state record, checked trip-number
  overflow, and exactly one revision for each atomic stage/entry/return/pending/departure transition;
- deterministic boundary manifest-id and zero-wallet spark-id collision probing; cargo transferred
  to a carter remains visible in the same derived cart load and is later unloaded; validation that
  the wallet-float map exactly covers party members; and complete rollback of cargo, all member
  wallets, trip, phase, presence, epoch, and derived cart view on a forced preflight failure;
- return-mode preemption of pressing needs, curfew, movement, every queue/errand, and gate expiry of
  a pending offer;
- deterministic target order, receipt ids, rollback on validation failure, and distinct stock-errand
  retry/end reasons; a visit budget persists across multiple purchases, failed fingerprints,
  conversation and need pauses, and a same-visit counter-group reselection, cannot be reset by
  clearing and recreating the errand against the same binding, and resets only for a new daily
  binding or road trip; the fixtures also include a mobile counter that binds after an early check
  and an insufficient-funds road leader who retries during the same office after Ewart's wool
  payment changes the buyer fingerprint;
- one purpose-neutral sale entry point used by both planners: a stock planner buys a twenty-loaf
  stack, later hunger consumes one of those held loaves without another sale or shopping trip, and
  the stock target then observes nineteen; a non-listed edible wins over listed bread when both are
  held, while food becoming usable during a queued or serving meal cancels that stale purchase;
- a reserved input that remains owned, blocks a whole transfer, permits only the uncommitted partial
  quantity, remains reserved across pauses and day boundaries, is consumed once on completion, and
  survives cloning `World` and `Round` together;
- transform pause/resume at movement, conversation, site, office, and crossed-clock boundaries,
  plus deterministic output-id collision probing, future-output capacity blocking an overflowing
  inbound transfer, infallible completion at the reserved capacity, completion-day receipt retention
  after a multi-day pause, replay within the window, and mutation-free rejection after pruning;
- legacy-restocked whole/partial item transfers that cannot be swept from the recipient, a returned
  real stack merged with a tracked restock share, and a pending offer kept bounded across repeated
  restocks;
- a pending offer protected from mechanical sale/transform, atomic full-stack re-offer replacement
  with the old commitment provisionally released, rollback to the old offer on invalid replacement,
  explicit eating that consumes only the uncommitted remainder then fails `item_committed`, a
  non-catalog negotiated exchange, and a zero-spark gift, none logged as `sale`;
- a hungry Averil holding only commercially listed loaves who eats exactly one uncommitted unit,
  starts no meal purchase or trip, leaves the remainder immediately sellable, and causes the normal
  production target to observe the reduced quantity without output-id registration;
- `Resident`/`Visitor`/`RoadParty` settlement, including the 4-spendable-spark resident floor when
  other sparks are offer-committed, exact residual shortfall mint, collision-free credit after a
  zero-wallet resident's former purse moved whole, streak reset after successful relief, a
  skipped/failed-handler missed-day diagnostic with positive wallets, and Ilse retaining her
  authored one spark;
- exact spark conservation through resident sales, conversational transfers, payroll, road sales,
  and boundary float settlement; and
- repeated market, production, and boundary-controller steps in a compact fixture, proving the
  configured stock, job, errand, and per-member road-wallet bounds without full-world pathfinding.

The automated gates are both the pure-sim fixture suite and a host integration suite:

```sh
cargo test -p cathedral-sim --test supply_chain_tests
cargo test -p cathedralbevy --test supply_chain_host_tests
```

The sim suite uses small deterministic worlds with fake/no cognition actions and asserts Sections
10 and 12 directly rather than parsing prose. The host suite loads the real committed
`default_config.ron` through the production loader and asserts Highmarket/Dayspring entry without a
local override. It also projects a loaded `RoadCart` snapshot through the Bevy systems, asserts one
visible cart root with the expected sack/bale/bolt mesh children, updates its transform/load, and
asserts despawn with the party. Both must complete as ordinary test runs; a multiweek authored-world
CLI transcript is optional diagnosis, never pass/fail authority.

The pure-sim snapshot fixture derives that `RoadCart` from party state and inventories: moving the
only wool or cloth stack from leader to carter leaves the load unchanged, moving it to a non-party
actor removes that category, and no cart/load record exists to update separately. The host suite
then treats the resulting snapshot as presentation input only.

M5a additionally has a required visual drive check because an ECS assertion cannot prove that the
cart is legible in the rendered city. With a clean committed-default config (and the fake backend if
offline), run:

```sh
CATHEDRAL_DRIVE='sleep 2; tp -35 16 530 0 -20; shot brede_cart_entry; sleep 20; shot brede_cart_route; quit' cargo run
```

Review the two archived screenshots and `logs/latest_session/logs.jsonl`: the first must visibly
show one loaded cart, its leader, and both carters entering at the Wool Gate; the second must show
the same single cart following the leader toward Seven Lofts, with no duplicate or orphan. Record
the reviewed session id in the M5a handoff. This visual check and the host committed-config test are
release criteria, not claims delegated to the pure-sim suite.

Implementation handoff (2026-07-20): `supply_chain_tests` and `supply_chain_host_tests` pass, and
the host fixture proves the committed day-2/Dayspring entry, one loaded cart, sack/bale/bolt mesh
projection, following transform updates, and despawn. The exact visual drive above was attempted,
but no session id or screenshots were produced because the agent sandbox could not open
`DISPLAY=:0` (`XOpenDisplayFailed`). Run it once from a graphical shell and append the reviewed
session id here before release.

`turn.j2` needs no new market prose. Grain and flour are normal held items; procurement is sim
behavior, not a new LLM verb.

Expected code blast radius includes sim world/snapshot state, the engine/round transition seam,
round ladder, central inventory helpers, food schema and seed, item catalog, authored character
assets, road-party/route/worksite data, the committed default clock configuration, headless
acceptance fixtures, and the Bevy cart projection. It does not include a new persistence system.
M5a should not be described as a round-only data edit.

---

## 14. Risks and mitigations

| risk | mitigation |
|---|---|
| Seven Lofts empties on non-arrival days | one-time persistent seed, four scheduled arrivals per week, explicit storage and buyer targets, focused inventory-flow and backpressure fixtures |
| same-morning ordering becomes flaky | the canonical acceptance spans Day N to Day N+1; buffers decouple each producer |
| resident merchants or cargo workers accidentally leave town | explicit party membership; no occupation-wide `road_trader` mapping |
| transformed output is stranded behind stale `stock_ids` | sell inventory is derived from the active vendor's current holds |
| a vendor holding bread ignores it and leaves to buy dinner, or consumes the whole board | every meal decision checks held uncommitted edibles first, prefers non-listed food but permits exactly one listed unit, and lets stock/production observe the resulting deficit |
| reserved inputs violate item ownership/stacking | reservations annotate quantities in the producer's existing stacks; no detached input item exists |
| matching stock arrives while a paused job already owes output and makes completion overflow | the job records future output; every inbound inventory path includes it in the `u32` capacity preflight and rejects `output_capacity_reserved` |
| a multi-day pause makes a new completion receipt look old immediately | retention uses `completed_on_day`, never the job's production day; replay-before-prune and no-op-after-prune fixtures cover both sides |
| every baker or miller transforms stock, or one producer floods stock | each plan names one producer and has output targets plus daily job caps |
| Ansel's schedule accidentally blocks the night bake | only Averil's producer/site/office eligibility is mechanical; Ansel is lore/presentation |
| a cart arrives after an early failed purchase check or leaves Seven Lofts too soon | mobile errands wake on binding changes; carts remain through High Wick and move to The Draper's Reach at Waning |
| a retry or temporary interruption silently resets a stock buyer's visit budget | `spent_sparks` lives on the resumable errand, every retry preserves it, and one bounded `ClosedMarketVisit` prevents recreation against any binding seen in that visit |
| a road trader is diverted from its return or trapped by an offer | `Returning` owns movement above needs/curfew/queues, and unresolved offers expire at the gate |
| an absent actor leaks through an owned item or offer | central presence predicate plus actor/item/offer/target tests |
| unread or in-flight-drained city percepts resurface on a later trip | departure discards both inbox forms without touching existing memories or `recent_history`; stale completions cannot requeue them |
| a pre-departure cognition result arrives after the same actor re-enters | requests carry a monotonic presence epoch; absent or epoch-mismatched completions are diagnostic-only and cannot restore inbox state |
| party phase and trip number drift or a non-boundary transition half-applies | one `PartyState` source plus uniform preflight/infallible commit and one-revision tests for every transition |
| commercial cargo transferred to a carter escapes boundary delivery or disappears from the cart | boundary preflight and snapshot derivation both scan every member in deterministic order; the receipt records each cargo owner |
| cloth exists only as a return-load prop | raw wool is a boundary manifest; Ewart buys it and performs a declared transform |
| a cloth return assumes impossible same-office production | Ewart's persistent buffer serves later carts; a cart with no matching bolt simply leaves without one |
| carts are promised but invisible, or a cached cart load drifts from inventory | the load is derived from all party inventories, host projection assertions cover visible mesh children, and a required drive-mode session captures the loaded cart entering and following |
| residents or chain firms go broke while their money is offer-committed | working reserves and the 4-spark floor use spendable sparks; commitments stay untouched and only the exact residual shortfall is minted |
| settlement is skipped while every wallet is positive, so the zero streak sees nothing | the generic Watch dispatcher compares sampled and completed days outside the handler and emits `household_settlement_missed` |
| a zero-wallet credit reuses a purse id that was transferred away | every credit preflights a recipient-owned stack or deterministic collision-free id; boundary and settlement fixtures transfer the old purse whole |
| road-party members accumulate cash forever | every successful off-map exchange logs symmetric settlement to the configured leader/carter floats; repeated boundary-controller fixtures cover every wallet |
| old restock deletes real or negotiated stock | operational provenance is quantity-aware; central inventory helpers detach shares from transferred/consumed quantity, and a sweep removes only the remaining share at its original vendor |
| a cloth buyer fails before Ewart's wool payment and never retries | the attempt fingerprint includes buyer funds, so Ewart's payment wakes a same-Waning retry independently of actor iteration order |
| replacing an offer double-counts its already committed stack | replacement validation provisionally releases only the old offer and rolls back atomically on failure |
| stack merging makes false provenance claims | receipts record quantity edges and actual destination ids; M5 claims commodity flow, not lot ancestry |
| implementation accidentally removes haggling | only `sale` is posted-price constrained; negotiated transfer and gift regressions remain green |
| a bad road silently changes posted prices | no multiplier exists; only schedule, manifests, and availability may change, while individual bargaining remains allowed |
| acceptance depends on arbitrary LLM wording | authored facts are asset/prompt assertions; natural conversation is a manual smoke test |
| persistence scope expands M5 | jobs, receipts, party state, presence epochs, and bounded market-visit state are authoritative in-memory state; versioned disk persistence is explicitly future work |
| a fresh default run misses the visible arrival | M5a commits day 2/Dayspring; a host test loads the real committed config without a local override, and the drive check starts from that default |
| a recurring transform collides with an existing five-character item id | call `mint_item_id` in the split path's deterministic probe loop and record chosen completion destinations for idempotent replay |
| a boundary id collision or late validation failure half-applies a trip | preflight every member cargo removal, wallet destination/delta, manifest destination, and checked trip advance, then commit atomically |
| embedded data is mistaken for hot reload | acceptance rebuilds after `food.json` or `items.json` changes |
