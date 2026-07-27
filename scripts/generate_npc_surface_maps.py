"""Derive the two surface assets the puppet meshes need from the artwork we
already have (`features/npc_bodies.md` §2 — bodies are textured like every
other surface in the project).

Both outputs are *derived*, not authored: this script needs no API key and is
deterministic, so a regeneration of the source artwork can simply be followed
by a re-run.

1. **Cloth normal maps** — `outfit_<class>_normal.png`, one per outfit texture.
   The albedo weave is treated as a height field (blurred luminance) and
   differentiated into a tangent-space normal map, so sunlight rakes across the
   fabric instead of sliding over a flat decal. This is most of what separates
   "a painted box" from "a clothed body" at 3 m.

2. **The skin-tone table** — the puppet's neck and hands are untextured
   geometry that has to match whichever painted face the actor drew, so this
   samples the face artwork (forehead + both cheeks + chin, medianed so a
   shadow or a beard cannot drag the tone) and prints a ready-to-paste Rust
   `const` table for `src/smart_actors/body.rs`.

    uv run scripts/generate_npc_surface_maps.py            # both outputs
    uv run scripts/generate_npc_surface_maps.py --skin     # just print the table
"""

# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pillow", "scipy"]
# ///

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import gaussian_filter

OUTFIT_CLASSES = [
    "cleric",
    "merchant",
    "craftsman",
    "laborer",
    "notable",
    "poor",
    "watch",
]
FACE_COUNT = 24

#: Height-field blur (texels) before differentiating. Below this the weave is
#: photographic noise and the normal map turns into sparkle.
HEIGHT_BLUR_TEXELS = 1.6
#: Metres one cloth tile spans in-world; the slope scale below is expressed
#: against it so the relief is physical, not "whatever looked fine at 1024².
CLOTH_TILE_M = 0.35
#: Peak relief of the weave in metres (~1 mm of thread stand-off).
CLOTH_RELIEF_M = 0.0011

#: Where to sample skin on a `reframe_faces.py`-framed portrait, as (x, y)
#: fractions of the image: forehead, both cheeks, chin. The head is cropped
#: tight and centred there, so these land on skin for every face.
SKIN_PATCHES = [(0.50, 0.26), (0.34, 0.55), (0.66, 0.55), (0.50, 0.76)]
SKIN_PATCH_FRACTION = 0.055


def repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in (here, *here.parents):
        if (parent / ".git").exists():
            return parent
    return here.parent


NPC_DIR = repo_root() / "assets" / "textures" / "npc"


def normal_map(albedo: Image.Image) -> Image.Image:
    """Tangent-space normal map from the albedo's luminance as a height field.

    Wraps at the edges (`np.gradient` on a rolled array) because the cloth
    textures tile, and a seam in the normal map would draw a hard line across
    every torso.
    """
    rgb = np.asarray(albedo.convert("RGB")).astype(np.float64) / 255.0
    height = rgb @ np.array([0.2126, 0.7152, 0.0722])
    height = gaussian_filter(height, HEIGHT_BLUR_TEXELS, mode="wrap")
    # Normalise so every class gets comparable relief regardless of how
    # contrasty its weave photograph happens to be.
    spread = height.max() - height.min()
    if spread > 1e-6:
        height = (height - height.min()) / spread

    size = height.shape[0]
    texel_m = CLOTH_TILE_M / size
    # Central differences across the wrap.
    d_x = (np.roll(height, -1, axis=1) - np.roll(height, 1, axis=1)) * 0.5
    d_y = (np.roll(height, -1, axis=0) - np.roll(height, 1, axis=0)) * 0.5
    slope = CLOTH_RELIEF_M / texel_m
    # Bevy/glTF tangent space: +X right, +Y up, +Z out. Image rows run down,
    # so the vertical derivative is negated.
    normals = np.stack([-d_x * slope, d_y * slope, np.ones_like(height)], axis=-1)
    normals /= np.linalg.norm(normals, axis=-1, keepdims=True)
    encoded = ((normals * 0.5 + 0.5) * 255.0).clip(0, 255).astype("uint8")
    return Image.fromarray(encoded, mode="RGB")


def skin_tone(face: Image.Image) -> tuple[float, float, float]:
    """The actor's skin colour, medianed over four patches of the portrait."""
    arr = np.asarray(face.convert("RGB")).astype(np.float64) / 255.0
    size = arr.shape[0]
    half = max(2, round(size * SKIN_PATCH_FRACTION * 0.5))
    samples = []
    for fx, fy in SKIN_PATCHES:
        x, y = round(fx * size), round(fy * size)
        patch = arr[y - half : y + half, x - half : x + half].reshape(-1, 3)
        samples.append(np.median(patch, axis=0))
    tone = np.median(np.stack(samples), axis=0)
    return tuple(float(channel) for channel in tone)


def write_normal_maps() -> None:
    for outfit_class in OUTFIT_CLASSES:
        source = NPC_DIR / f"outfit_{outfit_class}.png"
        if not source.exists():
            print(f"  skip {source.name}: not found")
            continue
        target = NPC_DIR / f"outfit_{outfit_class}_normal.png"
        normal_map(Image.open(source)).save(target, format="PNG", optimize=True)
        print(f"  wrote {target.relative_to(repo_root())}")


def print_skin_table() -> None:
    tones = []
    for index in range(FACE_COUNT):
        path = NPC_DIR / f"face_{index:02d}.png"
        if not path.exists():
            raise SystemExit(f"missing {path}")
        tones.append(skin_tone(Image.open(path)))

    print()
    print("/// Skin tone of each painted face, sampled from the portrait's")
    print("/// forehead, cheeks and chin by `scripts/generate_npc_surface_maps.py`.")
    print("/// The neck and hands are untextured geometry, so they take their")
    print("/// colour from here and match whichever face the actor drew.")
    print(f"const FACE_SKIN_TONES: [[f32; 3]; FACE_COUNT] = [")
    for index, (r, g, b) in enumerate(tones):
        print(f"    [{r:.3f}, {g:.3f}, {b:.3f}], // face_{index:02d}")
    print("];")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skin", action="store_true", help="only print the skin table")
    parser.add_argument("--normals", action="store_true", help="only write normal maps")
    args = parser.parse_args()
    both = not args.skin and not args.normals

    if both or args.normals:
        print("cloth normal maps:")
        write_normal_maps()
    if both or args.skin:
        print_skin_table()


if __name__ == "__main__":
    main()
