#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["openai", "python-dotenv", "pillow", "numpy", "scipy"]
# ///
"""Generate NPC body textures for features/npc_bodies.md (M0).

Two families under assets/textures/npc/:

* outfit_<class>.png - seven seamless 1024x1024 tiling cloth textures, one per
  OutfitClass (Cleric | Merchant | Craftsman | Laborer | Watch | Notable | Poor),
  same gpt-image-2 pipeline + offset-and-blend seamless post-process as
  generate_cloth_textures.py.
* face_00.png .. face_23.png - 24 distinct painted faces, each a head centred on
  a plain skin-tone background. Generated at 1024 quality "medium", downscaled to
  256x256, then reframed by reframe_faces.frame_head_portrait (cropped tight to
  the head, the background faded to a shaded periphery) so the head-sphere
  projection reads as a head instead of a small face on a pale ball. NOT run
  through the seamless pass (a face must not be rolled).

Existing outputs are skipped so an interrupted run resumes; pass --force to
regenerate everything. If the API rejects gpt-image-2 the script falls back to
gpt-image-1 and says so. Each image is retried once on transient failure; a
face that still fails is skipped and reported at the end.
"""

from __future__ import annotations

import argparse
import base64
import os
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
from reframe_faces import frame_head_portrait  # noqa: E402  (sibling script, shared reframing)


def _repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in (here, *here.parents):
        if (parent / ".git").exists():
            return parent
    return here.parent


REPO_ROOT = _repo_root()
OUTPUT_DIR = REPO_ROOT / "assets" / "textures" / "npc"
PRIMARY_MODEL = "gpt-image-2"
FALLBACK_MODEL = "gpt-image-1"
SIZE = "1024x1024"
FACE_SAVE_SIZE = 256
FACE_COUNT = 24

_model = PRIMARY_MODEL
_model_lock = threading.Lock()
_print_lock = threading.Lock()

CLOTH_COMMON = (
    "Seamless tileable material texture, photographed perfectly flat-on, even diffuse "
    "overcast lighting, no shadows, no vignette, no border, the weave fills the entire "
    "frame edge to edge at consistent scale. Muted late-medieval palette matching aged "
    "plaster and timber, desaturated engraving-adjacent tones. No text, no objects, no "
    "folds of a finished garment - only the flat woven material surface."
)

CLOTHS: dict[str, str] = {
    "outfit_cleric.png": (
        "Dark wool cassock cloth for a cleric: dense fine twill weave in near-black "
        "charcoal-brown wool, matte with a slightly worn nap, faint dust caught along the "
        "weave lines, sober and unadorned. " + CLOTH_COMMON
    ),
    "outfit_merchant.png": (
        "Fine dyed wool broadcloth for a prosperous merchant: tight even weave with a "
        "soft low sheen, deep muted madder-and-murrey tone with subtle tonal richness, "
        "gentle dye variation across the cloth, well kept. " + CLOTH_COMMON
    ),
    "outfit_craftsman.png": (
        "Sturdy craftsman's work cloth: medium-weight wool twill in muted grey-brown, "
        "hard-wearing and slightly abraded, with one or two flat stitched leather trim "
        "reinforcement strips lying flush with the weave. " + CLOTH_COMMON
    ),
    "outfit_laborer.png": (
        "Coarse undyed homespun wool for a laborer: loose uneven tabby weave, natural "
        "oatmeal and ash-grey fleece tones mixed thread by thread, slubs, small burrs "
        "and thin worn patches where the weave opens. " + CLOTH_COMMON
    ),
    "outfit_watch.png": (
        "Padded gambeson weave for the town watch: vertically quilted linen-canvas "
        "channels with flat stitched seams, dull ash-grey fabric, slight batting swell "
        "between the quilting lines, scuffed and serviceable. " + CLOTH_COMMON
    ),
    "outfit_notable.png": (
        "Rich wool cloth for a town notable, brocade-adjacent: fine dense weave carrying "
        "a subtle woven damask figure in tone-on-tone dark murrey and umber, restrained "
        "and heavy, still muted and desaturated. " + CLOTH_COMMON
    ),
    "outfit_poor.png": (
        "Patched threadbare sackcloth for the very poor: coarse jute-like weave worn "
        "thin and faded to grey-brown, several flat mismatched patches crudely stitched "
        "on, frayed threads lying flat, old stains. " + CLOTH_COMMON
    ),
}

FACE_COMMON = (
    "Painted portrait face in the muted tones of an old engraving-inspired oil study: "
    "desaturated warm greys, umber and pale ochre skin tones, soft painterly brushwork, "
    "late-medieval European character. Front-facing, perfectly centered, head upright, "
    "eyes exactly at the vertical midline of the image, neutral calm expression, looking "
    "straight ahead. The head floats on a completely plain, flat, uniform matte "
    "skin-tone background that fills every edge of the frame - no vignette, no gradient, "
    "no cast shadow, no border, and the head does not touch any edge. Hair, if any, "
    "cropped close to the skull and kept well away from the image edges. No headwear, no "
    "clothing, no shoulders, no text, no signature."
)

