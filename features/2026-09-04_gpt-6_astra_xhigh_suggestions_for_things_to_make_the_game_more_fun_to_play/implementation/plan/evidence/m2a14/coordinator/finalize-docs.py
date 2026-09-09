from pathlib import Path
import datetime, gzip, hashlib, json, re

root = Path('/home/ran/src/rust/cathedralbevy')
feature = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play'
plan = feature / 'implementation/plan'
e = plan / 'evidence/m2a14'
load = lambda p: json.loads(p.read_bytes())
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
checks = ['source_audit', 'format_audit', 'layout_audit', 'log_archive_audit', 'debug_workload_audit', 'release_archive_audit', 'auditor_regressions', 'tail_latency_audit']
checks += [f'{owner}_{kind}_audit' for owner in ('scheduler', 'night') for kind in ('smoke', 'performance')]
checks += ['historical_' + name + '_audit' for name in ('backbone', 'round', 'climate', 'knowledge', 'law', 'marks', 'animals', 'night', 'social', 'scheduler', 'continuity', 'speech')]
for name in checks:
    assert load(e / f'coordinator/{name}.json')['result'] == 'passed', name
verification = load(e / 'verification.json')
assert verification['workspace']['passed'] == 2129 and verification['workspace']['failed'] == 0
assert verification['final_focused']['passed'] == 193 and verification['final_public']['passed'] == 6
source_hash = sha(e / 'source_hashes.json')
assert source_hash == '8b96ea02213a334ed66e744aec624738b62b89654be7389c19d5d3c5c2dbe291'
tails = load(e / 'coordinator/tail_latency_audit.json')
timings = '; '.join(f'{owner} {mode} export pooled p99 {values["export_us"]["p99"]/1000:.6f} ms' for owner, modes in tails['pooled_300_sample_microseconds'].items() for mode, values in modes.items())
lines = ['# M2a14 exact-input release measurements — 2026-09-09', '',
    'Status: Component measurements accepted. Full capture, hydration, retry/adoption and host-frame acceptance remain pending.', '',
    'Each owner uses three interleaved 100-sample trials per population. All 7,200 phase samples and 48 smoke phase samples are retained. Engine construction, ordinary bounded setup, diagnostic witnesses and filesystem work are outside these component timings. Full source, binary and runner identities are checked before and after the commands.', '',
    '| Owner | Population | Actors | Payload J (B) | Expanded E (B) | Component peak (B) | Save + Load peak (B) |',
    '|---|---|---:|---:|---:|---:|---:|']
for owner in ('scheduler', 'night'):
    summary = load(e / f'performance/{owner}/SUMMARY.json')
    for mode, data in summary.items():
        m = data['metadata']; c = m['cost']
        assert c['validation_working_bytes'] == 4194304
        assert m['witnesses']['coarse_discard_diagnostics'] == 0
        actors = 520 if mode == 'authored' else 2520
        lines.append(f'| {owner} | {mode} | {actors:,} | {c["encoded_bytes"]:,} | {c["expanded_upper_bytes"]:,} | {c["peak_bytes"]:,} | {m["shared_reserved_peak_excluding_running_bytes"]:,} |')
lines += ['', 'Scheduler setup retains three actual successful calls and one exact saved flight, with held success, protected player reaction, handoff and retry authority. Night retains two actual calls, a saved Wick ward flight, held success and a previously committed ward mood. The submitted request method, full prompt and budget are independently matched to the saved row and a pinned primary witness hash. Real content/navigation places all 2,000 extra actors. Ordinary setup uses at most 40 ms scheduler steps and nominal 50 ms Night steps; neither discards physical time. Defaults preserve the old component metadata exactly. No visible application, audio device or external provider runs.', '',
    (e / 'coordinator/tail_latency_table.md').read_text().strip().replace('(tail_latency_audit.json)', '(../coordinator/tail_latency_audit.json)'), '',
    'Admission keeps 4 MiB of sequential borrowed old-owner validation scratch. The reservation is a conservative bound, not a measured allocator peak. Existing 128 MiB encoded/expanded, depth 64 and shared 1 GiB caps remain unchanged. The earlier naive backbone+Round Save+Load peak of 1,256,093,444 bytes exceeds 1 GiB before Running; full integration must solve actual phases and lifetimes.', '',
    '[Scheduler identity](scheduler/IDENTITY.json), [scheduler results](scheduler/RESULTS.json), [scheduler summary](scheduler/SUMMARY.json), [Night identity](night/IDENTITY.json), [Night results](night/RESULTS.json), [Night summary](night/SUMMARY.json), [release provenance](../coordinator/release_archive_audit.json).', '']
