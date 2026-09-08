from pathlib import Path
import subprocess, json, hashlib, datetime, sys

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0, str(here))
from component_input_sources import SOURCE_SCOPE, sources
e = here / 'm2a11'
out = e / 'coordinator'
out.mkdir(exist_ok=True)
sha = lambda b: hashlib.sha256(b).hexdigest()
git = lambda *args: subprocess.check_output(['/usr/bin/git', *args], cwd=root)
manifest = json.loads((e / 'source_hashes.json').read_bytes())
manifest = manifest.get('files', manifest)
assert sources() == manifest
assert git('rev-parse','HEAD').decode().strip() == '2a5115060b8dc8731d93ca14d0a015d72898f981'
changed = set(git('diff','--name-only','HEAD','--','crates','Cargo.toml','Cargo.lock').decode().splitlines())
changed.update(git('ls-files','--others','--exclude-standard','--','crates').decode().splitlines())
assert changed <= manifest.keys(), sorted(changed - manifest.keys())
comparisons = []
for name, addition in {
    'crates/cathedral-sim/src/engine.rs':'pub mod scheduler_checkpoint;\n',
    'crates/cathedral-sim/src/scheduler.rs':'pub mod checkpoint;\n\n',
}.items():
    before = git('show','HEAD:' + name)
    after = (root / name).read_bytes()
    assert after.count(addition.encode()) == 1, name
    assert after.replace(addition.encode(), b'', 1) == before, name
    comparisons.append({'path':name,'baseline_sha256':sha(before),'current_sha256':sha(after),
        'removed_module_bytes':addition,'remaining_bytes_identical':True})
name = 'crates/cathedral-sim/src/receipts/checkpoint.rs'
before = git('show','HEAD:' + name)
after = (root / name).read_bytes()
assert after.startswith(before), 'existing receipt checkpoint logic changed'
addition = after[len(before):]
assert addition.count(b'fn checkpoint_turn_root(') == 2
assert b'impl CommandLedgerDtoV1' in addition and b'impl CommandLedger {' in addition
comparisons.append({'path':name,'baseline_sha256':sha(before),'current_sha256':sha(after),
    'existing_bytes_identical':True,'appended_bytes':len(addition),
    'appended_sha256':sha(addition),'purpose':'borrowed known TURN semantic-root binding for live/saved ledgers'})
fixtures = {}
for name in git('ls-tree','-r','--name-only','HEAD','--','crates/cathedral-sim/tests/fixtures').decode().splitlines():
    baseline = git('show','HEAD:' + name)
    assert baseline == (root / name).read_bytes(), name
    fixtures[name] = sha(baseline)
checks = []
for name in sorted(changed):
    if not name.endswith('.rs'): continue
    cmd = ['/home/ran/.cargo/bin/rustfmt','--edition','2024','--config','skip_children=true','--check',name]
    result = subprocess.run(cmd,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    assert result.returncode == 0, result.stdout.decode()
    checks.append({'path':name,'command':cmd,'source_sha256':sha((root/name).read_bytes()),'exit_code':0,'output':result.stdout.decode()})
stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
report = {'checked_at_utc':stamp,'result':'passed','source_scope':SOURCE_SCOPE,
    'source_manifest_sha256':sha((e/'source_hashes.json').read_bytes()),
    'source_files_checked':len(manifest),'changed_paths_all_covered':True,
    'changed_paths':sorted(changed),'historical_fixtures_and_docs_unchanged':fixtures,
    'ordinary_behavior_comparisons':comparisons}
for name, value in [('source_audit.json',report),('format_audit.json',{'checked_at_utc':stamp,'result':'passed','checks':checks})]:
    assert not (out/name).exists()
    (out/name).write_text(json.dumps(value,indent=2)+'\n')
print(json.dumps({'result':'passed','source_files':len(manifest),'changed_paths':len(changed),
    'scoped_format_checks':len(checks),'historical_fixture_files':len(fixtures)}))
