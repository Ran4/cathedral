#!/usr/bin/env python3
"""Summarize the latest session's perf_frames.jsonl (CATHEDRAL_PERF=1 runs).

Usage: perf_report.py [label] [skip_seconds]
Prints one line: mean/p50/p95/p99/max frame ms and over-budget counts for
frames after skip_seconds (default 20 — past loading and teleport settle).
"""
import json
import os
import statistics
import sys

label = sys.argv[1] if len(sys.argv) > 1 else "run"
skip = float(sys.argv[2]) if len(sys.argv) > 2 else 20.0
root = os.path.realpath(os.path.join(os.path.dirname(__file__), "..", "logs", "latest_session"))
frames = []
for line in open(os.path.join(root, "perf_frames.jsonl")):
    record = json.loads(line)
    t = record["t"]
    for ms in record["frames_ms"]:
        frames.append((t, ms))
        t += ms / 1000.0
window = sorted(ms for t, ms in frames if t >= skip)


def pct(q: float) -> float:
    return window[min(len(window) - 1, round((len(window) - 1) * q))]


over_frame = sum(1 for ms in window if ms > 16.7)
print(
    f"{label:26s} frames={len(window):6d} mean={statistics.fmean(window):5.2f} "
    f"p50={pct(.5):5.2f} p95={pct(.95):5.2f} p99={pct(.99):6.2f} max={window[-1]:7.2f} "
    f"over16.7={over_frame:4d} ({100 * over_frame / len(window):4.1f}%) "
    f"over22={sum(1 for ms in window if ms > 22):3d} ({os.path.basename(root)})"
)
