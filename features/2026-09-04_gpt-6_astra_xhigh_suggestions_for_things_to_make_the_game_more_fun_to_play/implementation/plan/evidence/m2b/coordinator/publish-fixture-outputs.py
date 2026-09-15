"""Copy already verified outputs after every frozen-source command completes."""
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
assert performance['end_to_end_samples'] == 1200 and performance['stage_samples'] == 3600
directory = ROOT / 'src/host_checkpoint/fixtures/hydration-v1'
assert not directory.exists()
directory.mkdir(parents=True)
copied = []
for state, output in collected['outputs'].items():
    assert state in ('initial', 'active')
    original = Path(output['original'])
    assert sha(original) == output['sha256'] and original.stat().st_size == output['bytes']
    verified = next(row for row in audit['records'] if row['state'] == state and row['kind'] == 'write-1')
    assert verified['sha256'] == output['sha256'] and verified['bytes'] == output['bytes']
    destination = directory / f'{state}.json'
    with destination.open('xb') as stream:
        stream.write(original.read_bytes())
    assert destination.read_bytes() == original.read_bytes()
    copied.append(output | {'repository': str(destination.relative_to(ROOT))})

build = json.loads((OUT / 'release_build.json').read_bytes())
assert sha(Path(build['reference_binary'])) == collected['release_binary_sha256']
rows = '\n'.join(f"| {row['repository'].rsplit('/', 1)[1]} | {row['bytes']} | `{row['sha256']}` |"
                 for row in copied)
readme = directory / 'README.md'
readme.write_text(f'''# M2b complete hydration fixtures

These are exact complete-envelope version 1 outputs from ordinary initial and
active 520-character hosts. Three fresh writer processes produced identical
bytes for each state. A fresh process using the creating executable rebuilt
the actual World/Engine owners and matched all sixteen saved category hashes.
A different executable rejected both files before construction.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
{rows}

The creating executable is `{build['reference_binary']}` with SHA-256
`{collected['release_binary_sha256']}`. These payloads deliberately require that
exact executable image. A new build may refuse them even when its source is
unchanged; neither saved manifests nor payload bytes should be rewritten to
make a different image appear compatible. The executable is a local reference
artifact, not committed here. These JSON files are runtime test inputs and are
not compiled into the creating executable.

Saved lineage is `{collected['lineage_hex']}`. Validation and hydration preserve
it while assigning a fresh execution generation. Hydration disposes the source
App before constructing the fresh definition resolver, never seeds a replacement
Engine or polls solely for saving, and retains pending external obligations in
quarantine. M2c continuation and M3 application adoption remain separate work.

Exact logs, metrics, raw archives and command-start source/image identities are
in the feature plan's `evidence/m2b/fixtures-attempt-{collected['attempt']}/`;
the independent byte/category audit is `coordinator/fixture-audit.json`.
`coordinator/fixture-publication.json` records this output-only source delta.
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
    'scope': 'Two exact verified outputs and their README only; all prior source inputs remain byte-identical.',
})
print(json.dumps({'result': 'passed', 'fixtures': copied, 'source_count': len(after)}))
