# Milestones

Five milestones, each shippable, each verifiable with the repo's existing tools
(`cathedral-headless`, `CATHEDRAL_DRIVE`, the golden fixtures), plus one explicitly deferred. The
dependency shape: **M0 is the foundation and the risk** (every layer touches items, ~13 golden
fixtures pin the bytes); **M1 is pure content and fully parallel**; M2→M3→M4 stack on M0.

---

## M0 — Kinds & stacks

**Ships:** the embedded item catalog (`assets/world/items.json`);
`Item {id, kind, quantity, metadata}` with derived display names; merge/split semantics;
`offer_item` quantity + `Offer.quantity`; `eat` decrements + `NotEdible`/`BadQuantity` errors;
counted percept/HUD lines; snapshot/mirror/HUD plumbing; seed + fixture-manifest migration; **the
fixture regeneration** (shared with M2's condition line and M4's `you_sell` if they land close —
coordinate to pay it once, [01](01_items_and_stacks.md) §7).

**How you know:**

```sh
cargo test -p cathedral-sim                       # merge/split/quantity unit tests green
cargo test -p cathedral-sim --test golden_prompts # regenerated fixtures byte-stable
cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 8
```

The headless fake run's world dump shows `spark (c0prs) ×3`; a scripted turn offers
`quantity: 2`, the counterpart accepts, and the dump shows 1 and 2. In-game:
`CATHEDRAL_DRIVE='wait-online; key Tab; shot hud_stacks; quit'` — the HUD renders a stack with a
count.

**Definition of done for the invariants:** two same-stuff stacks on one holder is an
`assert_invariants` panic in a test; an unowned item still rejects the snapshot Bevy-side; quantity
0 is unrepresentable.

## M1 — The spark standard

**Ships:** the lore sweep of [02_the_spark_standard.md](02_the_spark_standard.md) §3 (bell and
lantern excised as money, bell prices converted at 12 sparks to the bell, spark prices untouched);
the catalog's `price_sparks` column filled; wallet seeding constants decided.

**How you know:**

```sh
grep -rin 'silver bell\|gold lantern\|twelve sparks\|sixty bells' lore/ features/
```

returns only the founding-marks history (which keeps its mark-not-a-coin framing) and this folder.
A spot-read of `trade_and_daily_life.md` §Money is three sentences about the spark. The 2-spark
loaf survives untouched in every file that stated it.

*Runs fully in parallel with M0 — content only, ideally one subagent per file group with §3's
conversion table as the contract.*

## M2 — Hunger

**Ships:** `Needs.hunger` + constants; universal decay in the renamed `decay_needs`; satiety on
`eat`; seeding spread (Ilse at 25); ladder rungs 3 & 7 (with `FAMISHED_PRESSURE`); the hearth
refill at meal offices; buyer wallets ([02](02_the_spark_standard.md) §4); the computed
`hungry`/`famished` condition on the sheet; Ilse's lore sheet drops static `"hungry"`. Optional:
`TooFar` raised on over-long errands.

**How you know:**

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- --fake --trace-food --watch-clock 1
```

The `[food]` census shows hungry counts climbing through the morning and collapsing at High Wick
(the hearth refill — stalls don't exist until M3). A unit test walks one actor: seed hungry → sheet
says `famished` → `eat` a fixture loaf → sheet says neither. The 20 golden fixtures stay stable
(fixture worlds are un-enrolled, [03_hunger.md](03_hunger.md) §7).

## M3 — The bread round *(the vertical slice)*

**Ships:** `FoodStall` + the seven pitches in `assets/world/food.json` (incl. the **Bell and
Ladle area**, closing the movement plan's known gap); vendor binding; the Kindling restock + Watch
ledger; the FIFO queue; the silent purchase (atomic swap, eat-at-pitch or carry); self-percepts;
`coin_clink` (and optionally `market_cry`) player-only sounds; `--trace-food` sale lines +
`food_summary()`; stall-dressing props if cheap.

**How you know:**

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- --fake --trace-food --watch-clock 1
```

On a Highmarket day: `[food]` logs the Kindling restock (stock counts per vendor), a morning of
sales at the Wickmarket with prices and dwindling stock, vendor coin stacks growing, and the Watch
reset closing the books. Cross-check invariants: total sparks constant between resets; no stack
ever 0.

