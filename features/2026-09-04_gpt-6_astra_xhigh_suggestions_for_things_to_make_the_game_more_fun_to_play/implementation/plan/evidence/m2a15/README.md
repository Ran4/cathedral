# M2a15 existing host continuation

Status: Accepted component cut (2026-09-14). [Coordinator review](coordinator/review.md)
records final workspace/source/fixture/admission verification and successful
release smoke and timing audits. [Release measurements](performance/README.md)
retain 3,600 phase samples and the explicit pending host-frame integration limit.

This leg implements a strict read-only component of the actual host at the
ordinary completed input/physical boundary. It does not implement complete
save/load, replacement Engine/World hydration, retry/interruption adoption,
filesystem publication or retiring-world admission.

- [Implemented owner inventory and boundary](OWNER_DESIGN.md)
- [Actual next-consumer coverage](OWNER_COVERAGE.md)
- [Allocation, limits and lifetime accounting](ADMISSION.md)
- `commands/`: exact command-start source/environment/helper identity, result
  metadata and deterministic gzip logs, including failed development runs.
- `owner_probes/`: exact raw JSON archives for development fixture smoke checks.
- `owner_fixtures/`: preserved canonical initial/active payload archives and
  independent-process equality/provenance in `fixture-equality-1.json`.
- `coordinator/`: independently owned review/release runner, auditor and results.

## Callable surface

The pure API is `cathedral_sim::checkpoint::host`:

```rust
checkpoint_host_cost(&impl HostCheckpointSource) -> Result<ComponentCost>
export_host_checkpoint(&impl HostCheckpointSource, HostCheckpointContext,
                       Reservation) -> Result<Admitted<HostDtoV1>>
HostDtoV1::decode(&[u8], HostCheckpointContext,
                  Reservation) -> Result<Admitted<HostDtoV1>>
Admitted<HostDtoV1>::encode(self) -> Result<Admitted<Vec<u8>>>
Admitted<HostDtoV1>::into_candidate(self, HostCheckpointContext)
    -> Result<Admitted<HostCandidate>>
```

`HostCheckpointSource` supplies Copy `ScalarsV1`, visits borrowed closed
`RecordRef` (`RecordV1<&str>`) rows and validates the actual source boundary.
`HostDtoV1` has no public Deserialize or unchecked admission constructor.
Read-only DTO/candidate getters expose scalars and typed rows. Context can borrow
actual Engine authority or coherent admitted saved backbone/law/knowledge/marks/
climate/animals/ledger components. The latter path constructs no Engine/World.

The executable adapter is `src/host_checkpoint.rs`:
`HostObservation::new(&World) -> Result<HostObservation>` and
`source.context() -> Result<HostCheckpointContext>`. `HostCaptureSet` runs after
ordinary DrainBridge and before ReconcileMirror/CollectInput. The coordinator
owns `src/host_checkpoint/tests_public.rs`; prepared restoration helpers are
private owner tests only.

## Actual city cost probe contract

Run the ignored `host_checkpoint::tests::m2a15_cost_probe` in the cathedralbevy
test binary, single threaded. Set `ALIBI_HOST_MODE=authored|populated`,
`ALIBI_HOST_SAMPLES=1..1000` and an absolute fresh `ALIBI_HOST_OUTPUT` path.
The modes use actual CityPlugin startup geometry with authored citizens or
2,000 additions, MinimalPlugins and no renderer/window/audio device/provider.
Startup assets and ECS render entities exist for honest scanning cost; no
RenderApp, AudioPlugin or visible window is created.

Ordinary debug speech first produces a real NPC subtitle/bubble. Four player
lines remain committed but unread at capture, while a real PreUpdate chat submit
is intentional pending work. Ordinary market/well cues establish cooldown and
well gates. The fixture reports actual placed/requested/unplaced citizens,
entity/character/collider/prism/gate/CutMargin/vermin counts and each typed row
family. Every repetition must encode identical bytes and keep exact revision,
event sequence and input watermark unchanged.

The JSON schema is `schema:1`, `scenario:"actual-host-boundary-v1"`. It includes
`mode`, `scope`, `samples`, `counts`, `placement`, `readable_counts`, `cost`,
`shared_reserved_peak_excluding_running_bytes`, `readonly`, and six fractional
microsecond arrays: `preflight_us`, `export_us`, `encode_us`,
`decode_validate_us`, `candidate_validate_us`, `drop_us`. The readonly object has
before/after keys for `world_revision`, `event_sequence` and `input_watermark`.
Cost includes `encoded_bytes`, `expanded_upper_bytes`, `peak_bytes`,
`validation_working_bytes` and `container_stride_bytes`.

Preflight/export include actual observation and definition hashing. Decode and
candidate phases include their prepared-context hashing/validation. Drop covers
both admitted owners with data-before-lease destruction. Diagnostic byte copies,
row-count JSON inspection, fixture setup and report writing occur outside timed
phases and are excluded from component reservation totals. Coordinator process
RSS includes such fixture overhead and is separately reported.

## Development findings retained in the evidence

