# /// script
# requires-python = ">=3.11"
# dependencies = ["playwright"]
# ///
"""Render the deterministic HTML animation frame-by-frame, piping JPEGs into ffmpeg.

  uv run render.py --probe            # keyframe PNGs for visual inspection
  uv run render.py                    # full render -> supply_chain.mp4
"""
import argparse, json, subprocess, sys
from pathlib import Path
from playwright.sync_api import sync_playwright

HERE = Path(__file__).parent
FPS = 30

ap = argparse.ArgumentParser()
ap.add_argument("--probe", action="store_true")
ap.add_argument("--probe-at", type=float, nargs="*")
ap.add_argument("--out", default="supply_chain.mp4")
args = ap.parse_args()

T = json.loads((HERE / "timings.json").read_text())
total = T["total"]

with sync_playwright() as pw:
    br = pw.chromium.launch(args=["--force-color-profile=srgb",
                                  "--disable-lcd-text",
                                  "--hide-scrollbars"])
    pg = br.new_page(viewport={"width": 1920, "height": 1080}, device_scale_factor=1)

    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("console", lambda m: errors.append(f"console.{m.type}: {m.text}")
          if m.type == "error" else None)

    pg.goto((HERE / "anim.html").as_uri())
    pg.evaluate("t => { window.TIMINGS = t; }", T)
    pg.evaluate("async () => { await document.fonts.ready; }")
    pg.evaluate("() => renderAt(0)")
    if errors:
        print("PAGE ERRORS:\n  " + "\n  ".join(errors[:20]), file=sys.stderr)
        sys.exit(1)

    if args.probe:
        # one frame mid-way through each scene, plus any explicit timestamps
        shots = args.probe_at or [
            round(s["start"] + s["dur"] * 0.72, 2) for s in T["scenes"]]
        out = HERE / "probe"
        out.mkdir(exist_ok=True)
        for i, t in enumerate(shots):
            pg.evaluate("t => renderAt(t)", t)
            name = out / f"probe_{i:02d}_t{t:07.2f}.png"
            pg.screenshot(path=str(name))
            print(f"  {name.name}")
        if errors:
            print("PAGE ERRORS:\n  " + "\n  ".join(errors[:20]), file=sys.stderr)
        br.close()
        sys.exit(0)

    # ---- full render: pipe JPEG frames straight into ffmpeg ----
    nframes = int(total * FPS)
    ff = subprocess.Popen([
        "ffmpeg", "-y", "-hide_banner", "-loglevel", "error",
        "-f", "image2pipe", "-vcodec", "mjpeg", "-framerate", str(FPS), "-i", "-",
        "-c:v", "libx264", "-preset", "medium", "-crf", "19",
        "-pix_fmt", "yuv420p", "-movflags", "+faststart",
        str(HERE / "video_only.mp4"),
    ], stdin=subprocess.PIPE)

    for f in range(nframes):
        t = f / FPS
        pg.evaluate("t => renderAt(t)", t)
        ff.stdin.write(pg.screenshot(type="jpeg", quality=94))
        if f % 300 == 0:
            print(f"  frame {f}/{nframes}  t={t:6.1f}s  ({100*f/nframes:4.1f}%)", flush=True)

    ff.stdin.close()
    ff.wait()
    br.close()

if errors:
    print("PAGE ERRORS:\n  " + "\n  ".join(errors[:20]), file=sys.stderr)
print(f"wrote video_only.mp4  ({nframes} frames @ {FPS}fps)")
