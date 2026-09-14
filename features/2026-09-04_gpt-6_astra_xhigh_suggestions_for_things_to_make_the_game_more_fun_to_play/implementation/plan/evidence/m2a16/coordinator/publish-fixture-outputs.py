"""Publish verified fixture outputs and their documentation after frozen runs."""
from pathlib import Path
import json
from release_common import ROOT, LEG, OUT, sources, frozen_sources, sha, write_json

before = frozen_sources()
readme_path = ROOT / 'crates/cathedral-sim/tests/fixtures/checkpoint_host/README.md'
readme_before = readme_path.read_bytes()
readme_after = readme_before.decode()
refresh = json.loads((LEG / 'host-component-fixture-refresh/record.json').read_bytes())
for row in refresh['fixtures']:
    assert sha(ROOT / row['fixture']) == row['current_sha256']
    old = f"| {row['prior_bytes']} | `{row['prior_sha256']}` |"
    new = f"| {row['current_bytes']} | `{row['current_sha256']}` |"
    assert readme_after.count(old) == 1
    readme_after = readme_after.replace(old, new)
readme_after = readme_after.replace('Preserve these bytes.',
    'The current pair was explicitly refreshed during M2a16 review on 2026-09-15.\n'
    'The strict installed-catalog fingerprint includes all of `src/city/mod.rs`,\n'
    'which gained a test-only memory observer. Three fresh writers from the\n'
    'preserved M2a16 debug executable agreed exactly. The only changed JSON path\n'
    'is `/scalars/definitions/installed_catalogs`; all gameplay state is identical.\n'
    'The original M2a15 bytes, current bytes, exact command provenance and\n'
    'independent review are preserved in M2a16 evidence under\n'
    '`host-component-fixture-refresh/` and\n'
    '`coordinator/host-fixture-refresh-audit.json`. The complete workspace rerun\n'
    'validates this current pair. The M2a15 record above remains historical.\n\n'
    'Preserve these bytes.')
assert readme_after.encode() != readme_before
collected = json.loads((OUT / 'fixtures-collected.json').read_bytes())
audit = json.loads((OUT / 'fixture-audit.json').read_bytes())
assert collected['repository_copy_pending'] is True and audit['result'] == 'passed'
assert audit['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
for name in ('smoke-audit.json', 'performance-audit.json'):
    checked = json.loads((OUT / name).read_bytes())
    assert checked['result'] == 'integrity passed'
    assert checked['source_manifest_sha256'] == audit['source_manifest_sha256']
    assert checked['binary_sha256'] == collected['release_sha256']
directory = ROOT / 'src/host_checkpoint/fixtures/complete-v1'
assert not directory.exists()
directory.mkdir(parents=True)
copied = []
for state in ('initial', 'active'):
    original = Path(collected['chosen_originals'][state])
    expected = next(row for row in audit['records'] if row['state'] == state and row['kind'] == 'writer-1')
    assert sha(original) == expected['sha256']
    destination = directory / (state + '.json')
    with destination.open('xb') as stream:
        stream.write(original.read_bytes())
    assert destination.read_bytes() == original.read_bytes()
    copied.append({'original': str(original), 'repository': str(destination.relative_to(ROOT)),
                   'bytes': destination.stat().st_size, 'sha256': sha(destination)})
readme_archive = OUT / 'host-fixture-readme-before-publication.md'
assert not readme_archive.exists() and readme_path.read_bytes() == readme_before
readme_archive.write_bytes(readme_before)
readme_path.write_text(readme_after)
after = sources()
added = sorted(set(after) - set(before))
assert added == sorted(row['repository'] for row in copied)
modified = sorted(path for path, digest in before.items() if after.get(path) != digest)
assert modified == [str(readme_path.relative_to(ROOT))]
assert len(after) == len(before) + 2
final_path = LEG / 'source_hashes_with_fixtures.json'
assert not final_path.exists()
write_json(final_path, after)
record = OUT / 'fixture-publication.json'
assert not record.exists()
write_json(record, {'result': 'passed', 'helper_sha256': sha(Path(__file__)),
    'before_source_manifest': str((LEG / 'source_hashes.json').relative_to(ROOT)),
    'before_source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'after_source_manifest': str(final_path.relative_to(ROOT)),
    'after_source_manifest_sha256': sha(final_path), 'added_paths': added,
    'modified_paths': modified, 'removed_paths': [], 'fixtures': copied,
    'readme_before_sha256': sha(readme_archive), 'readme_after_sha256': sha(readme_path),
    'scope': 'Two exact previously verified outputs and one documentation update only. Executable, Rust sources and existing fixture bytes remain unchanged. Pre-publication build/test records retain their original map.'})
print(json.dumps({'result': 'passed', 'copied': copied, 'source_count': len(after)}))
