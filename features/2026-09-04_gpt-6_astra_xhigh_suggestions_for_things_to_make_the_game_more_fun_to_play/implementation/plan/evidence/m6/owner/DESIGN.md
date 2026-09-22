# M6 current-authority foundation

Status: bounded source slice; full M6 remains open.

## Reconciliation and scope

At accepted predecessor `d209ba2`, the shipped city gates are host schedule and
animation owners. There is no general authoritative portal, lock-pattern, key
loan, property-owner or scoped-grant registry. M4 supplies bounded street
geometry queries and M5 supplies a sampled-presence fixture API; neither is a
property permission. The separate `keys_and_locked_places.md` remains a proposal.

The concrete authority already implemented is custody and committed duty. A
voluntary `go_to` must yield to committed work, city departure and custody. A
custody release belongs to the officer of record, a current holder, or the
existing nearby law policy. This slice makes those checks a common closed API
and gives prospective consumers an immutable, bounded decision receipt and an
actor-qualified route wrapper. It does not create a duplicate door owner.

## Authority and denial

`access::{voluntary_travel,release_custody}` are used by direct action reducers.
They preserve existing duty precedence and ordinary denial wording. Denials are
typed and expose a static useful clue, including speaking to the holder, waiting
for committed work, or asking an actual keeper. Direct release preserves the
existing custody policy, including records with more than 32 holders; its
existing holder scan is not newly bounded here. Optional receipt capture refuses
those larger records before fingerprinting. IDs are checked
for validity and at most 512 UTF-8 bytes before lookup/copy. Refusal does not
mutate authority, intent, events or the public revision.

`Receipt::capture` retains two fixed inline IDs at most and a SHA-256 authority
fingerprint. The fingerprint binds world/spatial/event revisions, navigation
identity or absence, actor positions and presence epochs, current presence and
departure, law-role qualification, committed operation identities, and relevant
custody officer/holder/state values. At most two actors and two records of at
most 32 holders are inspected. No world-wide scan, record clone, text allocation,
grant store or unchecked deserialization is introduced.

Receipts also bind the caller-supplied runtime generation and exact logical
sample time. A later boundary or changed authority refuses fresh use even if a
hot mutation did not bump the public revision. Old decisions and their clues
remain unchanged. This is a prospective synchronous capability check, not an
archived command/event receipt or an issued legal warrant.

## Route and reducer agreement

Only an allowed voluntary-travel receipt can create `PermittedRoute`. Permission
is checked before `StreetQuery` can allocate route scratch. The route remains
private and each borrowed projection requires the same current authority and
exact closure snapshot/revision; M4's geometry ticket provides the physical
closure fence. A custody-release receipt cannot authorize travel.
The voluntary capability does not mint a known place handle or target knowledge;
`go_to` retains its existing known-place and visible-person validation. A route
projection does not itself move an actor or bypass those action checks. The
returned geometry can be retained or cloned; consequential consumers must
revalidate permission after any authority/world change.

Direct `go_to` and its voluntary route-pricing helper both call the same current
policy. Direct `release` re-evaluates custody authority; a stale UI receipt has
no mutation path. Existing custody/road duty movement remains separately owned:
`take_into_charge` and `route_budget_for` price their already-authorized geometry
without applying the voluntary duty gate. This preserves the existing escort
and legacy XZ-anchor behavior. Raw `StreetQuery` is still geometry-only and is
not an actor permission or a world movement writer.

## Remaining gates

This does not install general property grants, consent, locks, key patterns,
exact-key loans, inspections, forced entry, timed door operations, or their
versioned persistence. It does not change collision, visual barriers, hearing,
keeper schedules, existing host custody controls, or debug teleport privileges.
Those require the missing shared portal/ownership/operation and persistence
adoption. No full M6, spatial, renderer, provider or frame acceptance is claimed.

Focused tests exercise action/query denial agreement, release revocation without
a revision bump, operation-specific scope, exact boundary/geometry staleness,
holder/ID capacity refusal, and committed-operation authority. Existing custody
integration tests verify that escort/release behavior remains compatible.
