# Giving things (item transfer)

Status: implemented in the Bevy/Python smart-actor slice; see
`features/implemented/smart_actors.md`.

The consent and ownership rules below remain normative. References to actors
being at the same textual location are superseded by the smart-actor feature's
inclusive 4 m interaction checks and 20 m event-delivery radius.

## Summary

Items change hands via a two-step handshake: one character offers an item
they hold, the other accepts it on a later turn. Items never move without
the receiver's consent, and the one-turn latency paces the exchange
naturally in the round-robin loop ("here, take this fish" → next turn:
accept + a thank-you line).

## Naming

Originally sketched as `give`/`take`, renamed to `offer_item` /
`accept_offered_item` / `decline_offer` / `retract_offer`. Rationale: the verb name is most of the
semantics the model sees, and models narrate and **remember** based on it.
`give` would produce memories like "I gave Ilse the fish" while the fish is
still pending in the giver's hand; `take` invites taking things that were
never offered (ground, stalls, other people's pockets). `offer`/`accept`
encode the actual state transitions.

## Verbs

### `offer_item {"item_id": "<item id>", "target": "<char id>"}`

Offer an item you hold to someone at your location.

- `target` optional: omitted or `null` (equivalent, per the general
  omit⇔null convention in AGENTS.md) means the offer is a **broadcast** —
  anyone at the location may accept, first `accept_offered_item` wins.
- Validation: actor holds the item; target (if given and non-null) exists,
  is at the same location, and isn't the actor. Failures → `system:` event
  in the actor's own inbox. A bad target id is an error, NOT a fallback to
  broadcast (a typo must not turn into "offered to everyone") — but `null`
  is not a bad id, it IS the broadcast form.
- Effect: `world.offers[item_id] = (giver_id, target_id | None)`. The item
  **stays in the giver's `holds`** until accepted.
- An item has at most one pending offer (dict keyed by item id). Re-offering
  replaces the old offer; if that changes/removes the target, the jilted
  target gets an inbox event ("X withdrew the offered fish"). Re-offering
  the same item to the same target just re-sends the event (a nudge).

### `accept_offered_item {"item_id": "<item id>"}`

Accept an item currently offered to you (or offered broadcast).

- Validation happens at accept-time against the offers table: the offer
  exists, is addressed to you or broadcast, and the giver is still at your
  location (future-proofs `move_to`). Otherwise → `system:` error
  ("nobody is offering you that").
- Effect: item moves `giver.holds` → `accepter.holds`, offer cleared.

### `decline_offer {"item_id": "<item id>"}`

Decline an item offered to you — the receiver-side resolution, so state
cleanup doesn't depend on the giver noticing a verbal "no thanks" and
retracting (models are demonstrably lazy about that kind of hygiene).

- Only valid for offers **targeted at you**. Declining a broadcast offer is
  a `system:` error ("not for me" ≠ "nobody wants it") — just ignore those.
- Effect: offer cleared, giver keeps the item.

### `retract_offer {"item_id": "<item id>"}`

Withdraw your own pending offer (explicit GC — offers otherwise persist
forever). Target (if any) gets an inbox event.

## Events

All rendered per-listener through `sim.identify()` (strangers stay
strangers). Item names are public.

| action                | target's inbox                                              | bystanders' inbox               | giver's inbox                     |
| --------------------- | ----------------------------------------------------------- | ------------------------------- | --------------------------------- |
| offer_item (targeted) | `Sven held out a fish (id fzbn9) to you`                     | `Sven offered a fish to Conny`  | —                                 |
| offer_item (broadcast)| everyone: `Sven held out a fish (id fzbn9) to anyone who wanted it` |                         | —                                 |
| accept_offered_item           | —                                                           | `Conny took a fish from Sven`   | `Conny accepted the fish (id fzbn9) you offered` |
| decline_offer         | —                                                           | `Ilse declined a fish from Conny` | `Ilse declined the fish (id fzbn9) you offered` |
| retract_offer         | `Sven withdrew the offered fish (id fzbn9)`                 | —                               | —                                 |

Events are descriptive history, past tense, with **no accept-syntax hint**:
by the time a character reads them they can be stale (a broadcast offer is
gone once someone earlier in the round accepts it, and models act on hints in
stale events). The accept syntax appears only in `offered_to_you` on the
sheet, which is re-rendered from `world.offers` every turn — so the hint's
presence always means the accept would actually succeed.

## Character sheet

The inbox event alone is not enough — characters have no memory, so a
pending offer must be visible every turn, not just when it happens:

- `you_offer`: `[{"item": {"id", "name"}, "to": {"id", "name"} | "anyone"}]`
  on the giver's sheet. `to.name` is rendered through `identify()` (so it
  may be the unknown-marker), exactly like people in `you_see`.
- `offered_to_you`: `[{"item": {"id", "name"}, "from": {"id", "name"}}]` on
  the potential accepter's sheet, with the accept syntax hinted
  (`accept_with`) — the only place accept syntax appears (see Events).

## Edge cases

- **Offer invariant:** an item only leaves a character's hands via
  `accept_offered_item`, which clears the offer — so "offer exists" ⇒ "giver still
  holds it". Keep a defensive check at accept-time anyway.
- **Name vs id:** ids only. `{"item_id": "fish"}` is a `system:` error — no
  name-fallback resolution. The sheet always shows the id next to the name,
  and the offer events/hints repeat it.
- **Self-offer** → error.
- **Refusal:** `decline_offer` resolves a targeted offer; simply ignoring
  one is also fine (it stays pending until declined, retracted, or
  replaced). Broadcast offers can only be ignored, not declined.
- **Simultaneous barter doesn't exist:** a trade is two offer/accept pairs
  in opposite directions; one side trusts first. Intentional — negotiation
  is where the roleplay is.

## Decisions

1. Untargeted `offer_item` = broadcast offer: **yes**.
2. Offers persist (no auto-expiry); explicit GC by the agent via
   `retract_offer`: **yes**.
3. Quantities/money: deferred. Items are singular entities; the demo seeds
   a single `{"id": "c0prs", "name": "copper coin"}` for Ilse so a purchase
   can emerge. Stacking ("two coppers") is a later feature.
4. Receiver-side refusal: separate `decline_offer` verb rather than folding
   accept/decline into one verb with a `"choice"` enum arg — verb names are
   what models get right most reliably; enum values drift ("refuse",
   "reject", ...) and would need extra validation.
5. Item argument key is `item_id` (not `item`) across all four verbs, for
   clarity that it takes an id.

## Demo scenario

Ilse (hungry pilgrim, holds a copper coin) + Conny (fishmonger) + Sven
(holds a fish, owes Conny two coppers): a full purchase should emerge —
ask price → offer coin → accept coin → offer fish → accept fish.
