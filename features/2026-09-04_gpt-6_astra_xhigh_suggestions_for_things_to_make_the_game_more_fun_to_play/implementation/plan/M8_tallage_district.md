Status: Planned (2026-10-07).

# M8 — The Tallage district

From: old M14 ([archive](archive/M14_tallage_district.md)); replaces
[archive/SPATIAL_PROOF_PROTOCOL.md](archive/SPATIAL_PROOF_PROTOCOL.md) with the design rule below and one
route-measurement script. R8 in [PLAN_REVIEW](PLAN_REVIEW.md). Entry: M7.

The largest risk in the quest, and the first real consumer of rooms, doors and containers. The discovery should come
from understanding the architecture: two distant public entrances belong to rooms connected above the street (Q4).
D03 authorises a larger redesign.

## Scope

- **The places** ([CASE_CONTRACT](CASE_CONTRACT.md), manuscript §C): pawn counter (A), hoist yard (Y), comparison
  room (C), lower beam (W), writer's station (G), loft rack (H), Corin's private desk and a review site, with real
  interiors, doors and containers from M5/M6. Current anchors (Tallage ≈ `(-213.5, 63)`, pawnshop ≈
  `(-188.8, 33.1)`, beam ≈ `(-214.2, 45.5)`, the Tally Bridge over the Cut) are references, not fixed coordinates.
- **The hoist (Q10).** A visible hoist cycle the player can watch and time, plus tenders who can testify that it ran
  uninterrupted. Its cycle is the blind interval.
- **Design rule.** Geometry, nav, places, doors, housing and collision describe the same spaces. No invisible
  blockers, no hidden speed changes, and a route that actually works is legal. The public detour must be measurably
  longer than the blind interval, with a margin chosen before measuring. If it is not, change the site or the
  interval, not the rules.
- **One route-measurement script.** It measures the private route and the best public detour (nav, player jumps,
  roofs, the Cut), plus the hoist interval, and prints them with the margin. Sightline check: the counter is not
  visible from Lise's yard position. Acoustic check: the cry reaches the two witness positions. Measured, not
  exhaustively proved (Q11); the developer also tries to beat the route.
- **Rebake chain.** After the rebake, re-pin places, homes, shelters and `resident_places`, and re-assign household
  doors (the bake is four steps; node pins rot silently on a renumber).
- **Saves.** An old save against the new map is refused before replacing the world, with a readable reason.

## Not in scope

Certification maths, a permissive lower-bound model, observer-coverage proofs, the cast, history or evidence (M9).

## Dev fixture (D01) and lockstep (D06)

The district works with the case disabled: tenants, carts, toll keepers and residents use it, and guards can search
and escort through it. Lockstep: save with NPCs on the upper route and a door in mid-use.

## Acceptance (player-visible)

1. Walk the district: from Lise's yard position the counter cannot be seen.
2. The script prints the private route, the best public detour (including jumps, roofs and the Cut) and the hoist
   interval; the detour is longer than the interval by the chosen margin.
3. The hoist runs a cycle the player can watch and time with the clock.
4. NPCs walk the upper route and use its doors; carts and residents go about their business.
5. After the rebake no one starts inside masonry, and every home and shelter still resolves.
6. A save from before the redesign is refused with a reason; the current city keeps running.
7. The developer, playing, cannot beat the measured detour.

Edge cases worth reusing: archive V20 (a jump or roof shortcut absent from the nav graph), V59 (an ordinary district
with the quest disabled), V60 (historical versus present configuration).
