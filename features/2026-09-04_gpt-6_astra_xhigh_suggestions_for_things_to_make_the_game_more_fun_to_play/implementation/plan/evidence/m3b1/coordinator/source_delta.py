"""Verify the M3b1 freeze against accepted M3a without running Cargo."""
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
prior_raw = (EVIDENCE / "m3a/source_hashes.json").read_bytes()
assert hashlib.sha256(prior_raw).hexdigest() == "97936f28124b1c9876b8f1de1c91a7e5e271f00eef982f38ee6126c3a5c52f23"
prior = json.loads(prior_raw)
assert module.sources() == frozen, "current source differs from the selected freeze"
added = sorted(frozen.keys() - prior.keys())
removed = sorted(prior.keys() - frozen.keys())
changed = sorted(path for path in prior.keys() & frozen.keys() if prior[path] != frozen[path])
assert not removed
print(json.dumps({
    "auditor_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "predecessor_commit": "5559d26654faba677971ceeef66f0869495e6489",
    "source_map_sha256": hashlib.sha256(freeze_raw).hexdigest(),
    "prior_count": len(prior), "current_count": len(frozen),
    "added": {path: frozen[path] for path in added},
    "removed": removed,
    "changed": {path: {"before": prior[path], "after": frozen[path]} for path in changed},
    "unchanged_predecessor_inputs": len(prior) - len(removed) - len(changed),
}, sort_keys=True, indent=2))
