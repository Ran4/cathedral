from pathlib import Path
import ast

root = Path('/home/ran/src/rust/cathedralbevy')
prior = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a11/coordinator/audit-release.py'
text = prior.read_text().replace('m2a11','m2a12').replace('alibi_scheduler_cost','alibi_continuity_cost')
# Confirm these final debug reference names with the owner before running.
text = text.replace('{mode}_bounded_smoke.json','{mode}_smoke.json')
ast.parse(text)
target = Path('/tmp/alibi-m2a12-audit-release.py')
assert not target.exists()
target.write_text(text)
print(target.name)
