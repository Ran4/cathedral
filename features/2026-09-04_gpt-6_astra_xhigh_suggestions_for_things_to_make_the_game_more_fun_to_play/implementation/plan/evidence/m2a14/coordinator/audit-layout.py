from pathlib import Path
import hashlib, json, re, datetime, sys, subprocess

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0, str(here))
from component_input_sources import sources
e = here / 'm2a14'
sha = lambda b: hashlib.sha256(b).hexdigest()
frozen = json.loads((e / 'source_hashes.json').read_bytes())
assert sources() == frozen
records = json.loads((e / 'commands.json').read_bytes())
record = next(r for r in records if r['name'] == 'checkpoint_final')
raw = Path(record['original_path']).read_bytes()
assert record['exit_code'] == 0 and sha(raw) == record['original_sha256']
assert json.loads((root / record['source_manifest']).read_bytes()) == frozen
def values(prefix):
    rows = re.findall(re.escape(prefix) + r'((?:[a-z_]+=\d+ ?)+)', raw.decode())
    assert len(rows) == 1, (prefix, rows)
    return {k: int(v) for k, v in re.findall(r'([a-z_]+)=(\d+)', rows[0])}
new = values('cognition_inputs_layout ')
night = values('cognition_inputs_night_layout ')
# The Engine layout prints another prefix; select the full owner record exactly.
rows = re.findall(r'scheduler_layout (owner=\d+(?: [a-z_]+=\d+)+)', raw.decode())
assert len(rows) == 1
scheduler = {k: int(v) for k, v in re.findall(r'([a-z_]+)=(\d+)', rows[0])}
assert new == {'dto':248,'candidate':248,'scheduler_row':96,'night_row':112,'context':48,'budget':8,'working':4194304}
assert night == {'flight':104,'owner':312,'accepted_budget':8}
assert scheduler['flight'] == 144 and scheduler['owner'] == 384
assert new['dto'] == new['candidate'] < 512 + 5 * 64
assert max(new['scheduler_row'], new['night_row']) < 512 + 8 * 64
assert night['flight'] < 512 + 6 * 64
assert scheduler['flight'] < 512 + 8 * 64
assert max(night['owner'], scheduler['owner']) < 512 + 13 * 64
node = lambda key, value: 16 + 12 * 8 + 11 * (key + value)
ledger = 256 * node(16, 0) + 4352 * (node(24, 0) + node(8, 0))
assert ledger == scheduler['ledger_scratch'] == 2580480
assert scheduler['lane_scratch'] == 2 * 25000 * 8
assert scheduler['root_scratch'] == 257 * 16
night_scratch = 2 * 25008 * 8 + 2 * (25008 + 1) * 16
assert max(ledger, scheduler['lane_scratch'] + scheduler['root_scratch'], night_scratch) + 65536 < new['working']
proof_file = here / 'm2a13/development/stdlib_allocation_audit.json'
proof = json.loads(proof_file.read_bytes())
prior_commands = json.loads((here / 'm2a13/commands.json').read_bytes())
compiler = {}
for key, args in [('compiler_sysroot_command', ['--print', 'sysroot']), ('compiler_version_command', ['-Vv'])]:
    prior = next(r for r in prior_commands if r['name'] == proof[key])
    original = Path(prior['original_path']).read_bytes()
    assert sha(original) == prior['original_sha256']
    current = subprocess.check_output(['/home/ran/.cargo/bin/rustc', *args])
    assert current == original
    compiler[key] = current.decode()
sysroot = Path(compiler['compiler_sysroot_command'].strip()).resolve()
assert sysroot.is_relative_to('/home/ran/.rustup/toolchains')
registry = Path('/tmp/alibi-m1b-cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
parser_roots = [(registry / name).resolve() for name in ('serde_json-1.0.150', 'serde_core-1.0.228')]
reused = []
for name, expected in proof['source_files'].items():
    path = Path(name).resolve()
    assert path.is_relative_to(sysroot) or any(path.is_relative_to(p) for p in parser_roots)
    content = path.read_bytes()
    assert sha(content) == expected['sha256'] and len(content) == expected['bytes']
    lines = content.decode().splitlines(keepends=True)
    for excerpt in expected['excerpts']:
        assert ''.join(lines[excerpt['start_line']-1:excerpt['end_line']]) == excerpt['text']
    reused.append({'path':name,'sha256':sha(content),'verified_excerpts':len(expected['excerpts'])})
report = {
    'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source_manifest_sha256':sha((e/'source_hashes.json').read_bytes()),
    'layout_command':record['name'],'original_log_sha256':sha(raw),
    'new_component':new,'old_night':night,'old_scheduler':scheduler,
    'ledger_scratch_bytes':ledger,'night_loose_scratch_bytes':night_scratch,
    'scratch_lifetimes':'Saved ledger indexes drop before owner indexes; scheduler validation returns before Night validation. The new component allocates no index.',
    'error_and_fixed_slack_bytes':65536,'working_bytes':new['working'],
    'reused_library_proof':str(proof_file.relative_to(root)),
    'reused_library_proof_sha256':sha(proof_file.read_bytes()),'verified_library_sources':reused,
    'compiler_matches_prior_proof':compiler,
    'scope':'Concrete record and conservative admitted-allocation bounds. Not RSS, a complete-envelope lifetime bound, or a host frame claim.',
}
path = e/'coordinator/layout_audit.json'
assert not path.exists()
path.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'result':'passed','layout':new,'ledger_scratch_bytes':ledger,'night_loose_scratch_bytes':night_scratch,'verified_library_sources':len(reused)}))