FACE_VARIANTS = [
    "a weathered male stone-hauler in his fifties, sun-creased skin, broad flattened nose",
    "a young male apprentice of about sixteen, smooth round face, faint first stubble",
    "an elderly matron of about seventy, hollow cheeks, deep-set patient eyes",
    "a gaunt middle-aged male cleric, ascetic features, thin lips, high forehead",
    "a young woman of about twenty, oval face, calm level gaze",
    "a male craftsman of about forty, square jaw, short dark stubble, heavy brows",
    "a very old man of about seventy-five, white eyebrows, deeply wrinkled papery skin",
    "a stout matron of about forty-five, plump ruddy cheeks, small firm mouth",
    "a sharp-featured young man in his mid-twenties, thin face, prominent cheekbones",
    "a female servant of about thirty, lightly freckled skin, wide-set eyes",
    "a male watchman of about thirty-five, once-broken nose, thin old scar through one eyebrow",
    "an elderly male cleric of about sixty-five, jowly, heavy-lidded eyes",
    "a laboring woman of about fifty, wind-chapped skin, tired kind eyes",
    "a young male journeyman of about nineteen, narrow face, alert dark eyes",
    "a middle-aged merchant of about fifty, well-fed full face, shrewd narrow eyes",
    "a woman of about thirty-five, long straight nose, composed weary expression",
    "an old riverman of about sixty, leathery weather-scoured skin, pale grey eyes",
    "a plain-faced young woman of about eighteen, soft features, downy brows",
    "a heavyset male brewer of about forty-five, thick jaw, florid skin",
    "a thin elderly woman of about sixty-eight, sunken temples, sharp chin",
    "a male scribe of about thirty, pale indoor skin, faint squint lines from close work",
    "a broad-faced country woman of about forty, high cheekbones, sun-browned skin",
    "a grizzled male veteran of about fifty-five, grey stubble, one drooping eyelid",
    "a solemn apprentice boy of about fifteen, smooth pale face, serious eyes",
]
assert len(FACE_VARIANTS) == FACE_COUNT


def log(message: str) -> None:
    with _print_lock:
        print(message, flush=True)


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


def load_api_key() -> None:
    from dotenv import load_dotenv

    load_dotenv(REPO_ROOT / ".env")
    if not os.environ.get("OPENAI_API_KEY"):
        load_dotenv(REPO_ROOT / "prompt_playgound" / ".env")
    if not os.environ.get("OPENAI_API_KEY"):
        sys.exit("OPENAI_API_KEY not found in .env or prompt_playgound/.env")


def _looks_like_model_rejection(exc: Exception) -> bool:
    message = str(exc).lower()
    return "model" in message and any(
        needle in message
        for needle in ("invalid", "does not exist", "not found", "unsupported", "unknown")
    )


def generate_image(client, prompt: str, quality: str, label: str) -> Image.Image:
    """One generation with model fallback and a single transient-failure retry."""

    global _model
    transient_failures = 0
    while True:
        model = _model
        try:
            response = client.images.generate(
                model=model,
                prompt=prompt,
                n=1,
                size=SIZE,
                quality=quality,
                output_format="png",
            )
        except Exception as exc:  # noqa: BLE001 - classified below
            if model == PRIMARY_MODEL and _looks_like_model_rejection(exc):
                with _model_lock:
                    if _model == PRIMARY_MODEL:
                        _model = FALLBACK_MODEL
                        log(
                            f"note: API rejected {PRIMARY_MODEL} "
                            f"({exc}); falling back to {FALLBACK_MODEL}"
                        )
                continue
            transient_failures += 1
            if transient_failures > 1:
                raise
            log(f"{label}: transient failure ({exc}); retrying once in 10 s")
            time.sleep(10)
            continue
        if not response.data or not response.data[0].b64_json:
            transient_failures += 1
            if transient_failures > 1:
                raise RuntimeError(f"no image data for {label}")
            log(f"{label}: empty response; retrying once in 10 s")
            time.sleep(10)
            continue
        return Image.open(BytesIO(base64.b64decode(response.data[0].b64_json)))


def save_atomic(image: Image.Image, target: Path) -> None:
    temporary = target.with_name(f".{target.name}.tmp")
    image.save(temporary, format="PNG")
    temporary.replace(target)


def produce_cloth(client, name: str, prompt: str) -> None:
    target = OUTPUT_DIR / name
    raw = generate_image(client, prompt, quality="high", label=name)
    save_atomic(make_seamless(raw.convert("RGB")), target)
    log(f"wrote {target} ({target.stat().st_size:,} bytes)")


