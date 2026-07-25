# Offers that end by themselves, and a HUD that says why

Status: implemented 2026-07-25. Amends `features/implemented/giving_things.md`
(whose Decision 2 — "offers persist, no auto-expiry" — no longer holds for
targeted offers).

## The two problems

**1. A promise nobody could answer never ended.** `World.offers` had no expiry
but the city gate. An offer stood until it was accepted, declined, retracted or
replaced — and *walking away did not resolve it, it made it unresolvable*, since
`accept_offered_item` and `decline_offer` both require 4 m. So an NPC who held
something out to a player who then wandered off kept:

- the offer on their sheet's `you_offer` every turn, forever (that section has no
  distance filter, unlike `offered_to_you`);
- their right arm extended toward the player's position at any range
  (`src/smart_actors/hands.rs`, npc_bodies M2);
- the offered units **committed**: `uncommitted_quantity` subtracts them, so the
  giver could not eat, sell or pocket their own bread until something resolved
  it. For the player that meant a stack silently locked with no HUD trace of the
  offer that locked it, retractable only with R while that exact item was the
  selected quickbar entry.

**2. A refusal was one line in an overwrite-prone slot.** Declines toasted for 4
seconds in the single `transient` slot any later event overwrites. Since a
refused offer looks exactly like a lapsed one in the world — the item simply
stays put — the player could easily never learn *whether* they were refused, let
alone why.

## What was built

### The lapse (sim)

`OFFER_LAPSE_RADIUS_M = HEARING_RADIUS_M = 20 m` (`lib.rs`), and
`actions::lapse_distant_offers(&mut World)` — a pure distance sweep, no clock:

- **targeted offers only.** A broadcast names nobody to drift from and travels
  with the giver, so whoever stands beside them can always still take it.
- **both parties present.** Someone beyond the walls already has the road party's
  own gate expiry (`round.rs`).
- **strictly greater than 20 m**, inclusive at exactly 20 m — the same boundary
  `offered_to_you` uses.
- removes the offer, delivers a percept to each party naming the other and the
  item, and emits **one `lapse_offer` world event** (actor = giver, target = the
  one it was held out to) with both as recipients — and nobody else, because at
  that distance no bystander can see them both.
- **no priority nudge**, deliberately, unlike an answered offer
  (`Engine::player_offer_reply`): the lapse fires precisely when the two are far
  apart, which is when the stage gate is right to leave the giver unprompted. The
  percept keeps in their inbox until they next think.

Swept once per `Engine::poll`, after the command loop (so the player's own
position update that poll counts) and before the scheduler (so the percept is on
the sheet of anyone prompted in the same poll).

`lapse_offer` is **not a verb**: no reply can ask for it, and no model has to
remember to.

### The notice (host)

`SmartActorHudState.offer_outcome` — its own slot on its own 8 s clock, so a
toast cannot swallow it. Rendered by `OfferOutcomeText` at 65 % height (between
the offer card it replaces at 58 % and the toast at 73 %) in `OFFLINE` red, two
lines: headline, then the cause.

`describe_offer_rejection` (`src/smart_actors/mod.rs`) produces it for
`decline_offer` and `lapse_offer` whenever the player is one of the two parties,
and the event then skips the toast. A refusal between two other people is news,
not feedback, and stays the plain toast it always was.

| what happened | notice |
|---|---|
| they refused yours | `OFFER DECLINED` / `Ilse refused the spark you held out` |
| you refused theirs | `OFFER DECLINED` / `You refused the spark Ilse held out` |
| lapsed, you were the giver | `OFFER LAPSED` / `You and Ilse drifted more than 20 m apart — you keep the spark` |
| lapsed, it was held out to you | `OFFER LAPSED` / `You and Ilse drifted more than 20 m apart — the spark stays with them` |

The 20 m in that text is formatted from `cathedral_sim::OFFER_LAPSE_RADIUS_M`, so
it cannot drift from the rule.

`retract_offer` was left alone: a withdrawal is the giver's own act, neither of
the two reasons this feature is about, and its existing toast already reads well.

## Tests

- `actions.rs`: lapses out of earshot; survives exactly at 20 m; broadcasts never
  lapse; a departed party is left to the gate; the freed units become spendable
  again (an `eat` that failed with `item_committed` succeeds after the lapse);
  the player gets the recipient entry but no inbox prose.
- `engine_tests.rs`: a player `SpatialUpdate` 40 m up the street lapses the
  offer, reaches the host as a `lapse_offer` message with the player on the
  recipient list, and nudges nobody.
- `smart_actors/mod.rs`: the notice text both ways for both reasons; a third
  party's decline still returns `None` and toasts instead; and the whole plugin
  end to end — Ilse holds her coin out, the card appears, the player walks 40 m,
  and the HUD carries the headline and the cause while the card is gone.
- `smart_actors/hud.rs`: the notice outlives a toast written over it, is cleared
  on disconnect, and spawns hidden and clear of its neighbours.

## Not done

- **In-game visual confirmation — a human still has to look at it.** Six of
  eight drive-mode launches hit the known
  `bevy_pbr::atmosphere::environment.rs:116` startup panic (it reproduces on the
  untouched `HEAD` build too, so it is not this change); the one that came up
  spawned the player 20 m from Ilse, so the scripted offer never landed; and the
  retry after that ran its whole script but wrote no session directory at all
  (the launch-cycle resource exhaustion noted for the Bellfoot work). The
  full-plugin test covers the same path headlessly. To see it, with Ilse's
  current position read off her prompt sheet's `you_are` line:

  ```sh
  CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE='wait-online; tp 0 2 129 180; sleep 1;
    key Enter; type Please offer me your coin; key Enter; sleep 12;
    shot 01_offer_card; tp 0 3 178 180; sleep 4; shot 02_offer_lapsed; quit' cargo run
  ```

  (Handy while scripting this: in fake mode Ilse re-issues the scripted offer
  every turn while it keeps failing, so walking into her 4 m is enough to make
  one land.)
- Broadcast offers still stand forever, by design (see above).
- Nothing tells the player that an offer *of theirs* is still standing — the
  lapse now bounds how long that can silently lock a stack, but a "you are
  offering X to Y" HUD line remains unbuilt.
