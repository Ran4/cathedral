#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["httpx", "openai", "python-dotenv"]
# ///
"""Storyboards for the ten routes through *Forty Pounds Over*.

One landscape four-panel board per playthrough in ``PLAYTHROUGHS.md``. Existing
PNGs are kept so an interrupted run resumes; pass ``--force`` to replace, or
``--only 03 07`` to regenerate a subset.

``OPENAI_API_KEY`` comes from the shell, the repo-root ``.env`` or
``prompt_playgound/.env``. The model is fixed to ``gpt-image-2``, matching
``scripts/generate_lore_inspiration_images.py``.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import binascii
import os
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUTPUT_DIR = Path(__file__).resolve().parent / "storyboards"
MODEL = "gpt-image-2"
DEFAULT_SIZE = "1536x1024"
DEFAULT_QUALITY = "medium"

# Shared art direction. Kept close to the lore-image generator's register so the
# boards read as the same project, but pushed toward ink-and-wash storyboarding:
# these are production documents, not concept art.
STYLE = (
    "Monochrome sepia ink-and-wash storyboard in the manner of a period engraving: "
    "confident pen line, cross-hatched shadow, restrained tonal washes, no colour "
    "beyond warm sepia and paper cream. Late-medieval northern European city, "
    "fictional year F.437."
)
MATERIALS = (
    "dry worn limestone and fieldstone, dusty lime plaster, exposed oak framing, "
    "rough cloth, rope, iron, cart-worn timber, terracotta and slate roofs, packed "
    "earth, dry paving, soot and visible hand construction"
)
HARD_RULES = (
    "Hard constraints: the Cut is a filled-in former channel, so there is NO water, "
    "canal, river, boat or mist anywhere in any panel. No modern objects, no "
    "printing, no firearms, no signage in modern type. Ordinary working people in "
    "wool and linen, not fantasy costume. No magic, no glow, no supernatural light."
)
LAYOUT = (
    "Layout: exactly four panels of equal size in a single horizontal row, left to "
    "right, separated by thin ruled ink borders, each panel numbered with a small "
    "handwritten numeral 1, 2, 3, 4 in its top-left corner. Beneath each panel a "
    "narrow cream caption strip carries its short caption in small neat handwritten "
    "capitals. Keep captions to the exact words given and spell them correctly."
)


@dataclass(frozen=True)
class Board:
    number: str
    slug: str
    title: str
    panels: tuple[tuple[str, str], ...]  # (caption, description)


BOARDS: tuple[Board, ...] = (
    Board(
        "01", "the_proved_weights", "The Proved Weights",
        (
            ("TURNED AT THE GATE",
             "Noon at a fortified city gate beside a great weigh-beam. A loaded ox cart "
             "is being turned around by customs officers; a corded cloth bale sits on the "
             "beam's pan. A gathering crowd. A spare, courteous trader in a travelling "
             "coat stands rigid with disbelief while a carter holds the ox's head."),
            ("THE BEAM SAID OTHERWISE",
             "A customs square: an elderly official weigher standing at a chained "
             "weigh-beam, speaking aloud with his chin up and his eyes clouded and "
             "unfocused, while a young woman clerk beside him reads the figure off the "
             "beam for him. Porters and merchants listening at a stand nearby."),
            ("A MILLER'S OWN WEIGHTS",
             "A broad, loud miller white with flour hauls a small chest of sealed brass "
             "weights on a handbarrow through a dusty street toward the customs square, "
             "laughing; a dry dusty notary in dark clothes walks beside him carrying a "
             "fee book, entirely unamused."),
            ("THE DUTY PAID",
             "Evening. A notary's counter by lamplight: an amended manifest being signed "
             "and witnessed, coins counted out in a small stack, the cloth bale visible "
             "behind under an officer's hand. A grey-bearded draper watches from the edge "
             "of the light with a very still face."),
        ),
    ),
    Board(
        "02", "three_in_one_room", "Three in One Room",
        (
            ("THREE ACCOUNTS",
             "A triptych-like single panel: three separate small vignettes of three "
             "people speaking in three different places — a trader at a gate, a young "
             "charming broker laughing on a stone overhead bridge, a woman weaver in her forties "
             "at a loom in a jettied upper room."),
            ("A YARD WITH TWO GATES",
             "A walled bonded weighing yard at dusk with two cart gates on different "
             "lanes, empty of carts, poor sightlines, deep shadow. A figure stands in the "
             "middle of it having chosen the place deliberately."),
            ("THE STORIES DO NOT STAND",
             "Inside that yard: three people confronting one another in raised argument — "
             "an older trader, a young broker with his hands open in protest, and a "
             "woman weaver in her forties standing straight and level. A fourth figure, the player, "
             "watches from the side."),
            ("HE RUNS",
             "Night. The young broker running away down a narrow lane between overhanging "
             "upper storeys, glancing back; a huge cheerful carter standing in the lane "
             "mouth watching him go and no longer protecting him."),
        ),
    ),
    Board(
        "03", "the_bolt_that_was_never_there", "The Bolt That Was Never There",
        (
            ("A DOOR AND A KEY",
             "Night. A heavy studded warehouse door under a stone overhead bridge, a "
             "single lantern, a hand turning a large iron key. Nobody else in the lane."),
            ("THE CORD CUT",
             "Warehouse interior by lantern light, stacked bales and casks. A figure "
             "kneeling over a corded cloth bale, cutting the cord with a small knife, one "
             "heavy bolt of cloth already drawn half out."),
            ("IT MATCHES",
             "Morning. A public opening before a crowd at the warehouse door: officers "
             "cutting the cord, counting bolts out onto trestles, a clerk reading a "
             "manifest aloud. The count agrees. Faces are surprised rather than "
             "triumphant."),
            ("FORTY POUNDS TO CARRY",
             "A market square in daylight, stalls and awnings. A lone figure carrying an "
             "awkward heavy roll of cloth on one shoulder, looking sideways at a draper's "
             "arcade across the square. Nowhere to put it."),
        ),
    ),
    Board(
        "04", "a_better_forgery", "A Better Forgery",
        (
            ("SIX SPARKS OF WAX",
             "A chandler's pitch in a busy market: blocks of wax, tallow, wicking, a "
             "lean-to over a stone hot-work island. A figure buying wax while an enormous "
             "cheerful carter helpfully carries it for them, beaming."),
            ("PRESSED CLEAN",
             "Night, warehouse interior. Hands pressing a seal into hot wax on a cloth "
             "bale's cord, a candle close by, the work careful and neat."),
            ("HE LOOKS TWICE",
             "Morning, the public opening. A dusty precise notary in dark clothes holding "
             "the broken seal up close to his eye before a crowd, speaking flatly. "
             "Officers turning to look. The accused trader's face changing."),
            ("THE WAX REMEMBERED",
             "The same crowd. The big carter, asked a question, pointing with an open "
             "honest hand at the player. Officers moving toward the player. The trader is "
             "no longer the one being watched."),
        ),
    ),
    Board(
        "05", "skell_buys_it", "Skell Buys It",
        (
            ("THE DRAPER'S REACH",
             "A cloth merchants' arcade: bolts of woollen cloth stacked deep, tenter "
             "frames beyond, a grey-bearded master draper measuring a bolt against a "
             "brass standard ell with his own hands, unhurried."),
            ("HE WILL BUY IT",
             "The same arcade. The draper naming a price with one hand flat on the cloth, "
             "the player opposite. The draper's face is courteous and about a finger short "
             "of kind."),
            ("WHOSE LOOM",
             "Close view across a counter: the draper asking one quiet question. Behind "
             "him a clerk with a ledger has stopped writing to listen."),
            ("BACK WHERE SHE STARTED",
             "A jettied upper weaving room over a narrow lane, four looms. A woman weaver "
             "working, her face level and closed, saying nothing. Through the window "
             "below, the draper's cart collecting cloth."),
        ),
    ),
    Board(
        "06", "six_households_at_the_door", "Six Households at the Door",
        (
            ("SIX UNPAID LOOMS",
             "Narrow weaving lanes at dusk, jettied upper storeys, looms visible through "
             "shutters. The player speaking to weavers in a doorway; another weaver "
             "listening from an upper window."),
            ("THEY COME",
             "Night. Forty working people with lanterns filling a lane before a bonded "
             "warehouse door. Not a riot — set faces, arms folded, one woman speaking for "
             "them. Two officers on the step."),
            ("OPENED BADLY",
             "The door opened by lamplight, the crowd pressing in, a cloth bale dragged "
             "out and its cord slashed and trampled underfoot, bolts pulled out into many "
             "hands, wax broken in pieces on the ground."),
            ("NOTHING LEFT TO PROVE IT",
             "Morning after. The empty warehouse doorway, trampled cord and broken wax in "
             "the dust, one officer holding the accused trader by the arm and walking him "
             "away up the street."),
        ),
    ),
    Board(
        "07", "the_penny_and_the_fright", "The Penny and the Fright",
        (
            ("A PENNY A MESSAGE",
             "A narrow lane. A ragged, quick, cheeky eleven-year-old message-runner with "
             "one arm healed crooked, holding out a hand for a coin, talking fast. The "
             "player crouched to her height."),
            ("SHE NAMES BOTH ENDS",
             "The same lane, the child gesturing rapidly — one hand toward the weaving "
             "quarter, one toward the bridge and the customs square — the player "
             "listening."),
            ("OR A RAISED VOICE",
             "The same lane, gone wrong: the player standing over the child with a raised "
             "hand or an officer at their shoulder. The child has gone completely quiet "
             "and still, eyes down, arms at her sides."),
            ("A DOOR NOBODY SEES",
             "Later. The child watching the player pass from a gap between buildings that "
             "no grown body would fit into, saying nothing at all."),
        ),
    ),
    Board(
        "08", "sold_to_the_man_across_the_square", "Sold to the Man Across the Square",
        (
            ("THE MONEY BROKER",
             "A money-dealer's counter at the toll-house end of a stone overhead bridge: "
             "a cold exact man of fifty behind an unblemished ledger, scales and a "
             "strongbox behind him, the player leaning in to say something quietly."),
            ("A KEY IS A ROUTE",
             "Close view: a large iron warehouse key changing hands over the counter "
             "instead of coins. Both hands are steady."),
            ("TAKEN AT THE OPENING",
             "Morning, the public opening. The extra bolt of cloth held up before a crowd; "
             "the innocent trader taken in charge by two officers, his face rigid; the "
             "player standing in the crowd, not intervening."),
            ("ONE FEWER CART",
             "A city gate on a later market morning. The gate stands open on an empty "
             "road; two carters with no cart sit on their packs beside the wall. Nothing "
             "arrives."),
        ),
    ),
    Board(
        "09", "late_to_the_opening", "Late to the Opening",
        (
            ("ELSEWHERE, ALL DAY",
             "The player deep in an unrelated part of the city — a crowded market street, "
             "back turned to a distant gate — while the day's light goes."),
            ("THE CORD IS CUT",
             "Morning. The public opening in full: officers cutting the cord before a "
             "crowd, bolts counted onto trestles, a clerk calling a figure, a notary "
             "examining broken wax."),
            ("TAKEN IN CHARGE",
             "The trader taken in charge and walked away toward a squat stone gaol with a "
             "single iron-bound door, the crowd parting."),
            ("A BETTER QUESTION",
             "The player at a hired writer's board on a bridge, having a custody copy read "
             "out to them, one finger on the hour that is missing from it."),
        ),
    ),
    Board(
        "10", "nobody_came", "Nobody Came",
        (
            ("A COMMOTION, IGNORED",
             "The player walking away down a side lane while, far behind at the end of the "
             "street, a small crowd forms around a turned cart at a gate. The player does "
             "not look back."),
            ("OPENED WITHOUT YOU",
             "The public opening, seen at a distance over heads and roofs: bolts on "
             "trestles, an officer's arm raised, nobody the player knows."),
            ("SHE WAITS FOR IT",
             "A weaving room. A woman weaver at her loom looking down into an empty street "
             "where a cart never came, then going back to work."),
            ("THREE MARKETS LATER",
             "A market morning at the same gate, months on. The gate is open and ordinary "
             "and there is no cart there at all, and nobody remarks on it."),
        ),
    ),
)


def build_prompt(board: Board) -> str:
    panels = "\n".join(
        f"Panel {i}. Caption: \"{caption}\". Scene: {description}"
        for i, (caption, description) in enumerate(board.panels, start=1)
    )
    return (
        f"{STYLE}\n\n"
        f"A four-panel production storyboard titled \"{board.title}\", showing one "
        f"route a player takes through an investigation in a walled medieval trading "
        f"city built around a customs square, its weigh-beams and its bonded "
        f"warehouses.\n\n"
        f"{LAYOUT}\n\n{panels}\n\n"
        f"Materials throughout: {MATERIALS}.\n\n{HARD_RULES}"
    )


async def generate(client, board: Board, target: Path, *, size: str, quality: str) -> None:
    response = await client.images.generate(
        model=MODEL,
        prompt=build_prompt(board),
        n=1,
        size=size,
        quality=quality,
        output_format="png",
    )
    if not response.data or not response.data[0].b64_json:
        raise RuntimeError(f"{board.slug}: response carried no base64 image data")
    try:
        png = base64.b64decode(response.data[0].b64_json, validate=True)
    except (binascii.Error, ValueError) as error:
        raise RuntimeError(f"{board.slug}: invalid base64") from error
    if not png.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError(f"{board.slug}: response was not a PNG")
    temporary = target.with_name(f".{target.name}.tmp")
    temporary.write_bytes(png)
    temporary.replace(target)
    print(f"  wrote {target.relative_to(REPO_ROOT)} ({len(png) // 1024} KiB)", flush=True)


async def run(jobs: list[tuple[Board, Path]], *, api_key: str, size: str, quality: str) -> int:
    from openai import AsyncOpenAI

    client = AsyncOpenAI(api_key=api_key, timeout=600.0, max_retries=3)
    failures = 0
    try:
        # Four at a time: enough to keep the endpoint busy, gentle enough that a
        # rate-limit reply does not cascade the whole batch into retries.
        semaphore = asyncio.Semaphore(4)

        async def one(board: Board, target: Path) -> None:
            nonlocal failures
            async with semaphore:
                try:
                    await generate(client, board, target, size=size, quality=quality)
                except Exception as error:  # noqa: BLE001 — report and continue
                    failures += 1
                    print(f"  FAILED {board.number} {board.slug}: {error}", file=sys.stderr)

        await asyncio.gather(*(one(board, target) for board, target in jobs))
    finally:
        await client.close()
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--force", action="store_true", help="replace existing PNGs")
    parser.add_argument("--only", nargs="*", metavar="NN", help="board numbers to generate")
    parser.add_argument("--size", default=DEFAULT_SIZE)
    parser.add_argument("--quality", default=DEFAULT_QUALITY, choices=["low", "medium", "high"])
    parser.add_argument("--dry-run", action="store_true", help="print prompts, call nothing")
    args = parser.parse_args()

    from dotenv import load_dotenv

    load_dotenv(REPO_ROOT / ".env")
    load_dotenv(REPO_ROOT / "prompt_playgound" / ".env")

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    selected = [b for b in BOARDS if not args.only or b.number in args.only]
    if not selected:
        print("nothing selected", file=sys.stderr)
        return 1

    if args.dry_run:
        for board in selected:
            print(f"===== {board.number} {board.slug}\n{build_prompt(board)}\n")
        return 0

    jobs: list[tuple[Board, Path]] = []
    for board in selected:
        target = OUTPUT_DIR / f"{board.number}_{board.slug}.png"
        if target.exists() and not args.force:
            print(f"  keeping {target.name}")
            continue
        jobs.append((board, target))
    if not jobs:
        print("all boards already present; --force to replace")
        return 0

    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        print("OPENAI_API_KEY is not set", file=sys.stderr)
        return 1

    print(f"generating {len(jobs)} board(s), model={MODEL}, size={args.size}, quality={args.quality}")
    failures = asyncio.run(run(jobs, api_key=api_key, size=args.size, quality=args.quality))
    if failures:
        print(f"{failures} board(s) failed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
