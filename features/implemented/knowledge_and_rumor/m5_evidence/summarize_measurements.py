#!/usr/bin/env -S uv run --script
"""Verify and summarize the saved final runs; no simulation or provider calls.

With no arguments, reads the raw /tmp measurement directory and archives it here.
After landing, --raw <this directory> also accepts the archived .gz files.
"""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import re

parser = argparse.ArgumentParser()
parser.add_argument('--raw', type=Path, default=Path('/tmp/knowledge-m5-measurements'))
parser.add_argument('--output', type=Path, default=Path(__file__).resolve().parent)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)


def read(name):
    path = args.raw / name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_name(name + '.gz').read_bytes())


def archive(name):
    data = read(name)
    if name.endswith(('.log', '.pollen')):
        (args.output / (name + '.gz')).write_bytes(gzip.compress(data, mtime=0))
    else:
        (args.output / name).write_bytes(data)


def timed(name):
    data = read(name + '.time').decode()
    def field(label):
        return float(re.search(re.escape(label) + r': ([\d.]+)', data)[1])
    assert field('Exit status') == 0, name
    return {'user_seconds': field('User time (seconds)'),
            'system_seconds': field('System time (seconds)'),
            'peak_rss_kib': int(field('Maximum resident set size (kbytes)'))}


runs = []
for phase in ('band', 'identity', 'crowd'):
    archive(phase + '.json')
    for record in json.loads(read(phase + '.json')):
        assert record['returncode'] == 0, record
        name = record['name']
        assert hashlib.sha256(read(name + '.pollen')).hexdigest() == record['pollen_sha256'], name
        for suffix in ('.log', '.pollen', '.time'):
            archive(name + suffix)
        runs.append(record | timed(name))
assert len({row['binary_sha256'] for row in runs}) == 1

topic_pattern = re.compile(
    r'^\[pollen\] (\w+) pollen\.\w+ mint (\w+) wards (\d+)/8 carriers (\d+) '
    r'warm (\d+) expect ([\d.]+) exits (\d+) \(same (\d+), warm-same-any (\d+)\) '
    r'hops ([\d.]+) heat ([\d.]+) age ([\d.]+)gh by (.*)$', re.M)
samples = []
for match in topic_pattern.finditer(read('band.pollen').decode()):
    topic, ward, reached, carriers, warm, expectation, exits, same, warm_same, hops, heat, age, by = match.groups()
    by_parts = by.split()
    samples.append({'topic': topic, 'mint_ward': ward, 'wards_reached': int(reached),
                    'carriers': int(carriers), 'warm': int(warm), 'expected_crossings': float(expectation),
                    'exits': int(exits), 'same_trade_exits': int(same), 'warm_same_trade_exits': int(warm_same),
                    'hops': float(hops), 'heat': float(heat), 'age_game_hours': float(age),
                    'carriers_by_ward': dict(zip(by_parts[::2], map(int, by_parts[1::2])))})
bed = [r for r in samples if r['topic'] == 'bed']
craft = [r for r in samples if r['topic'] == 'craft']
assert len(bed) == len(craft) == 48
at = lambda rows, age: next(r for r in rows if r['age_game_hours'] >= age)
assert at(bed, 1)['wards_reached'] < 8
assert at(bed, 6.5)['wards_reached'] >= 2
assert at(bed, 9.5)['wards_reached'] >= 4
assert bed[-1]['wards_reached'] == 8
assert all(r['wards_reached'] == 1 for r in craft)
assert 0 < craft[-1]['expected_crossings'] < 1
confined = math.exp(-at(craft, 11)['expected_crossings'])
assert confined > .5
assert read('base.pollen') == read('flat.pollen')

crowd = []
census_pattern = re.compile(
    r'^\[pollen\] t=([\d.]+)gd facts (\d+) holdings (\d+) air (\d+) '
    r'store ([\d.]+)KB rolls/gh\(bound\) ([\d.]+)', re.M)
for row in runs:
    if not row['name'].startswith('crowd-'):
        continue
    log = read(row['name'] + '.log').decode()
    censuses = census_pattern.findall(log)
    assert len(censuses) == 25, row['name']
    final = censuses[-1]
    assert abs(float(final[0]) - float(censuses[0][0]) - 1) < .0001
    crowd.append(row | {'extra_ambient': int(row['name'].split('-')[1]),
                       'knowledge': row['name'].endswith('-on'),
                       'facts': int(final[1]), 'holdings': int(final[2]), 'air': int(final[3]),
                       'final_store_kib': float(final[4]), 'rolls_per_game_hour_bound': float(final[5]),
                       'max_store_kib': max(float(c[4]) for c in censuses),
                       'max_auxiliary_bytes': max(map(int, re.findall(
                           r'\[pollen-aux\] consequence caches (\d+) bytes', log)))})
deltas = []
for count in (0, 1000, 20000):
    on, off = [r for r in crowd if r['extra_ambient'] == count]
    assert on['knowledge'] and not off['knowledge']
    delta = 100 * (on['user_seconds'] / off['user_seconds'] - 1)
    rss = (on['peak_rss_kib'] - off['peak_rss_kib']) / 1024
    assert delta < 15
    if count == 20000:
        assert rss < 32 and on['max_store_kib'] / 1024 < 32
    deltas.append({'extra_ambient': count, 'user_cpu_delta_percent': delta, 'peak_rss_delta_mib': rss})

manual = {}
for name in ('ignored-20000', 'same-trade'):
    log = read(name + '.log').decode()
    assert 'test result: ok. 1 passed; 0 failed;' in log
    for suffix in ('.log', '.time'):
        archive(name + suffix)
    manual[name] = timed(name)
guard = read('ignored-20000.log').decode()
guard_bytes = int(re.search(r'\[pollen\] footprint at 20000 extra ambient \(20520 bodies\): (\d+) bytes', guard)[1])
assert guard_bytes < 32 * 1024 * 1024
manual['ignored-20000']['store_bytes'] = guard_bytes
summary = {'status': 'passed', 'free_parameters_changed': False,
           'runs': runs, 'band_samples': samples,
           'bed_first_eight_wards': next(r for r in bed if r['wards_reached'] == 8),
           'bed_final': bed[-1], 'craft_final': craft[-1], 'craft_night': at(craft, 11),
           'night_confinement_probability': confined,
           'identity_lines': len(read('base.pollen').splitlines()),
           'identity_sha256': hashlib.sha256(read('base.pollen')).hexdigest(),
           'crowd': crowd, 'crowd_deltas': deltas, 'manual_tests': manual}
(args.output / 'RESULTS.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps({'status': summary['status'], 'crowd_deltas': deltas,
                  'guard_store_bytes': guard_bytes, 'identity_lines': summary['identity_lines']}, indent=2))
