"""Verify the M3b2b2 freeze against accepted M3b2b1 without running Cargo."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parents[1]
spec = importlib.util.spec_from_file_location("sources", EVIDENCE / "component_input_sources.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
freeze_raw = Path(sys.argv[1]).read_bytes()
frozen = json.loads(freeze_raw)
prior_raw = (EVIDENCE / "m3b2b1/owner/final-workspace-sources.json").read_bytes()
assert hashlib.sha256(prior_raw).hexdigest() == "8f246541e779e999382526d0a730eb95accb60965b1ff37df4b175f54c2ba0ca"
prior = json.loads(prior_raw)
assert module.sources() == frozen, "current source differs from the selected freeze"
added = sorted(frozen.keys() - prior.keys())
removed = sorted(prior.keys() - frozen.keys())
changed = sorted(path for path in prior.keys() & frozen.keys() if prior[path] != frozen[path])
assert not removed
print(json.dumps({
    "auditor_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "predecessor_commit": "b37d8fcff58efcd219766a98fd0e1a83b731df7c",
    "source_map_sha256": hashlib.sha256(freeze_raw).hexdigest(),
    "prior_count": len(prior), "current_count": len(frozen),
    "added": {path: frozen[path] for path in added},
    "removed": removed,
    "changed": {path: {"before": prior[path], "after": frozen[path]} for path in changed},
    "unchanged_predecessor_inputs": len(prior) - len(removed) - len(changed),
}, sort_keys=True, indent=2))
