from pathlib import Path
import json,hashlib
base=Path(__file__).resolve().parent
sysroot=Path('/tmp/alibi-m2a13-rustc_sysroot.log').read_text().strip()
prior=json.loads((base.parents[1]/'m2a11/development/stdlib_allocation_audit.json').read_text())
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

# Bind the exact installed parser/visitor implementations used by Cargo.lock.
registry=Path('/tmp/alibi-m1b-cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
for name,ranges in {
 'serde_json-1.0.150/src/read.rs':[(494,545),(872,920)],
 'serde_json-1.0.150/src/de.rs':[(1524,1566)],
 'serde_core-1.0.228/src/de/impls.rs':[(580,615),(682,704)],
}.items():
 p=registry/name;raw=p.read_bytes();lines=raw.decode().splitlines(keepends=True)
 files[str(p)]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'excerpts':[{'start_line':a,'end_line':b,'text':''.join(lines[a-1:b])}for a,b in ranges]}
record={'compiler_sysroot_command':'rustc_sysroot','compiler_version_command':'rustc_version','source_files':files,'claim':'Installed BTree node layout/occupancy; populated-length String/Vec copies and RawVec growth; serde_json borrowed/escaped string scratch and serde String visitor. Concrete speech layouts in final focused output. Saved ledger validation uses borrowed existing records, never candidate construction.'}
(base/'stdlib_allocation_audit.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'source_files_verified':len(files),'sysroot':sysroot,'result':'pass'}))