assert not (e / 'performance/README.md').exists()
(e / 'performance/README.md').write_text('\n'.join(lines))
review = f'''Status: Accepted as the M2a14 exact cognition input component (2026-09-09). Full M2 and host adoption remain pending.

# Independent coordinator review

Scheduler and Night retain the successful request's actual output-token argument alongside its existing flight. The new read-only admitted sidecar binds that argument to the exact original prompt, request method and old actor/ward, incarnation, lane, request, semantic root and owed-day identity. Busy calls acquire no accepted input. Historical V1 export, copy and decode explicitly strip the new authority to MissingLegacy and preserve their wire bytes; MissingLegacy is never intentional provider-default None. New sidecars refuse unknown live authority.

Independent review covers runtime ownership, strict closed decoding, mandatory nullable fields, borrowed live/saved binding, bitwise scheduler and nested Night/world boundaries, legacy injection refusal and exact prompt retention. No constructor, hydration, service call, retry or partial installation is provided. The six public tests record actual calls, change later actor lore/goals/inboxes, exercise Busy and both Night subjects, bind old unadopted candidates, and verify lexical padding/retained leases and malformed fields. Private tests cover all lanes, held/disabled/stale/error cleanup and the full optional budget domain against legacy unknown authority.

All 938 source inputs match `{source_hash}`. The source audit checks 23 changed paths, 22 scoped Rust formatting checks and 67 unchanged historical fixture/doc files. Four ordinary files are byte-identical after removing the intended wiring/flight-budget edits. New DTO/candidate layout is 248 bytes, scheduler row 96 bytes and Night row 112 bytes. The eight-byte budget enum grows old inline flight owners and remains covered by their existing bounds. Eleven unchanged installed allocator/parser sources and 24 excerpts reuse the prior proof with matching current toolchain identity. The loose 2,580,480-byte ledger bound and sequential scheduler/Night scratch remain below 4 MiB with framing; context creation only borrows.

The frozen workspace passes 2,129 tests with zero failures and 36 intentional ignores across 46 result-bearing targets. Focused tests pass 193 with 28 ignores; the independent public suite passes six. All 19 owner command originals, lossless archives and source-at-start maps pass independent audit. Development failures remain recorded: a private public-test import, three old test Flight literals missing the new field, and four test expectations copied from stale token-budget prose. Harness corrections preserved production behavior and all final checks ran on the frozen source. The coordinator source auditor's initial expectation of Accepted(None) in a legacy-only test literal was corrected to MissingLegacy; its original failure is retained.

All 7,200 release phase samples and 48 smoke phase samples pass independent provenance, counter, exact-input and admission checks. Twelve historical datasets retain and revalidate 43,200 archived phase samples without rerunning their workloads. Eight deliberate corruptions fail their intended gates, including a coherently rewritten prompt/budget witness and a missing runtime-default source. {timings}. No phase sample exceeds 2 ms. These component timings do not establish full host capture or frame acceptance.

Complete M2 still requires the full manifest, build/target/toolchain/DefaultHasher and category/root agreement, all-consumer numeric/time horizons, full capture/hydration and M2c execution replacement. Retry must use these exact inputs once while retaining scheduler priority/fairness and later arrivals; held outcomes apply once without a provider. Speech interruption must preserve terminal receipts and publish unsent drafts requiring new intentional submission. Host controller/custody/vermin/semantic audio gates and owed readable text need their actual owner bindings. M3 must publish a newly adopted generation without spending a poll. Actual Running/Save/Load/retiring lifetimes must solve the earlier full-composition memory excess without a cap increase.

[Verification](../verification.json), [source audit](source_audit.json), [allocation/layout audit](layout_audit.json), [log audit](log_archive_audit.json), [release audit](release_archive_audit.json), [performance record](../performance/README.md).
'''
assert not (e / 'coordinator/review.md').exists()
(e / 'coordinator/review.md').write_text(review)
verification.update(status='Accepted M2a14 component after independent review (2026-09-09); full M2/host pending', coordinator_review='coordinator/review.md', release_phase_samples=7200, release_binary_sha256=load(e / 'coordinator/release_build.json')['binary_sha256'])
(e / 'verification.json').write_text(json.dumps(verification, indent=2) + '\n')
def status(path, text):
    value, count = re.subn(r'^Status:.*$', text, path.read_text(), count=1, flags=re.M)
    assert count == 1, path
    path.write_text(value)
