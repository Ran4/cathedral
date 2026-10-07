Status: Planned (2026-10-07).

# M6 — Things in places

From: old M7 objects ([archive](archive/M7_objects_and_examinations.md)), M6's grants
([archive](archive/M6_access_and_permissions.md)) and M8's dated errands
([archive](archive/M8_activities_and_appointments.md)). R6 in [PLAN_REVIEW](PLAN_REVIEW.md). Entry: M5.

Items exist only in hands today. This milestone puts them on shelves and in desks, makes papers readable, lets an
owner grant a search, and lets an NPC move an item on a schedule.

## Scope

- **Containers.** Shelves, chests, racks and desks in M5's rooms; take, put and look-in. An item has one location:
  a hand, a pocket or a container.
- **Documents.** A document item with readable text and a read panel that keeps its position and never pauses (D05).
- **Examinations.** A data table of (method, object kinds) → result, carried out by an examiner over time. A completed
  examination mints a receipt in the Case tab; an interrupted one gives none.
- **Grants with expiry.** Consent to enter or search a named scope, given by the owner, checked when the act commits.
  Searching outside the scope or after expiry is refused with a reason.
- **Dated errands.** An NPC moves an item at a calendar office (e.g. Lamplight). A warning they actually received can
  change the errand; one the player only wrote in notes cannot. The same errand runs a stand-in demonstration on
  request.

## Not in scope

Object lineage, presentation descriptors, signed receipts, loans, authenticated copies, the activity kernel, a
strategy DSL, an entitlement economy.

## Dev fixture (D01) and lockstep (D06)

In the M5 dev building: a shelf with a marked item, a locked desk, a dev document and a keeper who moves the item at
Lamplight. Lockstep: save with the item on the shelf and the errand pending; the restored run moves it once.

## Acceptance (player-visible)

1. Look in the shelf, find the marked item, take it.
2. Read the dev document while the city keeps running; close it and reopen at the same place.
3. Ask the owner for consent to search the chest: the chest search succeeds, the desk search is refused as outside the
   grant, and after the grant expires the chest is refused too.
4. At Lamplight the keeper carries the item from the shelf to the desk; warned first, he does something else, and a
   player watching sees which.
5. An examiner compares two objects; the completed result appears in the Case tab, an interrupted one does not.
6. Save with the errand pending, load: the item moves once and is never duplicated.

Edge cases worth reusing: archive V30 (one identity across transfer, swallow and save), V31 (interrupted comparison),
V35 (a demonstration whose stand-in skips a step), V37 (only received warnings change behaviour).
