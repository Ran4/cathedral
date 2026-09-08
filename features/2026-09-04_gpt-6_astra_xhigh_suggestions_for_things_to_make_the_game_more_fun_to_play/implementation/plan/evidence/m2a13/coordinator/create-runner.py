from pathlib import Path
import ast, hashlib, json, pprint

root = Path('/home/ran/src/rust/cathedralbevy')
here = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
debug = {mode:json.loads((here/f'm2a13/development/{mode}_final_smoke.json').read_bytes()) for mode in ('authored','populated')}
canon = lambda v:hashlib.sha256(json.dumps(v,sort_keys=True,separators=(',',':')).encode()).hexdigest()
for mode,d in debug.items():
    assert d['counts'] == {
        'characters':2520 if mode=='populated' else 520, 'accepted_recordings':3,
        'available_text_bytes':85, 'available_texts':1, 'basename_bytes':84,
        'batch_pending':2, 'captures':3, 'parked':1, 'request_id_bytes':50,
        'semantic_receipts':3, 'streams':2, 'terminal_receipts':0, 'unique_roots':2,
    }
    w=d['witnesses']; state=d['boundary_speech']['state']
    assert w['poll_count']==17 and w['coarse_discard_diagnostics']==0
    assert 0<w['maximum_poll_step_seconds']<=.05+1e-12
    assert w['boundary_seconds']==.34
    assert len(w['all_speech_messages'])==2 and len(w['submitted_prompts'])==2
    assert len(w['submitted_inputs'])==13 and len(w['recording_receipts'])==3
    assert [r['source'] for r in state['accepted_recordings']]==['batch_pending','batch_pending','parked']
    assert state['status']=='interrupted_unsent' and state['purpose']=='public_player_speech'
    assert state['streams'][0]['available_text']==w['draft_text']
    assert len(w['tts_requests'])==1 and w['tts_requests'][0]['accepted']
    assert d['cost']['validation_working_bytes']==4194304

text=(here/'run_m2_continuity_probes.py').read_text()
text=text.replace('M2a12 floor and Engine continuity','M2a13 interrupted speech inputs')
text=text.replace('alibi_continuity_cost','alibi_speech_cost')
start=text.index('EXPECTED_COUNTS = '); end=text.index('\n\ndef main():',start)
constants='EXPECTED_COUNTS = '+pprint.pformat({m:d['counts'] for m,d in debug.items()},sort_dicts=True)+'\n'
constants+='EXPECTED_WITNESS_HASHES = '+pprint.pformat({m:canon(d['witnesses']) for m,d in debug.items()})+'\n'
constants+='EXPECTED_BOUNDARY_HASHES = '+pprint.pformat({m:canon(d['boundary_speech']) for m,d in debug.items()})+'\n'
text=text[:start]+constants+text[end:]
start=text.index('        "measurement": ');end=text.index('        "unchanged_source_binary_and_runners_at_end":',start)
text=text[:start]+'''        "measurement": "Sequential release interrupted speech component calls; six raw phases. "
            "Decode and candidate validation include exact receipt/root and player binding gates. "
            "Ordinary typed speech, scripted provider/TTS/STT values, onset and accepted recordings "
            "establish available unsent text, active capture and distinct pending obligations. "
            "Original input, every Speech publication and exact boundary records are retained. "
            "Admission bounds are not allocator measurements; process RSS includes Engine setup. "
            "No complete save/load, host, disk or renderer measurement.",
'''+text[end:]
start=text.index('            if data["scenario"] != ');end=text.index('            cost = data["cost"]',start)
text=text[:start]+'''            if data["scenario"] != "speech-ordinary-interrupted-inputs-v1" or counts != EXPECTED_COUNTS[mode]:
                raise RuntimeError("interrupted speech counts differ from the reviewed workload")
            if not (witnesses["poll_count"] == 17
                    and 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12
                    and witnesses["coarse_discard_diagnostics"] == 0):
                raise RuntimeError("ordinary setup did not retain bounded physical time")
            if len(witnesses["all_speech_messages"]) != 2 or len(witnesses["submitted_prompts"]) != 2:
                raise RuntimeError("original provider input or committed Speech output is missing")
            if len(witnesses["recording_receipts"]) != 3 or len(witnesses["submitted_inputs"]) != 13:
                raise RuntimeError("original recording inputs or exact receipts are missing")
            state = data["boundary_speech"]["state"]
            if state["purpose"] != "public_player_speech" or state["status"] != "interrupted_unsent":
                raise RuntimeError("speech input disposition differs")
            if [r["source"] for r in state["accepted_recordings"]] != ["batch_pending", "batch_pending", "parked"]:
                raise RuntimeError("accepted recording order or source differs")
            witness_hash = hashlib.sha256(json.dumps(
                witnesses, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            if witness_hash != EXPECTED_WITNESS_HASHES[mode]:
                raise RuntimeError("ordinary speech inputs or publications differ")
            boundary_hash = hashlib.sha256(json.dumps(
                data["boundary_speech"], sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            if boundary_hash != EXPECTED_BOUNDARY_HASHES[mode]:
                raise RuntimeError("exact draft, receipts or configuration differ")
'''+text[end:]
text=text.replace('cost["validation_working_bytes"] != 0','cost["validation_working_bytes"] != 4096 * 1024')
text=text.replace('Continuity validation','Speech validation').replace('Continuity component','Speech component')
ast.parse(text)
target=here/'run_m2_speech_probes.py';assert not target.exists();target.write_text(text)

