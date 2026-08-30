Status: SPEC ONLY — unimplemented (2026-08-27)

# Keys and locked places

## Pitch

Some places in Ombreval are locked. A key is not a colored permission card consumed by one quest; it is a
small, physical, transferable piece of long-term power over the city.

A bakehouse key may provide food, a bed and a shortcut between lanes. A bell-stair key may grant roof access,
an escape route and the ability to reach a civic signal. A counting-room key may expose ledgers in five
different quests. The interesting question is therefore not only *what is behind this door?* but:

> Who holds its keys, when do they use them, what would make them lend one, and what happens if the wrong person
> is seen entering?

The player can earn, borrow, buy, steal, copy, discover or extort keys. They can also solve the access problem
without possessing one: ask a holder to open the door, arrive during public hours, follow somebody inside,
prop the door, invoke a writ, enter through another route or force the lock. Physical access and lawful
permission are deliberately separate.

## Player promise

- Locked doors are visible promises of real spaces, routes, tools, people or information—not decoration.
- A key obtained in one story remains useful in later stories.
- Important locks support several approaches and never reduce a quest to finding one mandatory item.
- Owners and keyholders live in the simulation. They go to work, lend keys, notice loss, ask for returns and
  react to trespass.
- Entering is a spatial act with time, sound and witnesses. Having the key does not make it lawful.
- The sim owns whether a key fits and whether entry is permitted. Smart actors interpret why the player has it
  and what to do about that fact.

## Why this belongs in the core loop

Keys turn relationships and earlier quests into traversal abilities:

```text
learn what is behind a threshold
    -> learn who can open it
    -> obtain access cleanly, ambiguously or illegally
    -> use the space / route under real time and witness pressure
    -> keep, return, lose or expose the key
    -> carry that changed access network into later quests
```

This is progression without a skill tree. The player becomes capable because they know the city and possess
particular relationships and objects—not because a global lockpicking number increased.

## Current repository reality

The foundations are uneven but useful:

- `assets/world/navigation.json` currently contains **1,101** baked building doors. They are reachable semantic
  endpoints shared by the sim and renderer.
- The renderer draws door leaves, but ordinary buildings remain solid extruded prisms. Only the Lanthorn has a
  real playable interior; NPCs currently reach a home door and disappear into an abstract indoors state.
- `assets/world/items.json` already defines a palmable `key` item kind. It is currently stackable, has no lock
  identity metadata, and no key instances are seeded.
- Items already have stable ids, holders, catalog-validated metadata, transfers, offers, concealment and a
  player inventory UI.
- NPC rounds, `go_to`, named places and home/work doors already provide keyholders with real schedules.
- Witnesses, notices, warrants and custody already provide much of the consequence spine.
- There is no door-state, lock, permission, trespass or general interior-access system.
- There is no full save-game system. Keys can work within a session, but persistent access progression cannot
  responsibly ship until the wider world can be checkpointed.

Therefore this feature must **not** begin by declaring all 1,101 façade doors lockable. Begin with a few
authored access points that have something playable behind them. A locked painted rectangle with no interior,
route or fixture is not content.

## Scope

### This feature owns

- authored access points and their stable ids;
- open/closed/locked/barred state;
- physical keys and matching lock patterns;
- using, lending, copying, losing and returning keys;
- scheduled and actor-driven locking/unlocking;
- player/NPC commands for operating an access point;
- the separation between physical access and lawful permission;
- typed entry, trespass and forced-entry facts for existing witness/law systems;
- nearby interaction presentation and key/access knowledge;
- initial content for a small set of valuable locked places.

### This feature consumes

- the existing item, holder, offer, pocket and inventory systems;
- NPC rounds, places, navigation and player movement;
- actor perception and witness selection;
- notices, warrants, custody and restitution;
- future authored interiors/typed houses;
- future whole-world persistence.

### Non-goals

- locking every home in the generated city;
- a universal burglary simulator in the first milestone;
- Skyrim-style lockpicking minigames;
- random key loot or color-coded security tiers;
- keys that disappear when used;
- treating possession as automatic consent to enter;
- allowing an LLM to declare that a key fits;
- detailed lock metallurgy for hundreds of locks;
- procedural interiors behind every façade;
- making one irreplaceable key a permanent main-quest hard lock.

