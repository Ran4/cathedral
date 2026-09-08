from pathlib import Path
import subprocess, os, sys, time, json, hashlib, datetime
label,*args=sys.argv[1:]
if any(x['name']==label for x in json.loads((Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a8')/'commands.json').read_text())) if (Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a8')/'commands.json').exists() else False: raise SystemExit('duplicate command label')
base=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a8')
log=Path('/tmp/alibi-m2a8-'+label+'.log')
config={'PATH':'/usr/bin:/bin:/home/ran/.local/bin','CARGO_HOME':'/tmp/alibi-m1b-cargo','RUSTC':'/home/ran/.cargo/bin/rustc','RUSTDOC':'/home/ran/.cargo/bin/rustdoc','CATHEDRAL_HEADLESS':'1','CATHEDRAL_FAKE_BACKEND':'1'}
env=os.environ.copy();env.update(config)
command=args[1:] if args[0]=='--tool' else ['/home/ran/.cargo/bin/cargo',*args]
build_variables={k:v for k,v in env.items() if k in {'PATH','CC','CXX','AR','LD','CFLAGS','CXXFLAGS','LDFLAGS','CATHEDRAL_HEADLESS','CATHEDRAL_FAKE_BACKEND'} or k.startswith(('CARGO_','RUST','PKG_CONFIG'))}
paths=[]
for root in ['crates','src','assets','lore']:
    paths.extend(p for p in Path(root).rglob('*') if p.is_file() and p.suffix in {'.rs','.json','.bin','.ron','.toml','.j2'})
paths.extend(Path(p) for p in ['Cargo.toml','Cargo.lock','build.rs'] if Path(p).exists())
manifest={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(paths))}
source_file=base/'development'/(label+'.source.json')
source_file.write_text(json.dumps(manifest,indent=2)+'\n')
start=time.monotonic(); utc=datetime.datetime.now(datetime.timezone.utc).isoformat()
with log.open('wb') as out: completed=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT)
record={'source_manifest':str(source_file),'source_manifest_sha256':hashlib.sha256(source_file.read_bytes()).hexdigest(),'name':label,'command':command,'environment':config,'build_variable_whitelist':build_variables,'started_utc':utc,'wall_seconds':time.monotonic()-start,'exit_code':completed.returncode,'original_path':str(log),'original_bytes':log.stat().st_size,'original_sha256':hashlib.sha256(log.read_bytes()).hexdigest()}
p=base/'commands.json'; records=json.loads(p.read_text()) if p.exists() else [];records.append(record);p.write_text(json.dumps(records,indent=2)+'\n')
print(json.dumps(record))
sys.exit(completed.returncode)
