"""Read final screenshots and their authoritative sidecars; never launch a game."""
import collections
import argparse
import json
import math
import re
import struct
from pathlib import Path

from inspect_gpu_timing import inspect

directory = Path(__file__).resolve().parent
fixtures = json.loads((directory / "probe_fixtures.json").read_text())
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("populations", type=int, nargs="+")
parser.add_argument("--prefix", default="final")
parser.add_argument("--start-hour", type=float, default=7.0)
args = parser.parse_args()
for population in args.populations:
    meta = json.loads((directory / f"{args.prefix}_{population}_views.json").read_text())
    session = Path(meta["session"])
    timing = inspect(session)
    selected_for_shot = {}
    selected = None
    for line in (session / "logs.jsonl").read_text().splitlines():
        event = json.loads(line)
        match = re.match(r"\[drive\] [0-9.]+s (.*)", event["message"])
        if event["source"] != "drive" or not match:
            continue
        action = match[1]
        if action.startswith("tp "):
            selected = None
        elif action.startswith("framed actor "):
            selected = action.removeprefix("framed actor ")
        elif action.startswith("shot "):
            selected_for_shot[action.removeprefix("shot ")] = selected
    records = []
    for path in sorted((session / "screenshots").glob(f"{args.prefix}_*.json")):
        data = json.loads(path.read_text())
        clock = data["clock"]
        rows = data["residents"]
        assert data["generated_present"] == data["generated_total"] == population
        assert clock["scale"] == 1 and clock["seconds_per_day"] == 3600
        assert len(rows) == population and len({r["id"] for r in rows}) == population
        png = path.with_suffix(".png")
        assert png.is_file()
        with png.open("rb") as file:
            header = file.read(24)
        assert header[:8] == b"\x89PNG\r\n\x1a\n"
        phases = collections.Counter(r["resident"]["phase"] for r in rows)
        minutes = int(clock["fraction"] * 1440 + .5) % 1440
        record = {
            "screenshot": str(png), "sidecar": str(path),
            "dimensions": list(struct.unpack(">II", header[16:24])),
            "clock": f"Day {clock['day']} {minutes//60:02}:{minutes%60:02} {clock['office']} 1x",
            "elapsed_simulation_seconds_from_clock": round((clock["day"] - 2 + clock["fraction"] - args.start_hour/24) * 3600, 3),
            "virtual_seconds": data["virtual_seconds"], "wall_seconds": data["wall_seconds"],
            "weather": data["weather"]["kind"], "generated_present": population,
            "phases": dict(phases), "optional_claims": sum(r["resident"]["optional_walk"] for r in rows),
            "positive_speed": sum(r["speed"] > 1e-7 for r in rows),
            "pending_path": sum(r["pending_waypoints"] > 0 for r in rows),
            "sheltered": sum(r["resident"]["sheltered"] for r in rows),
            "scene_residents": {},
        }
        for view in fixtures["views"]:
            x0, z0, x1, z1 = view["observation_bounds_xz"]
            record["scene_residents"][view["id"]] = [r["id"] for r in rows
                if x0 <= r["position"][0] <= x1 and z0 <= r["position"][2] <= z1]
        selected = selected_for_shot.get(path.stem)
        record["framed_actor"] = next((r for r in rows if r["id"] == selected), None)
        records.append(record)
    timing["authoritative_capture_records"] = sorted(records, key=lambda r: r["wall_seconds"])
    previous = {}
    for record in timing["authoritative_capture_records"]:
        actor = record["framed_actor"]
        if actor is None:
            continue
        prior = previous.get(actor["id"])
        if prior:
            record["framed_displacement_from_previous_observation"] = {
                "previous_screenshot": prior["screenshot"],
                "metres": math.dist(prior["framed_actor"]["position"], actor["position"]),
                "virtual_seconds": record["virtual_seconds"] - prior["virtual_seconds"],
                "scope": "Endpoint displacement between two authoritative observations; not an integrated path length or proof of continuous movement.",
            }
        previous[actor["id"]] = record
    timing["capture_timing_note"] = "Sibling state is read at screenshot request; the asynchronous PNG follows. HUD values are visually checked. Speed and pending path counts are not displacement measurements."
    timing["initial_clock_hour"] = args.start_hour
    (directory / f"{args.prefix}_{population}_gpu_summary.json").write_text(json.dumps(timing, indent=2) + "\n")
    print(population, len(records), "complete capture records", len(timing["minute_intervals"]), "frame intervals")
