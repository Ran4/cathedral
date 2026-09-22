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
def suite(folder,name,count,code,baseline):
    result=read(folder/(name+'-result.json'))
    ordered(read(folder/(name+'-start.json')),read(folder/(name+'-launched.json')),result)
    assert result['cargo_exit_code']==code and result['sources_unchanged']
    assert result['passed_tests']==count and result['successful_nonempty_test_summary']==(count>0)
    assert read(folder/(name+'-sources-after.json'))==baseline
    assert result['source_map_sha256']==sha(folder/'sources-before.json')
    assert result['after_source_map_sha256']==sha(folder/(name+'-sources-after.json'))
    logs(folder,name,result)
    assert sum(int(n) for n in re.findall(r'test result: ok\. (\d+) passed;', (folder/(name+'.log')).read_text()))==count

def run(folder,names):
    record=read(folder/'run.json')
    assert sha(HERE/'run_tests.py')==record['helper_sha256']
    assert sha(ENUMERATOR)==record['enumerator_sha256']
    assert record['suites']==names
    baseline=read(folder/'sources-before.json')
    assert baseline==read(folder/'sources-after.json')
    return baseline

focused=HERE/'focused-01'
old=run(focused,['sim_sound_admission','sim_sound_legacy','installed_startup','local_engine_startup'])
suite(focused,'sim_sound_admission',8,0,old)
suite(focused,'sim_sound_legacy',7,0,old)
suite(focused,'installed_startup',0,101,old)
assert 'E0609' in (focused/'installed_startup.log').read_text()
summary=read(focused/'summary.json')
assert summary['exit_code']==101 and summary['passed_tests']==15 and summary['sources_unchanged']
assert not (focused/'local_engine_startup-result.json').exists()
host=HERE/'host-02'
baseline=run(host,['installed_startup','local_engine_startup'])
suite(host,'installed_startup',10,0,baseline)
suite(host,'local_engine_startup',2,0,baseline)
summary=read(host/'summary.json')
assert summary['exit_code']==0 and summary['passed_tests']==12 and summary['sources_unchanged']
delta={p:{'map_a':old.get(p),'map_b':baseline.get(p)} for p in sorted(old.keys()|baseline.keys()) if old.get(p)!=baseline.get(p)}
assert set(delta)=={'src/smart_actors/local_engine/startup_tests.rs'}
recorded=read(HERE/'source-delta.json')
assert recorded['delta']==delta and recorded['from_map']==sha(focused/'sources-before.json') and recorded['to_map']==sha(host/'sources-before.json')
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
assert identity['toml_features'] == ['default','display','parse','serde','std']
features=HERE/'features-01'
feature_result=read(features/'result.json')
ordered(read(features/'start.json'),read(features/'launched.json'),feature_result)
assert feature_result['exit_code']==0 and sha(features/'features.log')==feature_result['raw_sha256']
logs(features,'features',read(features/'archive.json'))
feature_text=(features/'features.log').read_text()
assert 'preserve_order' not in feature_text and 'unbounded' not in feature_text
for name in identity['toml_features']: assert 'toml feature "'+name+'"' in feature_text
spec=importlib.util.spec_from_file_location('component_sources',ENUMERATOR)
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
assert module.sources() == baseline
print(json.dumps({'audit':'passed','current_sources_match':True,'distinct_focused_tests':27,
    'sim_source_map_sha256':sha(focused/'sources-before.json'),'source_map_sha256':sha(build/'sources-before.json'),'production_build_exit':0,
    'executable_sha256':result['executable_sha256']},sort_keys=True))
