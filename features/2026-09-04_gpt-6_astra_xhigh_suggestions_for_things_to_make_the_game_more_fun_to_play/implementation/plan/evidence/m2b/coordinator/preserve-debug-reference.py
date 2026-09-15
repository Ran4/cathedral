"""Preserve the final tested debug image after the owner cedes Cargo."""
from pathlib import Path
import datetime
import shutil
import sys
from release_common import LEG, OUT, ROOT, frozen_sources, sha, write_json

assert len(sys.argv) in (2, 3)
attempt = int(sys.argv[2]) if len(sys.argv) == 3 else 1
assert attempt > 0
binary = Path(sys.argv[1]).resolve()
assert binary.is_relative_to(ROOT / 'target/debug') and binary.is_file()
destination = Path(f'/tmp/alibi-m2b-debug-reference-binary-{attempt}')
record_path = OUT / f'debug-reference-{attempt}.json'
assert not destination.exists() and not record_path.exists()
frozen_sources()
digest = sha(binary)
shutil.copy2(binary, destination)
assert sha(destination) == digest == sha(binary)
frozen_sources()
record = {
    'copied_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'original': str(binary), 'binary': str(destination), 'sha256': digest,
    'bytes': destination.stat().st_size,
    'source_manifest_sha256': sha(LEG / 'source_hashes.json'),
    'helper_sha256': sha(Path(__file__)),
}
write_json(record_path, record)
write_json(OUT / 'debug-reference.json', record)
print(record)
