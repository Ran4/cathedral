from pathlib import Path
import json,hashlib
base=Path(__file__).resolve().parent
sysroot=Path('/tmp/alibi-m2a11-rustc_sysroot.log').read_text().strip()
prior=json.loads((base.parents[1]/'m2a10/development/stdlib_allocation_audit.json').read_text())
files=prior['source_files']
for path,meta in files.items():
    raw=Path(path).read_bytes()
    assert str(path).startswith(sysroot+'/')
    assert hashlib.sha256(raw).hexdigest()==meta['sha256']
    assert len(raw)==meta['bytes']
    lines=raw.decode().splitlines(keepends=True)
    for part in meta['excerpts']:
        assert ''.join(lines[part['start_line']-1:part['end_line']])==part['text']
alloc=Path(sysroot)/'lib/rustlib/src/rust/library/alloc/src'
for rel,ranges in [('collections/btree/node.rs',[(43,67),(98,113)]),('collections/vec_deque/mod.rs',[(119,128)]),('raw_vec/mod.rs',[(417,429),(650,668)])]:
    path=alloc/rel;raw=path.read_bytes();lines=raw.decode().splitlines(keepends=True)
    entry=files.setdefault(str(path),{'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'excerpts':[]})
    for start,end in ranges:
        entry['excerpts'].append({'start_line':start,'end_line':end,'text':''.join(lines[start-1:end])})
record={'compiler_sysroot_command':'rustc_sysroot','compiler_version_command':'rustc_version','source_files':files,'claim':'Pinned stdlib source, direct node layouts, populated-length cloning and capacity growth; actual owner layout appears in final focused scheduler_layout output.'}
(base/'stdlib_allocation_audit.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'source_files_verified':len(files),'sysroot':sysroot,'result':'pass'}))
