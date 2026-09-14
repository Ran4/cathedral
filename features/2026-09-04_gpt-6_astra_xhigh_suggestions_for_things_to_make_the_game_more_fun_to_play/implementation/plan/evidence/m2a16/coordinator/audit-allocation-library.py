"""Revalidate installed allocation/parser sources against preserved excerpts."""
from pathlib import Path
import datetime
import gzip
import hashlib
import json
import subprocess
from release_common import ROOT, LEG, OUT, EVIDENCE, frozen_sources, sha, write_json

frozen = frozen_sources()
prior_path = EVIDENCE / 'm2a13/development/stdlib_allocation_audit.json'
prior = json.loads(prior_path.read_bytes())
later_path = EVIDENCE / 'm2a15/coordinator/allocation-library-audit.json'
later = json.loads(later_path.read_bytes())
compiler = {}
for key, args in (('compiler_sysroot_command', ['--print', 'sysroot']),
                  ('compiler_version_command', ['-Vv'])):
    current = subprocess.check_output(['/home/ran/.cargo/bin/rustc', *args]).decode()
    assert current == later['compiler_matches_prior_proof'][key]
    compiler[key] = current
sysroot = Path(compiler['compiler_sysroot_command'].strip()).resolve()
assert sysroot.is_relative_to('/home/ran/.rustup/toolchains')
registry = Path('/tmp/alibi-m1b-cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
roots = [p.resolve() for p in
         (sysroot, registry / 'serde_json-1.0.150', registry / 'serde_core-1.0.228')]
verified = []
for name, expected in prior['source_files'].items():
    path = Path(name).resolve()
    assert any(path.is_relative_to(root) for root in roots)
    raw = path.read_bytes()
    assert sha(path) == expected['sha256'] and len(raw) == expected['bytes']
    lines = raw.decode().splitlines(keepends=True)
    for excerpt in expected['excerpts']:
        assert ''.join(lines[excerpt['start_line'] - 1:excerpt['end_line']]) == excerpt['text']
    verified.append({'path': name, 'sha256': sha(path), 'verified_excerpts': len(expected['excerpts'])})
for excerpt in later['additional_library_excerpts']:
    path = Path(excerpt['path']).resolve()
    assert path.is_relative_to(sysroot)
    raw = path.read_bytes()
    assert sha(path) == excerpt['sha256'] and len(raw) == excerpt['bytes']
    assert ''.join(raw.decode().splitlines(keepends=True)[
        excerpt['start_line'] - 1:excerpt['end_line']]) == excerpt['text']
    verified.append({'path': str(path), 'sha256': sha(path), 'verified_excerpts': 1})
assert frozen_sources() == frozen
report = {
    'result': 'passed', 'audited_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'scope': 'Installed library assumptions, including Vec/String/BTree/Arc/parser and stable-sort scratch. The M2a16 typed meter and custom adapters require their separate owner proof; this is not a general heap theorem.',
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'helper_sha256': sha(Path(__file__)), 'compiler': compiler,
    'prior_proof': str(prior_path.relative_to(ROOT)), 'prior_proof_sha256': sha(prior_path),
    'additional_proof': str(later_path.relative_to(ROOT)), 'additional_proof_sha256': sha(later_path),
    'verified_library_sources': verified,
    'stable_sort_note': 'Scratch is max(n-n/2, min(n, 8_000_000/sizeof(T))) with a small-sort floor; the complete borrowed index therefore reserves up to8MiB scratch, not4MiB.',
}
raw = (json.dumps(report, indent=2, sort_keys=True) + '\n').encode()
original = Path('/tmp/alibi-m2a16-allocation-library-audit.json')
destination, archive = OUT / 'allocation-library-audit.json', OUT / 'allocation-library-audit.json.gz'
assert not any(p.exists() for p in (original, destination, archive))
original.write_bytes(raw)
destination.write_bytes(raw)
archive.write_bytes(gzip.compress(raw, mtime=0))
assert gzip.decompress(archive.read_bytes()) == original.read_bytes()
print(json.dumps({'result': 'passed', 'verified_library_sources': len(verified),
                  'original_sha256': hashlib.sha256(raw).hexdigest()}))
