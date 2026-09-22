"""Audit actual archived commands and current sources; no Cargo invocation."""
from pathlib import Path
import datetime
import gzip
import hashlib
import importlib.util
import json
import re
import sys
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ENUMERATOR = HERE.parents[1] / 'component_input_sources.py'
def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as source:
        while chunk := source.read(1024 * 1024): h.update(chunk)
    return h.hexdigest()
def read(path): return json.loads(path.read_text())
def ordered(start, launched, end):
    parse = datetime.datetime.fromisoformat
    assert parse(start['utc']) <= parse(launched['utc']) <= parse(end['utc'])
    assert launched['pid'] > 0
def logs(folder, name, result):
    raw, zipped = folder/(name+'.log'), folder/(name+'.log.gz')
    assert sha(raw) == result['raw_sha256'] and sha(zipped) == result['archive_sha256']
    assert gzip.decompress(zipped.read_bytes()) == raw.read_bytes()
focused = HERE/'focused-01'
run = read(focused/'run.json')
assert sha(HERE/'run_tests.py') == run['helper_sha256']
assert sha(ENUMERATOR) == run['enumerator_sha256']
baseline = read(focused/'sources-before.json')
assert baseline == read(focused/'sources-after.json')
expected = {'sim_shelter_admission':6,'sim_shelter_legacy':1,'installed_startup':7,'local_engine_startup':2}
assert run['suites'] == list(expected)
for suite,count in expected.items():
    result = read(focused/(suite+'-result.json'))
    ordered(read(focused/(suite+'-start.json')),read(focused/(suite+'-launched.json')),result)
    assert result['cargo_exit_code'] == 0 and result['sources_unchanged']
    assert result['passed_tests'] == count and result['successful_nonempty_test_summary']
    assert read(focused/(suite+'-sources-after.json')) == baseline
    assert result['source_map_sha256'] == sha(focused/'sources-before.json')
    assert result['after_source_map_sha256'] == sha(focused/(suite+'-sources-after.json'))
    logs(focused,suite,result)
    assert sum(int(n) for n in re.findall(r'test result: ok\. (\d+) passed;', (focused/(suite+'.log')).read_text())) == count
summary = read(focused/'summary.json')
assert summary['exit_code'] == 0 and summary['passed_tests'] == 16 and summary['sources_unchanged']
build = HERE/'production-01'
start,result = read(build/'start.json'),read(build/'result.json')
ordered(start,read(build/'launched.json'),result)
assert start['helper_sha256'] == sha(HERE/'run_build.py')
assert start['shared_runner_sha256'] == sha(HERE/'run_tests.py')
assert start['enumerator_sha256'] == sha(ENUMERATOR)
assert result['cargo_exit_code'] == 0 and result['sources_unchanged']
assert result['executable_was_run'] is False
assert read(build/'sources-before.json') == read(build/'sources-after.json') == baseline
assert result['source_map_sha256'] == sha(build/'sources-before.json')
assert result['after_source_map_sha256'] == sha(build/'sources-after.json')
assert result['executable_sha256'] == sha(Path(result['executable']))
logs(build,'production-build',result)
identity = read(HERE.parent/'audit/identity.json')
for path, digest in identity['files'].items(): assert sha(Path(path)) == digest
assert identity['numeric_tokens'] == 309 and identity['max_numeric_mantissa'] == 25727
assert identity['max_fractional_digits'] == 2 and not identity['exponent_notation']
spec=importlib.util.spec_from_file_location('component_sources',ENUMERATOR)
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
assert module.sources() == baseline
print(json.dumps({'audit':'passed','current_sources_match':True,'distinct_focused_tests':16,
    'source_map_sha256':sha(build/'sources-before.json'),'production_build_exit':0,
    'executable_sha256':result['executable_sha256']},sort_keys=True))
