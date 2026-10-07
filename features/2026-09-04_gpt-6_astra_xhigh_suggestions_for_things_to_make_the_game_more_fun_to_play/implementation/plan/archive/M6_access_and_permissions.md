Status: Planned (2026-09-05).

# M6 — Doors, authority and permission

Make access a relationship between a person, a physical place, an action and current authority. A key can open a lock without making the entry lawful; consent can permit entry without transferring a key.

## Entry

M4/M5 are accepted. Read `features/keys_and_locked_places.md` as an existing proposal and reconcile its vocabulary with shipped items, knowledge and spatial interfaces. Do not mark that whole separate feature implemented merely because its required subset ships here.

## Authoritative model

Extend M4's stable portals with access points, sides, lock/bar state, key pattern, keepers and allowed operations. M4 remains the owner of physical opening/obstruction state; M6's validated operations write it. Mesh animation, player collision, NPC route availability, sight and sound project the same revision.

Define scoped grants: grantor, recipient, place/access point, permitted act, purpose, interval and revocation conditions. Inspection permission does not automatically permit taking property or searching an unrelated desk. An arrest order's entry/search authority is separate from a resident's consent.

Introduce the common ownership/capability registry from M0's audited role map here. M7 adds examiner roles, M9 reviewer roles, and M11 legal issue/recall/disposition capabilities. Early grants use real owner/delegated authority; no unchecked “has warrant” boolean stands in for M11.

Use existing item identities for keys, including pattern matching and exact-item loans. A key loan has a real lender, due time and return receipt. Pattern copies may share mechanical capability while retaining different identities/custody. Implement required pattern semantics now; a broad locksmith economy and a city-wide catalogue of authored locks can remain the separate feature's content work.

Make keys non-stackable quantity-one objects now and preserve their exact ID through every allowed handling path, including gut containment. M7 generalises the identified-object/location service; M6 cannot defer its already-required exact-key guarantee until then.

## Operations

Support open/close, lock/unlock, request opening, grant/revoke consent, enter/leave, lend/return and scoped inspection. A door opening consumes real elapsed time and can be obstructed or interrupted. Do not animate it open before authoritative validation.

These are adapters to M1's single-operation/resource/duty kernel. Do not build a second activity scheduler for doors while waiting for M8's later composition layer. Ship versioned access declarations and load validation with the runtime.

Validate key accessibility against held/pocketed/reserved units. A borrowed key that has been returned stops working for the borrower immediately. Loan expiry creates an obligation and information for its actual participants; it does not teleport the key home.

Integrate keeper rounds and ordinary absence. A closed workplace remains physically closed until a capable actor opens it or a valid alternative is used. Provide egress and recovery routes so a player cannot be accidentally sealed into a room by an NPC's bedtime.

Add a bounded forced-entry operation usable by M12 when an order explicitly permits it. It takes time, creates a physical damaged/open state and emits local observations. It is not a magic unlock or a general combat/destruction simulator.

## Route and concurrency rules

- Planned access is revalidated at the threshold; a route is not a permanent opening guarantee.
- A revoked grant changes permission immediately, while physically closing a door follows its actual operation.
- An occupied threshold prevents clipping a body with a closing leaf; wait, refuse or stop at obstruction visibly.
- Inside release and emergency egress use authored mechanical rules, not hidden quest exemptions.
- Two simultaneous opening requests share/compete for the same door operation rather than producing contradictory states.
- Authority to search is limited by place/object scope and is rechecked before each consequential entry or taking.

## Development content

Use three independent access situations: a public door operated on a keeper's schedule, a loaned-key route and a consent-only supervised inspection. Test forced entry initially with an actual owner-authorised scope. Order-authorised entry remains an explicit M11/M12 integration gate, not a claimed M6 capability before legal orders exist. At least one place has two legitimate acquisition/access routes.

## Tests

- Missing/wrong/pocketed/reserved key failures leave state intact.
- Possession and legal permission can vary independently.
- Transfer/loan return/revocation affect the next attempted action without stale UI authority.
- Closed/open state agrees in nav, stepping, collision, sight and hearing.
- A keeper leaves, is held, or cannot reach a door; clients get a real wait/alternative/failure.
- Forced entry requires explicit scope and produces damage plus real local witnesses.
- Unwitnessed trespass creates no psychic owner/guard knowledge; later discovery remains possible.
- Save/load during opening, obstructed closure, an overdue loan and active consent preserves every relevant state.

## Completion gate

Access changes real journeys and inspections in the development slice. Ownership, mechanical capability and permission remain distinct. New grants, loans and door operations are covered by persistence and command receipts. M7 can inspect behind a real threshold, and M12 can enforce an order without bypassing it.
