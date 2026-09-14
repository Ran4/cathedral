import os,sys,subprocess,json,hashlib,gzip,time
from pathlib import Path
name=sys.argv[1]; cmd=sys.argv[2:]; base=Path('/tmp/alibi-m2a16-'+name)
env=os.environ.copy();env.update(CARGO_HOME='/tmp/alibi-m1b-cargo',RUSTC='/home/ran/.cargo/bin/rustc',RUSTDOC='/home/ran/.cargo/bin/rustdoc',CATHEDRAL_HEADLESS='1',CATHEDRAL_FAKE_BACKEND='1')
import importlib.util
scope_path=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/component_input_sources.py')
spec=importlib.util.spec_from_file_location('scope',scope_path);scope=importlib.util.module_from_spec(spec);spec.loader.exec_module(scope)
rows=scope.sources()
meta={'command':cmd,'start':time.time(),'head':subprocess.check_output(['/usr/bin/git','rev-parse','HEAD'],text=True).strip(),'environment':{k:v for k,v in env.items() if k.startswith(('CARGO','RUST','CATHEDRAL','LD','CC','CXX','AR','PATH'))},'source_scope':scope.SOURCE_SCOPE,'source_map':rows,'unset_build_environment':[k for k in ('LDFLAGS','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_BUILD_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER') if k not in env]}
base.with_suffix('.start.json').write_text(json.dumps(meta,indent=2))
with base.with_suffix('.log').open('wb') as f:
 p=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
raw=base.with_suffix('.log').read_bytes();base.with_suffix('.log.gz').write_bytes(gzip.compress(raw,mtime=0))
base.with_suffix('.result.json').write_text(json.dumps({'exit':p.returncode,'end':time.time(),'log_sha256':hashlib.sha256(raw).hexdigest()}))
print(raw.decode(errors='replace')[-12000:]);sys.exit(p.returncode)
