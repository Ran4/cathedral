# The bread round: stalls, stock, and the silent purchase

The water round is the template: a source with a keeper, a queue with rules, a timed service, a
need snapped back, self-percepts so the act is askable, player-only sounds so it costs nothing.
This document is the same machine with loaves in it — plus the one thing water never needed:
**stock that runs out, and the confessed magic that refills it.**

---

## 1. Food sources

A `FoodStall` mirrors `WaterSource` (`round.rs:235-253`):

```rust
struct FoodStall {
    name: &'static str,
    site: PlaceId,              // the square or tavern it belongs to
    pitch: Vec3,                // where the queue forms
    vendor: Option<ActorId>,    // bound per day, like a well keeper
    queue: Vec<ActorId>,
    serving: Option<(ActorId, f64)>,   // buyer + service end-time
}
```

The stall table lives in a new embedded `assets/world/food.json` (the `rounds.json` pattern —
`include_str!`, both hosts get it free), one entry per pitch:

| site | pitches | pitch positions | open |
|---|---|---|---|
| The Wickmarket | 2 (bread, provisions) | copied from two of the `wick_fixture_*` stall fixtures (`lore/places/ombreval_buildings.json`) so the queue forms at real geometry | market hours (Dayspring→the Waning), busiest Highmarket |
| Coswald's Yard | 1 | ditto | Highmarket only |
| The Tallage | 1 | ditto | Lowmarket only |
| Maren's Green | 2 (herring, eel) | ditto | market hours, busiest Lowmarket |
| The Hungry Ox | 1 (the pot) | at the door — the area exists (`hungry_ox`, areas.json) | the tavern archetype's whole day; curfew-exempt |
| The Bell and Ladle | 1 (the pot) | **blocked: the area does not exist** — `features/movement/08_risks.md:100` already demands it: *"Named in 4 of the 20 authored routes and it does not exist in areas.json. Add it, at the Bellstand."* This feature adds it. | as the Ox |

Seven pitches. Sixty stall fixtures stand in the streets; seven of them come alive. That asymmetry
is correct — the rest are set dressing until the supply chain gives them tenants — but `food.json`
naming fixture-anchored positions means lighting up an eighth is a data edit, not a code change.

**Open hours are predicates on the clock** (`WorldClock::at(now)`, office + weekday), not state:
a stall with no bound vendor, or outside its hours, is closed, and closed stalls fail the ladder
rungs' "source open" check ([03](03_hunger.md) §3) exactly as an unstaffed well draws no water
(`round.rs:1505-1506`).

## 2. Binding vendors

At round seed and at each Dayspring, each stall binds the nearest unbound townsperson whose
occupation is in the stall's trade list and whose **active leg is a `Trade` arrival at the stall's
site** — i.e. the people `rounds.json` already routes there:

| stall trade list | occupations (cast count) |
|---|---|
| bread | `baker` (8) |
| provisions | `food_provisioner` (18), `grocer_and_spicer` (5), `market_seller` (12) |
| fish / eel | `fish_trader` (7), `market_seller` |
| the pot | `cook` (8), `tavern_worker` (9) — the authored keepers first: Renna Tapster (`gr8tp`) at the Ladle, Bertran of the Ox (`g5brt`) at the Ox |

Like a well keeper, a bound vendor is pinned to the pitch with a short leash; unlike a keeper, the
binding follows the vendor's own round — when the Waning ends the market leg, the stall closes and
the vendor walks home like everyone else. No new movement code: the stall borrows the actor the
round already delivered.

## 3. The Kindling restock — the magic, and its confession

At each day's Kindling bell (the `offices_crossed` hook, `clock.rs:361-386`, consumed where
`ring_offices` already runs, `engine.rs:815`):

1. each vendor-to-be's **unsold stock from yesterday is removed** and their **stock re-conjured**
   from the occupation's template in `food.json`:

   | occupation | morning stock |
   |---|---|
   | baker | 12× `loaf {flour: rye}`, 3× `loaf {flour: wheat}` |
   | food_provisioner / grocer / market_seller | 6× `herring`, 4× `loaf {flour: rye}` |
   | fish_trader | 10× `herring`, 4× `smoked_eel` |
   | cook / tavern_worker | the pot: `stew` is conjured **per serving at sale time** — the lore's never-scraped pot is the one licensed infinite inventory |

2. vendor wallets reset to their float (6 sparks — change for a market morning), buyer wallets
   refill to seed ([02](02_the_spark_standard.md) §4) — at the Watch, so the books close before
   the bakers rise.

Stock items are real `World.items` stacks **held by the vendor** (deterministic ids per
`(vendor, game_day, slot)`, [01](01_items_and_stacks.md) §6) — so the Bevy mirror's owned-items
invariant holds, `you_hold` shows the baker their own bread, and an LLM conversation can sell from
the *same* inventory the ladder sells from. One stock, two doors.

**This is the cheat the brief ordered, and it is structural, not shameful** — but it must
eventually die, and the replacement is already sketched in §6. Everything above is written so the
supply chain replaces *step 1* and touches nothing else.

## 4. The queue

`enqueue` generalizes from the well's household-first rule (`round.rs:1457-1469`) to a plain FIFO —
markets have no vessel classes. `FOOD_QUEUE_SHORT` twins `WELL_QUEUE_SHORT` for rung 7's
convenience check; rung 3 (famished) joins regardless of length. Arrive, take your place, shuffle
forward: all existing phase machinery (`Phase::Approaching/Queued/…`, `resolve_arrivals`,
`round.rs:1418-1453`) reused wholesale.

