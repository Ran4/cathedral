"""Generate and validate frozen-image fixtures in /tmp before repository publication.

This script deliberately does not copy outputs into src: that later operation
has an explicit fixture-output-only source-map delta, after release measurements.
"""
from pathlib import Path
import json
import os
import sys
from release_common import LEG, OUT, ROOT, archive, capture, frozen_sources, sha, write_json

assert len(sys.argv) in (2, 3), 'supply the preserved final debug executable and optional attempt'
debug = Path(sys.argv[1]).resolve()
assert debug.is_file()
attempt = int(sys.argv[2]) if len(sys.argv) == 3 else 1
assert attempt > 0
assert not any(key in os.environ for key in (
    'ALIBI_COMPLETE_INITIAL', 'ALIBI_COMPLETE_FIXTURE',
    'ALIBI_COMPLETE_EXPECT_IMAGE_MISMATCH',
))
build = json.loads((OUT / 'release_build.json').read_bytes())
release = Path(build['reference_binary'])
assert build['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
assert sha(release) == build['reference_binary_sha256']
assert sha(debug) != sha(release), 'debug mismatch witness must use a different image'
dataset = 'fixture-generation' if attempt == 1 else f'fixture-generation-attempt-{attempt}'
destination = LEG / dataset
assert not destination.exists()
destination.mkdir()
world_id = '4d326131362d666978747572652d3031'
assert len(bytes.fromhex(world_id)) == 16
write_json(destination / 'IDENTITY.json', {
    'source_scope': build['source_scope'], 'source_hashes': frozen_sources(),
    'source_manifest_sha256': build['source_manifest_sha256'],
    'release': str(release), 'release_sha256': sha(release),
    'debug': str(debug), 'debug_sha256': sha(debug),
    'initial_world_identity_hex': world_id,
    'repository_fixture_outputs_created': False,
})
results = []
chosen = {}
for state in ('initial', 'active'):
    reference = None
    for repetition in range(1, 4):
        label = f'{dataset}-{state}-writer-{repetition}'
        payload = Path(f'/tmp/alibi-m2a16-{label}.json')
        report = Path(f'/tmp/alibi-m2a16-{label}.metrics.json')
        assert not payload.exists() and not report.exists()
        environment = {
            'ALIBI_COMPLETE_MODE': 'authored', 'ALIBI_COMPLETE_SAMPLES': '1',
            'ALIBI_COMPLETE_FIXTURE': str(payload), 'ALIBI_COMPLETE_REPORT': str(report),
            'ALIBI_COMPLETE_WORLD_ID': world_id,
        }
        if state == 'initial':
            environment['ALIBI_COMPLETE_INITIAL'] = '1'
        command = [str(release), '--ignored', '--exact',
                   'host_checkpoint::tests_complete_owner::m2a16_complete_probe',
                   '--nocapture', '--test-threads=1']
        result = capture(label, command, destination, environment, release)
        result |= {
            'state': state, 'repetition': repetition,
            'payload': archive(payload, destination / f'{state}-{repetition}.json.gz'),
            'metrics': archive(report, destination / f'{state}-{repetition}.metrics.json.gz'),
        }
        results.append(result)
        write_json(destination / 'RESULTS.json', results)
        raw = payload.read_bytes()
        value = json.loads(raw)
        assert bytes(value['manifest']['host_image']).hex() == sha(release)
        assert bytes(value['world_identity']).hex() == world_id
        assert value['version'] == 1 and value['profile'] == 'authored'
        metadata = json.loads(report.read_bytes())
        assert metadata['schema'] == 1 and metadata['scenario'] == 'actual-complete-host-boundary-v1'
        assert metadata['active'] is (state == 'active')
        assert metadata['characters'] == 520 and metadata['samples'] == 1
        assert metadata['costs'][0]['encoded_bytes'] == len(raw)
        if reference is None:
            reference = raw
            chosen[state] = str(payload)
        else:
            assert raw == reference, 'fresh fixture writer changed exact complete bytes'
    for mode, binary in (('same-image', release), ('incompatible-image', debug)):
        label = f'{dataset}-{state}-{mode}-validator'
        environment = {'ALIBI_COMPLETE_FIXTURE_IN': chosen[state]}
        if mode == 'incompatible-image':
            environment['ALIBI_COMPLETE_EXPECT_IMAGE_MISMATCH'] = '1'
        command = [str(binary), '--ignored', '--exact',
                   'host_checkpoint::tests_complete_owner::m2a16_verify_complete_fixture',
                   '--nocapture', '--test-threads=1']
        result = capture(label, command, destination, environment, binary)
        results.append(result | {'state': state, 'verification': mode})
        write_json(destination / 'RESULTS.json', results)
        log = Path(result['log']['original']).read_text()
        marker = ('SAME_IMAGE_EXACT_BYTES_ACCEPTED' if mode == 'same-image'
                  else 'EXPECTED_INCOMPATIBLE_HOST_IMAGE')
        assert marker in log and 'test result: ok. 1 passed; 0 failed;' in log
assert Path(chosen['initial']).read_bytes() != Path(chosen['active']).read_bytes()
write_json(OUT / 'fixtures-collected.json', {
    'dataset': str(destination.relative_to(ROOT)), 'chosen_originals': chosen,
    'identity_sha256': sha(destination / 'IDENTITY.json'),
    'results_sha256': sha(destination / 'RESULTS.json'),
    'source_manifest_sha256': build['source_manifest_sha256'],
    'release_sha256': sha(release), 'debug_sha256': sha(debug),
    'repository_copy_pending': True,
})
