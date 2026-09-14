"""Audit exact complete-checkpoint runs and retain cold and tail observations."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
from release_common import LEG, ROOT, SOURCE_SCOPE, frozen_sources, sha, write_json

PHASES = ('complete_capture', 'input_copy', 'complete_load', 'candidate_disposal')
STAGES = {
    'save_stages': ('Definitions', 'Preflight', 'Encode', 'OuterParse',
                    'TypedValidation', 'TypedDisposal', 'FinalRecheck', 'CandidateRetention'),
    'load_stages': ('Definitions', 'OuterParse', 'TypedValidation',
                    'TypedDisposal', 'CandidateRetention'),
}


def closed_object(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, f'duplicate JSON key: {key}'
        result[key] = value
    return result


def read_json(data):
    return json.loads(data, object_pairs_hook=closed_object,
                      parse_constant=lambda value: (_ for _ in ()).throw(
                          AssertionError(f'nonfinite JSON constant: {value}')))


def integer(value, minimum=0):
    assert type(value) is int and value >= minimum, value
    return value


def duration(value):
    assert type(value) in (int, float) and math.isfinite(value) and value >= 0
    return value


def check_metrics(data, mode, samples, binary):
    assert data['schema'] == 1 and data['scenario'] == 'actual-complete-host-boundary-v1'
    assert data['profile'] == mode and integer(data['samples'], 1) == samples
    assert data['active'] is True and data['ordinary_boundary_unchanged'] is True
    requested = 2000 if mode == 'populated' else 0
    assert data['placement'] == {'requested': requested, 'placed': requested, 'unplaced': 0}
    assert integer(data['characters']) == 520 + requested
    assert integer(data['placed']) == requested
    counts = data['counts']
    assert counts['characters'] == data['characters'] and counts['cut_margin'] is True
    assert integer(counts['entities']) >= (70000 if requested else 18000)
    for key, expected in {'collision_boxes': 751, 'collision_prisms': 1153,
                          'dynamic_barriers': 2, 'vermin_colonies': 8, 'rats': 150}.items():
        assert integer(counts[key]) == expected, (key, counts[key])
    readable = data['readable_counts']
    assert sum(integer(value, 1) for value in readable.values()) == integer(counts['rows'], 1)
    for key, expected in {'bubble': 1, 'subtitle': 1, 'player_receipt': 1,
                          'unread_speech': 4, 'unread_intent': 1,
                          'pending_command': 1, 'cooldown': 1, 'well_draw': 1}.items():
        assert readable[key] == expected, (key, readable[key])
    assert data['no_detached_full_world_copy'] is True
    staging = data['extraction_staging']
    assert staging['borrowed_host_index_bytes'] == 8 * 1024**2
    assert staging['host_sort_scratch_upper_bytes'] == 8 * 1024**2
    assert integer(staging['ledger_sublease_bytes'], 1)
    assert integer(staging['operations_sublease_bytes'], 1)
    running = integer(data['running_reservation_bytes'])
    assert running == 512 * 1024**2
    peak = integer(data['shared_peak_bytes'], running + 1)
    assert peak <= 1024**3
    image = data['host_image']
    assert bytes(image['digest']).hex() == sha(binary)
    assert integer(image['bytes'], 1) == binary.stat().st_size
    duration(image['initial_hash_microseconds'])
    inventory = data['running_inventory']
    assert inventory['render_ecs_assets_services_and_runtime_stacks_included'] is False
    assert inventory['world_engine_share_graph_arc'] is True
    assert inventory['world_nav'] == inventory['engine_nav']
    for key in ('world_nav', 'engine_nav', 'vermin_nav'):
        nav = inventory[key]
        assert integer(nav['nodes'], 1)
        assert integer(nav['graph_and_indexes_bytes'], 1)
        assert 0 <= integer(nav['cache_rows_retained']) <= nav['nodes']
        assert 0 < integer(nav['cache_retained_bytes']) <= integer(nav['cache_maximum_bytes'])
        assert nav['cache_maximum_bytes'] >= nav['nodes']**2 * 4
        assert nav['total_with_full_cache_bytes'] == nav['graph_and_indexes_bytes'] + nav['cache_maximum_bytes']
    a, b = inventory['world_nav'], inventory['vermin_nav']
    expected_nav = a['total_with_full_cache_bytes'] + b['graph_and_indexes_bytes']
    if not inventory['vermin_shares_world_cache']:
        expected_nav += b['cache_maximum_bytes']
    assert inventory['nav_distinct_graphs_and_full_caches_upper_bytes'] == expected_nav
    assert a['nodes'] == b['nodes'] == 10026
    registry_entries = 1992 if requested else 491
    assert a['cache_rows_retained'] <= registry_entries
    assert b['cache_rows_retained'] == 0 and inventory['vermin_shares_world_cache'] is False
    cache_base = a['cache_maximum_bytes'] - a['nodes'] * (a['nodes'] * 4 + 32)
    assert cache_base >= 0
    reachable_nav = (a['graph_and_indexes_bytes'] + b['graph_and_indexes_bytes']
        + cache_base + registry_entries * (a['nodes'] * 4 + 32) + b['cache_retained_bytes'])
    assert reachable_nav <= 128 * 1024**2
    for key in ('collision_retained_upper_bytes', 'vermin_excluding_nav_retained_upper_bytes',
                'cut_margin_retained_upper_bytes'):
        integer(inventory[key], 1)
    transport = inventory['host_transport']
    assert transport['seed_released'] is True
    assert transport['live_service_runtime_and_device_allocations_included'] is False
    assert transport['command_channel_capacity'] == 128
    assert transport['event_channel_capacity'] == 8192
    assert 0 <= integer(transport['command_rows']) <= 128
    assert 0 <= integer(transport['event_rows']) <= 8192
    assert transport['command_payload_upper_bytes'] == transport['command_rows'] * 24 * 1024
    assert 0 <= integer(transport['publication_payload_bytes']) <= 128 * 1024**2
    completion = transport['completions']
    # These are separate read-only snapshots of a live external mailbox.
    assert 0 <= integer(completion['queued']) <= 256
    assert 0 <= integer(completion['admitted_records']) <= 256
    assert 0 <= integer(completion['admitted_terminals']) <= 64
    assert 0 <= integer(completion['terminal_bytes']) <= 64 * 1024**2
    assert 0 <= integer(completion['stream_bytes']) <= 4 * 1024**2
    fake = transport['fake_cognition']
    assert 0 <= integer(fake['staged']) <= integer(fake['staged_capacity'])
    assert 0 <= integer(fake['prompts']) <= integer(fake['prompt_capacity'])
    integer(fake['retained_upper_bytes'], 1)
    integer(transport['transcript_rows'])
    integer(transport['transcript_retained_upper_bytes'], 1)
    integer(transport['round_ladder_scratch_bytes'], 1)
    assert len(data['costs']) == samples
    for cost in data['costs']:
        for value in cost.values():
            if type(value) is list:
                for item in value:
                    integer(item)
            else:
                integer(value)
        assert 0 < cost['encoded_bytes'] <= (128 if requested else 64) * 1024**2
        assert cost['raw_capacity_bytes'] >= cost['encoded_bytes']
        assert 0 < cost['expanded_upper_bytes'] <= 128 * 1024**2
        assert cost['categories'] == 16 and cost['characters'] == data['characters']
        assert len(cost['category_expansion_upper_bytes']) == 16
        assert sum(cost['category_expansion_upper_bytes']) <= cost['expanded_upper_bytes']
        assert cost['validation_scratch_bytes'] == 64 * 1024**2
        assert cost['diagnostic_scratch_bytes'] >= 4096
        assert cost['validation_peak_bytes'] == (3 * cost['raw_capacity_bytes']
            + cost['validation_scratch_bytes'] + cost['diagnostic_scratch_bytes']
            + cost['expanded_upper_bytes'])
        assert cost['raw_capacity_bytes'] <= cost['retained_candidate_bytes'] <= cost['validation_peak_bytes']
        assert cost['validation_peak_bytes'] + running <= peak
    for phase in PHASES:
        values = data['phase_microseconds'][phase]
        assert isinstance(values, list) and len(values) == samples
        for value in values:
            duration(value)
    for field, order in STAGES.items():
        assert len(data[field]) == samples
        total = 'complete_capture' if field == 'save_stages' else 'complete_load'
        for index, rows in enumerate(data[field]):
            assert tuple(row[0] for row in rows) == order
            assert sum(duration(row[1]) for row in rows) <= data['phase_microseconds'][total][index] + .01


def check_archive(record):
    original, archive = Path(record['original']), ROOT / record['archive']
    assert record['normalization'] == 'none; exact original bytes'
    assert sha(original) == record['original_sha256']
    assert sha(archive) == record['archive_sha256']
    zipped = archive.read_bytes()
    assert zipped[4:8] == bytes(4)
    raw = gzip.decompress(zipped)
    assert len(raw) == record['original_bytes'] == original.stat().st_size
    assert hashlib.sha256(raw).hexdigest() == record['original_sha256']
    assert raw == original.read_bytes()
    return raw


def distribution(values):
    ordered = sorted(values)
    return {'samples': len(values), 'min_ms': ordered[0] / 1000,
            'p50_ms': ordered[math.ceil(len(values) * .50) - 1] / 1000,
            'p95_ms': ordered[math.ceil(len(values) * .95) - 1] / 1000,
            'p99_ms': ordered[math.ceil(len(values) * .99) - 1] / 1000,
            'max_ms': ordered[-1] / 1000,
            'over_2ms': sum(value > 2000 for value in values),
            'over_30ms': sum(value > 30000 for value in values)}


def audit(dataset):
    identity = read_json((dataset / 'IDENTITY.json').read_bytes())
    records = read_json((dataset / 'RESULTS.json').read_bytes())
    source = frozen_sources()
    assert identity['source_scope'] == SOURCE_SCOPE and identity['source_hashes'] == source
    assert identity['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
    assert identity['scenario'] == 'actual-complete-host-boundary-v1'
    binary = Path(identity['binary'])
    assert sha(binary) == identity['binary_sha256']
    repetitions, samples = identity['repetitions'], identity['samples']
    assert (samples, repetitions) in ((2, 1), (100, 3))
    assert len(records) == 2 * repetitions
    seen, groups, runs, tails, shapes = set(), {}, [], [], {}
    for record in records:
        mode, repetition = record['mode'], record['repetition']
        assert mode in ('authored', 'populated') and 1 <= repetition <= repetitions
        assert (mode, repetition) not in seen
        seen.add((mode, repetition))
        label = f'{dataset.name}-{mode}-{repetition}'
        start = read_json((dataset / f'{label}.start.json').read_bytes())
        result = read_json((dataset / f'{label}.result.json').read_bytes())
        assert all(record[key] == value for key, value in result.items())
        assert all(result[key] == value for key, value in start.items())
        assert record['exit_code'] == 0
        assert all(record[key] is True for key in
                   ('unchanged_source', 'unchanged_helpers', 'unchanged_binary'))
        assert record['source_scope'] == SOURCE_SCOPE and record['source_count'] == len(source)
        assert record['source_manifest_sha256'] == identity['source_manifest_sha256']
        assert record['binary'] == str(binary) and record['binary_sha256'] == identity['binary_sha256']
        assert record['command'] == [str(binary), '--ignored', '--exact',
            'host_checkpoint::tests_complete_owner::m2a16_complete_probe', '--nocapture', '--test-threads=1']
        environment = record['environment_overrides']
        assert environment['CATHEDRAL_HEADLESS'] == environment['CATHEDRAL_FAKE_BACKEND'] == '1'
        assert environment['ALIBI_COMPLETE_MODE'] == mode
        assert environment['ALIBI_COMPLETE_SAMPLES'] == str(samples)
        assert environment['ALIBI_COMPLETE_REPORT'] == record['metrics']['original']
        assert environment['ALIBI_COMPLETE_WORLD_ID'] == identity['initial_world_identity_hex']
        for path, digest in record['helper_sha256'].items():
            assert sha(ROOT / path) == digest
        log = check_archive(record['log']).decode()
        assert re.search(r'test result: ok\. 1 passed; 0 failed;', log)
        assert log.count('test host_checkpoint::tests_complete_owner::m2a16_complete_probe ...') == 1
        registries = re.findall(r'wayfinding: (\d+) places in the registry \((\d+) homes\)', log)
        assert registries == [('1992', '1914') if mode == 'populated' else ('491', '413')]
        time_log = check_archive(record['time']).decode()
        rss = re.search(r'Maximum resident set size \(kbytes\): (\d+)', time_log)
        assert rss and int(rss[1]) > 0 and re.search(r'Exit status: 0\s*$', time_log)
        data = read_json(check_archive(record['metrics']))
        check_metrics(data, mode, samples, binary)
        shape = {key: data[key] for key in ('counts', 'placement', 'readable_counts', 'extraction_staging')}
        shape['cost'] = data['costs'][0]
        assert shapes.setdefault(mode, shape) == shape
        assert all(cost == shape['cost'] for cost in data['costs'])
        measured = dict(data['phase_microseconds'])
        for field, order in STAGES.items():
            for stage_index, name in enumerate(order):
                measured[f'{field}/{name}'] = [rows[stage_index][1] for rows in data[field]]
        run = {'mode': mode, 'repetition': repetition, 'shape': shape,
               'process_max_rss_kib': int(rss[1]), 'shared_peak_bytes': data['shared_peak_bytes'],
               'elapsed_seconds': record['elapsed_seconds'], 'host_image': data['host_image'],
               'running_inventory': data['running_inventory'],
               'registry_entries': int(registries[0][0]),
               'running_scope_allowances_mib': {
                   'semantic_and_unused_capacity': 176, 'navigation_reachable_destinations': 128,
                   'other_definitions_and_prompt': 32, 'fake_cognition': 64,
                   'completion_mailbox': 68, 'selected_host_and_transport': 32},
               'first_sample_ms': {key: values[0] / 1000 for key, values in measured.items()},
               'phases': {}}
        for phase, values in measured.items():
            groups.setdefault((mode, phase), []).extend(values)
            run['phases'][phase] = distribution(values)
            for index, value in enumerate(values):
                if value > 2000:
                    tails.append({'mode': mode, 'repetition': repetition, 'phase': phase,
                                  'index': index, 'milliseconds': value / 1000})
        runs.append(run)
    assert seen == {(mode, rep) for mode in ('authored', 'populated')
                    for rep in range(1, repetitions + 1)}
    phase_names = PHASES + tuple(f'{field}/{name}' for field, order in STAGES.items() for name in order)
    return {'result': 'integrity passed', 'dataset': str(dataset.relative_to(ROOT)),
            'source_manifest_sha256': identity['source_manifest_sha256'],
            'binary_sha256': identity['binary_sha256'],
            'raw_end_to_end_phase_samples': len(records) * samples * len(PHASES),
            'raw_stage_samples': len(records) * samples * sum(map(len, STAGES.values())),
            'scope': 'Complete read-only candidate; trusted Running authority scope. No Engine hydration, application adoption, renderer admission or frame-budget acceptance.',
            'runs': runs, 'pooled': {mode: {phase: distribution(groups[(mode, phase)])
                for phase in phase_names} for mode in ('authored', 'populated')},
            'all_samples_over_2ms': tails}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('dataset', type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    assert not args.report.exists(), 'preserve earlier audit reports'
    report = audit(args.dataset.resolve())
    report['auditor_sha256'] = sha(Path(__file__))
    write_json(args.report, report)
    print(json.dumps({key: report[key] for key in
        ('result', 'dataset', 'raw_end_to_end_phase_samples', 'raw_stage_samples', 'pooled')}))
