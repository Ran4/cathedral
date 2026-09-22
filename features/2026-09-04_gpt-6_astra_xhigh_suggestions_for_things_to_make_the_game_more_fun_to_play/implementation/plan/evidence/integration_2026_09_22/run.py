"""Record a serial, lower-priority workspace check with exact input identity."""
import datetime
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True
here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('sources', here.parent / 'component_input_sources.py')
sources = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sources)
name = sys.argv[1]
if (here / f'{name}-start.json').exists():
    raise SystemExit('run name already used')
env = dict(os.environ)
# Match the HTTP owner's default Cargo home/compiler/profile to reuse artifacts.
for key in ['CARGO_HOME', 'CARGO_TARGET_DIR', 'RUSTC', 'RUSTDOC', 'RUSTFLAGS',
            'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'RUSTC_WRAPPER',
            'RUSTC_WORKSPACE_WRAPPER', 'LDFLAGS']:
    env.pop(key, None)
env.update(CATHEDRAL_HEADLESS='1', CATHEDRAL_FAKE_BACKEND='1', PYTHONDONTWRITEBYTECODE='1')
command = ['/usr/bin/nice', '-n', '15', '/home/ran/.cargo/bin/cargo', 'test',
           '--workspace', '--locked', '--offline', '-j1', '--', '--test-threads=1']
def write(path, data):
    path.write_text(json.dumps(data, sort_keys=True, indent=2) + '\n')
def sha(data):
    return hashlib.sha256(data).hexdigest()
before = sources.sources()
write(here / f'{name}-sources.json', before)
raw = Path('/tmp') / f'alibi-integration-{name}.log'
write(here / f'{name}-start.json', {
    'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'command': command, 'cwd': str(sources.ROOT),
    'head': subprocess.check_output(['/usr/bin/git', 'rev-parse', 'HEAD'], cwd=sources.ROOT, text=True).strip(),
    'environment_policy': 'default Cargo home/compiler/profile; flags/wrappers cleared; headless/fake; nice15; one Cargo job and one test thread',
    'source_map_sha256': sha((here / f'{name}-sources.json').read_bytes()),
    'runner_sha256': sha(Path(__file__).read_bytes()),
    'raw_log': str(raw),
})
started = time.monotonic()
with raw.open('xb') as output:
    result = subprocess.run(command, cwd=sources.ROOT, env=env, stdout=output, stderr=subprocess.STDOUT)
archive = here / f'{name}.log.gz'
archive.write_bytes(gzip.compress(raw.read_bytes(), mtime=0))
write(here / f'{name}-result.json', {
    'exit_code': result.returncode, 'wall_seconds': time.monotonic() - started,
    'sources_unchanged': sources.sources() == before,
    'raw_sha256': sha(raw.read_bytes()), 'archive_sha256': sha(archive.read_bytes()),
})
print(json.dumps({'run': name, 'exit_code': result.returncode, 'raw_log': str(raw)}), flush=True)
raise SystemExit(result.returncode)
