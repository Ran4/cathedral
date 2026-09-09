from pathlib import Path
import gzip,hashlib,json,re,sys
base=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(base.parent))
from component_input_sources import sources
sha=lambda b:hashlib.sha256(b).hexdigest()
freeze=json.loads((base/'source_freeze.json').read_text())
manifest=json.loads((base/'source_hashes.json').read_text())
assert sources()==manifest
commands=json.loads((base/'commands.json').read_text())
by_name={r['name']:r for r in commands}
assert len(commands)==len(by_name)
assert by_name['workspace_serial']['exit_code']==0
final_names=['public_initial','checkpoint_final','probes_build','verify_smokes','rustc_version','workspace_serial']+[f'{lane}_{mode}_{kind}'for lane in ['scheduler','night']for mode in ['authored','populated']for kind in ['inputs','default']]
totals={}
for r in commands:
    source=Path(r['source_manifest']).read_bytes();assert sha(source)==r['source_manifest_sha256']
    raw=Path(r['original_path']).read_bytes();assert len(raw)==r['original_bytes'] and sha(raw)==r['original_sha256']
    archive=base/'development'/(r['name']+'.original.log.gz')
    compressed=gzip.compress(raw,mtime=0);archive.write_bytes(compressed);assert gzip.decompress(compressed)==raw
    r.update(archive=str(archive.relative_to(Path.cwd())),archive_bytes=len(compressed),archive_sha256=sha(compressed),decompressed_sha256=sha(raw))
    rows=[dict(zip(['passed','failed','ignored','measured','filtered_out'],map(int,m)))for m in re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',raw.decode())]
    r['test_results']=rows
    if r['name']in final_names:
        assert r['exit_code']==0 and r['source_manifest_sha256']==freeze['source_manifest_sha256'] and json.loads(source)==manifest
        totals[r['name']]={'command_name':r['name'],'targets_with_test_results':len(rows),**{k:sum(x[k]for x in rows)for k in ['passed','failed','ignored']}}
(base/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
prior=base.parent/'m2a13/development/stdlib_allocation_audit.json'
proof=json.loads(prior.read_text())
for path,row in proof['source_files'].items():
    p=Path(path);assert not p.is_relative_to('/home/ran/w')
    raw=p.read_bytes();assert len(raw)==row['bytes'] and sha(raw)==row['sha256']
old_commands=json.loads((base.parent/'m2a13/commands.json').read_text())
old_rustc=next(r for r in old_commands if r['name']=='rustc_version')
assert by_name['rustc_version']['original_sha256']==old_rustc['original_sha256']
log=Path(by_name['checkpoint_final']['original_path']).read_text()
layouts=[line for line in log.splitlines()if line.startswith(('cognition_inputs_layout ','cognition_inputs_night_layout ','scheduler_layout '))]
assert len(layouts)==4
allocation={'source_manifest_sha256':freeze['source_manifest_sha256'],'current_layout_command':'checkpoint_final','current_layout_lines':layouts,'reused_proof':str(prior),'reused_proof_sha256':sha(prior.read_bytes()),'source_files_current_hashes_match':{p:r['sha256']for p,r in proof['source_files'].items()},'rustc_version_command':'rustc_version','rustc_bytes_match_prior_command':True,'old_owner_growth_bytes':{'scheduler_flight':8,'night_flight':8},'working_bytes':4194304,'proof':'Existing borrowed ledger/lane/Night scratch runs sequentially; only two fixed rows and bounded prompt/identity copies are new. Current old V1 Flight/owner layout assertions pass under unchanged record charges.'}
(base/'development/allocation_evidence.json').write_text(json.dumps(allocation,indent=2)+'\n')
probe=json.loads((base/'development/probe_summary.json').read_text())
for r in probe['rows']:
    for key in ['historical','default','inputs']:assert sha(Path(r[key+'_path']).read_bytes())==r[key+'_sha256']
verification={'status':'M2a14 owner verification complete; independent coordinator acceptance pending (2026-09-09).','source_freeze':freeze,'commands_verified':len(commands),'final_focused':totals['checkpoint_final'],'final_public':totals['public_initial'],'public_final_basis':'The successful public_initial command already used exactly the final source-at-start map. Coordinator explicitly accepted it as final and prohibited a redundant rerun. Its original name and log remain unchanged.','workspace':totals['workspace_serial'],'smokes':'development/probe_summary.json','allocation':'development/allocation_evidence.json','development_failures':[r['name']for r in commands if r['exit_code']!=0],'complete_save_or_host_acceptance':False,'cargo_handed_to_root_immediately_after_workspace':True}
(base/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')
(base/'development/setup_failures.md').write_text('''# Retained M2a14 development failures — 2026-09-09

- `generate_fixture`: coordinator public harness imported private `world::checkpoint` instead of public `world::WorldBackboneDtoV1`. Compilation stopped before generating a fixture. Root corrected that import and also corrected its raw-input reservation harness; production validation was unchanged.
- `owner_fixture_compile`: three historical test-only Night Flight literals needed an explicit new field. They now use MissingLegacy because no actual provider submission created those fixture flights.
- `owner_initial`: six new Engine/sidecar tests passed, four owner lifecycle checks used stale 1,200/700 expectations from prose. Actual source/provider inputs were 2,400/1,400. Only those expectations changed; all ten private additions pass in the final focused suite.

All failed originals, command-start input maps and deterministic gzip archives remain present. No failed command was relabeled successful or normalized. A generated fixture is new; all historical fixture bytes remain unchanged. GPU unavailability is inherited and was not reprobed.
''')
status='Status: Owner implementation and frozen verification complete (2026-09-09); independent coordinator acceptance pending. Full M2c/M3 adoption remains pending.'
for name in ['OWNER_COVERAGE.md','ADMISSION.md']:
    p=base/name;s=p.read_text();s=re.sub(r'^Status:.*$',status,s,count=1,flags=re.M);p.write_text(s)
w=totals['workspace_serial'];f=totals['checkpoint_final'];p=totals['public_initial']
(base/'README.md').write_text(f'''# M2a14 exact cognition inputs — 2026-09-09

{status}

Both scheduler and Night flights now retain their exact successful-submission output budget. A strict separately admitted sidecar binds it to the original prompt, request method and existing semantic/subject/flight identity. Historical V1 components explicitly lack this new authority and keep their exact wire bytes; missing legacy input is never an intentional provider-default None.

- [Owner coverage](OWNER_COVERAGE.md) describes creation, Busy, retry, held, legacy and cleanup ownership.
- [Admission](ADMISSION.md) records fixed row costs, old inline growth and sequential borrowed validation under unchanged caps.
- [Verification](verification.json) records {f['passed']} focused passes, {p['passed']} independent public passes and {w['passed']} workspace passes, with zero final failures. Workspace has {w['ignored']} intentional ignores across {w['targets_with_test_results']} result-bearing targets.
- [Debug probes](development/probe_summary.json) verify four opt-in accepted-input workloads and four historical-default metadata comparisons on 520/2,520 actors. Timings are functional debug evidence, not release acceptance.
- [Allocation evidence](development/allocation_evidence.json) reuses eleven byte-identical installed allocator/parser sources and identical rustc 1.96.0 version output with current concrete layout assertions.
- [Command originals](commands.json) preserve all {len(commands)} command records and exact mtime-0 gzip archives, including [development failures](development/setup_failures.md).

Final scope is {len(manifest)} source/content inputs, SHA256 `{freeze['source_manifest_sha256']}`. Public verification already used this exact map before the explicit freeze record; root accepted that run as final without a redundant rerun. Sources remained unchanged throughout final focused/probe/workspace checks. Cargo was ceded to root immediately after workspace completion, before this owner evidence was finalized. Root owns independent release acceptance and the commit.

Complete M2c retry/adoption, full Engine/World/host assembly and publication, manifest/root/horizon agreement and actual Running/Save/Load/retiring lifetimes remain pending. No full-save, external-provider or host-frame claim is made; the earlier naive complete-composition peak excess remains unresolved without increasing caps.
''')
addition=f'\nFinal frozen verification passed: {f["passed"]} focused checks, {p["passed"]} independent public boundaries and {w["passed"]} workspace checks. See [verification.json](verification.json), exact [command originals](commands.json) and the [debug witness comparisons](development/probe_summary.json). All new private checks and historical fixtures pass.\n'
path=base/'OWNER_COVERAGE.md';path.write_text(path.read_text()+addition)
table='\n\nMeasured fixed layouts on rustc 1.96.0 x86_64: sidecar DTO/candidate 248 B, scheduler row 96 B, Night row 112 B, borrowing-only context 48 B and AcceptedOutputBudget 8 B. Existing scheduler Flight is now 144 B and NpcScheduler 384 B (V1 DTO 400 B, Engine V1 DTO 440 B); Night Flight is 104 B and NightOffice 312 B. Each live owner grows by 8 B. The original fixed record charges exceed these layouts; no old V1 allowance increased. The emitted scheduler scratch bound is 2,580,480 B for conservative borrowed-ledger sets, then separately 400,000 B lane pointers plus 4,112 B roots. Night validation runs after these allocations drop. [Current evidence](development/allocation_evidence.json) also verifies all eleven prior installed allocator/parser source hashes and matching compiler version output.\n\n| Debug workload | Actors | J bytes | E bytes | Per-cohort peak | Save + Load excluding Running |\n|---|---:|---:|---:|---:|---:|\n'
for r in probe['rows']:
    c=r['cost'];actors=520 if r['mode']=='authored'else 2520
    table+=f'| {r["lane"]} {r["mode"]} | {actors:,} | {c["encoded_bytes"]:,} | {c["expanded_upper_bytes"]:,} | {c["peak_bytes"]:,} | {2*c["peak_bytes"]:,} |\n'
table+='\nAll exact primary submitted inputs and saved rows remain in the debug JSONs. Scheduler uses 136 ordinary polls with observed maximum 0.040000000000000036 s; Night uses 405 nominal-50 ms polls with observed floating-point delta 0.05000000000000071 s (within 1e-12 s of 50 ms). Both report zero discarded physical work, and both populated variants place all 2,000 additions. Existing default-mode scenario/count/witness/mode/placement values exactly match historical accepted metadata. No debug timing is promoted to release or host-frame acceptance.\n'
path=base/'ADMISSION.md';path.write_text(path.read_text()+table)
print(json.dumps({'commands':len(commands),'source_files':len(manifest),'focused':f,'public':p,'workspace':w,'all_originals_archives_and_reused_source_hashes_verified':True}))
