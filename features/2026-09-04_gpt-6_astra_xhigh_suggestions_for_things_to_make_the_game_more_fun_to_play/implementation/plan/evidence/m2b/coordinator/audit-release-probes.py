"""Audit exact release originals and retain every sample above frame thresholds."""
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
assert len(records) == 2 * repetitions and identity['samples'] == count and identity['repetitions'] == repetitions
phases = ('hydrate_with_asset_factory', 'hydrated_owner_disposal')
values = {mode: {phase: [] for phase in phases} for mode in ('authored', 'populated')}
stages = {mode: {} for mode in values}
peaks = {mode: [] for mode in values}
costs = {mode: [] for mode in values}
allocations = {mode: [] for mode in values}
shared_peaks = {mode: [] for mode in values}
cold = []
tails = []
seen = set()


def original(record):
    path, archive = Path(record['original']), ROOT / record['archive']
    assert sha(path) == record['original_sha256']
    assert sha(archive) == record['archive_sha256']
    raw = path.read_bytes()
    assert len(raw) == record['original_bytes']
    assert gzip.decompress(archive.read_bytes()) == raw
    assert int.from_bytes(archive.read_bytes()[4:8], 'little') == 0
    return raw


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
    assert record['exit_code'] == 0 and record['unchanged_source'] and record['unchanged_binary']
    assert record['unchanged_helpers'] and record['binary_sha256'] == identity['binary_sha256']
    assert record['source_manifest_sha256'] == identity['source_manifest_sha256']
    assert sha(Path(record['binary'])) == identity['binary_sha256']
    assert record['command'] == [record['binary'], '--ignored', '--exact',
        'host_checkpoint::tests_hydration_owner::m2b_hydration_probe',
        '--nocapture', '--test-threads=1']
    log = original(record['log'])
    assert b'test result: ok. 1 passed; 0 failed;' in log
    timing = original(record['time'])
    peaks[mode].append(int(re.search(rb'Maximum resident set size \(kbytes\): (\d+)', timing)[1]))
    data = json.loads(original(record['metrics']))
    assert data['samples'] == count and data['profile'] == mode
    assert len(data['times_us']) == len(data['stages_us']) == len(data['costs']) == count
    costs[mode].extend(data['costs'])
    allocations[mode].extend(data['factory_plus_retained_shared_assets_upper_bytes'])
    shared_peaks[mode].append(data['shared_peak_bytes'])
    for index, timings in enumerate(data['times_us']):
        for phase, micros in zip(phases, timings, strict=True):
            assert math.isfinite(micros) and micros >= 0
            values[mode][phase].append(micros)
            row = {'mode': mode, 'repetition': repetition, 'sample': index,
                   'scope': 'end_to_end', 'phase': phase, 'microseconds': micros}
            if index == 0:
                cold.append(row)
            if micros > 2_000:
                tails.append(row | {'above_30ms': micros > 30_000})
        for stage, micros in data['stages_us'][index]:
            stages[mode].setdefault(stage, []).append(micros)
            assert math.isfinite(micros) and micros >= 0
            row = {'mode': mode, 'repetition': repetition, 'sample': index,
                   'scope': 'stage', 'phase': stage, 'microseconds': micros}
            if index == 0:
                cold.append(row)
            if micros > 2_000:
                tails.append(row | {'above_30ms': micros > 30_000})
    assert 0 < data['shared_peak_bytes'] <= 1024**3
    for cost in data['costs']:
        assert cost['decoded_upper_bytes'] <= 128 * 1024**2


def distribution(samples):
    ordered = sorted(samples)
    return {'samples': len(samples), 'minimum_us': ordered[0],
            'p50_us': ordered[math.ceil(len(ordered) * .50) - 1],
            'p95_us': ordered[math.ceil(len(ordered) * .95) - 1],
            'p99_us': ordered[math.ceil(len(ordered) * .99) - 1],
            'maximum_us': ordered[-1]}


assert seen == {(mode, repetition) for mode in values for repetition in range(1, repetitions + 1)}
summary = {
    'result': 'passed', 'dataset': str(dataset.relative_to(ROOT)),
    'source_manifest_sha256': identity['source_manifest_sha256'],
    'binary_sha256': identity['binary_sha256'],
    'results_sha256': sha(dataset / 'RESULTS.json'),
    'end_to_end_samples': sum(len(items) for mode in values.values() for items in mode.values()),
    'stage_samples': sum(len(items) for mode in stages.values() for items in mode.values()),
    'phases': {mode: {name: distribution(items) for name, items in group.items()}
               for mode, group in values.items()},
    'stages': {mode: {name: distribution(items) for name, items in group.items()}
               for mode, group in stages.items()},
    'maximum_rss_kib': {mode: max(items) for mode, items in peaks.items()},
    'maximum_shared_budget_bytes': {mode: max(items) for mode, items in shared_peaks.items()},
    'maximum_full_asset_bound_bytes': {mode: max(items) for mode, items in allocations.items()},
    'cost_ranges_bytes': {mode: {name: {'minimum': min(row[name] for row in items),
                                      'maximum': max(row[name] for row in items)}
                                for name in items[0]} for mode, items in costs.items()},
    'cold_first_samples': cold,
    'above_2ms': len(tails), 'above_30ms': sum(row['above_30ms'] for row in tails),
    'normalization': 'none; every process and sample retained, nearest-rank quantiles',
    'timing_scope': 'hydrate includes admitted asset factory; checkpoint-file IO and prior input validation excluded',
}
write_json(OUT / f'release-{kind}-audit.json', summary)
write_json(OUT / f'release-{kind}-frame-threshold-samples.json', tails)
print(json.dumps(summary, sort_keys=True))
