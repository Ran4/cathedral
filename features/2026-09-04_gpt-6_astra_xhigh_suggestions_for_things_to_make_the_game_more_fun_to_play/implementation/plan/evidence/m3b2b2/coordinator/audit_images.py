"""Audit final native layouts and preparation control allowance."""
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
assert sha(OWNER / "layout.py") == metadata["helper_sha256"]
assert metadata["binary_copies_preserved"] is False
raw_path = Path(layout_result["raw_log"])
assert sha(raw_path) == layout_result["raw_sha256"]
raw = raw_path.read_text()
workspace_log = Path(accepted["raw_log"])
assert sha(workspace_log) == accepted["raw_sha256"]
workspace_text = workspace_log.read_text()
assert any(Path(arg).resolve() == OWNER / "layout.py" for arg in layout_start["command"])
assert workspace in layout_start["command"]
expected_names = {
    "cathedral_sim": [
        "prompt_archive::PromptExchange", "prompt_archive::PromptExchangeData",
        "prompt_archive::PromptArchivePermit",
        "checkpoint::budget::Usage", "checkpoint::budget::Group",
        "checkpoint::budget::Reservation", "checkpoint::budget::PromotionPermit",
        "checkpoint::budget::RetirementOwner", "checkpoint::budget::RetirementRelease",
    ],
    "cathedral_backends": [
        "runtime::BackendRuntime", "runtime::BackendExecutor",
        "dns::DnsPool", "dns::NativeResolver", "dns::DnsPermit", "dns::DnsWork", "dns::Answer",
        "transcription::SttEngine", "transcription::Job", "transcription::Discard",
        "tts::TtsEngine", "tts::Job",
        "worker::Worker", "worker::Children", "worker::NativeChild", "worker::WorkerIo",
        "stt_realtime::SessionTask", "stt_realtime::RetainedClose",
        "prompt_log::Core", "prompt_log::PayloadCharge", "prompt_log::Receipt",
        "prompt_log::Order", "prompt_log::Progress", "prompt_log::Session",
        "prompt_log::WriteJob", "prompt_log::WriterOwner",
        "prompt_log::ArchiveWriter", "prompt_log::PromptLog",
        "checkpoint_preparation::Core", "checkpoint_preparation::Queue",
        "checkpoint_preparation::PrepState", "checkpoint_preparation::PreparedDelivery",
        "checkpoint_preparation::DeliveryDisposal", "checkpoint_preparation::RetiredPayload",
        "checkpoint_preparation::PreparationPermit", "checkpoint_preparation::RetirementPermit",
        "checkpoint_preparation::Job",
    ],
}
observed_sizes = [int(v) for v in re.findall(r"^\$\d+ = (\d+)$", raw, re.M)]
queried_sizes = []
for crate, names in expected_names.items():
    image = metadata["images"][crate]
    path = Path(image["path"])
    assert sha(path) == image["sha256"]
    assert path.stat().st_size == image["bytes"]
    assert image["target_executed"] is False
    assert re.search(r"Running unittests src/lib\.rs \(" + re.escape(str(path.relative_to(Path.cwd()))) + r"\)", workspace_text)
    query = image["command"]
    assert query[:7] == ["/usr/bin/gdb", "-nx", "-nh", "--batch", str(path), "-ex", "set language rust"]
    pairs = list(zip(query[7::2], query[8::2], strict=True))
    assert len(pairs) == len(names)
    assert {expr for option, expr in pairs if option == "-ex"} == {
        f"p sizeof({crate}::{name})" for name in names
    }
    for _, expr in pairs:
        name = expr.removeprefix(f"p sizeof({crate}::").removesuffix(")")
        queried_sizes.append(image["layout"][name])
assert queried_sizes == observed_sizes
s = metadata["images"]["cathedral_sim"]["layout"]
b = metadata["images"]["cathedral_backends"]["layout"]
largest = max(b["checkpoint_preparation::" + name] for name in
              ["PrepState", "PreparedDelivery", "DeliveryDisposal", "RetiredPayload"])
fixed = (b["checkpoint_preparation::Core"] + 4 * largest
         + b["checkpoint_preparation::PreparationPermit"]
         + b["checkpoint_preparation::RetirementPermit"] + b["checkpoint_preparation::Job"]
         + s["checkpoint::budget::Usage"] + 2 * s["checkpoint::budget::RetirementOwner"]
         + 4 * s["checkpoint::budget::RetirementRelease"]
         + 3 * s["checkpoint::budget::PromotionPermit"] + 1024)
assert fixed == metadata["fixed_control_upper_bytes"] < 64 * 1024
assert metadata["fixed_control_allowance_bytes"] == 64 * 1024
print(json.dumps({
    "auditor_sha256": sha(Path(__file__)),
    "workspace": workspace, "layout_run": layout_run,
    "source_map_sha256": accepted["source_map_sha256"],
    "verified_metadata": metadata,
    "fixed_control_upper_bytes": fixed,
    "scope": "Fixed DWARF native/archive layouts and preparation-control arithmetic. Configured runtime and speech native stacks are distinct from trusted TLS/allocator/native scratch allowances and the pending complete application allocation inventory. Both test ELFs were hashed in place, with no separately preserved copies.",
}, sort_keys=True, indent=2))
