"""Audit primary host measurements, archives and command-at-start identities.

Timing distributions are reported without equating component costs to a complete
Save/Load frame. Every slow sample remains visible, including cold first samples.
"""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
from release_common import LEG, OUT, ROOT, SOURCE_SCOPE, frozen_sources, sha, write_json

PHASES = ('preflight_us', 'export_us', 'encode_us', 'decode_validate_us',
          'candidate_validate_us', 'drop_us')


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


def check_metrics(data, mode, samples):
    assert data['schema'] == 1 and data['scenario'] == 'actual-host-boundary-v1'
    assert data['mode'] == mode and integer(data['samples'], 1) == samples
    counts = data['counts']
    for key in ('entities', 'characters', 'collision_boxes', 'collision_prisms',
                'dynamic_barriers', 'vermin_colonies', 'rats', 'rows'):
        integer(counts[key], 1)
    # Both collider families are required; box count alone is not a proxy for
    # this city's geometry, which also includes many convex prisms.
    assert counts['dynamic_barriers'] == 2
    assert counts['vermin_colonies'] == 8 and counts['cut_margin'] is True
    assert counts['entities'] > counts['characters']
    requested = 2000 if mode == 'populated' else 0
    placement = data['placement']
    assert set(placement) == {'requested', 'placed', 'unplaced'}
    for value in placement.values():
        integer(value)
    assert placement == {'requested': requested, 'placed': requested, 'unplaced': 0}
    assert counts['characters'] > requested
    readable = data['readable_counts']
    assert readable and all(isinstance(key, str) for key in readable)
    assert sum(integer(value, 1) for value in readable.values()) == counts['rows']
    # These come from ordinary submission, drain and presentation, not from
    # substituting a quiet city or generating synthetic timed DTO rows.
    for kind in ('draft', 'hud', 'unread_intent', 'unread_speech', 'subtitle', 'bubble'):
        assert readable.get(kind, 0) > 0, f'missing ordinary workload: {kind}'
    witness = data['readonly']
    assert set(witness) == {f'{key}_{side}' for key in
        ('world_revision', 'event_sequence', 'input_watermark') for side in ('before', 'after')}
    for key in ('world_revision', 'event_sequence', 'input_watermark'):
        assert integer(witness[f'{key}_before']) == integer(witness[f'{key}_after'])
    cost = data['cost']
    assert set(cost) == {'encoded_bytes', 'expanded_upper_bytes', 'peak_bytes',
                         'validation_working_bytes', 'container_stride_bytes'}
    for value in cost.values():
        integer(value, 1)
    assert cost['encoded_bytes'] <= 128 * 1024**2
    assert cost['expanded_upper_bytes'] <= 128 * 1024**2
    assert cost['validation_working_bytes'] == 4 * 1024**2
    assert cost['container_stride_bytes'] == 4096
    assert cost['peak_bytes'] >= cost['encoded_bytes'] + cost['expanded_upper_bytes']
    retained = integer(data['shared_reserved_peak_excluding_running_bytes'], 1)
    assert cost['peak_bytes'] <= retained <= 1024**3
    assert 'excluding full Save/Load/Running assembly' in data['scope']
    for phase in PHASES:
        values = data[phase]
        assert isinstance(values, list) and len(values) == samples
        assert all(type(value) in (int, float) and math.isfinite(value) and value >= 0
                   for value in values), phase


def check_archive(record):
    original = Path(record['original'])
    archive = ROOT / record['archive']
    assert record['normalization'] == 'none; exact original bytes'
    assert sha(original) == record['original_sha256']
    assert sha(archive) == record['archive_sha256']
    raw = gzip.decompress(archive.read_bytes())
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
    assert identity['source_scope'] == SOURCE_SCOPE
    assert identity['source_sha256'] == source
    assert identity['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
    assert identity['scenario'] == 'actual-host-boundary-v1'
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
            'host_checkpoint::tests::m2a15_cost_probe', '--nocapture', '--test-threads=1']
        environment = record['environment_overrides']
        assert environment['CATHEDRAL_HEADLESS'] == environment['CATHEDRAL_FAKE_BACKEND'] == '1'
        assert environment['ALIBI_HOST_MODE'] == mode
        assert environment['ALIBI_HOST_SAMPLES'] == str(samples)
        assert environment['ALIBI_HOST_OUTPUT'] == record['metrics']['original']
        for path, digest in record['helper_sha256'].items():
            assert sha(ROOT / path) == digest
        log = check_archive(record['log']).decode()
        assert re.search(r'test result: ok\. 1 passed; 0 failed;', log)
        # --nocapture permits ordinary engine diagnostics between the test
        # name and libtest's final `ok`; keep those bytes verbatim.
        assert log.count('test host_checkpoint::tests::m2a15_cost_probe ...') == 1
        time_log = check_archive(record['time']).decode()
        rss = re.search(r'Maximum resident set size \(kbytes\): (\d+)', time_log)
        assert rss and int(rss[1]) > 0
        assert re.search(r'Exit status: 0\s*$', time_log)
        data = read_json(check_archive(record['metrics']))
        check_metrics(data, mode, samples)
        shape = {key: data[key] for key in ('counts', 'placement', 'readable_counts', 'cost')}
        # Repeated processes use the same deterministic fixture. Differences
        # must be investigated instead of silently pooling unlike workloads.
        assert shapes.setdefault(mode, shape) == shape
        run = {'mode': mode, 'repetition': repetition, 'shape': shape,
               'readonly': data['readonly'], 'process_max_rss_kib': int(rss[1]),
               'elapsed_seconds': record['elapsed_seconds'], 'phases': {}}
        for phase in PHASES:
            values = data[phase]
            groups.setdefault((mode, phase), []).extend(values)
            run['phases'][phase] = distribution(values)
            for index, value in enumerate(values):
                if value > 2000:
                    tails.append({'mode': mode, 'repetition': repetition, 'phase': phase,
                                  'index': index, 'milliseconds': value / 1000})
        runs.append(run)
    assert seen == {(mode, rep) for mode in ('authored', 'populated')
                    for rep in range(1, repetitions + 1)}
    return {'result': 'integrity passed', 'dataset': str(dataset.relative_to(ROOT)),
            'source_manifest_sha256': identity['source_manifest_sha256'],
            'binary_sha256': identity['binary_sha256'],
            'raw_phase_samples': len(records) * samples * len(PHASES),
            'scope': 'Host component only; no complete capture/frame/Running budget claim.',
            'runs': runs, 'pooled': {mode: {phase: distribution(groups[(mode, phase)])
                for phase in PHASES} for mode in ('authored', 'populated')},
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
        ('result', 'dataset', 'raw_phase_samples', 'pooled')}))
