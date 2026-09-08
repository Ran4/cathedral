from pathlib import Path
import datetime, hashlib, json, re

root = Path('/home/ran/src/rust/cathedralbevy')
feature = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play'
plan = feature/'implementation/plan'
e = plan/'evidence/m2a12'
load = lambda p: json.loads(p.read_bytes())
verification = load(e/'verification.json')
workspace = verification['workspace']
assert workspace['passed'] > 2069 and workspace['failed'] == 0
final = verification['final_verification']
focused = next(v for k,v in final.items() if k.startswith('checkpoint'))
public = next(v for k,v in final.items() if k.startswith('public'))
assert focused['passed'] > 151 and focused['failed'] == 0
assert public['passed'] == 6 and public['failed'] == 0
historical = ['round','climate','knowledge','law','marks','animals','night','social','scheduler']
for name in ['source_audit','format_audit','log_archive_audit','stdlib_allocation_audit',
    'release_archive_audit','continuity_smoke_audit','continuity_performance_audit',
    'auditor_regressions','tail_latency_audit',*['historical_'+x+'_audit' for x in historical]]:
    assert load(e/f'coordinator/{name}.json')['result'] == 'passed', name
source = load(e/'source_hashes.json')
source_sha = hashlib.sha256((e/'source_hashes.json').read_bytes()).hexdigest()
source_audit = load(e/'coordinator/source_audit.json')
formats = load(e/'coordinator/format_audit.json')
logs = load(e/'coordinator/log_archive_audit.json')
build = load(e/'coordinator/release_build.json')
summary = load(e/'performance/SUMMARY.json')
tails = load(e/'coordinator/tail_latency_audit.json')
date = datetime.date(2026,9,9).isoformat()
pooled = tails['pooled_300_sample_microseconds']
timing = '; '.join(f'{mode} export pooled p99 {pooled[mode]["export_us"]["p99"]/1000:.6f} ms, observed maximum {pooled[mode]["export_us"]["max"]/1000:.6f} ms' for mode in ['authored','populated'])
lines = [f'# M2a12 continuity release measurements — {date}', '',
    'Status: Component measurements accepted. Complete capture, hydration, speech interruption, files and host-frame acceptance remain pending.', '',
    'Three interleaved 100-sample trials for each population retain all 3,600 phase timings. Each sample measures preflight, export, encode, decode/validation, candidate validation and drop. Engine construction, ordinary bounded setup, diagnostic witnesses and filesystem work are outside these component timings. Source, executable and runner identities are pinned at start and checked at end.', '',
    '| Population | Characters | Awaited speech | Payload J (B) | Expanded E (B) | Component peak (B) | Save + Load peak (B) |',
    '|---|---:|---:|---:|---:|---:|---:|']
for mode,d in summary.items():
    m=d['metadata']; c=m['cost']; n=m['counts']
    assert c['validation_working_bytes'] == 0
    assert m['witnesses']['coarse_discard_diagnostics'] == 0
    assert 0 < m['witnesses']['maximum_poll_step_seconds'] <= .05+1e-12
    lines.append(f'| {mode} | {n["characters"]:,} | {n["floor"]["awaiting"]} | {c["encoded_bytes"]:,} | {c["expanded_upper_bytes"]:,} | {c["peak_bytes"]:,} | {m["shared_reserved_peak_excluding_running_bytes"]:,} |')
lines += ['',
    'Ordinary typed speech and scripted provider/TTS values establish acknowledgements, a reply held through the conversation beat, accepted voiced speech, reading fallback after TTS refusal, a player sound and cooldown refusal, an active microphone hold, and Cloud-to-Local selection. The exact initial configuration remains distinct from current voice selection. No private Engine or World mutations manufacture the measured boundary; no audio service runs. Primary witnesses retain submitted input, TTS requests, all committed Speech publications and the exact boundary record. The full-publication FNV digest is diagnostic only.', '',
    '| Population | Phase | Median per-run p50 (ms) | Median per-run p95 (ms) | Median per-run p99 (ms) |',
    '|---|---|---:|---:|---:|']