def produce_face(client, index: int) -> None:
    target = OUTPUT_DIR / f"face_{index:02d}.png"
    prompt = f"{FACE_COMMON} The subject: {FACE_VARIANTS[index]}."
    raw = generate_image(client, prompt, quality="medium", label=target.name)
    small = raw.convert("RGB").resize((FACE_SAVE_SIZE, FACE_SAVE_SIZE), Image.LANCZOS)
    # Crop tight to the head and fade the beige margin to a shaded periphery, so
    # the sphere projection reads as a head, not a mask on a pale ball. No
    # seamless pass: faces must not be rolled.
    save_atomic(frame_head_portrait(small), target)
    log(f"wrote {target} ({target.stat().st_size:,} bytes)")


def contact_sheet(path: Path) -> None:
    """All 24 faces (6x4) plus the 7 cloths in a bottom row, labeled thumbnails."""

    thumb = 144
    pad = 8
    label_h = 18
    cell_w = thumb + pad
    cell_h = thumb + label_h + pad
    columns = 7
    face_rows = 4  # 24 faces at 6 per row
    sheet = Image.new("RGB", (columns * cell_w + pad, (face_rows + 1) * cell_h + pad), (34, 32, 30))
    draw = ImageDraw.Draw(sheet)
    try:
        font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 12)
    except OSError:
        font = ImageFont.load_default()

    def place(source: Path, column: int, row: int, label: str) -> None:
        x = pad + column * cell_w
        y = pad + row * cell_h
        if source.exists():
            tile = Image.open(source).convert("RGB").resize((thumb, thumb), Image.LANCZOS)
            sheet.paste(tile, (x, y))
        else:
            draw.rectangle([x, y, x + thumb, y + thumb], outline=(120, 60, 60), width=2)
            draw.text((x + 8, y + thumb // 2), "MISSING", fill=(200, 120, 120), font=font)
        draw.text((x, y + thumb + 3), label, fill=(210, 205, 195), font=font)

    for index in range(FACE_COUNT):
        place(OUTPUT_DIR / f"face_{index:02d}.png", index % 6, index // 6, f"face_{index:02d}")
    for column, name in enumerate(CLOTHS):
        place(OUTPUT_DIR / name, column, face_rows, Path(name).stem)

    path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(path, format="PNG")
    log(f"wrote contact sheet {path}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--force", action="store_true", help="regenerate even if the file exists")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument(
        "--contact-sheet",
        type=Path,
        default=None,
        help="also write a labeled thumbnail sheet of all outputs to this path",
    )
    args = parser.parse_args()

    load_api_key()
    from openai import OpenAI

    client = OpenAI()
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    jobs: list[tuple[str, object]] = []
    for name, prompt in CLOTHS.items():
        if not args.force and (OUTPUT_DIR / name).exists():
            log(f"skip {name} (exists)")
            continue
        jobs.append((name, lambda n=name, p=prompt: produce_cloth(client, n, p)))
    for index in range(FACE_COUNT):
        name = f"face_{index:02d}.png"
        if not args.force and (OUTPUT_DIR / name).exists():
            log(f"skip {name} (exists)")
            continue
        jobs.append((name, lambda i=index: produce_face(client, i)))

    failures: list[str] = []
    if jobs:
        # Settle the model choice on the first job alone so a gpt-image-2
        # rejection flips the fallback once instead of once per worker.
        first_name, first_job = jobs[0]
        try:
            first_job()
        except Exception as exc:  # noqa: BLE001
            log(f"FAILED {first_name}: {exc}")
            failures.append(first_name)
        with ThreadPoolExecutor(max_workers=max(1, args.workers)) as pool:
            futures = {pool.submit(job): name for name, job in jobs[1:]}
            for future, name in futures.items():
                try:
                    future.result()
                except Exception as exc:  # noqa: BLE001
                    log(f"FAILED {name}: {exc}")
                    failures.append(name)

    if args.contact_sheet:
        contact_sheet(args.contact_sheet)

    log("")
    log(f"model used: {_model}")
    written = sorted(OUTPUT_DIR.glob("*.png"))
    for path in written:
        log(f"  {path.relative_to(REPO_ROOT)}  {path.stat().st_size:,} bytes")
    missing = [f"face_{i:02d}" for i in range(FACE_COUNT) if not (OUTPUT_DIR / f"face_{i:02d}.png").exists()]
    missing += [Path(n).stem for n in CLOTHS if not (OUTPUT_DIR / n).exists()]
    if failures:
        log(f"failed this run: {', '.join(sorted(failures))}")
    if missing:
        log(f"still missing: {', '.join(missing)}")
        sys.exit(1)
    log("all 31 textures present")


if __name__ == "__main__":
    main()
