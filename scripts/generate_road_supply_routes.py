#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["httpx", "openai", "pillow", "python-dotenv"]
# ///
"""Regenerate the in-game road-supply route board.

The committed PNG is intentionally kept unless ``--force`` is supplied: image
generation is stochastic, and the checked-in artwork is the reviewed version.
Regeneration edits the authoritative cadastral-map preview with a pinned prompt,
then normalizes the result to the reviewed texture dimensions before replacing
the destination atomically.

``OPENAI_API_KEY`` may be exported by the shell or stored in the repo-root
``.env`` file. The model is fixed to ``gpt-image-2``.

Usage:
    uv run scripts/generate_road_supply_routes.py --dry-run
    uv run scripts/generate_road_supply_routes.py --force
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import binascii
import os
import sys
from io import BytesIO
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_PATH = REPO_ROOT / "lore" / "places" / "ombreval_top_down_map_preview.png"
OUTPUT_PATH = REPO_ROOT / "assets" / "textures" / "road_supply_routes.png"

MODEL = "gpt-image-2"
DEFAULT_SIZE = "1536x1152"
DEFAULT_QUALITY = "high"
OUTPUT_WIDTH = 1448
OUTPUT_HEIGHT = 1086

PROMPT = """\
Use case: precise-object-edit
Asset type: flat in-game parchment map texture for repeated placement on route
boards near the Wool Gate, Stone Gate, Seven Lofts, and the Draper's Reach.
The supplied authoritative Ombreval cadastral-map preview is the edit target.

Create a clean, diegetic road-supply map while preserving the supplied city's
landmark placement and street geometry. Crop away the entire right-hand sidebar,
including MAP KEY, NAMED PLACE INDEX, PLAN INVENTORY, and every associated panel.
Remove the OMBREVAL masthead, cadastral subtitle, river strip, warehouse symbols,
and every header element above the city map. Remove the small numbered place-index
dots and numbers. Enlarge the city map to fill a compact landscape parchment
rectangle with a modest clean margin. There must be no external scene, wooden
frame, wall, perspective, lighting mockup, modern UI, or watermark: output only
the flat parchment artwork suitable for an in-game texture.

At the top, add a large hand-inked title reading exactly:
"ROAD SUPPLY ROUTES"

Draw two bold, hand-painted route loops over the map. They are conceptual route
summaries rather than exact street-by-street paths. Keep the map legible beneath
translucent strokes.

ORANGE ROUTE: begin outside the west wall at the existing WOOL GATE, enter the
gate, point to the existing SEVEN LOFTS compound, continue to the existing
DRAPER'S REACH, and return to WOOL GATE. Use saturated orange for every line,
arrowhead, circle, and callout border in this route. Circle WOOL GATE, SEVEN
LOFTS, and DRAPER'S REACH. Add an orange-bordered callout containing exactly:

"BREDE CART"
"WOOL GATE"
"grain + raw wool coming in"
"broadcloth going out"
"Highmarket + Fourth"

BLUE ROUTE: begin outside the south wall at the existing STONE GATE, enter the
gate, point to the same SEVEN LOFTS compound, continue to the same DRAPER'S
REACH, and return to STONE GATE. Use saturated royal blue for every line,
arrowhead, circle, and callout border in this route. Circle STONE GATE. Add a
blue-bordered callout containing exactly:

"LANTERN ROAD CART"
"STONE GATE"
"grain coming in"
"kersey going out"
"Second + Fifth"

Label the shared destinations exactly:

"SEVEN LOFTS"
"morning grain store"

"DRAPER'S REACH"
"Waning wool/cloth trade"

Add one compact timing note containing exactly:
"Dayspring arrival • Waning cloth trade • Lamplight return"

Use warm, gently weathered parchment and confident black hand lettering. Use
large, high-contrast writing readable from several in-game metres away. Keep
all callout boxes inside the parchment margins without covering the four
landmarks or the route arrows. Retain enough streets and major place labels to
orient the player, but make the orange and blue route markup visually dominant.

