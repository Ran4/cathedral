import os,sys,subprocess,json,hashlib,gzip,time
from pathlib import Path
name=sys.argv[1]; cmd=sys.argv[2:]; base=Path('/tmp/alibi-m2a16-'+name)
env=os.environ.copy();env.update(CARGO_HOME='/tmp/alibi-m1b-cargo',RUSTC='/home/ran/.cargo/bin/rustc',RUSTDOC='/home/ran/.cargo/bin/rustdoc')
paths=subprocess.check_output(['/usr/bin/git','ls-files','-co','--exclude-standard'],text=True).splitlines()
rows=[]
for p in sorted(set(paths)):
 if p.startswith(('docs/codex_gdd/','gauntlet/','reference/')) or p=='features/2026_09_14_more_ambient_stuff.md': continue
 if Path(p).is_file(): rows.append((p,hashlib.sha256(Path(p).read_bytes()).hexdigest()))
meta={'command':cmd,'start':time.time(),'head':subprocess.check_output(['/usr/bin/git','rev-parse','HEAD'],text=True).strip(),'environment':{k:v for k,v in env.items() if k.startswith(('CARGO','RUST','CATHEDRAL','LD','CC','CXX','AR','PATH'))},'source_map':rows}
base.with_suffix('.start.json').write_text(json.dumps(meta,indent=2))
with base.with_suffix('.log').open('wb') as f:
 p=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
raw=base.with_suffix('.log').read_bytes();base.with_suffix('.log.gz').write_bytes(gzip.compress(raw,mtime=0))
base.with_suffix('.result.json').write_text(json.dumps({'exit':p.returncode,'end':time.time(),'log_sha256':hashlib.sha256(raw).hexdigest()}))
print(raw.decode(errors='replace')[-24000:]);sys.exit(p.returncode)