## The authored access point, not the façade door, is the unit

An `AccessPoint` is a playable threshold with:

- a stable id and display name;
- a real threshold position/radius in the world;
- an outside destination and a playable inside/behind destination;
- one current physical state;
- zero or more accepted key patterns;
- people/institutions who own and keep it;
- public opening hours or an actor-driven locking round;
- a permission policy;
- witness/law framing for unauthorized entry;
- a fail-safe exit rule;
- optional alternate routes and forced-entry behavior.

Access points can initially guard more than full interiors:

1. **Existing interior doors** such as selected Lanthorn thresholds.
2. **Exterior enclosures and civic fittings** such as a reserve gate, yard gate, stair gate or lockup.
3. **Shortcuts** through a covered passage, guild yard or building undercroft.
4. **Small authored front rooms** added by `more_interesting_houses.md`.
5. **Portal-style bounded interiors** if that becomes the chosen house implementation.

Every locked access point must answer: what new action, route, person, evidence, shelter or risk exists on the
other side?

## Door states

Use a small deterministic state machine:

```text
Open <-> Closed <-> Locked
          ^           |
          |           v
          +-------- Barred
```

- **Open:** passage is physically available.
- **Closed:** anyone can open it; the leaf blocks passage, sight and later sound until opened.
- **Locked:** opening requires a matching accessible key, an authorized code-side capability, or a separate
  forced-entry route.
- **Barred:** the outside key cannot open it. Somebody inside must unbar it, or the player must use an authored
  alternate/force route. Use this sparingly.

Closing is not locking. Unlocking is not opening. This matters when the player unlocks a door for somebody else,
leaves it apparently closed, props it open, or needs to conceal that it was used.

Locks must never trap the player accidentally. Ordinary doors always permit exit from inside even when locked;
only explicitly authored custody/barred thresholds may override that rule, and those need a separate escape or
release path.

## Key identity

A key matches a **lock pattern**, not an item id and not necessarily one door. This supports:

- two copies of the same household key;
- one key opening a front and rear door;
- a keeper's key opening several civic fittings;
- a master key accepted by a small authored group of locks;
- re-keying a lock after theft without deleting the stolen object.

Suggested item shape:

```json
{
  "id": "kybkh",
  "kind": "key",
  "quantity": 1,
  "metadata": {
    "key_pattern": "bakehouse_vell_03",
    "display": "Averil's bakehouse side-door key"
  }
}
```

Required catalog changes:

- make `key` non-stackable;
- declare unrestricted `key_pattern` metadata;
- keep the human-facing `display` override for individual keys;
- prevent machine-only `key_pattern` values from becoming visible adjectives if a display override is absent;
- validate that every seeded key pattern is accepted by at least one authored lock, unless explicitly marked
  `blank` or `obsolete` test/content material.

A key is not consumed on use. It must be in accessible carried inventory: a key offered, committed, swallowed or
hidden in a body pocket must first be retracted/retrieved. This reuses the inventory's existing reservation
rules and prevents one object from opening a door while mechanically promised to somebody else.

## Physical access is not permission

Track two independent questions:

1. **Can this actor physically pass the threshold now?**
2. **May this actor lawfully pass it for this purpose now?**

Examples:

| Situation | Physical access | Permission | Result |
|---|---:|---:|---|
| Owner opens their shop during public hours | yes | yes | ordinary entry |
| Player carries a borrowed key for an agreed inspection | yes | yes, bounded | clean entry until the grant expires |
| Player stole the correct key | yes | no | entry succeeds; witnessed/discovered trespass is possible |
| Player has a search writ but no keyholder present | no | yes | lawful authority, but still needs a key, opener or forced-entry order |
| Owner invites player through an open door | yes | yes | no key changes hands |
| Player follows a customer into a private back room | yes | no | no lock interaction, still trespass |
| Player holds an old key after the lock was changed | no | no/unknown | key fails without mutating state |

