from pathlib import Path
import sys,subprocess,json,re,datetime
from urllib.parse import urlsplit,unquote
root=Path('/home/ran/src/rust/cathedralbevy')
here=root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence'
e=here/'m2a12'
command=['/home/ran/.local/bin/uv','run','--no-project','--cache-dir','/tmp/alibi-uv',str(here/'validate_plan.py')]
result=subprocess.run(command,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
assert result.returncode==0, result.stderr.decode()
report=json.loads(result.stdout)
(e/'coordinator/plan_validation.json').write_text(json.dumps(report,indent=2)+'\n')
sys.path.insert(0,str(here))
from validate_plan import anchors,without_fences
paths=sorted(e.rglob('*.md'));checks=[]
for p in paths:
    for raw in re.findall(r'!?\[[^\]]*\]\(([^)]+)\)',without_fences(p.read_text())):
        target=raw.strip();target=target[1:target.index('>')] if target.startswith('<') and '>' in target else target.split(' "',1)[0]
        parsed=urlsplit(target)
        if parsed.scheme or parsed.netloc: continue
        destination=(p.parent/unquote(parsed.path)).resolve() if parsed.path else p.resolve()
        assert destination.is_relative_to(root), (p,target)
        assert destination.exists(),(p,target)
        if parsed.fragment and destination.suffix=='.md': assert unquote(parsed.fragment) in anchors(destination),(p,target)
        checks.append({'source':p.relative_to(e).as_posix(),'target':target})
links={'checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'result':'passed','markdown_files':len(paths),'local_links':len(checks),'checks':checks}
(e/'coordinator/evidence_link_audit.json').write_text(json.dumps(links,indent=2)+'\n')
print(json.dumps({'plan':report,'evidence_markdown_files':len(paths),'evidence_local_links':len(checks)},indent=2))
