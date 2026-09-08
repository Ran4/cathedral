from pathlib import Path
import os,json,time
rows={}
for p in Path('/proc').iterdir():
 if not p.name.isdigit():continue
 try:
  stat=(p/'stat').read_text();end=stat.rfind(')');fields=stat[end+2:].split();cmd=(p/'cmdline').read_bytes().split(b'\0');rows[int(p.name)]={'pid':int(p.name),'comm':stat[stat.find('(')+1:end],'ppid':int(fields[1]),'state':fields[0],'utime_ticks':int(fields[11]),'stime_ticks':int(fields[12]),'start_ticks':int(fields[19]),'rss_pages':int(fields[21]),'cmd':cmd}
 except (FileNotFoundError,ProcessLookupError,PermissionError,IndexError):pass
roots=[pid for pid,r in rows.items()if any(x.endswith(b'/m2a13/development/run_checked.py')for x in r['cmd']) and b'workspace_serial' in r['cmd']]
selected=set(roots)
while True:
 new={pid for pid,r in rows.items()if r['ppid']in selected}-selected
 if not new:break
 selected.update(new)
result=[]
for pid in sorted(selected):
 r=rows[pid];r.pop('cmd');r['rss_bytes']=r.pop('rss_pages')*os.sysconf('SC_PAGE_SIZE')
 for name in ['io','wchan']:
  try:r[name]=(Path('/proc')/str(pid)/name).read_text()
  except (FileNotFoundError,ProcessLookupError,PermissionError):r[name]='unavailable'
 result.append(r)
print(json.dumps({'sample_epoch_seconds':time.time(),'clock_ticks_per_second':os.sysconf('SC_CLK_TCK'),'runner_pids':roots,'processes':result,'memory_summary':[line for line in Path('/proc/meminfo').read_text().splitlines()if line.startswith(('MemTotal:','MemAvailable:','SwapTotal:','SwapFree:'))]},indent=2))