for mode,d in summary.items():
    for phase,q in d['median_run_us'].items():
        lines.append(f'| {mode} | {phase.removesuffix("_us")} | {q["p50"]/1000:.6f} | {q["p95"]/1000:.6f} | {q["p99"]/1000:.6f} |')
lines += ['', (e/'coordinator/tail_latency_table.md').read_text().strip(), '',
    'Validation uses borrowed player lookup and pairwise comparison of at most 32 awaited IDs, with no additional allocated index. The `4,096 + 4E + 3J` reservation is a conservative admission bound, not measured allocation. Variable parser/error strings remain charged to E/J. Process RSS includes the running Engine and setup. Existing 128 MiB E/J, depth 64 and shared 1 GiB ceilings are unchanged. Earlier naive backbone+Round Save+Load already exceeds 1 GiB before Running; complete integration must solve actual phase/lifetimes.', '',
    '[Identity](IDENTITY.json), [results](RESULTS.json), [summary](SUMMARY.json), [independent audit](../coordinator/continuity_performance_audit.json) and [release provenance](../coordinator/release_archive_audit.json).', '']
assert not (e/'performance/README.md').exists()
(e/'performance/README.md').write_text('\n'.join(lines))
review = f'''Status: Accepted as the M2a12 floor/Engine continuity component ({date}). Full M2 and host adoption remain pending.

# Independent coordinator review

Exact Floor records preserve insertion order, refreshed deadlines, scoped awaiting IDs and independent reading/beat/player holds without purging or rebasing. Engine composition adds original stored settings, current voice selection, startup/ready state, publication revisions and cadence/sound anchors. Closed read-only candidates do not construct or poll an Engine, replay speech, resume audio or adopt partial production state.

All {len(source):,} frozen inputs match `{source_sha}`. Source review verifies {len(source_audit['changed_paths'])} changed source/fixture paths and {len(formats['checks'])} scoped Rust formatting checks. Removing only the new module declaration from Engine/Floor leaves their ordinary bytes unchanged. All {len(source_audit['historical_fixtures_and_docs_unchanged'])} historical fixture/doc files remain intact. Exact layouts and installed-library excerpts support allocation-free validation scratch; raw lexical and retained candidate charges preserve the existing limits.

The final focused suite passes {focused['passed']} tests, the six independent public boundaries pass, and the full workspace passes {workspace['passed']:,} with zero failures and {workspace['ignored']} intentional ignores. Private continuation scrambles covered fields, restores immediate canonical equality and then compares ordinary messages and World behavior. Public boundaries cover arbitrary speech IDs, refresh/trim order, scoped pacing and acknowledgement behavior, historic/+infinite holds, signed-zero boundaries, raw padding/lease failure, stored config bits, current-versus-initial TTS and strict nullable/player/config refusal. The saved-backbone test proves reference binding under separate component budgets; it does not certify complete-envelope coexistence.

All {logs['command_records']} command originals and source-at-start manifests are independently checked and losslessly archived. Development failures remain in [commands.json](../commands.json) and owner notes. Initial private setup incorrectly assumed independently seeded missing-lamp diagnostics had identical order; the tests now restore captured initial authority and use explicit diagnostic fixture text without sorting production state. A later saved-backbone harness attempted two simultaneous leases in one LoadCandidate cohort and was corrected before decode verification.

Coordinator source checks first had an incorrect blank-line expectation in the audit adapter, then found module ordering that required scoped formatting. Both reports are retained. The module-order correction changes no ordinary behavior; source was frozen again before final verification and release measurements.

All 3,600 release phase samples and 24 smoke phase samples pass source/executable/counter/admission checks. Nine historical datasets still pass the amended auditor without rerunning their probes. Deliberate corruptions fail their intended gates. {timing}. All {tails['samples_above_2ms_count']} phase samples above 2 ms remain in the [tail audit](tail_latency_audit.json); phase records alone do not establish a cause. Full measurements are in the [performance record](../performance/README.md).

Exact saved Floor is an input to the later interruption policy: old audio acknowledgements and microphone holds must be transformed together with SpeechRouter obligations and owed readable presentation. Do not replay a committed say to republish it or reset saved caches to force Ready. Complete owner/root/content/build/target/toolchain agreement, player-readable history, capture/hydration, generation-fenced pending work, actual Running/Save/Load/retiring lifetimes and host scheduling remain required. No memory cap increases or complete-save/host-frame guarantee are supplied by this component.

[Verification](../verification.json), [source audit](source_audit.json), [log audit](log_archive_audit.json), [library proof](stdlib_allocation_audit.json), [release audit](release_archive_audit.json).
'''
assert not (e/'coordinator/review.md').exists()
(e/'coordinator/review.md').write_text(review)
verification.update(status=f'Accepted M2a12 component after independent review ({date}); full M2/host pending',
    coordinator_review='coordinator/review.md', release_phase_samples=3600,
    release_binary_sha256=build['binary_sha256']['alibi_continuity_cost'])
