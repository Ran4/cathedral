from pathlib import Path
import sys,json,hashlib,datetime,subprocess
base=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(base.parent))
from component_input_sources import SOURCE_SCOPE,sources
manifest=sources();raw=(json.dumps(manifest,indent=2)+'\n').encode();sha=lambda b:hashlib.sha256(b).hexdigest()
assert manifest==json.loads((base/'development/authored_bounded_smoke.source.json').read_text())
assert manifest==json.loads((base/'development/populated_bounded_smoke.source.json').read_text())
(base/'source_hashes.json').write_bytes(raw)
record={'frozen_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'base_head':subprocess.check_output(['/usr/bin/git','rev-parse','HEAD'],text=True).strip(),'manifest':'source_hashes.json','manifest_sha256':sha(raw),'files':len(manifest),'source_scope':SOURCE_SCOPE,'scope':'Shared v2 complete compiled Rust/tests/examples and input assets plus four new scheduler fixtures, including the independent root public test. Evidence/orchestration hashed separately.','source_helper_sha256':sha((base.parent/'component_input_sources.py').read_bytes()),'ordinary_source_changes':'Only scheduler/Engine module declarations and appended borrowed TURN-root receipt queries. No existing ordinary behavior edits.','public_test_included':True,'new_component_fixture_count':4,'supersedes_provisional_freeze':'development/provisional_source_freeze.json; original probe discarded physical time and final probe uses bounded accepted-time polls','full_m2_or_host_acceptance':False}
(base/'source_freeze.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record))
