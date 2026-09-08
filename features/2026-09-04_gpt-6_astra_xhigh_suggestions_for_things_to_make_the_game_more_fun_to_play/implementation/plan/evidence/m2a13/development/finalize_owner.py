from pathlib import Path
import datetime,gzip,hashlib,json,re,sys
base=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(base.parent))
from component_input_sources import sources
sha=lambda b:hashlib.sha256(b).hexdigest()
freeze=json.loads((base/'source_freeze.json').read_text())
manifest=json.loads((base/'source_hashes.json').read_text())
assert sources()==manifest, 'final source changed'
commands=json.loads((base/'commands.json').read_text())
by_name={r['name']:r for r in commands}
assert len(by_name)==len(commands)
assert by_name['workspace_serial']['exit_code']==0, 'workspace not complete'
final_names=['checkpoint_final','public_final','authored_final_smoke','populated_final_smoke','workspace_serial','rustc_version','rustc_sysroot','stdlib_audit']
results={}
for r in commands:
    source_file=Path(r['source_manifest']);assert sha(source_file.read_bytes())==r['source_manifest_sha256']
    raw=Path(r['original_path']).read_bytes();assert sha(raw)==r['original_sha256'];assert len(raw)==r['original_bytes']
    archive=base/'development'/(r['name']+'.original.log.gz')
    compressed=gzip.compress(raw,mtime=0);archive.write_bytes(compressed);assert gzip.decompress(compressed)==raw
    r.update(archive=str(archive.relative_to(Path.cwd())),archive_bytes=len(compressed),archive_sha256=sha(compressed),decompressed_sha256=sha(gzip.decompress(compressed)))
    rows=[dict(zip(['passed','failed','ignored','measured','filtered_out'],map(int,m)))for m in re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',raw.decode())]
    r['test_results']=rows
    if r['name'] in final_names:
        assert r['exit_code']==0
        assert r['source_manifest_sha256']==freeze['source_manifest_sha256']
        assert json.loads(source_file.read_text())==manifest
        results[r['name']]={'targets_with_test_results':len(rows),**{k:sum(x[k]for x in rows)for k in ['passed','failed','ignored']}}
(base/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
smokes={}
for mode in ['authored','populated']:
    file=base/'development'/(mode+'_final_smoke.json');d=json.loads(file.read_text())
    for phase in ['preflight','export','encode','decode_validate','candidate_validate','drop']:
        assert len(d[phase+'_us'])==d['samples'] and all(v>=0 for v in d[phase+'_us'])
    assert d['witnesses']['coarse_discard_diagnostics']==0
    assert d['witnesses']['maximum_poll_step_seconds']<=.05
    assert d['counts']['characters']==(520 if mode=='authored' else 2520)
    if mode=='populated':assert d['placement']=={'requested':2000,'placed':2000,'unplaced':0}
    smokes[mode]={'file':str(file.relative_to(Path.cwd())),'sha256':sha(file.read_bytes()),'samples':d['samples'],'counts':d['counts'],'cost':d['cost'],'claim':'Functional debug smoke only; coordinator owns independent release measurement'}
old=json.loads((base/'development/authored_smoke.json').read_text());current=json.loads((base/'development/authored_final_smoke.json').read_text())
assert old['witnesses']==current['witnesses'] and old['boundary_speech']==current['boundary_speech']
(base/'development/probe_summary.json').write_text(json.dumps({'source_freeze':freeze,'smokes':smokes,'provisional_authored_semantic_equality':True},indent=2)+'\n')
verification={'status':'Owner verification complete; independent coordinator release review pending (2026-09-09). Full M2c/M3 pending.','source_freeze':freeze,'source_files_verified':len(manifest),'commands_verified':len(commands),'final_verification':{k:results[k]for k in ['checkpoint_final','public_final','workspace_serial']},'workspace':{'command_name':'workspace_serial',**results['workspace_serial']},'layout':{'command_name':'checkpoint_final','artifact':'development/layout.json'},'stdlib':'development/stdlib_allocation_audit.json','smokes':smokes,'development_failures':[r['name']for r in commands if r['exit_code']!=0],'setup_failures':'development/setup_failures.md','full_m2_or_host_acceptance':False}
(base/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')
status='Status: M2a13 owner implementation, frozen verification and evidence complete (2026-09-09); independent coordinator acceptance pending. Full M2c/M3 adoption remains pending.'
for name in ['README.md','ADMISSION.md','OWNER_COVERAGE.md']:
    p=base/name;s=p.read_text();s=re.sub(r'^Status:.*$',status,s,count=1,flags=re.M);assert len(re.findall(r'^Status:',s,re.M))==1
    replacements={
        'charge and4MiB':'charge and 4 MiB',
        'authored520/all+2000placed2520':'authored 520 actors and all 2,000 additions placed (2,520 total)',
        'ordinary <=50ms commands':'ordinary commands and poll steps <=50 ms',
        'had13passes/one failure':'had 13 passes/one failure',
        'had5passes/one failure':'had 5 passes/one failure',
        'coordinator owns the fixture correction':'the coordinator corrected the test notification boundary without relaxing production validation',
        'exceeds1GiB':'exceeds 1 GiB',
        'bound64':'bound of 64',
        'call no constructor, normal resolve_transcription/apply say, on_audio_abort, submit_batch, provider, service availability, poll or IO':'construct no Engine, World or SpeechRouter and never call normal resolve_transcription/apply say, on_audio_abort, submit_batch, provider, service availability, poll or IO',
        'J<=128MiB':'J <=128 MiB',
        'E<=128MiB':'E <=128 MiB',
        'depth<=64':'depth <=64',
        'RetiringGeneration<=1GiB':'RetiringGeneration <=1 GiB',
        'E charges512':'E charges 512',
        'array,64':'array, 64',
        'FourE':'Four E',
        'threeJ':'three J',
        'additional4MiB':'additional 4 MiB',
        'are256 roots and4352':'are 256 roots and 4,352',
        'OperationId16, CommandId24, ordinal8':'OperationId 16 B, CommandId 24 B, ordinal 8 B',
        'Adding65536':'Adding 65,536 B of',
        'below4MiB':'below 4 MiB',
        'most8 rows':'most 8 rows',
        'most2 affected':'most 2 affected',
        'independently8 captures,8 streams and8 combined':'independently 8 captures, 8 streams and 8 combined',
        'its512E':'its 512 B of E in the',
        'needs992B for four248B slots':'needs 992 B for four 248 B slots',
        'String24 doubled=48B':'String 24 B doubled =48 B',
        'below64E':'below 64 B of E',
        'against64/512B':'against 64/512 B',
        'AffectedRef48 doubled=96B':'AffectedRef 48 B doubled =96 B',
        '128Unicode scalars, at most512UTF8B':'128 Unicode scalars, at most 512 UTF-8 bytes',
        'admit65536UTF8B':'admit 65,536 UTF-8 bytes',
        'admits400000UTF8B':'admits 400,000 UTF-8 bytes',
        'PLAYER_SPEECH_MAX_CHARS500':'PLAYER_SPEECH_MAX_CHARS (500)',
        'is1256093444B, above1GiB':'is 1,256,093,444 B, above 1 GiB',
        'confirms520actors;3captures,2streams,1available85B text,3accepted rows (batch2/parked1),3semantic receipts/2roots':'confirms 520 actors; 3 captures, 2 streams, one available 85 B text, 3 accepted rows (batch 2/parked 1), 3 semantic receipts/2 roots',
        'J=1824B,E=31756B,per-cohort peak4330896B and combined Save+Load8661792B':'J =1,824 B, E =31,756 B, per-cohort peak 4,330,896 B and combined Save+Load 8,661,792 B',
        'All17ordinary polls use at most0.04s':'All 17 ordinary polls use at most 0.04 s',
        'Deterministic gzip archives and final source freeze are populated during final verification.':'Deterministic gzip archives and the final source freeze are complete and verified.',
    }
    for old,new in replacements.items():s=s.replace(old,new)
    p.write_text(s)
summary=results['workspace_serial'];checks=results['checkpoint_final'];
addition=f'''\n\nFinal frozen verification: {checks['passed']} focused passes ({checks['ignored']} intentional ignores), 6 public passes, and {summary['passed']} workspace passes with {summary['failed']} failures/{summary['ignored']} intentional ignores across {summary['targets_with_test_results']} result-bearing targets. All {len(commands)} command originals, source-at-start maps and deterministic mtime 0 gzip archives are verified in [commands.json](commands.json); [verification.json](verification.json) records exact totals and failure history. Source remains {len(manifest)} paths, SHA256 `{freeze['source_manifest_sha256']}`. Cargo was ceded to the coordinator immediately after workspace exit; no owner Cargo work remains.\n'''
p=base/'README.md';p.write_text(p.read_text()+addition)
p=base/'OWNER_COVERAGE.md';p.write_text(p.read_text()+addition)
p=base/'ADMISSION.md';p.write_text(p.read_text()+'''\n\nFinal emitted 64-bit layouts: AcceptedRecording 248 B, InterruptedStream 48 B, Receipt 120 B, StateV1 80 B; standalone DTO/wire/candidate 96 B, context 104 B, StateView 112 B and RecordingView 128 B. Engine DTO/wire/candidate 120 B, borrowed View 136 B and context 112 B. String 24 B and AffectedRef 48 B. [Layout evidence](development/layout.json) and [installed source audit](development/stdlib_allocation_audit.json) pin these claims to frozen source and the recorded rustc toolchain, including serde_json 1.0.150 and serde_core 1.0.228 parser/visitor excerpts.\n\nBoth frozen debug smokes preserve the same speech counts: 3 captures, 2 streams, one 85 B draft, 3 accepted rows (batch 2/parked 1), 3 receipts/2 roots. Authored 520 has J 1,824 B, a per-cohort peak of 4,330,896 B and combined Save+Load of 8,661,792 B. The populated case places all 2,000 additions (2,520 total) and has J 1,815 B, a per-cohort peak of 4,330,869 B and combined Save+Load of 8,661,738 B. Both have E 31,756 B, and both combined figures exclude Running. Each uses 17 ordinary polls, maximum step 0.04 s, zero coarse discard, 2 primary provider prompts and one voiced committed NPC line after the player's committed line. Exact primary inputs/outputs, available text, receipt rows and all timing samples remain in the frozen smoke JSON files. Provisional authored witnesses/boundary match exactly but their timings remain separate originals.\n''')
(base/'development/setup_failures.md').write_text('''# Retained M2a13 development failures — 2026-09-09\n\n- owner_initial: test harness named Admission::Fresh; existing enum is Admission::New. Compile failure retained; production check had already passed.\n- owner_fixed: 13 passes/one failure, test attempted admitted encode on a LoadCandidate lease; correct production refusal. The corrected canonical comparison uses serde_json only inside test code.\n- public_initial: 5 passes/one failure. Coordinator's direct ledger.advance setup left an unflushed receipt update. Production boundary refusal was correct; coordinator drains/asserts that update without another Engine poll. Public source correction and both maps are retained; production validation was not relaxed.\n\nNo failed original or provisional smoke is relabeled final. Generator creates only the four new component fixtures. Source map was frozen before final public/focused/debug/workspace commands. GPU unavailable evidence is inherited, never reprobed.\n''')
print(json.dumps({'commands':len(commands),'workspace':summary,'focused':checks,'source_sha256':freeze['source_manifest_sha256'],'all_originals_and_archives_verified':True}))
