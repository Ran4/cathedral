#!/usr/bin/env python3
"""Compile the production animals against the existing Bevy build and capture.

Avoids Cargo's shared build lock while another agent works on conversations.
The extracted helpers are exact source slices, never separate visual models.
Usage: uv run scripts/capture_animals.py --out captures/animals/baseline
"""
import argparse
import csv
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import math
import os
import re
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "target" / "animal_studio"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    parser.add_argument("--dog", default="brindle")
    parser.add_argument("--species", choices=["dog", "rat"], default="dog")
    parser.add_argument("--motion", choices=["still", "idle", "trot", "sequence", "sit_departure", "production"], default="still")
    parser.add_argument("--travel", action="store_true")
    parser.add_argument("--ramp", action="store_true", help="Use the production dog acceleration/braking limits at 20 Hz")
    parser.add_argument("--rat-trace", type=Path, help="Replay a production rat regression trace CSV and sibling JSON metadata")
    parser.add_argument("--speed", type=float, default=1.7)
    parser.add_argument("--observer", choices=["camera", "away", "approach"], default="camera")
    parser.add_argument("--seconds", type=float, default=4.0)
    parser.add_argument("--start-seconds", type=float, default=0.0,
                        help="Advance this much extra simulation time before recording")
    parser.add_argument("--from-zero", action="store_true", help="Capture motion from time zero after paused shader warmup")
    parser.add_argument("--fps", type=int, default=12)
    parser.add_argument("--view", choices=["three_quarter", "side", "front", "rear", "street_distance"], default="three_quarter")
    parser.add_argument("--baseline", action="store_true")
    parser.add_argument("--snapshot", help="Animal sources saved in gauntlet/snapshots/NAME")
    parser.add_argument("--bench", action="store_true")
    parser.add_argument("--no-build", action="store_true")
    args = parser.parse_args()
    if args.fps < 1 or 60 % args.fps or not 0 < args.seconds <= 120:
        parser.error("fps must divide 60; seconds must be in (0, 120]")
    if not math.isfinite(args.speed) or args.speed < 0 or round(args.seconds * args.fps) < 1:
        parser.error("speed must be finite and nonnegative; capture must contain at least one frame")
    if not math.isfinite(args.start_seconds) or not 0 <= args.start_seconds <= 120:
        parser.error("start-seconds must be finite and in [0, 120]")
    if args.start_seconds and (args.motion == "still" or args.bench):
        parser.error("start-seconds requires a motion capture")
    if args.from_zero and (args.motion == 'still' or args.bench):
        parser.error("from-zero requires a motion capture")
    if args.ramp and (args.species != "dog" or args.motion == "still" or args.bench):
        parser.error("ramp requires a dog motion capture")
    if args.motion == "production" and (not args.travel or args.ramp or args.bench):
        parser.error("production requires --travel and supplies its own speeds")
    if args.motion == "production" and args.species == "rat" and not args.rat_trace:
        parser.error("rat production motion requires --rat-trace")
    trace_metadata = None
    if args.rat_trace:
        if args.species != "rat" or args.motion != "production":
            parser.error("rat-trace requires --species rat --motion production --travel")
        args.rat_trace = args.rat_trace.resolve()
        trace_metadata = json.loads(args.rat_trace.with_suffix('.json').read_text())
        with args.rat_trace.open() as stream:
            rows = list(csv.DictReader(stream))
        required = ['time_seconds', 'x_m', 'z_m', 'heading_x', 'heading_z']
        if not rows or any(any(not math.isfinite(float(row[key])) for key in required) for row in rows):
            parser.error("rat trace needs finite time, position and heading rows")
        times = [float(row['time_seconds']) for row in rows]
        if times[0] > 0 or any(b <= a or b - a > .02 for a, b in zip(times, times[1:])):
            parser.error("rat trace must start at zero and provide increasing samples at least at 50 Hz")
        if times[-1] < args.start_seconds + (0 if args.from_zero else .4) + args.seconds - 1 / args.fps:
            parser.error("rat trace does not cover the requested capture interval")
        for key in ['length_m', 'tint', 'phase', 'period']:
            if not math.isfinite(float(trace_metadata[key])):
                parser.error(f"rat trace metadata {key} must be finite")
        if trace_metadata['length_m'] <= 0 or trace_metadata['period'] <= 0:
            parser.error("rat trace length and period must be positive")
    BUILD.mkdir(parents=True, exist_ok=True)
    # Our tiny capture lock is independent of Cargo's game build lock. Keep
    # generated sources/binary paired when a builder and recorder overlap.
    lock = (BUILD / "capture.lock").open("w")
    fcntl.flock(lock, fcntl.LOCK_EX)
    body = (ROOT / "src/smart_actors/body.rs").read_text()
    start = body.index("#[derive(Debug, Clone, Copy)]\npub(super) struct Ring")
    end = body.index("// --- Trunk", start)
    (BUILD / "loft.rs").write_text(
        "use bevy::{asset::RenderAssetUsages, mesh::{Indices, PrimitiveTopology, VertexAttributeValues}, prelude::*};\n"
        "use std::f32::consts::TAU;\n" + body[start:end]
    )
    snapshot = ROOT / "gauntlet/snapshots" / args.snapshot if args.snapshot else None
    if snapshot and not snapshot.is_dir():
        parser.error(f"Source snapshot does not exist: {snapshot}")
    vermin_source = ROOT / "gauntlet/baseline/vermin.rs" if args.baseline else ROOT / "src/city/vermin.rs"
    if snapshot and (snapshot / "vermin.rs").is_file():
        vermin_source = snapshot / "vermin.rs"
    vermin = vermin_source.read_text()
    rat_structs = vermin[vermin.index("struct Leg {"):vermin.index("struct Colony {")]
    rat_hash = vermin[vermin.index("fn mix(mut x:"):vermin.index("fn walkable(")]
    rat_render = vermin[vermin.index("const BODY_SECTORS:"):vermin.index("#[cfg(test)]")]
    rat_ground = re.search(r"const RAT_GROUND_Y: f32 = [^;]+;", vermin).group(0)
    rat_motion_api = "struct RatMotion" in vermin
    rat_adapter = ROOT / "scripts/animal_rat_studio.rs"
    (BUILD / "rat.rs").write_text(
        "use bevy::{asset::RenderAssetUsages, mesh::{Indices, PrimitiveTopology}, prelude::*};\n"
        + rat_ground + "\n" + rat_structs + rat_hash + rat_render + rat_adapter.read_text()
    )
    source = (ROOT / "scripts/animal_studio.rs").read_text()
    dog_source = ROOT / "gauntlet/baseline/dogs.rs" if args.baseline else ROOT / "src/smart_actors/dogs.rs"
    if snapshot and (snapshot / "dogs.rs").is_file():
        dog_source = snapshot / "dogs.rs"
    source = source.replace('../src/smart_actors/dogs.rs', str(dog_source))
    (BUILD / "main.rs").write_text(source)
    source_paths = [dog_source, vermin_source, ROOT / "src/smart_actors/body.rs", ROOT / "scripts/animal_studio.rs", rat_adapter]
    source_paths.extend(sorted((dog_source.parent / "dogs").glob("*.rs")))
    env = os.environ | {
        "ANIMAL_STUDIO_BUILD_DIR": str(BUILD),
        "ANIMAL_CAPTURE_OUT": str((ROOT / args.out).resolve()),
        "ANIMAL_DOG": args.dog,
        "ANIMAL_SPECIES": args.species,
        "ANIMAL_MOTION": args.motion,
        "ANIMAL_SECONDS": str(args.seconds),
        "ANIMAL_START_SECONDS": str(args.start_seconds),
        "ANIMAL_INITIAL_STEPS": str(round(args.start_seconds * 60) + (0 if args.from_zero else 24)),
        "ANIMAL_FPS": str(args.fps),
        "ANIMAL_VIEW": args.view,
        "ANIMAL_TRAVEL": str(int(args.travel)),
        "ANIMAL_RAMP": str(int(args.ramp)),
        "ANIMAL_SPEED": str(args.speed),
        "ANIMAL_OBSERVER": args.observer,
        "CATHEDRAL_HEADLESS": "1",
        "BEVY_ASSET_ROOT": str(ROOT),
        "CARGO_MANIFEST_DIR": str(ROOT),
        "CARGO_PKG_NAME": "cathedralbevy",
    }
    # Explicit defaults prevent an unrelated shell fixture leaking into a run.
    for key, fallback in {'SEED': 37, 'LENGTH_M': .28, 'TINT': 1.0, 'PHASE': 0.0, 'PERIOD': 10.0}.items():
        env['ANIMAL_RAT_' + key] = str(trace_metadata[key.lower()] if trace_metadata else fallback)
    env.pop('ANIMAL_RAT_TRACE', None)
    if args.rat_trace:
        env['ANIMAL_RAT_TRACE'] = str(args.rat_trace)
    if not args.no_build:
        deps = ROOT / "target/debug/deps"
        libraries = {name: max(deps.glob(f"lib{name}-*.rlib"), key=lambda p: p.stat().st_mtime)
                     for name in ("bevy", "cathedral_sim")}
        command = ["rustc", "--edition=2024", "--crate-name", "animal_studio",
                   "-C", "opt-level=1", "-C", "debuginfo=0", "-C", "link-arg=-fuse-ld=lld",
                   "-L", f"dependency={deps}", "-o", str(BUILD / "animal_studio"),
                   str(BUILD / "main.rs")]
        if rat_motion_api:
            command.extend(["--cfg", "animal_rat_motion"])
        for name, path in libraries.items():
            command.extend(["--extern", f"{name}={path}"])
        build_manifest = {
            "built_at_utc": datetime.now(timezone.utc).isoformat(),
            "sources_sha256": {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_paths},
            "generated_sha256": {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in (BUILD / "main.rs", BUILD / "loft.rs", BUILD / "rat.rs")},
            "libraries": {name: {"path": str(path.relative_to(ROOT)), "bytes": path.stat().st_size, "mtime_ns": path.stat().st_mtime_ns} for name, path in libraries.items()},
            "optimization": "rustc opt-level=1; cached Bevy dependencies",
        }
        subprocess.run(command, cwd=ROOT, env=env, check=True, timeout=240)
        (BUILD / "build_manifest.json").write_text(json.dumps(build_manifest, indent=2) + "\n")
    else:
        recorded = json.loads((BUILD / "build_manifest.json").read_text())
        requested_sources = {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_paths}
        generated = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in (BUILD / "main.rs", BUILD / "loft.rs", BUILD / "rat.rs")}
        if recorded.get("sources_sha256") != requested_sources or recorded.get("generated_sha256") != generated:
            parser.error("Cached animal sources or harness differ from this request; omit --no-build to compile the requested version")
    if args.bench:
        env["ANIMAL_BENCH"] = "1"
    output = Path(env["ANIMAL_CAPTURE_OUT"])
    output.mkdir(parents=True, exist_ok=True)
    compiled = BUILD / "build_manifest.json"
    capture_manifest = {
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
        "completed": False,
        "species": args.species, "dog": args.dog, "motion": args.motion,
        "seconds": args.seconds, "fps": args.fps, "view": args.view,
        "start_seconds": args.start_seconds,
        "from_zero": args.from_zero,
        "travel": args.travel, "speed_mps": args.speed, "observer": args.observer, "benchmark": args.bench,
        "ramp": args.ramp,
        "build": json.loads(compiled.read_text()) if compiled.is_file() else None,
    }
    if args.motion == "production":
        capture_manifest["navigation_sha256"] = {
            str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in [ROOT / "assets/world/navigation.json", ROOT / "assets/world/navigation.bin"]
        }
    if args.rat_trace:
        capture_manifest['rat_trace'] = {
            'path': str(args.rat_trace), 'metadata': trace_metadata,
            'csv_sha256': hashlib.sha256(args.rat_trace.read_bytes()).hexdigest(),
            'metadata_sha256': hashlib.sha256(args.rat_trace.with_suffix('.json').read_bytes()).hexdigest(),
        }
    (output / "capture_manifest.json").write_text(json.dumps(capture_manifest, indent=2) + "\n")
    subprocess.run([str(BUILD / "animal_studio")], cwd=ROOT, env=env, check=True, timeout=600)
    if args.bench:
        capture_manifest["completed"] = True
        capture_manifest["completed_at_utc"] = datetime.now(timezone.utc).isoformat()
        (output / "capture_manifest.json").write_text(json.dumps(capture_manifest, indent=2) + "\n")
        return
    expected = ([f"frame_{index:03}" for index in range(round(args.seconds * args.fps))] if args.motion != "still"
                else ["three_quarter", "side", "front", "rear", "street_distance"])
    for name in expected:
        image = Path(env["ANIMAL_CAPTURE_OUT"]) / f"{name}.png"
        if not image.is_file() or image.stat().st_size < 1000:
            raise RuntimeError(f"Capture missing or empty: {image}")
        # Delayed shader compilation can produce a valid, uniformly dark PNG.
        # The neutral stage has a lit grey floor: darkness is a failed render,
        # never a visual result to hand to the critic or overwrite a baseline.
        if name == expected[0] or args.motion == "still":
            pixels = subprocess.run(["ffmpeg", "-v", "error", "-i", str(image),
                                     "-vf", "scale=32:24,format=gray", "-frames:v", "1",
                                     "-f", "rawvideo", "-"], check=True, capture_output=True).stdout
            if not pixels or sum(pixels) / len(pixels) < 28:
                raise RuntimeError(f"Dark/unfinished render rejected: {image}")
    print(f"Verified {len(expected)} captures: {env['ANIMAL_CAPTURE_OUT']}")
    if args.motion != "still":
        output = Path(env["ANIMAL_CAPTURE_OUT"])
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", str(args.fps),
                        "-i", str(output / "frame_%03d.png"), "-c:v", "libx264", "-crf", "22",
                        "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(output / "motion.mp4")], check=True)
    capture_manifest["completed"] = True
    capture_manifest["capture_count"] = len(expected)
    capture_manifest["completed_at_utc"] = datetime.now(timezone.utc).isoformat()
    (output / "capture_manifest.json").write_text(json.dumps(capture_manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
