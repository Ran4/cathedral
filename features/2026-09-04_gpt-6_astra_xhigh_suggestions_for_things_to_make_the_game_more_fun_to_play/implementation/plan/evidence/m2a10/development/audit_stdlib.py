from pathlib import Path
import hashlib,json
base=Path(__file__).parent
sysroot=Path('/tmp/alibi-m2a10-rustc_sysroot.log').read_text().strip()
root=Path(sysroot)/'lib/rustlib/src/rust/library/alloc/src'
selections={'collections/btree/map.rs':[(226,227),(303,308)],'collections/btree/node.rs':[(43,45)],'collections/btree/set.rs':[(116,123)],'string.rs':[(2349,2355)],'vec/mod.rs':[(3772,3776)],'slice.rs':[(444,457)],'raw_vec/mod.rs':[(158,173),(515,525)]}
result={'compiler_sysroot_command':'rustc_sysroot','compiler_version_command':'rustc_version','source_files':{}}
for relative,ranges in selections.items():
    p=root/relative;raw=p.read_bytes();lines=raw.decode().splitlines(keepends=True)
    result['source_files'][str(p)]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'excerpts':[{'start_line':a,'end_line':b,'text':''.join(lines[a-1:b])}for a,b in ranges]}
p=base/'stdlib_allocation_audit.json';p.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'output':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'files':len(result['source_files'])}))
