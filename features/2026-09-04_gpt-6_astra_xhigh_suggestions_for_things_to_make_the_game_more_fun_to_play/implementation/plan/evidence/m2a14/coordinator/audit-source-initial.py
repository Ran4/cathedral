from pathlib import Path
import subprocess, json, hashlib, datetime, sys

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
sys.path.insert(0, str(here))
from component_input_sources import SOURCE_SCOPE, sources
e = here / 'm2a14'
out = e / 'coordinator'
sha = lambda b: hashlib.sha256(b).hexdigest()
git = lambda *args: subprocess.check_output(['/usr/bin/git', *args], cwd=root)
manifest = json.loads((e / 'source_hashes.json').read_bytes())
assert sources() == manifest
assert git('rev-parse', 'HEAD').decode().strip() == '3aed5c264462e6e8c84c0d0e880e82057f47741d'
changed = set(git('diff', '--name-only', 'HEAD', '--', 'crates', 'Cargo.toml', 'Cargo.lock').decode().splitlines())
changed.update(git('ls-files', '--others', '--exclude-standard', '--', 'crates').decode().splitlines())
assert changed <= manifest.keys(), sorted(changed - manifest.keys())
allowed_files = {
    'crates/cathedral-sim/src/engine.rs',
    'crates/cathedral-sim/src/traits.rs',
    'crates/cathedral-sim/src/scheduler.rs',
    'crates/cathedral-sim/src/night.rs',
    'crates/cathedral-sim/src/night/tests.rs',
    'crates/cathedral-sim/src/engine/night_checkpoint.rs',
    'crates/cathedral-sim/src/engine/scheduler_checkpoint.rs',
    'crates/cathedral-sim/src/engine/cognition_inputs_checkpoint.rs',
    'crates/cathedral-sim/src/scheduler/checkpoint.rs',
    'crates/cathedral-sim/src/scheduler/checkpoint/records.rs',
    'crates/cathedral-sim/src/scheduler/checkpoint/tests.rs',
    'crates/cathedral-sim/src/scheduler/checkpoint/tests/cognition_inputs.rs',
    'crates/cathedral-sim/src/night/checkpoint.rs',
    'crates/cathedral-sim/src/night/checkpoint/records.rs',
    'crates/cathedral-sim/src/night/checkpoint/tests.rs',
    'crates/cathedral-sim/src/night/checkpoint/tests/cognition_inputs.rs',
    'crates/cathedral-sim/tests/checkpoint_cognition_inputs_boundary.rs',
    'crates/cathedral-backends/examples/alibi_scheduler_cost.rs',
    'crates/cathedral-backends/examples/alibi_night_cost.rs',
    'crates/cathedral-backends/examples/support/cognition_inputs_cost.rs',
}
allowed_prefixes = (
    'crates/cathedral-sim/src/engine/cognition_inputs_checkpoint/',
    'crates/cathedral-sim/tests/fixtures/checkpoint_cognition_inputs/',
)
assert all(p in allowed_files or p.startswith(allowed_prefixes) for p in changed), sorted(changed)

field = '    output_token_budget: crate::traits::AcceptedOutputBudget,\n'
accepted = '''                    output_token_budget: crate::traits::AcceptedOutputBudget::Accepted(
                        output_token_budget,
                    ),
'''
runtime_changes = {
    'crates/cathedral-sim/src/engine.rs': [('pub mod cognition_inputs_checkpoint;\n', '')],
    'crates/cathedral-sim/src/traits.rs': [('''/// Exact accepted argument, distinct from a historical component that never
/// retained it. This belongs to a flight, never an unaccepted Busy attempt.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcceptedOutputBudget {
    #[default]
    MissingLegacy,
    Accepted(Option<u32>),
}

''', '')],
    'crates/cathedral-sim/src/scheduler.rs': [
        (field, ''), (accepted, ''),
        ('            output_token_budget: crate::traits::AcceptedOutputBudget::Accepted(None),\n', ''),
    ],
    'crates/cathedral-sim/src/night.rs': [
        (field, ''), (accepted, ''),
        ('        let output_token_budget = due.subject.output_token_budget(world);\n', ''),
        ('        match cognition.request_night(prompt.clone(), output_token_budget) {\n',
         '        match cognition.request_night(prompt.clone(), due.subject.output_token_budget(world)) {\n'),
    ],
}
comparisons = []
for name, changes in runtime_changes.items():
    before = git('show', 'HEAD:' + name)
    after = (root / name).read_bytes()
    normalized = after.decode()
    for addition, replacement in changes:
        assert normalized.count(addition) == 1, (name, addition)
        normalized = normalized.replace(addition, replacement, 1)
    assert normalized.encode() == before, name
    comparisons.append({'path': name, 'baseline_sha256': sha(before), 'current_sha256': sha(after),
                        'reviewed_exact_replacements': changes, 'remaining_bytes_identical': True})
fixtures = {}
for name in git('ls-tree', '-r', '--name-only', 'HEAD', '--', 'crates/cathedral-sim/tests/fixtures').decode().splitlines():
    baseline = git('show', 'HEAD:' + name)
    assert baseline == (root / name).read_bytes(), name
    fixtures[name] = sha(baseline)
checks = []
for name in sorted(changed):
    if not name.endswith('.rs'):
        continue
    cmd = ['/home/ran/.cargo/bin/rustfmt', '--edition', '2024', '--config', 'skip_children=true', '--check', name]
    result = subprocess.run(cmd, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert result.returncode == 0, result.stdout.decode()
    checks.append({'path': name, 'command': cmd, 'source_sha256': sha((root / name).read_bytes()),
                   'exit_code': 0, 'output': result.stdout.decode()})
stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
report = {'checked_at_utc': stamp, 'result': 'passed', 'source_scope': SOURCE_SCOPE,
          'source_manifest_sha256': sha((e / 'source_hashes.json').read_bytes()),
          'source_files_checked': len(manifest), 'changed_paths_all_covered': True,
          'changed_paths': sorted(changed), 'historical_fixtures_and_docs_unchanged': fixtures,
          'ordinary_behavior_comparisons': comparisons}
for name, value in [('source_audit.json', report), ('format_audit.json', {'checked_at_utc': stamp, 'result': 'passed', 'checks': checks})]:
    assert not (out / name).exists()
    (out / name).write_text(json.dumps(value, indent=2) + '\n')
print(json.dumps({'result': 'passed', 'source_files': len(manifest), 'changed_paths': len(changed),
                  'scoped_format_checks': len(checks), 'historical_fixture_files': len(fixtures)}))
