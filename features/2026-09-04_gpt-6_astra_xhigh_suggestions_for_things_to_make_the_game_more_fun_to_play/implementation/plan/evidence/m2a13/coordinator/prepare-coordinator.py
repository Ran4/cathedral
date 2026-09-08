from pathlib import Path
import ast

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
previous = here / 'm2a12/coordinator'
names = ['release-build.py', 'audit-logs.py', 'audit-docs.py',
         'audit-performance.py', 'audit-tails.py']
for name in names:
    source = (previous / name).read_text()
    source = source.replace('m2a12', 'm2a13').replace('continuity', 'speech')
    if name == 'audit-logs.py':
        source = source.replace(
            "totals[workspace]['passed'] >= 2069 and totals[workspace]['ignored'] >= 31",
            "totals[workspace]['passed'] >= 2090 and totals[workspace]['ignored'] >= 33")
    if name == 'audit-performance.py':
        line = "    ('historical_scheduler','m2a11/performance',False),"
        assert source.count(line) == 1
        source = source.replace(line, line + "\n    ('historical_continuity','m2a12/performance',False),")
    ast.parse(source, filename=name)
    target = Path('/tmp') / ('alibi-m2a13-' + name)
    assert not target.exists(), target
    target.write_text(source)
print('Prepared five coordinator scripts; none executed.')
