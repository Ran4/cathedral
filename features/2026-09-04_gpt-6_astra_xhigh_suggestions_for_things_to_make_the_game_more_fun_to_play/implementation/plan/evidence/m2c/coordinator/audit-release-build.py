"""Check the preserved executable against original build output and archives."""
import gzip
import json
from pathlib import Path
from release_common import LEG, OUT, ROOT, frozen_sources, sha, write_json

frozen_sources()
build = json.loads((OUT / 'release_build.json').read_bytes())
assert build['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
rows = []
for path in sorted(OUT.glob('release-build-*.start.json')):
    label = path.name.removesuffix('.start.json')
    start = json.loads(path.read_bytes())
    result = json.loads((OUT / f'{label}.result.json').read_bytes())
    assert all(result[key] == value for key, value in start.items())
    assert all(result[key] is True for key in ('unchanged_source', 'unchanged_helpers', 'unchanged_binary'))
    assert result['source_manifest_sha256'] == build['source_manifest_sha256']
    for name, digest in result['helper_sha256'].items():
        assert sha(ROOT / name) == digest
    for name in ('log', 'time'):
        record = result[name]
        original, archive = Path(record['original']), ROOT / record['archive']
        assert sha(original) == record['original_sha256']
        assert sha(archive) == record['archive_sha256']
        raw = original.read_bytes()
        assert len(raw) == record['original_bytes']
        assert gzip.decompress(archive.read_bytes()) == raw
        assert int.from_bytes(archive.read_bytes()[4:8], 'little') == 0
    rows.append({'command': start['command'], 'exit_code': result['exit_code'],
                 'result_sha256': sha(OUT / f'{label}.result.json'),
                 'raw_sha256': result['log']['original_sha256']})
assert rows and rows[-1]['exit_code'] == 0
reference = Path(build['reference_binary'])
assert sha(reference) == build['reference_binary_sha256']
assert reference.stat().st_size == build['binary_bytes']
messages = []
for line in Path(build['log']['original']).read_text().splitlines():
    try:
        messages.append(json.loads(line))
    except json.JSONDecodeError:
        pass
assert any(row.get('reason') == 'build-finished' and row.get('success') is True for row in messages)
executables = {row['executable'] for row in messages
               if row.get('reason') == 'compiler-artifact'
               and row.get('target', {}).get('name') == 'cathedralbevy'
               and row.get('profile', {}).get('test') and row.get('executable')}
assert executables == {build['built_binary']}
assert sha(Path(build['built_binary'])) == sha(reference)
write_json(OUT / 'release-build-audit.json', {
    'result': 'passed', 'source_manifest_sha256': build['source_manifest_sha256'],
    'release_binary_sha256': sha(reference),
    'commands': rows, 'helper_sha256': sha(Path(__file__)),
})
print(json.dumps({'result': 'passed', 'build_commands': len(rows),
                  'release_binary_sha256': sha(reference)}))
