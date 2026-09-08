from pathlib import Path
import ast, hashlib, json, pprint

root = Path('/home/ran/src/rust/cathedralbevy')
here = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
debug = {mode:json.loads((here/f'm2a12/development/{mode}_final_smoke.json').read_bytes()) for mode in ('authored','populated')}
canon = lambda v:hashlib.sha256(json.dumps(v,sort_keys=True,separators=(',',':')).encode()).hexdigest()
for mode,d in debug.items():
    assert d['counts']['characters'] == (2520 if mode=='populated' else 520)
    assert d['witnesses']['poll_count'] == 25
    assert d['witnesses']['coarse_discard_diagnostics'] == 0
    assert len(d['witnesses']['all_speech_messages']) == 5
    assert d['cost']['validation_working_bytes'] == 0
text = (here/'run_m2_scheduler_probes.py').read_text()
text = text.replace('M2a11 existing scheduler authority','M2a12 floor and Engine continuity')
text = text.replace('alibi_scheduler_cost','alibi_continuity_cost')
start = text.index('EXPECTED_COUNTS = ')
end = text.index('\n\ndef main():',start)
constants = 'EXPECTED_COUNTS = '+pprint.pformat({m:d['counts'] for m,d in debug.items()},sort_dicts=True)+'\n'
constants += 'EXPECTED_WITNESS_HASHES = '+pprint.pformat({m:canon(d['witnesses']) for m,d in debug.items()})+'\n'
constants += 'EXPECTED_BOUNDARY_HASHES = '+pprint.pformat({m:canon(d['boundary_continuity']) for m,d in debug.items()})+'\n'
text = text[:start]+constants+text[end:]
start = text.index('        "measurement": ')
end = text.index('        "unchanged_source_binary_and_runners_at_end":',start)
text = text[:start]+'''        "measurement": "Sequential release Floor/Engine continuity component calls; six raw phases. "
            "Decode and candidate validation include scoped floor, config and player binding gates. "
            "Ordinary speech, scripted provider/TTS values, acknowledgements, voice selection, "
            "sound cooldown and microphone onset establish the boundary through bounded polls. "
            "Original input, every Speech publication and exact initial/boundary records are retained. "
            "Admission bounds are not allocator measurements; process RSS includes Engine setup. "
            "No complete save/load, host, disk or renderer measurement.",
'''+text[end:]
start = text.index('            if data["scenario"] != ')
end = text.index('            cost = data["cost"]',start)
text = text[:start]+'''            if data["scenario"] != "continuity-ordinary-voiced-reading-cadence-v1" or counts != EXPECTED_COUNTS[mode]:
                raise RuntimeError("floor/Engine continuity counts differ from the reviewed workload")
            if not (witnesses["poll_count"] == 25
                    and 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12
                    and witnesses["coarse_discard_diagnostics"] == 0):
                raise RuntimeError("ordinary setup did not retain bounded physical time")
            if len(witnesses["all_speech_messages"]) != 5 or len(witnesses["submitted_prompts"]) != 3:
                raise RuntimeError("original provider input or committed Speech output is missing")
            voices = witnesses["tts_requests"]
            if [v["accepted"] for v in voices] != [True, True, True, False]:
                raise RuntimeError("voiced and reading-fallback work differs")
            if [v["kind"] for v in voices] != ["cloud", "cloud", "local", "local"]:
                raise RuntimeError("queue-time voice selection was not retained")
            witness_hash = hashlib.sha256(json.dumps(
                witnesses, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            if witness_hash != EXPECTED_WITNESS_HASHES[mode]:
                raise RuntimeError("ordinary continuity inputs or publications differ")
            boundary_hash = hashlib.sha256(json.dumps(
                data["boundary_continuity"], sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            if boundary_hash != EXPECTED_BOUNDARY_HASHES[mode]:
                raise RuntimeError("exact boundary pacing, caches or configuration differ")
'''+text[end:]
text = text.replace('cost["validation_working_bytes"] != 4096 * 1024','cost["validation_working_bytes"] != 0')
text = text.replace('Scheduler validation','Continuity validation').replace('Scheduler component','Continuity component')
text = text.replace('if not cost["peak_bytes"] <= retained <= 1024**3:', 'if not retained == 2 * cost["peak_bytes"] <= 1024**3:')
ast.parse(text)
target = here/'run_m2_continuity_probes.py'
assert not target.exists()
target.write_text(text)
print(json.dumps({'created':str(target.relative_to(root)),'witness_sha256':{m:canon(d['witnesses']) for m,d in debug.items()},'boundary_sha256':{m:canon(d['boundary_continuity']) for m,d in debug.items()}}))
