# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Compare M0 and current release Engine::poll under balanced sequential pairs.

Build current alibi_baseline first, then stop source edits and other compilation.
This measures ordinary simulation, not the active M13 mix or renderer costs.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[5]
EVIDENCE = ROOT / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
REFERENCE_SHA = 'f7ba972a440a2d6d2690c4dd2afd230363e5778ebf2c3fe7d62d4becca3e9057'
COUNTERS = ['actors', 'placement', 'moved_actors_over_0_1m', 'message_count',
            'measured_speech_events', 'snapshot_publications', 'snapshot_bytes_final',
            'knowledge_bytes_max', 'cognition_calls_including_warmup', 'prompt_bytes_max']


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def sources():
    paths = command('/usr/bin/git', 'ls-files', '--cached', '--others', '--exclude-standard',
                    '--', 'Cargo*', 'config.ron', 'src', 'crates', 'assets/world',
                    'assets/prompts', 'assets/sounds/catalog.toml', 'lore/characters',
                    'lore/core_lore/occupations.json').splitlines()
    return {p: digest(ROOT / p) for p in sorted(set(paths)) if (ROOT / p).is_file()}


def percentiles(values):
    values = sorted(values)
    return {name: values[math.ceil(len(values) * quantile) - 1]
            for name, quantile in [('p50', .5), ('p95', .95), ('p99', .99), ('max', 1)]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, default=Path('/tmp/alibi-m0-reference-binary'))
    parser.add_argument('--current', type=Path, default=ROOT / 'target/release/examples/alibi_baseline')
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--smoke', action='store_true')
    args = parser.parse_args()
    binaries = {'m0': args.reference.resolve(), 'm1': args.current.resolve()}
    assert digest(binaries['m0']) == REFERENCE_SHA, 'reference executable differs from M0 identity'
    baseline = json.loads((EVIDENCE / 'm0_baseline/IDENTITY.json').read_text())
    before = sources()
    content = {p: v for p, v in baseline['source_sha256'].items()
               if p.startswith(('assets/', 'lore/'))}
    assert all(before[p] == expected for p, expected in content.items()), 'M0 content inputs changed'
    harness = 'crates/cathedral-backends/examples/alibi_baseline.rs'
    assert before[harness] == baseline['source_sha256'][harness], 'comparison harness changed'
    args.output_dir.mkdir(parents=True, exist_ok=True)
    if any(args.output_dir.iterdir()):
        raise RuntimeError('choose an empty output directory')
    identity = {
        'schema': 1, 'head': command('/usr/bin/git', 'rev-parse', 'HEAD'),
        'status': command('/usr/bin/git', 'status', '--short'),
        'platform': platform.platform(), 'cpu': command('/usr/bin/lscpu'),
        'meminfo': Path('/proc/meminfo').read_text(),
        'rustc': command('/home/ran/.cargo/bin/rustc', '-Vv'),
        'source_sha256': before,
        'binary_sha256': {k: digest(p) for k, p in binaries.items()},
        'reference_identity': 'm0_baseline/IDENTITY.json',
        'matched_content_files': len(content), 'runner_sha256': digest(__file__),
        'build_environment': {k: os.environ.get(k) for k in ['RUSTFLAGS',
            'CARGO_ENCODED_RUSTFLAGS', 'CARGO_TARGET_DIR', 'CARGO_PROFILE_RELEASE_OPT_LEVEL']},
        'measurement': 'Sequential paired release Engine::poll CPU samples; alternating old/new order; no renderer; unchanged M0 harness and content.',
        'smoke_only': args.smoke,
    }
    workloads = [(0, True), (0, False), (1000, False), (2000, False), (20000, False)]
    if args.smoke:
        workloads = [(0, False)]
    rows = []
    pairs = []
    for extra, idle in workloads:
        workload = f'{extra}-{"idle" if idle else "market"}'
        for pair in range(1 if extra == 20000 or args.smoke else 3):
            order = ['m0', 'm1'] if pair % 2 == 0 else ['m1', 'm0']
            by_binary = {}
            for side in order:
                name = f'{workload}-{pair + 1}-{side}'
                raw = args.output_dir / f'{name}.json'
                timing = args.output_dir / f'{name}.time'
                stderr = args.output_dir / f'{name}.stderr'
                cmd = [str(binaries[side]), '--extra', str(extra), '--polls',
                       str(20 if args.smoke else 1000 if extra == 20000 else 1200),
                       '--warmup', str(10 if args.smoke else 100), '--output', str(raw)]
                if idle:
                    cmd.append('--idle')
                with stderr.open('w') as error_log:
                    completed = subprocess.run(['/usr/bin/time', '-v', '-o', str(timing), *cmd],
                        cwd=ROOT, stdout=subprocess.DEVNULL, stderr=error_log, timeout=600)
                if completed.returncode:
                    raise RuntimeError(f'{name} exited {completed.returncode}; inspect {stderr}')
                data = json.loads(raw.read_text())
                times = {}
                for line in timing.read_text().splitlines():
                    for label in ['User time (seconds)', 'System time (seconds)', 'Maximum resident set size (kbytes)']:
                        if line.strip().startswith(label + ':'):
                            times[label] = float(line.split(':', 1)[1].strip())
                row = {k: v for k, v in data.items() if k != 'poll_us'}
                row.update(name=name, workload=workload, pair=pair + 1, binary=side,
                    command=cmd, poll_us=percentiles(data['poll_us']), process=times,
                    sample_sum_us=sum(data['poll_us']), sample_count=len(data['poll_us']))
                with gzip.GzipFile(str(raw) + '.gz', 'wb', mtime=0) as archive:
                    archive.write(raw.read_bytes())
                raw.unlink()
                rows.append(row)
                by_binary[side] = row
                print(json.dumps({k: row[k] for k in ['name', 'poll_us', 'process']}), flush=True)
                (args.output_dir / 'RESULTS.json').write_text(json.dumps(rows, indent=2) + '\n')
            old, new = by_binary['m0'], by_binary['m1']
            assert old['actors'] == new['actors'] and old['placement'] == new['placement']
            pairs.append({'workload': workload, 'pair': pair + 1, 'order': order,
                'delta_us': {p: new['poll_us'][p] - old['poll_us'][p] for p in old['poll_us']},
                'percent_change': {p: (new['poll_us'][p] / old['poll_us'][p] - 1) * 100 for p in old['poll_us']},
                'counter_differences': {k: {'m0': old[k], 'm1': new[k]} for k in COUNTERS if old[k] != new[k]}})
    assert sources() == before, 'source changed during measurements'
    assert {k: digest(p) for k, p in binaries.items()} == identity['binary_sha256'], 'executable changed during measurements'
    summary = {}
    for workload in sorted({row['workload'] for row in rows}):
        group = [p for p in pairs if p['workload'] == workload]
        for side in binaries:
            repeats = [r for r in rows if r['workload'] == workload and r['binary'] == side]
            for repeated in repeats[1:]:
                assert all(repeated[k] == repeats[0][k] for k in COUNTERS), \
                    f'non-repeatable semantic counters in {workload}/{side}'
        summary[workload] = {
            'pairs': len(group),
            'same_binary_semantic_counters_repeat': True,
            'per_binary_median_run_us': {side: {p: statistics.median(r['poll_us'][p] for r in rows if r['workload'] == workload and r['binary'] == side)
                for p in ['p50', 'p95', 'p99', 'max']} for side in binaries},
            'median_paired_delta_us': {p: statistics.median(r['delta_us'][p] for r in group) for p in ['p50', 'p95', 'p99', 'max']},
            'median_paired_percent_change': {p: statistics.median(r['percent_change'][p] for r in group) for p in ['p50', 'p95', 'p99', 'max']},
        }
    identity['unchanged_source_and_binaries_at_end'] = True
    for filename, data in [('IDENTITY.json', identity), ('PAIRS.json', pairs), ('SUMMARY.json', summary)]:
        (args.output_dir / filename).write_text(json.dumps(data, indent=2) + '\n')


if __name__ == '__main__':
    main()
