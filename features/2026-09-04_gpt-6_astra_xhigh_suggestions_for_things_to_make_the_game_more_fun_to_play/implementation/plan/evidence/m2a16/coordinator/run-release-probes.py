"""Preserve every complete-checkpoint release sample and its exact process inputs."""
from pathlib import Path
import json
import math
import os
import sys
from release_common import LEG, OUT, ROOT, archive, capture, frozen_sources, sha, write_json

assert len(sys.argv) in (2, 3) and sys.argv[1] in ('smoke', 'performance')
kind = sys.argv[1]
attempt = int(sys.argv[2]) if len(sys.argv) == 3 else 1
assert attempt > 0
assert not any(key in os.environ for key in ('ALIBI_COMPLETE_INITIAL', 'ALIBI_COMPLETE_FIXTURE'))
dataset = kind if attempt == 1 else f'{kind}-attempt-{attempt}'
samples, repetitions = (2, 1) if kind == 'smoke' else (100, 3)
build = json.loads((OUT / 'release_build.json').read_bytes())
assert build['exit_code'] == 0 and build['unchanged_source'] and build['unchanged_helpers']
assert build['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
binary = Path(build['reference_binary'])
assert sha(binary) == build['reference_binary_sha256']
destination = LEG / dataset
assert not destination.exists(), 'preserve earlier datasets, including failures'
destination.mkdir()
world_identity = '4d326131362d72656c656173652d3031'
assert len(bytes.fromhex(world_identity)) == 16
write_json(destination / 'IDENTITY.json', {
    'source_scope': build['source_scope'], 'source_hashes': frozen_sources(),
    'source_manifest_sha256': build['source_manifest_sha256'],
    'binary_sha256': sha(binary), 'binary': str(binary),
    'samples': samples, 'repetitions': repetitions,
    'scenario': 'actual-complete-host-boundary-v1',
    'initial_world_identity_hex': world_identity,
})
phases = ('complete_capture', 'input_copy', 'complete_load', 'candidate_disposal')
stage_orders = {
    'save_stages': ['Definitions', 'Preflight', 'Encode', 'OuterParse',
                    'TypedValidation', 'TypedDisposal', 'FinalRecheck', 'CandidateRetention'],
    'load_stages': ['Definitions', 'OuterParse', 'TypedValidation',
                    'TypedDisposal', 'CandidateRetention'],
}


def finite_nonnegative(value):
    return (isinstance(value, (int, float)) and not isinstance(value, bool)
            and math.isfinite(value) and value >= 0)


results = []
for mode in ('authored', 'populated'):
    for repetition in range(1, repetitions + 1):
        label = f'{dataset}-{mode}-{repetition}'
        output = Path(f'/tmp/alibi-m2a16-{label}.metrics.json')
        assert not output.exists()
        command = [str(binary), '--ignored', '--exact',
                   'host_checkpoint::tests_complete_owner::m2a16_complete_probe',
                   '--nocapture', '--test-threads=1']
        record = capture(label, command, destination, {
            'ALIBI_COMPLETE_MODE': mode, 'ALIBI_COMPLETE_SAMPLES': str(samples),
            'ALIBI_COMPLETE_REPORT': str(output), 'ALIBI_COMPLETE_WORLD_ID': world_identity,
        }, binary)
        assert output.is_file(), 'probe omitted its primary report'
        record |= {'metrics': archive(output, destination / f'{mode}-{repetition}.json.gz'),
                   'mode': mode, 'repetition': repetition, 'samples': samples}
        results.append(record)
        write_json(destination / 'RESULTS.json', results)
        data = json.loads(output.read_bytes())
        assert data['schema'] == 1 and data['scenario'] == 'actual-complete-host-boundary-v1'
        assert data['profile'] == mode and data['samples'] == samples
        assert data['characters'] == (520 if mode == 'authored' else 2520)
        assert data['placed'] == (0 if mode == 'authored' else 2000)
        assert data['active'] is True
        assert data['placement'] == {'requested': data['placed'], 'placed': data['placed'], 'unplaced': 0}
        counts = data['counts']
        assert counts['characters'] == data['characters'] and counts['cut_margin'] is True
        assert counts['entities'] >= (18000 if mode == 'authored' else 70000)
        for key, expected in {'collision_boxes': 751, 'collision_prisms': 1153,
                              'dynamic_barriers': 2, 'vermin_colonies': 8, 'rats': 150}.items():
            assert counts[key] == expected, (key, counts[key])
        readable = data['readable_counts']
        assert counts['rows'] == sum(readable.values())
        for key, expected in {'bubble': 1, 'subtitle': 1, 'player_receipt': 1,
                              'unread_speech': 4, 'unread_intent': 1,
                              'pending_command': 1, 'cooldown': 1, 'well_draw': 1}.items():
            assert readable[key] == expected, (key, readable[key])
        assert data['ordinary_boundary_unchanged'] is True
        assert data['running_reservation_bytes'] == 512 * 1024**2
        assert data['running_reservation_bytes'] < data['shared_peak_bytes'] <= 1024**3
        image = data['host_image']
        assert bytes(image['digest']).hex() == sha(binary)
        assert image['bytes'] == binary.stat().st_size
        assert finite_nonnegative(image['initial_hash_microseconds'])
        assert len(data['costs']) == samples
        for cost in data['costs']:
            assert 0 < cost['encoded_bytes'] <= (64 if mode == 'authored' else 128) * 1024**2
            assert cost['raw_capacity_bytes'] >= cost['encoded_bytes']
            assert 0 < cost['expanded_upper_bytes'] <= 128 * 1024**2
            assert cost['categories'] == 16 and cost['characters'] == data['characters']
            assert len(cost['category_expansion_upper_bytes']) == 16
            assert all(finite_nonnegative(v) for v in cost['category_expansion_upper_bytes'])
            assert sum(cost['category_expansion_upper_bytes']) <= cost['expanded_upper_bytes']
            assert cost['validation_scratch_bytes'] == 64 * 1024**2
            assert cost['diagnostic_scratch_bytes'] >= 4096
            assert cost['validation_peak_bytes'] == (
                3 * cost['raw_capacity_bytes'] + cost['validation_scratch_bytes']
                + cost['diagnostic_scratch_bytes'] + cost['expanded_upper_bytes'])
            assert cost['raw_capacity_bytes'] <= cost['retained_candidate_bytes'] <= cost['validation_peak_bytes']
            assert cost['validation_peak_bytes'] + data['running_reservation_bytes'] <= data['shared_peak_bytes']
        for phase in phases:
            values = data['phase_microseconds'][phase]
            assert len(values) == samples and all(finite_nonnegative(v) for v in values), phase
        for field, order in stage_orders.items():
            assert len(data[field]) == samples
            total_phase = 'complete_capture' if field == 'save_stages' else 'complete_load'
            for index, rows in enumerate(data[field]):
                assert [row[0] for row in rows] == order
                assert all(finite_nonnegative(row[1]) for row in rows)
                assert sum(row[1] for row in rows) <= data['phase_microseconds'][total_phase][index] + 0.01
        print(json.dumps({'mode': mode, 'repetition': repetition,
                          'shared_peak_bytes': data['shared_peak_bytes'],
                          'cost': data['costs'][0]}), flush=True)

write_json(OUT / f'{kind}-collected.json', {
    'dataset': str(destination.relative_to(ROOT)),
    'identity_sha256': sha(destination / 'IDENTITY.json'),
    'results_sha256': sha(destination / 'RESULTS.json'),
    'source_manifest_sha256': build['source_manifest_sha256'],
    'binary_sha256': sha(binary),
})
