from pathlib import Path
import sys,json,hashlib,datetime
base=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(base.parent))
from component_input_sources import SOURCE_SCOPE,sources
manifest=sources();path=base/'source_hashes.json';path.write_text(json.dumps(manifest,indent=2)+'\n')
record={'source_scope':SOURCE_SCOPE,'source_helper_sha256':hashlib.sha256((base.parent/'component_input_sources.py').read_bytes()).hexdigest(),'source_manifest':'source_hashes.json','source_manifest_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'source_files':len(manifest),'frozen_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'policy':'No source/input edits during final focused/public/workspace verification or coordinator release checks.'}
(base/'source_freeze.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record))
