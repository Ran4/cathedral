# Items: kinds, metadata, quantity

The brief asks for three things the item system cannot say today: *what kind of thing is this and
what is it made of* (metadata), *how many* (quantity), and *what is it worth* (a price the catalog
owns, [02](02_the_spark_standard.md)). This document is the data-model change and every layer it
touches. It is the foundation milestone (M0) because everything else stands on it.

---

## 1. What exists

An item is three strings, and possession is a list of ids:

```rust
// crates/cathedral-sim/src/item.rs
pub struct Item {
    pub id: ItemId,
    pub name: String,
    #[serde(default = "generic")]
    pub visual_key: String,
}
// crates/cathedral-sim/src/character.rs:176
pub holds: Vec<ItemId>,   // "Ordered: accept appends to the end"
```

The invariants, all enforced today and all worth keeping:

- Items live only in `World.items`; possession is by id in `Character.holds`
  (`item.rs:1-2`).
- Every held item exists; every item has **exactly one** holder — `World::assert_invariants`
  (`world.rs:657-692`) rebuilds an owner map per call, and the Bevy mirror goes further:
  a snapshot containing an *unowned* item is **rejected wholesale**
  (`src/smart_actors/model.rs:585-621`, `SnapshotError::UnownedItem`).
- At most one live offer per item (`World.offers: BTreeMap<ItemId, Offer>`); re-offering replaces
  (`offer.rs:1-2`).
- Eating is final: *"Items are singular: eating one removes it from the world forever"*
  (`actions.rs:887`).
- The seed validates unique ids, held-must-exist, held-only-once (`seed.rs:104-167`), and lore
  character sheets may `holds` an item **only if the base seed defines it** — which is why the
  entire shipped world contains two items (`assets/world/seed.json`: Sven's fish `fzbn9`, Ilse's
  copper `c0prs`).

"Quantities/money: deferred" was an explicit ruling
(`features/implemented/giving_things.md:127-129`): *"Items are singular entities… Stacking ('two
coppers') is a later feature."* This is the later feature.

---

## 2. The new shape

```rust
pub struct Item {
    pub id: ItemId,
    /// A row in the embedded item catalog (`assets/world/items.json`).
    pub kind: ItemKind,                       // newtype over String, id-validated
    /// How many. Never 0: a stack at 0 is removed from the world.
    pub quantity: u32,
    /// Small, catalog-declared descriptors: {"flour": "rye"}. Part of identity —
    /// stacks merge only when metadata is byte-equal.
    pub metadata: BTreeMap<String, String>,
}
```

**`name` is gone as stored state.** The display name is *derived*: catalog display name, prefixed
by metadata adjectives in a declared order — kind `loaf` + `{"flour": "rye"}` renders **"rye
loaf"**. One source of truth; a vendor cannot hold a "loaf" that secretly disagrees with its kind.
(The prompt renderer, snapshot, and HUD all render through one `display_name(&catalog)` helper.)

### 2.1 The catalog — `assets/world/items.json`

Embedded with `include_str!` like `rounds.json` (`round.rs:113`), so both hosts and the headless
binary get it with no wiring. One row per kind:

```json
{
  "schema_version": 1,
  "kinds": [
    { "kind": "spark", "display": "spark", "visual_key": "spark",
      "stackable": true },
    { "kind": "loaf", "display": "loaf", "visual_key": "loaf",
      "stackable": true, "edible": { "satiety": 150 },
      "metadata": { "flour": ["rye", "wheat"] },
      "price_sparks": { "": 2, "flour=wheat": 4 } },
    { "kind": "herring", "display": "herring", "visual_key": "fish",
      "stackable": true, "edible": { "satiety": 70 }, "price_sparks": { "": 1 } },
    { "kind": "smoked_eel", "display": "smoked eel", "visual_key": "fish",
      "stackable": true, "edible": { "satiety": 100 }, "price_sparks": { "": 3 } },
    { "kind": "stew", "display": "bowl of stew", "visual_key": "stew",
      "stackable": false, "edible": { "satiety": 170 }, "price_sparks": { "": 2 } },
    { "kind": "generic", "display": "thing", "visual_key": "generic", "stackable": true }
  ]
}
```

Rules the loader enforces:

- `metadata` declares the **only** keys a kind may carry, and the allowed values per key. An item
  with an undeclared key or value fails validation — this is what stops content (or a future
  supply-chain generator) from silently forking the stack space into unmergeable snowflakes.
