from pathlib import Path
import datetime, gzip, hashlib, json, re, sys

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0,str(here))
from component_input_sources import SOURCE_SCOPE, sources
e = here / 'm2a13'
sha = lambda b: hashlib.sha256(b).hexdigest()
records = json.loads((e/'commands.json').read_text())
verification = json.loads((e/'verification.json').read_text())
freeze = json.loads((e/'source_hashes.json').read_bytes())
freeze = freeze.get('files',freeze)
assert sources() == freeze
helper = sha((here/'component_input_sources.py').read_bytes())
totals, frozen_commands = {}, []
for r in records:
    path = (root/r['archive']).resolve()
    assert path.is_relative_to(e)
    packed = path.read_bytes(); raw = gzip.decompress(packed)
    assert sha(packed) == r['archive_sha256'] and len(packed) == r['archive_bytes']
    assert sha(raw) == r['original_sha256'] == r['decompressed_sha256']
    assert len(raw) == r['original_bytes'] and raw == Path(r['original_path']).read_bytes()
    source = (root/r['source_manifest']).read_bytes()
    assert sha(source) == r['source_manifest_sha256']
    assert r['source_scope'] == SOURCE_SCOPE and r['source_helper_sha256'] == helper
    if json.loads(source) == freeze: frozen_commands.append(r['name'])
    results = [dict(zip(['passed','failed','ignored','measured','filtered_out'],map(int,m)))
               for m in re.findall(rb'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out',raw)]
    assert results == r['test_results']
    totals[r['name']] = {k:sum(t[k] for t in results) for k in ['passed','failed','ignored']}
    totals[r['name']]['targets'] = len(results)
for name, expected in verification['final_verification'].items():
    record = next(r for r in records if r['name'] == name)
    assert record['exit_code'] == 0 and name in frozen_commands
    assert totals[name]['failed'] == 0
    assert all(totals[name][key] == expected[key] for key in ['passed','failed','ignored'])
    assert totals[name]['targets'] == expected['targets_with_test_results']
workspace = verification['workspace']['command_name']
assert totals[workspace]['passed'] >= 2090 and totals[workspace]['ignored'] >= 33
failures = [r['name'] for r in records if r['exit_code']]
assert verification['development_failures'] == failures
report = {
    'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'command_records':len(records),'cargo_records':sum(Path(r['command'][0]).name=='cargo' for r in records),
    'checks':['archive and exact retained original hashes/lengths','test totals independently parsed from originals','per-command source manifest hashes and v2 helper provenance','final verification uses complete frozen v2 source scope'],
    'frozen_source_commands':frozen_commands,'totals':totals,'development_failures':failures,
    'source_scope':SOURCE_SCOPE,'source_manifest_sha256':sha((e/'source_hashes.json').read_bytes()),
}
path = e/'coordinator/log_archive_audit.json'
assert not path.exists()
path.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
