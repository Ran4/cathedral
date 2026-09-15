"""Independently reconcile every owner command with its original output."""
import gzip
import json
import re
from pathlib import Path
from release_common import LEG, OUT, ROOT, sha, write_json

owner = LEG / 'owner'
frozen = json.loads((LEG / 'source_hashes.json').read_bytes())
helpers = {sha(path): str(path.relative_to(ROOT)) for path in owner.glob('run*.py')}
rows = []
for start_path in sorted(owner.glob('*-start.json')):
    name = start_path.name.removesuffix('-start.json')
    result_path = owner / f'{name}-result.json'
    assert result_path.is_file(), f'command still incomplete: {name}'
    start = json.loads(start_path.read_bytes())
    result = json.loads(result_path.read_bytes())
    source_path = owner / f'{name}-sources.json'
    assert sha(source_path) == start['source_map_sha256'] == result['source_map_sha256']
    sources = json.loads(source_path.read_bytes())
    helper = helpers[start['helper_sha256']]
    raw = Path(start['raw_log'])
    assert str(raw) == result['raw_log'] and raw.parent == Path('/tmp')
    assert raw.name.startswith('alibi-m2b-')
    archive = owner / f'{name}.log.gz'
    assert sha(raw) == result['raw_sha256']
    assert sha(archive) == result['archive_sha256']
    content = raw.read_bytes()
    assert gzip.decompress(archive.read_bytes()) == content
    assert start['cwd'] == str(ROOT)
    env = start['environment']
    assert env['CATHEDRAL_HEADLESS'] == env['CATHEDRAL_FAKE_BACKEND'] == '1'
    assert env['CARGO_HOME'] == '/tmp/alibi-m1b-cargo'
    assert all(env[key] is None for key in (
        'LDFLAGS', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS',
        'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'))
    groups = [tuple(map(int, match)) for match in re.findall(
        rb'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', content)]
    totals = [sum(group[index] for group in groups) for index in range(3)]
    rows.append({
        'name': name, 'command': start['command'], 'exit_code': result['exit_code'],
        'source_map_sha256': sha(source_path), 'source_count': len(sources),
        'matches_final_source': sources == frozen,
        'sources_unchanged_during_command': result['sources_unchanged'],
        'helper': helper, 'helper_sha256': start['helper_sha256'],
        'raw_sha256': sha(raw), 'archive_sha256': sha(archive),
        'gzip_mtime': int.from_bytes(archive.read_bytes()[4:8], 'little'),
        'test_groups': len(groups), 'passed': totals[0], 'failed': totals[1],
        'ignored': totals[2],
    })
assert rows
final_workspace = [row for row in rows if '--workspace' in row['command']
                   and 'test' in row['command'] and row['matches_final_source']
                   and row['sources_unchanged_during_command']
                   and row['exit_code'] == 0 and row['failed'] == 0 and row['passed'] > 0]
assert final_workspace, 'no passing frozen-source full workspace command'
write_json(OUT / 'owner-command-audit.json', {
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'commands': rows, 'final_workspace': final_workspace[-1]['name'],
    'all_originals_and_archives_verified': True,
})
print(json.dumps({'commands': len(rows), 'final_workspace': final_workspace[-1]}))
