"""Check only changed Rust files, without modifying frozen production inputs."""
from pathlib import Path
import json
import subprocess
from release_common import ROOT, LEG, OUT, capture, frozen_sources, sha, write_json

frozen = frozen_sources()
changed = subprocess.check_output(
    ['/usr/bin/git', 'diff', '--name-only', '-z'], cwd=ROOT).decode().split('\0')
new = subprocess.check_output(
    ['/usr/bin/git', 'ls-files', '--others', '--exclude-standard', '-z', '--', 'src', 'crates'],
    cwd=ROOT).decode().split('\0')
rust = sorted(p for p in set(changed + new) if p.endswith('.rs'))
assert rust and all(p in frozen for p in rust)
command = ['/home/ran/.cargo/bin/rustfmt', '--check', '--edition', '2024',
           '--config', 'skip_children=true', *rust]
record = capture('final-format', command, OUT)
diff_record = capture('final-diff-check', ['/usr/bin/git', 'diff', '--check'], OUT)
assert frozen_sources() == frozen
destination = OUT / 'format-source-audit.json'
assert not destination.exists()
report = {
    'result': 'passed', 'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'formatted_files': rust, 'format_result_sha256': sha(OUT / 'final-format.result.json'),
    'diff_check_result_sha256': sha(OUT / 'final-diff-check.result.json'),
    'helper_sha256': sha(Path(__file__)),
}
write_json(destination, report)
print(json.dumps({'result': 'passed', 'formatted_files': len(rust)}))
