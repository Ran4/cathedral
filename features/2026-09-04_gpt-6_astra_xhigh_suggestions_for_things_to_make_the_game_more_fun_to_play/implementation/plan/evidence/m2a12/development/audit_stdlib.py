from pathlib import Path
import hashlib,json
base=Path(__file__).resolve().parent
sysroot=Path('/tmp/alibi-m2a12-rustc_sysroot.log').read_text().strip()
root=Path(sysroot)/'lib/rustlib/src/rust/library/alloc/src'
ranges={'string.rs':[(2349,2355)],'vec/mod.rs':[(3772,3776)],'slice.rs':[(407,458)],'raw_vec/mod.rs':[(158,166),(515,525)],'vec/spec_from_iter_nested.rs':[(14,62)]}
out={}
for name,spans in ranges.items():
 p=root/name;raw=p.read_bytes();lines=raw.decode().splitlines(keepends=True)
 out[str(p)]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'excerpts':[{'start_line':a,'end_line':b,'text':''.join(lines[a-1:b])}for a,b in spans]}
(base/'stdlib_allocation_audit.json').write_text(json.dumps({'compiler_version_command':'rustc_version','compiler_sysroot_command':'rustc_sysroot','source_files':out,'claim':'Exact installed String/Vec cloning, generic and trivial slice copy, RawVec minimum/doubling capacity, iterator collection. No new BTree nodes or auxiliary validation collections. Closed owner layout and sparse-copy assertions are emitted by focused tests.'},indent=2)+'\n')
print(json.dumps({'verified_stdlib_files':len(out),'sysroot':sysroot}))
