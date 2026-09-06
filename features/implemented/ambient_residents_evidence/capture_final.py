"""Guarded final M4 GPU runs. Launch via uv in a unique detached host tmux.

All waits retain normal virtual clock scale. Stage names specify completed
cumulative simulation waits, not the age of every subsequent camera; sibling
JSON and screenshot HUD record each actual observed time.
"""
import argparse
import hashlib
import json
import os
import signal
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--populations", type=int, nargs="+", default=[1000, 2000])
args = parser.parse_args()
directory = Path(__file__).resolve().parent
root = next(p for p in directory.parents if (p / "Cargo.toml").exists())
binary = root / "target/debug/cathedralbevy"
fixtures = json.loads((directory / "probe_fixtures.json").read_text())
views = [(f"{v['id']}_{camera}", v[f"{camera}_tp"])
         for v in fixtures["views"] for camera in ["ground", "elevated"]]
views.append(("wickmarket_frontages", [-17.375, 1.7, 268.375, 180, 0]))
views.append(("wickmarket_east_frontage", [9.125, 1.7, 269.125, 180, 0]))


def captures(actions, label):
    for name, pose in views:
        actions.extend(["tp " + " ".join(map(str, pose)), f"shot final_{name}_{label}"])


actions = ["key KeyV", "wait-online", "weather clear", "sleep-sim 30"]
captures(actions, "stage30")
actions.extend(["tp -113.375 22 183.125 0 -45", "sleep-sim 150"])
captures(actions, "stage180")
actions.extend(["tp -113.375 22 183.125 0 -45", "sleep-sim 420"])
captures(actions, "stage600")
actions.extend([
    "frame @resident-lingering 4", "shot final_resident_lingering",
    "key KeyB", "shot final_resident_lingering_debug", "key KeyB",
    "frame @resident-moving 4", "shot final_resident_moving",
    "key KeyB", "shot final_resident_moving_debug", "sleep-sim 15",
    "shot final_resident_arrival_debug", "key KeyB", "frame @last 4",
    "shot final_resident_arrival", "sleep-sim 60", "frame @last 4",
    "shot final_resident_later", "key KeyB", "shot final_resident_later_debug", "key KeyB",
    "tp -17.375 1.7 268.375 180 0", "weather rain 0.9", "sleep-sim 60",
])
captures(actions, "rain60")
actions.extend([
    "frame @resident-sheltered 4", "shot final_sheltered", "key KeyB",
    "shot final_sheltered_debug", "key KeyB", "sleep-sim 60",
    "frame @last 4", "shot final_sheltered_later", "weather clear",
    "tp -17.375 1.7 268.375 0 0", "sleep-sim 750",
])
captures(actions, "evening")
actions.append("quit")
drive = "; ".join(actions)
timeout = 22000

for population in args.populations:
    assert population in [1000, 2000]
    environment = os.environ.copy()
    environment.update(
        CATHEDRAL_HEADLESS="1", CATHEDRAL_FAKE_BACKEND="1",
        CATHEDRAL_EXTRA_NPCS=str(population), CATHEDRAL_PERF="1",
        CATHEDRAL_DRIVE_RES="1280x720", CATHEDRAL_DRIVE=drive,
        CATHEDRAL_DRIVE_RESIDENT_EVIDENCE="1", CATHEDRAL_DRIVE_TIMEOUT=str(timeout),
        BEVY_ASSET_ROOT=str(root), WAYLAND_DISPLAY="",
    )
    for key in ["CATHEDRAL_HEADLESS_AUDIO", "CATHEDRAL_NO_ACTORS", "CATHEDRAL_NO_WEATHER", "CATHEDRAL_BODY_LINEUP"]:
        environment.pop(key, None)
    record_path = directory / f"final_{population}_views.json"
    log_path = Path(f"/tmp/cathedral-m4-gpu-{population}.log")
    record = {
        "population": population, "binary": str(binary), "drive": drive,
        "environment": {k: v for k, v in environment.items() if k.startswith("CATHEDRAL_") or k in ["BEVY_ASSET_ROOT", "WAYLAND_DISPLAY"]},
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "navigation_sha256": hashlib.sha256((root / "assets/world/navigation.json").read_bytes()).hexdigest(),
        "config_sha256": hashlib.sha256((root / "config.ron").read_bytes()).hexdigest(),
        "checks": [], "log": str(log_path),
        "stage_timing": "Cumulative virtual waits 30/180/600, plus action/capture overhead. Actual times are in each sibling JSON and HUD. Clock scale unchanged at1x.",
    }
    with log_path.open("w") as log:
        process = subprocess.Popen([str(binary)], cwd=root, env=environment,
                                   stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        started = time.monotonic()
        session = None
        try:
            while process.poll() is None:
                elapsed = time.monotonic() - started
                if elapsed > timeout + 15:
                    raise TimeoutError("Hidden screenshot watchdog expired")
                logged = log_path.read_text()
                if "device_type: Cpu" in logged or "Path not found:" in logged:
                    raise RuntimeError("Real GPU / asset validation failed")
                if session is None and "AdapterInfo" in logged:
                    assert "NVIDIA GeForce RTX 4070" in logged
                    session = (root / "logs/latest_session").resolve()
                    record["session"] = str(session)
                    print(population, "started", session, flush=True)
                windows = subprocess.run(["xdotool", "search", "--pid", str(process.pid)], capture_output=True, text=True).stdout.split()
                focus = subprocess.run(["xdotool", "getwindowfocus"], capture_output=True, text=True).stdout.strip()
                for window in windows:
                    info = subprocess.run(["xwininfo", "-id", window], capture_output=True, text=True)
                    if info.returncode:
                        continue
                    check = {"elapsed_s": round(elapsed, 2), "window": window,
                             "unmapped": "Map State: IsUnMapped" in info.stdout, "focused": focus == window}
                    record["checks"].append(check)
                    if not check["unmapped"] or check["focused"]:
                        raise RuntimeError("Hidden window guarantee failed")
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
    assert process.returncode == 0 and record["checks"] and session is not None
    screenshots = sorted((session / "screenshots").glob("final_*.png"))
    expected = sum(action.startswith("shot ") for action in actions)
    assert len(screenshots) == expected, (len(screenshots), expected)
    for shot in screenshots:
        evidence = json.loads(shot.with_suffix(".json").read_text())
        assert evidence["generated_present"] == population == evidence["generated_total"]
        assert evidence["clock"]["scale"] == 1 and evidence["clock"]["seconds_per_day"] == 3600
    record["screenshots"] = [str(path) for path in screenshots]
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    print(population, "complete", len(screenshots), "captures", len(record["checks"]), "unmapped checks", flush=True)
