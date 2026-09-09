from pathlib import Path
import json, hashlib, gzip, datetime, sys

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0, str(here))
from component_input_sources import SOURCE_SCOPE, sources
e = here / 'm2a14'
sha = lambda b: hashlib.sha256(b).hexdigest()
load = lambda p: json.loads(p.read_bytes())
build = load(e / 'coordinator/release_build.json')
assert build['exit_code'] == 0 and build['source_scope'] == SOURCE_SCOPE
assert build['exact_source_scope_unchanged_after_build'] and build['source_helper_unchanged_after_build']
helper_hash = sha((here / 'component_input_sources.py').read_bytes())
assert build['source_helper_sha256'] == helper_hash
assert build['build_wrapper_sha256'] == sha((e / 'coordinator/release-build.py').read_bytes())
freeze = load(e / 'source_hashes.json')
freeze_hash = sha((e / 'source_hashes.json').read_bytes())
assert freeze_hash == build['source_manifest_sha256'] and sources() == freeze
def verify_archive(record):
    path = (root / record['archive']).resolve()
    assert path.is_relative_to(e)
    packed = path.read_bytes()
    raw = gzip.decompress(packed)
    assert len(packed) == record['archive_bytes'] and sha(packed) == record['archive_sha256']
    assert len(raw) == record['original_bytes'] and sha(raw) == record['original_sha256']
    assert raw == Path(record['original_path']).read_bytes()
    return raw
for record in build['artifacts'].values():
    verify_archive(record)
binaries = {}
for owner in ('scheduler', 'night'):
    name = f'alibi_{owner}_cost'
    value = sha((root / 'target/release/examples' / name).read_bytes())
    assert build['binary_sha256'][name] == value
    reference = build['preserved_references'][name]
    assert sha(Path(reference['path']).read_bytes()) == reference['sha256'] == value
    binaries[owner] = value
identities = {}
runner_outputs = {}
for kind in ('smoke', 'performance'):
    records = load(e / 'coordinator' / (kind + '_runner_commands.json'))
    assert [r['owner'] for r in records] == ['scheduler', 'night']
    for record in records:
        owner = record['owner']
        assert record['status'] == 'completed' and record['exit_code'] == 0
        assert record['kind'] == kind and record['unchanged_source_binary_helpers']
        assert record['source_scope'] == SOURCE_SCOPE and record['source_manifest_sha256'] == freeze_hash
        assert record['binary_sha256'] == binaries[owner]
        assert record['wrapper_sha256'] == sha((e / 'coordinator/run-release-probes.py').read_bytes())
        raw = verify_archive(record)
        directory = e / kind / owner
        rows = load(directory / 'RESULTS.json')
        assert [json.loads(line) for line in raw.splitlines()] == [
            {'name': r['name'], 'phase_us': r['phase_us']} for r in rows]
        identity = load(directory / 'IDENTITY.json')
        assert identity['source_scope'] == SOURCE_SCOPE and identity['source_sha256'] == freeze
        assert identity['binary_sha256'] == binaries[owner]
        assert identity['runner_sha256'] == record['runner_sha256']
        for name, value in identity['runner_sha256'].items():
            assert Path(name).name == name and sha((here / name).read_bytes()) == value
        assert identity['unchanged_source_binary_and_runners_at_end']
        identities[kind + '/' + owner] = sha((directory / 'IDENTITY.json').read_bytes())
        runner_outputs[kind + '/' + owner] = {'original_sha256':sha(raw),'matches_result_rows':True}
        summary = load(directory / 'SUMMARY.json')
        for mode in ('authored', 'populated'):
            debug = load(e / 'development' / f'{owner}_{mode}_inputs.json')
            measured = summary[mode]['metadata']
            for key in ('scenario', 'mode', 'placement', 'counts', 'owner_counts', 'witnesses', 'submitted_requests', 'saved_inputs', 'cost', 'shared_reserved_peak_excluding_running_bytes'):
                assert measured[key] == debug[key], (kind, owner, mode, key)
report = {'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'source_scope':SOURCE_SCOPE,'source_files':len(freeze),'source_manifest_sha256':freeze_hash,
          'source_helper_sha256':helper_hash,'binary_sha256':binaries,'identity_sha256':identities,
          'exact_build_archives':list(build['artifacts']),'runner_output_archives':runner_outputs,
          'debug_and_release_exact_inputs_metadata_and_charges_agree':True,
          'runtime_default_sha256':freeze['default_config.ron']}
path = e / 'coordinator/release_archive_audit.json'
assert not path.exists()
path.write_text(json.dumps(report,indent=2) + '\n')
print(json.dumps({'result':'passed','release_datasets':len(identities),'source_files':len(freeze),'binaries':binaries}))
