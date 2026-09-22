"""Archive read-only platform identities/disassembly, not upstream source copies."""
from pathlib import Path
import hashlib
import json
import subprocess

ROOT = Path('/home/ran/src/rust/cathedralbevy')
OUT = ROOT / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m3_native_directory/audit'
UPSTREAM = Path('/tmp/alibi-native-directory-audit')
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
sources = {}
for name in ('opendir.c', 'dirstream.h', 'readdir64.c', 'malloc.c'):
    relative = 'malloc/malloc.c' if name == 'malloc.c' else 'sysdeps/unix/sysv/linux/' + name
    sources[name] = {'sha256': sha(UPSTREAM / name),
                     'url': 'https://raw.githubusercontent.com/bminor/glibc/glibc-2.35/' + relative}
libc = Path('/lib/x86_64-linux-gnu/libc.so.6')
commands = {}
for function in ('opendir', 'readdir', 'closedir', 'getdents64'):
    command = ['/usr/bin/objdump', '--disassemble=' + function, str(libc)]
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=True)
    path = OUT / (function + '.disassembly.txt')
    path.write_bytes(result.stdout)
    commands[function] = {'command': command, 'exit_code': result.returncode, 'sha256': sha(path)}
command = ['/usr/bin/objdump', '-T', str(libc)]
result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=True)
symbols = '\n'.join(line for line in result.stdout.splitlines() if line.split() and line.split()[-1] in ('opendir', 'readdir', 'readdir64', 'closedir')) + '\n'
(OUT / 'directory-symbols.txt').write_text(symbols)
rust = Path('/home/ran/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/std/src/sys/fs/unix.rs')
record = {'upstream': sources, 'libc_path': str(libc), 'libc_sha256': sha(libc),
          'libc_version': 'Ubuntu GLIBC 2.35-0ubuntu3.15',
          'rust_std_path': str(rust), 'rust_std_sha256': sha(rust),
          'disassembly': commands, 'symbols_command': command, 'symbols_sha256': sha(OUT / 'directory-symbols.txt'),
          'scope': 'x86_64 Linux/GNU; ordinary ptmalloc, glibc.malloc.hugetlb=0; 4096-byte pages',
          'maximum_request_bytes': 1048624, 'reserved_native_bytes': 1052736,
          'proof': '1 MiB max buffer + 48-byte DIR + 16-byte mmap overhead + 4096-byte conservative page remainder',
          'maximum_is_measured': False}
(OUT / 'identity.json').write_text(json.dumps(record, sort_keys=True, indent=2) + '\n')
print(json.dumps({'libc_sha256': sha(libc), 'native_allowance':1052736, 'upstream_count':len(sources)}))
