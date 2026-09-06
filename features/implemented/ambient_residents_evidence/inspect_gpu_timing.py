"""Inspect existing GPU logs; never launch or change the game.

uv run --no-project inspect_gpu_timing.py SESSION... --output REPORT.json
Frame deltas are rounded. Virtual ages are estimates from the existing 100ms
clamp and reported engine-online event, not replacements for screenshot HUDs.
"""
import argparse
import bisect
import json
import re
import statistics
from pathlib import Path


def inspect(session):
    events = []
    for line in (session / "logs.jsonl").read_text().splitlines():
        row = json.loads(line)
        match = re.match(r"\[drive\] ([0-9.]+)s (.*)", row["message"])
        if row["source"] == "drive" and match:
            events.append((float(match[1]), match[2]))
    frames, times, virtual = [], [], []
    elapsed_virtual = 0.0
    for line in (session / "perf_frames.jsonl").read_text().splitlines():
        chunk = json.loads(line)
        t = chunk["t"]
        for index, ms in enumerate(chunk["frames_ms"]):
            if index:
                t += ms / 1000
            elapsed_virtual += min(ms, 100) / 1000
            frames.append((t, ms))
            times.append(t)
            virtual.append(elapsed_virtual)
    assert times and all(a <= b for a, b in zip(times, times[1:]))

    def virtual_at(t):
        index = bisect.bisect_right(times, t) - 1
        return virtual[index] if index >= 0 else 0.0

    online = next(t for t, action in events if action == "online")
    origin = virtual_at(online)
    screenshots = [
        {"name": action.removeprefix("shot "), "drive_wall_s": t,
         "estimated_virtual_seconds_since_online": round(virtual_at(t) - origin, 2)}
        for t, action in events if action.startswith("shot ")
    ]
    intervals = []
    for start in range(60, int(times[-1]) - 59, 60):
        end = start + 60
        values = sorted(ms for t, ms in frames if start <= t < end)
        if not values:
            continue
        camera = next((action for t, action in reversed(events)
                       if t <= start and action.startswith(("tp ", "frame "))), "initial")
        changes = [action for t, action in events if start < t < end
                   and action.startswith(("tp ", "frame "))]
        intervals.append({
            "wall_seconds": [start, end], "camera": camera,
            "camera_changes": changes, "frames": len(values),
            "estimated_virtual_seconds_since_online": [round(virtual_at(t) - origin, 2) for t in (start, end)],
            "frame_mean_ms": round(statistics.mean(values), 3),
            "frame_p50_ms": round(statistics.median(values), 3),
            "frame_p95_ms": round(values[min(int(len(values) * .95), len(values) - 1)], 3),
            "frame_max_ms": round(max(values), 3),
            "frames_above_500ms": sum(ms > 500 for ms in values),
        })
    return {"session": str(session.resolve()), "online_drive_wall_seconds": online,
            "timing_caveat": "Rounded frame-delta integration with 100ms virtual clamp; confirm displayed HUD time. Frames above 500ms flag suspected pacing, not automatically excluded crowd spikes.",
            "screenshots": screenshots, "minute_intervals": intervals}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sessions", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = [inspect(session) for session in args.sessions]
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    for row in result:
        print(row["session"], len(row["screenshots"]), "shots;",
              len(row["minute_intervals"]), "complete minute intervals")
