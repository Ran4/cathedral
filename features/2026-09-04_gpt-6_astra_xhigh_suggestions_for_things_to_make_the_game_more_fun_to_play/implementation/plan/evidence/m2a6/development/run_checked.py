from pathlib import Path
import subprocess, os, sys, time, json, hashlib, datetime
label,*args=sys.argv[1:]
base=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a6')
log=Path('/tmp/alibi-m2a6-'+label+'.log')
config={'PATH':'/usr/bin:/bin:/home/ran/.local/bin','CARGO_HOME':'/tmp/alibi-m1b-cargo','RUSTC':'/home/ran/.cargo/bin/rustc','RUSTDOC':'/home/ran/.cargo/bin/rustdoc','CATHEDRAL_HEADLESS':'1','CATHEDRAL_FAKE_BACKEND':'1'}
env=os.environ.copy();env.update(config)
command=['/home/ran/.cargo/bin/cargo',*args]
start=time.monotonic(); utc=datetime.datetime.now(datetime.timezone.utc).isoformat()
with log.open('wb') as out: completed=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT)
record={'name':label,'command':command,'environment':config,'started_utc':utc,'wall_seconds':time.monotonic()-start,'exit_code':completed.returncode,'original_path':str(log),'original_bytes':log.stat().st_size,'original_sha256':hashlib.sha256(log.read_bytes()).hexdigest()}
p=base/'commands.json'; records=json.loads(p.read_text()) if p.exists() else [];records.append(record);p.write_text(json.dumps(records,indent=2)+'\n')
print(json.dumps(record))
sys.exit(completed.returncode)
