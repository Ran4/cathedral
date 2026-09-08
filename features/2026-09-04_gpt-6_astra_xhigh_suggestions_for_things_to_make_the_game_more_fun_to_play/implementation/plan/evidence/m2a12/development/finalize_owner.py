from pathlib import Path
import hashlib,json,gzip,re,platform,sys
base=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(base.parent))
from component_input_sources import SOURCE_SCOPE,sources
sha=lambda raw:hashlib.sha256(raw).hexdigest()
manifest=json.loads((base/'source_hashes.json').read_text());assert manifest==sources()
commands=json.loads((base/'commands.json').read_text())
for c in commands:
 p=Path(c['source_manifest']);assert sha(p.read_bytes())==c['source_manifest_sha256']
 raw=Path(c['original_path']).read_bytes();assert sha(raw)==c['original_sha256'] and len(raw)==c['original_bytes']
 out=base/'development'/(c['name']+'.original.log.gz');out.write_bytes(gzip.compress(raw,mtime=0));assert gzip.decompress(out.read_bytes())==raw
 c.update({'archive':str(out.relative_to(Path.cwd())),'archive_bytes':out.stat().st_size,'archive_sha256':sha(out.read_bytes()),'decompressed_sha256':sha(gzip.decompress(out.read_bytes()))})
 c['test_results']=[dict(zip(['passed','failed','ignored','measured','filtered_out'],map(int,m)))for m in re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',raw.decode())]
(base/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
by={c['name']:c for c in commands};results={}
for name in ['checkpoint_frozen_final','public_final','workspace_serial']:
 c=by[name];assert c['exit_code']==0;assert json.loads(Path(c['source_manifest']).read_text())==manifest
 rows=c['test_results'];assert rows
 results[name]={'targets_with_test_results':len(rows),**{k:sum(x[k]for x in rows)for k in ['passed','failed','ignored']}}
smokes={}
for mode in ['authored','populated']:
 p=base/'development'/(mode+'_frozen_smoke.json');j=json.loads(p.read_text());assert all(len(j[k])==j['samples']for k in ['preflight_us','export_us','encode_us','decode_validate_us','candidate_validate_us','drop_us'])
 assert j['witnesses']['coarse_discard_diagnostics']==0 and 0<j['witnesses']['maximum_poll_step_seconds']<=0.05+1e-12
 assert j['counts']['characters']==(520 if mode=='authored' else 2520)
 assert j['counts']['floor']['awaiting']==1 and j['counts']['floor']['player_hold'] and j['counts']['ready_emitted'] and j['counts']['sound_ever_emitted']
 assert len(j['witnesses']['all_speech_messages'])==5 and len(j['witnesses']['tts_requests'])==4
 smokes[mode]={'file':str(p.relative_to(Path.cwd())),'sha256':sha(p.read_bytes()),'samples':j['samples'],'counts':j['counts'],'claim':'Functional debug smoke only; coordinator release measures frozen source'}
layout=re.findall(r'continuity_layout [^\n]*',Path(by['checkpoint_frozen_final']['original_path']).read_text());assert len(layout)==2
(base/'development/layout.json').write_text(json.dumps({'source_command':'checkpoint_frozen_final','manifest_sha256':sha((base/'source_hashes.json').read_bytes()),'lines':layout},indent=2)+'\n')
record={'status':'Owner verification passed; independent coordinator release acceptance pending','source_freeze':json.loads((base/'source_freeze.json').read_text()),'source_files_verified':len(manifest),'commands_verified':len(commands),'final_verification':results,'workspace':{'command_name':'workspace_serial',**results['workspace_serial']},'layout':{'command_name':'checkpoint_frozen_final','artifact':'development/layout.json','lines':layout},'smokes':smokes,'development_failures':[c['name']for c in commands if c['exit_code']!=0],'setup_failures':'development/setup_failures.md','full_m2_or_host_acceptance':False}
(base/'verification.json').write_text(json.dumps(record,indent=2)+'\n')
c=by['workspace_serial'];env={'explicit_cargo_environment':c['environment'],'build_variable_whitelist':c['build_variable_whitelist'],'absent_build_variables':{k:c['build_variable_whitelist'].get(k)for k in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','RUSTUP_TOOLCHAIN','CC','CXX','CARGO_BUILD_JOBS']},'platform':platform.platform(),'machine':platform.machine(),'compiler_command':'rustc_version','compiler_output':Path(by['rustc_version']['original_path']).read_text(),'source_scope':SOURCE_SCOPE,'source_helper_sha256':sha((base.parent/'component_input_sources.py').read_bytes()),'runner_sha256':sha((base/'development/run_checked.py').read_bytes()),'uv_cache':'/tmp/alibi-uv','gpu':'No GPU/window reprobe; CATHEDRAL_HEADLESS=1 and CATHEDRAL_FAKE_BACKEND=1 throughout. TTS service is values-only, no real audio.'}
(base/'environment.json').write_text(json.dumps(env,indent=2)+'\n')
print(json.dumps({'commands':len(commands),'source_files':len(manifest),'verification':results,'layout_lines':layout,'archives_verified':True}))
