from pathlib import Path
import subprocess

root = Path('/home/ran/src/rust/cathedralbevy')
feature = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play'
plan = feature/'implementation/plan'
head = subprocess.check_output(['/usr/bin/git','rev-parse','HEAD'],cwd=root).decode().strip()
assert head == 'e68d57fcc795b401b10169c0ccb9ab00b21e1556'
for path in [feature/'README.md',plan/'README.md']:
    text = path.read_text()
    old = 'Remaining M2 owner/envelope work and M3–M19 remain.'
    assert text.splitlines()[0].count(old) == 1
    path.write_text(text.replace(old,'M2a12 floor/Engine continuity components are in progress. Remaining M2 owner/envelope work and M3–M19 remain.',1))
path = plan/'M2_simulation_checkpoints.md'
text = path.read_text()
assert '#### M2a12 design cut' not in text
old = 'M2a1–M2a11 private components are implemented and reviewed.'
assert text.splitlines()[0].count(old) == 1
text = text.replace(old,old+' M2a12 floor/Engine continuity is in progress.',1)
draft = Path('/tmp/alibi-m2a12-design-draft.md').read_text()
draft = draft.replace('After the accepted M2a11 scheduler commit,',f'After the accepted M2a11 scheduler commit `{head}`,',1)
path.write_text(text+'\n'+draft)
with Path('/tmp/alibi-m2a11-root-state.md').open('a') as file:
    file.write('\nM2a11 NOW ACCEPTED AND COMMITTED e68d57fcc795b401b10169c0ccb9ab00b21e1556.\nWorkspace2069/31ignored/43targets;151focused,6public. All20originals audited.\nRelease build62455, smoke97473, performance41294 ALL CLOSED exit0.\nRelease binary59af7eebb47a5366467ad2e006021ef11453468950a0a253987821010a2dd237.\nAll3600phases/current+8historical/6negative/source/stdlib/log/release/docs checks pass.\nZero phases>2ms; exportpooledp99authored0.077532ms,populated0.186618ms.\n161files committed; only3unrelateduntracked dirs remain. No scheduler work pending.\nM2a12 design now installed; see new root state for active work.\n')
print('M2a12 design/status installed after accepted M2a11 commit.')
