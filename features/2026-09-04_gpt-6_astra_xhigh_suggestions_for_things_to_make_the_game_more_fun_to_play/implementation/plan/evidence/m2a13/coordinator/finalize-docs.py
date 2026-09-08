from pathlib import Path
import datetime, hashlib, json, re

root=Path('/home/ran/src/rust/cathedralbevy')
feature=root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play'
plan=feature/'implementation/plan';e=plan/'evidence/m2a13'
load=lambda p:json.loads(p.read_bytes())
verification=load(e/'verification.json');workspace=verification['workspace']
assert workspace['passed']>2090 and workspace['failed']==0
final=verification['final_verification']
focused=next(v for k,v in final.items() if k.startswith('checkpoint'))
public=next(v for k,v in final.items() if k.startswith('public'))
assert focused['passed']==183 and focused['failed']==0
assert public['passed']==6 and public['failed']==0
historical=['round','climate','knowledge','law','marks','animals','night','social','scheduler','continuity']
for name in ['source_audit','format_audit','log_archive_audit','stdlib_allocation_audit','layout_audit',
    'release_archive_audit','speech_smoke_audit','speech_performance_audit',
    'auditor_regressions','tail_latency_audit',*['historical_'+x+'_audit' for x in historical]]:
    assert load(e/f'coordinator/{name}.json')['result']=='passed',name
source=load(e/'source_hashes.json');source_sha=hashlib.sha256((e/'source_hashes.json').read_bytes()).hexdigest()
source_audit=load(e/'coordinator/source_audit.json');formats=load(e/'coordinator/format_audit.json')
logs=load(e/'coordinator/log_archive_audit.json');build=load(e/'coordinator/release_build.json')
summary=load(e/'performance/SUMMARY.json');tails=load(e/'coordinator/tail_latency_audit.json')
date=datetime.date(2026,9,9).isoformat();pooled=tails['pooled_300_sample_microseconds']
timing='; '.join(f'{mode} export pooled p99 {pooled[mode]["export_us"]["p99"]/1000:.6f} ms, observed maximum {pooled[mode]["export_us"]["max"]/1000:.6f} ms' for mode in ['authored','populated'])
lines=[f'# M2a13 speech release measurements — {date}','','Status: Component measurements accepted. Complete capture, hydration, interruption adoption, files and host-frame acceptance remain pending.','',
    'Three interleaved 100-sample trials for each population retain all 3,600 phase timings. Each sample measures preflight, export, encode, decode/validation, candidate validation and drop. Engine construction, ordinary bounded setup, diagnostic witnesses and filesystem work are outside these component timings. Source, executable and runner identities are pinned at start and checked at end.','',
    '| Population | Characters | Accepted recordings | Available drafts | Payload J (B) | Expanded E (B) | Component peak (B) | Save + Load peak (B) |',
    '|---|---:|---:|---:|---:|---:|---:|---:|']
for mode,d in summary.items():
    m=d['metadata'];c=m['cost'];n=m['counts']
    assert c['validation_working_bytes']==4194304
    assert m['witnesses']['coarse_discard_diagnostics']==0
    assert 0<m['witnesses']['maximum_poll_step_seconds']<=.05+1e-12
    lines.append(f'| {mode} | {n["characters"]:,} | {n["accepted_recordings"]} | {n["available_texts"]} | {c["encoded_bytes"]:,} | {c["expanded_upper_bytes"]:,} | {c["peak_bytes"]:,} | {m["shared_reserved_peak_excluding_running_bytes"]:,} |')
lines+=['',
    'Ordinary typed speech and scripted provider/TTS/STT values establish two committed Speech publications, microphone onset, a completed unsubmitted 85-byte draft, an active stream, two batch jobs and one parked recording. Sibling command steps share a root and filename while remaining distinct obligations. All 17 setup polls are at most 0.04 seconds, with no discarded physical time. Real content/navigation and successful +2,000 placement are used. No private Engine or World mutations manufacture the timed boundary; no audio device or external provider runs. Primary witnesses retain submitted input, prompts and output budgets, TTS/STT requests, all committed Speech and exact boundary records. The full-publication FNV digest is diagnostic only.','',
    '| Population | Phase | Median per-run p50 (ms) | Median per-run p95 (ms) | Median per-run p99 (ms) |','|---|---|---:|---:|---:|']
