"""Independently check persisted host payloads and their actual writer/loader provenance."""
from pathlib import Path
from collections import Counter
import datetime
import gzip
import json
from release_common import ROOT, LEG, OUT, frozen_sources, sha, write_json

frozen = frozen_sources()
metadata_path = LEG / 'owner_fixtures/fixture-equality-1.json'
metadata = json.loads(metadata_path.read_bytes())
assert metadata['comparison'] == 'exact bytes, no normalization'
assert metadata['writers'] == [f'fixture-writer-{n}' for n in range(1, 4)]
commands = []
for label in [*metadata['writers'], 'fixture-loader-1']:
    start_path = LEG / f'commands/{label}.start.json'
    result_path = LEG / f'commands/{label}.result.json'
    start = json.loads(start_path.read_bytes())
    result = json.loads(result_path.read_bytes())
    raw = Path(start['raw_log'])
    archive = LEG / f'commands/{label}.log.gz'
    assert result['exit_code'] == 0 and result['source_changed_during_command'] is False
    assert sha(raw) == result['raw_sha256']
    assert gzip.decompress(archive.read_bytes()) == raw.read_bytes()
    assert archive.read_bytes()[4:8] == bytes(4)
    assert b'test result: ok. 1 passed; 0 failed;' in raw.read_bytes()
    assert start['environment']['CATHEDRAL_HEADLESS'] == '1'
    assert start['environment']['CATHEDRAL_FAKE_BACKEND'] == '1'
    if label != 'fixture-writer-2':
        assert '/usr/bin:/bin:/home/ran/.local/bin:/home/ran/.cargo/bin' in start['environment']['PATH']
    if label in ('fixture-writer-2', 'fixture-writer-3', 'fixture-loader-1'):
        assert start['source_sha256']['src/host_checkpoint/tests.rs'] == frozen['src/host_checkpoint/tests.rs']
    if label == 'fixture-loader-1':
        assert start['source_sha256'] == frozen
        assert 'host_checkpoint::tests::persisted_initial_and_active_host_fixtures_decode_at_actual_compatible_boundaries' in start['command']
    else:
        assert 'host_checkpoint::tests::m2a15_write_component_fixtures' in start['command']
        assert '--ignored' in start['command'] and '--exact' in start['command']
    commands.append({'label': label, 'start_sha256': sha(start_path),
                     'result_sha256': sha(result_path), 'raw_sha256': sha(raw),
                     'archive_sha256': sha(archive), 'environment': start['environment']})

fixtures = {}
for name, item in metadata['fixtures'].items():
    persisted = ROOT / item['persisted']
    raw = persisted.read_bytes()
    assert len(raw) == item['bytes'] and sha(persisted) == item['raw_sha256']
    assert frozen[item['persisted']] == sha(persisted)
    archive = ROOT / item['archive']
    assert sha(archive) == item['archive_sha256']
    assert gzip.decompress(archive.read_bytes()) == raw and archive.read_bytes()[4:8] == bytes(4)
    assert item['originals'] == [f'/tmp/alibi-m2a15-fixture-writer-{n}/{name}' for n in range(1, 4)]
    for original in item['originals']:
        assert Path(original).read_bytes() == raw
    dto = json.loads(raw)
    assert set(dto) == {'version', 'scalars', 'records'} and dto['version'] == 1
    families = dict(Counter(record['kind'] for record in dto['records']))
    assert families == item['families']
    assert dto['scalars']['boundary']['generation'] == item['generation']
    assert dto['scalars']['time']['accepted']['elapsed'] == item['elapsed']
    fixtures[name] = {'bytes': len(raw), 'sha256': sha(persisted), 'rows': len(dto['records']),
                      'families': families, 'three_processes_exact': True,
                      'persisted_and_archive_exact': True}
assert frozen_sources() == frozen
destination = OUT / 'fixture-audit.json'
assert not destination.exists()
write_json(destination, {
    'result': 'passed', 'audited_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'helper_sha256': sha(Path(__file__)), 'metadata_sha256': sha(metadata_path),
    'scope': 'Component fixture bytes and successful actual-boundary load/candidate test; not whole-world restoration or admission.',
    'commands': commands, 'fixtures': fixtures,
})
print(json.dumps({'result': 'passed', 'fixtures': fixtures}))
