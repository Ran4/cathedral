from pathlib import Path
import json, hashlib, datetime
root = Path('/home/ran/src/rust/cathedralbevy')
e = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a13'
sha = lambda b: hashlib.sha256(b).hexdigest()
proof_file = e/'development/stdlib_allocation_audit.json'
proof = json.loads(proof_file.read_bytes())
records = json.loads((e/'commands.json').read_bytes())
sysroot_cmd = next(r for r in records if r['name'] == proof['compiler_sysroot_command'])
version_cmd = next(r for r in records if r['name'] == proof['compiler_version_command'])
for record in [sysroot_cmd,version_cmd]:
    assert record['exit_code'] == 0
    assert sha(Path(record['original_path']).read_bytes()) == record['original_sha256']
sysroot = Path(Path(sysroot_cmd['original_path']).read_text().strip()).resolve()
assert sysroot.is_relative_to('/home/ran/.rustup/toolchains')
checks = []
registry = Path('/tmp/alibi-m1b-cargo/registry/src/index.crates.io-1949cf8c6b5b557f').resolve()
allowed_parser_crates = {'serde_json-1.0.150', 'serde_core-1.0.228'}
lock = (root/'Cargo.lock').read_text()
for crate in allowed_parser_crates:
    name, version = crate.rsplit('-', 1)
    assert f'name = "{name}"\nversion = "{version}"' in lock
for name, expected in proof['source_files'].items():
    path = Path(name).resolve()
    assert path.is_relative_to(sysroot) or (
        path.is_relative_to(registry) and path.relative_to(registry).parts[0] in allowed_parser_crates)
    raw = path.read_bytes()
    assert len(raw) == expected['bytes'] and sha(raw) == expected['sha256']
    lines = raw.decode().splitlines(keepends=True)
    for excerpt in expected['excerpts']:
        assert ''.join(lines[excerpt['start_line']-1:excerpt['end_line']]) == excerpt['text']
    checks.append({'path':name,'sha256':sha(raw),'verified_excerpts':len(expected['excerpts'])})
report = {'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'owner_proof_sha256':sha(proof_file.read_bytes()),'compiler_sysroot':str(sysroot),'compiler_version':Path(version_cmd['original_path']).read_text(),'compiler_command_records':[r['name'] for r in [sysroot_cmd,version_cmd]],'checks':checks}
(e/'coordinator').mkdir(exist_ok=True)
path = e/'coordinator/stdlib_allocation_audit.json'
assert not path.exists()
path.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'result':'passed','source_files':len(checks),'excerpts':sum(c['verified_excerpts'] for c in checks)}))
