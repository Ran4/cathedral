"""Independently verify retained M3a release bytes and bounded API samples."""
import hashlib
import json
import math
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
OWNER = HERE.parent / "owner"
mode = sys.argv[1]
assert mode in {"pre-fix", "final"}
suffix = "-pre-fix" if mode == "pre-fix" else ""
retention_path = OWNER / f"release-retention{suffix}.json"
retention = json.loads(retention_path.read_bytes())
writer_path = OWNER / f"release-writer-{mode}-report.json"
writer = json.loads(writer_path.read_bytes())


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def check_report(report):
    assert report["schema"] == 1
    assert bytes(report["host_image_sha256"]).hex() == retention["image_sha256"]
    assert report["service_allowance_bytes"] == 3 * 1024 * 1024
    assert report["configured_worker_stack_bytes"] == 2 * 1024 * 1024
    assert report["retained_after_shutdown"] == 512 * 1024 * 1024
    assert report["shared_peak_bytes"] <= 1024 * 1024 * 1024
    assert report["complete_m2_validation"] is True
    assert report["capture_cost_and_real_host_frames_excluded"] is True
    for phase, samples in report["microseconds"].items():
        values = samples if isinstance(samples, list) else [samples]
        assert all(isinstance(n, (int, float)) and math.isfinite(n) and n >= 0 for n in values)


assert retention["mode"] == mode
assert digest(writer_path) == retention["report_sha256"]
assert Path(retention["original_report"]).read_bytes() == writer_path.read_bytes()
image = Path(retention["preserved_image"])
assert image.stat().st_size == retention["image_bytes"]
assert digest(image) == retention["image_sha256"]
assert writer["executable"] == retention["original_image"]
assert writer["mode"] == "writer" and writer["samples"] == 32
check_report(writer)

fixture = OWNER / f"fixture-release{suffix}"
files = retention["fixture_files"]
assert {p.name for p in fixture.iterdir()} == set(files)
for name, expected in files.items():
    path = fixture / name
    assert path.is_file() and not path.is_symlink()
    assert path.stat().st_size == expected["bytes"]
    assert digest(path) == expected["sha256"]
    assert digest(Path(expected["original"])) == expected["sha256"]

refs = {}
payload_names = set()
for kind, sequence in [("active", 32), ("previous", 31)]:
    raw = (fixture / f"slot-manual-1.{kind}").read_text()
    assert raw.startswith('{"body":')
    body, end = json.JSONDecoder().raw_decode(raw, len('{"body":'))
    record = json.loads(raw)
    assert set(record) == {"body", "sha256"}
    assert hashlib.sha256(raw[len('{"body":'):end].encode()).digest() == bytes(record["sha256"])
    assert body["format_version"] == 1 and body["slot"] == "manual-1"
    assert body["generation"]["sequence"] == sequence
    assert body["metadata"] == {"title": "A", "captured_unix_seconds": 1789430400 + sequence - 1,
                                "known_location": "The square"}
    service = bytes(body["generation"]["service"]).hex()
    name = f"slot-manual-1.gen-{service}-{sequence:016x}.json"
    payload_names.add(name)
    payload = (fixture / name).read_bytes()
    assert len(payload) == body["payload_bytes"]
    assert hashlib.sha256(payload).digest() == bytes(body["payload_sha256"])
    envelope = json.loads(payload)
    assert envelope["world_identity"] == body["world_identity"]
    assert envelope["boundary"] == body["boundary"]
    assert bytes(envelope["manifest"]["host_image"]).hex() == retention["image_sha256"]
    refs[kind] = body
assert set(files) == payload_names | {"slot-manual-1.active", "slot-manual-1.previous"}
for key in ["payload_bytes", "payload_sha256", "generation"]:
    assert writer[key] == refs["active"][key]

phases = {}
for phase in ["submit", "attach", "terminal_take", "durable_from_attach"]:
    values = writer["microseconds"][phase]
    assert len(values) == 32
    ordered = sorted(values)
    phases[phase] = {"samples": 32, **{f"p{p}": ordered[math.ceil(32 * p / 100) - 1]
                                      for p in [50, 95, 99]}, "max": ordered[-1]}

reader_audit = None
if mode == "final":
    reader_path = Path(sys.argv[2])
    reader = json.loads(reader_path.read_bytes())
    check_report(reader)
    assert reader["mode"] == "fresh-reader" and reader["samples"] == 0
    assert reader["executable"] == str(image)
    assert reader["directory"] == writer["directory"]
    for key in ["payload_bytes", "payload_sha256", "generation"]:
        assert reader[key] == writer[key]
    assert all(reader["microseconds"][phase] == [] for phase in phases)
    reader_audit = {"path": str(reader_path), "sha256": digest(reader_path),
                    "same_image_and_slot": True, "complete_m2_validation": True}

print(json.dumps({"auditor_sha256": digest(Path(__file__)), "mode": mode,
                  "image": str(image), "image_bytes": retention["image_bytes"],
                  "image_sha256": retention["image_sha256"],
                  "writer_report_sha256": digest(writer_path), "fixture_files": files,
                  "microseconds_nearest_rank": phases,
                  "startup_us": writer["microseconds"]["startup"],
                  "shutdown_signal_us": writer["microseconds"]["shutdown_signal"],
                  "shared_peak_bytes": writer["shared_peak_bytes"], "reader": reader_audit,
                  "scope": "32 serial storage API cycles using a complete demo checkpoint; capture, host frames, populated city, full heap and physical power loss excluded"},
                 sort_keys=True, indent=2))
