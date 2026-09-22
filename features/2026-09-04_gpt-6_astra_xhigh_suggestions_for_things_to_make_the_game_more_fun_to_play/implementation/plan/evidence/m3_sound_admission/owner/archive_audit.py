"""Record the pinned parser/container/numeric implementation used by sound admission."""
import hashlib
import json
from pathlib import Path
ROOT=Path('/home/ran/src/rust/cathedralbevy')
DEST=ROOT/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m3_sound_admission/audit'
REG=Path('/home/ran/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
STD=Path('/home/ran/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library')
files=[ROOT/'Cargo.lock',ROOT/'crates/cathedral-sim/Cargo.toml',ROOT/'assets/sounds/catalog.toml',REG/'serde_core-1.0.228/src/de/impls.rs',STD/'alloc/src/raw_vec/mod.rs',STD/'alloc/src/collections/btree/node.rs',STD/'alloc/src/vec/mod.rs']
for base in [REG/'toml-0.9.12+spec-1.1.0/src/de',REG/'toml_parser-1.1.2+spec-1.1.0/src',REG/'toml_datetime-0.7.5+spec-1.1.0/src',REG/'serde_spanned-1.1.1/src',STD/'core/src/num/imp/dec2flt']:
    assert base.is_dir(),base
    files.extend(sorted(base.rglob('*.rs')))
files.append(REG/'toml-0.9.12+spec-1.1.0/src/map.rs')
identities={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
result={'platform':'x86_64 Linux/GNU; rustc1.96.0', 'files':identities,
    'toml_features':['default','display','parse','serde','std'],
    'parser':'toml0.9.12+spec1.1.0; toml_parser1.1.2+spec1.1.0; serde_spanned1.1.1; serde_core1.0.228',
    'numeric_path':'DeFloat::to_f64 -> core/num/imp/dec2flt -> slow::parse_long_mantissa -> DecimalSeq digits:[u8;768]; no heap for arbitrary supported float tokens',
    'scope':'Runtime-editable captured source, complete pinned TOML grammar, closed CatalogFile schema. Source storage independently admitted; no arbitrary Deserialize, shared allocator/RSS or whole-world proof.'}
DEST.mkdir(exist_ok=True)
(DEST/'identity.json').write_text(json.dumps(result,sort_keys=True,indent=2)+'\n')
print(json.dumps({'audit_files':len(identities),'identity':str(DEST/'identity.json')},sort_keys=True))
