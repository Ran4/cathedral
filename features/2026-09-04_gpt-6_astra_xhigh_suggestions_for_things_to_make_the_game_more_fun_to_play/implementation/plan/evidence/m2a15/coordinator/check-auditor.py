"""Negative checks for audit logic; synthetic data is never host evidence."""
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import sys
from release_common import OUT, sha, write_json

path = OUT / 'audit-release-probes.py'
spec = importlib.util.spec_from_file_location('host_audit', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
valid = {
    'schema': 1, 'scenario': 'actual-host-boundary-v1', 'mode': 'authored', 'samples': 2,
    'counts': {'entities': 5000, 'characters': 500, 'collision_boxes': 2000,
               'collision_prisms': 10, 'dynamic_barriers': 2, 'vermin_colonies': 8,
               'rats': 100, 'rows': 6, 'cut_margin': True},
    'placement': {'requested': 0, 'placed': 0, 'unplaced': 0},
    'readable_counts': {kind: 1 for kind in
        ('draft', 'hud', 'unread_intent', 'unread_speech', 'subtitle', 'bubble')},
    'readonly': {f'{key}_{side}': 1 for key in
        ('world_revision', 'event_sequence', 'input_watermark') for side in ('before', 'after')},
    'cost': {'encoded_bytes': 8192, 'expanded_upper_bytes': 32768,
             'peak_bytes': 5 * 1024**2, 'validation_working_bytes': 4 * 1024**2,
             'container_stride_bytes': 4096},
    'shared_reserved_peak_excluding_running_bytes': 10 * 1024**2,
    'scope': 'synthetic checker fixture excluding full Save/Load/Running assembly',
    **{phase: [0.5, 1.25] for phase in module.PHASES},
}
module.check_metrics(valid, 'authored', 2)
cases = [
    ('missing sample', lambda data: data['preflight_us'].pop()),
    ('NaN sample', lambda data: data['export_us'].__setitem__(0, float('nan'))),
    ('boolean sample', lambda data: data['encode_us'].__setitem__(0, True)),
    ('negative sample', lambda data: data['drop_us'].__setitem__(0, -1)),
    ('simulation advanced', lambda data: data['readonly'].__setitem__('event_sequence_after', 2)),
    ('wrong placement', lambda data: data['placement'].__setitem__('unplaced', 1)),
    ('missing speech', lambda data: data['readable_counts'].__setitem__('unread_speech', 0)),
    ('missing collision world', lambda data: data['counts'].__setitem__('collision_boxes', 0)),
    ('row count mismatch', lambda data: data['counts'].__setitem__('rows', 10)),
    ('shared budget exceeded', lambda data: data.__setitem__('shared_reserved_peak_excluding_running_bytes', 1024**3 + 1)),
    ('unsupported record stride', lambda data: data['cost'].__setitem__('container_stride_bytes', 1)),
]
refused = []
for name, mutate in cases:
    changed = deepcopy(valid)
    mutate(changed)
    try:
        module.check_metrics(changed, 'authored', 2)
    except AssertionError:
        refused.append(name)
    else:
        raise AssertionError(f'auditor accepted {name}')
for name, encoded in [('duplicate JSON', '{"a":1,"a":2}'), ('JSON NaN', '{"a":NaN}')]:
    try:
        module.read_json(encoded)
    except AssertionError:
        refused.append(name)
    else:
        raise AssertionError(f'auditor accepted {name}')
distribution = module.distribution([0.5, 2000, 2000.1, 30000, 30000.1])
assert distribution['over_2ms'] == 3 and distribution['over_30ms'] == 1
assert distribution['p99_ms'] == distribution['max_ms'] == 30.0001
assert len(sys.argv) <= 2
report = OUT / (sys.argv[1] if len(sys.argv) == 2 else 'auditor-negative-checks.json')
assert not report.exists()
write_json(report, {'scope': 'synthetic auditor checks only; no host execution',
                   'checker_sha256': sha(Path(__file__)), 'auditor_sha256': sha(path),
                   'result': 'passed', 'refused_corruptions': refused,
                   'tail_threshold_and_nearest_rank_check': 'passed'})
print(json.dumps({'result': 'passed', 'refused_corruptions': len(refused)}))