counts=pprint.pformat(debug['authored']['counts'],sort_dicts=True)
counts=counts.replace("'characters': 520", "'characters': 520 + expected_extra")
branch='''            elif data["scenario"] == "speech-ordinary-interrupted-inputs-v1":
                assert validation_working_bytes == 4096 * 1024
                assert counts == COUNTS
                witnesses = data["witnesses"]
                assert witnesses["coarse_discard_diagnostics"] == 0
                assert witnesses["poll_count"] == 17
                assert 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12
                assert witnesses["boundary_seconds"] == 0.34
                assert len(witnesses["submitted_prompts"]) == 2
                assert len(witnesses["all_speech_messages"]) == 2
                assert len(witnesses["submitted_inputs"]) == 13
                assert len(witnesses["recording_receipts"]) == 3
                assert witnesses["provider_submissions"] == 2
                assert witnesses["all_message_digest_algorithm"] == "fnv1a64-debug-stream-v1"
                voices = witnesses["tts_requests"]
                assert len(voices) == 1 and voices[0]["accepted"] and voices[0]["kind"] == "cloud"
                expected_witness_hash = WITNESSES[mode]
                assert sha(json.dumps(witnesses, sort_keys=True,
                                      separators=(",", ":")).encode()) == expected_witness_hash
                expected_boundary_hash = BOUNDARIES[mode]
                assert sha(json.dumps(data["boundary_speech"], sort_keys=True,
                                      separators=(",", ":")).encode()) == expected_boundary_hash
                assert data["shared_reserved_peak_excluding_running_bytes"] == 2 * cost["peak_bytes"]
'''
branch=branch.replace('COUNTS',counts.replace('\n','\n                '))
branch=branch.replace('WITNESSES',repr({m:canon(d['witnesses']) for m,d in debug.items()}))
branch=branch.replace('BOUNDARIES',repr({m:canon(d['boundary_speech']) for m,d in debug.items()}))
audit=here/'audit_m2_component_probes.py';old=audit.read_text()
anchor='            else:\n                raise AssertionError("unrecognized component validation workload")'
assert old.count(anchor)==1
updated=old.replace(anchor,branch+anchor)
assert updated.replace(branch,'',1)==old
ast.parse(updated);audit.write_text(updated)
print(json.dumps({'created':str(target.relative_to(root)),'historical_auditor_branches_unchanged':True,'witness_sha256':{m:canon(d['witnesses']) for m,d in debug.items()},'boundary_sha256':{m:canon(d['boundary_speech']) for m,d in debug.items()}}))
