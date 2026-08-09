"""Summarize the spike attribution a `CATHEDRAL_PERF=1` run wrote.

Reads the `[spike]` lines `src/perf.rs` puts in `logs.jsonl` — one per frame
over 20 ms, naming the probes that spent the time — and answers the question
those lines exist for: *what is a bad frame made of?*

Usage: perf_spikes.py [session_dir] [skip_seconds]
       (default: logs/latest_session, 15)
"""
import collections
import json
import os
import re
import sys

root = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    os.path.dirname(__file__), "..", "logs", "latest_session"
)
root = os.path.realpath(root)
skip = float(sys.argv[2]) if len(sys.argv) > 2 else 15.0

LINE = re.compile(
    r"\[spike\] t=([\d.]+)s frame ([\d.]+) ms "
    r"\(ecs ([\d.]+), render\+present ([\d.]+)\): (.*)"
)
ENTRY = re.compile(r"([a-z ]+?) ([\d.]+) \(x(\d+)\)")

spikes = []
for line in open(os.path.join(root, "logs.jsonl"), errors="replace"):
    try:
        record = json.loads(line)
    except ValueError:
        continue
    if record.get("source") != "perf":
        continue
    match = LINE.search(record.get("message", ""))
    if not match:
        continue
    t, frame_ms = float(match[1]), float(match[2])
    ecs_ms, render_ms, rest = float(match[3]), float(match[4]), match[5]
    if t < skip:
        continue
    probes = dict((m[1].strip(), (float(m[2]), int(m[3]))) for m in ENTRY.finditer(rest))
    spikes.append((t, frame_ms, ecs_ms, render_ms, probes))

if not spikes:
    print(f"no spikes over 20 ms after t={skip}s in {os.path.basename(root)}")
    sys.exit(0)

total_ms = collections.Counter()
peak_ms = collections.Counter()
appearances = collections.Counter()
for _, _, _, _, probes in spikes:
    for name, (ms, _hits) in probes.items():
        total_ms[name] += ms
        peak_ms[name] = max(peak_ms[name], ms)
        appearances[name] += 1

frame_total = sum(s[1] for s in spikes)
ecs_total = sum(s[2] for s in spikes)
render_total = sum(s[3] for s in spikes)
print(f"{os.path.basename(root)}: {len(spikes)} frames over 20 ms after t={skip}s")
print(f"  spike frame time {frame_total:.0f} ms total, {frame_total / len(spikes):.1f} ms mean")
print(f"    ECS main schedule   {ecs_total:8.0f} ms ({100 * ecs_total / frame_total:3.0f}%)"
      f"  of which probed {sum(total_ms.values()):.0f} ms")
print(f"    render + present    {render_total:8.0f} ms ({100 * render_total / frame_total:3.0f}%)")
print()
print(f"  {'probe':<14} {'total ms':>9} {'mean/spike':>11} {'worst':>7} {'in N spikes':>12}")
for name, ms in total_ms.most_common():
    print(f"  {name:<14} {ms:9.0f} {ms / len(spikes):11.2f} {peak_ms[name]:7.1f} "
          f"{appearances[name]:6d} ({100 * appearances[name] / len(spikes):3.0f}%)")

print()
print("  worst 12 frames:")
for t, frame_ms, ecs_ms, render_ms, probes in sorted(spikes, key=lambda s: -s[1])[:12]:
    named = ", ".join(
        f"{n} {ms:.1f}" for n, (ms, _) in sorted(probes.items(), key=lambda kv: -kv[1][0])[:4]
    )
    print(f"    t={t:7.1f}s {frame_ms:7.1f} ms = ecs {ecs_ms:6.1f} + render {render_ms:6.1f}"
          f"   {named or '-'}")
