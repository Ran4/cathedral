Status: Trimmed 2026-10-07 to the keys-proposal boundary and the no-case-IDs rule (PLAN_REVIEW item 4). The 2026-09-07 owner table, subcuts and gates are in git history; the milestone files and [README](README.md) govern order and scope.

# Ownership boundaries

## No case IDs in engine code

Engine code (`crates/cathedral-sim`, `crates/cathedral-backends`, `src/`) never branches on an An Alibi in Stone character, object, place or evidence ID to make a system work. Case content is data: a case pack binds existing cast and places and is loaded through the same loader the dev fixtures use (M4). Disabling the case leaves a coherent ordinary city with every capability intact; loading it is never what makes a guard, door, examination, save or appointment work.

Check it with a grep over engine code for the cast's IDs and names (`fc9rn`, `fl5cp`, `fo6gl`, `fw7ub`, `fa8tn`, `fg4br`, `p00a3`, Corin, Lise, Odo, Warin, Averil, Gile, Mott) and the case pack's object/fact namespace. Hits outside case data, character files and tests are defects.

## Boundary with the separate keys proposal

[Keys and locked places](../../../keys_and_locked_places.md) still says "SPEC ONLY — unimplemented", and its milestone numbers are local to that feature. This plan absorbs a required subset; it does not mark that feature shipped.

| Required behaviour | Owner here | Boundary |
|---|---|---|
| Doors with real open/closed/locked state that nav, collision and senses all respect | [M5](M5_rooms.md) | One door state per threshold; an NPC reroutes around or unlocks a locked door. |
| Non-stackable keys matching a lock pattern; possession separate from permission | M5 | A wrong or pocketed key fails; a copied pattern can work with a distinct item ID. |
| Grants with expiry (consent to enter or search a scope) | [M6](M6_things_in_places.md) | A grant covers its stated scope and time only; revocation and expiry are honoured at commit time. |
| Authorised opening of locked premises under an order (D07) | [M7](M7_orders_enforcement_agreements.md) | Entry needs an order actually delivered to the officer who opens; not a warrant boolean. |
| Save/restore of door, key and grant state | Each owner, in its own lockstep scenario | A door mid-operation or an expiring grant restores exactly. |

Still pending in the broader keys feature: impressions and locksmith copies, copy authorisation, loss discovery and re-keying, its acquisition routes, a persistent access network across many places, and a key-ring/learned-lock UI. General lockpicking, burglary and destructible doors are out of scope here. Later keys work consumes these services rather than adding another door or loan store.