- `price_sparks` is keyed by a metadata selector (`""` = default, `"flour=wheat"` overrides).
  Prices live here and nowhere else — the ladder's silent purchases and the vendor's `you_sell`
  sheet line ([05](05_the_llm_seam.md) §3) quote the same number.
- `generic` stays as the escape hatch for test worlds and one-off props (the manifest's anvil and
  rope become `kind: "generic"` items with a `display` override — see §7).

### 2.2 Identity and merging

Two stacks are **the same stuff** iff same `kind` and byte-equal `metadata`. The merge rule:

- **On accept** (and any other transfer): if the receiver already holds a same-stuff stack, the
  transferred quantity folds into the receiver's existing stack — **the receiver's id survives**,
  the moving id disappears (if it moved wholly) or stays with the giver's remainder (if split).
  Receiver-id-wins keeps every id the receiver's LLM has already seen in its history valid.
- **Never merge across holders**, never merge on offer — offers do not move items.
- `quantity == 0` is unrepresentable: any operation that would produce it removes the stack from
  `World.items` and the holder's `holds`.
- `stackable: false` kinds (`stew` — a served bowl, not a commodity) always occupy one stack of
  quantity 1; a second bowl is a second id. This keeps "a bowl each" natural at the tavern bench.

`assert_invariants` gains: quantity ≥ 1; metadata valid per catalog; **no two same-stuff stacks on
one holder** (they should have merged — finding two is a bug, same spirit as the existing
duplicate-holds assertion).

---

## 3. The verbs

The surface stays small. One verb grows an argument; the rest keep their exact shape.

### 3.1 `offer_item` gains `quantity`

```
offer_item {"item_id": "c0prs", "target": "4bfk4", "quantity": 3}
offer_item {"item_id": "c0prs"}                  # no quantity: the whole stack
```

- `Offer` gains `quantity: u32`. The offered portion **stays in the giver's stack** — consistent
  with today's *"The item does NOT move: it stays in the giver's holds until accepted"*
  (`actions.rs:556`).
- Validation at offer time: `1 ≤ quantity ≤ stack.quantity`, else a new `BadQuantity` action error
  ("you hold only 2 of those").
- Validation again at accept time: if the giver's stack has meanwhile shrunk below the offered
  quantity (they ate one, sold one), the accept fails through the existing `repair_and_fail`
  stale-offer path — the same pattern that already handles "giver no longer holds that item".
- Accept executes the split-and-merge of §2.2 atomically: giver stack −n (removed at 0), receiver
  same-stuff stack +n or a new stack carrying the offered id if the whole stack moved… and a fresh
  id if a partial split needs one (§6).
- One offer per stack id still holds; re-offering replaces quantity and target, with the existing
  jilted-target percept (`actions.rs:494-512`).

### 3.2 `eat` consumes one unit

`eat {"item_id": "bd7k2"}` decrements the stack by 1 and applies the kind's `satiety` to the
eater's hunger gauge ([03_hunger.md](03_hunger.md) §3). At 0 the stack is removed — the existing
removal path, including the no-`retract_offer`-event offer cleanup (`actions.rs:865-878`), now
fires only when the *last* unit goes. Eating a non-`edible` kind fails with a new `NotEdible` error
("a spark is not food"). Today's `eat` allowed anything; nothing of value is lost.

### 3.3 Unchanged

`accept_offered_item`, `decline_offer`, `retract_offer` keep their exact arguments — they act on
the offer as a whole. No `split`/`combine`/`give_coins` verbs: the LLM's mental model stays "hold
out, accept", now with a number on it.

---

## 4. Percept and event lines

Counted lines pluralize only when n > 1, so single-item traffic — every line in every existing
fixture — renders **byte-identically** to today:

| n | line |
|---|---|
| 1 | `Sven held out a herring (id fzbn9) to you` |
| 3 | `Ilse held out 3 sparks (id c0prs) to you` |
| 3 | `Ilse offered 3 sparks to a stranger (id p003n)` |
| 2 | `You accepted the 2 sparks (id c0prs) Ilse offered` |
| 1 | `Petronel ate a herring` |

Pluralization is naive ("s" appended) with an optional `display_plural` in the catalog for the
irregulars ("loaves"). `DomainEvent`/`EngineMessage::WorldEvent` gain `quantity: u32` so the HUD
toast can say "You accept the 3 sparks."

---

## 5. The sheet

