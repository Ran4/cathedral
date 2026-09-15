"""Publish verified runtime fixtures only after all frozen-source checks finish."""
from pathlib import Path
import json
from release_common import LEG, OUT, ROOT, frozen_sources, sha, sources, write_json

before = frozen_sources()
collected = json.loads((OUT / 'fixture-outputs.json').read_bytes())
audit = json.loads((OUT / 'fixture-audit.json').read_bytes())
owner = json.loads((OUT / 'owner-command-audit.json').read_bytes())
performance = json.loads((OUT / 'release-performance-audit.json').read_bytes())
smoke = json.loads((OUT / 'release-smoke-audit.json').read_bytes())
assert audit['result'] == 'passed' and audit['repository_copy_pending'] is True
assert owner['all_originals_and_archives_verified'] is True
assert performance['result'] == smoke['result'] == 'passed'
for report in (collected, audit, owner, performance, smoke):
    assert report['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
assert collected['release_binary_sha256'] == performance['binary_sha256'] == smoke['binary_sha256']
assert performance['end_to_end_samples'] == 1800 and performance['stage_samples'] == 3000
directory = ROOT / 'src/host_checkpoint/fixtures/continuation-v2'
assert not directory.exists()
directory.mkdir(parents=True)
copied = []
for mode, output in collected['outputs'].items():
    assert mode in ('authored', 'populated')
    original = Path(output['original'])
    assert sha(original) == output['sha256'] and original.stat().st_size == output['bytes']
    verified = next(row for row in audit['records'] if row['mode'] == mode and row['kind'] == 'write-1')
    assert verified['sha256'] == output['sha256'] and verified['bytes'] == output['bytes']
    destination = directory / f'{mode}.json'
    with destination.open('xb') as stream:
        stream.write(original.read_bytes())
    assert destination.read_bytes() == original.read_bytes()
    copied.append(output | {'repository': str(destination.relative_to(ROOT))})

build = json.loads((OUT / 'release_build.json').read_bytes())
assert sha(Path(build['reference_binary'])) == collected['release_binary_sha256']
rows = '\n'.join(f"| {row['repository'].rsplit('/', 1)[1]} | {row['bytes']} | `{row['sha256']}` |"
                 for row in copied)
readme = directory / 'README.md'
readme.write_text(f'''# M2c prepared continuation fixtures

These complete-envelope version 1 outputs contain the explicit pending-owner
extensions produced by M2c preparation, including durable unsent speech and its
terminal interruption receipt. They come from ordinary 520- and 2,520-character
hosts with accepted recording and pending cognition. Reports retain the actual
held/deferred source shape; this fixture set does not claim every pending shape.

Three fresh writer processes produced identical bytes for each profile. A fresh
process using the creating executable validated, hydrated and prepared each file,
then required both its first prepared output and second re-save to match the
original input bytes exactly. The original host remains a separate fixture;
whole-host adoption and retirement remain M3 work.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
{rows}

The creating executable is `{build['reference_binary']}` with SHA-256
`{collected['release_binary_sha256']}`. These payloads require that exact image.
Do not rewrite manifests or payloads to disguise a different executable as
compatible. The executable is a local reference artifact and is not committed.
These JSON files are runtime inputs, not compiled into the creating executable.

Saved lineage is `{collected['lineage_hex']}`. Preparation preserves saved time,
host choices and lineage while replacing external execution ownership. No
microphone, provider, TTS or ordinary simulation poll runs during preparation.

Exact originals, archives and command-start identities are recorded under the
feature plan's `evidence/m2c/fixtures-attempt-{collected['attempt']}/`; the
independent audit is `coordinator/fixture-audit.json`. Publication adds only these
two verified outputs and this README; `coordinator/fixture-publication.json`
records the resulting source-map delta. Historical V1 fixtures remain unchanged.
''')
after = sources()
added = sorted(set(after) - set(before))
expected = sorted([row['repository'] for row in copied] + [str(readme.relative_to(ROOT))])
assert added == expected
assert all(after.get(path) == digest for path, digest in before.items())
final = LEG / 'source_hashes_with_fixtures.json'
assert not final.exists()
write_json(final, after)
write_json(OUT / 'fixture-publication.json', {
    'result': 'passed', 'helper_sha256': sha(Path(__file__)),
    'before_source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'after_source_manifest_sha256': sha(final), 'added_paths': added,
    'modified_paths': [], 'removed_paths': [], 'fixtures': copied,
    'scope': 'Two verified runtime outputs and README only; all prior source inputs remain byte-identical.',
})
print(json.dumps({'result': 'passed', 'fixtures': copied, 'source_count': len(after)}))
