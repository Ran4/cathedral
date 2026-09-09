from pathlib import Path
import subprocess, os, time, json, hashlib, sys, gzip, datetime

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0, str(here))
from component_input_sources import SOURCE_SCOPE, sources
kind = sys.argv[1]
assert kind in ('smoke', 'performance') and len(sys.argv) == 2
e = here / 'm2a14'
out = e / 'coordinator'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
frozen = json.loads((e / 'source_hashes.json').read_bytes())
assert sources() == frozen
build = json.loads((out / 'release_build.json').read_bytes())
assert build['exit_code'] == 0 and build['exact_source_scope_unchanged_after_build']
assert build['source_manifest_sha256'] == sha(e / 'source_hashes.json')
overrides = {'PATH':'/usr/bin:/bin:/home/ran/.local/bin:/home/ran/.cargo/bin', 'CARGO_HOME':'/tmp/alibi-m1b-cargo', 'RUSTC':'/home/ran/.cargo/bin/rustc', 'RUSTDOC':'/home/ran/.cargo/bin/rustdoc', 'CATHEDRAL_HEADLESS':'1', 'CATHEDRAL_FAKE_BACKEND':'1'}
env = os.environ.copy()
env.update(overrides)
report_path = out / (kind + '_runner_commands.json')
assert not report_path.exists()
records = []
for owner in ('scheduler', 'night'):
    binary = root / 'target/release/examples' / f'alibi_{owner}_cost'
    assert sha(binary) == build['binary_sha256'][binary.name]
    runner = here / 'run_m2_cognition_inputs_probes.py'
    target = e / kind / owner
    original = Path(f'/tmp/alibi-m2a14-{owner}-{kind}-runner-original.log')
    assert not original.exists() and not target.exists()
    cmd = ['/home/ran/.local/bin/uv', 'run', '--no-project', '--cache-dir', '/tmp/alibi-uv', str(runner), '--owner', owner, '--output-dir', str(target)]
    if kind == 'smoke':
        cmd.append('--smoke')
    helpers = [runner, here / 'run_m1_comparison.py', here / 'run_m2_checkpoint_probes.py', here / 'component_input_sources.py']
    before_helpers = {p.name: sha(p) for p in helpers}
    assert sources() == frozen
    record = {'owner':owner,'kind':kind,'command':cmd,'environment_overrides':overrides,
              'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'source_scope':SOURCE_SCOPE,'source_manifest_sha256':sha(e / 'source_hashes.json'),
              'binary_sha256':sha(binary),'runner_sha256':before_helpers,
              'wrapper_sha256':sha(Path(__file__)),'status':'running'}
    records.append(record)
    report_path.write_text(json.dumps(records, indent=2) + '\n')
    tick = time.monotonic()
    with original.open('wb') as log:
        result = subprocess.run(cmd, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
    raw = original.read_bytes()
    archive = out / f'{owner}_{kind}_runner.log.gz'
    assert not archive.exists()
    archive.write_bytes(gzip.compress(raw, mtime=0))
    record.update({'status':'completed','exit_code':result.returncode,'elapsed_seconds':time.monotonic()-tick,
                   'original_path':str(original),'original_bytes':len(raw),'original_sha256':sha(original),
                   'archive':str(archive.relative_to(root)),'archive_bytes':archive.stat().st_size,
                   'archive_sha256':sha(archive),'normalization':'none; exact original bytes',
                   'unchanged_source_binary_helpers':sources()==frozen and sha(binary)==record['binary_sha256'] and {p.name:sha(p) for p in helpers}==before_helpers})
    report_path.write_text(json.dumps(records, indent=2) + '\n')
    print(json.dumps({'owner':owner,'kind':kind,'exit_code':result.returncode,'elapsed_seconds':record['elapsed_seconds']}), flush=True)
    assert record['unchanged_source_binary_helpers']
    if result.returncode:
        raise SystemExit(result.returncode)
