from pathlib import Path
import hashlib,importlib.util,json,shutil,time
BASE=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a16')
ARCHIVE=BASE/'host-component-fixture-refresh'
ARCHIVE.mkdir()
source_path=BASE.parent/'component_input_sources.py'
spec=importlib.util.spec_from_file_location('scope',source_path); scope=importlib.util.module_from_spec(spec);spec.loader.exec_module(scope)
sha=lambda b:hashlib.sha256(b).hexdigest()
initial=scope.sources()
assert initial==json.loads((BASE/'pre-repair-10-source_hashes.json').read_text())

def differences(a,b,path=''):
    if type(a)!=type(b):return [{'path':path,'before':a,'after':b}]
    if isinstance(a,dict):
        assert a.keys()==b.keys(), (path,'key shape changed')
        return sum([differences(a[k],b[k],path+'/'+k) for k in sorted(a)],[])
    if isinstance(a,list):
        return [] if a==b else [{'path':path,'before':a,'after':b}]
    return [] if a==b else [{'path':path,'before':a,'after':b}]

rows=[]
fixture_dir=Path('crates/cathedral-sim/tests/fixtures/checkpoint_host')
for name in ('initial-v1.json','active-v1.json'):
    dest=fixture_dir/name;before=dest.read_bytes()
    writers=[Path('/tmp/alibi-m2a16-host-fixtures-'+str(i))/name for i in (11,12,13)]
    outputs=[p.read_bytes() for p in writers]
    assert outputs[0]==outputs[1]==outputs[2],name
    diff=differences(json.loads(before),json.loads(outputs[0]))
    assert [d['path'] for d in diff]==['/scalars/definitions/installed_catalogs'], diff
    (ARCHIVE/('prior-'+name)).write_bytes(before)
    (ARCHIVE/('current-'+name)).write_bytes(outputs[0])
    rows.append({'fixture':str(dest),'prior_sha256':sha(before),'prior_bytes':len(before),'current_sha256':sha(outputs[0]),'current_bytes':len(outputs[0]),'writers':[str(p) for p in writers],'three_fresh_processes_identical':True,'json_differences':diff})
for row in rows:
    dest=Path(row['fixture']);shutil.copyfile(ARCHIVE/('current-'+dest.name),dest)
final=scope.sources();changed=[k for k in sorted(set(initial)|set(final)) if initial.get(k)!=final.get(k)]
assert changed==sorted(row['fixture'] for row in rows),changed
for i in (11,12,13):
    name='host-fixtures-'+str(i)
    result=json.loads(Path('/tmp/alibi-m2a16-'+name+'.result.json').read_text())
    assert result['exit']==0 and result['source_unchanged']
    for ext in ('start.json','result.json','log.gz'):
        dst=BASE/'owner-commands'/(name+'.'+ext);assert not dst.exists()
        shutil.copyfile('/tmp/alibi-m2a16-'+name+'.'+ext,dst)
script=Path(__file__).read_bytes();(ARCHIVE/'refresh.py').write_bytes(script)
record={'schema':1,'helper_sha256':sha(script),'source_enumerator_sha256':sha(source_path.read_bytes()),'time_unix':time.time(),'accepted_predecessor':'e7147719ab59629a2088bca587f7e5d47c13b342','cause':'Existing strict host installed_catalogs hashes full src/city/mod.rs bytes. Its sole change is appended cfg(test) read-only capacity observer; all other installed_catalogs source inputs are unchanged. No old identity normalization; existing writer regenerated both exact current fixtures.','binary':json.loads((BASE/'debug-reference-binary.json').read_text()),'fixtures':rows,'source_delta':changed,'rust_sources_changed':False}
(ARCHIVE/'record.json').write_text(json.dumps(record,indent=2)+'\n')
(BASE/'source_hashes.json').write_text(json.dumps(final,indent=2)+'\n')
freeze={'source_scope':scope.SOURCE_SCOPE,'files':len(final),'source_manifest_sha256':sha((BASE/'source_hashes.json').read_bytes()),'frozen_at_unix':time.time(),'state':'pre-release complete-fixture freeze; current legacy host component fixtures regenerated from unchanged production/test image','source_enumerator_sha256':sha(source_path.read_bytes()),'prior_manifest':'pre-repair-10-source_hashes.json','delta_record':'host-component-fixture-refresh/record.json'}
(BASE/'source-freeze.json').write_text(json.dumps(freeze,indent=2)+'\n')
print(json.dumps(freeze));print(json.dumps(rows))
