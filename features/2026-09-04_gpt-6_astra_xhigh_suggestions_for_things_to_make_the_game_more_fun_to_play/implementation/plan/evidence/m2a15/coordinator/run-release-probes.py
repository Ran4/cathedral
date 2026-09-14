"""Independent-process actual-city host smokes and six-phase release samples."""
from pathlib import Path
import json
import math
import sys
from release_common import LEG, OUT, ROOT, archive, capture, frozen_sources, sha, write_json

assert len(sys.argv) in (2, 3) and sys.argv[1] in ('smoke', 'performance')
kind = sys.argv[1]
attempt = int(sys.argv[2]) if len(sys.argv) == 3 else 1
assert attempt > 0
dataset_name = kind if attempt == 1 else f'{kind}-attempt-{attempt}'
samples, repetitions = (2, 1) if kind == 'smoke' else (100, 3)
build = json.loads((OUT / 'release_build.json').read_bytes())
assert build['exit_code'] == 0 and build['unchanged_source'] and build['unchanged_helpers']
assert build['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
binary = Path(build['reference_binary'])
assert sha(binary) == build['reference_binary_sha256']
destination = LEG / dataset_name
assert not destination.exists(), 'preserve every previous dataset, including failures'
destination.mkdir()
write_json(destination / 'IDENTITY.json', {
    'source_scope': build['source_scope'], 'source_sha256': frozen_sources(),
    'source_manifest_sha256': build['source_manifest_sha256'],
    'binary_sha256': sha(binary), 'binary': str(binary),
    'samples': samples, 'repetitions': repetitions,
    'scenario': 'actual-host-boundary-v1',
})
phases = (
    'preflight_us', 'export_us', 'encode_us', 'decode_validate_us',
    'candidate_validate_us', 'drop_us',
)
results = []
for mode in ('authored', 'populated'):
    for repetition in range(1, repetitions + 1):
        label = f'{dataset_name}-{mode}-{repetition}'
        output = Path(f'/tmp/alibi-m2a15-{label}.metrics.json')
        assert not output.exists()
        command = [str(binary), '--ignored', '--exact',
                   'host_checkpoint::tests::m2a15_cost_probe', '--nocapture', '--test-threads=1']
        record = capture(label, command, destination, {
            'ALIBI_HOST_MODE': mode, 'ALIBI_HOST_SAMPLES': str(samples),
            'ALIBI_HOST_OUTPUT': str(output),
        }, binary)
        assert output.is_file(), 'probe did not emit its primary metrics'
        metric_archive = archive(output, destination / f'{mode}-{repetition}.json.gz')
        data = json.loads(output.read_bytes())
        record['metrics'] = metric_archive
        record['mode'] = mode
        record['repetition'] = repetition
        record['samples'] = samples
        results.append(record)
        write_json(destination / 'RESULTS.json', results)
        assert data['schema'] == 1 and data['scenario'] == 'actual-host-boundary-v1'
        assert data['mode'] == mode and data['samples'] == samples
        for phase in phases:
            values = data[phase]
            assert len(values) == samples and all(
                isinstance(value, (int, float)) and not isinstance(value, bool)
                and math.isfinite(value) and value >= 0 for value in values
            ), phase
        print(json.dumps({'mode': mode, 'repetition': repetition,
                          'counts': data['counts'], 'cost': data['cost']}), flush=True)
write_json(OUT / f'{kind}-collected.json', {
    'dataset': str(destination.relative_to(ROOT)),
    'identity_sha256': sha(destination / 'IDENTITY.json'),
    'results_sha256': sha(destination / 'RESULTS.json'),
    'source_manifest_sha256': build['source_manifest_sha256'],
    'binary_sha256': sha(binary),
})