`you_hold` keeps its id-first shape (`prompt/mod.rs:824-827` — *"items are always id-first, like
places"*), gaining a count suffix only when n > 1:

```
**you_hold**:
- c0prs spark ×7
- bd7k2 rye loaf ×2
- fz001 smoked eel
```

`you_offer` / `offered_to_you` likewise:

```
**you_offer**:
- c0prs spark ×3 — to id p003n: a stranger (you don't know their name)
**offered_to_you**:
- hr7k2 herring — from id p003n: Bertran Kern (accept with: accept_offered_item {"item_id": "hr7k2"})
```

The `accept_with` template string (`assets/prompts/strings.toml`) is unchanged — accepting takes
the bundle. The turn-prompt prose and examples change in [05_the_llm_seam.md](05_the_llm_seam.md)
§2; the ×N notation is explained there in one sentence.

---

## 6. Ids for stacks that split and stacks that are conjured

- **Seeded stacks** keep their authored ids (`c0prs` stays `c0prs`, now with `quantity`).
- **Partial-split residue**: when an accept moves part of a stack to a receiver holding no
  same-stuff stack, the moved part needs a fresh id. Ids are deterministic —
  `hash of (parent_id, world event seq)` rendered in the 5-char base-32 style the cast already
  uses — so headless runs stay reproducible and never collide (event seq is unique).
- **Conjured stock** ([04](04_the_bread_round.md) §3): deterministic per
  `(vendor_id, game_day, slot)` for the same reason.
- All well under `MAX_ID_CHARS = 128`; validated by the existing `ItemId::new`.

---

## 7. Migration, layer by layer

The cost of M0 is that items appear at every layer, and most of the layers are byte-pinned. In
dependency order:

| layer | change |
|---|---|
| `item.rs` | new struct; `display_name(&catalog)`; the catalog loader + validation |
| `seed.rs` | `ItemSeed {id, kind, quantity?, metadata?}` (name dropped, defaults: 1, empty); validation against the catalog; `assets/world/seed.json` rewritten (2 items: `fzbn9` "fish" → `kind: herring` — there is no generic fish kind; `c0prs` "copper coin" → `kind: spark`) |
| `actions.rs` | `offer_item` quantity, split/merge in accept, `eat` decrement + `NotEdible`, `BadQuantity`; counted percept lines |
| `offer.rs` | `quantity: u32` on `Offer` |
| `world.rs` | invariants (§2.2); merge helper |
| `prompt/` | ×N rendering; counted history lines |
| `snapshot.rs` | `ItemSnapshot {id, kind, display_name, visual_key, quantity, metadata}` — keep sending the *derived* display name so the host never needs the catalog |
| `engine.rs` | `WorldEvent.quantity`; player commands unchanged (player offers whole stacks in v1 — a HUD quantity picker is later polish) |
| `model.rs` (Bevy) | mirror the new snapshot fields; validation limits (quantity ≥ 1, metadata size cap); HUD toasts pluralize |
| `actors.rs` (Bevy) | new `visual_key` matches: `loaf` (a flattened brown capsule), `stew` (a squat cylinder "bowl") — everything else already falls back to the yellow generic cuboid. One offer prop per stack regardless of quantity (a floating armful of seven coins is not the medieval look; revisit with stall dressing in [04](04_the_bread_round.md) §7) |
| headless | the world-dump prints `name (id) ×n` |
| tests | `fixtures/prompts/manifest.json` items gain kinds (anvil/rope/loaf → `generic` with display overrides, or real kinds where they exist); **regenerate all fixtures once**: `cargo test -p cathedral-sim --test golden_prompts -- --ignored` |

The fixture regeneration is the "most expensive small change" the movement plan kept deferring
(`features/implemented/movement/07_milestones.md:357`). M0 and the hunger condition line
([03](03_hunger.md) §5) land in the **same regeneration** — pay it once.

---

## 8. What deliberately does not change

- **No ground items.** Every item keeps exactly one holder; the Bevy `UnownedItem` rejection stays
  as armor. Stalls do not "contain" items — the *vendor* holds the stock ([04](04_the_bread_round.md) §3).
- **No containers, no weight, no encumbrance.** A stack is a number, not a basket.
- **No item quality decay.** `day_old` bread is a decision for later (README §8.3).
- **No multi-item trades.** A trade remains two offer/accept pairs; the quantity argument makes
  "two sparks for a loaf" a *two-step* trade instead of a three-step one, which is enough
  (the trust question is discussed in [05](05_the_llm_seam.md) §6).