An access grant should be a typed record with issuer, recipient, access-point scope, purpose and optional expiry.
Do not store permission only in dialogue memory. The LLM may offer or revoke an authored grant; code determines
what the grant covers.

Using a matching key never silently erases trespass. Conversely, permission never makes a locked door open by
magic.

## Ways to obtain or bypass a key

Every major locked place should support at least three approaches, normally including one clean, one relational
and one unlawful route.

| Route | What the player actually does | Persistent cost or consequence |
|---|---|---|
| **Earn it permanently** | Complete work or accept a role whose duties genuinely need the key. | New route/capability; new responsibilities and people who know the player has it. |
| **Borrow it** | Negotiate a purpose and return deadline; receive the real item plus a typed loan. | Late return, damage or loss creates debt, anger and possibly a notice. |
| **Have the holder open the door** | Arrange an appointment and get both people to the threshold. | No inventory reward; dependence on the holder's schedule and willingness. |
| **Call in a precise favor** | Redeem a debt for one opening, one night's access or one key loan. | That favor is gone; the helper becomes implicated if entry was questionable. |
| **Buy or pawn it** | Purchase a spare, redeem a pledged key or buy it from somebody who should not sell it. | A money trail and an informed seller; ownership may still be disputed. |
| **Find a lost/spare key** | Follow clues to a hook, hiding place, drain, work apron or former holder. | The owner may not yet know it is missing; discovery depends on their actual use/check. |
| **Steal it** | Take an unattended key or, once supported, pick a holder's pocket. | Witness/percept and custody risk; the holder can later fail to open their own door. |
| **Borrow then secretly copy it** | Make an impression while the item is in hand, return the original on time, commission a copy. | The relationship appears intact until file marks, a locksmith or unexplained entry exposes the copy. |
| **Commission an authorized copy** | Obtain the owner's permission and pay a locksmith for a second key. | Clean but slow, recorded and potentially expensive. |
| **Bribe or blackmail the holder** | Exchange money, silence, evidence or threat for the item or one opening. | Creates leverage in both directions and a person motivated to recover the key. |
| **Obtain it under a writ** | A lawful officer/custodian signs the key out for a named inspection or seizure. | Narrow permission, custody record and a mandatory return; not a permanent reward. |
| **Take it from a defeated arrangement** | A holder is arrested, dismissed, dies, flees or transfers office; decide who receives their ring. | Changes institutional access and may create competing ownership claims. |
| **Follow, tailgate or prop the door** | Enter while an authorized person uses it, or keep it from latching. | No key; timing, witnesses and later discovery matter. |
| **Use another route** | Roof, window, undercroft, adjoining property or public opening hours. | Different traversal/tool/witness risk; the lock remains intact. |
| **Force or pick the lock** | Use a later burglary/tool seam rather than a key. | Noise, damage, visible evidence and possible re-keying; not part of the first milestone. |

There must be no global “pickpocket to collect every key” strategy. Keyholders, schedules, pockets, key rings,
public hours and consequences should make acquisition contextual.

## Borrowing and returning

A loan is mechanically explicit:

```text
KeyLoan {
    key_item_id,
    lender,
    borrower,
    allowed_access_points,
    purpose,
    lent_at,
    due_at,
    status
}
```

Returning uses the ordinary two-sided item transfer. The loan clears only when the lender or an authored
custodian accepts the exact item. A copied replacement does not satisfy a loan for the original item id, even if
it opens the same lock.

No psychic alarm fires the instant a key changes hands. A holder notices loss when they check the ring, attempt
to use the lock, receive a report, or witness the taking. A missed return can become a deterministic morning
receipt because the due time is known.

## Copying keys

Copying should be a short systemic operation, not a crafting tree:

1. possess the source key long enough to inspect or impress it;
2. obtain wax/clay or a willing locksmith's direct access to the original;
3. obtain a blank/metal and pay, persuade or coerce a locksmith;
4. wait through the authored work interval;
5. receive a new non-stackable key item carrying the same `key_pattern` and its own item id/provenance fact.

The existing catalog already contains keys and wax; locksmith lore already exists. The result works because its
pattern matches, not because a model says it probably should.

