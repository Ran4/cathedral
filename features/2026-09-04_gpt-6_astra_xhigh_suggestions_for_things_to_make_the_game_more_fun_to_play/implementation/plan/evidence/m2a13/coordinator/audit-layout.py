from pathlib import Path
import datetime,hashlib,json,re
root=Path('/home/ran/src/rust/cathedralbevy')
e=root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a13'
p=e/'development/layout.json';saved=json.loads(p.read_bytes())
records=json.loads((e/'commands.json').read_bytes())
record=next(r for r in records if r['name']==saved['command'])
raw=Path(record['original_path']).read_bytes()
assert record['exit_code']==0
assert hashlib.sha256(raw).hexdigest()==record['original_sha256']
assert saved['source_manifest_sha256']==record['source_manifest_sha256']==hashlib.sha256((e/'source_hashes.json').read_bytes()).hexdigest()
observed={name:json.loads(value) for name,value in re.findall(rb'(speech_layout|speech_engine_layout)=(\{[^\n]+\})',raw)}
observed={name.decode():value for name,value in observed.items()}
assert observed==saved['layouts']
assert observed['speech_layout']['accepted']<=512 and observed['speech_layout']['stream']<=64
assert observed['speech_layout']['string']==24 and observed['speech_layout']['affected']==48
scratch=256*(16+12*8+11*16)+4352*((16+12*8+11*24)+(16+12*8+11*8))
assert scratch==2580480 and scratch+65536<4194304
report={'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source_manifest_sha256':saved['source_manifest_sha256'],'original_layout_command':saved['command'],'layouts':observed,'loose_saved_ledger_node_bytes':scratch,'fixed_framing_bytes':65536,'working_charge_bytes':4194304}
out=e/'coordinator/layout_audit.json';assert not out.exists();out.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'result':'passed','layouts':observed,'saved_ledger_node_bytes':scratch}))
