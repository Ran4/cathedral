#!/usr/bin/env -S uv run --script
"""Run the M5 knell, ward-map, journal and door UI shots on a monitored unmapped X11 window."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import time

root = Path('/home/ran/src/rust/cathedralbevy')
out = Path('/tmp/knowledge-m5-visuals-ui')
out.mkdir(exist_ok=True)
drives = {
    # Be inside ordinary witness earshot before the bell: this verifies the real
    # mint receipt without relying on a probabilistic ward pickup during a shot.
    'knell': 'wait-online; weather clear; tp -140.5 20 -270 0; sleep 4; bell knell 17; sleep 15; key KeyM; sleep 2; shot ward_heat; key KeyM; key KeyJ; sleep 2; shot knell_journal; key KeyJ; quit',
    # Hamel begins 5.57 m from his own door and the night-watch round is home
    # at Dayspring. This tests the actual distance predicate without depending
    # on wall-clock sleeps to advance a slow software-rendered simulation.
    'door': 'wait-online; weather clear; tp -52.4 1 -378.1 180; raise-word Hamel Skep -> stranger Player chalked the Bellstand wall; sleep 12; shot shut_door; quit',
}
# UI-only fallback: symlink the actual UI assets, omit costly 3D textures on llvmpipe.
ui_assets = Path('/tmp/cathedral-m5-ui-assets/assets')
(ui_assets / 'textures').mkdir(parents=True, exist_ok=True)
for dest, src in [(ui_assets / 'fonts', root / 'assets/fonts'),
                  (ui_assets / 'textures/city_map.png', root / 'assets/textures/city_map.png')]:
    if not dest.exists():
        dest.symlink_to(src)
results = []
parser = argparse.ArgumentParser()
parser.add_argument('drives', choices=list(drives), nargs='*', default=list(drives))
selected = parser.parse_args().drives
if (out / 'RUN.json').exists():
    results = [row for row in json.loads((out / 'RUN.json').read_text()) if row['drive'] not in selected]
for name, drive in drives.items():
    if name not in selected:
        continue
    inherited = {key: value for key, value in os.environ.items()
                 if key not in {'CATHEDRAL_NO_KNOWLEDGE', 'CATHEDRAL_POLLEN_FLAT',
                                'CATHEDRAL_POLLEN_NO_SALIENCE'}}
    env = {**inherited, 'CATHEDRAL_HEADLESS': '1', 'CATHEDRAL_FAKE_BACKEND': '1',
           'CATHEDRAL_DRIVE_TIMEOUT': '180', 'CATHEDRAL_DRIVE': drive,
           'BEVY_ASSET_ROOT': '/tmp/cathedral-m5-ui-assets', 'CATHEDRAL_DRIVE_RES': '960x540',
           'CATHEDRAL_EXTRA_NPCS': '0'}
    cwd = root
    overrides = {}
    if name == 'door':
        # Make the door, rather than the separate novelty/curiosity cost gate,
        # decide this idle turn. The player's real config is never rewritten.
        cwd = Path('/tmp/cathedral-m5-door-fixture')
        cwd.mkdir(exist_ok=True)
        config = (root / 'config.ron').read_text()
        assert config.count('require_news: true') == 1
        (cwd / 'config.ron').write_text(config.replace('require_news: true', 'require_news: false'))
        overrides = {'smart_actors.idle_cognition.require_news': False}
    start = time.monotonic()
    samples = []
    with (out / f'{name}.log').open('w') as log:
        process = subprocess.Popen([str(root / 'target/debug/cathedralbevy')], cwd=cwd, env=env,
                                   stdout=log, stderr=subprocess.STDOUT)
        while process.poll() is None:
            found = subprocess.run(['xdotool', 'search', '--pid', str(process.pid)],
                                   capture_output=True, text=True)
            focus = subprocess.run(['xdotool', 'getwindowfocus'], capture_output=True, text=True).stdout.strip()
            for window in found.stdout.splitlines():
                info = subprocess.run(['xwininfo', '-id', window], capture_output=True, text=True).stdout
                state = next((line.strip() for line in info.splitlines() if 'Map State:' in line), 'missing')
                samples.append({'window': window, 'state': state, 'focused': window == focus})
                if state != 'Map State: IsUnMapped' or window == focus:
                    process.terminate()
                    raise RuntimeError(f'Window visibility check failed: {samples[-1]}')
            if time.monotonic() - start > 240:
                process.terminate()
                raise RuntimeError(f'{name} exceeded timeout')
            time.sleep(0.5)
    transcript = (out / f'{name}.log').read_text()
    match = re.search(r'logging to ([^\n]+)', transcript)
    if not match: raise RuntimeError('No session path in process log')
    session = Path(match[1].strip())
    observed = ('[bell] smallvoice knell: 17 strokes' in transcript and 'Maren Smallvoice counts 17:' in transcript
                if name == 'knell' else 'is at their own door and does not open to' in transcript)
    row = {'drive': name, 'returncode': process.returncode, 'seconds': round(time.monotonic()-start, 2),
           'script': drive, 'cwd': str(cwd), 'config_overrides': overrides, 'trigger_observed': observed,
           'binary_sha256': hashlib.sha256((root / 'target/debug/cathedralbevy').read_bytes()).hexdigest(),
           'session': str(session), 'window_samples': samples,
           'screenshots': [str(p) for p in (session / 'screenshots').glob('*.png')]}
    results.append(row)
    (out / 'RUN.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps({k: v for k, v in row.items() if k != 'window_samples'}), flush=True)
    if process.returncode != 0:
        raise RuntimeError(f'{name} failed; see {out / (name + ".log")}')
    if not samples:
        raise RuntimeError(f'{name} had no X11 visibility samples')
    if not observed:
        raise RuntimeError(f'{name}: the intended trigger was not observed; screenshots alone are insufficient')
