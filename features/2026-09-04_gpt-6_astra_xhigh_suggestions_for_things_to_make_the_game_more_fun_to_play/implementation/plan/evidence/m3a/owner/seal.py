"""Seal exact M3a command/image/fixture evidence after frozen verification.

No Cargo or production writes. The coordinator independently audits this record.
"""
import gzip
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import shutil

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parent
ENUMERATOR = EVIDENCE.parent / 'component_input_sources.py'
spec = importlib.util.spec_from_file_location('sources', ENUMERATOR)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        while block := f.read(1024 * 1024):
            h.update(block)
    return h.hexdigest()

def read(path):
    return json.loads(path.read_text())

final_runs = ('format-final-03', 'focused-final-04',
              'release-writer-final-02', 'retain-release-final-01',
              'release-reader-final-01', 'workspace-final-01')
final_path = HERE / 'workspace-final-01-sources.json'
final_map = read(final_path)
assert final_map == module.sources() and len(final_map) == 993
commands = []
raw_by_name = {}
for start_path in sorted(HERE.glob('*-start.json')):
    name = start_path.name.removesuffix('-start.json')
    start = read(start_path)
    result = read(HERE / f'{name}-result.json')
    assert sha(HERE / f'{name}-sources.json') == start['source_map_sha256']
    assert result['source_map_sha256'] == start['source_map_sha256']
    assert sha(HERE / 'run.py') == start['helper_sha256']
    assert sha(ENUMERATOR) == start['enumerator_sha256']
    archive = HERE / f'{name}.log.gz'
    assert sha(archive) == result['archive_sha256']
    archived = archive.read_bytes()
    assert archived[4:8] == b'\0' * 4
    raw = gzip.decompress(archived)
    assert hashlib.sha256(raw).hexdigest() == result['raw_sha256']
    assert Path(result['raw_log']).read_bytes() == raw
    raw_by_name[name] = raw
    # Embedded subprocess JSON has escaped newlines and is NOT a parent group.
    groups = re.findall(rb'^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw, re.MULTILINE)
    counts = {key: sum(int(group[i]) for group in groups)
              for i, key in enumerate(('passed', 'failed', 'ignored'))}
    if name in final_runs:
        assert result['exit_code'] == 0 and result['sources_unchanged']
        assert read(HERE / f'{name}-sources.json') == final_map
        assert start['source_map_sha256'] == sha(final_path)
        assert counts['failed'] == 0
    helper_bindings = [a.split('=', 1)[1] for a in start['command']
                       if a.startswith('ALIBI_M3A_HELPER_SHA256=')]
    if helper_bindings:
        helpers = [Path(start['cwd']) / a for a in start['command'] if a.endswith('.py')]
        assert len(helper_bindings) == len(helpers) == 1
        helper = helpers[0]
        assert helper.resolve().parent == HERE
        assert sha(helper) == helper_bindings[0]
        assert sha(helper).encode() in raw
    commands.append({'name': name, 'start': start, 'result': result,
                     'archive': str(archive.relative_to(EVIDENCE)),
                     'test_groups': len(groups), 'test_counts': counts})
commands.sort(key=lambda c: c['start']['utc'])
by_name = {c['name']: c for c in commands}
assert by_name['focused-final-04']['test_counts'] == {'passed': 16, 'failed': 0, 'ignored': 2}
assert by_name['sim-focused-final-01']['test_counts'] == {'passed': 4, 'failed': 0, 'ignored': 0}
# This focused sim run preceded the final backend-only fixes. Its actual pure
# checkpoint inputs remain byte-identical; final workspace also runs these tests.
prior_sim_map = read(HERE / 'sim-focused-final-01-sources.json')
pure_sim_inputs = [p for p in final_map if p.startswith('crates/cathedral-sim/')]
assert pure_sim_inputs and all(prior_sim_map[p] == final_map[p] for p in pure_sim_inputs)
workspace = by_name['workspace-final-01']
assert workspace['test_counts'] == {'passed': 2249, 'failed': 0, 'ignored': 46}
assert workspace['test_groups'] == 46
formatted = [a for a in by_name['format-final-03']['start']['command'] if a.endswith('.rs')]
assert len(formatted) == 10

focused = raw_by_name['focused-final-04'].decode('utf-8')
faults = re.findall(r'returned-fault (\w+) (Before|After):', focused)
assert len(faults) == len(set(faults)) == 42
children = []
for line in focused.splitlines():
    at = line.find('{"exit_code":')
    if at >= 0:
        child = json.loads(line[at:])
        assert child['subprocess'] == 'm3a-storage'
        children.append(child)
for mode, count, code in [('publish', 21, 86), ('recover', 16, 86), ('read', 37, 0)]:
    selected = [c for c in children if c['mode'] == mode]
    assert len(selected) == count and all(c['exit_code'] == code for c in selected)

retention = read(HERE / 'release-retention.json')
historical_retention = read(HERE / 'release-retention-pre-fix.json')
assert not by_name['release-writer-final-01']['result']['sources_unchanged']
assert sha(Path(historical_retention['preserved_image'])) == historical_retention['image_sha256']
for name, entry in historical_retention['fixture_files'].items():
    assert sha(Path(entry['original'])) == entry['sha256']
    assert sha(HERE / 'fixture-release-pre-fix' / name) == entry['sha256']
writer = read(HERE / 'release-writer-final-report.json')
reader_path = Path('/tmp/alibi-m3a-release-reader-final-report.json')
reader = read(reader_path)
assert sha(Path(retention['preserved_image'])) == retention['image_sha256']
assert sha(Path(retention['original_image'])) == retention['image_sha256']
assert sha(HERE / 'retain_release.py') == retention['helper_sha256']
assert bytes(writer['host_image_sha256']).hex() == retention['image_sha256']
assert writer['host_image_sha256'] == reader['host_image_sha256']
assert writer['payload_sha256'] == reader['payload_sha256']
assert writer['payload_bytes'] == reader['payload_bytes']
assert writer['generation'] == reader['generation']
assert writer['complete_m2_validation'] and reader['complete_m2_validation']
assert writer['samples'] == 32 and reader['samples'] == 0
for name, entry in retention['fixture_files'].items():
    assert sha(Path(entry['original'])) == entry['sha256']
    assert sha(HERE / 'fixture-release' / name) == entry['sha256']
assert sha(Path(retention['original_report'])) == retention['report_sha256']
assert sha(HERE / 'release-writer-final-report.json') == retention['report_sha256']
shutil.copyfile(reader_path, HERE / 'release-reader-final-report.json')

def stats(values):
    ordered = sorted(values)
    return {'samples': len(ordered), 'min': ordered[0],
            **{f'p{n}': ordered[math.ceil(len(ordered)*n/100)-1] for n in (50, 95, 99)},
            'max': ordered[-1]}

result = {
    'status': 'owner frozen verification complete; coordinator acceptance pending',
    'predecessor_commit': 'b0cc0f27c11f596839a53bd69a64ac9269757667',
    'source_scope': module.SOURCE_SCOPE, 'source_count': len(final_map),
    'source_map_sha256': sha(final_path), 'source_map_matches_current': True,
    'seal_helper_sha256': sha(Path(__file__)), 'owner_command_count': len(commands),
    'failed_development_commands': [c['name'] for c in commands if c['result']['exit_code'] != 0],
    'final_runs': list(final_runs), 'formatted_rust_files': formatted,
    'pre_final_pure_sim_focused': {'command': 'sim-focused-final-01',
                                 'byte_identical_final_crate_inputs': len(pure_sim_inputs)},
    'workspace': {'test_counts': workspace['test_counts'], 'groups': workspace['test_groups']},
    'returned_fault_cases': len(faults), 'process_death_cases': 37,
    'fresh_process_readers': 37, 'subprocess_records': children,
    'release': {'retention': retention,
                'reader_report_sha256': sha(reader_path),
                'microseconds': {key: stats(value) if isinstance(value, list) else value
                                 for key, value in writer['microseconds'].items()}},
    'historical_mixed_source_release_retention': historical_retention,
    'scope': 'Backend durability, bounded ownership and small-fixture service measurements only; no M3b adoption, real host-frame, full-city disk-latency, renderer or power-loss acceptance.',
    'commands': commands,
}
(EVIDENCE / 'source_hashes.json').write_bytes(final_path.read_bytes())
(EVIDENCE / 'verification.json').write_text(json.dumps(result, sort_keys=True, indent=2)+'\n')
print(json.dumps({'commands': len(commands), 'sources': len(final_map),
                  'source_map_sha256': sha(final_path), 'workspace': result['workspace'],
                  'release': result['release']['microseconds']}))
