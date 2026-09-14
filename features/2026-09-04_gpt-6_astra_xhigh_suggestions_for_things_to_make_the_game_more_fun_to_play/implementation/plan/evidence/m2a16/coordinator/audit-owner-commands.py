"""Audit original owner attempts, including failures and final workspace evidence."""
import argparse
import datetime
import gzip
import json
from pathlib import Path
import re
from release_common import ROOT, LEG, OUT, frozen_sources, sha, write_json

parser = argparse.ArgumentParser()
parser.add_argument('--final-label', required=True)
parser.add_argument('--command-count', required=True, type=int)
parser.add_argument('--passed', required=True, type=int)
parser.add_argument('--ignored', required=True, type=int)
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
assert re.fullmatch(r'[a-z0-9-]+', args.final_label)
assert not args.output.exists()
frozen = frozen_sources()
directory = LEG / 'owner-commands'
records = []
for result_path in sorted(directory.glob('*.result.json')):
    label = result_path.name.removesuffix('.result.json')
    assert re.fullmatch(r'[a-z0-9-]+', label)
    original_base = Path('/tmp/alibi-m2a16-' + label)
    start_path = directory / (label + '.start.json')
    assert start_path.read_bytes() == original_base.with_suffix('.start.json').read_bytes()
    assert result_path.read_bytes() == original_base.with_suffix('.result.json').read_bytes()
    start = json.loads(start_path.read_bytes())
    result = json.loads(result_path.read_bytes())
    archive = directory / (label + '.log.gz')
    zipped = archive.read_bytes()
    raw = original_base.with_suffix('.log').read_bytes()
    assert zipped == original_base.with_suffix('.log.gz').read_bytes()
    assert zipped[4:8] == bytes(4) and gzip.decompress(zipped) == raw
    assert sha(original_base.with_suffix('.log')) == result['log_sha256']
    assert result['end'] >= start['start']
    helpers = start.get('helpers')
    if helpers:
        wrapper = directory / Path(helpers['wrapper_path']).name
        assert sha(wrapper) == helpers['wrapper_sha256']
        assert wrapper.read_bytes() == Path(helpers['wrapper_path']).read_bytes()
        assert sha(ROOT / helpers['source_enumerator_path']) == helpers['source_enumerator_sha256']
    source = start.get('source_map')
    if result.get('source_unchanged') is True:
        assert result['changed_paths'] == [] and result['source_after'] == source
    environment = start.get('environment', {})
    results = [tuple(map(int, match)) for match in re.findall(
        rb'^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw, re.M)]
    record = {
        'label': label, 'command': start['command'], 'exit': result['exit'],
        'elapsed_seconds': result['end'] - start['start'],
        'original_log': str(original_base.with_suffix('.log')),
        'original_bytes': len(raw), 'original_sha256': result['log_sha256'],
        'archive_sha256': sha(archive), 'start_sha256': sha(start_path),
        'result_sha256': sha(result_path), 'source_scope': start.get('source_scope'),
        'source_count': None if source is None else len(source),
        'source_matches_final_freeze': source == frozen,
        'source_unchanged': result.get('source_unchanged'),
        'helper_identity_recorded': helpers is not None,
        'headless_fake_recorded': environment.get('CATHEDRAL_HEADLESS') == '1'
                                  and environment.get('CATHEDRAL_FAKE_BACKEND') == '1',
        'test_groups': len(results),
        'test_totals': [sum(group[index] for group in results) for index in range(3)],
    }
    if label == args.final_label:
        assert result['exit'] == 0 and result['source_unchanged'] is True
        assert source == frozen and start['source_scope'] == 'component-inputs-v2'
        assert start['command'] == ['/home/ran/.cargo/bin/cargo', 'test', '--offline', '-j1', '--workspace']
        assert record['headless_fake_recorded'] and helpers
        for key in ('LDFLAGS', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
                    'CARGO_BUILD_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'):
            assert key not in environment and key in start['unset_build_environment']
        assert len(results) == 46
        assert record['test_totals'] == [args.passed, 0, args.ignored]
        public = re.findall(rb'^test host_checkpoint::tests_complete_public::([^ ]+) \.\.\. (\w+)$', raw, re.M)
        assert len(public) == 12 and all(status == b'ok' for _, status in public)
        for test in ('complete_world_event_fence_refuses_without_draining_or_writing',
                     'complete_place_registry_rebuilt_indexes_fit_precharged_layout',
                     'typed_wrong_type_and_identifier_diagnostics_are_bounded',
                     'subordinate_owner_keeps_slot_and_charge_until_actual_drop'):
            assert re.search(rb'^test [^\n]*::' + test.encode() + rb' \.\.\. ok$', raw, re.M)
        record['independent_public_tests'] = [name.decode() for name, _ in public]
    records.append(record)
assert len(records) == args.command_count
assert len(list(directory.glob('*.start.json'))) == args.command_count
assert any(record['label'] == args.final_label for record in records)
assert frozen_sources() == frozen
report = {
    'result': 'passed', 'audited_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'helper_sha256': sha(Path(__file__)), 'final_label': args.final_label,
    'scope': 'Exact original command evidence. Missing early helper/source-change metadata stays missing; only the explicitly selected unchanged final workspace establishes final verification.',
    'commands': records,
}
write_json(args.output, report)
print(json.dumps({'result': 'passed', 'commands': len(records),
                  'workspace_passed': args.passed, 'workspace_ignored': args.ignored,
                  'public_passes': 12}))
