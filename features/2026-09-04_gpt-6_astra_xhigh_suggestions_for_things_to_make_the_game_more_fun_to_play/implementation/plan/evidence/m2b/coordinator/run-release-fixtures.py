"""Fresh-process writers/readers of exact-image hydration fixtures."""
from pathlib import Path
import json
import sys
from release_common import LEG, OUT, ROOT, archive, capture, frozen_sources, sha, write_json

attempt = int(sys.argv[1]) if len(sys.argv) == 2 else 1
assert attempt > 0 and len(sys.argv) <= 2
build = json.loads((OUT / 'release_build.json').read_bytes())
debug = json.loads((OUT / 'debug-reference.json').read_bytes())
release_binary = Path(build['reference_binary'])
debug_binary = Path(debug['binary'])
assert sha(release_binary) == build['reference_binary_sha256']
assert sha(debug_binary) == debug['sha256']
assert sha(LEG / 'source_hashes.json') == build['source_manifest_sha256'] == debug['source_manifest_sha256']
frozen_sources()
destination = LEG / f'fixtures-attempt-{attempt}'
assert not destination.exists()
destination.mkdir()
lineage = b'M2b-fixture-0001'
assert len(lineage) == 16
records = []
outputs = {}

for state in ('initial', 'active'):
    hashes = []
    for repetition in (1, 2, 3):
        label = f'fixtures-{attempt}-{state}-write-{repetition}'
        output = Path(f'/tmp/alibi-m2b-{label}.json')
        report = Path(f'/tmp/alibi-m2b-{label}.report.json')
        assert not output.exists() and not report.exists()
        env = {'ALIBI_COMPLETE_WORLD_ID': lineage.hex(),
               'ALIBI_HYDRATION_FIXTURE': str(output), 'ALIBI_HYDRATION_REPORT': str(report)}
        if state == 'initial':
            env['ALIBI_HYDRATION_INITIAL'] = '1'
        record = capture(label, [str(release_binary), '--ignored', '--exact',
            'host_checkpoint::tests_hydration_owner::m2b_hydration_fixture_write',
            '--nocapture', '--test-threads=1'], destination, env, release_binary)
        record |= {'state': state, 'operation': 'write', 'repetition': repetition,
                   'fixture': archive(output, destination / f'{state}-write-{repetition}.json.gz'),
                   'metrics': archive(report, destination / f'{state}-write-{repetition}.report.json.gz')}
        records.append(record)
        write_json(destination / 'RESULTS.json', records)
        data = json.loads(report.read_bytes())
        assert data['schema'] == 'm2b-hydration-fixture-v1' and data['writer'] is True
        assert data['active'] is (state == 'active')
        assert bytes(data['lineage']) == lineage
        assert bytes(data['host_image']).hex() == sha(release_binary)
        assert bytes(data['raw_sha256']).hex() == sha(output)
        assert data['actual_owner_categories'] == 16 and len(data['category_sha256']) == 16
        assert data['source_app_disposed_before_fresh_resolver'] is True
        assert data['factory_plus_retained_shared_assets_upper_bytes'] <= 64 * 1024**2
        hashes.append(sha(output))
        outputs.setdefault(state, output)
    assert len(set(hashes)) == 1, f'{state} writer bytes changed between fresh processes'
    for kind, binary in (('compatible', release_binary), ('incompatible', debug_binary)):
        label = f'fixtures-{attempt}-{state}-read-{kind}'
        report = Path(f'/tmp/alibi-m2b-{label}.report.json')
        assert not report.exists()
        env = {'ALIBI_HYDRATION_FIXTURE': str(outputs[state]), 'ALIBI_HYDRATION_REPORT': str(report)}
        if state == 'initial':
            env['ALIBI_HYDRATION_INITIAL'] = '1'
        if kind == 'incompatible':
            env['ALIBI_HYDRATION_EXPECT_INCOMPATIBLE'] = '1'
        record = capture(label, [str(binary), '--ignored', '--exact',
            'host_checkpoint::tests_hydration_owner::m2b_hydration_fixture_read',
            '--nocapture', '--test-threads=1'], destination, env, binary)
        record |= {'state': state, 'operation': 'read', 'compatibility': kind,
                   'input': str(outputs[state]), 'input_sha256': sha(outputs[state])}
        if kind == 'compatible':
            record['metrics'] = archive(report, destination / f'{state}-read-compatible.report.json.gz')
            data = json.loads(report.read_bytes())
            assert data['writer'] is False and data['actual_owner_categories'] == 16
            assert bytes(data['raw_sha256']).hex() == hashes[0]
            assert data['source_app_disposed_before_fresh_resolver'] is True
        else:
            assert b'hydration fixture rejected before construction:' in Path(record['log']['original']).read_bytes()
        records.append(record)
        write_json(destination / 'RESULTS.json', records)

write_json(OUT / 'fixture-outputs.json', {
    'attempt': attempt, 'results': str((destination / 'RESULTS.json').relative_to(ROOT)),
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'release_binary_sha256': sha(release_binary), 'debug_binary_sha256': sha(debug_binary),
    'lineage_hex': lineage.hex(),
    'outputs': {state: {'original': str(path), 'bytes': path.stat().st_size, 'sha256': sha(path)}
                for state, path in outputs.items()},
})
