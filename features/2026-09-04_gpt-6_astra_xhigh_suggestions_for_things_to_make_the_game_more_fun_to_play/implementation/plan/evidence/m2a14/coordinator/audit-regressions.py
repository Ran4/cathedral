from pathlib import Path
import subprocess, json, hashlib, gzip, shutil, tempfile, datetime

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
audit = here / 'audit_m2_component_probes.py'
sha = lambda b: hashlib.sha256(b).hexdigest()
checks = []
cases = {
    'percentile': ('scheduler', 'assert quantiles(data[phase]) == row["phase_us"][phase]'),
    'working_charge': ('scheduler', 'assert data["cost"]["validation_working_bytes"] == 4 * 1024**2'),
    'lost_row': ('scheduler', 'assert counts == {'),
    'coarse_step': ('night', 'assert 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12'),
    'changed_saved_budget': ('scheduler', 'assert {k: row[k] for k in match[0]} == match[0]'),
    'coherent_rewritten_request': ('scheduler', 'assert sha(json.dumps(primary, sort_keys=True,'),
    'lost_committed_mood': ('night', 'assert owner["ward_moods"] == len(witnesses["committed_ward_moods"]) == 1'),
    'missing_runtime_defaults': ('scheduler', 'assert component_sources() == identity["source_sha256"]'),
}
for name, (owner, gate) in cases.items():
    source = here / 'm2a14/performance' / owner
    folder = Path(tempfile.mkdtemp(prefix='alibi-m2a14-auditor-' + name + '-')) / 'data'
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
            data['cost']['validation_working_bytes'] += 1024
            data['cost']['peak_bytes'] += 1024
            data['shared_reserved_peak_excluding_running_bytes'] += 2048
        elif name == 'lost_row':
            data['saved_inputs']['scheduler'] = None
        elif name == 'coarse_step':
            data['witnesses']['maximum_poll_step_seconds'] = .35
        elif name == 'changed_saved_budget':
            data['saved_inputs']['scheduler']['output_token_budget'] += 1
        elif name == 'coherent_rewritten_request':
            data['saved_inputs']['scheduler']['output_token_budget'] += 1
            data['submitted_requests'][-1]['output_token_budget'] += 1
            data['witnesses']['submitted_prompts'][-1][1] += 1
        elif name == 'lost_committed_mood':
            data['witnesses']['committed_ward_moods'].clear()
        raw = (json.dumps(data, indent=2) + '\n').encode()
        file.write_bytes(gzip.compress(raw, mtime=0))
        row['artifacts']['uncompressed_json_sha256'] = sha(raw)
        row['artifacts'][file.name] = sha(file.read_bytes())
        phases = {'preflight_us', 'export_us', 'encode_us', 'decode_validate_us', 'candidate_validate_us', 'drop_us'}
        row['metadata'] = {k: {'raw_samples': k} if k in phases else v for k, v in data.items()}
    (folder / 'RESULTS.json').write_text(json.dumps(rows, indent=2) + '\n')
    report = folder.parent / 'unexpected-success.json'
    command = ['/home/ran/.local/bin/uv', 'run', '--no-project', '--cache-dir', '/tmp/alibi-uv', str(audit), str(folder), '--report', str(report)]
    if name == 'missing_runtime_defaults':
        command += ['--check-current', str(root / 'target/release/examples' / f'alibi_{owner}_cost')]
    result = subprocess.run(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    output = result.stdout.decode()
    assert result.returncode != 0 and not report.exists(), name
    assert gate in output, (name, output)
    checks.append({'case': name, 'owner': owner, 'command': command, 'exit_code': result.returncode,
                   'expected_failing_gate': gate, 'success_report_absent': True, 'output': output})
value = {'checked_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
         'auditor_sha256': sha(audit.read_bytes()), 'result': 'passed', 'checks': checks}
path = here / 'm2a14/coordinator/auditor_regressions.json'
assert not path.exists()
path.write_text(json.dumps(value, indent=2) + '\n')
print(json.dumps({'result': 'passed', 'negative_cases': len(checks)}))
