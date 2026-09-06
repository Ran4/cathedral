"""Run serial hidden-window frame baselines. uv run --no-project this_file.py

Run from the repository root, with no other benchmark/build running. The
existing compiled binary is intentionally used: record its hash before M1
changes shared geometry. This never toggles debug time scale.
"""

import hashlib
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

directory = Path(__file__).resolve().parent
binary = Path("target/debug/cathedralbevy").resolve()
binary_hash = hashlib.file_digest(binary.open("rb"), "sha256").hexdigest()
drive = (
    "wait-online; weather clear; tp -17.375 1.7 268.375 0 0; sleep 30; "
    "shot ambient_m0_wick_ground_30; tp -17.375 26 285.375 0 -35; "
    "shot ambient_m0_wick_above_32; tp -17.375 1.7 268.375 0 0; sleep 40; "
    "shot ambient_m0_wick_ground_75; quit"
)
for count in [1000, 2000]:
    environment = os.environ.copy()
    environment.update(
        CATHEDRAL_HEADLESS="1", CATHEDRAL_FAKE_BACKEND="1", CATHEDRAL_EXTRA_NPCS=str(count),
        CATHEDRAL_PERF="1", CATHEDRAL_DRIVE_RES="1280x720", CATHEDRAL_DRIVE=drive,
        CATHEDRAL_DRIVE_TIMEOUT="120", WAYLAND_DISPLAY="",
        BEVY_ASSET_ROOT=str(Path.cwd()),
    )
    environment.pop("CATHEDRAL_HEADLESS_AUDIO", None)
    checks = []
    with Path(f"/tmp/ambient-m0-bevy-{count}.log").open("w") as log:
        process = subprocess.Popen([str(binary)], env=environment, stdout=log, stderr=subprocess.STDOUT)
        started = time.monotonic()
        try:
            while process.poll() is None:
                if time.monotonic() - started > 130:
                    raise TimeoutError("Hidden Bevy baseline exceeded watchdog")
                # Software-rendered frames cannot serve as the GPU baseline.
                # Abort early if the environment cannot expose the real card.
                log.flush()
                if "device_type: Cpu" in Path(f"/tmp/ambient-m0-bevy-{count}.log").read_text():
                    raise RuntimeError("Real GPU unavailable: software renderer selected")
                windows = subprocess.run(["xdotool", "search", "--pid", str(process.pid)], capture_output=True, text=True).stdout.split()
                focus = subprocess.run(["xdotool", "getwindowfocus"], capture_output=True, text=True).stdout.strip()
                for window in windows:
                    info = subprocess.run(["xwininfo", "-id", window], capture_output=True, text=True)
                    if info.returncode:
                        continue  # The final exit can destroy it between the two reads.
                    unmapped = "Map State: IsUnMapped" in info.stdout
                    focused = focus == window
                    checks.append({"elapsed_s": round(time.monotonic() - started, 2), "window": window, "unmapped": unmapped, "focused": focused})
                    if not unmapped or focused:
                        raise RuntimeError("Hidden-window guarantee failed")
                time.sleep(0.5)
        finally:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=10)
    assert process.returncode == 0, process.returncode
    assert checks, "No window found: cannot claim hidden-window verification"
    assert "Path not found:" not in Path(f"/tmp/ambient-m0-bevy-{count}.log").read_text(), "Missing assets invalidate a frame baseline"
    session = Path("logs/latest_session").resolve()
    shutil.copyfile(session / "perf_summary.json", directory / f"baseline_{count}_frames.json")
    record = {
        "population": count, "session": str(session), "binary_sha256": binary_hash,
        "binary": str(binary), "drive": drive,
        "environment": {key: environment[key] for key in ["CATHEDRAL_HEADLESS", "CATHEDRAL_FAKE_BACKEND", "CATHEDRAL_EXTRA_NPCS", "CATHEDRAL_PERF", "CATHEDRAL_DRIVE_RES", "BEVY_ASSET_ROOT"]},
        "clock": "config.ron 3600s/day, day 2 Dayspring, scale 1; no T key",
        "checks": checks,
    }
    (directory / f"baseline_{count}_window_checks.json").write_text(json.dumps(record, indent=2) + "\n")
    print(count, session, "unmapped checks", len(checks), flush=True)
