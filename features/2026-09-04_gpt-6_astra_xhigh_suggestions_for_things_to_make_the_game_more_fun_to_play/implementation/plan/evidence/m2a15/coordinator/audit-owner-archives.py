"""Audit final and failed owner commands without replaying their workloads."""
from pathlib import Path
import datetime
import gzip
import json
import re
from release_common import LEG, OUT, frozen_sources, sha, write_json

frozen = frozen_sources()
commands = []
for result_path in sorted((LEG / 'commands').glob('*.result.json')):
    label = result_path.name.removesuffix('.result.json')
    start_path = result_path.with_name(label + '.start.json')
    archive_path = result_path.with_name(label + '.log.gz')
    result = json.loads(result_path.read_bytes())
    start = json.loads(start_path.read_bytes())
    original = Path(start['raw_log'])
    raw = original.read_bytes()
    zipped = archive_path.read_bytes()
    assert gzip.decompress(zipped) == raw and zipped[4:8] == bytes(4), label
    assert sha(original) == result['raw_sha256'], label
    assert start['source_scope'] == 'component-inputs-v2'
    helper = result_path.parent / 'helpers' / (start['helper_sha256'] + '.py')
    assert sha(helper) == start['helper_sha256'], label
    commands.append({
        'label': label, 'command': start['command'], 'exit_code': result['exit_code'],
        'original': str(original), 'original_bytes': len(raw), 'original_sha256': sha(original),
        'archive_sha256': sha(archive_path), 'start_sha256': sha(start_path),
        'result_sha256': sha(result_path), 'source_count': len(start['source_sha256']),
        'start_matches_final_source': start['source_sha256'] == frozen,
        'source_changed_during_command': result.get('source_changed_during_command'),
        'test_results': re.findall(r'^test result:.*$', raw.decode(), re.M),
    })
assert len(commands) == 13, len(commands)
assert len(list((LEG / 'commands').glob('*.start.json'))) == len(commands)
final = next(c for c in commands if c['label'] == 'workspace-final-2')
assert final['exit_code'] == 0 and final['start_matches_final_source']
assert final['source_changed_during_command'] is False

probes = []
for metadata_path in sorted((LEG / 'owner_probes').glob('*.archive.json')):
    metadata = json.loads(metadata_path.read_bytes())
    original = Path(metadata['raw'])
    archive_path = metadata_path.with_name(metadata_path.name.removesuffix('.archive.json') + '.json.gz')
    assert sha(original) == metadata['sha256']
    assert gzip.decompress(archive_path.read_bytes()) == original.read_bytes()
    assert archive_path.read_bytes()[4:8] == bytes(4)
    start_path = (metadata_path.parent / metadata['command']).resolve()
    start = json.loads(start_path.read_bytes())
    assert start['environment']['ALIBI_HOST_OUTPUT'] == str(original)
    probes.append({'metadata': str(metadata_path.relative_to(LEG)),
                   'metadata_sha256': sha(metadata_path), 'original_sha256': sha(original),
                   'archive_sha256': sha(archive_path), 'command_start_sha256': sha(start_path)})
assert len(probes) == 2
assert frozen_sources() == frozen
report = {
    'result': 'passed', 'audited_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'scope': 'Exact owner originals and metadata, including failed development commands. Null source-change fields were absent in older helpers; only matching unchanged-source runs are final-source evidence.',
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'), 'helper_sha256': sha(Path(__file__)),
    'commands': commands, 'development_probe_archives': probes,
}
destination = OUT / 'owner-archive-audit.json'
assert not destination.exists()
write_json(destination, report)
print(json.dumps({'result': 'passed', 'commands': len(commands), 'development_probes': len(probes)}))
