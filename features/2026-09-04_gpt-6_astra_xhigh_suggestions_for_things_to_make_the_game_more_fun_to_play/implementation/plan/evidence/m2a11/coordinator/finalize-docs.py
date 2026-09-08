from pathlib import Path
import datetime, hashlib, json, re

root = Path('/home/ran/src/rust/cathedralbevy')
feature = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play'
plan = feature / 'implementation/plan'
e = plan / 'evidence/m2a11'
load = lambda p: json.loads(p.read_bytes())
verification = load(e/'verification.json')
workspace = verification['workspace']
assert workspace['passed'] >= 2069 and workspace['failed'] == 0
assert any(v['passed']==151 and v['failed']==0 for v in verification['final_verification'].values())
assert any(v['passed']==6 and v['failed']==0 for v in verification['final_verification'].values())
for name in ['source_audit','format_audit','log_archive_audit','stdlib_allocation_audit',
    'release_archive_audit','scheduler_smoke_audit','scheduler_performance_audit',
    'auditor_regressions','tail_latency_audit',
    *['historical_'+x+'_audit' for x in ['round','climate','knowledge','law','marks','animals','night','social']]]:
    assert load(e/f'coordinator/{name}.json')['result']=='passed', name
summary = load(e/'performance/SUMMARY.json')
tails = load(e/'coordinator/tail_latency_audit.json')
source = load(e/'source_hashes.json')
source_sha = hashlib.sha256((e/'source_hashes.json').read_bytes()).hexdigest()
build = load(e/'coordinator/release_build.json')
logs = load(e/'coordinator/log_archive_audit.json')
fmt = lambda v: f'{v:,.0f}'
date = datetime.date(2026,9,8).isoformat()
lines = [f'# M2a11 scheduler release measurements — {date}', '',
    'Status: Component measurements accepted. Complete Engine capture, hydration, load-specific retry, files and host-frame acceptance remain pending.', '',
    'The frozen release binary ran three interleaved 100-sample trials for each population. Each sample records preflight, export, encode, decode/validation, candidate validation and drop. All 3,600 phase timings are retained with exact raw archives, process timing and source/binary/runner identity. World generation, the 136 ordinary setup polls, publication diagnostics and filesystem writes are outside these component timings.', '',
    '| Population | Characters | Weighted slots | Payload J (B) | Expanded E (B) | Component peak (B) | Save + Load peak (B) |',
    '|---|---:|---:|---:|---:|---:|---:|']
for mode,d in summary.items():
    m=d['metadata']; c=m['cost']; n=m['counts']
    lines.append(f'| {mode} | {n["characters"]:,} | {n["order_slots"]:,} | {c["encoded_bytes"]:,} | {c["expanded_upper_bytes"]:,} | {c["peak_bytes"]:,} | {m["shared_reserved_peak_excluding_running_bytes"]:,} |')
lines += ['', 'Both measured states retain a protected flight with its exact 59-byte held success, a later protected follow-up for that actor, one ordinary handoff, one failed semantic obligation and provider failure count one. The weighted cursor is zero because setup uses event-driven turns; private tests separately verify weighted fairness. The probe records all three original provider prompts and output budgets, separates later input from the submitted prompt, and uses at most 40 ms ordinary steps with zero coarse-discard diagnostics. A full publication FNV digest is diagnostic; the independently checked SHA-256 binds the entire primary witness JSON.', '',
    '| Population | Phase | Median per-run p50 (ms) | Median per-run p95 (ms) | Median per-run p99 (ms) |',
    '|---|---|---:|---:|---:|']
for mode,d in summary.items():
    for phase,q in d['median_run_us'].items():
        lines.append(f'| {mode} | {phase.removesuffix("_us")} | {q["p50"]/1000:.6f} | {q["p95"]/1000:.6f} | {q["p99"]/1000:.6f} |')
