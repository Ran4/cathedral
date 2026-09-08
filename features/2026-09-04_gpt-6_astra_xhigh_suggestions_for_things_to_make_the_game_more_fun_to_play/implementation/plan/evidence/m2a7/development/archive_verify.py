from pathlib import Path
import json,gzip,hashlib,re,shutil
base=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a7')
sha=lambda b:hashlib.sha256(b).hexdigest()
commands=json.loads((base/'commands.json').read_text())
for record in commands:
    raw=Path(record['original_path']).read_bytes()
    assert len(raw)==record['original_bytes'] and sha(raw)==record['original_sha256']
    archive=base/'development'/(record['name']+'.original.log.gz')
    archive.write_bytes(gzip.compress(raw,mtime=0))
    assert gzip.decompress(archive.read_bytes())==raw
    record.update(archive=str(archive),archive_bytes=archive.stat().st_size,archive_sha256=sha(archive.read_bytes()),decompressed_sha256=sha(raw))
    if 'source_manifest' in record:
        source=Path(record['source_manifest']);assert sha(source.read_bytes())==record['source_manifest_sha256']
    else:
        record['source_binding_note']='Early development command predates per-command source manifests; later frozen-source verification supersedes it.'
    record['test_results']=[dict(zip(['passed','failed','ignored','measured','filtered_out'],map(int,m))) for m in re.findall(rb'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out',raw)]
(base/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
def archive_format(record, hash_key):
    if record.get('log_encoding')=='gzip':
        archive=Path(record['log']);raw=gzip.decompress(archive.read_bytes())
        assert sha(raw)==record['original_sha256'] and sha(archive.read_bytes())==record['archive_sha256']
        return
    original=Path(record['log']);raw=original.read_bytes();assert sha(raw)==record[hash_key]
    retained=Path('/tmp')/('alibi-m2a7-'+original.stem+'.original.log');retained.write_bytes(raw)
    archive=original.with_name(original.stem+'.original.log.gz');archive.write_bytes(gzip.compress(raw,mtime=0));assert gzip.decompress(archive.read_bytes())==raw
    record.update(log=str(archive),log_encoding='gzip',original_path=str(retained),original_bytes=len(raw),original_sha256=sha(raw),archive=str(archive),archive_bytes=archive.stat().st_size,archive_sha256=sha(archive.read_bytes()),decompressed_sha256=sha(raw))
    record[hash_key]=sha(archive.read_bytes());original.unlink()
fmt=json.loads((base/'development/format.json').read_text());archive_format(fmt,'log_sha256');(base/'development/format.json').write_text(json.dumps(fmt,indent=2)+'\n')
scope=json.loads((base/'development/source_scope_audit.json').read_text())
for key in ['format_scoped','baseline_marks_format']: archive_format(scope[key],'sha256')
(base/'development/source_scope_audit.json').write_text(json.dumps(scope,indent=2)+'\n')
manifest=json.loads((base/'source_hashes.json').read_text())
for path,digest in manifest.items(): assert sha(Path(path).read_bytes())==digest,path
freeze=json.loads((base/'source_freeze.json').read_text())
assert sha((base/'source_hashes.json').read_bytes())==freeze['manifest_sha256']
smokes={}
for mode in ['authored','populated']:
    original=Path('/tmp/alibi-m2a7-'+mode+'-smoke.json');copy=base/'development'/(mode+'_smoke.json');shutil.copyfile(original,copy)
    data=json.loads(copy.read_text());assert all(len(data[p])==2 for p in ['preflight_us','export_us','encode_us','decode_validate_us','candidate_validate_us','drop_us'])
    smokes[mode]={'file':str(copy),'sha256':sha(copy.read_bytes()),'samples':2,'claim':'functional debug smoke only, not release measurements'}
environment=json.loads((base/'environment.json').read_text());compiler=environment['compiler']
assert sha(Path(compiler['archive']).read_bytes())==compiler['archive_sha256']
assert sha(gzip.decompress(Path(compiler['archive']).read_bytes()))==compiler['original_sha256']
workspace=next((r for r in commands if r['name']=='workspace_serial'),None)
assert workspace and workspace['exit_code']==0
assert workspace['source_manifest_sha256']==freeze['manifest_sha256']
sums={k:sum(t[k] for t in workspace['test_results']) for k in ['passed','failed','ignored']}
result={'status':'Owner verification passed; coordinator review/release acceptance pending','source_freeze':freeze,'source_files_verified':len(manifest),'commands_verified':len(commands),'workspace':{'command_name':'workspace_serial','targets_with_test_results':len(workspace['test_results']),**sums},'smokes':smokes,'format':'12 scoped Rust files pass; marks.rs unchanged test-comment alignment drift independently matches HEAD after module wiring is removed. Original failing check and baseline check retained.','development_failures':{'checkpoint_final':'New test referred to nonexistent LogicalTime::ZERO; corrected to LogicalTime::new(0.0).unwrap().','checkpoint_final_corrected':'Duplicate valid MarkId correctly refused; assertion expected duplicate map key instead of the adapter\'s duplicate owner key; corrected assertion, no production change.'},'lossless_logs':'All cargo stdout+stderr captured before summaries; original bytes and gzip mtime=0 archives have independently checked SHA-256. Three early development commands have no retrospective source binding; final workspace/focused/layout commands are source-pinned.'}
(base/'verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