for mode,d in summary.items():
    for phase,q in d['median_run_us'].items():
        lines.append(f'| {mode} | {phase.removesuffix("_us")} | {q["p50"]/1000:.6f} | {q["p95"]/1000:.6f} | {q["p99"]/1000:.6f} |')
lines+=['',(e/'coordinator/tail_latency_table.md').read_text().strip(),'',
    'Validation reserves 4 MiB for bounded saved-ledger sets; subsequent at-most-eight-row scans borrow records and allocate no index. The `4,096 + 4E + 3J + 4,194,304` reservation is a conservative admission bound, not measured allocation. Variable parser/error strings remain charged to E/J. Process RSS includes the running Engine and setup. Existing 128 MiB E/J, depth 64 and shared 1 GiB ceilings are unchanged. Earlier naive backbone+Round Save+Load already exceeds 1 GiB before Running; complete integration must solve actual phase/lifetimes.','',
    '[Identity](IDENTITY.json), [results](RESULTS.json), [summary](SUMMARY.json), [independent audit](../coordinator/speech_performance_audit.json) and [release provenance](../coordinator/release_archive_audit.json).','']
assert not (e/'performance/README.md').exists();(e/'performance/README.md').write_text('\n'.join(lines))
review=f'''Status: Accepted as the M2a13 interrupted speech input component ({date}). Full M2 and host adoption remain pending.

# Independent coordinator review

Strict read-only SpeechRouter/Engine projections retain separate ordered captures, available stream text and accepted recording obligations with exact optional command IDs, retained receipts, request correlation, accepted pose/backend and parked deadline provenance. Grace bits remain exact and independent of stored Engine configuration. The projection classifies inputs as interrupted and unsent; it cannot construct an active router, poll services, resubmit STT, apply a say or partially adopt an Engine.

All {len(source):,} frozen inputs match `{source_sha}`. Source review verifies {len(source_audit['changed_paths'])} changed source/fixture paths and {len(formats['checks'])} scoped Rust formatting checks. Removing seven wiring insertions from four existing files leaves their ordinary bytes unchanged. All {len(source_audit['historical_fixtures_and_docs_unchanged'])} historical fixture/doc files remain intact. Eleven installed Rust/parser sources and 24 exact excerpts were independently checked. Emitted layouts include 248-byte accepted rows, 48-byte stream rows and 120-byte receipts. The loose saved-ledger tree bound is 2,580,480 bytes, below the 4 MiB working allowance with fixed framing; raw lexical and retained candidate charges preserve existing limits.

The final focused suite passes {focused['passed']} tests, the six independent public boundaries pass, and the full workspace passes {workspace['passed']:,} with zero failures and {workspace['ignored']} intentional ignores. Tests preserve invalid-for-submission text, Unicode basenames, duplicate filename/request correlations with sibling command steps, already terminal receipts and raw float bits. They reject mismatched receipts/roots, live owner-index drift, missing/duplicate/unknown fields, type aliases, unsupported lengths/counts and mid-command resolved/unflushed state. Independent public tests also prove arbitrary abandoned stream-job counts do not create a false active-eight bound and that raw padding/candidate leases retain their charges. Saved-backbone binding uses no candidate World or copied ledger. These tests establish component projection and validation, not complete interruption adoption.

All {logs['command_records']} command originals and source-at-start manifests are independently checked and losslessly archived. Development failures remain in [commands.json](../commands.json): an incorrect test Admission variant, an invalid attempt to encode under a LoadCandidate lease, and the coordinator test's direct terminal-receipt edit with an unflushed notification. Only harness code was corrected for those failures; production completed-boundary and lease checks remain intact. The earlier authored smoke is honestly provisional.

The coordinator source auditor initially assumed a fixture subdirectory, and the library auditor initially assumed parser crates were direct children instead of symlinks to the installed registry. Failed scripts and explicit summaries are retained; original tool-returned tracebacks were not redirected and are not represented as archived raw logs. These audit adapter corrections required no source change.

All 3,600 release phase samples and 24 smoke phase samples pass source/executable/counter/admission checks. Ten historical datasets still pass the amended auditor without rerunning their probes. Seven deliberate corruptions fail their intended gates. {timing}. All {tails['samples_above_2ms_count']} phase samples above 2 ms remain in the [tail audit](tail_latency_audit.json); phase records alone do not establish a cause. Full measurements are in the [performance record](../performance/README.md).

Full M2c must atomically consume this projection with Floor/ledger/shared roots, preserve already terminal receipts, interrupt accepted obligations, remove old execution/holds and publish unsent drafts requiring a new intentional submission. Previously committed say effects remain committed; owed readable presentation must come from its actual host/event owner. Original submitted spatial sequence/payload pose is absent from router tasks, so this owner classification does not prove the original command digest. Complete owner/category/root/content/build/target/toolchain agreement, original cognition output budgets, player-readable history, capture/hydration, generation-fenced pending work, actual Running/Save/Load/retiring lifetimes and host scheduling remain required. No memory cap increase or complete-save/host-frame guarantee is supplied.

[Verification](../verification.json), [source audit](source_audit.json), [log audit](log_archive_audit.json), [library proof](stdlib_allocation_audit.json), [release audit](release_archive_audit.json).
'''
assert not (e/'coordinator/review.md').exists();(e/'coordinator/review.md').write_text(review)
verification.update(status=f'Accepted M2a13 component after independent review ({date}); full M2/host pending',coordinator_review='coordinator/review.md',release_phase_samples=3600,release_binary_sha256=build['binary_sha256']['alibi_speech_cost'])
(e/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')
def status(path,text):
    content,n=re.subn(r'^Status:.*$',text,path.read_text(),count=1,flags=re.M);assert n==1,path;path.write_text(content)
for name in ['README.md','ADMISSION.md','OWNER_COVERAGE.md']:
    status(e/name,f'Status: M2a13 component implemented, verified and independently accepted ({date}); see [coordinator review](coordinator/review.md). Complete M2 and host adoption remain pending.')
with (e/'README.md').open('a') as file:
    file.write(f'\nCoordinator acceptance verifies {workspace["passed"]:,} workspace passes, {focused["passed"]} focused passes, six public boundaries and all 3,600 release phase samples. {timing}. The [performance record](performance/README.md) retains all samples and complete-save/host limitations.\n')
status(feature/'README.md',f'Status: M0 baseline delivered; M1a–M1d and M2a1–M2a13 implemented and reviewed ({date}). Remaining M2 envelope/adoption work and M3–M19 remain. Sequential implementation continues at the developer\'s request. Reference-renderer/full-stress acceptance is pending.')
status(plan/'README.md',f'Status: M0 baseline delivered; M1a–M1d and M2a1–M2a13 implemented and reviewed ({date}). Remaining M2 envelope/adoption work and M3–M19 remain. Sequential implementation continues. M0 renderer/full-stress evidence is pending; see EXECUTION_AUTHORITY.md.')
status(plan/'M2_simulation_checkpoints.md',f'Status: In progress ({date}). M2a1–M2a13 private components are implemented and reviewed. Complete M2a envelope and remaining host owners, M2b capture/hydration, M2c pending-work restoration and M2d continuation remain pending.')
with (plan/'README.md').open('a') as file:
    file.write(f'\n[M2a13’s reviewed speech handoff](evidence/m2a13/README.md) adds exact interrupted input drafts and accepted recording obligations. The frozen workspace passes {workspace["passed"]:,}; all 3,600 release phase samples pass independent review. {timing}. Complete assembly, interruption adoption and actual save/load/host acceptance remain pending.\n')
with (plan/'M2_simulation_checkpoints.md').open('a') as file:
    file.write(f'\n#### M2a13 coordinator acceptance — {date}\n\nThe [independent review](evidence/m2a13/coordinator/review.md) accepts interrupted speech projections against {len(source):,} frozen inputs, {workspace["passed"]:,} workspace passes, {focused["passed"]} focused passes and six public boundaries. New fixtures leave all historical bytes intact. Ordinary authored/+2,000 setup uses 17 bounded polls with zero discarded physical time. All 3,600 release phase samples pass provenance/counter/admission audits. {timing}. See the [full performance record](evidence/m2a13/performance/README.md).\n\nComplete manifest/root/category agreement, original cognition output budgets, full capture/hydration, atomic Floor/ledger interruption and host draft/readable publication remain pending. The candidate cannot restore an active microphone or automatically submit speech. Actual phase/lifetimes must resolve the earlier complete-save peak excess without a cap increase; no synchronous host-frame claim is made.\n')
print(json.dumps({'result':'accepted_component','workspace_passed':workspace['passed'],'focused_passed':focused['passed'],'source_files':len(source),'phase_samples':3600}))
