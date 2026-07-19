#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow", "numpy", "scipy"]
# ///
"""Reframe the NPC portrait faces so they read as heads, not floating masks.

The faces authored by generate_npc_textures.py are a small painted head centred
on a wide, uniform *light beige* background. The head mesh (src/smart_actors/
body.rs) projects the front hemisphere from the image and clamp-to-edges the
rest, so that beige margin becomes the entire back/sides/top of the head: a pale
ball with a small face floating on the front.

This pass fixes both problems, in place, without touching the image API:

* crop tight to the head so the face fills the front hemisphere, and
* replace the border-connected background with a warm shaded tone, feathered at
  the hair edge, so the smeared periphery reads as hair-in-shadow.

The exterior is found by flood-fill from the image border (not a global colour
threshold) so a pale forehead that happens to match the background is never
mistaken for background.

The same `frame_head_portrait` is applied by generate_npc_textures.py right after
downscale, so a future regeneration produces already-reframed faces. This script
reframes the *existing* files:

    scripts/reframe_faces.py                     # reframe assets/textures/npc in place
    scripts/reframe_faces.py --from BACKUP_DIR   # read originals from BACKUP_DIR

Cropping is content-driven, so re-running on already-reframed files is close to a
no-op, but pass --from a pristine copy when you want an exact redo.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import gaussian_filter, label

# Warm dark tone the head's shaded periphery (hair / rounded-away sides) fades to.
SHADOW = np.array([64.0, 53.0, 45.0])
OUT_SIZE = 256


def _repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in (here, *here.parents):
        if (parent / ".git").exists():
            return parent
    return here.parent


FACE_DIR = _repo_root() / "assets" / "textures" / "npc"


def _exterior_mask(arr: np.ndarray, background: np.ndarray, tolerance: float) -> np.ndarray:
    """Border-connected pixels within `tolerance` of `background` — the true
    exterior, so an interior region that merely matches the background colour
    (a pale forehead) is kept as foreground."""
    near = np.abs(arr - background).sum(2) < tolerance
    labelled, _ = label(near)
    border = (
        set(labelled[0].tolist())
        | set(labelled[-1].tolist())
        | set(labelled[:, 0].tolist())
        | set(labelled[:, -1].tolist())
    )
    border.discard(0)
    return np.isin(labelled, list(border))


def _median_corner(arr: np.ndarray, size: int) -> np.ndarray:
    corners = np.concatenate(
        [
            arr[:size, :size].reshape(-1, 3),
            arr[:size, -size:].reshape(-1, 3),
            arr[-size:, :size].reshape(-1, 3),
            arr[-size:, -size:].reshape(-1, 3),
        ]
    )
    return np.median(corners, axis=0)


def frame_head_portrait(image: Image.Image) -> Image.Image:
    """Crop tight to the head and fade the exterior background to `SHADOW`."""
    image = image.convert("RGB")
    arr = np.asarray(image).astype(float)

    background = _median_corner(arr, 12)
    foreground = ~_exterior_mask(arr, background, tolerance=42)
    ys, xs = np.where(foreground)
    y0, y1, x0, x1 = ys.min(), ys.max(), xs.min(), xs.max()
    cy, cx = (y0 + y1) / 2, (x0 + x1) / 2
    half = max(y1 - y0, x1 - x0) / 2 * 1.04
    box = (round(cx - half), round(cy - half), round(cx + half), round(cy + half))

    cropped = image.crop(box).resize((OUT_SIZE, OUT_SIZE), Image.LANCZOS)
    arr = np.asarray(cropped).astype(float)

    background = _median_corner(arr, 8)
    mask = _exterior_mask(arr, background, tolerance=48).astype(float)
    mask = gaussian_filter(mask, 5.0)[..., None]
    blended = arr * (1.0 - mask) + SHADOW * mask
    return Image.fromarray(blended.clip(0, 255).astype("uint8"))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--from",
        dest="source",
        type=Path,
        default=FACE_DIR,
        help="directory to read pristine face_*.png from (default: the asset dir itself)",
    )
    args = parser.parse_args()

    faces = sorted(args.source.glob("face_*.png"))
    if not faces:
        raise SystemExit(f"no face_*.png in {args.source}")
    for path in faces:
        reframed = frame_head_portrait(Image.open(path))
        target = FACE_DIR / path.name
        temporary = target.with_name(f".{target.name}.tmp")
        reframed.save(temporary, format="PNG")
        temporary.replace(target)
        print(f"reframed {target.relative_to(_repo_root())}")
    print(f"reframed {len(faces)} faces")


if __name__ == "__main__":
    main()
