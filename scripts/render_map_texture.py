#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["cairosvg"]
# ///
"""Bake the authoritative city plan into an in-game map texture.

Takes only the building footprints from `lore/places/ombreval_top_down_map.svg`
(the deterministic cadastral plan) and renders them on a *transparent*
background — no ground, no walls, no river, no labels — so the city's houses
"float". The result, `assets/textures/city_map.png`, is the single image behind
both the in-game minimap and the M-toggled fullscreen map (`src/map.rs`).

World <-> image transform (MUST stay in sync with `src/map.rs`):

    The SVG projects world (x, z) -> SVG user units by  screen(x, z) = (-z, -x)
    (see `generate_top_down_map.py::screen`), east-right / north-up. The image is
    the building bounding box [VX0, VX1] x [VY0, VY1] (SVG user units), so for a
    normalized point (u, v) in [0, 1]^2 over the image:

        u = (-z - VX0) / (VX1 - VX0)     v = (-x - VY0) / (VY1 - VY0)
        z = -(VX0 + u * (VX1 - VX0))     x = -(VY0 + v * (VY1 - VY0))

    North (+x world) is up; east (-z world) is right. The crop is computed here
    from the footprints and PRINTED — copy VX0/VX1/VY0/VY1 into `src/map.rs`.

Run once and commit the PNG:  ./scripts/render_map_texture.py
"""

from __future__ import annotations

from pathlib import Path
import re

import cairosvg

# SVG user units per output pixel. 1.5 keeps the fullscreen map crisp on a 1080p
# display while keeping the PNG a couple of megabytes.
SCALE = 1.5
# A little breathing room around the outermost footprints, in SVG units.
MARGIN = 6.0


def _repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in (here, *here.parents):
        if (parent / ".git").exists():
            return parent
    return here.parent


REPO_ROOT = _repo_root()
SVG_PATH = REPO_ROOT / "lore" / "places" / "ombreval_top_down_map.svg"
OUT_PATH = REPO_ROOT / "assets" / "textures" / "city_map.png"


def main() -> None:
    svg = SVG_PATH.read_text(encoding="utf-8")

    # The <style> block carries the material fill/stroke classes the footprints
    # reference; keep it so they render with their materials.
    defs = re.search(r"<defs>.*?</defs>", svg, re.DOTALL)
    # Only the footprints (polygons + their roof-ridge lines), nothing else.
    buildings = re.search(
        r'(<g id="buildings">.*?</g>)\s*<g id="fortifications">', svg, re.DOTALL
    )
    if defs is None or buildings is None:
        raise SystemExit("map SVG structure changed: <defs> or buildings group not found")

    # Bounding box of every footprint vertex, in SVG (screen) units.
    xs: list[float] = []
    ys: list[float] = []
    for points in re.findall(r'points="([^"]+)"', buildings.group(1)):
        for pair in points.split():
            sx, sy = pair.split(",")
            xs.append(float(sx))
            ys.append(float(sy))
    vx0 = min(xs) - MARGIN
    vx1 = max(xs) + MARGIN
    vy0 = min(ys) - MARGIN
    vy1 = max(ys) + MARGIN
    width = vx1 - vx0
    height = vy1 - vy0

    document = (
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'viewBox="{vx0:.2f} {vy0:.2f} {width:.2f} {height:.2f}">'
        f"{defs.group(0)}{buildings.group(1)}</svg>"
    )

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    cairosvg.svg2png(
        bytestring=document.encode("utf-8"),
        write_to=str(OUT_PATH),
        output_width=round(width * SCALE),
        output_height=round(height * SCALE),
    )

    print(f"footprints only, transparent background, scale {SCALE}")
    print("copy these into src/map.rs:")
    print(f"    const VX0: f32 = {vx0:.1f};")
    print(f"    const VX1: f32 = {vx1:.1f};")
    print(f"    const VY0: f32 = {vy0:.1f};")
    print(f"    const VY1: f32 = {vy1:.1f};")
    print(f"wrote {OUT_PATH.relative_to(REPO_ROOT)}  {round(width * SCALE)}x{round(height * SCALE)}")


if __name__ == "__main__":
    main()
