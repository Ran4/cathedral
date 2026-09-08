from pathlib import Path
import datetime, gzip, hashlib, json, math

root = Path('/home/ran/src/rust/cathedralbevy')
e = root / 'features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m2a12'
folder = e / 'performance'
rows = json.loads((folder / 'RESULTS.json').read_bytes())
phases = ('preflight_us','export_us','encode_us','decode_validate_us','candidate_validate_us','drop_us')
pooled = {mode:{phase:[] for phase in phases} for mode in ('authored','populated')}
per_run, exceeded = {}, []
def quantiles(values):
    ordered = sorted(values)
    assert ordered and all(math.isfinite(x) and x >= 0 for x in ordered)
    return {k:ordered[math.ceil(len(ordered)*p)-1] for k,p in (('p50',.5),('p95',.95),('p99',.99),('max',1))}
for row in rows:
    name = row['name']
    raw = gzip.decompress((folder / f'{name}.json.gz').read_bytes())
    assert hashlib.sha256(raw).hexdigest() == row['artifacts']['uncompressed_json_sha256']
    data = json.loads(raw)
    assert data['samples'] == 100
    per_run[name] = {}
    for phase in phases:
        values = data[phase]
        assert len(values) == 100
        observed = quantiles(values)
        assert observed == row['phase_us'][phase]
        per_run[name][phase] = observed
        pooled[data['mode']][phase].extend(values)
        exceeded.extend({'run':name,'phase':phase,'sample_index':i,'microseconds':v}
            for i,v in enumerate(values) if v > 2000)
assert all(len(values)==300 for data in pooled.values() for values in data.values())
report = {
    'result':'passed','checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'method':'nearest-rank percentiles recomputed from exact original phase arrays; no outlier removal or rerun',
    'per_run_microseconds':per_run,
    'pooled_300_sample_microseconds':{mode:{phase:quantiles(values) for phase,values in data.items()} for mode,data in pooled.items()},
    'samples_above_2ms':exceeded,'samples_above_2ms_count':len(exceeded),
    'limit':'These component phase samples do not establish the full host poll/frame capture budget. No attribution of timing tails is possible from phase timing records alone.',
}
path = e / 'coordinator/tail_latency_audit.json'
assert not path.exists()
path.write_text(json.dumps(report,indent=2)+'\n')
lines = [
    'All 3,600 measured phase samples are retained. The table below pools the three 100-sample runs per population, rather than taking a median of per-run percentiles.',
    '', '| Population | Phase | Pooled p99 (ms) | Observed maximum (ms) |',
    '|---|---|---:|---:|',
]
for mode,data in report['pooled_300_sample_microseconds'].items():
    for phase,q in data.items():
        lines.append(f'| {mode} | {phase.removesuffix("_us")} | {q["p99"]/1000:.6f} | {q["max"]/1000:.6f} |')
lines += ['',f'{len(exceeded)} phase samples exceeded 2 ms. Their exact run, phase, sample index and duration are retained in [the tail audit](../coordinator/tail_latency_audit.json). These measurements establish component costs only; the complete host capture, worker coordination and frame budget remain separate acceptance gates. Timing records alone do not establish the cause of individual long samples.','']
(e/'coordinator/tail_latency_table.md').write_text('\n'.join(lines))
print(json.dumps({'result':'passed','samples_above_2ms':len(exceeded),
    'pooled_300_sample_microseconds':report['pooled_300_sample_microseconds']},indent=2))
