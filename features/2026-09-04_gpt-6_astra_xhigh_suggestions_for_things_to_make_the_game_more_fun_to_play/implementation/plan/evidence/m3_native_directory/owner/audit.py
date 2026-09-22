"""Audit archived native-directory records and current sources; never runs Cargo."""
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
    digest = hashlib.sha256()
    with path.open('rb') as source:
        while chunk := source.read(1024 * 1024): digest.update(chunk)
    return digest.hexdigest()
def read(path): return json.loads(path.read_text())
def ordered(start, launched, end):
    parse = datetime.datetime.fromisoformat
    assert parse(start['utc']) <= parse(launched['utc']) <= parse(end['utc'])
    assert launched['pid'] > 0
def log_ok(directory, label, result):
    raw, zipped = directory / (label + '.log'), directory / (label + '.log.gz')
    assert result['raw_sha256'] == sha(raw)
    assert result['archive_sha256'] == sha(zipped)
    assert gzip.decompress(zipped.read_bytes()) == raw.read_bytes()
focused = HERE / 'focused-01'
run = read(focused / 'run.json')
assert run['helper_sha256'] == sha(HERE / 'run_tests.py')
assert run['enumerator_sha256'] == sha(ENUMERATOR)
baseline = read(focused / 'sources-before.json')
assert baseline == read(focused / 'sources-after.json')
summary = read(focused / 'summary.json')
assert summary['exit_code'] == 0 and summary['sources_unchanged']
assert summary['passed_tests'] == 15
for suite, count in [('backend_capture', 9), ('installed_startup', 6)]:
    result = read(focused / (suite + '-result.json'))
    ordered(read(focused / (suite + '-start.json')), read(focused / (suite + '-launched.json')), result)
    assert result['cargo_exit_code'] == 0 and result['sources_unchanged']
    assert result['passed_tests'] == count and result['successful_nonempty_test_summary']
    assert result['source_map_sha256'] == sha(focused / 'sources-before.json')
    assert baseline == read(focused / (suite + '-sources-after.json'))
    log_ok(focused, suite, result)
native = HERE / 'native-01'
assert read(native / 'sources-before.json') == read(native / 'sources-after.json') == baseline
for phase in ['compile', 'probe']:
    start, result = read(native / (phase + '-start.json')), read(native / (phase + '-result.json'))
    ordered(start, read(native / (phase + '-launched.json')), result)
    assert result['exit_code'] == 0
    assert start['runner_sha256'] == sha(HERE / 'run_native.py')
    assert start['shared_runner_sha256'] == sha(HERE / 'run_tests.py')
    assert start['enumerator_sha256'] == sha(ENUMERATOR)
    assert start['probe_c_sha256'] == sha(HERE / 'native_probe.c')
    assert start['compiler_sha256'] == sha(Path(start['compiler']))
    assert start['libc_sha256'] == sha(Path(start['libc']))
    assert start['environment']['GLIBC_TUNABLES'] == 'glibc.malloc.hugetlb=0'
    log_ok(native, phase, result)
    if phase == 'probe':
        assert start['probe_library_sha256'] == sha(native / 'native_probe.so')
        assert start['test_executable_sha256'] == sha(Path(start['test_executable']))
native_summary = read(native / 'summary.json')
assert native_summary['exit_code'] == 0 and native_summary['sources_unchanged']
assert native_summary['successful_exact_test']
assert 'test result: ok. 1 passed; 0 failed;' in (native / 'probe.log').read_text()
build = HERE / 'production-01'
start, result = read(build / 'start.json'), read(build / 'result.json')
ordered(start, read(build / 'launched.json'), result)
assert start['helper_sha256'] == sha(HERE / 'run_build.py')
assert start['shared_runner_sha256'] == sha(HERE / 'run_tests.py')
assert start['enumerator_sha256'] == sha(ENUMERATOR)
assert result['cargo_exit_code'] == 0 and result['sources_unchanged']
assert result['executable_was_run'] is False
assert read(build / 'sources-before.json') == read(build / 'sources-after.json') == baseline
assert result['executable_sha256'] == sha(Path(result['executable']))
log_ok(build, 'production-build', result)
platform = HERE.parent / 'audit'
identity = read(platform / 'identity.json')
for function, command in identity['disassembly'].items():
    assert command['exit_code'] == 0
    assert command['sha256'] == sha(platform / (function + '.disassembly.txt'))
assert identity['symbols_sha256'] == sha(platform / 'directory-symbols.txt')
assert identity['libc_sha256'] == sha(Path(identity['libc_path']))
assert identity['rust_std_sha256'] == sha(Path(identity['rust_std_path']))
spec = importlib.util.spec_from_file_location('component_sources', ENUMERATOR)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
assert module.sources() == baseline
print(json.dumps({'audit':'passed', 'source_map_sha256':sha(build / 'sources-before.json'),
    'distinct_focused_tests':16, 'production_build_exit':result['cargo_exit_code'],
    'executable_sha256':result['executable_sha256'], 'current_sources_match':True}, sort_keys=True))
