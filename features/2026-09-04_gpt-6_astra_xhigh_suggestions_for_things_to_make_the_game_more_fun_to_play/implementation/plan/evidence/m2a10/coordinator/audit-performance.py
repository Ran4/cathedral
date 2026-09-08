from pathlib import Path
import subprocess, json, argparse

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
output = here / 'm2a10/coordinator'
binary = root / 'target/release/examples/alibi_social_cost'
datasets = [
    ('social_smoke','m2a10/smoke',True),
    ('social_performance','m2a10/performance',True),
    ('historical_round','m2a3/performance/round',False),
    ('historical_climate','m2a4/performance/climate',False),
    ('historical_knowledge','m2a5/performance',False),
    ('historical_law','m2a6/performance',False),
    ('historical_marks','m2a7/performance',False),
    ('historical_animals','m2a8/performance',False),
    ('historical_night','m2a9/performance',False),
]
parser = argparse.ArgumentParser()
which = parser.add_mutually_exclusive_group()
which.add_argument('--historical-only',action='store_true')
which.add_argument('--current-only',action='store_true')
args = parser.parse_args()
for name, path, current in datasets:
    if args.historical_only and current or args.current_only and not current: continue
    report = output / (name + '_audit.json')
    assert not report.exists()
    command = ['/home/ran/.local/bin/uv','run','--no-project','--cache-dir','/tmp/alibi-uv',str(here/'audit_m2_component_probes.py'),str(here/path),'--report',str(report)]
    if current: command += ['--check-current',str(binary)]
    result = subprocess.run(command,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    assert result.returncode == 0, result.stdout.decode()
    value = json.loads(report.read_text())
    assert value['result'] == 'passed'
    print(json.dumps({'dataset':path,'result':'passed','raw_phase_samples':value['raw_phase_samples']}),flush=True)
