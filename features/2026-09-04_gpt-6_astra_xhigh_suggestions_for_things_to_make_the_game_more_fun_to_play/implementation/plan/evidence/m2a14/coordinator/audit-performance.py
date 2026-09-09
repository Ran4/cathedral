from pathlib import Path
import subprocess, json, argparse

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
output = here / 'm2a14/coordinator'
datasets = [
    ('scheduler_smoke', 'm2a14/smoke/scheduler', 'scheduler'),
    ('night_smoke', 'm2a14/smoke/night', 'night'),
    ('scheduler_performance', 'm2a14/performance/scheduler', 'scheduler'),
    ('night_performance', 'm2a14/performance/night', 'night'),
    ('historical_backbone', 'm2a2/performance', None),
    ('historical_round', 'm2a3/performance/round', None),
    ('historical_climate', 'm2a4/performance/climate', None),
    ('historical_knowledge', 'm2a5/performance', None),
    ('historical_law', 'm2a6/performance', None),
    ('historical_marks', 'm2a7/performance', None),
    ('historical_animals', 'm2a8/performance', None),
    ('historical_night', 'm2a9/performance', None),
    ('historical_social', 'm2a10/performance', None),
    ('historical_scheduler', 'm2a11/performance', None),
    ('historical_continuity', 'm2a12/performance', None),
    ('historical_speech', 'm2a13/performance', None),
]
parser = argparse.ArgumentParser()
which = parser.add_mutually_exclusive_group()
which.add_argument('--historical-only', action='store_true')
which.add_argument('--current-only', action='store_true')
args = parser.parse_args()
for name, path, owner in datasets:
    if args.historical_only and owner or args.current_only and not owner:
        continue
    report = output / (name + '_audit.json')
    assert not report.exists()
    command = ['/home/ran/.local/bin/uv', 'run', '--no-project', '--cache-dir', '/tmp/alibi-uv', str(here / 'audit_m2_component_probes.py'), str(here / path), '--report', str(report)]
    if owner:
        command += ['--check-current', str(root / 'target/release/examples' / f'alibi_{owner}_cost')]
    result = subprocess.run(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert result.returncode == 0, result.stdout.decode()
    value = json.loads(report.read_text())
    assert value['result'] == 'passed'
    print(json.dumps({'dataset': path, 'result': 'passed', 'raw_phase_samples': value['raw_phase_samples']}), flush=True)
