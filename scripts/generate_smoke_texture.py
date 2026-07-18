#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pillow"]
# ///
"""Generate the chimney-smoke puff atlas (assets/textures/ombreval_smoke.png).

Unlike the cloth artwork this one is fully procedural — a smoke puff is just
soft fractal noise under a radial falloff, and doing it in numpy keeps the
atlas deterministic and free of API keys. The output is a 2x2 atlas of four
distinct puff sprites (1024x1024 total, 512 per cell): near-white RGB with a
touch of per-pixel warmth, and everything shape-defining in the alpha channel.
Each cell fades to fully transparent well inside its border so the quads can
sample with plain repeat addressing and never bleed into a neighbour.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
from PIL import Image


def _repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in (here, *here.parents):
        if (parent / ".git").exists():
            return parent
    return here.parent


CELL = 512
OUTPUT = _repo_root() / "assets" / "textures" / "ombreval_smoke.png"


def fbm(rng: np.random.Generator, size: int, octaves: int = 5) -> np.ndarray:
    """Value-noise fbm in [0, 1]: random grids upscaled with bicubic blending."""
    total = np.zeros((size, size), dtype=np.float64)
    amplitude, amplitude_sum = 1.0, 0.0
    for octave in range(octaves):
        cells = 4 * (2**octave)
        grid = rng.random((cells, cells))
        layer = Image.fromarray((grid * 255).astype(np.uint8), mode="L")
        layer = layer.resize((size, size), Image.Resampling.BICUBIC)
        total += amplitude * (np.asarray(layer, dtype=np.float64) / 255.0)
        amplitude_sum += amplitude
        amplitude *= 0.55
    return total / amplitude_sum


def puff(seed: int) -> np.ndarray:
    """One RGBA puff cell: lumpy fractal blob, transparent at the cell edge."""
    rng = np.random.default_rng(seed)
    axis = (np.arange(CELL) + 0.5) / CELL * 2.0 - 1.0
    xs, ys = np.meshgrid(axis, axis)
    radius = np.sqrt(xs**2 + ys**2)

    noise = fbm(rng, CELL)
    # Let the noise gnaw at the rim so the silhouette is wispy, not a disc.
    rim = radius + (noise - 0.5) * 0.55
    body = np.clip(1.0 - rim / 0.82, 0.0, 1.0) ** 1.3
    alpha = body * (0.6 + 0.4 * noise)
    # Hard guarantee of an empty border: atlas cells must never bleed.
    alpha *= np.clip((0.94 - radius) / 0.14, 0.0, 1.0)

    # Slightly warm, slightly uneven grey — the plume tint rides vertex colour.
    shade = 0.82 + 0.18 * fbm(rng, CELL, octaves=3)
    rgba = np.empty((CELL, CELL, 4), dtype=np.uint8)
    rgba[..., 0] = np.clip(shade * 255, 0, 255).astype(np.uint8)
    rgba[..., 1] = np.clip(shade * 0.985 * 255, 0, 255).astype(np.uint8)
    rgba[..., 2] = np.clip(shade * 0.96 * 255, 0, 255).astype(np.uint8)
    rgba[..., 3] = np.clip(alpha * 255, 0, 255).astype(np.uint8)
    return rgba


def main() -> None:
    atlas = np.zeros((CELL * 2, CELL * 2, 4), dtype=np.uint8)
    for index, seed in enumerate((11, 23, 47, 71)):
        row, col = divmod(index, 2)
        atlas[row * CELL : (row + 1) * CELL, col * CELL : (col + 1) * CELL] = puff(seed)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(atlas, mode="RGBA").save(OUTPUT)
    print(f"wrote {OUTPUT}")


if __name__ == "__main__":
    main()
