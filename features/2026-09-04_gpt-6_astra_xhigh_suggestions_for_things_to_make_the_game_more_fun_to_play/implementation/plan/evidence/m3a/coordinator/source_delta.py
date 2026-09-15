"""Verify the current freeze and enumerate its exact accepted-M2d source delta."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parents[1]
spec = importlib.util.spec_from_file_location("sources", EVIDENCE / "component_input_sources.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
freeze_path = Path(sys.argv[1])
freeze_raw = freeze_path.read_bytes()
frozen = json.loads(freeze_raw)
prior_raw = (EVIDENCE / "m2d/source_hashes.json").read_bytes()
assert hashlib.sha256(prior_raw).hexdigest() == "28a535a0b232d887c0e2653a308ee0c92eb62c9b04240b7aaa587886c2815188"
prior = json.loads(prior_raw)
assert module.sources() == frozen, "current source no longer matches the chosen freeze"
added = sorted(frozen.keys() - prior.keys())
removed = sorted(prior.keys() - frozen.keys())
changed = sorted(path for path in prior.keys() & frozen.keys() if prior[path] != frozen[path])
print(json.dumps({
    "source_map_sha256": hashlib.sha256(freeze_raw).hexdigest(),
    "prior_count": len(prior), "current_count": len(frozen),
    "added": {path: frozen[path] for path in added},
    "removed": removed,
    "changed": {path: {"before": prior[path], "after": frozen[path]} for path in changed},
    "unchanged_predecessor_inputs": len(prior) - len(removed) - len(changed),
}, sort_keys=True, indent=2))
assert not removed
