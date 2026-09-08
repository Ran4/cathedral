from pathlib import Path
import json,gzip,hashlib,re
base=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a8')
sha=lambda b:hashlib.sha256(b).hexdigest()
commands=json.loads((base/'commands.json').read_text())
for record in commands:
    raw=Path(record['original_path']).read_bytes()
    assert len(raw)==record['original_bytes'] and sha(raw)==record['original_sha256']
    archive=base/'development'/(record['name']+'.original.log.gz')
    archive.write_bytes(gzip.compress(raw,mtime=0))
    assert archive.read_bytes()==gzip.compress(raw,mtime=0)
    assert gzip.decompress(archive.read_bytes())==raw
    record.update(archive=str(archive),archive_bytes=archive.stat().st_size,archive_sha256=sha(archive.read_bytes()),decompressed_sha256=sha(raw))
    source=Path(record['source_manifest']);assert sha(source.read_bytes())==record['source_manifest_sha256']
    record['test_results']=[dict(zip(['passed','failed','ignored','measured','filtered_out'],map(int,m))) for m in re.findall(rb'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out',raw)]
(base/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
manifest=json.loads((base/'source_hashes.json').read_text())
for path,digest in manifest.items():assert sha(Path(path).read_bytes())==digest,path
freeze=json.loads((base/'source_freeze.json').read_text())
assert sha((base/'source_hashes.json').read_bytes())==freeze['manifest_sha256']
final={}
for name in ['checkpoint_final','layout','workspace_serial']:
    record=next(r for r in commands if r['name']==name)
    assert record['exit_code']==0 and record['source_manifest_sha256']==freeze['manifest_sha256'],name
    final[name]={'targets_with_test_results':len(record['test_results']),**{k:sum(t[k] for t in record['test_results']) for k in ['passed','failed','ignored']}}
smokes={}
for mode in ['authored','populated']:
    copy=base/'development'/(mode+'_smoke.json');data=json.loads(copy.read_text())
    assert all(len(data[p])==2 for p in ['preflight_us','export_us','encode_us','decode_validate_us','candidate_validate_us','drop_us'])
    smokes[mode]={'file':str(copy),'sha256':sha(copy.read_bytes()),'samples':2,'claim':'functional debug smoke only, preceding final scoped whitespace/test additions; coordinator release measures final frozen source'}
environment=json.loads((base/'environment.json').read_text());environment['compiler']=next(r for r in commands if r['name']=='rustc_version');(base/'environment.json').write_text(json.dumps(environment,indent=2)+'\n')
result={'status':'Owner verification passed; coordinator review/release acceptance pending','source_freeze':freeze,'source_files_verified':len(manifest),'commands_verified':len(commands),'final_verification':final,'workspace':{'command_name':'workspace_serial',**final['workspace_serial']},'smokes':smokes,'format':'Eight new Rust files scoped formatted; ordinary source equals HEAD after only module wiring removal. Coordinator independently checks all ten scoped Rust files.','development_failures':[r['name'] for r in commands if r['exit_code']!=0],'lossless_logs':'Every command stdout+stderr captured before summaries; deterministic gzip mtime=0, original/archive/decompressed SHA-256 verified. Every command has a source manifest from START; final focused/layout/workspace share one unchanged manifest.'}
(base/'verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
