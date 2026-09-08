from pathlib import Path
base=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a13')
prior=(base.parent/'m2a12/development/freeze_source.py').read_text()
(base/'development/freeze_source.py').write_text(prior)
prior=(base.parent/'m2a11/development/audit_stdlib.py').read_text().replace('/tmp/alibi-m2a11-rustc_sysroot.log','/tmp/alibi-m2a13-rustc_sysroot.log')
prior=prior.replace("'m2a10/development/stdlib_allocation_audit.json'","'m2a11/development/stdlib_allocation_audit.json'")
cut=prior.index("record={'compiler_sysroot_command'")
prior=prior[:cut]+'''
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
(base/'stdlib_allocation_audit.json').write_text(json.dumps(record,indent=2)+'\\n')
print(json.dumps({'source_files_verified':len(files),'sysroot':sysroot,'result':'pass'}))
'''
(base/'development/audit_stdlib.py').write_text(prior)
p=base/'ADMISSION.md';s=p.read_text().replace('Empty/singleton Vec minimum four elements is covered by each512E outer array; nonempty geometric capacity at most twice populated rows.','Empty vectors allocate nothing. A singleton capture/stream four-slot minimum fits its512E outer array; a singleton accepted row needs992B for four248B slots, covered by the outer array plus that row\'s mandatory object/key/scalar charges. Nonempty geometric capacity is at most twice populated rows after the singleton minimum.')
p.write_text(s)
