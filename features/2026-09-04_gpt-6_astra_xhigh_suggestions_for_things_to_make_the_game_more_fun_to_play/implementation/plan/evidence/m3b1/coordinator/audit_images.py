"""Check read-only final ELF layout provenance and the fixed control arithmetic."""
import hashlib
import json
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
OWNER = HERE.parent / "owner"


def read(path):
    return json.loads(path.read_bytes())


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


workspace, layout_run = sys.argv[1:3]
accepted = read(OWNER / f"{workspace}-result.json")
layout_result = read(OWNER / f"{layout_run}-result.json")
layout_start = read(OWNER / f"{layout_run}-start.json")
metadata = read(OWNER / "final-images.json")
assert accepted["exit_code"] == layout_result["exit_code"] == 0
assert accepted["sources_unchanged"] and layout_result["sources_unchanged"]
assert accepted["source_map_sha256"] == layout_result["source_map_sha256"] == metadata["source_map_sha256"]
assert sha(OWNER / "layout.py") == metadata["layout_helper_sha256"]
raw_path = Path(layout_result["raw_log"])
assert sha(raw_path) == layout_result["raw_sha256"]
raw = raw_path.read_text()
image = Path(metadata["path"])
assert sha(image) == metadata["sha256"]
assert image.stat().st_size == metadata["bytes"]
assert metadata["target_executed"] is False
assert metadata["binary_copy_preserved"] is False
query = metadata["query_command"]
assert query[:4] == ["/usr/bin/gdb", "-nx", "-nh", "--batch"]
assert query[4] == str(image)
assert query[5:7] == ["-ex", "set language rust"]
names = ["Core", "Queue", "PrepState", "PreparedDelivery", "DeliveryDisposal", "RetiredPayload", "Job", "PreparationPermit", "RetirementPermit"]
expected = sum((["-ex", f"p sizeof(cathedral_backends::checkpoint_preparation::{name})"] for name in names), [])
assert query[7:] == expected
sizes = [int(value) for value in re.findall(r"^\$\d+ = (\d+)$", raw, re.M)]
assert dict(zip(names, sizes, strict=True)) == metadata["layout"]
assert any(Path(arg).resolve() == OWNER / "layout.py" for arg in layout_start["command"])
assert workspace in layout_start["command"]
predecessor = metadata["preserved_predecessor"]
assert sha(Path(predecessor["path"])) == predecessor["sha256"] == "52f7f4f408a5286194b509d477bf324d9ec44571c8bfdd2bf9dbeaa193bf2246"
assert Path(predecessor["path"]).stat().st_size == predecessor["bytes"] == 26730496
layout = metadata["layout"]
largest_extra = max(layout[name] for name in ["PrepState", "PreparedDelivery", "DeliveryDisposal", "RetiredPayload"])
roots = layout["Core"] + 4 * largest_extra + layout["PreparationPermit"] + layout["RetirementPermit"] + layout["Job"]
assert roots <= 64 * 1024
assert (64 + 64 + 32 + 1888) * 1024 == 2 * 1024 * 1024
print(json.dumps({
    "auditor_sha256": sha(Path(__file__)),
    "workspace": workspace, "layout_run": layout_run,
    "source_map_sha256": accepted["source_map_sha256"],
    "verified_metadata": metadata,
    "control_fixed_roots_bound_bytes": roots,
    "control_fixed_roots_allowance_bytes": 64 * 1024,
    "service_nonstack_allowance_bytes": 2 * 1024 * 1024,
    "trusted_native_thread_tls_allocator_allowance_bytes": 1888 * 1024,
    "scope": "DWARF fixed layouts and arithmetic only; native overhead is a trusted assumption, not a whole-heap census. Final test ELF was hashed in place, with no retained copy.",
}, sort_keys=True, indent=2))