An unauthorized copy is not automatically detected. A skilled locksmith may recognize file marks; an owner may
re-key after unexplained entry; witnesses may connect player, impression and locksmith. These are typed facts
and social interpretation, not a random counterfeit roll.

## Locking schedules and people

Meaningful doors should be operated by people rather than all flipping state at an office boundary:

- a verger physically opens and locks a church side door on their round;
- a keeper opens a reserve gate only during an inspection or emergency;
- a merchant opens the front room at Dayspring and locks the counting room whenever they leave it;
- an alehouse bars its door after the Snuffing while a private lock-in continues;
- an arrested, delayed or persuaded keyholder may fail to open on time.

The access catalog declares expected hours and responsible holders, but the sim changes state when the actor
performs the validated action. This lets schedules, custody, illness and player-made appointments matter.

NPC verbs, conditionally rendered only to actors with relevant access, should be narrow:

```text
unlock_access {"access_point":"...","key_item_id":"..."}
lock_access   {"access_point":"...","key_item_id":"..."}
open_access   {"access_point":"..."}
close_access  {"access_point":"..."}
grant_access  {"access_point":"...","person":"...","until":"...","purpose":"..."}
revoke_access {"grant_id":"..."}
```

The dispatcher verifies distance, state, possession, key match, authority and grant vocabulary. An actor cannot
unlock a door from across the city, invent a master key, grant access to somebody else's house or lock a door
whose state they do not control.

## Player interaction

Looking at a usable threshold should show only knowable state:

```text
Averil's bakehouse side door — closed
Averil's bakehouse side door — locked
Step Cistern reserve gate — barred from within
```

Interaction rules:

- `E` opens/closes an operable door or attempts the obvious matching carried key.
- If exactly one accessible key fits, use it without making the player hunt through inventory.
- If several fit, show a small selection using human display names.
- If no key fits, the attempt produces a terse mechanical response and suitable nearby sound/percept.
- Trying a key can teach the player that it fits/does not fit; the casebook/inventory may thereafter list the
  learned connection. Unknown keys do not arrive with supernatural tooltips naming every lock.
- Lock/unlock should have readable hand and sound feedback. The existing key mesh and key-jingle/gaol-door
  sound material can be reused where appropriate.

The inventory UI should collapse several carried keys into a **key-ring presentation** without merging their
item identities. This is UI grouping, not a magical container: individual keys remain offerable, stealable and
returnable.

## Witnesses, discovery and law

Unlocking and entering are separate events:

- using a key can be seen/heard at the threshold;
- passing through can be lawful, invited or trespass;
- forcing a lock creates damage/noise facts even with no human witness;
- leaving a door open can be discovered later;
- taking something behind the door uses the existing item/custody systems rather than being folded into
  `trespass`;
- an owner reacts only to facts they perceive or receive, never global omniscience.

Suggested typed domain facts:

```text
access_unlocked(actor, access_point, key_item)
access_opened(actor, access_point)
access_entered(actor, access_point, permission_status)
access_forced(actor, access_point, damage_kind)
access_left_open(actor, access_point)
key_missing(key_item, expected_holder)
key_copied(source_key, copy_key, locksmith)
lock_rekeyed(access_point, old_pattern, new_pattern)
```

The law layer decides what reported facts warrant. Do not automatically issue a notice merely because the sim
knows entry was unauthorized; an eligible witness, later inspection or authored institutional check must expose
it.

## Navigation and soft-lock rules

- Closed/locked access points block only the portal/edge they own, not the surrounding outdoor graph.
- Opening publishes a small access revision so affected routes can retry; do not republish every actor/item.
- NPCs denied by a required lock wait, seek a holder, choose an authored alternate or emit a legible lapse.
- No critical daily need may depend on one irreplaceable key. A locked food/water route needs a public fallback.
- No ordinary lock can imprison the player from the inside.
- A quest-critical place needs at least two recovery routes after loss, confiscation or re-keying.
- Keys may be destroyed only if the wider item system gains intentional destruction and the content supplies a
  recovery route. Dropping a key into unreachable geometry is never an intended puzzle.

## Suggested data ownership

