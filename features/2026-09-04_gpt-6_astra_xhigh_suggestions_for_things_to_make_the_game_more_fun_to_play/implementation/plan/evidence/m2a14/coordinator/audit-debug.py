from pathlib import Path
import hashlib, json, sys, datetime

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0, str(here))
from component_input_sources import sources
from audit_m2_component_probes import validate_cognition_inputs, COGNITION_PRIMARY_SHA256
from run_m2_cognition_inputs_probes import primary, EXPECTED_PRIMARY_HASHES
assert EXPECTED_PRIMARY_HASHES == COGNITION_PRIMARY_SHA256
e = here / 'm2a14'
assert sources() == json.loads((e / 'source_hashes.json').read_bytes())
sha = lambda b: hashlib.sha256(b).hexdigest()
owner_report = json.loads((e / 'development/probe_summary.json').read_bytes())
assert len(owner_report['rows']) == 4
rows = []
for row in owner_report['rows']:
    files = {}
    for name in ('historical', 'default', 'inputs'):
        p = Path(row[name + '_path']).resolve()
        assert p.is_relative_to(here)
        raw = p.read_bytes()
        assert sha(raw) == row[name + '_sha256']
        files[name] = json.loads(raw)
    default, historical, data = files['default'], files['historical'], files['inputs']
    keys = ('scenario', 'mode', 'placement', 'counts', 'witnesses', 'cost', 'shared_reserved_peak_excluding_running_bytes')
    assert {k: default[k] for k in keys} == {k: historical[k] for k in keys}
    mode, owner = row['mode'], row['lane']
    assert data['mode'] == mode and data['scenario'] == 'cognition-inputs-' + owner + '-v1'
    extra = 2000 if mode == 'populated' else 0
    assert data['placement'] == {'requested': extra, 'placed': extra, 'unplaced': 0}
    validate_cognition_inputs(data, extra)
    assert data['owner_counts'] == default['counts']
    assert all(data['witnesses'][k] == v for k, v in default['witnesses'].items())
    c = data['cost']
    assert c['peak_bytes'] == 4096 + 4*c['expanded_upper_bytes'] + 3*c['encoded_bytes'] + 4194304
    assert 0 < c['encoded_bytes'] <= (128 if extra else 64)*1024**2
    assert 0 < c['expanded_upper_bytes'] <= 128*1024**2
    assert data['shared_reserved_peak_excluding_running_bytes'] == 2*c['peak_bytes'] < 1024**3
    rows.append({'owner':owner,'mode':mode,'historical_default_metadata_equal':True,
                 'default_old_cost_and_lease_equal':True,'primary_sha256':sha(json.dumps(primary(data),sort_keys=True,separators=(',',':')).encode()),
                 'raw_inputs_sha256':row['inputs_sha256'],'cost':c,'counts':data['counts'],
                 'poll_count':data['witnesses']['poll_count'],
                 'maximum_poll_step_seconds':data['witnesses']['maximum_poll_step_seconds'],
                 'zero_discard':data['witnesses']['coarse_discard_diagnostics']==0})
report = {'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'auditor_sha256':sha((here/'audit_m2_component_probes.py').read_bytes()),
          'source_manifest_sha256':sha((e/'source_hashes.json').read_bytes()),'rows':rows,
          'scope':'Existing debug runs inspected without rerunning; timings excluded from release acceptance.'}
p = e/'coordinator/debug_workload_audit.json'
assert not p.exists()
p.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'result':'passed','old_default_and_new_input_workloads':len(rows),'population_modes':['authored','populated']}))
