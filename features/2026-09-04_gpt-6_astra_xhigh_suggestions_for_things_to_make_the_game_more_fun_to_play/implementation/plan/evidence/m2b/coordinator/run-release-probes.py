"""Collect exact release hydration samples; preserve failures and cold runs."""
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
    'scenario': 'actual-complete-hydration-v1',
})


def finite_nonnegative(value):
    return (isinstance(value, (int, float)) and not isinstance(value, bool)
            and math.isfinite(value) and value >= 0)


results = []
stage_order = ['Assets', 'Definitions', 'TypedOwners', 'Construction', 'RawDisposal', 'Retention']
for mode in ('authored', 'populated'):
    for repetition in range(1, repetitions + 1):
        label = f'{dataset}-{mode}-{repetition}'
        output = Path(f'/tmp/alibi-m2b-{label}.metrics.json')
        assert not output.exists()
        command = [str(binary), '--ignored', '--exact',
                   'host_checkpoint::tests_hydration_owner::m2b_hydration_probe',
                   '--nocapture', '--test-threads=1']
        record = capture(label, command, destination, {
            'ALIBI_HYDRATION_MODE': mode, 'ALIBI_HYDRATION_SAMPLES': str(samples),
            'ALIBI_HYDRATION_REPORT': str(output),
        }, binary)
        assert output.is_file(), 'probe omitted primary metrics'
        record |= {'metrics': archive(output, destination / f'{mode}-{repetition}.json.gz'),
                   'mode': mode, 'repetition': repetition, 'samples': samples}
        results.append(record)
        write_json(destination / 'RESULTS.json', results)
        data = json.loads(output.read_bytes())
        assert data['schema'] == 'm2b-hydration-v1'
        assert data['scenario'] == 'actual-city-rich-ordinary-boundary'
        assert data['profile'] == mode and data['pending_case'] == 0
        assert data['samples'] == samples and data['extra'] == (0 if mode == 'authored' else 2000)
        assert data['characters'] == (520 if mode == 'authored' else 2520)
        assert data['requested'] == data['placed'] == data['extra'] and data['unplaced'] == 0
        assert bytes(data['host_image']).hex() == sha(binary)
        assert data['actual_owner_categories'] == 16 and data['capture_boundary_unchanged'] is True
        assert 512 * 1024**2 < data['shared_peak_bytes'] <= 1024**3
        assert 0 < data['asset_lease_bytes'] <= 1024**3
        assert len(data['costs']) == len(data['times_us']) == len(data['stages_us']) == samples
        assert len(data['capture_costs']) == len(data['factory_cumulative_requested_bytes']) == samples
        assert len(data['factory_plus_retained_shared_assets_upper_bytes']) == samples
        assert data['shared_prompt_runtime_upper_bytes'] == 1024**2
        assert data['world_nav_before'] == data['world_nav_after']
        assert data['engine_nav_before'] == data['engine_nav_after']
        navs = [data['world_nav_before']]
        if not data['world_engine_share_graph_arc']:
            navs.append(data['engine_nav_before'])
        nav_bytes = sum(nav['graph_and_indexes_bytes'] + nav['cache_retained_bytes'] + 64
                        for nav in navs if nav is not None)
        assert data['retained_distinct_shared_nav_upper_bytes'] == nav_bytes
        for index, cost in enumerate(data['costs']):
            assert 0 < cost['decoded_upper_bytes'] <= 128 * 1024**2
            assert cost['structural_upper_bytes'] == 256 * 1024
            assert cost['assets_upper_bytes'] == data['asset_lease_bytes']
            assert cost['retained_upper_bytes'] == cost['decoded_upper_bytes'] + cost['assets_upper_bytes']
            assert cost['retained_upper_bytes'] + 512 * 1024**2 <= data['shared_peak_bytes']
            original = data['capture_costs'][index]
            assert original['categories'] == 16 and original['characters'] == data['characters']
            assert 0 < original['encoded_bytes'] <= (64 if mode == 'authored' else 128) * 1024**2
            assert cost['decoded_upper_bytes'] == original['expanded_upper_bytes'] + cost['structural_upper_bytes']
            assert 0 < data['factory_cumulative_requested_bytes'][index] <= data['asset_lease_bytes']
            full_assets = data['factory_cumulative_requested_bytes'][index] + nav_bytes + 1024**2
            assert data['factory_plus_retained_shared_assets_upper_bytes'][index] == full_assets
            assert full_assets <= data['asset_lease_bytes']
            timings = data['times_us'][index]
            assert len(timings) == 2 and all(finite_nonnegative(value) for value in timings)
            stages = data['stages_us'][index]
            assert [row[0] for row in stages] == stage_order
            assert all(finite_nonnegative(row[1]) for row in stages)
            assert sum(row[1] for row in stages) <= timings[0] + 0.01
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
