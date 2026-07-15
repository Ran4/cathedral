#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["httpx", "openai", "python-dotenv"]
# ///
"""Generate the "map of the world as Ombreval knows it".

Ombreval already has a top-down *city* map (``lore/places/ombreval_top_down_map.svg``).
This script draws the complementary *wider world* map: the river Serle from the
Combs to the sea, the towns strung along it (Brede, Ombreval, Harne, Salorge),
and far-off Ostrelle at the end of the Lantern Road. Everything on it is canon
drawn only from the lore, and only from what the city could actually know — the
world beyond the walls is secondhand in Ombreval and the map stays that way
(``lore/second_sun/12_beyond_the_walls.md``, ``lore/core_lore/setting_and_geography.md``).

Canon sources folded into the prompt:
- The Serle rises in the Combs (sheep-hills, west), keeps one name "from the
  Combs to the sea"; nobody renames water.
- Brede: wool-stack town, head of navigation, three days' pole upstream (west).
  Reached from the city by the Wool Gate.
- Ombreval: the free walled cathedral-city; the Serle runs beyond its south wall,
  freight lands at outer wharves. The map's centre.
- Harne: a lord's town four leagues downstream (east), castle over the river, a
  chain once thrown across it. Reached by the Harne Gate.
- Salorge: the river mouth among the salt-pans, six days down / twelve back.
- Ostrelle: the far primatial capital, six weeks' carrying by the Lantern Road
  through two toll-gates; the Lantern Road leaves north (from the Stone Gate).
  Ombreval imagines its cathedral as "the Lanthorn drawn again, smaller".
- One sun everywhere: nothing like the impossible light shows beyond the walls.

``OPENAI_API_KEY`` may be exported in the shell or stored in the repo-root
``.env`` file. The image model is deliberately fixed to ``gpt-image-2`` to match
``scripts/generate_lore_inspiration_images.py``.

Usage:
    uv run scripts/map_of_the_world.py
    uv run scripts/map_of_the_world.py --quality high --size 1536x1024
    uv run scripts/map_of_the_world.py --dry-run        # print the prompt only
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import binascii
import os
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = REPO_ROOT / "lore" / "places" / "world_map.png"

MODEL = "gpt-image-2"
DEFAULT_SIZE = "1536x1024"  # landscape: the river runs the long way, west -> east
DEFAULT_QUALITY = "high"

# A single, carefully composed prompt. Register: an antique, hand-drawn pilgrim's
# / boatman's chart, aged and parchment-toned — Ombreval's own short map of the
# world, river-centric because "the boat-folk think of the world as one water
# with many tolls on it." Realistic medieval world, one sun, no magic shown.
PROMPT = """\
An antique hand-drawn map on aged, stained parchment: "the world as the city of \
Ombreval knows it" — a medieval boatman's-and-pilgrim's chart, not a modern map. \
Muted sepia and iron-gall-ink browns with faded ochre, dull red, and moss-green \
washes; worn edges, a few water stains and creases; hand-lettered labels in an \
old blackletter-influenced hand. A decorative title cartouche at the top reads \
"THE SERLE, FROM THE COMBS TO THE SEA". A small compass rose with a fleur-de-lis \
pointing north sits in a corner. A faint scale bar labelled in "days' pole" and \
"leagues". A single ordinary sun in the sky (nothing strange, no green or \
doubled light anywhere).

The whole map is organised around one great river, the SERLE, drawn as a bold \
winding blue-grey ribbon running the long way across the sheet from the upper \
LEFT (WEST) down to the lower RIGHT (EAST), where it widens and empties into an \
open SEA at the right edge dotted with a small sailing barge or two. The river \
keeps a single label, "the Serle", repeated a couple of times along its length.

From west (left) to east (right), place and label these settlements as little \
pictorial map-vignettes:

- Far WEST, at the river's source: "THE COMBS" — rolling bare sheep-hills with \
tiny grazing sheep, a thin spring-trickle becoming the river. A note in small \
script: "where the Serle rises".

- Upriver, WEST of the city: "BREDE" — a small wool town of stacked wool bales \
and a timber quay, marked as the head of navigation (last boats). A note: \
"three days' pole upstream".

- CENTRE, the largest and most detailed vignette: "OMBREVAL" — a free walled \
cathedral-city. Its ring of stone walls is punctuated by SQUARE, boxy \
gatehouses and SQUARE rectangular wall-towers with flat crenellated tops — the \
city's gates and mural towers are square-plan blocks, NOT round turrets or \
cylindrical towers. Draw a couple of clear square gate-towers straddling the \
roads where they pierce the wall. Inside the walls stands a great cathedral \
with twin west towers and a round rose window (the Lanthorn). The river runs \
just OUTSIDE its south wall with little wharves and moored barges drawn beyond \
the wall (no river inside the walls). This is clearly the heart of the map, \
drawn bigger and finer than everything else.

- Downriver, EAST of the city: "HARNE" — a lord's town with a castle on a bluff \
above the river and an iron chain drawn stretched across the water below the \
castle. A note: "four leagues below".

- Far EAST, at the river mouth by the sea: "SALORGE" — flat coastal salt-pans \
drawn as a grid of shallow evaporation ponds with little heaps of white salt \
and salt-barges. A note: "six days down".

Away from the river, toward the TOP of the sheet (NORTH), draw a long overland \
road leaving the region as a dashed/dotted track labelled "THE LANTERN ROAD", \
passing through two small drawn gatehouses each labelled "toll-gate", and ending \
at the far top edge at a distant, deliberately SMALL city-vignette labelled \
"OSTRELLE" — drawn as a miniature echo of Ombreval's own cathedral (the same \
towers and rose window, but tiny and faint, as a far capital the city has never \
seen). A note beside it: "six weeks' carrying".

Optional period map flourishes: faint hatched hills, a few tiny trees and \
sheep, a small sea-ship, curling banner labels. Keep every place name clearly \
legible. Do NOT include any modern elements, and do NOT depict any supernatural \
light, second sun, or glowing window — the wider world has one plain sun and \
knows nothing stranger.\
"""


async def generate_map(
    *,
    api_key: str,
    target: Path,
    size: str,
    quality: str,
    timeout: float,
) -> None:
    """Generate one PNG with gpt-image-2 and install it atomically at target."""

    from openai import AsyncOpenAI

    client = AsyncOpenAI(api_key=api_key, timeout=timeout, max_retries=3)

    response = await client.images.generate(
        model=MODEL,
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
    temporary.write_bytes(png_bytes)
    temporary.replace(target)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=OUTPUT_PATH,
        help=f"where to write the PNG (default: {OUTPUT_PATH.relative_to(REPO_ROOT)})",
    )
    parser.add_argument(
        "--size",
        default=DEFAULT_SIZE,
        help=f"gpt-image-2 output size (default: {DEFAULT_SIZE})",
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
        help="per-request timeout in seconds (default: 300)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print the prompt and the resolved output path, then exit",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    target: Path = args.output.resolve()

    if args.dry_run:
        print(f"model={MODEL}, size={args.size}, quality={args.quality}")
        print(f"output={target}")
        print("--- prompt ---")
        print(PROMPT)
        return 0

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
        f"generating world map with model={MODEL}, size={args.size}, "
        f"quality={args.quality}",
        flush=True,
    )
    try:
        asyncio.run(
            generate_map(
                api_key=api_key,
                target=target,
                size=args.size,
                quality=args.quality,
                timeout=args.timeout,
            )
        )
    except Exception as error:  # noqa: BLE001 - report and fail cleanly
        print(f"failed: {error}", file=sys.stderr)
        return 1

    print(f"wrote {target}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
