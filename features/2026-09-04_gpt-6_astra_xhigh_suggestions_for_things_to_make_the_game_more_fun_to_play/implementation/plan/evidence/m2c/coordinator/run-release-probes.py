"""Collect admitted pending-work preparation measurements after source freeze."""
from pathlib import Path
import json
import math
import sys
from release_common import LEG, OUT, ROOT, archive, capture, frozen_sources, sha, write_json

assert len(sys.argv) in (2, 3) and sys.argv[1] in ('smoke', 'performance')
kind = sys.argv[1]
attempt = int(sys.argv[2]) if len(sys.argv) == 3 else 1
assert attempt > 0
dataset = kind if attempt == 1 else f'{kind}-attempt-{attempt}'
samples, repetitions = (2, 1) if kind == 'smoke' else (100, 3)
build = json.loads((OUT / 'release_build.json').read_bytes())
assert build['exit_code'] == 0 and build['unchanged_source'] and build['unchanged_helpers']
assert build['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
binary = Path(build['reference_binary'])
assert sha(binary) == build['reference_binary_sha256']
destination = LEG / dataset
assert not destination.exists(), 'preserve all prior datasets, including failures'
destination.mkdir()
write_json(destination / 'IDENTITY.json', {
    'source_scope': build['source_scope'], 'source_hashes': frozen_sources(),
    'source_manifest_sha256': build['source_manifest_sha256'],
    'binary_sha256': sha(binary), 'binary': str(binary),
    'samples': samples, 'repetitions': repetitions,
    'scenario': 'actual-city-pending-cognition-and-accepted-recording',
})


def finite_nonnegative(value):
    return (isinstance(value, (int, float)) and not isinstance(value, bool)
            and math.isfinite(value) and value >= 0)


results = []
stage_order = ['Admission', 'Inputs', 'Speech', 'Floor', 'Prepared']
for mode in ('authored', 'populated'):
    for repetition in range(1, repetitions + 1):
        label = f'{dataset}-{mode}-{repetition}'
        output = Path(f'/tmp/alibi-m2c-{label}.metrics.json')
        assert not output.exists()
        record = capture(label, [str(binary), '--ignored', '--exact',
            'host_checkpoint::tests_continuation_owner::m2c_continuation_probe',
            '--nocapture', '--test-threads=1'], destination, {
                'ALIBI_CONTINUATION_MODE': mode,
                'ALIBI_CONTINUATION_SAMPLES': str(samples),
                'ALIBI_CONTINUATION_REPORT': str(output),
            }, binary)
        assert output.is_file(), 'probe omitted primary metrics'
        record |= {'metrics': archive(output, destination / f'{mode}-{repetition}.json.gz'),
                   'mode': mode, 'repetition': repetition, 'samples': samples}
        results.append(record)
        write_json(destination / 'RESULTS.json', results)
        data = json.loads(output.read_bytes())
        assert data['schema'] == 'm2c-continuation-v1'
        assert data['scenario'] == 'actual-city-pending-cognition-and-accepted-recording'
        assert data['profile'] == mode and data['samples'] == samples
        assert data['characters'] == (520 if mode == 'authored' else 2520)
        assert data['requested'] == data['placed'] == (0 if mode == 'authored' else 2000)
        assert data['unplaced'] == 0
        assert bytes(data['host_image']).hex() == sha(binary)
        assert data['capture_boundary_unchanged'] is True
        assert data['second_preparation_exact_bytes'] is True
        assert data['fixture_reader'] is False
        assert data['unchanged_categories_equal'] is True and data['unchanged_category_count'] == 8
        assert 0 < data['immediate_v2_resave_bytes'] <= (64 if mode == 'authored' else 128) * 1024**2
        assert 512 * 1024**2 < data['shared_peak_bytes'] <= 1024**3
        assert data['asset_lease_bytes'] == 64 * 1024**2
        assert data['service_lease_bytes'] == 64 * 1024
        assert data['time_columns'] == ['preparation', 'service_binding', 'prepared_owner_disposal']
        assert len(data['reports']) == len(data['times_us']) == len(data['stages_us']) == samples
        assert len(data['capture_costs']) == samples
        assert len(data['pending_shapes']) == samples
        for index, report in enumerate(data['reports']):
            pending = data['pending_shapes'][index]
            assert pending['scheduler_submitted'] is True
            assert pending['scheduler_unfinished'] is (not pending['scheduler_held'])
            assert pending['accepted_recordings'] == 1
            assert report['scheduler_held'] is pending['scheduler_held']
            assert report['scheduler_retries'] == pending['scheduler_deferred'] + int(pending['scheduler_unfinished'])
            assert report['services_bound'] is True
            assert report['interruption_receipts'] == 1
            assert 1 <= report['interrupted_inputs'] <= 64
            assert report['scheduler_retries'] == 1 or report['scheduler_held'] is True
            assert 0 < report['typed_upper_bytes'] <= 128 * 1024**2
            assert report['typed_upper_bytes'] + data['asset_lease_bytes'] + data['service_lease_bytes'] + 512 * 1024**2 <= data['shared_peak_bytes']
            original = data['capture_costs'][index]
            assert original['categories'] == 16 and original['characters'] == data['characters']
            assert 0 < original['encoded_bytes'] <= (64 if mode == 'authored' else 128) * 1024**2
            assert report['typed_upper_bytes'] == original['expanded_upper_bytes'] + (256 + 128) * 1024
            timings = data['times_us'][index]
            assert len(timings) == 3 and all(finite_nonnegative(value) for value in timings)
            stages = data['stages_us'][index]
            assert [row[0] for row in stages] == stage_order
            assert all(finite_nonnegative(row[1]) for row in stages)
            assert sum(row[1] for row in stages) <= timings[0] + 0.01
        print(json.dumps({'mode': mode, 'repetition': repetition,
            'shared_peak_bytes': data['shared_peak_bytes'], 'report': data['reports'][0]}), flush=True)

write_json(OUT / f'{kind}-collected.json', {
    'dataset': str(destination.relative_to(ROOT)),
    'identity_sha256': sha(destination / 'IDENTITY.json'),
    'results_sha256': sha(destination / 'RESULTS.json'),
    'source_manifest_sha256': build['source_manifest_sha256'],
    'binary_sha256': sha(binary),
})
