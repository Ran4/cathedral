#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["openai", "python-dotenv", "pillow"]
# ///
"""Generate the cloth material textures (laundry linen, awning canvas).

Same gpt-image-2 pipeline as generate_lore_inspiration_images.py, but for
1024x1024 tiling material artwork in assets/textures/. The model does not
guarantee a perfectly tileable result, so each texture is post-processed with
the offset-and-blend trick: roll by half a tile (moving the border seams to a
centre cross), then feather the original — seamless at its own centre — back
over that cross. The result tiles cleanly in both axes.
"""

from __future__ import annotations

import base64
from io import BytesIO
from pathlib import Path
import tempfile

from PIL import Image


def _repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in (here, *here.parents):
        if (parent / ".git").exists():
            return parent
    return here.parent


REPO_ROOT = _repo_root()
OUTPUT_DIR = REPO_ROOT / "assets" / "textures"
MODEL = "gpt-image-2"
SIZE = "1024x1024"
QUALITY = "high"

COMMON = (
    "Seamless tileable material texture, photographed perfectly flat-on, even diffuse "
    "overcast lighting, no shadows, no vignette, no border, the weave fills the entire "
    "frame edge to edge at consistent scale. Muted late-medieval palette matching aged "
    "plaster and timber. No text, no objects, no folds of a finished garment - only the "
    "flat woven material surface."
)

TEXTURES = {
    "ombreval_linen.png": (
        "Hand-woven undyed linen cloth for washing lines: fine tabby weave, sun-bleached "
        "off-white with faint warm grey streaks, slight slubs and irregular threads, a few "
        "pale water stains and thin worn patches where the weave shows lighter. " + COMMON
    ),
    "ombreval_canvas.png": (
        "Heavy hemp canvas for market awnings: coarse plain weave with visible thick warp "
        "threads, natural ecru tone with sun-faded blotches, light grime along the weave "
        "lines, one or two subtle hand-stitched repair seams lying flat. " + COMMON
    ),
}


def make_seamless(image: Image.Image, feather: int = 96) -> Image.Image:
    """Hide border seams: roll half a tile, feather the original over the seam cross."""

    width, height = image.size
    rolled = Image.new("RGB", (width, height))
    rolled.paste(image, (-width // 2, -height // 2))
    rolled.paste(image, (width // 2, -height // 2))
    rolled.paste(image, (-width // 2, height // 2))
    rolled.paste(image, (width // 2, height // 2))

    mask = Image.new("L", (width, height), 0)
    pixels = mask.load()
    for y in range(height):
        for x in range(width):
            # Weight peaks on the centre cross (the rolled image's seam) and
            # falls to zero within `feather` px, so the borders stay untouched.
            dx = abs(x - width // 2)
            dy = abs(y - height // 2)
            distance = min(dx, dy)
            if distance < feather:
                pixels[x, y] = int(255 * (1.0 - distance / feather))
    return Image.composite(image, rolled, mask)


def tiling_contact_sheet(image: Image.Image) -> Image.Image:
    """2x2 repeat for eyeballing seam quality."""

    width, height = image.size
    sheet = Image.new("RGB", (width * 2, height * 2))
    for offset_x in (0, width):
        for offset_y in (0, height):
            sheet.paste(image, (offset_x, offset_y))
    return sheet.resize((width, height), Image.LANCZOS)


def main() -> None:
    from dotenv import load_dotenv
    from openai import OpenAI

    load_dotenv(REPO_ROOT / ".env")
    client = OpenAI()
    for name, prompt in TEXTURES.items():
        target = OUTPUT_DIR / name
        response = client.images.generate(
            model=MODEL,
            prompt=prompt,
            n=1,
            size=SIZE,
            quality=QUALITY,
            output_format="png",
        )
        if not response.data or not response.data[0].b64_json:
            raise RuntimeError(f"no image data for {name}")
        raw = Image.open(BytesIO(base64.b64decode(response.data[0].b64_json)))
        seamless = make_seamless(raw.convert("RGB"))
        temporary = target.with_name(f".{target.name}.tmp")
        seamless.save(temporary, format="PNG")
        temporary.replace(target)
        sheet_path = Path(tempfile.gettempdir()) / (target.stem + "_tiling_preview.png")
        tiling_contact_sheet(seamless).save(sheet_path, format="PNG")
        print(f"wrote {target} (tiling preview: {sheet_path})")


if __name__ == "__main__":
    main()
