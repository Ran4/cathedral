import os,sys,subprocess,json,hashlib,gzip,time
from pathlib import Path
name=sys.argv[1]; cmd=sys.argv[2:]; base=Path('/tmp/alibi-m2a16-'+name)
assert not list(base.parent.glob(base.name+'.*')), 'refusing to overwrite attempt artifacts'
env=os.environ.copy();env.update(CARGO_HOME='/tmp/alibi-m1b-cargo',RUSTC='/home/ran/.cargo/bin/rustc',RUSTDOC='/home/ran/.cargo/bin/rustdoc',CATHEDRAL_HEADLESS='1',CATHEDRAL_FAKE_BACKEND='1')
import importlib.util
scope_path=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/component_input_sources.py')
spec=importlib.util.spec_from_file_location('scope',scope_path);scope=importlib.util.module_from_spec(spec);spec.loader.exec_module(scope)
rows=scope.sources()
meta={'command':cmd,'start':time.time(),'head':subprocess.check_output(['/usr/bin/git','rev-parse','HEAD'],text=True).strip(),'environment':{k:v for k,v in env.items() if k.startswith(('CARGO','RUST','CATHEDRAL','ALIBI','LD','CC','CXX','AR','PATH'))},'helpers':{'wrapper_path':str(Path(__file__).resolve()),'wrapper_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'source_enumerator_path':str(scope_path),'source_enumerator_sha256':hashlib.sha256(scope_path.read_bytes()).hexdigest()},'source_scope':scope.SOURCE_SCOPE,'source_map':rows,'unset_build_environment':[k for k in ('LDFLAGS','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_BUILD_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER') if k not in env]}
base.with_suffix('.start.json').write_text(json.dumps(meta,indent=2))
with base.with_suffix('.log').open('wb') as f:
 p=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
raw=base.with_suffix('.log').read_bytes();base.with_suffix('.log.gz').write_bytes(gzip.compress(raw,mtime=0))
after=scope.sources(); changed=[k for k in sorted(set(rows)|set(after)) if rows.get(k)!=after.get(k)]
base.with_suffix('.result.json').write_text(json.dumps({'exit':p.returncode,'end':time.time(),'log_sha256':hashlib.sha256(raw).hexdigest(),'source_unchanged':not changed,'changed_paths':changed,'source_after':after},indent=2))
if changed: print('SOURCE CHANGED DURING COMMAND:',changed)
print(raw.decode(errors='replace')[-12000:]);sys.exit(p.returncode)
