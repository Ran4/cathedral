from pathlib import Path
import subprocess, json, hashlib, gzip, shutil, tempfile, datetime

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
audit = here / 'audit_m2_component_probes.py'
source = here / 'm2a11/performance'
binary = root / 'target/release/examples/alibi_scheduler_cost'
sha = lambda b: hashlib.sha256(b).hexdigest()
checks = []
cases = {
    'percentile':'assert quantiles(data[phase]) == row["phase_us"][phase]',
    'working_charge':'assert validation_working_bytes == 4096 * 1024',
    'lost_retry':'assert counts == {',
    'coarse_step':'assert 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12',
    'changed_provider_budget':'assert sha(json.dumps(witnesses, sort_keys=True,',
    'missing_runtime_defaults':'assert component_sources() == identity["source_sha256"]',
}
for name, expected_gate in cases.items():
    folder = Path(tempfile.mkdtemp(prefix='alibi-m2a11-auditor-' + name + '-')) / 'data'
    shutil.copytree(source, folder)
    rows = json.loads((folder / 'RESULTS.json').read_bytes())
    row = rows[0]
    if name == 'percentile':
        row['phase_us']['export_us']['p99'] += 1
    elif name == 'missing_runtime_defaults':
        identity = json.loads((folder / 'IDENTITY.json').read_bytes())
        del identity['source_sha256']['default_config.ron']
        (folder / 'IDENTITY.json').write_text(json.dumps(identity, indent=2) + '\n')
    else:
        file = folder / (row['name'] + '.json.gz')
        data = json.loads(gzip.decompress(file.read_bytes()))
        if name == 'working_charge':
            data['cost']['validation_working_bytes'] -= 1024
            data['cost']['peak_bytes'] -= 1024
            data['shared_reserved_peak_excluding_running_bytes'] -= 2048
        elif name == 'lost_retry':
            data['counts']['retry_work'] = 0
        elif name == 'coarse_step':
            data['witnesses']['maximum_poll_step_seconds'] = 0.35
        else:
            data['witnesses']['submitted_prompts'][0][1] += 1
        raw = (json.dumps(data, indent=2) + '\n').encode()
        file.write_bytes(gzip.compress(raw, mtime=0))
        row['artifacts']['uncompressed_json_sha256'] = sha(raw)
        row['artifacts'][file.name] = sha(file.read_bytes())
        phases = {'preflight_us','export_us','encode_us','decode_validate_us','candidate_validate_us','drop_us'}
        row['metadata'] = {key:{'raw_samples':key} if key in phases else value for key,value in data.items()}
    (folder / 'RESULTS.json').write_text(json.dumps(rows, indent=2) + '\n')
    report = folder.parent / 'unexpected-success.json'
    command = ['/home/ran/.local/bin/uv','run','--no-project','--cache-dir','/tmp/alibi-uv',str(audit),str(folder),'--report',str(report)]
    if name == 'missing_runtime_defaults': command += ['--check-current',str(binary)]
    result = subprocess.run(command,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    output = result.stdout.decode()
    assert result.returncode != 0 and not report.exists(), name
    assert expected_gate in output, output
    checks.append({'case':name,'command':command,'exit_code':result.returncode,
        'expected_failing_gate':expected_gate,'success_report_absent':True,'output':output})
value = {'checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'auditor_sha256':sha(audit.read_bytes()),'measurement_summary_sha256':sha((source/'SUMMARY.json').read_bytes()),
    'result':'passed','checks':checks}
path = here / 'm2a11/coordinator/auditor_regressions.json'
assert not path.exists()
path.write_text(json.dumps(value,indent=2)+'\n')
print(json.dumps({'result':'passed','negative_cases':len(checks)},indent=2))
