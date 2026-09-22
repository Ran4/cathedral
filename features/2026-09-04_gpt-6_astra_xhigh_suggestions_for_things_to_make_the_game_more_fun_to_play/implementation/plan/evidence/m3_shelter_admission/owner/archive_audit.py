"""Record the actual embedded artifact and pinned parser/container source audit."""
import hashlib
import json
from pathlib import Path

ROOT=Path('/home/ran/src/rust/cathedralbevy')
DEST=ROOT/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m3_shelter_admission/audit'
REG=Path('/home/ran/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
STD=Path('/home/ran/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src')
files=[ROOT/'assets/world/shelters.json',REG/'serde_core-1.0.228/src/de/impls.rs',
       REG/'serde_json-1.0.150/src/de.rs',REG/'serde_json-1.0.150/src/read.rs',
       REG/'serde_json-1.0.150/src/lexical/parse.rs',REG/'serde_json-1.0.150/src/lexical/algorithm.rs',
       REG/'serde_json-1.0.150/src/lexical/num.rs',STD/'raw_vec/mod.rs',STD/'collections/btree/node.rs']
identities={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
source=files[0].read_text()
tokens=[]
def number(text):
    tokens.append(text)
    return float(text) if '.' in text or 'e' in text.lower() else int(text)
document=json.loads(source,parse_int=number,parse_float=number)
assert all('e' not in token.lower() for token in tokens)
mantissas=[int(token.lstrip('-').replace('.','')) for token in tokens]
fractional=[len(token.split('.')[1]) if '.' in token else 0 for token in tokens]
assert max(mantissas)<2**53 and max(fractional)<=22
rows=document['shelters']
result={'platform':'x86_64 Linux/GNU; rustc1.96.0; serde_core1.0.228; serde_json1.0.150 float_roundtrip',
        'files':identities,'artifact_bytes':len(source.encode()),'rows':len(rows),
        'vertices':sum(len(row['polygon_xz']) for row in rows),
        'offices':sum(len(row.get('offices',[])) for row in rows),
        'numeric_tokens':len(tokens),'max_numeric_mantissa':max(mantissas),
        'max_fractional_digits':max(fractional),'exponent_notation':False,
        'numeric_path':'de.rs625 -> lexical parse_concise_float -> fast_path; mantissa<2^53 and exponent -2..0 within -22..22',
        'scope':'Current embedded artifact only. Future numeric/serde changes require re-audit; arbitrary bigint parsing is not admitted.'}
(DEST/'identity.json').write_text(json.dumps(result,sort_keys=True,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='files'},sort_keys=True))
