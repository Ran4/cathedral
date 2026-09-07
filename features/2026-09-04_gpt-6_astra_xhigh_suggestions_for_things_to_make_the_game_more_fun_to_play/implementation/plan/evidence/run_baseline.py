#!/usr/bin/env -S uv run --script
"""M0 paired, provider-free Engine::poll baseline; build the example first.

Records raw poll samples, process time/RSS, hardware, content/source hashes and
same-binary A/B variation. No actual checkpoint or renderer exists in this probe.
"""
import argparse
import gzip
import hashlib
import json
import math
import os
import platform
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
OUT = Path(__file__).resolve().parent / 'm0_baseline'
BINARY = ROOT / 'target/release/examples/alibi_baseline'


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def digest(path):
    return hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()


def percentiles(values):
    ordered = sorted(values)
    return {name: ordered[max(0, math.ceil(len(ordered) * q) - 1)]
            for name, q in [('p50', .50), ('p95', .95), ('p99', .99), ('max', 1)]}


def source():
    paths = command('/usr/bin/git', 'ls-files', 'Cargo*', 'config.ron', 'src',
                    'crates', 'assets/world', 'assets/prompts', 'assets/sounds/catalog.toml',
                    'lore/characters', 'lore/core_lore/occupations.json').splitlines()
    paths += ['crates/cathedral-backends/examples/alibi_baseline.rs']
    return {p: digest(ROOT / p) for p in sorted(set(paths)) if (ROOT / p).is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--smoke', action='store_true')
    parser.add_argument('--output-dir', type=Path)
    args = parser.parse_args()
    out = args.output_dir or (Path(tempfile.mkdtemp(prefix='alibi-m0-smoke-')) if args.smoke else OUT)
    out.mkdir(parents=True, exist_ok=True)
    if (out / 'RESULTS.json').exists():
        raise RuntimeError(f'{out} already has results; choose a fresh --output-dir')
    before = source()
    identity = {
        'schema': 1, 'head': command('/usr/bin/git', 'rev-parse', 'HEAD'),
        'status': command('/usr/bin/git', 'status', '--short'),
        'platform': platform.platform(), 'cpu': command('lscpu'),
        'meminfo': Path('/proc/meminfo').read_text(),
        'rustc': command('rustc', '-Vv'),
        'binary_sha256': digest(BINARY), 'source_sha256': before,
        'runner_sha256': digest(Path(__file__)),
        'cgroup': {str(path): path.read_text().strip() if path.exists() else None for path in
                   [Path('/sys/fs/cgroup/cpu.max'), Path('/sys/fs/cgroup/memory.max'),
                    Path('/sys/fs/cgroup/cpuset.cpus.effective')]},
        'build': 'cargo build --release -p cathedral-backends --example alibi_baseline',
        'build_environment': {key: os.environ.get(key) for key in
                              ['CARGO_BUILD_JOBS', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
                               'CARGO_TARGET_DIR', 'CARGO_PROFILE_RELEASE_OPT_LEVEL']},
        'measurement': 'single-process sequential release pairs; elapsed microseconds per poll; no renderer; real IO excluded from timed poll',
    }
    workloads = [(0, True), (0, False), (1000, False), (2000, False), (20000, False)]
    if args.smoke:
        workloads = [(0, False)]
    rows = []
    for extra, idle in workloads:
        for pair in range(1 if extra == 20000 or args.smoke else 3):
            for side in ['a', 'b']:
                name = f'{extra}-{"idle" if idle else "market"}-{pair + 1}{side}'
                raw = out / f'{name}.json'
                timing = out / f'{name}.time'
                cmd = [str(BINARY), '--extra', str(extra), '--polls',
                       str(20 if args.smoke else 1000 if extra == 20000 else 1200),
                       '--warmup', str(10 if args.smoke else 100),
                       '--output', str(raw)] + (['--idle'] if idle else [])
                process = subprocess.run(['/usr/bin/time', '-v', '-o', str(timing), *cmd],
                                         cwd=ROOT, capture_output=True, text=True, timeout=600)
                if process.returncode:
                    raise RuntimeError(f'{name}: {process.stderr}')
                data = json.loads(raw.read_text())
                times = {}
                for line in timing.read_text().splitlines():
                    for label in ['User time (seconds)', 'System time (seconds)',
                                  'Maximum resident set size (kbytes)']:
                        if line.strip().startswith(label + ':'):
                            times[label] = float(line.split(':', 1)[1].strip())
                row = {k: v for k, v in data.items() if k != 'poll_us'}
                row.update(name=name, pair=pair + 1, side=side, command=cmd,
                           poll_us=percentiles(data['poll_us']), process=times,
                           sample_sum_us=sum(data['poll_us']))
                rows.append(row)
                with gzip.GzipFile(str(raw) + '.gz', 'wb', mtime=0) as archive:
                    archive.write(raw.read_bytes())
                raw.unlink()
                print(json.dumps({k: row[k] for k in ['name', 'poll_us', 'process',
                                                     'measured_speech_events', 'moved_actors_over_0_1m']}), flush=True)
                (out / 'RESULTS.json').write_text(json.dumps(rows, indent=2) + '\n')
    after = source()
    if after != before or digest(BINARY) != identity['binary_sha256']:
        raise RuntimeError('source or binary changed during measurements')
    identity['unchanged_source_and_binary_at_end'] = True
    (out / 'IDENTITY.json').write_text(json.dumps(identity, indent=2) + '\n')
    paired = []
    for a, b in zip(rows[::2], rows[1::2]):
        counters = ['actors', 'placement', 'moved_actors_over_0_1m', 'message_count', 'measured_speech_events',
                    'snapshot_publications', 'snapshot_bytes_final', 'knowledge_bytes_max',
                    'cognition_calls_including_warmup', 'prompt_bytes_max']
        if any(a[key] != b[key] for key in counters):
            raise RuntimeError(f'paired semantic counters differ: {a["name"]}, {b["name"]}')
        paired.append({'a': a['name'], 'b': b['name'],
                       'semantic_counters_equal': True,
                       'percent_change_b_vs_a': {p: (b['poll_us'][p] / a['poll_us'][p] - 1) * 100
                                                  for p in ['p50', 'p95', 'p99', 'max']}})
    (out / 'PAIRS.json').write_text(json.dumps(paired, indent=2) + '\n')


if __name__ == '__main__':
    main()