In-game, on a Highmarket noon:

```sh
CATHEDRAL_DRIVE='wait-online; tp -28 25 356 0 -35; shot wickmarket_noon; sleep 20; shot wickmarket_queue; quit' cargo run
```

The shots show a queue at the bread pitch, a purchase resolving, an eater at the bench. Then walk
up and *ask* — a buyer can say what they paid, a vendor can say what they've sold, from
self-percepts alone, zero extra turns.

## M4 — The Ilse purchase

**Ships:** `you_sell` on bound vendors' sheets; the quantity examples + one-sentence stack prose in
`turn.j2`; the two new fixtures (vendor-with-stock, famished-holder); HUD coin-count offer prompt
for the player's purse ([05_the_llm_seam.md](05_the_llm_seam.md) §7).

**How you know — the acceptance test this whole folder exists for.** Yesterday's session
(`logs/session_224_2026-07-17_14_46_02`), replayed against a real market:

1. Ilse (famished, 1 spark) is told to try the Wickmarket;
2. she arrives; the *sheet* — not improvisation — prices the baker's loaf at 2 sparks, which she
   cannot pay, and the provisions stall's herring at 1, which she can;
3. she offers `c0prs` ×1, the vendor accepts, offers a herring, she accepts, she **eats**;
4. her next sheet has no hunger condition, her wallet is empty, and her goal
   ("Find affordable food") is hers to clear.

Verified headless with a scripted conversation (`--one-shot` style), then live:
`CATHEDRAL_DRIVE='wait-online; tp -19 2 342; type go buy yourself some bread; …'` with the prompt
archive (`logs/latest_session/prompts/`) showing the baker's turn *reading prices off the sheet*.
The three-NPC hallucinated-market failure mode of session 224 is structurally impossible for bound
vendors.

## M5 — The supply chain *(scoped: [07_the_supply_chain.md](07_the_supply_chain.md))*

Country carriers through the gates → grain as an item → millers milling flour at the Wool Gate →
bakers baking at a bakehouse on the grain road — each step replacing one conjuring with one
purchase, until the Kindling restock and the Watch ledger are both dead code. The sketch was
[04](04_the_bread_round.md) §6; **[07](07_the_supply_chain.md) is the scoped version**, split into
four shippable sub-milestones:

| | ships | the cheat it kills |
|---|---|---|
| **M5a** | `Presence` (off-map actors); the carrier round; the fixed carrier roster at the Wool and Stone gates; `grain` | — (millers can buy) |
| **M5b** | `flour`; the `Transform` verb; the Wool Gate mill | — (bakers can buy) |
| **M5c** | the bakehouse; bakers bake what they sell | **`restock`**, for bread |
| **M5d** | takings leaving through the gate; institutional payroll | **`close_books`** |

Two design commitments from that document are binding on anything downstream: **the cast is fixed
and hand-authored — no procedural travellers** ([07](07_the_supply_chain.md) §1), and **the gate is
the edge of the world** — nav stops at the gate nodes and this feature does not extend it
([07](07_the_supply_chain.md) §2).

When it starts, `features/the_near_countryside__aka_add_market_stalls.md` merges into this folder.

---

## Risk ledger (what to watch while building)

| risk | mitigation |
|---|---|
| M0's blast radius — every layer, 13+ fixtures, `deny_unknown_fields` everywhere | land it as one PR with the regeneration; no partial states. The manifest edit is mechanical; the split/merge logic is the only genuinely subtle code — test it to death (property test: any sequence of offers/accepts/eats conserves total quantity per kind) |
| double-print of hunger ("hungry" static + computed) | M2 removes Ilse's static string in the same change that adds the computed one; grep the other 513 sheets for `"hungry"` in `conditions` |
| market percept flood | the discipline is already designed-in (self-percepts + player-only sounds, [04](04_the_bread_round.md) §5) — the risk is a future contributor "just adding" a bystander line per sale; the census counter makes regressions visible |
| coin conservation bugs (sparks minted/burned by a bad merge) | the M3 headless cross-check (total sparks constant between ledgers) as a standing assertion under `--trace-food` |
| the LLM offers `quantity` on a non-stack or over-holds | ordinary action errors (`BadQuantity`), which the engine already routes back as system lines — the model self-corrects like it does for every other arg error |