Rules and mutable access state belong in the IO-free `cathedral-sim`; the host owns asset loading, mesh
animation, collision/portal presentation and input.

Suggested content file:

```text
assets/world/access_points.json
```

Illustrative schema:

```json
{
  "schema_version": 1,
  "access_points": [
    {
      "id": "bakehouse_side_door",
      "display": "Averil's bakehouse side door",
      "building_id": "omb_i0001",
      "threshold_m": [12.0, 0.0, -24.0],
      "destination": "bakehouse_front_room",
      "initial_state": "locked",
      "accepted_key_patterns": ["bakehouse_vell_03"],
      "owner_ids": ["..."],
      "keeper_ids": ["..."],
      "public_hours": ["kindling", "lamplight"],
      "exit_from_inside": "always",
      "permission_policy": "private"
    }
  ]
}
```

The host reads this file and passes parsed/plain values into the sim. Validate duplicate ids, missing actors,
unknown destinations/buildings, non-finite thresholds, invalid state transitions, orphan key patterns and access
points with neither usable destination nor explicit fixture action.

Suggested authoritative types:

```text
AccessCatalog
AccessPointDef
AccessPointId
KeyPattern
AccessState
AccessPointState
AccessGrant
KeyLoan
AccessPermission
AccessFact
```

Likely engine seams:

```text
EngineCommand::PlayerOperateAccess { request_id, access_point_id, operation, key_item_id }
EngineMessage::NearbyAccess(AccessView)
EngineMessage::AccessResult { request_id, result }
```

The host never decides that a key matches. The sim never moves a door mesh or reads a file.

## First content set

Do not choose final sites until checking current lore and geometry. The first slice should nevertheless include
three different value propositions:

| Access-point role | What should be behind it | What it proves |
|---|---|---|
| **Civic fitting** | a reserve gate, records cage or other public-work control | key + narrow writ; lawful versus physical access |
| **Shortcut** | a stair, yard or covered passage connecting two useful routes | a key changes travel planning across quests |
| **Private working room** | a bakehouse room, counting room or workshop with people/items | owner invitation, loan, theft and witnessed trespass |

Later candidates:

- bell-stair and roof doors;
- Stone House service/prisoner doors once their geometry exists;
- guild yards and halls;
- merchant counting rooms and warehouses;
- rotating alehouse back doors;
- inn rooms and stable yards;
- Chapter archives and vestries;
- cistern reserve gates and civic lockups;
- a player-owned room whose key may be lent, stolen or copied.

## Milestones

### M0 — Pure key and access model

- Change the existing `key` catalog kind to unique non-stackable items with machine key patterns.
- Add validated access catalog/state to the pure sim behind an absent/default-off content gate.
- Implement open/close/lock/unlock, matching, grants and fail-safe exit rules.
- Add player/NPC command reducers and small access views; no production geometry yet.
- Headless tests cover state transitions, transfer, stale/wrong keys and physical-versus-lawful access.

### M1 — Three playable thresholds

- Author one civic fitting, one shortcut and one private work room/enclosure.
- Add focus prompts, door/gate animation, collision/portal changes and key sounds.
- Seed one permanent key and one held by an NPC whose real round operates a lock.
- Prove that possession changes an actual route or action, not only a UI line.

### M2 — People, appointments and loans

- Make responsible NPCs lock/unlock through their rounds.
- Add asking a holder to open without transferring the key.
- Add typed access grants, key loans, due times, exact-item returns and morning lapse receipts.
- NPC absence/custody can leave a place closed, with authored recovery routes.

### M3 — Multiple acquisition routes

- Ship at least four routes across the slice: earned, borrowed, unattended theft and copied.
- Add impression/locksmith work using existing item and offer systems.
- Add key-ring inventory grouping and learned lock associations.
- Keep pickpocketing, forcing and lockpicking deferred unless their own general seams exist.

### M4 — Witnesses, trespass and re-keying

- Emit typed access facts with real witnesses and integrate reports with existing law.
- Add later discovery of missing keys/open doors and owner-driven re-keying.
- Ensure stolen/copy routes produce consequences without omniscient reactions.
- Exercise one arrest/confiscation/recovery path without hard-locking active content.

