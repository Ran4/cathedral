from pathlib import Path
import subprocess, os, time, json, hashlib, shutil, sys, gzip, datetime

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0, str(here))
from component_input_sources import SOURCE_SCOPE, sources
e = here / 'm2a12'
out = e / 'coordinator'
out.mkdir(parents=True, exist_ok=True)
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
manifest = json.loads((e / 'source_hashes.json').read_text())
before = sources()
assert before == manifest.get('files', manifest), 'release needs exact frozen v2 source set'
helper_before = sha(here / 'component_input_sources.py')
original = Path('/tmp/alibi-m2a12-release-build-original.log')
timing = Path('/tmp/alibi-m2a12-release-build-original.time')
assert not original.exists() and not timing.exists()
assert not (out / 'release_build.json').exists()
cmd = ['/home/ran/.cargo/bin/cargo', 'build', '--offline', '--release', '-p', 'cathedral-backends', '--example', 'alibi_continuity_cost', '-j', '1']
overrides = {'PATH':'/usr/bin:/bin:/home/ran/.local/bin:/home/ran/.cargo/bin', 'CARGO_HOME':'/tmp/alibi-m1b-cargo', 'RUSTC':'/home/ran/.cargo/bin/rustc', 'RUSTDOC':'/home/ran/.cargo/bin/rustdoc', 'CATHEDRAL_HEADLESS':'1', 'CATHEDRAL_FAKE_BACKEND':'1'}
env = os.environ.copy(); env.update(overrides)
build_vars = {k:v for k,v in env.items() if k in {'PATH','CC','CXX','AR','LD','CFLAGS','CXXFLAGS','LDFLAGS','CATHEDRAL_HEADLESS','CATHEDRAL_FAKE_BACKEND'} or k.startswith(('CARGO_','RUST','PKG_CONFIG'))}
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
tick = time.monotonic()
with original.open('wb') as log:
    result = subprocess.run(['/usr/bin/time', '-v', '-o', str(timing), *cmd], cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
elapsed = time.monotonic() - tick
artifacts = {}
for name, path in [('release_build.log', original), ('release_build.time', timing)]:
    raw = path.read_bytes()
    packed = out / (name + '.gz')
    assert not packed.exists()
    packed.write_bytes(gzip.compress(raw, mtime=0))
    artifacts[name] = {'original_path':str(path), 'original_bytes':len(raw), 'original_sha256':sha(path), 'archive':str(packed.relative_to(root)), 'archive_bytes':packed.stat().st_size, 'archive_sha256':sha(packed), 'normalization':'none; exact original bytes'}
report = {'command':cmd, 'environment_overrides':overrides, 'build_variable_whitelist':build_vars, 'started_utc':started, 'elapsed_seconds':elapsed, 'exit_code':result.returncode, 'artifacts':artifacts, 'source_scope':SOURCE_SCOPE, 'source_manifest_sha256':sha(e / 'source_hashes.json'), 'source_helper_sha256':helper_before}
report['exact_source_scope_unchanged_after_build'] = sources() == before
report['source_helper_unchanged_after_build'] = sha(here / 'component_input_sources.py') == helper_before
if result.returncode == 0:
    binary = root / 'target/release/examples/alibi_continuity_cost'
    reference = Path('/tmp/alibi-m2a12-continuity-reference-binary')
    assert not reference.exists()
    shutil.copy2(binary, reference)
    report['binary_sha256'] = {'alibi_continuity_cost':sha(binary)}
    report['preserved_reference'] = {'path':str(reference), 'sha256':sha(reference)}
(out / 'release_build.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2), flush=True)
assert report['exact_source_scope_unchanged_after_build'] and report['source_helper_unchanged_after_build']
raise SystemExit(result.returncode)
