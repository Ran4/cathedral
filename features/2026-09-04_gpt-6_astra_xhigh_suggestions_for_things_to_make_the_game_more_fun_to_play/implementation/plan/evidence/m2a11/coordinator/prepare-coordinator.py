from pathlib import Path
import ast

root = Path('/home/ran/src/rust/cathedralbevy')
here = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
for suffix in ['release-build', 'audit-logs', 'audit-stdlib', 'audit-docs']:
    source = Path(f'/tmp/alibi-m2a10-{suffix}.py').read_text()
    source = source.replace('m2a10', 'm2a11').replace('M2a10', 'M2a11')
    source = source.replace('alibi_social_cost', 'alibi_scheduler_cost')
    source = source.replace('m2a11-social-reference', 'm2a11-scheduler-reference')
    source = source.replace('fd2f3d491a313881a6e71985ef6e6f486849903a', '2a5115060b8dc8731d93ca14d0a015d72898f981')
    ast.parse(source)
    target = Path(f'/tmp/alibi-m2a11-{suffix}.py')
    assert not target.exists()
    target.write_text(source)
    print(target)
print('Templates prepared only; review owner-specific assertions before execution.')
