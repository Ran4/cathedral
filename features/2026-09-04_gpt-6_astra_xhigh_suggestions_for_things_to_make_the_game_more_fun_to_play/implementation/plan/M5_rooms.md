Status: Planned (2026-10-07).

# M5 — Rooms: floors, doors, locks, senses

From: old M4 layered navigation ([archive](archive/M4_layered_navigation.md)), M5 perception's space filter
([archive](archive/M5_perception_and_observations.md)) and M6's physical access
([archive](archive/M6_access_and_permissions.md)). R5 in [PLAN_REVIEW](PLAN_REVIEW.md). Entry: M4.

The quest needs a few authored upper rooms that people actually walk, with doors that matter and walls that stop
sound. The gaps are real: the player controller has no step-up, nav is 2D at `WALK_Y` (about 60 sites), the escort
ignores y (`custody.rs`), and no door or lock state exists.

## Scope

- **Floors.** Player step-up on stairs; y-aware navigation for NPCs; an escort that keeps both bodies on the stair and
  the floor they are on.
- **A dev building (Q2).** A separate two-storey building with a stair, upper rooms and a locked door, not the
  Tallage (which comes at M8). Ground-only routes elsewhere keep working.
- **Doors and locks.** Open/closed/locked state per threshold that nav, collision and senses all read. An NPC
  reroutes around a locked door or, holding the key, unlocks it. Keys are non-stackable items matching a lock
  pattern; possession is separate from permission (see [OWNERSHIP_AND_GATES](OWNERSHIP_AND_GATES.md)).
- **Senses.** One space/portal filter for hearing and sight: a street NPC no longer overhears upstairs, and a closed
  door blocks what it should.
- **Fix: `GutEntry`** keeps the `ItemId` of non-stackable items, so a swallowed key comes back as the same key.

Q1 (D08): systems are finished on dev fixtures before case authoring. Q4: the case's private route stays upstairs.

## Not in scope

Route certification, a shared spatial crate, crossing yielding, a capability registry, loans and copies, an
observation archive tier, swept continuity, the host line-of-sight ray (M10).

## Dev fixture (D01) and lockstep (D06)

The dev building with two NPC posts upstairs and one locked room. Lockstep: save with a door locked, one open and a
key in someone's pocket; the restored run routes the same way.

## Acceptance (player-visible)

1. Walk up the dev building's stair without flying; an NPC walks up the same stair to their post.
2. Lock the door on an NPC's round: they go around it, or unlock it if they hold the key.
3. A conversation upstairs behind a closed door is not heard by someone in the street below; someone in the same room
   hears it.
4. A guard escorts a person down the stair without either clipping through the floor.
5. Swallow a key and recover it: the same key opens its lock; a key of another pattern does not.
6. Save with doors in mixed states, load: the same doors are open, closed and locked.
7. After the nav rebake, ground-floor routes, places, homes and shelters still work (re-pin them).

Edge cases worth reusing: archive V19 (L-corners, landings, low headroom, overlapping rooms), V22 (perception across
floors and doors), V26 (wrong, borrowed, swallowed keys), V29 (a door closed during traversal).
