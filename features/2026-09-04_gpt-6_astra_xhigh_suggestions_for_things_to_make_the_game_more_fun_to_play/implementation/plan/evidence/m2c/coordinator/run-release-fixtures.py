"""Fresh-process exact-image writers/readers for prepared continuation V2."""
from pathlib import Path
import json
import sys
from release_common import LEG, OUT, ROOT, archive, capture, frozen_sources, sha, write_json

attempt = int(sys.argv[1]) if len(sys.argv) == 2 else 1
assert attempt > 0 and len(sys.argv) <= 2
build = json.loads((OUT / 'release_build.json').read_bytes())
binary = Path(build['reference_binary'])
assert build['exit_code'] == 0 and build['unchanged_source'] and build['unchanged_helpers']
assert sha(binary) == build['reference_binary_sha256']
assert sha(LEG / 'source_hashes.json') == build['source_manifest_sha256']
frozen_sources()
destination = LEG / f'fixtures-attempt-{attempt}'
assert not destination.exists()
destination.mkdir()
lineage = b'M2c-fixture-0001'
assert len(lineage) == 16
records, outputs = [], {}
command = [str(binary), '--ignored', '--exact',
           'host_checkpoint::tests_continuation_owner::m2c_continuation_probe',
           '--nocapture', '--test-threads=1']


def check_report(data, mode, reader):
    assert data['schema'] == 'm2c-continuation-v1' and data['profile'] == mode
    assert data['fixture_reader'] is reader and data['samples'] == 1
    assert data['characters'] == (520 if mode == 'authored' else 2520)
    assert data['requested'] == data['placed'] == (0 if mode == 'authored' else 2000)
    assert data['unplaced'] == 0
    assert bytes(data['host_image']).hex() == sha(binary)
    assert data['capture_boundary_unchanged'] and data['second_preparation_exact_bytes']
    assert data['unchanged_categories_equal'] and data['unchanged_category_count'] == 8
    assert bytes(data['fixture_sha256']) == bytes(data['resave_sha256'])
    assert len(data['input_sha256']) == 1
    if reader:
        assert bytes(data['input_sha256'][0]) == bytes(data['fixture_sha256'])
    return bytes(data['fixture_sha256']).hex()


for mode in ('authored', 'populated'):
    hashes = []
    for repetition in (1, 2, 3):
        label = f'fixtures-{attempt}-{mode}-write-{repetition}'
        output = Path(f'/tmp/alibi-m2c-{label}.json')
        report = Path(f'/tmp/alibi-m2c-{label}.report.json')
        assert not output.exists() and not report.exists()
        record = capture(label, command, destination, {
            'ALIBI_COMPLETE_WORLD_ID': lineage.hex(),
            'ALIBI_CONTINUATION_MODE': mode, 'ALIBI_CONTINUATION_SAMPLES': '1',
            'ALIBI_CONTINUATION_FIXTURE': str(output),
            'ALIBI_CONTINUATION_REPORT': str(report),
        }, binary)
        record |= {'mode': mode, 'operation': 'write', 'repetition': repetition,
                   'fixture': archive(output, destination / f'{mode}-write-{repetition}.json.gz'),
                   'metrics': archive(report, destination / f'{mode}-write-{repetition}.report.json.gz')}
        records.append(record)
        write_json(destination / 'RESULTS.json', records)
        data = json.loads(report.read_bytes())
        assert check_report(data, mode, False) == sha(output)
        assert data['immediate_v2_resave_bytes'] == output.stat().st_size
        hashes.append(sha(output))
        outputs.setdefault(mode, output)
    assert len(set(hashes)) == 1, f'{mode} writer bytes differ across fresh processes'
    label = f'fixtures-{attempt}-{mode}-read'
    report = Path(f'/tmp/alibi-m2c-{label}.report.json')
    assert not report.exists()
    record = capture(label, command, destination, {
        'ALIBI_CONTINUATION_MODE': mode, 'ALIBI_CONTINUATION_SAMPLES': '1',
        'ALIBI_CONTINUATION_FIXTURE': str(outputs[mode]),
        'ALIBI_CONTINUATION_READ_FIXTURE': '1', 'ALIBI_CONTINUATION_REPORT': str(report),
    }, binary)
    record |= {'mode': mode, 'operation': 'read',
               'input': str(outputs[mode]), 'input_sha256': sha(outputs[mode]),
               'metrics': archive(report, destination / f'{mode}-read.report.json.gz')}
    records.append(record)
    write_json(destination / 'RESULTS.json', records)
    assert check_report(json.loads(report.read_bytes()), mode, True) == hashes[0]

write_json(OUT / 'fixture-outputs.json', {
    'attempt': attempt, 'results': str((destination / 'RESULTS.json').relative_to(ROOT)),
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'release_binary_sha256': sha(binary), 'lineage_hex': lineage.hex(),
    'outputs': {mode: {'original': str(path), 'bytes': path.stat().st_size, 'sha256': sha(path)}
                for mode, path in outputs.items()},
    'scope': 'fresh same-image prepared-V2 writers/readers; M2b retains separate wrong-image rejection evidence',
})
