"""Read-only platform identity for the supported durable filesystem evidence."""
import json
import hashlib
import os
import subprocess
from pathlib import Path

helper_sha256 = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
assert helper_sha256 == os.environ['ALIBI_M3A_HELPER_SHA256']
print(json.dumps({'helper_sha256': helper_sha256}))

commands = [
    ['/usr/bin/uname', '-a'],
    ['/usr/bin/findmnt', '-T', '/tmp', '-n', '-o', 'FSTYPE,TARGET,OPTIONS'],
    ['/home/ran/.cargo/bin/rustc', '-Vv'],
    ['/usr/bin/git', 'rev-parse', 'HEAD'],
]
for command in commands:
    p = subprocess.run(command, capture_output=True)
    assert p.returncode == 0
    print(json.dumps({'argv': command, 'stdout': p.stdout.decode('utf-8'),
                      'stderr': p.stderr.decode('utf-8'), 'exit_code': p.returncode}))
