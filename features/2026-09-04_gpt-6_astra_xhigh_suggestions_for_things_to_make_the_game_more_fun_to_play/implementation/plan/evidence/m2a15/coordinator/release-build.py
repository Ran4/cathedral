"""Build once after owner cedes Cargo; preserve the exact release test binary."""
import json
from pathlib import Path
import shutil
import sys
from release_common import ROOT, OUT, capture, sha, write_json

attempt = int(sys.argv[1]) if len(sys.argv) == 2 else 1
assert attempt > 0 and len(sys.argv) <= 2
summary = OUT / 'release_build.json'
reference = Path(f'/tmp/alibi-m2a15-host-reference-binary-{attempt}')
assert not reference.exists()
command = [
    '/home/ran/.cargo/bin/cargo', 'test', '--offline', '-j1', '--release',
    '--bin', 'cathedralbevy', '--no-run', '--message-format=json',
]
record = capture(f'release-build-{attempt}', command, OUT)
executables = set()
with open(record['log']['original']) as stream:
    for line in stream:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (message.get('reason') == 'compiler-artifact'
                and message.get('target', {}).get('name') == 'cathedralbevy'
                and message.get('profile', {}).get('test')
                and message.get('executable')):
            executables.add(message['executable'])
assert len(executables) == 1, executables
binary = Path(executables.pop())
assert binary.is_relative_to(ROOT / 'target/release') and binary.is_file()
shutil.copy2(binary, reference)
assert sha(reference) == sha(binary)
record |= {
    'built_binary': str(binary), 'reference_binary': str(reference),
    'reference_binary_sha256': sha(reference), 'binary_bytes': reference.stat().st_size,
}
write_json(OUT / f'release-build-{attempt}.binary.json', record)
write_json(summary, record)
