"""Preserve the exact tested executable and published slot without rewriting identity."""
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
helper_sha256 = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
assert helper_sha256 == os.environ['ALIBI_M3A_HELPER_SHA256']
mode = sys.argv[1]
assert mode in ('pre-fix', 'final')
report_path = Path('/tmp/alibi-m3a-release-writer-final-report.json' if mode == 'pre-fix'
                   else '/tmp/alibi-m3a-release-writer-final-02-report.json')
report = json.loads(report_path.read_text())
image = Path(report['executable'])
preserved = Path('/tmp/alibi-m3a-pre-fix-release-image' if mode == 'pre-fix'
                 else '/tmp/alibi-m3a-final-release-image')

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        while block := f.read(1024 * 1024):
            h.update(block)
    return h.hexdigest()

expected = bytes(report['host_image_sha256']).hex()
assert digest(image) == expected
with image.open('rb') as source, preserved.open('xb') as target:
    shutil.copyfileobj(source, target, 1024 * 1024)
preserved.chmod(0o755)
assert digest(preserved) == expected
destination = HERE / ('fixture-release-pre-fix' if mode == 'pre-fix' else 'fixture-release')
destination.mkdir()
files = {}
for path in sorted(Path(report['directory']).iterdir()):
    assert path.is_file() and not path.is_symlink()
    copied = destination / path.name
    with path.open('rb') as source, copied.open('xb') as target:
        shutil.copyfileobj(source, target)
    files[path.name] = {'original': str(path), 'bytes': path.stat().st_size,
                        'sha256': digest(path)}
    assert digest(copied) == files[path.name]['sha256']
shutil.copyfile(report_path, HERE / f'release-writer-{mode}-report.json')
record = {'mode': mode, 'original_image': str(image), 'preserved_image': str(preserved),
          'helper_sha256': helper_sha256,
          'image_sha256': expected, 'image_bytes': image.stat().st_size,
          'original_report': str(report_path), 'report_sha256': digest(report_path),
          'fixture_files': files}
(HERE / ('release-retention-pre-fix.json' if mode == 'pre-fix' else 'release-retention.json')).write_text(json.dumps(record, sort_keys=True, indent=2) + '\n')
print(json.dumps(record, sort_keys=True))
