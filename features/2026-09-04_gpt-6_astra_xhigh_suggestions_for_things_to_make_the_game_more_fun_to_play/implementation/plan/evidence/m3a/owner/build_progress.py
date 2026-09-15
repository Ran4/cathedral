"""Bounded read-only build/resource observation; not functional verification."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

helper = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
assert helper == os.environ['ALIBI_M3A_HELPER_SHA256']
print(json.dumps({'scope': 'read-only build/resource observation', 'helper_sha256': helper}))
names = {'cargo', 'rustc', 'rust-lld', 'ld.lld', 'ld', 'cc', 'gcc', 'clang', 'collect2', 'mold'}
for sample in range(2):
    result = subprocess.run(['/usr/bin/ps', '-eo', 'pid,ppid,stat,etime,time,pcpu,pmem,comm'],
                            capture_output=True, check=True, timeout=5)
    rows = result.stdout.decode('utf-8').splitlines()
    selected = [row for row in rows[1:] if row.split() and row.split()[-1] in names]
    print(json.dumps({'sample': sample, 'unix_seconds': time.time(),
                      'ps_header': rows[0], 'processes': selected,
                      'stderr': result.stderr.decode('utf-8')}))
    if not sample:
        time.sleep(2)
memory = [line for line in Path('/proc/meminfo').read_text().splitlines()
          if line.split(':')[0] in {'MemTotal', 'MemFree', 'MemAvailable', 'SwapTotal', 'SwapFree'}]
print(json.dumps({'meminfo': memory}))
result = subprocess.run(['/usr/bin/df', '-B1', '-T', 'target'], capture_output=True, timeout=5)
print(json.dumps({'argv': ['/usr/bin/df', '-B1', '-T', 'target'],
                  'stdout': result.stdout.decode('utf-8'), 'stderr': result.stderr.decode('utf-8'),
                  'exit_code': result.returncode}))
assert result.returncode == 0
