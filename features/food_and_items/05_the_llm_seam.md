# The LLM seam

The ladder feeds the city; the LLM makes it worth talking to. This document is every prompt-visible
change: the verb surface, the sheet, the vendor's price line, the percept wording, what it costs in
tokens, and the one trust problem we are deliberately not solving.

The governing principle is the movement plan's: **the LLM is not the brain — it is a voice, a
memory, and an occasional change of mind.** Food gives it no new powers, only truer state: a
quantity on an offer, a price on a sheet, a condition that goes away when you eat.

---

## 1. Verb surface: one argument, one constraint

| verb | change |
|---|---|
| `offer_item` | gains optional `quantity` (default: the whole stack) |
| `eat` | unchanged shape; consumes **one unit**; now fails on non-food (`NotEdible`) |
| `accept_offered_item`, `decline_offer`, `retract_offer` | unchanged — they act on the offer as a whole |

`assets/prompts/turn.j2`'s example block (today at lines 112-117) becomes:

```
offer_item {"item_id": "fzbn9", "target": "4bfk4"}   # Hold out an item you hold to that person
offer_item {"item_id": "c0prs", "quantity": 3, "target": "4bfk4"}  # Hold out 3 from a stack you hold
offer_item {"item_id": "fzbn9"}                      # Without target: offered to anyone nearby, first to accept gets it
accept_offered_item {"item_id": "fzbn9"}             # Take what is currently offered to you
decline_offer {"item_id": "fzbn9"}                   # Turn down an offer (the offerer keeps it)
retract_offer {"item_id": "fzbn9"}                   # Withdraw an offer you made
eat {"item_id": "fzbn9"}                             # Eat one of something you hold
```

And one sentence joins the item prose (near *"item_id always takes an id"*): *"Items can stack: ×3
after an item means you hold three; offer_item with "quantity" hands over part of a stack, and
eating consumes one."* That is the entire quantity tutorial — the ×N notation on the sheet does
the rest by being self-evident.

## 2. The sheet

Formats specified in [01_items_and_stacks.md](01_items_and_stacks.md) §5: `×N` suffix on
`you_hold`/`you_offer`/`offered_to_you` rows when N > 1, single items byte-identical to today.
Conditions line grows the computed `hungry`/`famished` ([03_hunger.md](03_hunger.md) §5).

## 3. `you_sell` — the vendor's price line

A bound vendor ([04](04_the_bread_round.md) §2) gets one extra sheet section, rendered between
`you_hold` and `you_see`:

```
**you_sell** (your stall's prices):
- rye loaf, 2 coppers
- wheat loaf, 4 coppers
- heel of rye bread, 1 copper
```

Generated from the catalog's `price_coppers` for the kinds in the vendor's stock template — not
from current stock, so a sold-out baker still knows their prices (and can say "come back at the
Kindling"). Unbound actors never see the section (`skip_serializing_if` empty, the
`you_offer` pattern).

What this kills: yesterday's session, where three Wickmarket NPCs *invented* bread stalls and
prices out of conversational politeness. With `you_sell` on real vendors and real stock in
`you_hold`, the improvisation snaps to truth: the baker quotes 2 coppers because the sheet says
so, and hands over `bd7k2` because they hold it.

Haggling remains free roleplay — `you_sell` is what the stall *charges*, and an LLM vendor talked
into generosity is the game working. The ladder's silent customers always pay list
([04](04_the_bread_round.md) §5), so kindness to the player never bankrupts the till mechanically.

## 4. Percepts and history

Counted lines per [01](01_items_and_stacks.md) §4. Two additions worth calling out:

- **The purchase self-percepts** (*"You bought a heel of rye bread from Petronel for 1 copper"*)
  land in `recent_history` via `remember_percept`, not the inbox — they never schedule a turn, but
  the next time that actor *does* speak, their morning is in their memory. The player asking a
  queue "what did you pay?" gets a true answer for the price of one ordinary turn.
- **The eat percept stays terse** (*"Ilse ate a heel of bread"*) — bystander lines don't carry
  prices; commerce detail belongs to the parties.

## 5. Fixtures and cost

- **One regeneration** covers M0's ×N + M2's conditions + `you_sell`
  (`cargo test -p cathedral-sim --test golden_prompts -- --ignored`; the declarative worlds in
  `fixtures/prompts/manifest.json` get kinds/quantities in the same edit —
  [01](01_items_and_stacks.md) §7). Add two new fixtures while there: a bound vendor with stock
  and `you_sell`; a famished actor holding a stack.
- **Token cost is a rounding error**: `×3` is one token; `you_sell` is ~20 tokens on the handful
  of staged actors who are vendors; the conditions word was already being paid as static lore.
  Compare: the sheet's `places_you_know` is ~23 lines. No new inbox traffic exists by design —
  the bread round's percept discipline ([04](04_the_bread_round.md) §5) is what keeps a market
  square from becoming a token furnace.

## 6. The trust problem, left standing

A trade is still two one-way gifts: coin offered and accepted, then bread offered and accepted —
*"one side trusts first"* (`features/implemented/giving_things.md`). With quantities this is now a
clean two-beat ("two coppers for the fish" is one offer each way), but a counterparty can still
take the coin and walk.

We keep it. Reasons:

1. The prompt already teaches the mitigation — record half-done deals as memories ("I took payment
   but have not handed over the goods"), and it works in practice;
2. an atomic `trade` verb doubles the item-verb surface and needs an escrow model the sim doesn't
   have, to solve a swindle that is *content* — a vendor who cheats is a story, and one the lore
   would happily own;
3. the ladder's silent purchases, where atomicity actually matters (no LLM judgment in the loop),
   **are** atomic — the swap happens inside one sim tick ([04](04_the_bread_round.md) §5).

If LLM-vs-LLM commerce ever matters at scale, revisit with an escrowed two-sided offer. Not before.

## 7. The player's side

- The HUD already lists holds and offers via the mirror; it learns to render `×N` and pluralized
  toasts (`model.rs` `describe_world_event`, [01](01_items_and_stacks.md) §7).
- Player offers stay whole-stack in v1 (no quantity picker in the offer UI yet) — with one
  exception worth the wiring: **the coin stack**, where the offer flow prompts for a count (a
  medieval market where you can only offer your entire purse is a comedy, not an economy).
- `PlayerEat` remains unimplemented; the player buys to give, not to live (README §8.4).
