"""Verify the installed allocator/parser assumptions without rebuilding anything."""
from pathlib import Path
import datetime
import gzip
import hashlib
import json
import subprocess
import sys

ROOT = Path('/home/ran/src/rust/cathedralbevy')
OUT = Path(__file__).resolve().parent
EVIDENCE = OUT.parent.parent
sys.path.insert(0, str(EVIDENCE))
from component_input_sources import sources


def digest(data):
    return hashlib.sha256(data).hexdigest()


start_path = OUT.parent / 'commands/workspace-final-2.start.json'
start = json.loads(start_path.read_bytes())
assert sources() == start['source_sha256']
prior_path = EVIDENCE / 'm2a13/development/stdlib_allocation_audit.json'
prior = json.loads(prior_path.read_bytes())
commands = json.loads((EVIDENCE / 'm2a13/commands.json').read_bytes())
compiler = {}
for key, arguments in (
    ('compiler_sysroot_command', ['--print', 'sysroot']),
    ('compiler_version_command', ['-Vv']),
):
    previous = next(c for c in commands if c['name'] == prior[key])
    original = Path(previous['original_path']).read_bytes()
    assert digest(original) == previous['original_sha256']
    current = subprocess.check_output(['/home/ran/.cargo/bin/rustc', *arguments])
    assert current == original
    compiler[key] = current.decode()

sysroot = Path(compiler['compiler_sysroot_command'].strip()).resolve()
assert sysroot.is_relative_to('/home/ran/.rustup/toolchains')
registry = Path('/tmp/alibi-m1b-cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
parser_roots = [(registry / n).resolve() for n in ('serde_json-1.0.150', 'serde_core-1.0.228')]
verified = []
for name, expected in prior['source_files'].items():
    path = Path(name).resolve()
    assert path.is_relative_to(sysroot) or any(path.is_relative_to(p) for p in parser_roots)
    raw = path.read_bytes()
    assert digest(raw) == expected['sha256'] and len(raw) == expected['bytes']
    lines = raw.decode().splitlines(keepends=True)
    for excerpt in expected['excerpts']:
        assert ''.join(lines[excerpt['start_line'] - 1:excerpt['end_line']]) == excerpt['text']
    verified.append({'path': name, 'sha256': digest(raw), 'verified_excerpts': len(expected['excerpts'])})

additional = []
for relative, first, last in (
    ('alloc/src/sync.rs', 382, 432),
    ('core/src/slice/sort/stable/mod.rs', 98, 132),
):
    path = sysroot / 'lib/rustlib/src/rust/library' / relative
    raw = path.read_bytes()
    additional.append({
        'path': str(path), 'sha256': digest(raw), 'bytes': len(raw),
        'start_line': first, 'end_line': last,
        'text': ''.join(raw.decode().splitlines(keepends=True)[first - 1:last]),
    })

assert sources() == start['source_sha256']
report = {
    'result': 'passed', 'audited_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'scope': 'Installed allocation assumptions only; final workspace and release outcomes are separate.',
    'command_start_sha256': digest(start_path.read_bytes()),
    'helper_sha256': digest(Path(__file__).read_bytes()),
    'prior_proof': str(prior_path.relative_to(ROOT)), 'prior_proof_sha256': digest(prior_path.read_bytes()),
    'compiler_matches_prior_proof': compiler,
    'verified_library_sources': verified, 'additional_library_excerpts': additional,
}
raw = (json.dumps(report, indent=2) + '\n').encode()
original = Path('/tmp/alibi-m2a15-allocation-library-audit.json')
destination = OUT / 'allocation-library-audit.json'
archive = OUT / 'allocation-library-audit.json.gz'
assert not any(p.exists() for p in (original, destination, archive))
original.write_bytes(raw)
destination.write_bytes(raw)
archive.write_bytes(gzip.compress(raw, mtime=0))
assert gzip.decompress(archive.read_bytes()) == original.read_bytes()
print(json.dumps({'result': 'passed', 'verified_library_sources': len(verified),
                  'additional_excerpts': len(additional), 'original_sha256': digest(raw)}))
