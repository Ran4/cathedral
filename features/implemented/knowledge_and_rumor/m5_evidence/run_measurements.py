#!/usr/bin/env -S uv run --script
"""Run the frozen M5 binary sequentially; preserve exact commands and raw output.

Build cathedral-headless in release mode first. Run `band`, assess the unchanged
M2 assertions, then run `identity`, then `crowd`. No source edits during a phase.
The crowd phase measures full game days at the coarse 3-second cost-guard step.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('phase', choices=['band', 'identity', 'crowd'])
args = parser.parse_args()
root = Path('/home/ran/src/rust/cathedralbevy')
out = Path('/tmp/knowledge-m5-measurements')
out.mkdir(exist_ok=True)
binary = root / 'target/release/cathedral-headless'
binary_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
common = [str(binary), '--fake', '--watch-clock', '1', '--seconds-per-day', '3600',
          '--start-office', 'dayspring', '--trace-pollen', '--pollen-seed', 'The Wickmarket']
if args.phase == 'band':
    runs = [('band', common + ['--pollen-per-day', '48'], {})]
elif args.phase == 'identity':
    runs = [(name, common + ['--pollen-per-day', '48', lever], {})
            for name, lever in [('base', '--pollen-no-salience'), ('flat', '--pollen-flat')]]
else:
    runs = [(f'crowd-{n}-{label}', common + ['--extra-ambient', str(n), '--pollen-step', '3',
             '--pollen-saturate'], env)
            for n in [0, 1000, 20000]
            for label, env in [('on', {}), ('off', {'CATHEDRAL_NO_KNOWLEDGE': '1'})]]
records = []
for name, command, changes in runs:
    # Compare identical defaults: an inherited developer ablation must not turn
    # an ON row into a second OFF row. The only run-specific ablation is recorded.
    env = {key: value for key, value in os.environ.items()
           if key not in {'CATHEDRAL_NO_KNOWLEDGE', 'CATHEDRAL_POLLEN_FLAT',
                          'CATHEDRAL_POLLEN_NO_SALIENCE'}}
    env.update(changes)
    started = time.monotonic()
    print(json.dumps({'starting': name, 'command': command, 'environment': changes}), flush=True)
    with (out / f'{name}.log').open('w') as log:
        result = subprocess.run(['/usr/bin/time', '-v', '-o', str(out / f'{name}.time'), *command],
                                cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
    log_text = (out / f'{name}.log').read_text()
    pollen = '\n'.join(line for line in log_text.splitlines() if line.startswith('[pollen]')) + '\n'
    (out / f'{name}.pollen').write_text(pollen)
    record = {'name': name, 'command': command, 'environment': changes,
              'binary_sha256': binary_sha256, 'returncode': result.returncode,
              'wall_seconds': round(time.monotonic() - started, 3),
              'pollen_sha256': hashlib.sha256(pollen.encode()).hexdigest()}
    records.append(record)
    (out / f'{args.phase}.json').write_text(json.dumps(records, indent=2) + '\n')
    print(json.dumps(record), flush=True)
    if result.returncode:
        raise SystemExit(f'{name} failed; see {out / (name + ".log")}')
    if hashlib.sha256(binary.read_bytes()).hexdigest() != binary_sha256:
        raise SystemExit('Binary changed while measuring')
if args.phase == 'identity':
    if (out / 'base.pollen').read_bytes() != (out / 'flat.pollen').read_bytes():
        raise SystemExit('Flat/no-salience pollen lines differ')
    print('Flat/no-salience identity: byte-identical pollen lines', flush=True)
