# /// script
# requires-python = ">=3.11"
# ///
"""Lay each scene's narration onto one timeline at its exact start, then mux with the video."""
import json, subprocess, sys
from pathlib import Path

HERE = Path(__file__).parent
T = json.loads((HERE / "timings.json").read_text())
scenes = T["scenes"]
total = T["total"]

ins, filts, labels = [], [], []
for i, s in enumerate(scenes):
    ins += ["-i", str(HERE / "audio" / s["audio"])]
    ms = int(round(s["start"] * 1000))
    # mono -> stereo, delayed to its slot on the timeline
    filts.append(f"[{i}:a]aformat=channel_layouts=stereo,adelay={ms}|{ms}[a{i}]")
    labels.append(f"[a{i}]")

filts.append(
    "".join(labels) + f"amix=inputs={len(scenes)}:normalize=0:dropout_transition=0[mixed]")
# gentle level control so the loud and quiet takes sit together, then pad to length
filts.append(
    f"[mixed]dynaudnorm=f=250:g=15:p=0.72:m=6,alimiter=limit=0.92,"
    f"apad,atrim=0:{total},afade=t=in:st=0:d=0.6,"
    f"afade=t=out:st={total-1.6:.3f}:d=1.6[out]")

cmd = (["ffmpeg", "-y", "-hide_banner", "-loglevel", "error"] + ins +
       ["-filter_complex", ";".join(filts), "-map", "[out]",
        "-c:a", "pcm_s16le", "-ar", "48000", str(HERE / "narration.wav")])
subprocess.run(cmd, check=True)
print("wrote narration.wav")

vid = HERE / "video_only.mp4"
if not vid.exists():
    print("video_only.mp4 not rendered yet — run render.py first", file=sys.stderr)
    sys.exit(1)

subprocess.run([
    "ffmpeg", "-y", "-hide_banner", "-loglevel", "error",
    "-i", str(vid), "-i", str(HERE / "narration.wav"),
    "-map", "0:v:0", "-map", "1:a:0",
    "-c:v", "copy", "-c:a", "aac", "-b:a", "192k",
    "-shortest", "-movflags", "+faststart",
    str(HERE / "the_supply_chain.mp4"),
], check=True)
print("wrote the_supply_chain.mp4")