Do not add M5 text, a map key, a named-place index, plan inventory, an Ombreval
masthead, invented gates, invented destinations, extra routes, characters,
carts, decorative seals, unrelated symbols, or unrelated text. Spell every
supplied label exactly. The orange and blue routes must each visibly connect
their own gate to Seven Lofts and the Draper's Reach and return to their own
gate.
"""


async def generate_texture(
    *,
    api_key: str,
    source: Path,
    target: Path,
    size: str,
    quality: str,
    timeout: float,
) -> None:
    """Edit the source map and install a normalized PNG atomically."""

    from openai import AsyncOpenAI
    from PIL import Image

    client = AsyncOpenAI(api_key=api_key, timeout=timeout, max_retries=3)
    with source.open("rb") as source_image:
        response = await client.images.edit(
            model=MODEL,
            image=source_image,
            prompt=PROMPT,
            n=1,
            size=size,
            quality=quality,
            output_format="png",
        )

    if not response.data or not response.data[0].b64_json:
        raise RuntimeError("Image API response did not contain base64 image data")

    try:
        png_bytes = base64.b64decode(response.data[0].b64_json, validate=True)
    except (binascii.Error, ValueError) as error:
        raise RuntimeError("Image API returned invalid base64 data") from error
    if not png_bytes.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError("Image API response was not a PNG")

    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_name(f".{target.name}.tmp")
    try:
        with Image.open(BytesIO(png_bytes)) as generated:
            normalized = generated.convert("RGB")
            if normalized.size != (OUTPUT_WIDTH, OUTPUT_HEIGHT):
                normalized = normalized.resize(
                    (OUTPUT_WIDTH, OUTPUT_HEIGHT), Image.Resampling.LANCZOS
                )
            normalized.save(temporary, format="PNG", optimize=True)
        temporary.replace(target)
    finally:
        temporary.unlink(missing_ok=True)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        type=Path,
        default=SOURCE_PATH,
        help=f"map image to edit (default: {SOURCE_PATH.relative_to(REPO_ROOT)})",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=OUTPUT_PATH,
        help=f"where to write the PNG (default: {OUTPUT_PATH.relative_to(REPO_ROOT)})",
    )
    parser.add_argument(
        "--size",
        default=DEFAULT_SIZE,
        help=f"gpt-image-2 working size (default: {DEFAULT_SIZE})",
    )
    parser.add_argument(
        "--quality",
        default=DEFAULT_QUALITY,
        choices=["low", "medium", "high", "auto"],
        help=f"gpt-image-2 quality (default: {DEFAULT_QUALITY})",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=300.0,
        help="request timeout in seconds (default: 300)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="replace the reviewed committed texture",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print the pinned request without calling the API or writing files",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    source = args.source.resolve()
    target = args.output.resolve()

    if args.dry_run:
        print(f"model={MODEL}, size={args.size}, quality={args.quality}")
        print(f"source={source}")
        print(f"output={target} ({OUTPUT_WIDTH}x{OUTPUT_HEIGHT})")
        print("--- prompt ---")
        print(PROMPT)
        return 0

    if target.exists() and not args.force:
        print(f"kept reviewed texture {target}")
        print("pass --force to make a new stochastic edit")
        return 0
    if not source.is_file():
        print(f"source image does not exist: {source}", file=sys.stderr)
        return 1

    from dotenv import load_dotenv

    load_dotenv(REPO_ROOT / ".env")
    api_key = os.environ.get("OPENAI_API_KEY", "").strip()
    if not api_key:
        print(
            "OPENAI_API_KEY is not set (shell environment or repo-root .env)",
            file=sys.stderr,
        )
        return 1

    print(
        f"editing {source} with model={MODEL}, size={args.size}, "
        f"quality={args.quality}",
        flush=True,
    )
    try:
        asyncio.run(
            generate_texture(
                api_key=api_key,
                source=source,
                target=target,
                size=args.size,
                quality=args.quality,
                timeout=args.timeout,
            )
        )
    except Exception as error:  # noqa: BLE001 - report and preserve prior asset
        print(f"failed: {error}", file=sys.stderr)
        return 1

    print(
        f"wrote {target} ({OUTPUT_WIDTH}x{OUTPUT_HEIGHT})",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
