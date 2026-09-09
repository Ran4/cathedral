from pathlib import Path
import gzip, hashlib, json, subprocess, sys

root = Path('/home/ran/src/rust/cathedralbevy')
feature = Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play')
plan = feature / 'implementation/plan'
here = plan / 'evidence'
e = here / 'm2a14'
sys.path.insert(0, str(root / here))
from component_input_sources import sources
git = lambda *args: subprocess.check_output(['/usr/bin/git', *args], cwd=root)
load = lambda path: json.loads((root / path).read_bytes())
sha = lambda data: hashlib.sha256(data).hexdigest()
assert git('rev-parse', 'HEAD').decode().strip() == '3aed5c264462e6e8c84c0d0e880e82057f47741d'
assert subprocess.run(['/usr/bin/git', 'diff', '--cached', '--quiet'], cwd=root).returncode == 0
assert (root / e / 'coordinator/review.md').read_text().startswith('Status: Accepted')
for name in ('source_audit', 'format_audit', 'layout_audit', 'log_archive_audit', 'debug_workload_audit', 'release_archive_audit', 'scheduler_smoke_audit', 'night_smoke_audit', 'scheduler_performance_audit', 'night_performance_audit', 'auditor_regressions', 'tail_latency_audit', 'evidence_link_audit', 'plan_validation'):
    assert load(e / f'coordinator/{name}.json')['result'] == 'passed', name
source = load(e / 'source_hashes.json')
assert sources() == source
archives_path = root / e / 'coordinator/audit_log_archives.json'
archives = json.loads(archives_path.read_bytes())
for name in ('finalize-docs', 'docs-audit'):
    original = Path(f'/tmp/alibi-m2a14-{name}-original.log')
    raw = original.read_bytes()
    target = root / e / f'coordinator/{name}.log.gz'
    assert not target.exists()
    target.write_bytes(gzip.compress(raw, mtime=0))
    archives.append({'original_path': str(original), 'original_bytes': len(raw), 'original_sha256': sha(raw), 'archive': target.name, 'archive_bytes': target.stat().st_size, 'archive_sha256': sha(target.read_bytes()), 'normalization': 'none; exact original bytes'})
for record in archives:
    packed = (root / e / 'coordinator' / record['archive']).read_bytes()
    assert sha(packed) == record['archive_sha256']
    raw = gzip.decompress(packed)
    assert sha(raw) == record['original_sha256'] and len(raw) == record['original_bytes']
    assert raw == Path(record['original_path']).read_bytes()
archives_path.write_text(json.dumps(archives, indent=2) + '\n')
scripts = {p.relative_to(root / e).as_posix(): sha(p.read_bytes()) for p in sorted((root / e).rglob('*.py'))}
(root / e / 'coordinator/script_hashes.json').write_text(json.dumps(scripts, indent=2) + '\n')
paths = load(e / 'coordinator/source_audit.json')['changed_paths']
paths += [str(p) for p in (feature / 'README.md', plan / 'README.md', plan / 'M2_simulation_checkpoints.md', plan / 'PERSISTENCE_INVENTORY.md', here / 'audit_m2_component_probes.py', here / 'run_m2_cognition_inputs_probes.py', e)]
subprocess.run(['/usr/bin/git', 'add', '--', *paths], cwd=root, check=True)
subprocess.run(['/usr/bin/git', 'diff', '--cached', '--check'], cwd=root, check=True)
staged = git('diff', '--cached', '--name-only').decode().splitlines()
assert all(p in paths or p.startswith(str(e) + '/') for p in staged)
for path in load(e / 'coordinator/source_audit.json')['changed_paths']:
    assert sha(git('show', ':' + path)) == source[path], path
print(git('diff', '--cached', '--shortstat').decode().strip())
print(json.dumps({'staged_files': len(staged), 'frozen_source_files': len(source), 'scope_checked': True, 'commit_not_yet_created': True}))
