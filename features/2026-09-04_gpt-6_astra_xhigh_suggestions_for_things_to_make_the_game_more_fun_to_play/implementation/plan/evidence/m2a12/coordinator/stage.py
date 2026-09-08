from pathlib import Path
import hashlib, json, subprocess, shutil, sys

root = Path('/home/ran/src/rust/cathedralbevy')
feature = Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play')
plan = feature/'implementation/plan'
here = plan/'evidence'
e = here/'m2a12'
sys.path.insert(0,str(root/here))
from component_input_sources import sources
git = lambda *args: subprocess.check_output(['/usr/bin/git',*args],cwd=root)
assert git('rev-parse','HEAD').decode().strip() == 'e68d57fcc795b401b10169c0ccb9ab00b21e1556'
assert subprocess.run(['/usr/bin/git','diff','--cached','--quiet'],cwd=root).returncode == 0
assert (root/e/'coordinator/review.md').read_text().startswith('Status: Accepted')
for name in ['source_audit','format_audit','log_archive_audit','stdlib_allocation_audit','release_archive_audit','continuity_smoke_audit','continuity_performance_audit','auditor_regressions','tail_latency_audit','evidence_link_audit']:
    assert json.loads((root/e/f'coordinator/{name}.json').read_text())['result'] == 'passed'
assert (root/e/'coordinator/plan_validation.json').exists()
source = json.loads((root/e/'source_hashes.json').read_text())
assert sources() == source
script_hashes = {}
for p in sorted(Path('/tmp').glob('alibi-m2a12-*.py')):
    target = root/e/'coordinator'/p.name.removeprefix('alibi-m2a12-')
    assert not target.exists()
    shutil.copy2(p,target)
    script_hashes[target.name] = hashlib.sha256(target.read_bytes()).hexdigest()
(root/e/'coordinator/script_hashes.json').write_text(json.dumps(script_hashes,indent=2)+'\n')
paths = json.loads((root/e/'coordinator/source_audit.json').read_text())['changed_paths']
paths += [str(p) for p in [feature/'README.md',plan/'README.md',plan/'M2_simulation_checkpoints.md',plan/'PERSISTENCE_INVENTORY.md',here/'audit_m2_component_probes.py',here/'run_m2_continuity_probes.py',here/'component_input_sources.py',e]]
subprocess.run(['/usr/bin/git','add','--',*paths],cwd=root,check=True)
subprocess.run(['/usr/bin/git','diff','--cached','--check'],cwd=root,check=True)
staged = git('diff','--cached','--name-only').decode().splitlines()
assert all(p in paths or p.startswith(str(e)+'/') for p in staged)
for path in json.loads((root/e/'coordinator/source_audit.json').read_text())['changed_paths']:
    assert hashlib.sha256(git('show',':'+path)).hexdigest() == source[path], path
print(git('diff','--cached','--shortstat').decode().strip())
print(json.dumps({'staged_files':len(staged),'frozen_source_files':len(source),'scope_checked':True,'commit_not_yet_created':True},indent=2))
