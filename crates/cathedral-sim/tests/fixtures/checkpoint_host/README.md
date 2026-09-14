# Host component V1 fixtures

These are exact canonical host component payloads captured from the actual
renderer-free `CityPlugin` fixture in `src/host_checkpoint/tests.rs`, with the
authored cast, installed collision/gate/CutMargin/vermin definitions, fake
cognition, disabled microphone, no renderer/audio plugin and manual 17 ms frames.
They are components, not complete saves or whole-world admission evidence.

| Payload | Boundary | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `initial-v1.json` | First requested eligible startup continuation, 34 ms, generation 1, 10 rows | 3553 | `dca6128035a23ba7a88427f1b005e991cd3397bad609a26f9cb78470478595ce` |
| `active-v1.json` | Ordinary readable workload, 68 ms, generation 2, 22 rows | 6279 | `e01b1aeb7f775b5023cd8c76006ea67eaee814b9bc13d040f15120124bdef32a` |

Initial means the first captured supported host boundary after startup has
published its clock and accepted physical state. It is not a zero-time bare
Engine. Active retains an NPC subtitle and separate bubble, the original player
caption receipt, four still-unread committed speech messages, a submitted but
unforwarded chat intent and pending UI command, plus sound cooldown/well state.

The ordinary test
`persisted_initial_and_active_host_fixtures_decode_at_actual_compatible_boundaries`
reads these files, reserves fixture byte storage before allocation, and checks
decode, canonical roundtrip and candidate bytes at the actual `HostCaptureSet`.
Its newly prepared runtime has another process-global generation. Only a detached
copy used for comparing the fresh export has that one fence aligned; saved bytes,
decoded candidate and live owners keep their original identities. All scalar and
record comparisons use canonical bytes, including signed zero. Extra output
buffers retain explicit Save admission. The fixture storage Running charge does
not claim admission of the live World.

The ignored `m2a15_write_component_fixtures` test writes initial then active to a
fresh `ALIBI_HOST_OUTPUT` directory using `create_new`. It never overwrites these
files. Three independent writer processes produced identical bytes on
2026-09-14; the second and third used the final test source, and the third repeated
the required environment after a harmless recorded PATH typo in the second.
Command identities, originals and deterministic gzip archives are recorded in
M2a15 evidence `owner_fixtures/fixture-equality-1.json`.

The current pair was explicitly refreshed during M2a16 review on 2026-09-15.
The strict installed-catalog fingerprint includes all of `src/city/mod.rs`,
which gained a test-only memory observer. Three fresh writers from the
preserved M2a16 debug executable agreed exactly. The only changed JSON path
is `/scalars/definitions/installed_catalogs`; all gameplay state is identical.
The original M2a15 bytes, current bytes, exact command provenance and
independent review are preserved in M2a16 evidence under
`host-component-fixture-refresh/` and
`coordinator/host-fixture-refresh-audit.json`. The complete workspace rerun
validates this current pair. The M2a15 record above remains historical.

Preserve these bytes. A deliberately changed compatibility contract needs a new
supported fixture and explicit review, not silent regeneration of this pair.
