from pathlib import Path
base=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a13')
prior=base.parent/'m2a12/development/run_checked.py'
(base/'development/run_checked.py').write_text(prior.read_text().replace('m2a12','m2a13'))
p=Path('crates/cathedral-sim/src/engine/speech_checkpoint.rs')
p.write_text(p.read_text().replace('filter_map(|r|r.semantic()) )','filter_map(|r|r.semantic())'))
for p in ('crates/cathedral-sim/src/speech_router/checkpoint/tests.rs','crates/cathedral-sim/src/engine/speech_checkpoint/tests.rs'):
 Path(p).write_text('// Private checkpoint tests follow.\n')
