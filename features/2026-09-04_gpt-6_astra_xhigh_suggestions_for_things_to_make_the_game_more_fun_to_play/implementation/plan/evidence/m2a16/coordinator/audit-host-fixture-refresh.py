"""Check the exact historical/current host fixture delta and retained format proof."""
import json
from pathlib import Path
import subprocess
from release_common import ROOT, LEG, OUT, frozen_sources, sha, write_json

source = frozen_sources()
prior = json.loads((LEG / 'pre-repair-10-source_hashes.json').read_bytes())
directory = LEG / 'host-component-fixture-refresh'
record = json.loads((directory / 'record.json').read_bytes())
assert sha(directory / 'refresh.py') == record['helper_sha256']
changed = sorted(path for path in set(source) | set(prior) if source.get(path) != prior.get(path))
assert changed == record['source_delta'] == sorted(row['fixture'] for row in record['fixtures'])
assert record['rust_sources_changed'] is False
assert all(source[path] == digest for path, digest in prior.items() if path.endswith('.rs'))
checked = []
for row in record['fixtures']:
    path = ROOT / row['fixture']
    old, current = directory / ('prior-' + path.name), directory / ('current-' + path.name)
    assert sha(old) == row['prior_sha256'] == prior[row['fixture']]
    assert sha(current) == row['current_sha256'] == source[row['fixture']] == sha(path)
    assert old.stat().st_size == row['prior_bytes'] and current.stat().st_size == row['current_bytes']
    predecessor = subprocess.check_output(['/usr/bin/git', 'show', record['accepted_predecessor'] + ':' + row['fixture']], cwd=ROOT)
    assert predecessor == old.read_bytes()
    before, after = json.loads(old.read_bytes()), json.loads(current.read_bytes())
    old_identity = before['scalars']['definitions'].pop('installed_catalogs')
    new_identity = after['scalars']['definitions'].pop('installed_catalogs')
    assert before == after and old_identity != new_identity
    assert row['json_differences'] == [{'path': '/scalars/definitions/installed_catalogs',
                                      'before': old_identity, 'after': new_identity}]
    assert len(row['writers']) == 3 and row['three_fresh_processes_identical'] is True
    for writer in row['writers']:
        assert Path(writer).read_bytes() == current.read_bytes() == path.read_bytes()
    checked.append({'fixture': row['fixture'], 'old_sha256': sha(old), 'new_sha256': sha(path),
                    'only_changed_json_path': '/scalars/definitions/installed_catalogs'})
format_proof = json.loads((OUT / 'format-source-audit.json').read_bytes())
assert format_proof['result'] == 'passed'
assert format_proof['source_manifest_sha256'] == sha(LEG / 'pre-repair-10-source_hashes.json')
assert all(source[path] == prior[path] for path in format_proof['formatted_files'])
assert frozen_sources() == source
destination = OUT / 'host-fixture-refresh-audit.json'
assert not destination.exists()
write_json(destination, {'result': 'passed', 'helper_sha256': sha(Path(__file__)),
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'prior_source_manifest_sha256': sha(LEG / 'pre-repair-10-source_hashes.json'),
    'fixture_delta': checked, 'rust_source_unchanged': True,
    'unchanged_formatted_files': len(format_proof['formatted_files']),
    'scope': 'Exact prior commit bytes and three current writers; original formatting remains valid because every Rust input is unchanged. Writer command provenance is independently covered by owner command audit.'})
print(json.dumps({'result': 'passed', 'fixtures': len(checked),
                  'unchanged_formatted_files': len(format_proof['formatted_files'])}))
