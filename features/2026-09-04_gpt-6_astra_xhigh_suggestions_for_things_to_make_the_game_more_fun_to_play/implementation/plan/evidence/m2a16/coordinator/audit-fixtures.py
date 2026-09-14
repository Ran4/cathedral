"""Independently check all fresh writers and exact-image fixture readers."""
import importlib.util
import json
from pathlib import Path
from release_common import ROOT, LEG, OUT, SOURCE_SCOPE, frozen_sources, sha, write_json

spec = importlib.util.spec_from_file_location('probe_audit', OUT / 'audit-release-probes.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)
source = frozen_sources()
collected = probe.read_json((OUT / 'fixtures-collected.json').read_bytes())
dataset = ROOT / collected['dataset']
assert collected['repository_copy_pending'] is True
assert sha(dataset / 'IDENTITY.json') == collected['identity_sha256']
assert sha(dataset / 'RESULTS.json') == collected['results_sha256']
identity = probe.read_json((dataset / 'IDENTITY.json').read_bytes())
records = probe.read_json((dataset / 'RESULTS.json').read_bytes())
assert identity['source_scope'] == SOURCE_SCOPE and identity['source_hashes'] == source
assert identity['source_manifest_sha256'] == collected['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
assert identity['repository_fixture_outputs_created'] is False
release, debug = Path(identity['release']), Path(identity['debug'])
assert sha(release) == identity['release_sha256'] == collected['release_sha256']
assert sha(debug) == identity['debug_sha256'] == collected['debug_sha256']
assert identity['release_sha256'] != identity['debug_sha256']
assert len(records) == 10
seen, payloads, report_rows = set(), {}, []
for record in records:
    state = record['state']
    assert state in ('initial', 'active')
    writer = 'repetition' in record
    kind = f"writer-{record['repetition']}" if writer else record['verification'] + '-validator'
    assert (state, kind) not in seen
    seen.add((state, kind))
    label = f'{dataset.name}-{state}-{kind}'
    start = probe.read_json((dataset / f'{label}.start.json').read_bytes())
    result = probe.read_json((dataset / f'{label}.result.json').read_bytes())
    assert all(result[key] == value for key, value in start.items())
    assert all(record[key] == value for key, value in result.items())
    assert record['exit_code'] == 0
    assert all(record[key] is True for key in ('unchanged_source', 'unchanged_helpers', 'unchanged_binary'))
    assert record['source_count'] == len(source) and record['source_scope'] == SOURCE_SCOPE
    assert record['source_manifest_sha256'] == collected['source_manifest_sha256']
    for path, digest in record['helper_sha256'].items():
        assert sha(ROOT / path) == digest
    environment = record['environment_overrides']
    assert environment['CATHEDRAL_HEADLESS'] == environment['CATHEDRAL_FAKE_BACKEND'] == '1'
    binary = release if writer or record['verification'] == 'same-image' else debug
    assert record['binary'] == str(binary) and record['binary_sha256'] == sha(binary)
    test_name = 'm2a16_complete_probe' if writer else 'm2a16_verify_complete_fixture'
    assert record['command'] == [str(binary), '--ignored', '--exact',
        'host_checkpoint::tests_complete_owner::' + test_name, '--nocapture', '--test-threads=1']
    log = probe.check_archive(record['log']).decode()
    probe.check_archive(record['time'])
    assert 'test result: ok. 1 passed; 0 failed;' in log
    row = {'state': state, 'kind': kind, 'binary_sha256': sha(binary),
           'result_sha256': sha(dataset / f'{label}.result.json')}
    if writer:
        assert record['repetition'] in (1, 2, 3)
        assert environment['ALIBI_COMPLETE_WORLD_ID'] == identity['initial_world_identity_hex']
        assert environment.get('ALIBI_COMPLETE_INITIAL') == ('1' if state == 'initial' else None)
        raw = probe.check_archive(record['payload'])
        data = probe.read_json(raw)
        assert data['version'] == 1 and data['profile'] == 'authored'
        assert bytes(data['world_identity']).hex() == identity['initial_world_identity_hex']
        assert bytes(data['manifest']['host_image']).hex() == sha(release)
        metrics = probe.read_json(probe.check_archive(record['metrics']))
        assert metrics['active'] is (state == 'active') and metrics['samples'] == 1
        assert metrics['characters'] == 520 and metrics['costs'][0]['encoded_bytes'] == len(raw)
        assert metrics['ordinary_boundary_unchanged'] is True
        assert metrics['shared_peak_bytes'] <= 1024**3
        if state in payloads:
            assert raw == payloads[state]
        else:
            payloads[state] = raw
        row |= {'bytes': len(raw), 'sha256': record['payload']['original_sha256']}
    else:
        assert record['verification'] in ('same-image', 'incompatible-image')
        assert environment['ALIBI_COMPLETE_FIXTURE_IN'] == collected['chosen_originals'][state]
        incompatible = record['verification'] == 'incompatible-image'
        assert environment.get('ALIBI_COMPLETE_EXPECT_IMAGE_MISMATCH') == ('1' if incompatible else None)
        marker = 'EXPECTED_INCOMPATIBLE_HOST_IMAGE' if incompatible else 'SAME_IMAGE_EXACT_BYTES_ACCEPTED'
        assert marker in log
        row['marker'] = marker
    report_rows.append(row)
assert seen == {(state, kind) for state in ('initial', 'active') for kind in
                ('writer-1', 'writer-2', 'writer-3', 'same-image-validator', 'incompatible-image-validator')}
assert payloads['initial'] != payloads['active']
for state, raw in payloads.items():
    assert raw == Path(collected['chosen_originals'][state]).read_bytes()
assert frozen_sources() == source
destination = OUT / 'fixture-audit.json'
assert not destination.exists()
write_json(destination, {'result': 'passed', 'source_manifest_sha256': collected['source_manifest_sha256'],
    'helper_sha256': sha(Path(__file__)), 'archive_auditor_sha256': sha(OUT / 'audit-release-probes.py'),
    'records': report_rows, 'repository_copy_pending': True})
print(json.dumps({'result': 'passed', 'fresh_writers': 6, 'exact_readers': 2,
                  'incompatible_readers': 2, 'payload_bytes': {key: len(raw) for key, raw in payloads.items()}}))
