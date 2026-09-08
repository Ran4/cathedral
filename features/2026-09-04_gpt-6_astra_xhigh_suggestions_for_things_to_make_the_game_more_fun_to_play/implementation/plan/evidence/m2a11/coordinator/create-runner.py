from pathlib import Path
import ast, hashlib, json, pprint

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
data = {mode:json.loads((here/f'm2a11/development/{mode}_bounded_smoke.json').read_text())
    for mode in ('authored','populated')}
sha = lambda v: hashlib.sha256(json.dumps(v,sort_keys=True,separators=(',',':')).encode()).hexdigest()
for mode,d in data.items():
    c,w = d['counts'],d['witnesses']
    assert d['scenario'] == 'scheduler-held-and-retry-v1'
    assert c['characters'] == (520 if mode=='authored' else 2520)
    assert c['order_slots'] > c['characters'] and c['round_robin_index'] == 0
    assert c['retry_work'] == 1 and c['provider_failures'] == 1
    assert c['priority_handoffs'] >= 1 and c['player_reactions'] >= 1
    assert all(c[k] is True for k in ('running','in_flight','flight_player_reaction','held_success'))
    assert c['held_error'] is False and c['submitted'] is False
    assert c['drained_rows'] > 0 and c['presented_rows'] > 0
    assert w['floor_busy_hold'] is True and w['provider_submissions'] == 3
    assert w['boundary_seconds'] == 5.3
    assert w['coarse_discard_diagnostics'] == 0
    assert 0 < w['maximum_poll_step_seconds'] <= 0.05 + 1e-12
    assert len(w['submitted_prompts']) == 3
    assert w['submitted_actor'] == w['partner']
    assert w['late_utterance'] not in w['submitted_prompts'][-1][0]
    assert len(w['submitted_prompts'][-1][0].encode()) == c['prompt_bytes']
    assert len(w['held_reply'].encode()) == c['held_bytes']
    assert d['cost']['validation_working_bytes'] == 4 * 1024**2
    print(json.dumps({'mode':mode,'counts':c,'witness_sha256':sha(w),'cost':d['cost']}))
source = (here/'run_m2_social_probes.py').read_text()
source = source.replace('M2a10 existing social authority','M2a11 existing scheduler authority')
source = source.replace('alibi_social_cost','alibi_scheduler_cost')
source = source.replace('include social and player binding gates.', 'include scheduler, semantic-root and player binding gates.')
source = source.replace('"Ordinary typed speech, a scripted completion and a later gaze sample "\n            "establish reciprocal engagement, partial focus, warm NPC exchanges and submission-time novelty. "',
    '"Ordinary typed speech and scripted completions establish an unfinished protected "\n            "flight with a raw held reply, a later queued followup, an ordinary handoff "\n            "and a retained failed obligation. Original prompts and output budgets are witnessed. "')
start = source.index('            counts = data["counts"]')
end = source.index('            cost = data["cost"]', start)
gate = '''            counts = data["counts"]
            witnesses = data["witnesses"]
            if data["scenario"] != "scheduler-held-and-retry-v1" or counts != EXPECTED_COUNTS[mode]:
                raise RuntimeError("scheduler held flight, queues, retry or resolved-input counts differ")
            witness_hash = hashlib.sha256(json.dumps(
                witnesses, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            if witness_hash != EXPECTED_WITNESS_HASHES[mode]:
                raise RuntimeError("ordinary scheduler inputs, outputs or floor-hold witnesses differ")
'''
source = source[:start] + gate + source[end:]
source = source.replace('cost["validation_working_bytes"] != 64 * 1024', 'cost["validation_working_bytes"] != 4096 * 1024')
source = source.replace('Social validation', 'Scheduler validation').replace('Social component', 'Scheduler component')
constants = '\n# Reviewed functional debug workloads; no private owner mutation in the probe.\n'
constants += 'EXPECTED_COUNTS = ' + pprint.pformat({m:d['counts'] for m,d in data.items()},sort_dicts=True,width=100) + '\n'
constants += 'EXPECTED_WITNESS_HASHES = ' + pprint.pformat({m:sha(d['witnesses']) for m,d in data.items()},sort_dicts=True,width=110) + '\n'
source = source.replace('\n\ndef main():', constants + '\n\ndef main():',1)
assert 'social' not in source.lower()
ast.parse(source)
target = here/'run_m2_scheduler_probes.py'
assert not target.exists()
target.write_text(source)
print(target)
