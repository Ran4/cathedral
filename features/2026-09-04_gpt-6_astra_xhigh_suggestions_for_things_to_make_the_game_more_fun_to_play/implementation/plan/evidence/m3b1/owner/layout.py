"""Read final test ELF layout without running the program or changing source."""
import hashlib, json, re, subprocess, sys
from pathlib import Path
HERE = Path(__file__).resolve().parent
ROOT = Path.cwd()
def sha(path):
    digest=hashlib.sha256()
    with path.open('rb') as source:
        while block:=source.read(1024*1024): digest.update(block)
    return digest.hexdigest()
run_name=sys.argv[1]
log=Path('/tmp/alibi-m3b1-'+run_name+'.log').read_text()
images=re.findall(r'Running unittests src/lib\.rs \((target/debug/deps/cathedral_backends-[a-f0-9]+)\)',log)
assert len(images)==1,images
image=ROOT/images[0]
before=sha(image)
predecessor=Path('/tmp/alibi-m3a-final-release-image')
predecessor_sha=sha(predecessor)
assert predecessor_sha=='52f7f4f408a5286194b509d477bf324d9ec44571c8bfdd2bf9dbeaa193bf2246'
assert predecessor.stat().st_size==26730496
names=['Core','Queue','PrepState','PreparedDelivery','DeliveryDisposal','RetiredPayload','Job','PreparationPermit','RetirementPermit']
command=['/usr/bin/gdb','-nx','-nh','--batch',str(image),'-ex','set language rust']
for name in names: command+=['-ex','p sizeof(cathedral_backends::checkpoint_preparation::'+name+')']
result=subprocess.run(command,text=True,capture_output=True)
print(result.stdout,end='')
print(result.stderr,end='')
assert result.returncode==0,result.returncode
sizes=[int(n) for n in re.findall(r'^\$\d+ = (\d+)$',result.stdout,re.M)]
assert len(sizes)==len(names),(sizes,result.stdout)
assert sha(image)==before,'ELF changed during read-only layout query'
metadata={'path':str(image),'bytes':image.stat().st_size,'sha256':before,'layout':dict(zip(names,sizes)),
    'query_command':command,'layout_helper_sha256':sha(Path(__file__)),'target_executed':False,
    'binary_copy_preserved':False,'preserved_predecessor':{'path':str(predecessor),'bytes':predecessor.stat().st_size,'sha256':predecessor_sha},'source_map_sha256':json.loads((HERE/(run_name+'-start.json')).read_text())['source_map_sha256']}
(HERE/'final-images.json').write_text(json.dumps(metadata,sort_keys=True,indent=2)+'\n')
print(json.dumps(metadata,sort_keys=True))