Initial compiler failures and owner fixture errors are retained, not normalized
away. The independent actual chat test found a real ordinary ordering bug:
PreUpdate stamped a UI action with spatial sequence S, the pump's final physical
sample advanced past S, and CollectInput forwarded the action afterward. Fresh
identity at ordinary forwarding now preserves the original frozen action pose
without relaxing sim stale-sequence rejection or restamping accepted commands.

`host-tests-5` passed all eleven coordinator tests and thirteen owner/existing
host tests, with one owner fixture failure and one ignored probe. The failed
fixture incorrectly required over 1,000 boxes; actual city geometry is 751 boxes
plus 1,153 convex prisms. Successful capture preceded that assertion. The fixed
fixture requires both actual families and binds their complete definitions.

Authored development smoke 1 passed two meaningful samples with 520 characters,
18,496 entities, two barriers, installed CutMargin, eight colonies/150 rats and
22 typed rows. It encoded 6,287 bytes with a 5,081,285-byte component reservation
and a 10,162,570-byte simultaneous Save plus Load charge, excluding Running.
These test-profile smoke timings are not frozen-source release acceptance.

The final workspace/source freeze is recorded below; independent release
acceptance is owned by the coordinator. No complete-save or frame-budget pass
is inferred: the previous naive backbone plus Round Save+Load peak already
exceeds 1 GiB before Running, and actual full cohort lifetimes remain pending.

Populated development smoke 1 passed two samples with all 2,000 additions
placed: 2,520 characters and 73,984 actual ECS entities. Physical/vermin and
readable row counts matched the authored fixture's shape. It encoded 6,275 bytes
with a 5,081,249-byte component charge and 10,162,498-byte simultaneous Save plus
Load charge, excluding Running. Test-profile preflight was 7.78–7.95 ms and export
18.49–18.65 ms; these are reported costs, not a frame-budget pass.

`workspace-final-1` retained an unchanged source snapshot and completed 2,132
passing tests, two failures and 37 ignored before its early exit. Both failures
were fixture assumptions: the refused-gap test jumped beyond the host enqueue
window, and an existing old-generation speech test compared newly explicit
reader progress as though it were presentation mutation. The corrected gap test
uses an allowed identified Hello followed by an action the sim's separate window
refuses. The speech test separately verifies reader progress and unchanged
presentation/acknowledgement, then checks actual current-generation words.

The new receipt-storage fallback preserves ordinary display/audio and never
acknowledges early. Targeted verification passed the gap, generation and direct
receipt-limit cases. The actual host refusal/expiry case first failed only on its
expected diagnostic prefix, then passed in `fallback-targeted-2`. Every original
failure and source-change status remains in `commands/`.

## Final owner verification and Cargo handoff

`workspace-final-3` completed successfully on 2026-09-14 with unchanged source:
**2,157 passed, 0 failed, 38 ignored across 46 groups**. The host unit target
passed 569 tests (five ignored); the sim unit target passed 799 (28 ignored).
All eleven independent public host tests passed. The full command includes
workspace integrations and doc tests, not just the host test filter.

[source_hashes.json](source_hashes.json) is the exact 950-file
`component-inputs-v2` map from that command's START metadata. It was compared
with current source after successful exit. [owner_final_check.json](owner_final_check.json)
records that comparison and the raw test totals. Cargo ownership was explicitly
ceded immediately after this verification, before documentation finalization.
The coordinator owns release build/run/audit and commit; the owner did not
commit, push, create a branch/worktree or run a visible app/device/provider.

The preceding `workspace-final-2` remains successful pre-fixture evidence at
2,156/0/37. Its 947-file map and final-check record are preserved verbatim under
`coordinator/before-fixtures-*.json`, alongside the first release build's retained
binary/source evidence. The only subsequent source delta is the owner fixture
test and three new files under `crates/cathedral-sim/tests/fixtures/checkpoint_host`.

## Persisted supported host fixtures

The initial payload is 3,549 bytes at the supported 34 ms startup boundary with
10 rows; the active payload is 6,275 bytes at 68 ms with 22 rows, including four
unread committed speech messages and one unread intentional chat submission.
Both come from actual renderer-free CityPlugin geometry and authored citizens.
Three fresh `create_new` writer processes produced identical exact bytes. Writer
2's recorded PATH typo was superseded by writer 3 with the required environment;
all originals and command identities are preserved. `fixture-equality-1` creates
the new files, so its source-changed result is expected and not a frozen check.

`fixture-loader-1` and the full workspace verify decoded canonical roundtrip,
unchanged candidate bytes and the matching actual host export at HostCaptureSet.
Only a detached copy of the fresh export's generation fence is aligned with the
stored generation. Original payloads, candidate and live owners are unchanged.
The fixture byte storage and serialization buffers retain explicit admission;
the test's Running charge covers fixture storage, not the live World. Exact
payload hashes, original paths and mtime-zero gzip hashes are in
[fixture-equality-1.json](owner_fixtures/fixture-equality-1.json).

Historical fixture bytes and unrelated untracked user work were preserved.
Protocol, runtime budgets and persistence inventory carry the M2a15 owner delta.
[HANDOFF.md](HANDOFF.md) records the source families and remaining gates.
