from pathlib import Path
import json, gzip, hashlib, math, datetime
root = Path('/home/ran/src/rust/cathedralbevy')
plan = root/'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan'
e = plan/'evidence/m2a10'
sha = lambda b: hashlib.sha256(b).hexdigest()
rows = json.loads((e/'performance/RESULTS.json').read_text())
phases = ['preflight_us','export_us','encode_us','decode_validate_us','candidate_validate_us','drop_us']
pool = {mode:{phase:[] for phase in phases} for mode in ['authored','populated']}
outliers = []
per_run = []
def quantile(values, q): return sorted(values)[math.ceil(len(values)*q)-1]
for row in rows:
    raw = gzip.decompress((e/f'performance/{row["name"]}.json.gz').read_bytes())
    assert sha(raw) == row['artifacts']['uncompressed_json_sha256']
    data = json.loads(raw)
    for phase in phases:
        pool[data['mode']][phase].extend(data[phase])
        assert quantile(data[phase],.99) == row['phase_us'][phase]['p99']
        for i,value in enumerate(data[phase]):
            if value > 2000: outliers.append({'run':row['name'],'phase':phase,'sample_index_zero_based':i,'microseconds':value})
    per_run.append({'run':row['name'],'export_p99_ms':quantile(data['export_us'],.99)/1000,'export_max_ms':max(data['export_us'])/1000,'decode_p99_ms':quantile(data['decode_validate_us'],.99)/1000,'decode_max_ms':max(data['decode_validate_us'])/1000})
pooled = {mode:{phase:{'samples':len(values),'p99_ms':quantile(values,.99)/1000,'max_ms':max(values)/1000,'over_2_ms':sum(v>2000 for v in values)} for phase,values in data.items()} for mode,data in pool.items()}
assert len(outliers) == 19 and {x['run'] for x in outliers} == {'populated-3'}
report = {'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'result_file_sha256':sha((e/'performance/RESULTS.json').read_bytes()),'samples_retained_without_remeasurement':3600,'per_run':per_run,'pooled_by_mode':pooled,'phase_samples_over_2_ms':outliers,'cause':'not determined by these timing records','host_frame_acceptance':False}
path = e/'coordinator/tail_latency_audit.json'; assert not path.exists()
path.write_text(json.dumps(report,indent=2)+'\n')
table = '\n'.join('| '+r['run']+' | '+' | '.join(f'{r[k]:.6f}' for k in ['export_p99_ms','export_max_ms','decode_p99_ms','decode_max_ms'])+' |' for r in per_run)
paragraph = f'''The median table hides a slow third populated run. The [tail audit](../coordinator/tail_latency_audit.json) retains and identifies all nineteen phase samples above 2 ms, all in `populated-3`; four are exports. The recorded timings do not identify the cause. No run was discarded or repeated. Pooling the 300 populated samples per phase gives export p99 **{pooled['populated']['export_us']['p99_ms']:.6f} ms** and decode p99 **{pooled['populated']['decode_validate_us']['p99_ms']:.6f} ms**. This component also requires safe host scheduling; low median per-run figures do not establish a frame bound.

| Run | Export p99 ms | Export max ms | Decode p99 ms | Decode max ms |
|---|---:|---:|---:|---:|
{table}

'''
path = e/'performance/README.md'; value = path.read_text()
marker = 'The largest individual export is '
assert value.count(marker) == 1
value = value.replace(marker,paragraph+marker)
value = value.replace('Earlier components still need offload or incremental host coordination.', 'Complete host integration still needs offload or incremental coordination for these measured tails and the earlier components.')
path.write_text(value)
tail_sentence = f'The [tail audit](tail_latency_audit.json) records nineteen phase samples above 2 ms in the third populated run, including four exports. All samples remain; the cause is undetermined. Populated pooled export/decode p99 is {pooled["populated"]["export_us"]["p99_ms"]:.6f}/{pooled["populated"]["decode_validate_us"]["p99_ms"]:.6f} ms, and maximum decode is {pooled["populated"]["decode_validate_us"]["max_ms"]:.6f} ms. This social component also lacks a synchronous frame guarantee.'
path = e/'coordinator/review.md'; value = path.read_text()
marker = '\nThis accepts the social component only.'
assert value.count(marker) == 1
path.write_text(value.replace(marker,'\n'+tail_sentence+'\n'+marker))
for path in [e/'README.md',plan/'README.md',plan/'M2_simulation_checkpoints.md']:
    with path.open('a') as f:
        link = 'coordinator/tail_latency_audit.json' if path.parent == e else 'evidence/m2a10/coordinator/tail_latency_audit.json'
        f.write(f'\nM2a10 [tail review]({link}) retains a slow third populated run: pooled populated export p99 is {pooled["populated"]["export_us"]["p99_ms"]:.6f} ms, maximum export 13.120826 ms and maximum decode 55.358842 ms. The smaller figures above are medians of per-run percentiles; they do not establish synchronous host-frame acceptance.\n')
print(json.dumps({'result':'passed','retained_phase_samples_above_2_ms':len(outliers),'populated_export_p99_ms':pooled['populated']['export_us']['p99_ms'],'populated_decode_p99_ms':pooled['populated']['decode_validate_us']['p99_ms']}))
