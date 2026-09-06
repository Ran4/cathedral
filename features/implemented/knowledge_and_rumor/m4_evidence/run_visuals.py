#!/usr/bin/env -S uv run --script
"""Run the required M4 UI shots on a monitored unmapped X11 window."""
import json
import os
import re
from pathlib import Path
import subprocess
import time

root = Path('/home/ran/src/rust/cathedralbevy')
out = Path('/tmp/knowledge-m4-visuals-ui')
out.mkdir(exist_ok=True)
drives = {
    'journal': 'wait-online; seed-fact ashe.salt.short; sleep 2; key KeyJ; sleep 1; shot journal; key KeyJ; quit',
    'lie': 'wait-online; frame Ilse 3; sleep 1; key Enter; type Grigor Ashe gave me short measure at the salt cellars yesterday; key Enter; sleep 20; shot lie_told; key KeyJ; sleep 1; shot lie_journal; quit',
    'stake': 'wait-online; raise-word Ilse -> bed the reeve wife was at the Bellstand after curfew; sleep 15; shot stake_line; quit',
    'pickup': 'wait-online; key KeyT; key KeyT; seed-fact ashe.salt.short -> wick; sleep 20; key KeyJ; sleep 1; shot picked_up; quit',
}
# UI-only fallback: symlink the actual UI assets, omit costly 3D textures on llvmpipe.
ui_assets = Path('/tmp/cathedral-m4-ui-assets/assets')
(ui_assets / 'textures').mkdir(parents=True, exist_ok=True)
for dest, src in [(ui_assets / 'fonts', root / 'assets/fonts'),
                  (ui_assets / 'textures/city_map.png', root / 'assets/textures/city_map.png')]:
    if not dest.exists():
        dest.symlink_to(src)
results = []
for name, drive in drives.items():
    env = {**os.environ, 'CATHEDRAL_HEADLESS': '1', 'CATHEDRAL_FAKE_BACKEND': '1',
           'CATHEDRAL_DRIVE_TIMEOUT': '180', 'CATHEDRAL_DRIVE': drive,
           'BEVY_ASSET_ROOT': '/tmp/cathedral-m4-ui-assets', 'CATHEDRAL_DRIVE_RES': '960x540',
           'CATHEDRAL_EXTRA_NPCS': '0'}
    start = time.monotonic()
    samples = []
    with (out / f'{name}.log').open('w') as log:
        process = subprocess.Popen([str(root / 'target/debug/cathedralbevy')], cwd=root, env=env,
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
    match = re.search(r'logging to ([^\n]+)', (out / f'{name}.log').read_text())
    if not match: raise RuntimeError('No session path in process log')
    session = Path(match[1].strip())
    row = {'drive': name, 'returncode': process.returncode, 'seconds': round(time.monotonic()-start, 2),
           'session': str(session), 'window_samples': samples,
           'screenshots': [str(p) for p in (session / 'screenshots').glob('*.png')]}
    results.append(row)
    (out / 'RUN.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps({k: v for k, v in row.items() if k != 'window_samples'}), flush=True)
    if process.returncode != 0:
        raise RuntimeError(f'{name} failed; see {out / (name + ".log")}')
    if not samples:
        raise RuntimeError(f'{name} had no X11 visibility samples')
