"""One-owner command runner. Preserve start identity, raw bytes and exact archive."""
import datetime, gzip, hashlib, importlib.util, json, os, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parents[1]
spec = importlib.util.spec_from_file_location('sources', EVIDENCE/'component_input_sources.py')
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
name, *command = sys.argv[1:]
if not command or (HERE/(name+'-start.json')).exists(): raise SystemExit('missing command or reused run name')
def write(path, value): path.write_text(json.dumps(value, sort_keys=True, indent=2)+'\n')
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
removed = ['LDFLAGS','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_BUILD_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER']
env = dict(os.environ)
inherited = {key:env.get(key) for key in removed}
for key in removed: env.pop(key, None)
env.update(PATH='/usr/bin:/bin:/home/ran/.local/bin:/home/ran/.cargo/bin', CARGO_HOME='/tmp/alibi-m1b-cargo',RUSTC='/home/ran/.cargo/bin/rustc',RUSTDOC='/home/ran/.cargo/bin/rustdoc',CATHEDRAL_HEADLESS='1',CATHEDRAL_FAKE_BACKEND='1',PYTHONDONTWRITEBYTECODE='1')
source_path=HERE/(name+'-sources.json'); write(source_path,module.sources())
raw=Path('/tmp')/('alibi-m3b2b3-'+name+'.log')
start={'command':command,'cwd':str(module.ROOT),'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'inherited_removed_environment':inherited,'environment':{key:env.get(key) for key in [*removed,'PATH','CARGO_HOME','RUSTC','RUSTDOC','CATHEDRAL_HEADLESS','CATHEDRAL_FAKE_BACKEND','PYTHONDONTWRITEBYTECODE']},'source_map_sha256':sha(source_path),'helper_sha256':sha(Path(__file__)),'enumerator_sha256':sha(EVIDENCE/'component_input_sources.py'),'inherited_target_environment':{k:os.environ.get(k) for k in ['CARGO_TARGET_DIR','CARGO_BUILD_TARGET']},'raw_log':str(raw)}
write(HERE/(name+'-start.json'),start)
t=time.monotonic()
with raw.open('xb') as output: result=subprocess.run(command,cwd=module.ROOT,env=env,stdout=output,stderr=subprocess.STDOUT)
archive=HERE/(name+'.log.gz')
with raw.open('rb') as source, archive.open('wb') as archive_file, gzip.GzipFile(filename='', mode='wb',fileobj=archive_file,mtime=0) as target:
 while chunk:=source.read(1024*1024): target.write(chunk)
write(HERE/(name+'-result.json'),{'exit_code':result.returncode,'wall_seconds':time.monotonic()-t,'raw_log':str(raw),'raw_sha256':sha(raw),'archive_sha256':sha(archive),'source_map_sha256':sha(source_path),'sources_unchanged':module.sources()==json.loads(source_path.read_text())})
print(json.dumps({'name':name,'exit_code':result.returncode,'raw_log':str(raw)}),flush=True)
raise SystemExit(result.returncode)
