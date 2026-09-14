"""Check the build and preserve the coordinator's refused malformed input run."""
import importlib.util
import json
from pathlib import Path
from release_common import ROOT, LEG, OUT, sha, write_json

spec = importlib.util.spec_from_file_location('audit', OUT / 'audit-release-probes.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)
rows = []
for directory, label, expected in ((OUT, 'release-build-1', 0), (LEG / 'smoke', 'smoke-authored-1', 101)):
    start = json.loads((directory / (label + '.start.json')).read_bytes())
    record = json.loads((directory / (label + '.result.json')).read_bytes())
    assert all(record[k] == value for k, value in start.items())
    assert record['exit_code'] == expected
    assert all(record[k] is True for k in ('unchanged_source', 'unchanged_helpers', 'unchanged_binary'))
    assert record['source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
    for path, digest in record['helper_sha256'].items():
        resolved = ROOT / path
        if label == 'smoke-authored-1' and resolved.name == 'run-release-probes.py':
            resolved = OUT / 'run-release-probes-before-identity-fix.py'
        assert sha(resolved) == digest
    log = audit.check_archive(record['log']).decode()
    audit.check_archive(record['time'])
    if expected:
        assert 'world lineage identity is unavailable' in log
        assert len(bytes.fromhex(record['environment_overrides']['ALIBI_COMPLETE_WORLD_ID'])) == 17
        assert 'test result: FAILED. 0 passed; 1 failed;' in log
    else:
        assert '"reason":"build-finished","success":true' in log
    rows.append({'label': label, 'exit': expected, 'log_sha256': record['log']['original_sha256'],
                 'result_sha256': sha(directory / (label + '.result.json'))})
build = json.loads((OUT / 'release_build.json').read_bytes())
binary = Path(build['reference_binary'])
assert sha(binary) == build['reference_binary_sha256'] and binary.stat().st_size == build['binary_bytes']
report = {'result': 'passed', 'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'commands': rows, 'release_binary_sha256': sha(binary), 'helper_sha256': sha(Path(__file__)),
    'identity_input_correction': json.loads((OUT / 'identity-input-correction.json').read_bytes()),
    'prior_auditor_attempt': 'An inline import failed before auditing because its module directory was not on sys.path. This saved standalone helper uses the ordinary script import path.',
    'scope': 'Exact successful build and failed coordinator smoke input. No production change; successful retry and performance datasets have separate audits.'}
destination = OUT / 'release-history-audit.json'
assert not destination.exists()
write_json(destination, report)
print(json.dumps({'result': 'passed', 'commands': len(rows), 'release_bytes': binary.stat().st_size}))
