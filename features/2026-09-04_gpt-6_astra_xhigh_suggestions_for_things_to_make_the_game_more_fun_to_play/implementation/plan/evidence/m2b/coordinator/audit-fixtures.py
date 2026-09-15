"""Independently audit exact raw fixtures and fresh-process owner observations."""
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
    raw = path.read_bytes()
    archive = compressed.read_bytes()
    assert len(raw) == record['original_bytes'] and gzip.decompress(archive) == raw
    assert int.from_bytes(archive[4:8], 'little') == 0
    return raw


def category_hashes(raw):
    # Decode offsets only; hash the original UTF-8 slices, without reserializing
    # floats, strings, whitespace or object order through Python.
    text = raw.decode('utf-8')
    decoder = json.JSONDecoder(object_pairs_hook=unique)
    assert text[0] == '{'
    cursor, hashes = 1, {}
    while True:
        key, cursor = decoder.raw_decode(text, cursor)
        assert text[cursor] == ':' and key not in hashes
        start = cursor + 1
        _, cursor = decoder.raw_decode(text, start)
        hashes[key] = hashlib.sha256(text[start:cursor].encode('utf-8')).hexdigest()
        if text[cursor] == '}':
            assert cursor == len(text) - 1
            break
        assert text[cursor] == ','
        cursor += 1
    categories = ('ledger', 'operations', 'backbone', 'round', 'climate',
                  'knowledge', 'law', 'marks', 'animals', 'social', 'continuity',
                  'scheduler', 'night', 'speech', 'cognition_inputs', 'host')
    return [hashes[name] for name in categories]


source = frozen_sources()
collected = decode((OUT / 'fixture-outputs.json').read_bytes())
dataset = (ROOT / collected['results']).parent
records = decode((dataset / 'RESULTS.json').read_bytes())
assert len(records) == 10
assert collected['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
assert collected['release_binary_sha256'] != collected['debug_binary_sha256']
seen, payloads, summaries = set(), {}, []
for record in records:
    state = record['state']
    writer = record['operation'] == 'write'
    kind = f"write-{record['repetition']}" if writer else f"read-{record['compatibility']}"
    assert (state, kind) not in seen
    seen.add((state, kind))
    label = f"fixtures-{collected['attempt']}-{state}-{kind}"
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
    incompatible = not writer and record['compatibility'] == 'incompatible'
    image = collected['debug_binary_sha256' if incompatible else 'release_binary_sha256']
    assert record['binary_sha256'] == image == sha(Path(record['binary']))
    operation = 'write' if writer else 'read'
    assert record['command'] == [record['binary'], '--ignored', '--exact',
        'host_checkpoint::tests_hydration_owner::m2b_hydration_fixture_' + operation,
        '--nocapture', '--test-threads=1']
    env = record['environment_overrides']
    assert env['CATHEDRAL_HEADLESS'] == env['CATHEDRAL_FAKE_BACKEND'] == '1'
    assert env.get('ALIBI_HYDRATION_INITIAL') == ('1' if state == 'initial' else None)
    assert env.get('ALIBI_HYDRATION_EXPECT_INCOMPATIBLE') == ('1' if incompatible else None)
    log = original(record['log'])
    original(record['time'])
    assert b'test result: ok. 1 passed; 0 failed;' in log
    summary = {'state': state, 'kind': kind, 'image_sha256': image}
    if writer:
        assert env['ALIBI_COMPLETE_WORLD_ID'] == collected['lineage_hex']
        raw = original(record['fixture'])
        if state in payloads:
            assert raw == payloads[state]
        else:
            payloads[state] = raw
        wire = decode(raw)
        assert wire['version'] == 1 and wire['profile'] == 'authored'
        assert bytes(wire['world_identity']).hex() == collected['lineage_hex']
        assert bytes(wire['manifest']['host_image']).hex() == image
    else:
        assert record['input'] == collected['outputs'][state]['original']
        raw = Path(record['input']).read_bytes()
        assert raw == payloads[state]
        assert record['input_sha256'] == collected['outputs'][state]['sha256']
        wire = decode(raw)
    if incompatible:
        assert b'hydration fixture rejected before construction:' in log
        assert b'exact running host image mismatch' in log
        assert 'metrics' not in record and not Path(env['ALIBI_HYDRATION_REPORT']).exists()
    else:
        report = decode(original(record['metrics']))
        assert report['schema'] == 'm2b-hydration-fixture-v1'
        assert report['writer'] is writer and report['active'] is (state == 'active')
        assert bytes(report['raw_sha256']).hex() == hashlib.sha256(raw).hexdigest()
        assert bytes(report['host_image']).hex() == image
        assert bytes(report['lineage']).hex() == collected['lineage_hex']
        assert report['boundary'] == wire['boundary']
        assert report['actual_owner_categories'] == 16
        assert [bytes(digest).hex() for digest in report['category_sha256']] == category_hashes(raw)
        assert report['source_app_disposed_before_fresh_resolver'] is True
        capture, hydration = report['capture_cost'], report['hydration_cost']
        assert capture['encoded_bytes'] == len(raw) and capture['characters'] == 520
        assert capture['categories'] == 16 and len(raw) <= 64 * 1024**2
        assert hydration['decoded_upper_bytes'] == capture['expanded_upper_bytes'] + 256 * 1024
        assert hydration['decoded_upper_bytes'] <= 128 * 1024**2
        assert hydration['assets_upper_bytes'] == 64 * 1024**2
        assert hydration['retained_upper_bytes'] == hydration['decoded_upper_bytes'] + hydration['assets_upper_bytes']
        assert report['shared_prompt_runtime_upper_bytes'] == 1024**2
        allocated = report['factory_cumulative_requested_bytes']
        shared = report['retained_distinct_shared_nav_upper_bytes']
        assert report['factory_plus_retained_shared_assets_upper_bytes'] == allocated + shared + 1024**2
        assert allocated + shared + 1024**2 <= hydration['assets_upper_bytes']
        assert 512 * 1024**2 < report['shared_peak_bytes'] <= 1024**3
        summary |= {'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
                    'actual_owner_categories_verified': 16}
    summaries.append(summary)

assert seen == {(state, kind) for state in ('initial', 'active') for kind in
                ('write-1', 'write-2', 'write-3', 'read-compatible', 'read-incompatible')}
assert payloads['initial'] != payloads['active']
assert frozen_sources() == source
write_json(OUT / 'fixture-audit.json', {
    'result': 'passed', 'source_manifest_sha256': collected['source_manifest_sha256'],
    'release_binary_sha256': collected['release_binary_sha256'],
    'debug_binary_sha256': collected['debug_binary_sha256'],
    'results_sha256': sha(dataset / 'RESULTS.json'), 'helper_sha256': sha(Path(__file__)),
    'records': summaries, 'repository_copy_pending': True,
})
print(json.dumps({'result': 'passed', 'writers': 6, 'compatible_readers': 2,
                  'incompatible_readers': 2, 'actual_owner_category_comparisons': 128}))
