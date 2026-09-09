from pathlib import Path
import hashlib,json,math,sys
base=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(base.parent))
from component_input_sources import sources
assert sources()==json.loads((base/'source_hashes.json').read_text())
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
rows=[]
for lane in ['scheduler','night']:
    for mode in ['authored','populated']:
        old_path=base.parent/('m2a11' if lane=='scheduler' else 'm2a9')/'development'/(mode+('_bounded_smoke.json' if lane=='scheduler' else '_smoke.json'))
        old=json.loads(old_path.read_text())
        default_path=base/'development'/f'{lane}_{mode}_default.json'
        default=json.loads(default_path.read_text())
        for key in ['scenario','counts','witnesses','mode','placement']:
            assert default[key]==old[key],(lane,mode,key)
        new_path=base/'development'/f'{lane}_{mode}_inputs.json'
        new=json.loads(new_path.read_text())
        assert new['owner_counts']==default['counts']
        assert new['scenario']==f'cognition-inputs-{lane}-v1'
        assert new['mode']==mode and new['placement']==default['placement']
        assert new['owner_counts']['characters']==(520 if mode=='authored' else 2520)
        assert new['witnesses']['coarse_discard_diagnostics']==0
        assert new['witnesses']['maximum_poll_step_seconds']<=.05+1e-12
        assert new['counts'][lane] and not new['counts']['night' if lane=='scheduler' else 'scheduler']
        row=new['saved_inputs'][lane]
        actual=[r for r in new['submitted_requests']if r['request_id']==row['request_id']]
        assert len(actual)==1
        for key in ['method','request_id','prompt','output_token_budget']:assert actual[0][key]==row[key]
        assert new['counts'][lane+'_prompt_bytes']==len(row['prompt'].encode())
        for phase in ['preflight','export','encode','decode_validate','candidate_validate','drop']:
            samples=new[phase+'_us'];assert len(samples)==new['samples'] and all(math.isfinite(x)and x>=0 for x in samples)
        c=new['cost'];assert c['validation_working_bytes']==4*1024**2
        assert c['peak_bytes']==4096+4*c['expanded_upper_bytes']+3*c['encoded_bytes']+c['validation_working_bytes']
        assert new['shared_reserved_peak_excluding_running_bytes']==2*c['peak_bytes']<1024**3
        rows.append({'lane':lane,'mode':mode,'historical_path':str(old_path),'historical_sha256':digest(old_path),'default_path':str(default_path),'default_sha256':digest(default_path),'historical_scenario_counts_witnesses_mode_placement_equal':True,'inputs_path':str(new_path),'inputs_sha256':digest(new_path),'counts':new['counts'],'cost':c,'actual_requests':[{k:r[k]for k in ['request_id','method','output_token_budget']}|{'prompt_bytes':len(r['prompt'].encode())}for r in new['submitted_requests']],'poll_count':new['witnesses']['poll_count'],'maximum_poll_step_seconds':new['witnesses']['maximum_poll_step_seconds'],'coarse_discard_diagnostics':0})
out={'status':'Four opt-in and four unchanged default debug smokes passed; timings are not release acceptance.','source_freeze':json.loads((base/'source_freeze.json').read_text()),'rows':rows,'debug_binary_sha256':{lane:digest(Path.cwd()/'target/debug/examples'/f'alibi_{lane}_cost')for lane in ['scheduler','night']}}
(base/'development/probe_summary.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out,indent=2))
