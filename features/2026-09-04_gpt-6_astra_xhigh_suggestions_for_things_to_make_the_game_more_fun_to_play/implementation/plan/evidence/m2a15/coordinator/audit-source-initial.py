"""Final-source, scoped-format and original owner-check audit."""
from pathlib import Path
import datetime
import gzip
import json
import re
import subprocess
from release_common import ROOT, LEG, OUT, archive, frozen_sources, sha, write_json

frozen = frozen_sources()
start_path = LEG / 'commands/workspace-final-2.start.json'
start = json.loads(start_path.read_bytes())
result_path = LEG / 'commands/workspace-final-2.result.json'
result = json.loads(result_path.read_bytes())
assert start['source_sha256'] == frozen
assert result['exit_code'] == 0 and result['source_changed_during_command'] is False
raw = Path(start['raw_log']).read_bytes()
assert sha(Path(start['raw_log'])) == result['raw_sha256']
assert gzip.decompress((LEG / 'commands/workspace-final-2.log.gz').read_bytes()) == raw
groups = [tuple(map(int, m)) for m in re.findall(
    rb'^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw, re.M)]
totals = [sum(g[i] for g in groups) for i in range(3)]
assert len(groups) == 46 and totals == [2156, 0, 37], (len(groups), totals)
assert len(re.findall(rb'^test host_checkpoint::tests_public::\S+ \.\.\. ok$', raw, re.M)) == 11
assert b'test checkpoint::host::wire::tests::host_wire_mixed_units_and_nullable_are_closed ... ok' in raw

changed = subprocess.check_output(['/usr/bin/git', 'diff', '--name-only', '-z'], cwd=ROOT).decode().split('\0')
new = subprocess.check_output(['/usr/bin/git', 'ls-files', '--others', '--exclude-standard', '-z', '--', 'src', 'crates'], cwd=ROOT).decode().split('\0')
rust = sorted(p for p in set(changed + new) if p.endswith('.rs'))
assert rust and all(p in frozen for p in rust)
command = ['/home/ran/.cargo/bin/rustfmt', '--check', '--edition', '2024', '--config', 'skip_children=true', *rust]
original = Path('/tmp/alibi-m2a15-final-format.log')
assert not original.exists()
with original.open('wb') as stream:
    completed = subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
format_log = archive(original, OUT / 'final-format.log.gz')
assert completed.returncode == 0, 'format failure preserved in original log'
assert frozen_sources() == frozen
subprocess.run(['/usr/bin/git', 'diff', '--check'], cwd=ROOT, check=True)
report = {
    'result': 'passed', 'checked_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source_scope': start['source_scope'], 'source_files': len(frozen),
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'workspace_start_sha256': sha(start_path), 'workspace_result_sha256': sha(result_path),
    'workspace_original_sha256': result['raw_sha256'], 'groups': len(groups),
    'passed': totals[0], 'failed': totals[1], 'ignored': totals[2], 'public_passes': 11,
    'closed_wire_and_inline_stride_test_passed': True,
    'format_command': command, 'format_checked_files': rust, 'format_log': format_log,
    'helper_sha256': sha(Path(__file__)),
}
destination = OUT / 'source-audit.json'
assert not destination.exists()
write_json(destination, report)
print(json.dumps({'result': 'passed', 'source_manifest_sha256': report['source_manifest_sha256'],
                  'source_files': len(frozen), 'formatted_files': len(rust), 'passed': totals[0]}))
