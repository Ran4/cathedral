"""Separate normal-1x Lamplight-start visual fixtures; run only after main GPU runs.

Launch through a uniquely named detached host tmux. This copies config.ron into
a fresh /tmp cwd and changes only start_office. It never edits the user's config
or changes a running clock. The continuous two-day traces prove transitions;
these bounded fixtures inspect evening light, resting bodies and interaction.
"""
import argparse
import hashlib
import json
import os
import signal
import subprocess
import tempfile
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
views.extend([
    ("wickmarket_frontages", [-17.375, 1.7, 268.375, 180, 0]),
    ("wickmarket_east_frontage", [9.125, 1.7, 269.125, 180, 0]),
])
actions = ["key KeyV", "wait-online", "weather clear", "sleep-sim 30"]
for label, wait in [("stage30", 150), ("stage180", None)]:
    for name, pose in views:
        actions.extend(["tp " + " ".join(map(str, pose)), f"shot evening_{name}_{label}"])
    if wait:
        actions.extend(["tp -113.375 22 183.125 0 -45", f"sleep-sim {wait}"])
actions.extend([
    "frame @resident-resting 4", "shot evening_resting", "key KeyB",
    "shot evening_resting_debug", "sleep-sim 30", "shot evening_resting_later_debug",
    "key KeyB", "frame @last 4", "shot evening_resting_later", "quit",
])
drive = "; ".join(actions)
timeout = 6000
original_config = (root / "config.ron").read_bytes()
assert original_config.count(b'start_office: "dayspring"') == 1
fixture_config = original_config.replace(b'start_office: "dayspring"', b'start_office: "lamplight"')
working = Path(tempfile.mkdtemp(prefix="cathedral-m4-evening-"))
(working / "config.ron").write_bytes(fixture_config)
for name in ["default_config.ron", "prompt_playgound", "assets", "lore"]:
    assert (root / name).exists()
    (working / name).symlink_to(root / name, target_is_directory=(root / name).is_dir())
logs = root / "logs" / working.name
logs.mkdir()
(working / "logs").symlink_to(logs, target_is_directory=True)

for population in args.populations:
    assert population in [1000, 2000]
    env = os.environ.copy()
    env.update(CATHEDRAL_HEADLESS="1", CATHEDRAL_FAKE_BACKEND="1",
               CATHEDRAL_EXTRA_NPCS=str(population), CATHEDRAL_PERF="1",
               CATHEDRAL_DRIVE_RES="1280x720", CATHEDRAL_DRIVE=drive,
               CATHEDRAL_DRIVE_RESIDENT_EVIDENCE="1", CATHEDRAL_DRIVE_TIMEOUT=str(timeout),
               BEVY_ASSET_ROOT=str(root), WAYLAND_DISPLAY="")
    for key in ["CATHEDRAL_HEADLESS_AUDIO", "CATHEDRAL_NO_ACTORS", "CATHEDRAL_NO_WEATHER", "CATHEDRAL_BODY_LINEUP"]:
        env.pop(key, None)
    record_path = directory / f"evening_{population}_views.json"
    log_path = directory / f"evening_{population}_gpu.log"
    record = {
        "population": population, "binary": str(binary), "cwd": str(working), "drive": drive,
        "environment": {k: v for k, v in env.items() if k.startswith("CATHEDRAL_") or k in ["BEVY_ASSET_ROOT", "WAYLAND_DISPLAY"]},
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "navigation_sha256": hashlib.sha256((root / "assets/world/navigation.json").read_bytes()).hexdigest(),
        "source_config_sha256": hashlib.sha256(original_config).hexdigest(),
        "fixture_config_sha256": hashlib.sha256(fixture_config).hexdigest(),
        "config_difference": 'Only start_office: "dayspring" -> "lamplight" in a copied config.',
        "scope": "Separate Day 2 Lamplight startup at normal 1x,3600s/day. Not a continuous morning-to-evening GPU run; continuous transitions are covered by formal two-day full-engine traces.",
        "checks": [], "log": str(log_path),
    }
    with log_path.open("w") as log:
        process = subprocess.Popen([str(binary)], cwd=working, env=env,
                                   stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        started = time.monotonic()
        session = None
        try:
            while process.poll() is None:
                elapsed = time.monotonic() - started
                if elapsed > timeout + 15:
                    raise TimeoutError("Hidden evening watchdog expired")
                output = log_path.read_text()
                if "device_type: Cpu" in output or "Path not found:" in output:
                    raise RuntimeError("Real GPU / asset validation failed")
                if session is None and "AdapterInfo" in output:
                    assert "NVIDIA GeForce RTX 4070" in output
                    session = (working / "logs/latest_session").resolve()
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
    screenshots = sorted((session / "screenshots").glob("evening_*.png"))
    assert len(screenshots) == sum(a.startswith("shot ") for a in actions)
    for shot in screenshots:
        data = json.loads(shot.with_suffix(".json").read_text())
        assert data["generated_present"] == population == data["generated_total"]
        assert data["clock"]["scale"] == 1 and data["clock"]["seconds_per_day"] == 3600
        assert data["clock"]["office"] == "Lamplight"
        assert any(r["resident"]["resting_at_household_frontage"] or r["resident"]["resting_without_home"] for r in data["residents"])
    assert (root / "config.ron").read_bytes() == original_config
    record["screenshots"] = [str(p) for p in screenshots]
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    print(population, "complete", len(screenshots), "evening captures", flush=True)
