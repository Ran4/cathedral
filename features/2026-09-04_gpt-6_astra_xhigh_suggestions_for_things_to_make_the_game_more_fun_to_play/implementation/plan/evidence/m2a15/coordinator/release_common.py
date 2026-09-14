"""Coordinator release evidence; no execution or source mutation on import."""
from pathlib import Path
import datetime
import gzip
import hashlib
import json
import os
import re
import subprocess
import sys
import time

ROOT = Path('/home/ran/src/rust/cathedralbevy')
OUT = Path(__file__).resolve().parent
LEG = OUT.parent
EVIDENCE = LEG.parent
sys.path.insert(0, str(EVIDENCE))
from component_input_sources import SOURCE_SCOPE, sources

OVERRIDES = {
    'PATH': '/usr/bin:/bin:/home/ran/.local/bin:/home/ran/.cargo/bin',
    'CARGO_HOME': '/tmp/alibi-m1b-cargo',
    'RUSTC': '/home/ran/.cargo/bin/rustc',
    'RUSTDOC': '/home/ran/.cargo/bin/rustdoc',
    'CATHEDRAL_HEADLESS': '1',
    'CATHEDRAL_FAKE_BACKEND': '1',
}


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def frozen_sources():
    frozen = json.loads((LEG / 'source_hashes.json').read_bytes())
    assert sources() == frozen, 'production inputs differ from final owner freeze'
    return frozen


def archive(original, destination):
    assert original.is_file() and not destination.exists()
    raw = original.read_bytes()
    destination.write_bytes(gzip.compress(raw, mtime=0))
    assert gzip.decompress(destination.read_bytes()) == raw
    return {
        'original': str(original), 'original_bytes': len(raw),
        'original_sha256': sha(original),
        'archive': str(destination.relative_to(ROOT)),
        'archive_sha256': sha(destination),
        'normalization': 'none; exact original bytes',
    }


def capture(label, command, destination, extra_environment=None, binary=None):
    """Record inputs BEFORE command start; preserve every failure and original."""
    assert re.fullmatch(r'[a-z0-9-]+', label)
    frozen = frozen_sources()
    destination.mkdir(parents=True, exist_ok=True)
    original = Path(f'/tmp/alibi-m2a15-{label}.stdout-stderr.log')
    timing = Path(f'/tmp/alibi-m2a15-{label}.time')
    start_path = destination / f'{label}.start.json'
    result_path = destination / f'{label}.result.json'
    assert not any(p.exists() for p in (original, timing, start_path, result_path))
    overrides = OVERRIDES | (extra_environment or {})
    environment = os.environ.copy()
    environment.update(overrides)
    helpers = [Path(__file__), Path(sys.argv[0]).resolve(), EVIDENCE / 'component_input_sources.py']
    helper_hashes = {str(p.relative_to(ROOT)): sha(p) for p in helpers}
    start = {
        'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'command': command, 'cwd': str(ROOT), 'environment_overrides': overrides,
        'inherited_build_environment': {
            key: os.environ.get(key) for key in (
                'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'LDFLAGS', 'CARGO_BUILD_TARGET',
                'CARGO_TARGET_DIR', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
            )
        },
        'source_scope': SOURCE_SCOPE,
        'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
        'source_count': len(frozen), 'helper_sha256': helper_hashes,
        'binary': None if binary is None else str(binary),
        'binary_sha256': None if binary is None else sha(binary),
    }
    write_json(start_path, start)
    tick = time.monotonic()
    with original.open('wb') as stream:
        result = subprocess.run(
            ['/usr/bin/time', '-v', '-o', str(timing), *command],
            cwd=ROOT, env=environment, stdout=stream, stderr=subprocess.STDOUT,
        )
    record = start | {
        'completed_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'exit_code': result.returncode, 'elapsed_seconds': time.monotonic() - tick,
        'log': archive(original, destination / f'{label}.log.gz'),
        'time': archive(timing, destination / f'{label}.time.gz'),
        'unchanged_source': sources() == frozen,
        'unchanged_helpers': all(sha(ROOT / p) == digest for p, digest in helper_hashes.items()),
        'unchanged_binary': binary is None or sha(binary) == start['binary_sha256'],
    }
    write_json(result_path, record)
    print(json.dumps({key: record[key] for key in ('command', 'exit_code', 'elapsed_seconds')}), flush=True)
    assert record['unchanged_source'] and record['unchanged_helpers'] and record['unchanged_binary']
    if result.returncode:
        raise SystemExit(result.returncode)
    return record