## 5. The purchase

When a buyer reaches the head and the stall is open, `serving` runs `PURCHASE_SECONDS` (~4 s — a
coin counted, a loaf wrapped), then the atomic swap, all inside the sim tick:

- price = catalog list price for the kind+metadata; the buyer's coin stack −price, the vendor's
  +price (merge rules from [01](01_items_and_stacks.md) §2.2);
- one unit of the cheapest edible stock the buyer can afford moves vendor→buyer (famished buyers
  take the cheapest that satisfies; a herring if a spark is all they have — Ilse's exact
  arithmetic);
- then the buyer **eats it at the pitch** over `EAT_SECONDS` unless their next leg is home within
  the supper span, in which case they carry it (and rung 3's eat-what-you-hold finishes the story
  at the hearth). Satiety applies on the actual eat, not the buy. **The ladder's auto-eat is
  silent** (M3, as implemented): like the well draw, the eater *remembers their own meal*
  (`remember_percept`) and the player sees the item vanish from the snapshot, but the terse
  `X ate a herring` bystander line the `eat` *verb* delivers to neighbours' inboxes
  ([05](05_the_llm_seam.md) §4) is **dropped** — a code-driven meal fires far too often to nudge a
  reaction turn per bite, which would break the "thirty sales an hour never schedule a turn"
  discipline below. A *deliberate* `eat` (an LLM or the player choosing it) keeps its bystander
  line; only the ladder's automatic pitch/held eats are silenced (via `round.rs`'s `silent_eat`).
  "Next leg is home" is read off the buyer's **active round leg** (`is_home`), not merely the
  clock, so a buyer still at their market post eats in view rather than pocketing the loaf.

**Percepts follow the water round's discipline** (*"a clock, not an event"*, `round.rs:1474-1478`):

- **self-percepts, both parties** (`remember_percept`, `character.rs:373-387`): buyer — *"You
  bought a herring from Wyn for 1 spark."*; vendor — *"You sold a herring for 1
  spark."* Consecutive-repeat dedup already collapses a busy morning into one line,
  which is exactly right. The player can ask either party what they bought or sold, and they can
  answer, for zero tokens.
- **no NPC inbox lines, no nudges.** Thirty sales an hour in a market square must not schedule a
  single LLM turn. The stage gate and settled-actor hash already keep a busy square affordable
  (`attention.rs:452-507` — walkers don't churn the hash; buyers who *stop* at a stall do, and
  that is genuine news);
- **player-only sound**: a new `coin_clink` catalog entry emitted per sale as an unattributed
  world sound (the windlass pattern, `round.rs:1581-1589`), audible ~15 m. Optional flourish, same
  mechanism: a `market_cry` sound from bound vendors every minute or two, so a square *sounds*
  like a market before it looks like one.

If the buyer cannot pay by the time they are served (spent it in an LLM trade mid-queue), the
service resolves as a no-sale — buyer leaves the queue, rung re-evaluates; the same graceful
degradation as a stale offer.

## 6. What replaces the magic (M5, sketched, not scoped)

The brief: *"note that this needs to change. Ideally they should need to buy it from bakeries,
bakeries buy flour from marketpeople, flour coming in from outside of the town etc."* The pieces
are already on the map:

- **bakehouses**: *"Communal bakehouses cluster near grain routes and ward edges"*
  (`lore/places/03_new_places_and_infrastructure.md:254-257`); bakers' Kindling leg becomes "bake
  at the bakehouse", conjuring loaves *there*, carried to the pitch — the restock becomes a walk
  you can watch;
- **flour**: a `flour {grain: rye|wheat}` kind; bakers buy from millers (`miller`, 3 cast,
  workplace already the Wool Gate mill side) — the same silent-purchase machinery pointed at a
  different stall;
- **grain from outside**: carts through the Wool Gate to **Seven Lofts**, the defended granary
  (`~(360,335)`), on a morning schedule — the visible edge of
  `features/the_near_countryside__aka_add_market_stalls.md`, whose harvest/weather/trouble signals
  then have a physical carrier;
- at that point the nightly wallet resets become real wages and real costs, and the Chapter's
  granary — *"F.183: the Chapter opened its granary late"* — becomes a lever somebody can pull in
  a famine questline.

Each step replaces one conjuring with one purchase; none changes the stall, the queue, or the
verbs. That is the test that the M3 shapes are right.

## 7. Bevy

- **New offer-prop visual keys** (`loaf`, `stew`) per [01](01_items_and_stacks.md) §7;
  `herring`/`smoked_eel` reuse the fish mesh at reduced scale.
- **Stall dressing** (nice-to-have, ships with M3 if cheap): the bound stall renders a bread board
  whose prop count tracks the vendor's stock stack — the first time item *state* has had a world
  presence beyond offer props. Snapshot already carries everything needed (vendor's holds +
  quantities).
- The queue itself needs nothing: bodies standing in line are just the round's walkers, and the
  market crowd on Highmarket is already M4/M7's shipped behavior.

## 8. Observability

- `--trace-food` headless flag, twin of `--trace-water` (`cathedral_headless.rs:169-170`): a
  `[food]` census line each tick (open stalls / bound vendors / queued / serving / hungry / famished)
  and a `[food]` line per sale with price and stock remaining.
- `Engine::food_summary()` mirroring `water_summary()` (`engine.rs:1000`), so the drive HUD and
  tests share the numbers.
