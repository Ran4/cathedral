"""Audit exact prepared-V2 fixtures and all fresh-process command evidence."""
import gzip
import hashlib
import json
from pathlib import Path
from release_common import LEG, OUT, ROOT, frozen_sources, sha, write_json


def unique(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, f'duplicate JSON key: {key}'
        result[key] = value
    return result


def decode(raw):
    return json.loads(raw, object_pairs_hook=unique)


def original(record):
    path, compressed = Path(record['original']), ROOT / record['archive']
    assert sha(path) == record['original_sha256']
    assert sha(compressed) == record['archive_sha256']
    raw, archive = path.read_bytes(), compressed.read_bytes()
    assert len(raw) == record['original_bytes'] and gzip.decompress(archive) == raw
    assert int.from_bytes(archive[4:8], 'little') == 0
    return raw


source = frozen_sources()
collected = decode((OUT / 'fixture-outputs.json').read_bytes())
dataset = (ROOT / collected['results']).parent
records = decode((dataset / 'RESULTS.json').read_bytes())
assert len(records) == 8
assert collected['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
seen, payloads, summaries = set(), {}, []
for record in records:
    mode = record['mode']
    writer = record['operation'] == 'write'
    kind = f"write-{record['repetition']}" if writer else 'read'
    assert (mode, kind) not in seen
    seen.add((mode, kind))
    label = f"fixtures-{collected['attempt']}-{mode}-{kind}"
    start = decode((dataset / f'{label}.start.json').read_bytes())
    result = decode((dataset / f'{label}.result.json').read_bytes())
    assert all(result[key] == value for key, value in start.items())
    assert all(record[key] == value for key, value in result.items())
    assert record['exit_code'] == 0
    assert all(record[key] is True for key in ('unchanged_source', 'unchanged_helpers', 'unchanged_binary'))
    assert record['source_manifest_sha256'] == collected['source_manifest_sha256']
    assert record['source_count'] == len(source)
    for path, digest in record['helper_sha256'].items():
        assert sha(ROOT / path) == digest
    image = collected['release_binary_sha256']
    assert record['binary_sha256'] == image == sha(Path(record['binary']))
    assert record['command'] == [record['binary'], '--ignored', '--exact',
        'host_checkpoint::tests_continuation_owner::m2c_continuation_probe',
        '--nocapture', '--test-threads=1']
    env = record['environment_overrides']
    assert env['CATHEDRAL_HEADLESS'] == env['CATHEDRAL_FAKE_BACKEND'] == '1'
    assert env['ALIBI_CONTINUATION_MODE'] == mode and env['ALIBI_CONTINUATION_SAMPLES'] == '1'
    assert env.get('ALIBI_CONTINUATION_READ_FIXTURE') == (None if writer else '1')
    assert b'test result: ok. 1 passed; 0 failed;' in original(record['log'])
    original(record['time'])
    if writer:
        assert env['ALIBI_COMPLETE_WORLD_ID'] == collected['lineage_hex']
        raw = original(record['fixture'])
        if mode in payloads:
            assert raw == payloads[mode]
        else:
            payloads[mode] = raw
    else:
        assert record['input'] == collected['outputs'][mode]['original']
        raw = Path(record['input']).read_bytes()
        assert raw == payloads[mode]
        assert record['input_sha256'] == collected['outputs'][mode]['sha256']
    digest = hashlib.sha256(raw).hexdigest()
    wire = decode(raw)
    assert wire['version'] == 1 and wire['profile'] == mode
    assert bytes(wire['world_identity']).hex() == collected['lineage_hex']
    assert bytes(wire['manifest']['host_image']).hex() == image
    categories = ('ledger', 'operations', 'backbone', 'round', 'climate', 'knowledge', 'law',
                  'marks', 'animals', 'social', 'continuity', 'scheduler', 'night', 'speech',
                  'cognition_inputs', 'host')
    assert all(name in wire for name in categories)
    speech = wire['speech']
    assert set(speech) == {'version', 'base', 'interrupted'} and speech['version'] == 2
    assert speech['base']['version'] == 1 and speech['interrupted']
    assert not speech['base']['state']['accepted_recordings']
    report = decode(original(record['metrics']))
    assert report['schema'] == 'm2c-continuation-v1' and report['profile'] == mode
    assert report['fixture_reader'] is (not writer) and report['samples'] == 1
    assert report['characters'] == (520 if mode == 'authored' else 2520)
    assert bytes(report['fixture_sha256']).hex() == bytes(report['resave_sha256']).hex() == digest
    assert len(report['input_sha256']) == 1
    if not writer:
        assert bytes(report['input_sha256'][0]).hex() == digest
    assert bytes(report['host_image']).hex() == image
    assert report['immediate_v2_resave_bytes'] == len(raw)
    assert len(raw) <= (64 if mode == 'authored' else 128) * 1024**2
    assert report['second_preparation_exact_bytes'] and report['capture_boundary_unchanged']
    assert report['unchanged_categories_equal'] and report['unchanged_category_count'] == 8
    prepared = report['reports'][0]
    assert prepared['interruption_receipts'] == 1 and 1 <= prepared['interrupted_inputs'] <= 64
    assert 0 < prepared['typed_upper_bytes'] <= 128 * 1024**2
    assert 512 * 1024**2 < report['shared_peak_bytes'] <= 1024**3
    summaries.append({'mode': mode, 'kind': kind, 'image_sha256': image,
                      'bytes': len(raw), 'sha256': digest, 'source_pending_shape': report['pending_shapes'][0]})

assert seen == {(mode, kind) for mode in ('authored', 'populated')
                for kind in ('write-1', 'write-2', 'write-3', 'read')}
assert payloads['authored'] != payloads['populated']
assert frozen_sources() == source
write_json(OUT / 'fixture-audit.json', {
    'result': 'passed', 'source_manifest_sha256': collected['source_manifest_sha256'],
    'release_binary_sha256': collected['release_binary_sha256'],
    'results_sha256': sha(dataset / 'RESULTS.json'), 'helper_sha256': sha(Path(__file__)),
    'records': summaries, 'repository_copy_pending': True,
})
print(json.dumps({'result': 'passed', 'writers': 6, 'same_image_readers': 2,
                  'exact_fresh_reader_input_and_resave_checks': 2}))
