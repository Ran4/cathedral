#!/usr/bin/env python3
"""Make a chronological sheet and contact-point report from studio captures.

Usage: uv run scripts/summarize_animal_motion.py captures/animals/dog_walk
       uv run scripts/summarize_animal_motion.py <capture> --start 6 --duration 2

The sheet contains actual captured frames with their recorded simulation times.
Contact metrics are sampled observations, not a substitute for continuous gait
tests. They only report named sole joints when the production rig supplies them.
"""

import argparse
import json
import math
from pathlib import Path
import statistics
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--start", type=float, default=0.0)
    parser.add_argument("--duration", type=float)
    parser.add_argument("--name", default="contact_sheet")
    args = parser.parse_args()
    records = []
    for path in sorted(args.capture.glob("frame_*.json")):
        record = json.loads(path.read_text())
        time = record["time_seconds"]
        if time >= args.start - 1e-6 and (args.duration is None or time <= args.start + args.duration + 1e-6):
            records.append((path.with_suffix(".png"), record))
    if not records:
        parser.error("No captured frames in the requested interval")

    count = min(16, len(records))
    selected = [records[round(index * (len(records) - 1) / max(1, count - 1))] for index in range(count)]
    command = ["ffmpeg", "-y", "-loglevel", "error"]
    for path, _ in selected:
        command.extend(["-threads", "1", "-i", str(path)])
    filters = []
    for index, (_, record) in enumerate(selected):
        time = record["time_seconds"]
        filters.append(
            f"[{index}:v]scale=400:300,drawtext=text='t = {time:.3f} s':"
            f"fontcolor=white:fontsize=18:x=9:y=9:box=1:boxcolor=black@0.65:boxborderw=4[s{index}]"
        )
    layout = "|".join(f"{index % 4 * 400}_{index // 4 * 300}" for index in range(count))
    if count > 1:
        filters.append("".join(f"[s{index}]" for index in range(count)) + f"xstack=inputs={count}:layout={layout}:fill=black[sheet]")
    else:
        filters.append("[s0]null[sheet]")
    output = args.capture / f"{args.name}.png"
    command.extend(["-filter_complex_threads", "1", "-filter_complex", ";".join(filters),
                    "-map", "[sheet]", "-frames:v", "1", "-threads", "1", str(output)])
    subprocess.run(command, check=True)

    heights = []
    travel_slip = []
    stopped_slip = []
    def contact_points(record):
        if record.get('species') == 'rat' and record.get('rat_pose', {}).get('foot_anchors_world'):
            # Anchors are relative to the actual 0.012 m road plane, not proof
            # that every rendered toe/limb vertex clears it.
            return {f'Rat foot anchor {name}': [p[0], p[1] - .012, p[2]]
                    for name, p in zip(['FL', 'FR', 'RL', 'RR'], record['rat_pose']['foot_anchors_world'])}
        return {name: point for name, point in record.get('rig_points', {}).items()
                if name.startswith('Dog sole ')}

    for index, (_, record) in enumerate(records):
        points = contact_points(record)
        for name, point in points.items():
            heights.append(point[1])
            if not index or not record.get("travel"):
                continue
            previous = records[index - 1][1]
            other = contact_points(previous).get(name)
            dt = record["time_seconds"] - previous["time_seconds"]
            # Exclude swing endpoints and coarse intervals which could contain
            # an entire unseen step. Millimetre tolerance matches rig tests.
            if other and dt > 0 and dt < 0.1 and abs(point[1]) <= 0.003 and abs(other[1]) <= 0.003:
                speed = math.hypot(point[0] - other[0], point[2] - other[2]) / dt
                if record.get("moving"):
                    travel_slip.append(speed)
                elif not previous.get("moving"):
                    stopped_slip.append({"sole": name, "speed_mps": speed,
                                         "from_seconds": previous["time_seconds"],
                                         "to_seconds": record["time_seconds"]})
    report = {
        "capture": str(args.capture),
        "first_time_seconds": records[0][1]["time_seconds"],
        "last_time_seconds": records[-1][1]["time_seconds"],
        "captured_frames": len(records),
        "sheet_frames": [{"image": str(path), "time_seconds": record["time_seconds"]} for path, record in selected],
        "contact_observations": {
            "point_kind": "virtual rat foot anchors relative to road" if records[0][1].get('species') == 'rat' else "authored dog sole points",
            "sole_points": len(heights),
            "minimum_y_m": min(heights) if heights else None,
            "maximum_y_m": max(heights) if heights else None,
            "travel_intervals_both_endpoints_planted": len(travel_slip),
            "planted_endpoint_speed_median_mps": statistics.median(travel_slip) if travel_slip else None,
            "planted_endpoint_speed_max_mps": max(travel_slip) if travel_slip else None,
            "stopped_intervals_both_endpoints_planted": len(stopped_slip),
            "stopped_endpoint_speed_median_mps": statistics.median(s["speed_mps"] for s in stopped_slip) if stopped_slip else None,
            "stopped_endpoint_worst": max(stopped_slip, key=lambda s: s["speed_mps"]) if stopped_slip else None,
            "limit": "Finite-rate sole-joint observations on the flat stage; cannot establish contact during an unsampled interval or rendered skin-surface clearance.",
        },
    }
    output.with_suffix(".json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Wrote {output}; {len(records)} source frames, {len(heights)} sole observations")


if __name__ == "__main__":
    main()
