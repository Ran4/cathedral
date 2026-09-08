from pathlib import Path
import json, hashlib, gzip, datetime, sys
root = Path('/home/ran/src/rust/cathedralbevy')
here = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0,str(here))
from component_input_sources import SOURCE_SCOPE, sources
e = here/'m2a11'
sha = lambda b: hashlib.sha256(b).hexdigest()
load = lambda p: json.loads(p.read_bytes())
build = load(e/'coordinator/release_build.json')
assert build['exit_code'] == 0
assert build['source_scope'] == SOURCE_SCOPE
assert build['exact_source_scope_unchanged_after_build'] and build['source_helper_unchanged_after_build']
helper_hash = sha((here/'component_input_sources.py').read_bytes())
assert build['source_helper_sha256'] == helper_hash
freeze = load(e/'source_hashes.json')
assert sha((e/'source_hashes.json').read_bytes()) == build['source_manifest_sha256']
assert sources() == freeze
artifacts = []
for name, record in build['artifacts'].items():
    path = (root/record['archive']).resolve()
    assert path.is_relative_to(e)
    packed = path.read_bytes(); raw = gzip.decompress(packed)
    assert len(packed) == record['archive_bytes'] and sha(packed) == record['archive_sha256']
    assert len(raw) == record['original_bytes'] and sha(raw) == record['original_sha256']
    assert raw == Path(record['original_path']).read_bytes()
    artifacts.append(name)
binary = root/'target/release/examples/alibi_scheduler_cost'
binary_hash = sha(binary.read_bytes())
assert build['binary_sha256']['alibi_scheduler_cost'] == binary_hash
reference = build['preserved_reference']
assert sha(Path(reference['path']).read_bytes()) == reference['sha256'] == binary_hash
identities = {}
runner_outputs = {}
for directory in ['smoke','performance']:
    identity = load(e/directory/'IDENTITY.json')
    assert identity['source_scope'] == SOURCE_SCOPE
    assert identity['source_sha256'] == freeze
    assert identity['binary_sha256'] == binary_hash
    assert identity['runner_sha256']['component_input_sources.py'] == helper_hash
    assert identity['unchanged_source_binary_and_runners_at_end']
    identities[directory] = sha((e/directory/'IDENTITY.json').read_bytes())
    original = Path(f'/tmp/alibi-m2a11-release-{directory}-runner.log')
    raw = original.read_bytes()
    rows = load(e/directory/'RESULTS.json')
    assert [json.loads(line) for line in raw.splitlines()] == [
        {'name':row['name'],'phase_us':row['phase_us']} for row in rows]
    packed_path = e/f'coordinator/{directory}_runner.log.gz'
    assert not packed_path.exists()
    packed_path.write_bytes(gzip.compress(raw,mtime=0))
    runner_outputs[directory] = {'original_path':str(original),'original_bytes':len(raw),'original_sha256':sha(raw),'archive':str(packed_path.relative_to(root)),'archive_bytes':packed_path.stat().st_size,'archive_sha256':sha(packed_path.read_bytes()),'matches_result_rows':True}
summary = load(e/'performance/SUMMARY.json')
for mode in ['authored','populated']:
    debug = load(e/f'development/{mode}_bounded_smoke.json')
    measured = summary[mode]['metadata']
    for key in ['counts','witnesses','cost','shared_reserved_peak_excluding_running_bytes']:
        assert measured[key] == debug[key], (mode,key)
stdlib = load(e/'coordinator/stdlib_allocation_audit.json')
assert stdlib['owner_proof_sha256'] == sha((e/'development/stdlib_allocation_audit.json').read_bytes())
report = {'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source_scope':SOURCE_SCOPE,'source_files':len(freeze),'source_manifest_sha256':sha((e/'source_hashes.json').read_bytes()),'source_helper_sha256':helper_hash,'binary_sha256':binary_hash,'identity_sha256':identities,'exact_release_archives':artifacts,'runner_output_archives':runner_outputs,'debug_and_release_semantic_metadata_agree':True,'stdlib_owner_proof_unchanged':True,'runtime_default_sha256':freeze['default_config.ron']}
path = e/'coordinator/release_archive_audit.json'
assert not path.exists()
path.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
