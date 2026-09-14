# Host component V1 fixtures

These are exact canonical host component payloads captured from the actual
renderer-free `CityPlugin` fixture in `src/host_checkpoint/tests.rs`, with the
authored cast, installed collision/gate/CutMargin/vermin definitions, fake
cognition, disabled microphone, no renderer/audio plugin and manual 17 ms frames.
They are components, not complete saves or whole-world admission evidence.

| Payload | Boundary | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `initial-v1.json` | First requested eligible startup continuation, 34 ms, generation 1, 10 rows | 3549 | `09d2d2bc5b45df80603b6a44daff7851ead3241945617918acefd258b8a86060` |
| `active-v1.json` | Ordinary readable workload, 68 ms, generation 2, 22 rows | 6275 | `c0b20da7dfbf0e7b0e1085b95987193f0a3197c5588b43b6cde6929d85b9f761` |

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

Preserve these bytes. A deliberately changed compatibility contract needs a new
supported fixture and explicit review, not silent regeneration of this pair.
