"""Check the exact post-fixture source and staged milestone scope before commit."""
import ast
import json
import re
import subprocess
from pathlib import Path
from release_common import LEG, OUT, ROOT, archive, sha, sources, write_json

final = json.loads((LEG / 'source_hashes_with_fixtures.json').read_bytes())
before = json.loads((LEG / 'source_hashes.json').read_bytes())
assert sources() == final
publication = json.loads((OUT / 'fixture-publication.json').read_bytes())
assert publication['before_source_manifest_sha256'] == sha(LEG / 'source_hashes.json')
assert publication['after_source_manifest_sha256'] == sha(LEG / 'source_hashes_with_fixtures.json')
assert all(final.get(path) == digest for path, digest in before.items())
assert sorted(set(final) - set(before)) == publication['added_paths']
assert len(final) == 970 and len(before) == 967
for row in publication['fixtures']:
    assert sha(ROOT / row['repository']) == row['sha256']
    assert (ROOT / row['repository']).read_bytes() == Path(row['original']).read_bytes()
for path in sorted(OUT.glob('*.py')):
    ast.parse(path.read_text(), filename=str(path))
for path in (LEG / 'README.md', OUT / 'review.md', LEG / 'performance/README.md'):
    for target in re.findall(r'\]\(([^)]+)\)', path.read_text()):
        if '://' not in target and not target.startswith('#'):
            assert (path.parent / target.split('#', 1)[0]).exists(), (path, target)

assert subprocess.check_output(['/usr/bin/git', 'branch', '--show-current'], cwd=ROOT).strip() == b'develop'
staged = subprocess.check_output(['/usr/bin/git', 'diff', '--cached', '--name-only', '-z'], cwd=ROOT)
paths = [part.decode() for part in staged.split(b'\0') if part]
assert paths
plan = LEG.parent.parent
feature = plan.parent.parent
allowed_documents = {str(path.relative_to(ROOT)) for path in (
    feature / 'README.md', plan / 'README.md', plan / 'M2_simulation_checkpoints.md',
    plan / 'CHECKPOINT_PROTOCOL.md')}
allowed_source = {
    path for path, digest in final.items()
    if path not in before or path.endswith('.rs')
}
for path in paths:
    assert path in allowed_documents or path in allowed_source or path.startswith(str(LEG.relative_to(ROOT)) + '/')
assert not any(path.endswith(('.pyc', '.pyo')) or '__pycache__' in path for path in paths)
diff = subprocess.run(['/usr/bin/git', 'diff', '--cached', '--check'], cwd=ROOT,
                      stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
original = Path('/tmp/alibi-m2b-final-staged-diff-check.log')
assert not original.exists()
original.write_bytes(diff.stdout)
archived_diff = archive(original, OUT / 'final-staged-diff-check.log.gz')
assert diff.returncode == 0, diff.stdout.decode()
write_json(OUT / 'commit-source-audit.json', {
    'result': 'passed', 'helper_sha256': sha(Path(__file__)),
    'source_count': len(final),
    'source_manifest_sha256': sha(LEG / 'source_hashes_with_fixtures.json'),
    'prior_source_inputs_unchanged': True, 'publication_delta': publication['added_paths'],
    'staged_paths_before_this_report': paths, 'staged_diff_check_exit_code': diff.returncode,
    'staged_diff_check_log': str(original), 'staged_diff_check_log_sha256': sha(original),
    'staged_diff_check_archive': archived_diff,
})
print(json.dumps({'result': 'passed', 'source_count': len(final), 'staged_paths': len(paths)}))
