Status: M3b2b1 accepted after independent source, boundary, command and layout review (2026-09-15).

# M3b2b1 coordinator review

Accepted predecessor: M3b2a commit
`30f8188301b8f1585df478e2e6df5f16bc1434cc`, source map
`04046178497e975d6942f76232ef3696fd5a4ad7674494ffc817e79d5b978032`
(1,003 inputs). Its final workspace passed 2,280 tests with zero failures and
47 ignored. The current cut is a production archive prerequisite, not complete
application allocation or adoption acceptance.

## Agreed design boundary

Archive capacity must be admitted before the simulation accepts cognition work
that will owe an archive. The opaque permit travels with the actual
immutable exchange, so message clones share payload and admission. A held result
retains its capacity after provider completion. The queued job and active writer
retain capacity through actual rendering, IO and disposal. A standalone record
refusal returns caller ownership without advancing the filename sequence.

Every production archive caller and forwarding adapter uses the same admission
path. Unsupported metadata and backing capacities are checked before accepting
provider work; shared ingress also verifies the actual record capacities against
its receipt. The reviewed bound and scoped native assumptions are recorded in
the owner's implementation document.

M2c preserves foreground and night held results without resubmitting them.
Their new service binding therefore needs transactional archive admission.
The permit must not depend on importing saved RequestId values into a new
backend counter: those numbers may be reused by new requests. Saved semantic
roots and retry obligations remain unchanged.

## Independent review ownership

Root owns `crates/cathedral-backends/src/prompt_archive_review_tests.rs` with
six independent witnesses, all passing on the final workspace source:

- Byte saturation returns the original String allocation and does not spend a
  filename suffix; retry writes the expected next record.
- An admitted shared-event clone retains capacity after the worker finishes;
  duplicate publication returns the same Arc without writing another record.
- Forked session handles share same-second filename order, while a foreign
  session returns the unchanged Arc.
- A real admitted writer keeps its persistent charge after outer handles end
  while an actual permit survives, and releases after its final worker/permit.
- A short ActorId backed by a 1 MiB String is refused intact when unadmitted
  and cannot bypass an existing receipt's capacity validation.
- Direct shared ingress refuses an unadmitted Arc; legacy convenience ingress
  creates a separately admitted copy while preserving the original caller graph.

The fifth test followed source review: ActorId already exposes allocated_bytes,
but the initial shared-record path counted only its visible string length.
The implementation owner corrected owned ActorId capacity accounting.
The owner additionally tests actual scheduler/night acceptance and transactional
held-result binding. focused-sim-01 passed all six owner witnesses before the
final provider-attempt counter refinements. focused-backend-01 passed all six
independent witnesses plus the existing archive compatibility coverage: 20
passed, zero failed/ignored, with source map
`5ab3cf5b7f1b7c02976df03b9ff7a07473ccc024abad21b891c4569c179fc550`
unchanged during execution. The refreshed continuation filter subsequently
passed 37 tests with one existing ignored
test, including all six owner witnesses with provider-attempt assertions.

The final exact-capacity ward-ID refinement produces freeze
`8f246541e779e999382526d0a730eb95accb60965b1ff37df4b175f54c2ba0ca`.
The source audit matches 1,005 inputs: two new Rust files, 29 changed predecessor
inputs and 974 unchanged, with no removed inputs. Full-workspace acceptance
uses this map; the earlier focused maps remain earlier verification records.

Review required an opaque immutable payload with read-only access; public
String fields would allow admission to be detached or bypassed. It also raised
the lifetime of successful fallback admission for originally unadmitted Arc
records: a writer-only extra permit cannot protect surviving external clones.
The selected contract refuses unadmitted direct shared ingress, and reserves
before copying legacy convenience input. Disabled default log construction is
lazy and does not initialize the global worker. The independent tests now cover
shared/convenience refusal, unchanged input ownership and surviving event copies.

The reviewed destructor structure declares charged job strings and session
references before the final exchange, and receipt session metadata before its
payload charge. The worker retains a separate fixed progress control after
dropping the job, then signals flush. It does not retain directory/model strings
merely to notify completion.

ArchiveWriter drops its sender before its shared off-frame join owner. The last
join owner retains Core until the native thread has actually terminated.
Receipts retain only Core, so releasing a final permit cannot join or wait for
IO. This closes the earlier interval between worker-closure destruction and
actual native stack teardown. The real admitted-writer test verifies the
surviving-permit case; source review supplies the destruction-order argument.

Idle caller-supplied directory/model configuration remains service ownership
outside the outstanding-job governor. The later immutable startup recipe must
admit it. start_admitted is an actual tested shared-budget integration seam;
default production startup has a finite writer but does not yet use that seam.
This cut cannot establish whole-host or complete persistent-service accounting.

The implementation owner ceded source/Cargo/executable ownership after all
checks completed, with no command left running. Root owns this review,
independent tests, evidence audits, top-level status documents and the commit.

## Final acceptance

The frozen offline workspace passed 2,292 tests, with zero failures and 47
ignored, across 46 printed outer groups. Its named fresh-process preparation
test passed; no separately printed child summary is added to the total.
All 12 new archive witnesses and 14 predecessor boundaries pass the independent
boundary audit. Compilation took 22m53s; total recorded time was 1,481.283s.

All 19 command records pass raw/gzip/source/environment/helper audits. Earlier
compiler failures and source-changing development runs remain accurately
recorded. Final formatting, whitespace, workspace and layout checks share the
accepted map. Read-only inspection verified 28 exact DWARF queries and both
final test ELF hashes. Preparation fixed control is 60,952 bytes, below its
65,536-byte allowance. The ELFs were hashed in place; separate copies are not
preserved. Layout arithmetic does not measure a whole native/process heap.

The ordinary application now has a cosmetic unused standalone PromptExchange
import, needed only by retirement tests. Moving it into the test module is
explicitly deferred to the next substantive host edit; no assertion was
weakened and no Result-return warning is ignored in archive tests. Existing
performance/future-integration warnings remain distinct from this import.

## Explicit successors

Native runtime/detached STT-discard/reaper lifetimes, immutable installed assets,
diagnostic sinks and the actual disjoint live/candidate/retired/persistent
capacity proof remain later M3b2b work. In particular, session_log.rs has an
unbounded stderr queue and a JSONL buffer that can synchronously flush after
its high-water mark; closing PromptLog alone does not close those owners.
M3b2c complete host adoption, M3c controls, M3d frame acceptance and M4–M19 remain
pending. No shared cap or Running minimum is lowered by this cut.
