Status: Planned (2026-09-05).

# M7 — Objects that can be investigated

Let an object exist somewhere in the city, be handled under ordinary rules, and retain enough identity/history to support an investigation. The player should manipulate papers and objects, not collect invisible evidence flags.

## Entry and starting point

M3–M6 are accepted. Existing `World.items`, `Character.holds`, inventory reservations, transforms, pockets and transfers are the foundations. Current items are stacks with catalog metadata; the renderer's hand props do not establish a general placed-object/container system.

## Implementation

### M7a — One location and one identity

Introduce an authoritative location service for actor-held, placed and contained objects. Migrate or derive existing hold indexes through it; do not maintain two independently mutable ownership tables. Preserve ordered held-item presentation and the existing stock/quantity invariants.

A physical object has exactly one location. A reservation restricts use; it does not create a second copy in an activity. Containers form an acyclic bounded hierarchy with accessible openings and capacity rules. Inspecting an exterior does not reveal its sealed contents.

Distinguish fungible stacks from individually tracked objects. Papers, paired fragments, keys and sealed packets must not merge merely because their display metadata matches. Any operation that changes an identified object's representation must preserve identity or emit an explicit lineage transition that references the physical original.

Audit partial transfer, restamping, pocketing, swallowing, expelling and transformation paths. A document becoming wet is a condition change, not permission to lose its evidential identity. Do not grant essential objects global invulnerability: model the relevant alteration and retain actual prior examinations without inventing a replacement original.

Extend M5's authoritative observable-presentation descriptor for placed/contained objects and multi-step handover: exposure/concealment, support anchor, visible outline/features, object revision and transfer phase. Host hands currently render selected props and animate after ledger commit; align that projection with what observation is allowed to claim. Private location/identity remains distinct from a viewer's supported object track.

### M7b — Readable documents and physical inspection

Author document content as structured immutable/versioned text and fields bound to an object. Provide readable excerpts, pagination/zoom and captions through the ordinary non-pausing UI. The displayed words must be actual authored evidence; tiny textures and handwriting recognition are not the only path.

Define inspectable features and examination methods: read a wrapper, compare an inventory list, align two fractured parts, check a seal, inspect a container and show an object to an authorised examiner. The method validates physical availability, consent, necessary tools/participants and time.

Implement examination work through M1's single-operation kernel and M6's capability registry. M8 later composes it into multi-step activities/meetings. Ship versioned object/document/examination declarations and validation here; no temporary private quest interpreter is needed.

An examination result is a signed/attributed domain receipt referencing the actual objects, revisions, examiner, method, time and supported conclusion. It cannot certify a match by searching the hidden case solution. A similarity result is weaker than a unique physical fit or matching recorded identifier.

Supply inspectable mechanical fixture declarations where a later timed experiment needs them: stroke endpoints, rate/range, load class and interruption state. M8 composes operation and observation into a demonstration. A decorative beam animation cannot serve as the causal source of a historical duration.

### M7c — Transfers, copies and custody

Use the ordinary inventory transfer service for taking, handing over, seizing, returning and lodging objects. Attach custody receipts with participants, source/destination, time and scope. A formal custody log records an institution's handling; an unwitnessed player theft does not become publicly known merely because the private sim ledger knows it happened.

Copies have their own identity and a provenance link to a specific source and copying/authentication act. Losing the original does not erase an earlier valid examination, but a copy does not automatically meet a rule requiring the original.

Distinguish “last location the player knows” from actual current location. UI location records update only through a valid observation, handover or report. Hidden NPC retrieval never silently updates the player's map.

## Development content

Use a workshop record and two similar-looking tools. One has a uniquely fitting broken part; the other merely shares its type. Move the record between shelf, actor, container and keeper. Create an authenticated copy and an unauthenticated imitation. None of the test objects is the final quest packet.

## Tests

- Quantity/identity conservation through holds, placements, containers, offers and reservations.
- Cyclic containers, duplicate locations and unreachable contents are rejected before mutation.
- Two visually identical documents remain distinct; a generic bundle is not sufficient proof of identity.
- Restamping/pockets/gut/transforms preserve or explicitly document physical lineage.
- Wrong/missing part, changed object revision, absent examiner and expired inspection grant prevent a false result.
- A witnessed handover and an unwitnessed taking produce different public knowledge while both conserve inventory.
- A later copy authenticates only against the exact source/result it references.
- Save/load during transfer preparation, examination and containment preserves location, progress and receipts once.

## Completion gate

Independent objects can be found, read, moved, compared and recovered without case-specific code. Existing commerce and bodily inventory operations still obey their invariants. M9 receives typed examination/custody receipts, and M10 receives readable object projections with no hidden-state leaks.