### M5 — Persistent access network and content pass

- Expand only after the three-point slice is fun: target roughly 12–20 memorable access points, not every door.
- Integrate with authored typed houses/interiors as they become real.
- Persist access state, grants, loans, patterns, key possession and learned associations through the future
  whole-engine checkpoint.
- Complete accessibility, fake-backend, headless and hidden-window drive acceptance.

## Acceptance criteria

### Rules

- A wrong, missing, reserved or pocketed key cannot unlock a point and leaves state unchanged.
- A matching accessible key unlocks without being consumed.
- Transferring the exact key transfers its physical capability immediately.
- Two copied keys with the same pattern work; their item ids and custody remain distinct.
- Re-keying invalidates old patterns deterministically without deleting old keys.
- Permission and physical access remain independently testable.
- No model reply can invent a match, grant, opening, entry, loan return or re-key.

### Play

- The player can reach the private slice destination through at least three materially different routes.
- At least one clean solution obtains no key at all because the holder opens the door.
- At least one key earned in one situation creates a useful shortcut/opportunity in another.
- Borrowing is legible: lender, exact item, purpose and due time are always recoverable.
- A witnessed unlawful entry can reach the law system; an unwitnessed entry does not create psychic knowledge.
- An ordinary door cannot trap the player or permanently block food, water or a critical quest.
- Trying to use several keys is low-friction and does not become inventory-menu busywork.

### Integration

- Access state and key matching are authoritative in `cathedral-sim` and deterministic under fake/headless runs.
- The host's collision/animation agrees with the latest sim access revision.
- Disabled/absent access content leaves existing prompts and snapshots byte-stable where practical.
- NPC route failure at a lock produces a visible wait, alternative or lapse rather than silent spinning.
- Save/load, when available, cannot duplicate a key, forget a loan or reset a re-keyed lock.

## Risks

1. **There is nowhere behind most doors.** This is the dominant dependency. Build valuable spaces/shortcuts,
   then lock some of them—not the reverse.
2. **Key clutter.** Twenty unique items can become UI sludge. Group them visually, keep the authored set small
   and make obsolete/returned keys leave the active ring.
3. **Binary quest gating.** If one key is the only route, the feature becomes a fetch quest. Require alternatives
   and recovery paths in content validation/review.
4. **Possession becomes magic permission.** Keep law and physical access separate in data, prompts and feedback.
5. **NPC schedules deadlock.** A stolen work key can strand a critical worker. This should create a scene and
   fallback, not permanently break the city.
6. **Omniscient owners.** A missing key is learned through checks, failed use, witnesses or reports.
7. **Door/physics disagreement.** Never animate first and ask the sim later. The validated state transition leads;
   collision and visuals project it.
8. **Save dependency.** A long-term access network is one more reason the game needs a whole-world checkpoint,
   not a keys-only save.

## Open decisions

- Which three existing/authored locations form the first playable slice after a geometry and lore audit?
- Should public opening hours be informational expectations only, or may selected doors have a deterministic
  fallback opener when their keeper is unavailable?
- May the player lock any door for which they hold a key, including from outside? Recommended: yes for authored
  locks, with route fallbacks and an explicit witnessed act.
- Does a lawful officer with a writ gain authority to force entry when no keyholder arrives? Recommended: yes,
  as a separate noisy/damaging command rather than a magic unlock.
- When should a locksmith refuse unauthorized copying, and what evidence/leverage can change that decision?
- Is a key ring only UI grouping, or eventually a stealable container whose loss transfers several keys at once?
  Recommended first version: UI grouping only.

## Related features and sources

- `features/systemic_quest_suggestions.md`
- `features/more_interesting_houses.md`
- `features/implemented/movement/README.md`
- `features/food_and_items/README.md`
- `features/implemented/extra_pockets.md`
- `features/implemented/law_and_order.md`
- `features/implemented/chalking_the_walls.md`
- `assets/world/items.json`
- `assets/world/navigation.json`
- `crates/cathedral-sim/src/item.rs`
- `crates/cathedral-sim/src/nav/mod.rs`