(e/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')
def status(path,text):
    content,n = re.subn(r'^Status:.*$',text,path.read_text(),count=1,flags=re.M)
    assert n == 1, path
    path.write_text(content)
for name in ['README.md','ADMISSION.md','OWNER_COVERAGE.md']:
    status(e/name,f'Status: M2a12 component implemented, verified and independently accepted ({date}); see [coordinator review](coordinator/review.md). Complete M2 and host adoption remain pending.')
with (e/'README.md').open('a') as file:
    file.write(f'\nCoordinator acceptance verifies {workspace["passed"]:,} workspace passes, {focused["passed"]} focused passes, six public boundaries and all 3,600 release phase samples. {timing}. The [performance record](performance/README.md) retains all samples and the complete-save/host limitations.\n')
status(feature/'README.md',f'Status: M0 baseline delivered; M1a–M1d and M2a1–M2a12 implemented and reviewed ({date}). Remaining M2 owner/envelope work and M3–M19 remain. Sequential implementation continues at the developer\'s request. Reference-renderer/full-stress acceptance is pending.')
status(plan/'README.md',f'Status: M0 baseline delivered; M1a–M1d and M2a1–M2a12 implemented and reviewed ({date}). Remaining M2 owner/envelope work and M3–M19 remain. Sequential implementation continues. M0 renderer/full-stress evidence is pending; see EXECUTION_AUTHORITY.md.')
status(plan/'M2_simulation_checkpoints.md',f'Status: In progress ({date}). M2a1–M2a12 private components are implemented and reviewed. The complete M2a envelope and remaining owners, M2b capture/hydration, M2c pending-work restoration and M2d continuation remain pending.')
with (plan/'README.md').open('a') as file:
    file.write(f'\n[M2a12’s reviewed continuity handoff](evidence/m2a12/README.md) adds exact conversation pacing and remaining Engine cadence/publication/stored configuration. The frozen workspace passes {workspace["passed"]:,}; all 3,600 release phase samples pass independent review. {timing}. SpeechRouter interruption, complete assembly and actual save/load/host acceptance remain pending.\n')
with (plan/'M2_simulation_checkpoints.md').open('a') as file:
    file.write(f'\n#### M2a12 coordinator acceptance — {date}\n\nThe [independent review](evidence/m2a12/coordinator/review.md) accepts Floor and Engine continuity against {len(source):,} frozen inputs, {workspace["passed"]:,} workspace passes, {focused["passed"]} focused passes and six public boundaries. New fixtures leave all historical bytes intact. The ordinary authored/+2,000 setup uses bounded polls with zero discarded physical time. All 3,600 release phase samples pass provenance/counter/admission audits. {timing}. See the [full performance record](evidence/m2a12/performance/README.md).\n\nSpeechRouter interruption and readable presentation, complete manifest/root/owner agreement, full capture/hydration, pending-work execution replacement and actual phase/lifetimes remain pending. Saved Floor holds are not a promise to resume old audio or microphone activity. No cap increase or synchronous host-frame claim is made.\n')
print(json.dumps({'result':'accepted_component','workspace_passed':workspace['passed'],
    'focused_passed':focused['passed'],'source_files':len(source),'phase_samples':3600}))
