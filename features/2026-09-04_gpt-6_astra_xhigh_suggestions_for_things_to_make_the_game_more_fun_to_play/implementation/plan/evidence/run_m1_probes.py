# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Measure M1 publication accounting and 0/16 timed fixtures on a stable build.

Run only after release builds finish and other compilation/benchmarks stop.
These probes do not measure rendering or the full M13 foundation workload.
"""
from __future__ import annotations

import argparse
from collections import Counter
import gzip
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess

from run_m1_comparison import ROOT, command, digest, percentiles, sources


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--smoke', action='store_true')
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    if any(args.output_dir.iterdir()):
        raise RuntimeError('choose an empty output directory')
    binaries = {name: ROOT / f'target/release/examples/alibi_{name}_cost'
                for name in ('publication', 'operation')}
    before = sources()
    identity = {
        'schema': 1, 'head': command('/usr/bin/git', 'rev-parse', 'HEAD'),
        'status': command('/usr/bin/git', 'status', '--short'),
        'platform': platform.platform(), 'cpu': command('/usr/bin/lscpu'),
        'meminfo': Path('/proc/meminfo').read_text(),
        'rustc': command('/home/ran/.cargo/bin/rustc', '-Vv'),
        'source_sha256': before,
        'binary_sha256': {k: digest(p) for k, p in binaries.items()},
        'runner_sha256': digest(__file__),
        'build_environment': {k: os.environ.get(k) for k in ['RUSTFLAGS',
            'CARGO_ENCODED_RUSTFLAGS', 'CARGO_TARGET_DIR', 'CARGO_PROFILE_RELEASE_OPT_LEVEL']},
        'smoke_only': args.smoke,
        'limits': 'CPU-only. Publication: hot repeated accounting traversal, excluding snapshot construction/encoding, queues and consumers. Operations: stationary timed fixtures and shared needs cadence, excluding M13 adapter mix and renderer.',
    }
    rows = []

    def run(probe, name, options, samples_key):
        raw = args.output_dir / f'{name}.json'
        timing = args.output_dir / f'{name}.time'
        stderr = args.output_dir / f'{name}.stderr'
        cmd = [str(binaries[probe]), *options, '--output', str(raw)]
        with stderr.open('w') as errors:
            completed = subprocess.run(['/usr/bin/time', '-v', '-o', str(timing), *cmd],
                cwd=ROOT, stdout=subprocess.DEVNULL, stderr=errors, timeout=600)
        if completed.returncode:
            raise RuntimeError(f'{name} exited {completed.returncode}; inspect {stderr}')
        data = json.loads(raw.read_text())
        values = data[samples_key]
        row = {k: v for k, v in data.items() if k not in
               (samples_key, 'active_counts', 'receipts', 'transition_trace', 'admission_refusals')}
        row.update(name=name, command=cmd, sample_count=len(values),
                   sample_us=percentiles(values), sample_sum_us=sum(values))
        row['process'] = {}
        for line in timing.read_text().splitlines():
            for label in ['User time (seconds)', 'System time (seconds)',
                          'Maximum resident set size (kbytes)']:
                if line.strip().startswith(label + ':'):
                    row['process'][label] = float(line.split(':', 1)[1].strip())
        if probe == 'operation':
            row['active_min'] = min(data['active_counts'])
            row['active_max'] = max(data['active_counts'])
            row['admission_refusal_count'] = len(data['admission_refusals'])
            row['receipt_states'] = [r['outcome']['state'] for r in data['receipts']]
            row['transition_codes'] = dict(Counter(r['outcome']['code'] for r in data['transition_trace']))
        with gzip.GzipFile(str(raw) + '.gz', 'wb', mtime=0) as archive:
            archive.write(raw.read_bytes())
        raw.unlink()
        rows.append(row)
        (args.output_dir / 'RESULTS.json').write_text(json.dumps(rows, indent=2) + '\n')
        print(json.dumps({k: row[k] for k in ('name', 'sample_us', 'process')}), flush=True)
        return row

    repeats = 1 if args.smoke else 3
    populations = [0] if args.smoke else [0, 2000]
    summary = {'publication': {}, 'operation': {}}
    for extra in populations:
        group = [run('publication', f'publication-{extra}-{repeat + 1}',
                     ['--extra', str(extra), '--samples', str(100 if args.smoke else 5000)],
                     'samples_us') for repeat in range(repeats)]
        for row in group[1:]:
            assert all(row[k] == group[0][k] for k in
                       ('actor_count', 'encoded_snapshot_bytes', 'charged_message_bytes'))
        summary['publication'][str(extra)] = {
            'runs': repeats, 'sample_count': sum(r['sample_count'] for r in group),
            'actor_count': group[0]['actor_count'],
            'encoded_snapshot_bytes': group[0]['encoded_snapshot_bytes'],
            'charged_message_bytes': group[0]['charged_message_bytes'],
            'median_run_us': {p: statistics.median(r['sample_us'][p] for r in group)
                              for p in ('p50', 'p95', 'p99', 'max')},
        }
    for extra in populations:
        pairs = []
        by_active = {0: [], 16: []}
        for pair in range(repeats):
            order = [0, 16] if pair % 2 == 0 else [16, 0]
            measured = {}
            for active in order:
                row = run('operation', f'operation-{extra}-{pair + 1}-{active}',
                          ['--extra', str(extra), '--active', str(active),
                           '--polls', str(20 if args.smoke else 1200),
                           '--warmup', str(10 if args.smoke else 300),
                           '--step-seconds', str(1 / 60)], 'poll_us')
                assert row['admitted'] == active
                assert row['active_min'] == active == row['active_max'], \
                    f'active fixture ended during measured interval: {row["name"]}'
                assert row['completed_units'] == 0
                assert row['placement']['placed'] == extra
                by_active[active].append(row)
                measured[active] = row
            assert measured[0]['total_actors'] == measured[16]['total_actors']
            assert measured[0]['placement'] == measured[16]['placement']
            pairs.append({'pair': pair + 1, 'order': order,
                'delta_us': {p: measured[16]['sample_us'][p] - measured[0]['sample_us'][p]
                             for p in ('p50', 'p95', 'p99', 'max')},
                'counter_differences': {k: {'empty': measured[0][k], 'active16': measured[16][k]}
                    for k in ('moved_actors', 'kernel_heap_upper_bound')
                    if measured[0][k] != measured[16][k]}})
        for active, group in by_active.items():
            for row in group[1:]:
                assert all(row[k] == group[0][k] for k in
                    ('admitted', 'completed_units', 'moved_actors', 'kernel_heap_upper_bound',
                     'total_actors', 'placement', 'active_min', 'active_max',
                     'admission_refusal_count', 'receipt_states', 'transition_codes'))
        summary['operation'][str(extra)] = {
            'pairs': pairs,
            'median_run_us': {str(active): {p: statistics.median(r['sample_us'][p] for r in group)
                for p in ('p50', 'p95', 'p99', 'max')} for active, group in by_active.items()},
            'median_paired_delta_us': {p: statistics.median(r['delta_us'][p] for r in pairs)
                                      for p in ('p50', 'p95', 'p99', 'max')},
            'same_binary_semantic_counters_repeat': True,
        }
    trace = run('operation', 'operation-completion-trace',
                ['--extra', '0', '--active', '16', '--polls', '20', '--warmup', '0',
                 '--work-seconds', '0.15'], 'poll_us')
    assert trace['completed_units'] == 16 and trace['active_min'] == 0
    assert all(trace['transition_codes'].get(code) == 16 for code in
               ('operation_accepted', 'operation_running', 'operation_completed'))
    assert sources() == before, 'source changed during measurements'
    assert {k: digest(p) for k, p in binaries.items()} == identity['binary_sha256'], \
        'executable changed during measurements'
    identity['unchanged_source_and_binaries_at_end'] = True
    for filename, data in [('IDENTITY.json', identity), ('SUMMARY.json', summary)]:
        (args.output_dir / filename).write_text(json.dumps(data, indent=2) + '\n')


if __name__ == '__main__':
    main()
