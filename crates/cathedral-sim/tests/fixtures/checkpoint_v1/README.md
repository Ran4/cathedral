# V1 private component fixtures

These pin the real M2a1 owner wire format: an accepted timed operation and its
ledger, an exact manifest component, the clock segment, and accepted host time
with 400 ms ordinary debt / 15 ms fixed residual / 25 ms simulation residual.
They are **not complete supported saves** and must not be used as M3 slot files.
World/characters, Round/residents, knowledge, law and pending external-work
owners must join the complete M2a envelope before full save fixtures exist.

The normal `supported_component_fixtures` test validates and byte-compares these
with production-owner exports. Intentional changes can regenerate them through
`cargo test -p cathedral-sim --lib regenerate_checkpoint_component_fixtures -- --ignored`.
Existing prompt/snapshot golden fixtures are unrelated and unchanged.

M2a2 adds three strict **WorldBackbone component** fixtures: `backbone.json`
(private generated identity/state, live inventory/reservations, offers, restock,
movement/gut/legacy intent/edit, ordered registry with duplicate display names),
`backbone_completed.json` (historical completion lineage after every live item
has been consumed), and `backbone_empty.json` (virgin empty backbone). All fields
are mandatory, including explicit null authority. These cannot be loaded as a
city; the complete owner/manifest envelope and Engine hydration remain pending.
The explicit ignored `world::checkpoint::tests::regenerate_backbone_fixture`
generator is separate from the M2a1 component fixtures.

M2a5 adds `knowledge.json` and `engine_knowledge.json`. They preserve the private store (including all sealed provenance variants, historical receipt/seated obligations and a garbled view) and the Engine's pollen deadlines, journal/ward-heat caches and expired door timer. The source-bearing bytes belong only to persistence; runtime Debug/projections and malformed-source errors remain redacted. These are component fixtures, not a complete save or public partial adoption path.
