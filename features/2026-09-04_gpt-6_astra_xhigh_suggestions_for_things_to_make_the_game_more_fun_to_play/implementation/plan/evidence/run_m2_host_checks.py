# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Serial M2a15 command evidence; original log is retained in /tmp."""
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from component_input_sources import ROOT, SOURCE_SCOPE, sources

label, *command = sys.argv[1:]
if command[:1] == ["--"]:
    command.pop(0)
out = Path(__file__).with_name("m2a15") / "commands"
out.mkdir(exist_ok=True)
if (out / (label + ".start.json")).exists():
    raise RuntimeError("Choose a new evidence label")
raw = Path("/tmp") / ("alibi-m2a15-" + label + ".log")
if raw.exists():
    raise RuntimeError("Choose a new raw log path")
helper_bytes = Path(__file__).read_bytes()
helper_digest = hashlib.sha256(helper_bytes).hexdigest()
helpers = out / "helpers"
helpers.mkdir(exist_ok=True)
(helpers / (helper_digest + ".py")).write_bytes(helper_bytes)
identity = {"started_utc": datetime.now(timezone.utc).isoformat(),
            "command": command, "cwd": str(ROOT), "source_scope": SOURCE_SCOPE,
            "source_sha256": sources(),
            "helper_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "head": subprocess.check_output(["/usr/bin/git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "environment": {k: os.environ.get(k) for k in ("PATH", "CARGO_HOME", "RUSTC", "RUSTDOC", "RUSTFLAGS", "CARGO_TARGET_DIR", "CATHEDRAL_HEADLESS", "CATHEDRAL_FAKE_BACKEND", "LDFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "ALIBI_HOST_MODE", "ALIBI_HOST_SAMPLES", "ALIBI_HOST_OUTPUT")},
            "raw_log": str(raw)}
(out / (label + ".start.json")).write_text(json.dumps(identity, indent=2) + "\n")
with raw.open("wb") as stream:
    result = subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
data = raw.read_bytes()
with (out / (label + ".log.gz")).open("wb") as dest:
    with gzip.GzipFile(filename="", mode="wb", fileobj=dest, mtime=0) as stream:
        stream.write(data)
(out / (label + ".result.json")).write_text(json.dumps({"exit_code": result.returncode, "raw_sha256": hashlib.sha256(data).hexdigest(), "completed_utc": datetime.now(timezone.utc).isoformat(), "source_changed_during_command": identity["source_sha256"] != sources()}, indent=2) + "\n")
print(data.decode(errors="replace")[-18000:])
sys.exit(result.returncode)
