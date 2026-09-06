Status: Planned (2026-09-05).

# M14 — Build the investigative district

Create places worth investigating, using the accepted reusable systems. The discovery should come from understanding the architecture: two distant public entrances belong to rooms connected above the street.

## Entry

M13's foundation gate is accepted. Review [CASE_CONTRACT.md](CASE_CONTRACT.md) and the actual tools/interfaces that shipped. M0 survey notes are hypotheses, not final placements.

## Site and redesign

The user permits a larger district redesign. Compare at least two plausible site arrangements before committing: adapt the current pawnshop block, or rework a larger set of lanes/property connections while retaining the Tallage's recognisable civic relationships.

For each candidate supply plans and sections, ordinary property/storage/work purposes, every entrance connection and unaffected pedestrian/cart journeys before claiming route distances. Start from the [source-data illustration](evidence/tallage_existing.png); it shows existing footprints and the door/place mismatch, not validated new rooms.

Current anchors include the Tallage at approximately `(-213.5, 63)`, the pawnshop centred near `(-188.8, 33.1)`, the weigh-beam near `(-214.2, 45.5)`, and the Tally Bridge between warehouse and toll-house over the dry Cut. These are audit references, not mandatory quest coordinates. If an established anchor changes under the authorised redesign, update current lore/geography and every projection explicitly; historical implemented-feature records keep their original coordinates.

## Required spaces

| Space | Player experience | Structural requirements |
|---|---|---|
| Pawn counter | Hear the strong alibi; inspect where each sighting occurred | Doorway, counter sightlines, wallet handling distance and a believable route to the yard |
| Hoist yard | Discover the interval in which Lise could not watch | Real occlusion, usable fixture, marked travel and safe operator/viewer positions |
| Comparison room | Reconstruct the shove/removal and examine its traces | Desk, packet work surface, fragment origin, upper entrance and non-traversable back light |
| Lower beam | Place Warin at the relevant cry | A credible working task and distinct witness positions with supported hearing/sight |
| Writer's station | Obtain Gile's account and counterfoil | Ordinary public working position, privacy options and access to nearby activity |
| Loft/rack | Discover the connection and possibly recover the packet early | Storage purpose, reachable rack, alternative consent/search access and no one-way trap |
| Private desk | Recover the packet after a successful retrieval | Real named container, lawful search scope, keeper/access path and no hidden teleport |
| Review site | Present a bounded public reconstruction | Space for necessary participants, spectators and passing city traffic |

Provide visual reasons to look: a latch facing the neighbouring property, a shared rear wall, storage labels, a change in roof/wall construction. Avoid a glowing clue trail or a conveniently incriminating emblem. Finding the passage before hearing the alibi remains useful.

## Timing and spatial proof

Measure the complete historical itinerary and demonstration profile with real NPC movement. Include departure delay, stairs, turns, door operations, the encounter, concealment and the return to the sighting position. Fix the GDD's delay/lower-bound inconsistency as part of re-authoring the chronology.

Survey all admissible public routes in player collision space. Distinguish a tested traversal from a conservative impossibility bound. Test walking/running, open/closed lawful doors, corner cuts and permitted jumps. Developer flight/teleport are debug capabilities, not evidence, and must be visibly flagged as invalid for a demonstration.

Follow [SPATIAL_PROOF_PROTOCOL](SPATIAL_PROOF_PROTOCOL.md). Export named sighting regions, conservative gap `[Gmin,Gmax]`, complete valid private execution with uncertainty `Uprivate`, certified public lower bound `Lpublic`, historical configuration and observer coverage. Select a margin `m` of at least two simulation seconds before tuning, enlarged where uncertainty demands it. The strong relation requires `Uprivate + m <= Gmin` and `Lpublic >= Gmax + m`.

If the proposed 576/48 m topology feels contrived, change the site/interval rather than manufacturing a detour. If the impossibility claim cannot be supported, keep that strong-claim gate unaccepted and assess a concrete narrower inference with the affected dialogue, proof predicates and walkthrough revised together; never reject a valid faster route. Do not silently call opportunity an impossibility result.

## District integration

Update cadastral source, structural interiors, collision, layered nav, entrances, place/area records, map projections, ordinary rounds, housing assignments and affected supply/crowd paths together. Preserve stable authored IDs through rebakes. Review current home/door associations rather than moving residents silently into the case's private rooms.

Keep the civic area usable: carts pass, toll/work positions remain reachable, unrelated residents can go home, and guards can search/escort through the new spaces. Finish art, lighting, signs and interactable silhouettes enough for discovery to be spatially legible. A correct greybox alone is not the completed district.

## Acceptance

- Independent testers can describe the public arrangement and recognise the upper connection without a map spoiler.
- Lise's yard/counter relationship creates a real blind interval from actual eye positions.
- Last/renewed sighting boundaries include actual travel around the mechanism; historical versus present occluders/access/configuration are explicitly supported.
- The cry reaches the two intended witness positions without identifying an unseen assailant.
- Warin's continued beam-side observation and cry uncertainty exclude the relevant assault journey without using an unavailable exact author timestamp.
- The private route is genuinely walkable by NPC and player, and the public-route claim survives permitted alternatives.
- Rack, desk and wallet searches have real access/interaction positions and recovery paths.
- Ordinary traffic, housing, work and enforcement remain functional after the redesign.
- Save compatibility is explicit and tested for geometry/content changes.
- Hidden-window drive evidence records all key sightlines and traversals; numerical reports identify actual source geometry and revisions.

## Handoff

Deliver final stable place/portal/fixture IDs, measured route/interval contracts, before/after district evidence, ordinary-city regression results and the authored visual layout. M15 writes history and witness records against this finished site, not the other way around.
