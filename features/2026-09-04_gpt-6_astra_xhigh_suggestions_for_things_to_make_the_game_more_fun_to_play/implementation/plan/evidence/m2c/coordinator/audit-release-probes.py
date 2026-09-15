"""Audit exact continuation measurements without discarding cold or slow samples."""
import gzip
import json
import math
import re
import sys
from pathlib import Path
from release_common import LEG, OUT, ROOT, sha, write_json

assert len(sys.argv) <= 2
kind = sys.argv[1] if len(sys.argv) == 2 else 'performance'
assert kind in ('smoke', 'performance')
collected = json.loads((OUT / f'{kind}-collected.json').read_bytes())
dataset = ROOT / collected['dataset']
identity = json.loads((dataset / 'IDENTITY.json').read_bytes())
records = json.loads((dataset / 'RESULTS.json').read_bytes())
assert sha(dataset / 'IDENTITY.json') == collected['identity_sha256']
assert sha(dataset / 'RESULTS.json') == collected['results_sha256']
assert identity['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
count, repetitions = (2, 1) if kind == 'smoke' else (100, 3)
assert len(records) == 2 * repetitions
assert (identity['samples'], identity['repetitions']) == (count, repetitions)
phases = ('preparation', 'service_binding', 'prepared_owner_disposal')
stage_names = ('Admission', 'Inputs', 'Speech', 'Floor', 'Prepared')
values = {mode: {phase: [] for phase in phases} for mode in ('authored', 'populated')}
stages = {mode: {stage: [] for stage in stage_names} for mode in values}
peaks = {mode: [] for mode in values}
shared_peaks = {mode: [] for mode in values}
typed = {mode: [] for mode in values}
pending = {mode: [] for mode in values}
cold, tails, seen = [], [], set()


def original(record):
    path, archive = Path(record['original']), ROOT / record['archive']
    assert sha(path) == record['original_sha256']
    assert sha(archive) == record['archive_sha256']
    raw, compressed = path.read_bytes(), archive.read_bytes()
    assert len(raw) == record['original_bytes']
    assert gzip.decompress(compressed) == raw
    assert int.from_bytes(compressed[4:8], 'little') == 0
    return raw


def retain(mode, repetition, index, scope, phase, micros):
    assert isinstance(micros, (int, float)) and not isinstance(micros, bool)
    assert math.isfinite(micros) and micros >= 0
    row = {'mode': mode, 'repetition': repetition, 'sample': index,
           'scope': scope, 'phase': phase, 'microseconds': micros}
    if index == 0:
        cold.append(row)
    if micros > 2000:
        tails.append(row | {'above_30ms': micros > 30000})


for record in records:
    mode, repetition = record['mode'], record['repetition']
    assert (mode, repetition) not in seen
    seen.add((mode, repetition))
    label = f'{dataset.name}-{mode}-{repetition}'
    start = json.loads((dataset / f'{label}.start.json').read_bytes())
    result = json.loads((dataset / f'{label}.result.json').read_bytes())
    assert all(result[key] == value for key, value in start.items())
    assert all(record[key] == value for key, value in result.items())
    for path, digest in record['helper_sha256'].items():
        assert sha(ROOT / path) == digest
    assert record['exit_code'] == 0
    assert record['unchanged_source'] and record['unchanged_binary'] and record['unchanged_helpers']
    assert record['binary_sha256'] == identity['binary_sha256'] == sha(Path(record['binary']))
    assert record['source_manifest_sha256'] == identity['source_manifest_sha256']
    assert record['command'] == [record['binary'], '--ignored', '--exact',
        'host_checkpoint::tests_continuation_owner::m2c_continuation_probe',
        '--nocapture', '--test-threads=1']
    assert b'test result: ok. 1 passed; 0 failed;' in original(record['log'])
    timing = original(record['time'])
    peaks[mode].append(int(re.search(rb'Maximum resident set size \(kbytes\): (\d+)', timing)[1]))
    data = json.loads(original(record['metrics']))
    assert data['schema'] == 'm2c-continuation-v1' and data['profile'] == mode
    assert data['scenario'] == 'actual-city-pending-cognition-and-accepted-recording'
    assert data['samples'] == count and data['characters'] == (520 if mode == 'authored' else 2520)
    assert data['requested'] == data['placed'] == (0 if mode == 'authored' else 2000)
    assert data['unplaced'] == 0 and bytes(data['host_image']).hex() == identity['binary_sha256']
    assert data['fixture_reader'] is False
    assert data['capture_boundary_unchanged'] and data['second_preparation_exact_bytes']
    assert data['unchanged_categories_equal'] and data['unchanged_category_count'] == 8
    assert data['time_columns'] == list(phases)
    assert all(len(data[key]) == count for key in
               ('times_us', 'stages_us', 'capture_costs', 'reports', 'pending_shapes'))
    shared_peaks[mode].append(data['shared_peak_bytes'])
    assert 512 * 1024**2 < data['shared_peak_bytes'] <= 1024**3
    assert data['asset_lease_bytes'] == 64 * 1024**2 and data['service_lease_bytes'] == 64 * 1024
    for index, timings in enumerate(data['times_us']):
        for phase, micros in zip(phases, timings, strict=True):
            values[mode][phase].append(micros)
            retain(mode, repetition, index, 'end_to_end', phase, micros)
        stage_row = data['stages_us'][index]
        assert [name for name, _ in stage_row] == list(stage_names)
        assert sum(micros for _, micros in stage_row) <= timings[0] + .01
        for stage, micros in stage_row:
            stages[mode][stage].append(micros)
            retain(mode, repetition, index, 'stage', stage, micros)
        report, cost, shape = (data[key][index] for key in ('reports', 'capture_costs', 'pending_shapes'))
        assert report['services_bound'] and report['interruption_receipts'] == 1
        assert 1 <= report['interrupted_inputs'] <= 64
        assert shape['accepted_recordings'] == 1 and shape['scheduler_submitted'] is True
        assert shape['scheduler_unfinished'] is (not shape['scheduler_held'])
        assert report['scheduler_held'] is shape['scheduler_held']
        assert report['scheduler_retries'] == shape['scheduler_deferred'] + int(shape['scheduler_unfinished'])
        assert cost['categories'] == 16 and cost['characters'] == data['characters']
        assert 0 < cost['encoded_bytes'] <= (64 if mode == 'authored' else 128) * 1024**2
        assert report['typed_upper_bytes'] == cost['expanded_upper_bytes'] + 384 * 1024
        assert 0 < report['typed_upper_bytes'] <= 128 * 1024**2
        assert report['typed_upper_bytes'] + 576 * 1024**2 + 64 * 1024 <= data['shared_peak_bytes']
        typed[mode].append(report['typed_upper_bytes'])
        pending[mode].append(shape)


def distribution(samples):
    ordered = sorted(samples)
    return {'samples': len(samples), 'minimum_us': ordered[0],
            'p50_us': ordered[math.ceil(len(ordered) * .50) - 1],
            'p95_us': ordered[math.ceil(len(ordered) * .95) - 1],
            'p99_us': ordered[math.ceil(len(ordered) * .99) - 1],
            'maximum_us': ordered[-1]}


assert seen == {(mode, rep) for mode in values for rep in range(1, repetitions + 1)}
summary = {
    'result': 'passed', 'dataset': str(dataset.relative_to(ROOT)),
    'source_manifest_sha256': identity['source_manifest_sha256'],
    'binary_sha256': identity['binary_sha256'], 'results_sha256': sha(dataset / 'RESULTS.json'),
    'end_to_end_samples': sum(len(items) for group in values.values() for items in group.values()),
    'stage_samples': sum(len(items) for group in stages.values() for items in group.values()),
    'phases': {mode: {name: distribution(items) for name, items in group.items()} for mode, group in values.items()},
    'stages': {mode: {name: distribution(items) for name, items in group.items()} for mode, group in stages.items()},
    'maximum_rss_kib': {mode: max(items) for mode, items in peaks.items()},
    'maximum_shared_budget_bytes': {mode: max(items) for mode, items in shared_peaks.items()},
    'typed_ranges_bytes': {mode: {'minimum': min(items), 'maximum': max(items)} for mode, items in typed.items()},
    'source_pending_shape_counts': {mode: {shape: sum(bool(row[shape]) for row in items) for shape in
        ('scheduler_submitted', 'scheduler_held', 'scheduler_unfinished', 'night_submitted', 'night_held')}
        for mode, items in pending.items()},
    'cold_first_samples': cold, 'above_2ms': len(tails), 'above_30ms': sum(row['above_30ms'] for row in tails),
    'normalization': 'none; every process and sample retained, nearest-rank quantiles',
    'timing_scope': 'preparation, inert service factory binding and prepared-owner disposal measured separately; prior capture/validation/hydration and subsequent re-save excluded; source host retains shared nav',
}
write_json(OUT / f'release-{kind}-audit.json', summary)
write_json(OUT / f'release-{kind}-frame-threshold-samples.json', tails)
print(json.dumps(summary, sort_keys=True))
