"""Descriptive same-camera frame comparison; no game launch or speedup claim.

Complete minute intervals must lie within estimated simulation ages90–600s
and hold the unchanged elevated Needle camera throughout. Keep every frame,
including ordinary large spikes. The before runs overlapped implementation
work and had microphone input enabled, so this does not isolate causation.
"""
import hashlib
import json
import statistics
from pathlib import Path

directory = Path(__file__).resolve().parent
camera = "tp -113.375 22 183.125 0 -45"
records = []
for population in [1000, 2000]:
    for prefix, filename in [
        ("before", f"ambient_before_{population}_timing.json"),
        ("final", f"final_{population}_gpu_summary.json"),
    ]:
        summary = json.loads((directory / filename).read_text())
        if isinstance(summary, list):
            assert len(summary) == 1
            summary = summary[0]
        intervals = [r for r in summary["minute_intervals"]
                     if r["camera"] == camera and not r["camera_changes"]
                     and r["estimated_virtual_seconds_since_online"][0] >= 90
                     and r["estimated_virtual_seconds_since_online"][1] <= 600]
        assert intervals
        ranges = [r["wall_seconds"] for r in intervals]
        path = Path(summary["session"]) / "perf_frames.jsonl"
        values = []
        for line in path.read_text().splitlines():
            chunk = json.loads(line)
            time = chunk["t"]
            for index, ms in enumerate(chunk["frames_ms"]):
                if index:
                    time += ms / 1000
                if any(lo <= time < hi for lo, hi in ranges):
                    values.append(ms)
        assert len(values) == sum(r["frames"] for r in intervals)
        values.sort()
        record = {
            "population": population, "version": prefix, "session": summary["session"],
            "camera": camera, "wall_intervals": ranges,
            "estimated_virtual_intervals": [r["estimated_virtual_seconds_since_online"] for r in intervals],
            "frames": len(values), "mean_ms": statistics.mean(values),
            "p50_ms": statistics.median(values), "p95_ms": values[int(len(values) * .95)],
            "max_ms": max(values), "frames_above_500ms": sum(ms > 500 for ms in values),
            "perf_frames_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
        records.append(record)
        print(prefix, population, len(intervals), "minutes", len(values), "frames",
              "mean/p50/p95/max", *(round(record[k], 3) for k in ["mean_ms", "p50_ms", "p95_ms", "max_ms"]))
result = {
    "selection": "All complete 60-second intervals at exact fixed Needle camera, estimated virtual start>=90s and end<=600s. No per-frame exclusions or threshold-based removal of spikes.",
    "settings": "RTX4070,Vulkan,1280x720logical,normal1x,3600s/day,clear. Current shots physically1493x840. Before geometry isM1; final geometry isM3.",
    "causal_limit": "Descriptive observations only: before runs overlapped implementation activity and microphone was enabled. No controlled causal overall GPU speedup is established. Full minute summaries retain startup and late pacing outside these intervals.",
    "records": records,
}
(directory / "gpu_frame_comparison.json").write_text(json.dumps(result, indent=2) + "\n")