for name in ('README.md', 'ADMISSION.md', 'OWNER_COVERAGE.md'):
    status(e / name, 'Status: M2a14 component implemented, verified and independently accepted (2026-09-09); see [coordinator review](coordinator/review.md). Complete M2 and host adoption remain pending.')
with (e / 'README.md').open('a') as f:
    f.write(f'\nCoordinator acceptance verifies 2,129 workspace passes, 193 focused passes, six public boundaries and all 7,200 release phase samples. {timings}. The [performance record](performance/README.md) retains all samples and complete-save/host limitations.\n')
for path in (feature / 'README.md', plan / 'README.md'):
    status(path, 'Status: M0 baseline delivered; M1a–M1d and M2a1–M2a14 implemented and reviewed (2026-09-09). Remaining M2 envelope/adoption work and M3–M19 remain. Sequential implementation continues. M0 renderer/full-stress evidence is pending.')
status(plan / 'M2_simulation_checkpoints.md', 'Status: In progress (2026-09-09). M2a1–M2a14 private components are implemented and reviewed. Complete M2a envelope and remaining host owners, M2b capture/hydration, M2c pending-work restoration and M2d continuation remain pending.')
with (plan / 'README.md').open('a') as f:
    f.write('\n[M2a14’s reviewed handoff](evidence/m2a14/README.md) retains exact accepted scheduler/Night request inputs. The frozen workspace passes 2,129 tests; all 7,200 release phase samples pass independent review. Complete assembly, retry/adoption and host restoration remain pending.\n')
with (plan / 'M2_simulation_checkpoints.md').open('a') as f:
    f.write(f'\n#### M2a14 coordinator acceptance — 2026-09-09\n\nThe [independent review](evidence/m2a14/coordinator/review.md) accepts exact cognition input authority against 938 frozen inputs, 2,129 workspace passes, 193 focused passes and six public boundaries. All 7,200 release phase samples pass; historical defaults and fixture bytes remain unchanged. {timings}. Complete envelope/root agreement, full capture/hydration, once-only pending-work adoption, host owner restoration and actual phase/lifetime admission remain pending. See the [performance record](evidence/m2a14/performance/README.md).\n')
archives = []
for name in ('source-audit-corrected', 'layout-audit', 'debug-audit', 'historical-audit', 'log-audit', 'performance-audit', 'regressions-audit', 'release-audit', 'tails-audit'):
    original = Path(f'/tmp/alibi-m2a14-{name}-original.log')
    target = e / f'coordinator/{name}.log.gz'
    assert original.exists() and not target.exists(), original
    raw = original.read_bytes(); target.write_bytes(gzip.compress(raw, mtime=0))
    assert gzip.decompress(target.read_bytes()) == raw
    archives.append({'original_path': str(original), 'original_bytes': len(raw), 'original_sha256': sha(original), 'archive': target.name, 'archive_bytes': target.stat().st_size, 'archive_sha256': sha(target), 'normalization': 'none; exact original bytes'})
(e / 'coordinator/audit_log_archives.json').write_text(json.dumps(archives, indent=2) + '\n')
print(json.dumps({'result': 'accepted_component', 'workspace_passed': 2129, 'focused_passed': 193, 'source_files': 938, 'phase_samples': 7200}))
