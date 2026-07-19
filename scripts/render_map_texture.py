#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["cairosvg", "Pillow"]
# ///
"""Bake the authoritative city plan into an in-game map texture.

Renders `lore/places/ombreval_top_down_map.svg` (the deterministic cadastral
plan produced by `generate_top_down_map.py`) to a PNG and crops off the
right-hand legend / place-index panel, leaving just the city. The result,
`assets/textures/city_map.png`, is the single image behind both the in-game
minimap and the M-toggled fullscreen map (`src/map.rs`).

World <-> image transform (MUST stay in sync with `src/map.rs`):

    The SVG projects world (x, z) -> SVG user units by  screen(x, z) = (-z, -x)
    (see `generate_top_down_map.py::screen`), east-right / north-up. We keep only
    the viewBox sub-rectangle [VX0, VX1] x [VY0, VY1] (SVG user units), so for a
    normalized point (u, v) in [0, 1]^2 over the cropped image:

        u = (-z - VX0) / (VX1 - VX0)     v = (-x - VY0) / (VY1 - VY0)
        z = -(VX0 + u * (VX1 - VX0))     x = -(VY0 + v * (VY1 - VY0))

    North (+x world) is up; east (-z world) is right.

Run once and commit the PNG:  ./scripts/render_map_texture.py
"""

from __future__ import annotations

import io
from pathlib import Path

import cairosvg
from PIL import Image

# Full SVG viewBox, straight from `generate_top_down_map.py::svg_header`.
VIEWBOX_MIN_X = -600.0
VIEWBOX_MIN_Y = -610.0
VIEWBOX_W = 1720.0
VIEWBOX_H = 1440.0

# The city crop: the whole plan minus the legend / place-index / inventory panels
# (all drawn at SVG x >= 735). Keeps the title, scale bar, walls, and the river.
VX0, VX1 = -600.0, 725.0
VY0, VY1 = -610.0, 830.0

# SVG user units per output pixel. 1.5 keeps the fullscreen map crisp on a 1080p
# display (~2160 px tall texture) while keeping the PNG a couple of megabytes.
SCALE = 1.5


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
    full_w = round(VIEWBOX_W * SCALE)
    full_h = round(VIEWBOX_H * SCALE)
    png_bytes = cairosvg.svg2png(
        url=str(SVG_PATH), output_width=full_w, output_height=full_h
    )
    full = Image.open(io.BytesIO(png_bytes)).convert("RGBA")

    # viewBox user unit -> full-render pixel: (unit - viewBox_min) * SCALE.
    left = round((VX0 - VIEWBOX_MIN_X) * SCALE)
    upper = round((VY0 - VIEWBOX_MIN_Y) * SCALE)
    right = round((VX1 - VIEWBOX_MIN_X) * SCALE)
    lower = round((VY1 - VIEWBOX_MIN_Y) * SCALE)
    cropped = full.crop((left, upper, right, lower))

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    cropped.save(OUT_PATH, optimize=True)

    aspect = (VX1 - VX0) / (VY1 - VY0)
    print(f"rendered full {full.size[0]}x{full.size[1]} @ scale {SCALE}")
    print(f"crop viewBox  x[{VX0}, {VX1}]  y[{VY0}, {VY1}]  aspect(w/h)={aspect:.4f}")
    print(f"crop pixels   ({left}, {upper}) -> ({right}, {lower})")
    print(f"wrote {OUT_PATH.relative_to(REPO_ROOT)}  {cropped.size[0]}x{cropped.size[1]}")


if __name__ == "__main__":
    main()
