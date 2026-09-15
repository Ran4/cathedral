"""Source-backed bound for MiniJinja allocations reused by a fresh PromptEnv."""
from pathlib import Path
import datetime
import hashlib
import json
import re
import subprocess
import tomllib

ROOT = Path('/home/ran/src/rust/cathedralbevy')
OUT = Path(__file__).resolve().parent
REGISTRY = Path('/home/ran/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
package = next(p for p in tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
               if p['name'] == 'minijinja')
assert package['version'] == '2.21.0'
library = REGISTRY / 'minijinja-2.21.0/src'
names = ['environment.rs', 'defaults.rs', 'functions.rs', 'value/mod.rs',
         'value/type_erase.rs', 'compiler/codegen.rs', 'compiler/tokens.rs',
         'utils.rs', 'syntax.rs', 'macros.rs']
inputs = {name: (library / name).read_text() for name in names}
defaults = inputs['defaults.rs'].split('#[cfg(test)]')[0]
counts = {}
for kind in ('builtin_filters', 'builtin_tests', 'globals'):
    body = defaults.split(f'fn build_{kind}()', 1)[1].split('pub(crate) fn get_', 1)[0]
    counts[kind] = len(re.findall(r'rv\.insert\(\s*"', body))
assert 0 < sum(counts.values()) <= 128
assert 'assert_eq!(std::mem::size_of::<Value>(), 24)' in inputs['value/mod.rs']
assert 'const CODEGEN_POOL_MAX_ITEMS: usize = 64;' in inputs['compiler/codegen.rs']
assert 'const CODEGEN_POOLED_MAX_CAPACITY: usize = 64;' in inputs['compiler/codegen.rs']
assert 'const SMALL_INT_FORMAT_CACHE_LIMIT: usize = 256;' in inputs['utils.rs']
compiler = subprocess.check_output(['/home/ran/.cargo/bin/rustc', '-vV'], text=True)
assert 'release: 1.96.0' in compiler and 'host: x86_64-unknown-linux-gnu' in compiler
sysroot = Path(subprocess.check_output(['/home/ran/.cargo/bin/rustc', '--print', 'sysroot'], text=True).strip())
btree = sysroot / 'lib/rustlib/src/rust/library/alloc/src/collections/btree/node.rs'
btree_source = btree.read_text()
assert 'const B: usize = 6;' in btree_source
assert 'pub(super) const CAPACITY: usize = 2 * B - 1;' in btree_source
components = {
    'three_builtin_maps_and_function_arcs': 128 * (1024 + 128) + 4096,
    'both_tls_codegen_pools_and_vec_roots': 2 * 64 * 64 * 64 + 8192,
    'small_integer_string_cache': 16 * 1024,
    'default_delimiter_and_callback_roots': 4096,
    'remaining_empty_tls_and_environment_roots': 8192,
}
allowance = 1024 * 1024
assert sum(components.values()) < allowance
paths = [library / name for name in names] + [btree, ROOT / 'Cargo.lock', Path(__file__)]
result = {
    'audited_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'compiler': compiler, 'package': package,
    'all_feature_literal_insert_counts': counts,
    'upper_components_bytes': components, 'derived_upper_bytes': sum(components.values()),
    'reserved_shared_prompt_upper_bytes': allowance,
    'sources': [{'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
                for path in paths],
    'assumptions': 'See prompt-shared-bound.md; source-specific fixed-cache bound, not arbitrary template/global mutation.',
    'lifetime': 'Count within retained hydration assets; persistent static/TLS owners remain covered by coordinated Running/process admission after candidate drop.',
}
(OUT / 'prompt-shared-bound.json').write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
print(json.dumps({'counts': counts, 'derived_upper_bytes': sum(components.values()),
                  'allowance_bytes': allowance}))
