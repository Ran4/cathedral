"""Run final full-engine acceptance serially, with no competing build/GPU run.

uv run --no-project features/implemented/ambient_residents_evidence/run_final.py
"""
import datetime
import hashlib
import json
import subprocess
import time
from pathlib import Path

directory = Path(__file__).resolve().parent
root = next(p for p in directory.parents if (p / "Cargo.toml").exists())
binary = root / "target/debug/cathedral-headless"
metadata = {
    "binary": str(binary),
    "sha256": {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in [binary, root / "assets/world/navigation.json", root / "assets/world/navigation.bin", root / "config.ron"]},
    "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
    "isolation": "Populations run serially after all checks/builds, before GPU verification; no other agent builds or benchmarks.",
    "runs": [],
}
for population, days, prefix in [(1000, "2", "final"), (2000, "2", "final"), (20000, "0.03333333333333333", "stress")]:
    command = [str(binary), "--fake", "--stage", "--curiosity", "--night-office", "--weather", "clear",
               "--extra-ambient", str(population), "--start-day", "2", "--start-office", "dayspring",
               "--seconds-per-day", "3600", "--watch-clock", days, "--trace-motion", "--census-per-day", "120"]
    record = {"requested": population, "command": command,
              "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    metadata["runs"].append(record)
    record_path = directory / "final_meta.json"
    record_path.write_text(json.dumps(metadata, indent=2) + "\n")
    print(population, "started", record["started_utc"], flush=True)
    started = time.monotonic()
    with (directory / f"{prefix}_{population}.log").open("w") as stdout, (directory / f"{prefix}_{population}.err").open("w") as stderr:
        result = subprocess.run(command, cwd=root, stdout=stdout, stderr=stderr)
    record.update(returncode=result.returncode, wall_seconds=time.monotonic() - started,
                  finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    record_path.write_text(json.dumps(metadata, indent=2) + "\n")
    assert result.returncode == 0
    print(population, "complete", record["wall_seconds"], flush=True)
    if prefix == "final":
        subprocess.run(["uv", "run", "--no-project", str(directory / "summarize_final.py"),
                        str(directory / f"final_{population}.log")], cwd=root, check=True)
