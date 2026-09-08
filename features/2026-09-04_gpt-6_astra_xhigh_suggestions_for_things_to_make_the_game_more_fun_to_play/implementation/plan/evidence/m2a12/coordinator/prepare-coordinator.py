from pathlib import Path
import ast

root = Path('/home/ran/src/rust/cathedralbevy')
here = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
prior = here/'m2a11/coordinator'
for name in ['release-build.py','audit-logs.py','audit-stdlib.py','audit-docs.py','audit-performance.py','audit-source.py','audit-tails.py']:
    text = (prior/name).read_text().replace('m2a11','m2a12')
    text = text.replace('alibi_scheduler_cost','alibi_continuity_cost')
    text = text.replace('m2a12-scheduler-reference-binary','m2a12-continuity-reference-binary')
    if name == 'audit-logs.py':
        text = text.replace("['passed'] >= 2051 and totals[workspace]['ignored'] >= 29", "['passed'] >= 2069 and totals[workspace]['ignored'] >= 31")
    if name == 'audit-performance.py':
        text = text.replace("('scheduler_smoke'", "('continuity_smoke'").replace("('scheduler_performance'", "('continuity_performance'")
        text = text.replace("    ('historical_social','m2a10/performance',False),", "    ('historical_social','m2a10/performance',False),\n    ('historical_scheduler','m2a11/performance',False),")
    if name == 'audit-source.py':
        text = text.replace('2a5115060b8dc8731d93ca14d0a015d72898f981','e68d57fcc795b401b10169c0ccb9ab00b21e1556')
        text = text.replace("'pub mod scheduler_checkpoint;\\n'", "'pub mod continuity_checkpoint;\\n'")
        text = text.replace("'crates/cathedral-sim/src/scheduler.rs'", "'crates/cathedral-sim/src/floor.rs'")
        start = text.index("name = 'crates/cathedral-sim/src/receipts/checkpoint.rs'")
        end = text.index('fixtures = {}',start)
        text = text[:start]+text[end:]
        marker = "assert changed <= manifest.keys(), sorted(changed - manifest.keys())\n"
        assert marker in text
        text = text.replace(marker, marker+"""allowed_files = {
    'crates/cathedral-sim/src/engine.rs',
    'crates/cathedral-sim/src/floor.rs',
    'crates/cathedral-sim/src/engine/continuity_checkpoint.rs',
    'crates/cathedral-sim/tests/checkpoint_continuity_boundary.rs',
    'crates/cathedral-backends/examples/alibi_continuity_cost.rs',
}
allowed_prefixes = ('crates/cathedral-sim/src/floor/checkpoint',
    'crates/cathedral-sim/src/engine/continuity_checkpoint/',
    'crates/cathedral-sim/tests/fixtures/checkpoint_continuity/',
    'crates/cathedral-sim/tests/fixtures/checkpoint_floor/')
assert all(p in allowed_files or p.startswith(allowed_prefixes) for p in changed), sorted(changed)
""")
    path = Path('/tmp/alibi-m2a12-'+name)
    assert not path.exists(), path
    ast.parse(text)
    path.write_text(text)
    print(path.name)