lines += ['', (e/'coordinator/tail_latency_table.md').read_text().strip(), '',
    'The 4 MiB validation allowance and `4,096 + 4E + 3J` aggregate charge are conservative admission bounds, not measured allocations. Process RSS includes the running Engine and setup. The existing 128 MiB E/J and shared 1 GiB ceilings are unchanged. Naive backbone+Round Save+Load already exceeds 1 GiB before Running; full composition needs an actual phase/lifetime solution.', '',
    '[Source and executable identity](IDENTITY.json), [results](RESULTS.json), [summary](SUMMARY.json), [independent audit](../coordinator/scheduler_performance_audit.json) and [release archive audit](../coordinator/release_archive_audit.json).', '']
assert not (e/'performance/README.md').exists()
(e/'performance/README.md').write_text('\n'.join(lines))

pooled = tails['pooled_300_sample_microseconds']
timing = '; '.join(f'{mode} export pooled p99 {pooled[mode]["export_us"]["p99"]/1000:.6f} ms, observed maximum {pooled[mode]["export_us"]["max"]/1000:.6f} ms' for mode in ['authored','populated'])
review = f'''Status: Accepted as the M2a11 scheduler component cut ({date}). Full M2 and host adoption remain pending.

# Independent coordinator review

The existing scheduler now preserves exact weighted order/cursor, both FIFO lanes, original submitted input, semantic retries, held success/error, pacing, failure count and submission notification. Engine composition separately preserves raw original configuration and immutable player binding. Admission permits a flying actor to be queued again, historical absent/incarnation state and known committed roots for replay suppression. A floor-held oversized answer is preserved for its ordinary failure path. No constructor, provider call, prompt rendering, ordinary poll or public partial installation occurs on decode.

All {len(source):,} frozen inputs match `{source_sha}`. The source audit checks 15 changed source/fixture paths, 11 scoped Rust formatting checks and all 56 historical fixture/doc files. Existing Engine/scheduler behavior is unchanged beyond module wiring; existing receipt checkpoint bytes remain intact with two appended borrowed TURN-root queries. The independent standard-library audit verifies eight installed files and 14 exact excerpts. Sequential saved-ledger scratch is 2,580,480 bytes; lane and root indexes fit the existing 4 MiB allowance.

The frozen focused suite passes 151 tests (23 ignored checks); six independent public boundaries pass; the full workspace passes {workspace['passed']:,}, with {workspace['ignored']} ignored and zero failures. Private tests deliberately scramble owner fields, require immediate canonical equality, then compare ordinary continuation, messages and World state. They include completed results held by floor or receipt capacity, failures/Busy in each lane, stale epochs and committed-root replay guards. All {logs['command_records']} command originals and their archives are independently verified with source-at-start manifests and preserved failures.

The initial private run exposed two test assumptions: Busy idle consumed a weighted slot, and a duplicate-key edit matched no bytes. Both were corrected with explicit assertions. Initial successful probe snapshots and provisional freeze are retained: their coarse quiet interval discarded physical work. The corrected probe uses 136 bounded ordinary polls, maximum 40 ms, and zero discarded-time diagnostics; it preserves original prompts/budgets and compact relevant events. No production owner behavior was changed for these corrections.

All 3,600 release phase samples and the two-sample smoke pass independent source/binary/counter/admission audits. Eight historical component datasets still pass the amended auditor without rerunning their probes. Six deliberate corruptions fail their intended gates. {timing}. There are {tails['samples_above_2ms_count']} phase samples above 2 ms; all remain in the [per-run and pooled tail audit](tail_latency_audit.json). Timing records do not identify their cause. See the [full phase tables](../performance/README.md).

Complete owner/root/content/build/target/toolchain agreement, Floor/SpeechRouter interruption policy, remaining Engine state, capture/hydration, generation-fenced load retries and actual Running/Save/Load/retiring lifetimes remain required. Current flight lacks a separately stored output-token budget, so M2c must bind the full original resolved input before retry. Component acceptance supplies no synchronous host-frame guarantee and raises no memory cap.

[Verification](../verification.json), [source audit](source_audit.json), [log archive audit](log_archive_audit.json), [standard-library proof audit](stdlib_allocation_audit.json), [release provenance](release_archive_audit.json).
'''
assert not (e/'coordinator/review.md').exists()
(e/'coordinator/review.md').write_text(review)
verification['status'] = f'Accepted M2a11 component after independent coordinator review ({date}); full M2/host pending'
verification['coordinator_review'] = 'coordinator/review.md'
verification['release_phase_samples'] = 3600
verification['release_binary_sha256'] = build['binary_sha256']['alibi_scheduler_cost']
(e/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')

def status(path, text):
    content = path.read_text()
    content,n = re.subn(r'^Status:.*$',text,content,count=1,flags=re.M)
    assert n==1, path
    path.write_text(content)
status(e/'README.md',f'Status: M2a11 component implemented, verified and independently accepted ({date}). Complete M2a/M2b/M2c and host adoption remain pending.')
content = (e/'README.md').read_text()
content += f'\nCoordinator acceptance verifies {workspace["passed"]:,} workspace passes, 151 focused passes, six public boundaries and all 3,600 release phase samples. The [review](coordinator/review.md) and [performance record](performance/README.md) include all long samples and explicit full-save/host limits. {timing}.\n'
(e/'README.md').write_text(content)
status(feature/'README.md',f'Status: M0 baseline delivered; M1a–M1d and M2a1–M2a11 implemented and reviewed ({date}). Remaining M2 owner/envelope work and M3–M19 remain. Sequential implementation continues at the developer\'s request. Reference-renderer/full-stress acceptance is pending.')
status(plan/'README.md',f'Status: M0 baseline delivered; M1a–M1d and M2a1–M2a11 implemented and reviewed ({date}). Remaining M2 owner/envelope work and M3–M19 remain. Sequential implementation continues. M0 renderer/full-stress evidence is pending; see EXECUTION_AUTHORITY.md.')
status(plan/'M2_simulation_checkpoints.md',f'Status: In progress ({date}). M2a1–M2a11 private components are implemented and reviewed. The complete M2a envelope and remaining owners, M2b capture/hydration, M2c pending-work restoration and M2d continuation remain pending.')
with (plan/'README.md').open('a') as stream:
    stream.write(f'\n[M2a11’s reviewed scheduler handoff](evidence/m2a11/README.md) preserves weighted fairness, queued reactions/handoffs, retry and exact held/submitted input with independent original Engine pacing configuration. The frozen workspace passes {workspace["passed"]:,}; all 3,600 release phase samples pass independent review. {timing}. Full phase tails remain in the [measurement record](evidence/m2a11/performance/README.md); complete save/load and host-frame acceptance remain pending.\n')
with (plan/'M2_simulation_checkpoints.md').open('a') as stream:
    stream.write(f'\n#### M2a11 coordinator acceptance — {date}\n\nThe [independent review](evidence/m2a11/coordinator/review.md) accepts exact existing scheduler and Engine configuration components against {len(source):,} frozen inputs, {workspace["passed"]:,} workspace passes, 151 focused passes and six public boundaries. Four new fixtures preserve all historical bytes. All 3,600 release phase samples pass source/binary/counter/admission audits; the corrected authored/+2,000 setup uses 136 bounded ordinary polls with no discarded physical time. {timing}. All tails are retained in the [performance record](evidence/m2a11/performance/README.md).\n\nFloor/SpeechRouter, remaining Engine fields, complete envelope/manifest/root agreement, all-consumer horizons, full capture/hydration, generation-fenced M2c resubmission and actual cohort lifetimes remain pending. Current saved flight retains the exact prompt but the derived provider output budget still needs a complete resolved-input receipt before retry. No synchronous host-frame or complete-save claim is made, and no byte cap increases.\n')
for name in ['ADMISSION.md', 'OWNER_COVERAGE.md']:
    path = e/name
    content = path.read_text()
    assert content.count('Coordinator release review is pending.') == 1, name
    path.write_text(content.replace('Coordinator release review is pending.',
        'Independent coordinator release review passed; see [review](coordinator/review.md).'))
print(json.dumps({'result':'accepted_component','workspace_passed':workspace['passed'],
    'source_files':len(source),'phase_samples':3600,'tails_above_2ms':tails['samples_above_2ms_count']}))
