"""Guarded hidden GPU views at approximate normal-speed simulation ages.

Run via uv from a new detached host tmux session. The default planning ratio
of 10 covers the observed 1 Hz driver state and 100 ms virtual-time clamp;
normal-throughput runs instead approach a ratio of 1. The state can change
during a run. Screenshot HUD times, not requested ages or a fixed planning
ratio, establish the actual photographed moment.
This helper never changes the clock scale or modifies game configuration.
"""
import argparse
import hashlib
import json
import os
import signal
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--binary", default="target/debug/cathedralbevy")
parser.add_argument("--prefix", required=True)
parser.add_argument("--populations", type=int, nargs="+", default=[1000, 2000])
parser.add_argument("--ages", type=int, nargs="+", default=[30, 180, 600])
parser.add_argument("--wall-per-sim", type=float, default=10.0)
args = parser.parse_args()
assert args.prefix.replace("_", "").isalnum()
assert args.ages == sorted(set(args.ages)) and min(args.ages) > 0
assert 0 < args.wall_per_sim <= 10

root = Path.cwd()
directory = Path(__file__).resolve().parent
binary = Path(args.binary).resolve()
binary_hash = hashlib.file_digest(binary.open("rb"), "sha256").hexdigest()
fixtures = json.loads((directory / "probe_fixtures.json").read_text())
actions = ["wait-online", "weather clear"]
previous = 0
for age in args.ages:
    actions.append(f"sleep {(age - previous) * args.wall_per_sim}")
    for view in fixtures["views"]:
        for camera in ["ground", "elevated"]:
            pose = " ".join(map(str, view[f"{camera}_tp"]))
            actions += [f"tp {pose}", f"shot {args.prefix}_{view['id']}_{camera}_approx{age}"]
    previous = age
actions.append("quit")
drive = "; ".join(actions)
timeout = int(max(args.ages) * args.wall_per_sim + len(actions) * 4 + 90)

for count in args.populations:
    assert 0 <= count <= 20000
    environment = os.environ.copy()
    environment.update(
        CATHEDRAL_HEADLESS="1", CATHEDRAL_FAKE_BACKEND="1",
        CATHEDRAL_EXTRA_NPCS=str(count), CATHEDRAL_PERF="1",
        CATHEDRAL_DRIVE_RES="1280x720", CATHEDRAL_DRIVE=drive,
        CATHEDRAL_DRIVE_TIMEOUT=str(timeout), WAYLAND_DISPLAY="",
        BEVY_ASSET_ROOT=str(root),
    )
    environment.pop("CATHEDRAL_HEADLESS_AUDIO", None)
    checks = []
    log_path = Path(f"/tmp/{args.prefix}-{count}-views.log")
    nav_hash = hashlib.sha256((root / "assets/world/navigation.json").read_bytes()).hexdigest()
    record_path = directory / f"{args.prefix}_{count}_views.json"
    record = {
        "population": count, "binary": str(binary), "binary_sha256": binary_hash,
        "navigation_sha256": nav_hash, "drive": drive,
        "requested_approx_simulation_ages": args.ages,
        "wall_per_sim_estimate": args.wall_per_sim,
        "clock": "config.ron, 3600s/day, day2 Dayspring, scale1; no scale changes",
        "checks": checks,
    }
    with log_path.open("w") as log:
        process = subprocess.Popen([str(binary)], env=environment, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        started = time.monotonic()
        session = None
        try:
            while process.poll() is None:
                elapsed = time.monotonic() - started
                if elapsed > timeout + 15:
                    raise TimeoutError("Hidden screenshot run exceeded watchdog")
                logged = log_path.read_text()
                if "device_type: Cpu" in logged or "Path not found:" in logged:
                    raise RuntimeError("Renderer or asset validation failed")
                if session is None and "AdapterInfo" in logged:
                    session = (root / "logs/latest_session").resolve()
                    record["session"] = str(session)
                    print(count, "started", session, flush=True)
                windows = subprocess.run(["xdotool", "search", "--pid", str(process.pid)], capture_output=True, text=True).stdout.split()
                focus = subprocess.run(["xdotool", "getwindowfocus"], capture_output=True, text=True).stdout.strip()
                for window in windows:
                    info = subprocess.run(["xwininfo", "-id", window], capture_output=True, text=True)
                    if info.returncode:
                        continue
                    check = {"elapsed_s": round(elapsed, 2), "window": window,
                             "unmapped": "Map State: IsUnMapped" in info.stdout,
                             "focused": focus == window}
                    checks.append(check)
                    if not check["unmapped"] or check["focused"]:
                        raise RuntimeError("Hidden-window guarantee failed")
                record_path.write_text(json.dumps(record, indent=2) + "\n")
                time.sleep(1)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
            record["returncode"] = process.returncode
            record["wall_seconds"] = time.monotonic() - started
            record_path.write_text(json.dumps(record, indent=2) + "\n")
    assert process.returncode == 0 and checks and session is not None
    expected = len(fixtures["views"]) * 2 * len(args.ages)
    screenshots = sorted((session / "screenshots").glob(f"{args.prefix}_*.png"))
    assert len(screenshots) == expected, (len(screenshots), expected)
    record["screenshots"] = [str(path) for path in screenshots]
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    print(count, "complete", len(screenshots), "screenshots", len(checks), "unmapped checks", flush=True)
