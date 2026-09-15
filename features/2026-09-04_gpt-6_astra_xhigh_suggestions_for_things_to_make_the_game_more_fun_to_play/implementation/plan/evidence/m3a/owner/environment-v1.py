"""Read-only platform identity for the supported durable filesystem evidence."""
import json
import subprocess

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
