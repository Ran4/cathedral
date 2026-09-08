from pathlib import Path
import json, hashlib

root = Path('/home/ran/src/rust/cathedralbevy')
feature = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play'
plan = feature/'implementation/plan'
e = plan/'evidence/m2a10'
load = lambda p: json.loads(p.read_text())
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
for name in ['source_audit','log_archive_audit','stdlib_allocation_audit','release_archive_audit','social_smoke_audit','social_performance_audit','auditor_regressions']:
    assert load(e/f'coordinator/{name}.json')['result'] == 'passed'
for name in ['round','climate','knowledge','law','marks','animals','night']:
    assert load(e/f'coordinator/historical_{name}_audit.json')['result'] == 'passed'
logs = load(e/'coordinator/log_archive_audit.json')
verification = load(e/'verification.json')
workspace_name = verification['workspace']['command_name']
counts = logs['totals'][workspace_name]
assert counts['passed'] == 2045 and counts['failed'] == 0 and counts['ignored'] == 29
assert logs['totals']['checkpoint_final']['passed'] == 133
assert logs['totals']['public_final']['passed'] == 6
workspace = next(r for r in load(e/'commands.json') if r['name'] == workspace_name)
summary = load(e/'performance/SUMMARY.json')
identity = load(e/'performance/IDENTITY.json')
audit = load(e/'coordinator/social_performance_audit.json')
assert audit['raw_phase_samples'] == 3600 and audit['current_source_binary_runner_hashes_checked']
assert load(e/'coordinator/social_smoke_audit.json')['raw_phase_samples'] == 24
p99 = {mode:summary[mode]['median_run_us']['export_us']['p99']/1000 for mode in ['authored','populated']}
largest = max(audit['global_max_us'][mode]['export_us'] for mode in p99)/1000
labels = {'preflight_us':'Preflight','export_us':'Export','encode_us':'Encode','decode_validate_us':'Decode + validate','candidate_validate_us':'Candidate validation','drop_us':'Drop'}
table = '\n'.join('| '+label+' | '+' | '.join(f"{summary[mode]['median_run_us'][phase][q]/1000:.6f}" for mode in p99 for q in ['p50','p99'])+' |' for phase,label in labels.items())
costs = {mode:summary[mode]['metadata']['cost'] for mode in p99}
assert costs == {
    'authored':{'encoded_bytes':695,'expanded_upper_bytes':14296,'validation_working_bytes':65536,'peak_bytes':128901},
    'populated':{'encoded_bytes':937,'expanded_upper_bytes':18632,'validation_working_bytes':65536,'peak_bytes':146971},
}
performance = f'''# M2a10 release measurements — 2026-09-08

The [runner](../../run_m2_social_probes.py) executes three 100-sample runs per mode in alternating order on one frozen release executable. All **3,600 phase samples** remain in lossless JSON gzip archives. [Results](RESULTS.json), [summary](SUMMARY.json) and [identity](IDENTITY.json) retain commands, exact counters and witnesses, source/binary/runner hashes and process timings. The [independent audit](../coordinator/social_performance_audit.json) recomputes every percentile and summary, validates all semantic and admission gates, and checks current source/binary/runner identity. The separate [smoke audit](../coordinator/social_smoke_audit.json) covers 24 phase samples.

Each value is milliseconds, the median of three per-run percentiles, not a pooled percentile. Each mode has 300 samples per phase.

| Phase | Authored p50 | Authored p99 | Populated p50 | Populated p99 |
|---|---:|---:|---:|---:|
{table}

The largest individual export is {largest:.6f} ms. These are social component API costs, excluding complete Engine capture/hydration, host adoption, disk, rendering and maximum supported collection occupancy. Earlier components still need offload or incremental host coordination.

The authored world has 520 characters; all requested 2,000 production placements succeed in the 2,520-character populated world. The probe selects real settled bodies from each loaded population. Ordinary named player speech submits one prompt; a scripted successful reply addresses the player and a nearby peer, and a later gaze sample leaves partial focus at the 0.3-second boundary. Both modes retain reciprocal engagement, an independent invitation, focus, one warm NPC pair and one submitted Novelty context. Authored/populated state retains three/six prior witnesses and two/six Novelty memories. The populated peer is a generated resident. Exact utterances, all three emitted speech events and recipient lists, positions, selected actor and the 19,849/19,029-byte submitted prompt lengths repeat. Canonical witness digests in the runner/auditor pin the complete reviewed functional-smoke records.

| Admission quantity | Authored bytes | Populated bytes |
|---|---:|---:|
| Encoded J | 695 | 937 |
| Expanded upper E | 14,296 | 18,632 |
| Fixed validation working | 65,536 | 65,536 |
| One cohort peak | 128,901 | 146,971 |
| Save + Load, excluding Running | 257,802 | 293,942 |

Charges are conservative admission bounds, not allocator measurements. The [admission proof](../ADMISSION.md) covers private layouts and malformed aggregate-bounded shapes separately. The prior naive backbone+Round Save+Load sum of 1,256,093,444 B still exceeds the unchanged 1 GiB shared cap before Running and other owners; full integration must resolve phase lifetimes.

The [release build](../coordinator/release_build.json) retains exact original stdout/stderr and timing bytes in gzip archives. Executable SHA-256: `{identity['binary_sha256']}`. The shared [input enumerator](../../component_input_sources.py) unifies prior owner and measurement scopes and includes `default_config.ron`; the same 895-path map is captured at command start, checked before/after release build and measurement, and independently verified in the [release archive audit](../coordinator/release_archive_audit.json). Historical records retain their original source scopes.

All seven historical component datasets pass the amended auditor. [Four corrupted-data checks](../coordinator/auditor_regressions.json) fail at the intended gates: altered percentile, reduced working charge, loss of the retained partner, and omission of runtime defaults from the source identity. Updating raw hashes and metadata cannot conceal the semantic or charge changes.
'''
assert not (e/'performance/README.md').exists()
(e/'performance/README.md').write_text(performance)
failure_names = ', '.join(logs['development_failures'])
review = f'''Status: Accepted existing component scope — 2026-09-08. Complete M2/M3 remain pending.

# M2a10 coordinator review

Conversation, WarmExchanges and Novelty now have opaque admitted checkpoints for every existing private field. Engine composition also retains original idle configuration and binds the player to a borrowed World or unadopted backbone. Decode preserves historical/expired references and exact logical anchors without expiry, context rehash, constructors, scheduler reconstruction or production adoption. Stage radius and curiosity scale retain raw IEEE bits; max_actors retains zero and usize::MAX.

All six independent public tests pass. They continue partial gaze dwell and its original expiry, distinguish reciprocal partners from independent invitations and newcomers from prior witnesses, preserve historical unordered exchange pairs until ordinary pruning, and retain opaque Novelty visit/context values through a late inbox arrival and exact expiry. Backwards focus history and signed-zero boundaries remain distinct. Malformed fields, wrong player binding and duplicate pairs reject cleanly; one MiB of raw padding keeps at least three MiB of additional reservation through candidate validation.

Fourteen private/fixture tests pass. They deliberately scramble covered owners/configuration, require immediate canonical equality before continuation, and compare ordinary Engine messages and later social state. Same-time newer group speech fences an older captured utterance; u64::MAX tokens saturate correctly. Tests cover all four new fixtures, sparse/full maps, 128 witnesses, 25,000 independent warm pairs and 25,000 Novelty rows, malformed overcount/large-ID/deep inputs, exact raw config bits and error-time lease disposal. CapturedAttention used as controlled test input does not claim restoration of an active STT job.

The serial [workspace verification](../verification.json) passes **{counts['passed']:,} tests, zero failures and {counts['ignored']} intentional ignores across {counts['targets']} targets** in {workspace['wall_seconds']:.2f} seconds. Final focused verification passes 133 tests and public verification passes six on the same frozen source; the focused log includes all three layout diagnostics. The [archive audit](log_archive_audit.json) checks exact original/archive hashes, parses totals independently and verifies each command's start-time source manifest. Development failures ({failure_names}) and earlier [setup failures](../development/setup_failures.md) remain recorded, with their corrections described in the owner handoff.

The [source audit](source_audit.json) verifies the complete 895-path v2 input scope and twenty changed source/fixture paths; sixteen Rust files pass scoped formatting. All 52 historical fixture/document files remain unchanged. Ordinary Engine, Conversation and checkpoint module bytes match base `fd2f3d4` after module declarations are removed; attention additionally contains only the two reviewed documentation corrections. Frozen source SHA-256: `{sha(e/'source_hashes.json')}`. Tests and measurements now use one shared input enumeration, including runtime defaults.

The [admission proof](../ADMISSION.md) pins Conversation152 B, Engagement64 B, standalone Conversation DTO168 B, Warm/Novelty24 B each, Memory32 B, map slots56 B, standalone Warm/Novelty DTO40 B and Engine DTO272 B. Sparse tree nodes and variable owned strings fit lexical E/J and the four-E allowance; fixed parser/serializer/inline-row work retains 64 KiB. Malformed overcount maps and oversized IDs remain metered before typed parsing. Validators build no secondary index. The [independent standard-library audit](stdlib_allocation_audit.json) verifies seven installed source hashes and nine exact excerpts for clone, sparse-node and growth assumptions. All global caps remain unchanged.

[Release measurements](../performance/README.md) independently audit 3,600 phase samples. Authored/populated export p99 is **{p99['authored']:.6f}/{p99['populated']:.6f} ms**, the median of three per-run percentiles; largest individual export is {largest:.6f} ms. All repetitions retain the ordinary reciprocal/focus/warm/Novelty witnesses. Save+Load reservation is 257,802/293,942 B excluding Running. Seven historical datasets pass the amended auditor, and four deliberately corrupted datasets fail their intended gates.

This accepts the social component only. Complete scheduler/Floor/SpeechRouter ownership, interrupted-draft policy, generation-fenced pending-work restoration, remaining Engine configuration/cadence and full World/Engine/host assembly remain pending. Full accepted-time/consumer horizons, exact content/build/target/toolchain/DefaultHasher resolution, host initial publication and Running/Save/Load/retiring lifetimes still need proof. The naive backbone+Round Save+Load reservation already exceeds 1 GiB, and earlier components exceed synchronous frame budgets. This adds no complete-save, renderer or full-stress acceptance.
'''
assert not (e/'coordinator/review.md').exists()
(e/'coordinator/review.md').write_text(review)
verification['status'] = 'Owner verification passed; coordinator component review and release acceptance complete'
verification['coordinator_review'] = 'coordinator/review.md'
(e/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')
def status(path, text):
    lines = path.read_text().splitlines()
    indices = [i for i,line in enumerate(lines) if line.startswith('Status:')]
    assert len(indices) == 1, path
    lines[indices[0]] = 'Status: '+text
    path.write_text('\n'.join(lines)+'\n')
for path in [e/'README.md',e/'OWNER_COVERAGE.md',e/'ADMISSION.md']:
    status(path,'Implemented, independently reviewed and accepted (2026-09-08). Existing component scope; complete M2/M3 remain pending and global caps are unchanged.')
path = e/'README.md'
value = path.read_text()
assert value.count('Coordinator release measurements remain pending.') == 1
path.write_text(value.replace('Coordinator release measurements remain pending.', 'Coordinator release measurements and independent acceptance are recorded below.'))
with (e/'README.md').open('a') as f:
    f.write(f'\n[Coordinator acceptance](coordinator/review.md) verifies {counts["passed"]:,} workspace passes, exact source/log/admission evidence and [3,600 release phase samples](performance/README.md). Authored/populated export p99 is {p99["authored"]:.6f}/{p99["populated"]:.6f} ms. Seven historical datasets and four negative auditor checks pass their intended gates.\n')
    f.write('\nThree development failures remain in the command archive: missing test imports/type inference, a test constructor that disabled the scripted cognition capability, and a probe witness that tried to serialize Vec3 directly. The corrections supply the test types, enable the intended scripted capability, and encode coordinates through to_array. No assertions were weakened. The earlier uv/cache and harness setup failures are retained separately in [setup notes](development/setup_failures.md).\n')
old = 'M1a–M1d and M2a1–M2a9 implemented and reviewed (2026-09-08). M2a10 social components are in progress.'
for path in [feature/'README.md',plan/'README.md']:
    value = path.read_text(); assert value.count(old) == 1
    path.write_text(value.replace(old,'M1a–M1d and M2a1–M2a10 implemented and reviewed (2026-09-08).'))
with (plan/'README.md').open('a') as f:
    f.write(f'\n[M2a10’s reviewed handoff](evidence/m2a10/README.md) adds exact Conversation/WarmExchanges/Novelty and original idle configuration. It passes {counts["passed"]:,} workspace tests and 3,600 audited release phase samples; export p99 is {p99["authored"]:.3f}/{p99["populated"]:.3f} ms. Remaining scheduler/speech/Engine ownership and complete envelope/capture/hydration stay pending.\n')
path = plan/'M2_simulation_checkpoints.md'
value = path.read_text()
old = 'M2a1–M2a9 private components are implemented and reviewed; M2a10 social components are in design/implementation.'
assert value.count(old) == 1
value = value.replace(old,'M2a1–M2a10 private components are implemented and reviewed.')
value += f'''\n\n#### M2a10 coordinator review — 2026-09-08

Strict Conversation/WarmExchanges/Novelty/EngineSocial components preserve independent partner/invitation/focus, historical warmth, prior witnesses, utterance counters, exact opaque context/visit salts and original idle configuration. [Coverage](evidence/m2a10/OWNER_COVERAGE.md) and [admission](evidence/m2a10/ADMISSION.md) bind exact fields and player identity without pruning or recomputation, under unchanged caps.

[Coordinator acceptance](evidence/m2a10/coordinator/review.md) verifies the frozen 895-path unified input scope, 133 focused/six public and **{counts['passed']:,} workspace passes**, private continuation and four new fixtures, complete archived failures, and all 3,600 release phase samples. Authored/populated export p99 is {p99['authored']:.3f}/{p99['populated']:.3f} ms; Save+Load reservations are 257,802/293,942 B excluding Running. All seven historical datasets pass the amended auditor, with four intended negative refusals.

Scheduler/Floor/SpeechRouter, remaining Engine configuration/cadence and host state, complete owner/root/manifest assembly, all-consumer horizons, actual cohort lifetimes, capture/hydration and generation-fenced pending-work restoration remain pending. Initial idle mode cannot rebuild scheduler fairness. Interrupted unsent attention is a later draft-policy transformation; this component introduces no active STT restoration or automatic speech. Complete M2 is not accepted.
'''
path.write_text(value)
path = plan/'PERSISTENCE_INVENTORY.md'
value = path.read_text()
old = 'New social fixtures preserve prior fixture bytes. Owner verification passes 2,045 workspace tests, with 133 focused checkpoint passes and six independent public boundaries on the unchanged source freeze. Independent coordinator release acceptance remains pending.'
assert value.count(old) == 1
value = value.replace(old,f'New social fixtures preserve prior fixture bytes. [Coordinator acceptance](evidence/m2a10/coordinator/review.md) verifies {counts["passed"]:,} workspace passes and 3,600 release phase samples, with complete source/log/admission audits.')
path.write_text(value)
print(json.dumps({'result':'review and docs installed','workspace':counts,'export_p99_ms':p99,'largest_export_ms':largest},indent=2))
