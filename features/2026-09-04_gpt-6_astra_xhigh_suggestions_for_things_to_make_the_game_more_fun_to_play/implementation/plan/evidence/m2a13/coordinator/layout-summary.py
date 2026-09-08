from pathlib import Path
import json,hashlib,re
base=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a13')
log=Path('/tmp/alibi-m2a13-checkpoint_final.log').read_text()
layouts={key:json.loads(re.search(r'^'+key+r'=(\{.*\})$',log,re.M).group(1))for key in ['speech_layout','speech_engine_layout']}
(base/'development/layout.json').write_text(json.dumps({'source_manifest_sha256':json.loads((base/'source_freeze.json').read_text())['source_manifest_sha256'],'command':'checkpoint_final','layouts':layouts},indent=2)+'\n')
record={'compiler_verbose':Path('/tmp/alibi-m2a13-rustc_version.log').read_text(),'compiler_sysroot':Path('/tmp/alibi-m2a13-rustc_sysroot.log').read_text().strip(),'source_freeze':json.loads((base/'source_freeze.json').read_text()),'command_environment':json.loads((base/'commands.json').read_text())[-1]['environment'],'constraints':['offline serial Cargo','headless and fake-backend environment','pure controlled STT/TTS/Cognition probe; no devices or providers','GPU unavailable in earlier accepted evidence; not reprobed']}
(base/'environment.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(layouts))
