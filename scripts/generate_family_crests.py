#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["httpx", "openai", "python-dotenv"]
# ///
"""Heraldic crests for the families of Ombreval, in two variants.

Only *some* families bear arms — the houses of standing (canonical fixed
surnames with real property, office, or a workshop worth naming), plus a couple
of jumped-up ambient houses that have *assumed* rough arms nobody granted them.
The nameless poor and the plain ambient houses have none, on purpose; see
``lore/families/overview.md`` and the naming rule in
``core_lore/naming_language.md`` (*"There are more Hawsers than Alders."*).

Two backends draw the same set of arms:

* ``--backend svg`` (default, free, no network): procedural vector line-art.
  Clean, crisp, scalable ``family_<name>_crest.svg``. Pure Python.
* ``--backend image`` (needs ``OPENAI_API_KEY``): realistic painted crests via
  ``gpt-image-2``, saved as ``family_<name>_crest.png`` — the "pretty" variant.
  Auth like ``generate_lore_inspiration_images.py``: the key may be exported or
  live in the repo-root ``.env`` (or ``prompt_playgound/.env``).

Both are grounded in each family's story:
  * The five boat-families (Alder, Skell, Hobbe, Crake, Tarn) share a **wavy
    base** — the diverted Serle — but each keeps its own charge and field colour
    for the answer it gave to *do we keep the boat?* (see ``the_dry_boatmen.md``).
  * Grand houses get rich, gilded arms; minor houses plainer ones; the assumed
    arms (Skep, Kern) are deliberately crude and lopsided.

Existing files are kept so an interrupted run resumes; ``--force`` redraws,
``--only NAME[,NAME...]`` limits the set, ``--list`` prints the plan. Both
backends (re)write ``showcase.html`` (both variants side by side) and
``README.md``; the image backend also records every prompt in
``image_prompts.md``.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import hashlib
import math
import os
import random
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = REPO_ROOT / "lore" / "families" / "crests"

# SVG canvas (vector — no supersampling needed).
W, H = 480, 600

# Image backend.
MODEL = "gpt-image-2"
IMG_SIZE = "1024x1024"
IMG_QUALITY = "medium"

# --------------------------------------------------------------------------- #
# Tinctures — aged, slightly muted to sit beside the "dry worn" city palette.  #
# (fill, edge) pairs.                                                          #
# --------------------------------------------------------------------------- #
TINCT = {
    "or":      ((202, 164, 78),  (150, 116, 40)),   # gold
    "argent":  ((228, 226, 216), (168, 166, 156)),  # silver / white
    "gules":   ((150, 46, 40),   (96, 26, 22)),     # red
    "azure":   ((44, 74, 112),   (24, 44, 74)),     # blue
    "sable":   ((40, 38, 36),    (16, 15, 14)),     # black
    "vert":    ((54, 88, 56),    (30, 54, 32)),     # green
    "purpure": ((96, 52, 86),    (62, 32, 56)),     # purple
    "grey":    ((112, 112, 114), (72, 72, 74)),     # the Custody's cloth
    "tenne":   ((124, 84, 50),   (82, 54, 30)),     # tawny brown (base / rough)
}
# Plain words for the image-model prompts.
TINCT_WORDS = {
    "or": "gold", "argent": "silver-white", "gules": "deep red", "azure": "deep blue",
    "sable": "black", "vert": "green", "purpure": "purple", "grey": "slate grey",
    "tenne": "tawny brown",
}
CHARGE_WORDS = {
    "lymphad": "a single-masted medieval river boat with a small pennon",
    "eel": "a curving eel",
    "cartwheel": "a spoked wooden cartwheel",
    "bird": "a small plump corncrake bird",
    "tun": "an upright wooden barrel (tun) with hoops",
    "rose_window": "a cathedral rose window of radiating stone tracery",
    "tower": "a crenellated stone tower with an open mason's compass above it",
    "lantern": "a glowing hanging lantern casting rays of light",
    "candle": "a single lit candle with a golden flame",
    "estoile_needle": "a golden wavy-rayed star crossed by a slender sewing needle",
    "star_crescent": "a golden star above a silver crescent moon",
    "bezants": "three gold coins arranged two above and one below",
    "eye": "a single open watching human eye",
    "keys": "two crossed keys in saltire",
    "spade": "a sexton's iron spade with a small star above the handle",
    "horn": "a curved hunting horn (bugle)",
    "bell": "a church bell resting on a small green mound",
    "saltire_roundel": "a silver diagonal cross (saltire) with a gold roundel at its centre",
    "beehive": "a domed straw beehive (skep) with a few small bees",
    "garb": "a bound sheaf of wheat",
}
STYLE_WORDS = {
    "formal": ("Rich, finely painted arms of a house of standing, with real "
               "gold-leaf detailing and a crisp gilded border; dignified and precise."),
    "plain": ("Plainer, honest arms of a minor house — solid flat pigments, "
              "little gilding, workmanlike."),
    "rough": ("Crude, self-made 'assumed' arms of a poor house reaching above its "
              "station: folk-painted on rough board, slightly lopsided and "
              "asymmetrical, cheap flaking pigments, a clumsy hand-drawn outline."),
}


def t(name: str):
    return TINCT[name]


# --------------------------------------------------------------------------- #
# Family arms.                                                                 #
# --------------------------------------------------------------------------- #
@dataclass(frozen=True)
class Arms:
    key: str
    name: str
    field: str
    charge: str
    charge_tinct: str
    ordinary: str | None = None      # base_wavy | saltire | chief | bordure
    ord_tinct: str = "argent"
    style: str = "formal"            # formal | plain | rough
    blazon: str = ""
    extra: dict = field(default_factory=dict)


BOAT = dict(ordinary="base_wavy", ord_tinct="argent")

ARMS: list[Arms] = [
    # --- the five boat-families: one set, five answers ---------------------- #
    Arms("alder", "Alder", "azure", "lymphad", "or", **BOAT, style="formal",
         blazon="Azure, a wavy base argent, a boat Or — the house that kept the boat."),
    Arms("skell", "Skell", "vert", "eel", "or", **BOAT, style="formal",
         blazon="Vert, a wavy base argent, an eel Or — went ashore to fish and thrived."),
    Arms("hobbe", "Hobbe", "tenne", "cartwheel", "argent", **BOAT, style="plain",
         blazon="Tenné, a wavy base argent, a cartwheel argent — went to the cart."),
    Arms("crake", "Crake", "azure", "bird", "argent", **BOAT, style="plain",
         blazon="Azure, a wavy base argent, a bird argent between mullets — the Long Departure.",
         extra={"scatter": True}),
    Arms("tarn", "Tarn", "vert", "tun", "or", **BOAT, style="plain",
         blazon="Vert, a wavy base argent, a tun Or — went to barrel and hide."),

    # --- grand houses of standing ------------------------------------------ #
    Arms("sparr", "Sparr", "gules", "rose_window", "or", ordinary="bordure",
         ord_tinct="or", style="formal",
         blazon="Gules, a rose-window Or within a bordure Or — the master glaziers of Idonea's line."),
    Arms("stott", "Stott", "sable", "tower", "argent", style="formal",
         blazon="Sable, a tower embattled argent beneath a compass — warden of the masons' lodge."),
    Arms("dorn", "Dorn", "argent", "lantern", "or", ordinary="chief",
         ord_tinct="azure", style="formal",
         blazon="Argent, a lantern radiant Or, on a chief azure three mullets Or — the Praelucent's arms.",
         extra={"chief_stars": True}),
    Arms("vell", "Vell", "azure", "candle", "or", style="formal",
         blazon="Azure, a candle argent enflamed Or — the wax-house of the Wickmarket."),
    Arms("marle", "Marle", "azure", "estoile_needle", "or", style="formal",
         blazon="Azure, an estoile Or and a needle argent in bend — she charts the sky in thread."),
    Arms("ashe", "Ashe", "gules", "saltire_roundel", "argent", ordinary="saltire",
         ord_tinct="argent", style="formal",
         blazon="Gules, a saltire argent, a roundel Or in the crossing — the salt house (a *saltire* for salt)."),
    Arms("copp", "Copp", "gules", "bezants", "or", style="formal",
         blazon="Gules, three bezants Or — the money and paper house of the Tallage."),
    Arms("ferrant", "Ferrant", "azure", "star_crescent", "or", style="formal",
         blazon="Azure, an estoile Or above a crescent argent — the physician-astronomer."),
    Arms("rasp", "Rasp", "grey", "eye", "argent", ordinary="bordure",
         ord_tinct="sable", style="formal",
         blazon="Grey, an eye argent within a bordure sable — the master of the Custody of the Eye."),

    # --- minor houses of standing (plainer arms) --------------------------- #
    Arms("pike", "Pike", "azure", "keys", "argent", style="plain",
         blazon="Azure, two keys in saltire argent — the vergers who keep the Lanthorn's doors."),
    Arms("fitch", "Fitch", "sable", "spade", "argent", style="plain",
         blazon="Sable, a spade argent, a mullet in chief argent — the sexton's house of Saint Maren."),
    Arms("brant", "Brant", "vert", "horn", "or", style="plain",
         blazon="Vert, a hunting-horn Or — the criers whose voice runs before the bells."),
    Arms("mott", "Mott", "sable", "bell", "argent", style="plain",
         blazon="Sable, a bell argent upon a mount vert — the bellfounder with a foot in the Grey Press."),

    # --- assumed / rough arms (ambient houses reaching above their station) - #
    Arms("skep", "Skep", "vert", "beehive", "tenne", style="rough",
         blazon="Vert, a skep tenné with bees — canting arms a Skep drew himself; nobody granted them."),
    Arms("kern", "Kern", "gules", "garb", "or", style="rough",
         blazon="Gules, a garb Or — assumed arms of a lettered house with more letters than coin."),
]

NO_CREST = {
    "Rud": "cloth-poor; the Unwalled do not display arms",
    "Quern": "poor milling household; no property to blazon",
    "Sedge": "small counter-and-tavern folk of the weigh-streets",
    "Pell": "smallholders across three wards; no house of standing",
    "Rusk": "victuallers and touts; no arms",
    "Wren": "small makers of modest standing",
    "Dask": "plain working house; nothing to blazon",
    "Clove": "drovers and debt-men; no standing",
    "Thorn": "porters and tanners of the customs edge",
    "Rook": "servants and one bell-ringer; no house",
    "Bram": "soldiering poor; bear the town's arms, not their own",
    "Sark": "washhouse-and-kitchen labour",
    "Toll": "wall-quarter roughs; no grant",
    "Fenn": "watch-and-candle poor of Bell-and-Sluice",
    "Lark": "carters and cooks; no standing",
    "Mere": "small victuallers",
    "Kett": "market-food folk with a light finger",
    "Nett": "quiet cloth-and-timber house, no arms",
    "Rill": "a little water-name household",
    "Husk": "drovers of the Tallage edge",
    "Wick": "nondescript; a name that hides",
    "Dunn": "barely a house at all",
}


# --------------------------------------------------------------------------- #
# Image-backend prompt                                                        #
# --------------------------------------------------------------------------- #
def image_prompt(a: Arms) -> str:
    parts = [
        "A single medieval European heraldic coat of arms, painted like an aged "
        "illuminated manuscript on weathered board.",
        f"A heater shield. The field (background of the shield) is {TINCT_WORDS[a.field]}.",
    ]
    if a.ordinary == "base_wavy":
        parts.append("Across the lower third runs a wavy horizontal band of "
                     "silver-white water with a faint blue ripple (a river).")
    elif a.ordinary == "saltire":
        parts.append(f"A broad {TINCT_WORDS[a.ord_tinct]} diagonal cross (saltire) "
                     "spans the whole shield.")
    elif a.ordinary == "chief":
        band = f"Across the top is a {TINCT_WORDS[a.ord_tinct]} band (a chief)"
        parts.append(band + (" bearing three small gold stars." if a.extra.get("chief_stars") else "."))
    elif a.ordinary == "bordure":
        parts.append(f"A {TINCT_WORDS[a.ord_tinct]} border (bordure) runs around the shield's edge.")
    charge = f"Centred on the shield: {CHARGE_WORDS[a.charge]}, in {TINCT_WORDS[a.charge_tinct]}."
    if a.extra.get("scatter"):
        charge += " Two tiny silver stars are scattered beside it."
    parts.append(charge)
    parts.append(STYLE_WORDS[a.style])
    parts.append(
        "Matte historical pigments, subtle age, craquelure and wear. The shield is "
        "perfectly centred and front-facing on a plain dark charcoal background, "
        "soft even museum lighting. Absolutely no text, no letters, no numerals, no "
        "words, no motto scroll, no banner or ribbon — a single shield only."
    )
    return " ".join(parts)


# --------------------------------------------------------------------------- #
# SVG backend                                                                 #
# --------------------------------------------------------------------------- #
def cubic(p0, p1, p2, p3, u):
    mu = 1 - u
    x = mu**3 * p0[0] + 3 * mu**2 * u * p1[0] + 3 * mu * u**2 * p2[0] + u**3 * p3[0]
    y = mu**3 * p0[1] + 3 * mu**2 * u * p1[1] + 3 * mu * u**2 * p2[1] + u**3 * p3[1]
    return (x, y)


def shield_points(box, n=48, wobble=0.0, rng=None):
    l, tp, r, b = box
    w, h = r - l, b - tp
    cx = (l + r) / 2
    side_y = tp + 0.50 * h
    pts = [(l, tp), (r, tp), (r, side_y)]
    pts += [cubic((r, side_y), (r, tp + 0.80 * h),
                  (cx + 0.30 * w, b - 0.01 * h), (cx, b), i / n) for i in range(1, n + 1)]
    pts += [cubic((cx, b), (cx - 0.30 * w, b - 0.01 * h),
                  (l, tp + 0.80 * h), (l, side_y), i / n) for i in range(1, n + 1)]
    if wobble and rng:
        pts = [(x + rng.uniform(-wobble, wobble), y + rng.uniform(-wobble, wobble))
               for (x, y) in pts]
    return pts


def wavy_line(x0, x1, y, amp, wavelen, phase=0.0, step=6):
    pts, x = [], x0
    while x <= x1:
        pts.append((x, y + amp * math.sin((x - x0) / wavelen * 2 * math.pi + phase)))
        x += step
    pts.append((x1, y + amp * math.sin((x1 - x0) / wavelen * 2 * math.pi + phase)))
    return pts


def rot(px, py, cx, cy, deg):
    a = math.radians(deg)
    dx, dy = px - cx, py - cy
    return (cx + dx * math.cos(a) - dy * math.sin(a),
            cy + dx * math.sin(a) + dy * math.cos(a))


def rotpoly(points, cx, cy, deg):
    return [rot(x, y, cx, cy, deg) for (x, y) in points]


def crescent(cx, cy, r_out, r_in, offset, n=40):
    outer = [(cx + r_out * math.cos(2 * math.pi * i / n),
              cy + r_out * math.sin(2 * math.pi * i / n)) for i in range(n + 1)]
    inner = [(cx + offset + r_in * math.cos(2 * math.pi * i / n),
              cy + r_in * math.sin(2 * math.pi * i / n)) for i in range(n, -1, -1)]
    return outer + inner


def star_points(cx, cy, r_out, r_in, tips=5, rotation=-90):
    pts = []
    for i in range(tips * 2):
        r = r_out if i % 2 == 0 else r_in
        a = math.radians(rotation + i * 180 / tips)
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def path_d(points, close=True):
    d = "M " + " ".join(f"{x:.2f} {y:.2f}" for x, y in points)
    return d + (" Z" if close else "")


class SvgDraw:
    """A minimal shim exposing the PIL ImageDraw methods the charges call,
    so the exact same charge code emits SVG elements."""

    def __init__(self):
        self.parts: list[str] = []

    @staticmethod
    def col(c):
        if c is None:
            return "none"
        return f"rgb({int(round(c[0]))},{int(round(c[1]))},{int(round(c[2]))})"

    def _emit(self, s):
        self.parts.append(s)

    def polygon(self, pts, fill=None, outline=None, width=1):
        self._emit(f'<polygon points="{" ".join(f"{x:.2f},{y:.2f}" for x, y in pts)}" '
                   f'fill="{self.col(fill)}" stroke="{self.col(outline)}" '
                   f'stroke-width="{width:.2f}" stroke-linejoin="round"/>')

    def line(self, pts, fill=None, width=1, joint=None):
        self._emit(f'<path d="{path_d(pts, close=False)}" fill="none" '
                   f'stroke="{self.col(fill)}" stroke-width="{width:.2f}" '
                   f'stroke-linecap="round" stroke-linejoin="round"/>')

    def ellipse(self, box, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = box
        self._emit(f'<ellipse cx="{(x0 + x1) / 2:.2f}" cy="{(y0 + y1) / 2:.2f}" '
                   f'rx="{abs(x1 - x0) / 2:.2f}" ry="{abs(y1 - y0) / 2:.2f}" '
                   f'fill="{self.col(fill)}" stroke="{self.col(outline)}" '
                   f'stroke-width="{width:.2f}"/>')

    def rectangle(self, box, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = box
        self._emit(f'<rect x="{min(x0, x1):.2f}" y="{min(y0, y1):.2f}" '
                   f'width="{abs(x1 - x0):.2f}" height="{abs(y1 - y0):.2f}" '
                   f'fill="{self.col(fill)}" stroke="{self.col(outline)}" '
                   f'stroke-width="{width:.2f}" stroke-linejoin="round"/>')

    def arc(self, box, start, end, fill=None, width=1, n=48):
        x0, y0, x1, y1 = box
        cx, cy, rx, ry = (x0 + x1) / 2, (y0 + y1) / 2, abs(x1 - x0) / 2, abs(y1 - y0) / 2
        pts = [(cx + rx * math.cos(math.radians(start + (end - start) * i / n)),
                cy + ry * math.sin(math.radians(start + (end - start) * i / n)))
               for i in range(n + 1)]
        self.line(pts, fill=fill, width=width)

    def svg(self):
        return "\n".join(self.parts)


def _lw(s, k=0.09):
    return max(1.5, s * k)


# ---- charges (identical geometry to the raster version) -------------------- #
def c_lymphad(d, cx, cy, s, fill, edge, rng, style):
    top = wavy_line(cx - s, cx + s, cy + 0.10 * s, -0.14 * s, 2 * s)
    hull = top + [(cx + 0.62 * s, cy + 0.78 * s), (cx - 0.62 * s, cy + 0.78 * s)]
    d.polygon(hull, fill=fill, outline=edge, width=_lw(s))
    d.line([(cx, cy + 0.05 * s), (cx, cy - 0.95 * s)], fill=fill, width=_lw(s, 0.13))
    d.polygon([(cx, cy - 0.95 * s), (cx + 0.6 * s, cy - 0.78 * s), (cx, cy - 0.6 * s)],
              fill=fill, outline=edge, width=_lw(s, 0.05))


def c_eel(d, cx, cy, s, fill, edge, rng, style):
    pts = wavy_line(cx - 0.95 * s, cx + 0.85 * s, cy, 0.42 * s, 1.15 * s, step=4)
    d.line(pts, fill=fill, width=_lw(s, 0.26))
    hx, hy = pts[-1]
    d.ellipse([hx - 0.22 * s, hy - 0.22 * s, hx + 0.22 * s, hy + 0.22 * s], fill=fill)
    d.ellipse([hx - 0.02 * s, hy - 0.08 * s, hx + 0.08 * s, hy + 0.02 * s], fill=edge)
    tx, ty = pts[0]
    d.polygon([(tx, ty), (tx - 0.3 * s, ty - 0.28 * s), (tx - 0.3 * s, ty + 0.28 * s)], fill=fill)


def c_cartwheel(d, cx, cy, s, fill, edge, rng, style):
    d.ellipse([cx - s, cy - s, cx + s, cy + s], outline=fill, width=_lw(s, 0.16))
    d.ellipse([cx - 0.24 * s, cy - 0.24 * s, cx + 0.24 * s, cy + 0.24 * s],
              fill=fill, outline=edge, width=_lw(s, 0.04))
    for k in range(8):
        a = math.radians(k * 45)
        d.line([(cx + 0.22 * s * math.cos(a), cy + 0.22 * s * math.sin(a)),
                (cx + 0.9 * s * math.cos(a), cy + 0.9 * s * math.sin(a))],
               fill=fill, width=_lw(s, 0.09))


def c_bird(d, cx, cy, s, fill, edge, rng, style):
    body = [(cx - 0.7 * s, cy + 0.1 * s), (cx - 0.2 * s, cy - 0.35 * s),
            (cx + 0.5 * s, cy - 0.3 * s), (cx + 0.85 * s, cy + 0.05 * s),
            (cx + 0.4 * s, cy + 0.55 * s), (cx - 0.4 * s, cy + 0.5 * s)]
    d.polygon(body, fill=fill, outline=edge, width=_lw(s, 0.05))
    d.ellipse([cx + 0.55 * s, cy - 0.55 * s, cx + 1.0 * s, cy - 0.1 * s], fill=fill)
    d.polygon([(cx + 1.0 * s, cy - 0.34 * s), (cx + 1.35 * s, cy - 0.28 * s),
               (cx + 1.0 * s, cy - 0.16 * s)], fill=t("or")[0] if style != "rough" else fill)
    d.ellipse([cx + 0.8 * s, cy - 0.42 * s, cx + 0.9 * s, cy - 0.32 * s], fill=edge)
    d.polygon([(cx - 0.1 * s, cy - 0.2 * s), (cx + 0.55 * s, cy - 0.05 * s),
               (cx + 0.05 * s, cy + 0.35 * s)], fill=edge)
    for dx in (-0.15, 0.2):
        d.line([(cx + dx * s, cy + 0.5 * s), (cx + dx * s, cy + 0.85 * s)],
               fill=fill, width=_lw(s, 0.06))


def c_tun(d, cx, cy, s, fill, edge, rng, style):
    body = [(cx - 0.6 * s, cy - 0.85 * s), (cx + 0.6 * s, cy - 0.85 * s),
            (cx + 0.85 * s, cy), (cx + 0.6 * s, cy + 0.85 * s),
            (cx - 0.6 * s, cy + 0.85 * s), (cx - 0.85 * s, cy)]
    d.polygon(body, fill=fill, outline=edge, width=_lw(s, 0.06))
    for yy in (-0.85, -0.4, 0.4, 0.85):
        wfac = 0.85 if abs(yy) < 0.5 else 0.6
        d.line([(cx - wfac * s, cy + yy * s), (cx + wfac * s, cy + yy * s)],
               fill=edge, width=_lw(s, 0.08))
    for xx in (-0.3, 0.0, 0.3):
        d.line([(cx + xx * s, cy - 0.8 * s), (cx + xx * s, cy + 0.8 * s)],
               fill=edge, width=_lw(s, 0.04))


def c_rose_window(d, cx, cy, s, fill, edge, rng, style):
    d.ellipse([cx - s, cy - s, cx + s, cy + s], outline=fill, width=_lw(s, 0.11))
    d.ellipse([cx - 0.32 * s, cy - 0.32 * s, cx + 0.32 * s, cy + 0.32 * s],
              outline=fill, width=_lw(s, 0.07))
    for k in range(12):
        a = math.radians(k * 30)
        d.line([(cx + 0.32 * s * math.cos(a), cy + 0.32 * s * math.sin(a)),
                (cx + 0.9 * s * math.cos(a), cy + 0.9 * s * math.sin(a))],
               fill=fill, width=_lw(s, 0.05))
    for k in range(12):
        a = math.radians(k * 30 + 15)
        fx, fy = cx + 0.62 * s * math.cos(a), cy + 0.62 * s * math.sin(a)
        d.ellipse([fx - 0.1 * s, fy - 0.1 * s, fx + 0.1 * s, fy + 0.1 * s],
                  outline=fill, width=_lw(s, 0.03))


def c_tower(d, cx, cy, s, fill, edge, rng, style):
    bx0, bx1 = cx - 0.6 * s, cx + 0.6 * s
    by0, by1 = cy - 0.45 * s, cy + 0.95 * s
    d.rectangle([bx0, by0, bx1, by1], fill=fill, outline=edge, width=_lw(s, 0.05))
    mw = (bx1 - bx0) / 5
    for i in (0, 2, 4):
        d.rectangle([bx0 + i * mw, by0 - 0.22 * s, bx0 + (i + 1) * mw, by0],
                    fill=fill, outline=edge, width=_lw(s, 0.03))
    d.rectangle([cx - 0.16 * s, by1 - 0.5 * s, cx + 0.16 * s, by1], fill=edge)
    d.ellipse([cx - 0.16 * s, by1 - 0.66 * s, cx + 0.16 * s, by1 - 0.34 * s], fill=edge)
    apex = (cx, cy - 1.15 * s)
    d.line([apex, (cx - 0.4 * s, cy - 0.55 * s)], fill=fill, width=_lw(s, 0.08))
    d.line([apex, (cx + 0.4 * s, cy - 0.55 * s)], fill=fill, width=_lw(s, 0.08))
    d.ellipse([apex[0] - 0.08 * s, apex[1] - 0.08 * s,
               apex[0] + 0.08 * s, apex[1] + 0.08 * s], fill=fill)


def c_lantern(d, cx, cy, s, fill, edge, rng, style):
    for k in range(16):
        a = math.radians(k * 22.5)
        d.line([(cx + 0.75 * s * math.cos(a), cy + 0.75 * s * math.sin(a)),
                (cx + 1.25 * s * math.cos(a), cy + 1.25 * s * math.sin(a))],
               fill=fill, width=_lw(s, 0.05))
    body = [(cx - 0.42 * s, cy - 0.35 * s), (cx + 0.42 * s, cy - 0.35 * s),
            (cx + 0.5 * s, cy + 0.45 * s), (cx - 0.5 * s, cy + 0.45 * s)]
    d.polygon(body, fill=fill, outline=edge, width=_lw(s, 0.05))
    d.rectangle([cx - 0.28 * s, cy - 0.52 * s, cx + 0.28 * s, cy - 0.35 * s], fill=fill)
    d.ellipse([cx - 0.12 * s, cy - 0.72 * s, cx + 0.12 * s, cy - 0.48 * s],
              outline=fill, width=_lw(s, 0.05))
    d.rectangle([cx - 0.22 * s, cy - 0.18 * s, cx + 0.22 * s, cy + 0.3 * s], fill=edge)


def c_candle(d, cx, cy, s, fill, edge, rng, style):
    d.rectangle([cx - 0.18 * s, cy - 0.55 * s, cx + 0.18 * s, cy + 0.8 * s],
                fill=fill, outline=edge, width=_lw(s, 0.04))
    d.polygon([(cx - 0.45 * s, cy + 0.95 * s), (cx + 0.45 * s, cy + 0.95 * s),
               (cx + 0.22 * s, cy + 0.78 * s), (cx - 0.22 * s, cy + 0.78 * s)],
              fill=fill, outline=edge, width=_lw(s, 0.03))
    d.line([(cx, cy - 0.55 * s), (cx, cy - 0.7 * s)], fill=edge, width=_lw(s, 0.04))
    flame = t("or")
    d.polygon([(cx, cy - 1.25 * s), (cx + 0.22 * s, cy - 0.78 * s),
               (cx, cy - 0.62 * s), (cx - 0.22 * s, cy - 0.78 * s)],
              fill=flame[0], outline=flame[1], width=_lw(s, 0.03))


def c_estoile_needle(d, cx, cy, s, fill, edge, rng, style):
    d.polygon(star_points(cx, cy, s, 0.4 * s, tips=6, rotation=-90),
              fill=fill, outline=edge, width=_lw(s, 0.04))
    n0 = rot(cx - 1.05 * s, cy, cx, cy, -35)
    n1 = rot(cx + 1.05 * s, cy, cx, cy, -35)
    d.line([n0, n1], fill=t("argent")[0], width=_lw(s, 0.09))
    d.ellipse([n0[0] - 0.1 * s, n0[1] - 0.1 * s, n0[0] + 0.1 * s, n0[1] + 0.1 * s],
              outline=t("argent")[1], width=_lw(s, 0.04))


def c_star_crescent(d, cx, cy, s, fill, edge, rng, style):
    d.polygon(star_points(cx, cy - 0.35 * s, 0.75 * s, 0.3 * s, tips=6, rotation=-90),
              fill=fill, outline=edge, width=_lw(s, 0.04))
    ar = t("argent")
    d.polygon(crescent(cx, cy + 0.7 * s, 0.55 * s, 0.42 * s, 0.22 * s), fill=ar[0], outline=ar[1])


def c_bezants(d, cx, cy, s, fill, edge, rng, style):
    r = 0.42 * s
    for (dx, dy) in [(-0.5, -0.45), (0.5, -0.45), (0.0, 0.5)]:
        px, py = cx + dx * s, cy + dy * s
        d.ellipse([px - r, py - r, px + r, py + r], fill=fill, outline=edge, width=_lw(s, 0.05))
        d.line([(px - 0.2 * r, py), (px + 0.2 * r, py)], fill=edge, width=_lw(s, 0.03))


def c_eye(d, cx, cy, s, fill, edge, rng, style):
    top = [cubic((cx - s, cy), (cx - 0.4 * s, cy - 0.7 * s),
                 (cx + 0.4 * s, cy - 0.7 * s), (cx + s, cy), i / 24) for i in range(25)]
    bot = [cubic((cx + s, cy), (cx + 0.4 * s, cy + 0.7 * s),
                 (cx - 0.4 * s, cy + 0.7 * s), (cx - s, cy), i / 24) for i in range(25)]
    d.polygon(top + bot, fill=fill, outline=edge, width=_lw(s, 0.05))
    d.ellipse([cx - 0.42 * s, cy - 0.42 * s, cx + 0.42 * s, cy + 0.42 * s],
              fill=t("azure")[0], outline=edge, width=_lw(s, 0.04))
    d.ellipse([cx - 0.17 * s, cy - 0.17 * s, cx + 0.17 * s, cy + 0.17 * s], fill=t("sable")[0])


def c_keys(d, cx, cy, s, fill, edge, rng, style):
    def place(dx, dy, mirror, ang):
        return rot(cx + (-dx if mirror else dx), cy + dy, cx, cy, ang)

    def one_key(mirror, ang):
        rc = place(0.0, 0.78 * s, mirror, ang)
        rr = 0.26 * s
        d.ellipse([rc[0] - rr, rc[1] - rr, rc[0] + rr, rc[1] + rr],
                  outline=fill, width=_lw(s, 0.10))
        d.ellipse([rc[0] - 0.08 * s, rc[1] - 0.08 * s, rc[0] + 0.08 * s, rc[1] + 0.08 * s],
                  fill=edge)
        d.line([place(0, 0.52 * s, mirror, ang), place(0, -0.9 * s, mirror, ang)],
               fill=fill, width=_lw(s, 0.11))
        for yy in (-0.55, -0.78):
            d.line([place(0, yy * s, mirror, ang), place(0.34 * s, yy * s, mirror, ang)],
                   fill=fill, width=_lw(s, 0.10))
    one_key(False, 40)
    one_key(True, -40)


def c_spade(d, cx, cy, s, fill, edge, rng, style):
    d.line([(cx, cy - 0.9 * s), (cx, cy + 0.35 * s)], fill=fill, width=_lw(s, 0.12))
    d.arc([cx - 0.28 * s, cy - 1.15 * s, cx + 0.28 * s, cy - 0.72 * s], 180, 360,
          fill=fill, width=_lw(s, 0.09))
    d.line([(cx - 0.28 * s, cy - 0.93 * s), (cx, cy - 0.9 * s)], fill=fill, width=_lw(s, 0.09))
    d.line([(cx + 0.28 * s, cy - 0.93 * s), (cx, cy - 0.9 * s)], fill=fill, width=_lw(s, 0.09))
    d.polygon([(cx - 0.42 * s, cy + 0.3 * s), (cx + 0.42 * s, cy + 0.3 * s),
               (cx + 0.34 * s, cy + 0.9 * s), (cx, cy + 1.05 * s), (cx - 0.34 * s, cy + 0.9 * s)],
              fill=fill, outline=edge, width=_lw(s, 0.04))
    d.polygon(star_points(cx, cy - 1.5 * s, 0.32 * s, 0.13 * s), fill=fill)


def c_horn(d, cx, cy, s, fill, edge, rng, style):
    outer = [(cx + 1.05 * s * math.cos(math.radians(a)),
              cy + 0.7 * s * math.sin(math.radians(a))) for a in range(20, 200, 10)]
    inner = [(cx + 0.72 * s * math.cos(math.radians(a)),
              cy + 0.42 * s * math.sin(math.radians(a))) for a in range(200, 20, -10)]
    d.polygon(outer + inner, fill=fill, outline=edge, width=_lw(s, 0.05))
    bx, by = cx + 1.05 * s * math.cos(math.radians(20)), cy + 0.7 * s * math.sin(math.radians(20))
    d.ellipse([bx - 0.16 * s, by - 0.26 * s, bx + 0.22 * s, by + 0.26 * s],
              fill=fill, outline=edge, width=_lw(s, 0.04))
    d.line([(cx - 0.9 * s, cy - 0.55 * s), (cx + 0.4 * s, cy + 0.75 * s)],
           fill=edge, width=_lw(s, 0.05))


def c_bell(d, cx, cy, s, fill, edge, rng, style):
    body = [(cx - 0.15 * s, cy - 0.75 * s), (cx + 0.15 * s, cy - 0.75 * s),
            (cx + 0.55 * s, cy + 0.45 * s), (cx + 0.72 * s, cy + 0.6 * s),
            (cx - 0.72 * s, cy + 0.6 * s), (cx - 0.55 * s, cy + 0.45 * s)]
    d.polygon(body, fill=fill, outline=edge, width=_lw(s, 0.05))
    d.ellipse([cx - 0.16 * s, cy - 0.98 * s, cx + 0.16 * s, cy - 0.66 * s],
              outline=fill, width=_lw(s, 0.07))
    d.ellipse([cx - 0.1 * s, cy + 0.62 * s, cx + 0.1 * s, cy + 0.82 * s], fill=fill)
    mount = t("vert")
    d.polygon([(cx - 1.1 * s, cy + 1.4 * s), (cx + 1.1 * s, cy + 1.4 * s),
               (cx + 0.7 * s, cy + 0.95 * s), (cx, cy + 0.85 * s), (cx - 0.7 * s, cy + 0.95 * s)],
              fill=mount[0], outline=mount[1], width=_lw(s, 0.03))


def c_saltire_roundel(d, cx, cy, s, fill, edge, rng, style):
    r = 0.34 * s
    ro = t("or")
    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=ro[0], outline=ro[1], width=_lw(s, 0.05))


def c_beehive(d, cx, cy, s, fill, edge, rng, style):
    for i, yy in enumerate((0.55, 0.15, -0.2, -0.5)):
        wd = (1.0 - i * 0.2) * s
        d.arc([cx - wd, cy + yy * s - 0.35 * s, cx + wd, cy + yy * s + 0.35 * s],
              180, 360, fill=fill, width=_lw(s, 0.14))
    d.ellipse([cx - 0.14 * s, cy + 0.35 * s, cx + 0.14 * s, cy + 0.72 * s], fill=edge)
    for _ in range(3):
        bx = cx + rng.uniform(-1.1, 1.1) * s
        by = cy + rng.uniform(-0.9, 0.2) * s
        d.ellipse([bx - 0.06 * s, by - 0.06 * s, bx + 0.06 * s, by + 0.06 * s], fill=t("sable")[0])


def c_garb(d, cx, cy, s, fill, edge, rng, style):
    for k in range(-3, 4):
        lean = k * 6
        top = rot(cx + k * 0.16 * s, cy - 0.95 * s, cx, cy, lean)
        bot = rot(cx + k * 0.1 * s, cy + 0.7 * s, cx, cy, lean)
        d.line([top, bot], fill=fill, width=_lw(s, 0.12))
        d.ellipse([top[0] - 0.1 * s, top[1] - 0.16 * s, top[0] + 0.1 * s, top[1] + 0.06 * s], fill=fill)
    d.rectangle([cx - 0.5 * s, cy + 0.05 * s, cx + 0.5 * s, cy + 0.32 * s],
                fill=edge, outline=fill, width=_lw(s, 0.04))


CHARGES = {
    "lymphad": c_lymphad, "eel": c_eel, "cartwheel": c_cartwheel, "bird": c_bird,
    "tun": c_tun, "rose_window": c_rose_window, "tower": c_tower, "lantern": c_lantern,
    "candle": c_candle, "estoile_needle": c_estoile_needle, "star_crescent": c_star_crescent,
    "bezants": c_bezants, "eye": c_eye, "keys": c_keys, "spade": c_spade, "horn": c_horn,
    "bell": c_bell, "saltire_roundel": c_saltire_roundel, "beehive": c_beehive, "garb": c_garb,
}


def draw_ordinary(d, box, arms, rng):
    l, tp, r, b = box
    w, h = r - l, b - tp
    fill, edge = t(arms.ord_tinct)
    if arms.ordinary == "base_wavy":
        y = tp + 0.66 * h
        top = wavy_line(l - 20, r + 20, y, 0.035 * h, 0.30 * w, step=8)
        d.polygon(top + [(r + 20, b + 40), (l - 20, b + 40)], fill=fill)
        rip = wavy_line(l - 20, r + 20, y + 0.11 * h, 0.03 * h, 0.30 * w, phase=1.1, step=8)
        d.line(rip, fill=t("azure")[0], width=max(3, 0.02 * h))
        d.line(top, fill=edge, width=max(2, 0.008 * h))
    elif arms.ordinary == "saltire":
        wd = 0.16 * w
        for (a0, a1) in [((l, tp), (r, b)), ((r, tp), (l, b))]:
            dx, dy = a1[0] - a0[0], a1[1] - a0[1]
            ln = math.hypot(dx, dy)
            nx, ny = -dy / ln * wd, dx / ln * wd
            d.polygon([(a0[0] + nx, a0[1] + ny), (a1[0] + nx, a1[1] + ny),
                       (a1[0] - nx, a1[1] - ny), (a0[0] - nx, a0[1] - ny)],
                      fill=fill, outline=edge, width=3)
    elif arms.ordinary == "chief":
        ch = tp + 0.24 * h
        d.rectangle([l - 20, tp - 20, r + 20, ch], fill=fill, outline=edge, width=3)
        if arms.extra.get("chief_stars"):
            sc = t("or")
            for k in (-1, 0, 1):
                d.polygon(star_points(l + w / 2 + k * 0.26 * w, tp + 0.12 * h,
                                      0.055 * h, 0.023 * h), fill=sc[0], outline=sc[1])
    elif arms.ordinary == "bordure":
        pts = shield_points((l, tp, r, b))
        d.line(pts + [pts[0]], fill=fill, width=0.08 * w)


def build_svg(arms: Arms) -> str:
    rng = random.Random(int(hashlib.md5(arms.name.encode()).hexdigest(), 16))
    w, h = W, H
    mx, mt, mb = 0.10 * w, 0.06 * h, 0.05 * h
    box = (mx, mt, w - mx, h - mb)
    wobble = 0.010 * w if arms.style == "rough" else 0.0
    shield = shield_points(box, wobble=wobble, rng=rng)
    sd = path_d(shield)
    inner = shield_points((box[0] + 0.02 * w, box[1] + 0.02 * h,
                           box[2] - 0.02 * w, box[3] - 0.015 * h))

    d = SvgDraw()
    d.rectangle([0, 0, w, h], fill=t(arms.field)[0])
    d._emit(f'<rect x="0" y="0" width="{w}" height="{h}" fill="url(#shade)"/>')
    if arms.ordinary:
        draw_ordinary(d, box, arms, rng)

    cx = (box[0] + box[2]) / 2
    cy = box[1] + (0.40 if arms.ordinary != "chief" else 0.50) * (box[3] - box[1])
    s = 0.20 * (box[2] - box[0])
    if arms.ordinary == "base_wavy":
        cy = box[1] + 0.34 * (box[3] - box[1])
        s = 0.19 * (box[2] - box[0])
    fill, edge = t(arms.charge_tinct)
    if arms.extra.get("scatter"):
        sc = t("argent")
        for (dx, dy) in [(-0.62, -0.36), (0.6, -0.5)]:
            d.polygon(star_points(cx + dx * 1.4 * s, cy + dy * 1.6 * s, 0.28 * s, 0.11 * s),
                      fill=sc[0])
    CHARGES[arms.charge](d, cx, cy, s, fill, edge, rng, arms.style)
    if arms.style == "rough":
        d._emit(f'<rect x="0" y="0" width="{w}" height="{h}" filter="url(#grain)" opacity="0.08"/>')

    ow = 0.012 * w if arms.style != "rough" else 0.016 * w
    grain = ('<filter id="grain"><feTurbulence type="fractalNoise" baseFrequency="0.9" '
             'numOctaves="2" stitchTiles="stitch"/><feColorMatrix type="saturate" '
             'values="0"/></filter>') if arms.style == "rough" else ""
    inner_trim = (f'<path d="{path_d(inner)}" fill="none" stroke="{SvgDraw.col(t("or")[0])}" '
                  f'stroke-width="{max(1.5, ow / 3):.2f}" stroke-linejoin="round"/>'
                  if arms.style == "formal" else "")

    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img">
<title>Arms of the {arms.name}s — {arms.blazon}</title>
<defs>
<clipPath id="shield"><path d="{sd}"/></clipPath>
<linearGradient id="shade" x1="0" y1="0" x2="0" y2="1">
<stop offset="0" stop-color="black" stop-opacity="0.05"/>
<stop offset="1" stop-color="black" stop-opacity="0.22"/>
</linearGradient>
<filter id="blur" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="{0.018 * w:.1f}"/></filter>
{grain}
</defs>
<path d="{sd}" fill="black" fill-opacity="0.32" transform="translate({0.012 * w:.1f},{0.016 * h:.1f})" filter="url(#blur)"/>
<g clip-path="url(#shield)">
{d.svg()}
</g>
<path d="{sd}" fill="none" stroke="{SvgDraw.col(t("sable")[0])}" stroke-width="{ow:.2f}" stroke-linejoin="round" stroke-linecap="round"/>
{inner_trim}
</svg>
'''


# --------------------------------------------------------------------------- #
# Image backend                                                               #
# --------------------------------------------------------------------------- #
async def generate_images(jobs, api_key, size, quality, concurrency):
    from openai import AsyncOpenAI

    client = AsyncOpenAI(api_key=api_key, timeout=240.0, max_retries=3)
    sem = asyncio.Semaphore(concurrency)
    failures: list[tuple[str, Exception]] = []

    async def one(arms: Arms, path: Path):
        async with sem:
            print(f"  generate {path.relative_to(REPO_ROOT)}", flush=True)
            try:
                resp = await client.images.generate(
                    model=MODEL, prompt=image_prompt(arms), size=size, quality=quality)
                b64 = resp.data[0].b64_json if resp.data else None
                if not b64:
                    raise RuntimeError("empty image response")
                path.write_bytes(base64.b64decode(b64, validate=True))
                print(f"    wrote {path.relative_to(REPO_ROOT)}", flush=True)
            except Exception as exc:  # noqa: BLE001 — one bad image must not kill the run
                failures.append((arms.name, exc))
                print(f"    FAILED {arms.name}: {exc}", file=sys.stderr, flush=True)

    await asyncio.gather(*(one(a, p) for a, p in jobs))
    return failures


def load_api_key() -> str:
    from dotenv import load_dotenv
    for env in (REPO_ROOT / ".env", REPO_ROOT / "prompt_playgound" / ".env"):
        if env.exists():
            load_dotenv(env, override=False)
    return os.environ.get("OPENAI_API_KEY", "").strip()


# --------------------------------------------------------------------------- #
# Showcase + README + prompt log                                              #
# --------------------------------------------------------------------------- #
def write_showcase():
    cards = "\n".join(
        f'''      <figure>
        <div class="pair">
          <img class="png" src="family_{a.key}_crest.png" alt="Painted arms of the {a.name}s" loading="lazy"
               onerror="this.style.display='none'">
          <img class="svg" src="family_{a.key}_crest.svg" alt="Line arms of the {a.name}s" loading="lazy">
        </div>
        <figcaption><b>{a.name}</b><br><span>{a.blazon}</span></figcaption>
      </figure>''' for a in ARMS)
    html = f'''<!doctype html>
<meta charset="utf-8">
<title>Arms of the Families of Ombreval</title>
<style>
  body {{ background:#1a1714; color:#e7e2d6; font-family:Georgia,serif; margin:0; padding:2rem; }}
  h1 {{ font-weight:normal; letter-spacing:.03em; }}
  p.lede {{ max-width:52rem; color:#b8b0a0; }}
  .grid {{ display:grid; grid-template-columns:repeat(auto-fill,minmax(260px,1fr)); gap:1.6rem; margin-top:2rem; }}
  figure {{ margin:0; text-align:center; background:#221e19; border:1px solid #3a342c;
            border-radius:8px; padding:1rem; }}
  .pair {{ display:flex; gap:.8rem; align-items:flex-end; justify-content:center; }}
  .png {{ width:150px; height:auto; border-radius:4px; filter:drop-shadow(0 4px 8px rgba(0,0,0,.5)); }}
  .svg {{ width:90px; height:auto; filter:drop-shadow(0 3px 6px rgba(0,0,0,.5)); }}
  figcaption {{ margin-top:.6rem; font-size:.9rem; }}
  figcaption span {{ color:#9c9482; font-size:.8rem; font-style:italic; }}
</style>
<h1>Arms of the Families of Ombreval</h1>
<p class="lede">Two variants per house: the larger is the painted
<code>gpt-image-2</code> crest (PNG); the smaller is the procedural line-art
(SVG). Only houses of standing bear arms; the five boat-families share a wavy
base (the diverted Serle); Skep and Kern bear crooked <i>assumed</i> arms.
Generated by <code>scripts/generate_family_crests.py</code>.</p>
<div class="grid">
{cards}
</div>
'''
    (OUT_DIR / "showcase.html").write_text(html, encoding="utf-8")


def write_readme():
    order = {"formal": 0, "plain": 1, "rough": 2}
    lines = [
        "# Family crests", "",
        "Heraldry for the families of Ombreval, in **two variants**, drawn by",
        "`scripts/generate_family_crests.py`. Preview both together in",
        "[`showcase.html`](showcase.html).", "",
        "- **`family_<name>_crest.svg`** — procedural vector line-art (free, no network):",
        "  `uv run scripts/generate_family_crests.py` (default `--backend svg`).",
        "- **`family_<name>_crest.png`** — realistic painted crest from `gpt-image-2`:",
        "  `uv run scripts/generate_family_crests.py --backend image` (needs `OPENAI_API_KEY`).",
        "  Every prompt is logged in [`image_prompts.md`](image_prompts.md).", "",
        "**Not every family bears arms.** Heraldry follows standing: a house needs",
        "property, an office, or a workshop worth naming. The nameless poor carry",
        "bynames, not arms (*\"There are more Hawsers than Alders\"*).", "",
        "## Who bears arms", "",
        "| Family | Style | Blazon |", "|---|---|---|",
    ]
    for a in sorted(ARMS, key=lambda a: (order[a.style], a.name)):
        style = {"formal": "grand", "plain": "minor", "rough": "assumed (rough)"}[a.style]
        lines.append(f"| **{a.name}** | {style} | {a.blazon} |")
    lines += [
        "", "### Shared motifs",
        "- **The boat-families** (Alder, Skell, Hobbe, Crake, Tarn) all bear a **wavy",
        "  base** — the Serle that was moved outside the wall — over which each sets",
        "  its own charge for the answer it gave to *do we keep the boat?*",
        "- **Grand houses** get rich, gilded arms; **minor** houses (a verger, a crier,",
        "  a bellfounder) get plainer ones.",
        "- **Assumed arms** (Skep's beehive, Kern's wheatsheaf) are drawn crooked and",
        "  muddy on purpose — a house inventing a crest above its station.",
        "", "## Who bears none, and why", "",
    ]
    for name, why in sorted(NO_CREST.items()):
        lines.append(f"- **{name}** — {why}.")
    lines += ["", "See [`../overview.md`](../overview.md) for the full family index."]
    (OUT_DIR / "README.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_prompt_log():
    lines = ["# Image-generation prompts",
             "", f"Prompts sent to `{MODEL}` by `scripts/generate_family_crests.py "
             "--backend image`. Re-run with `--force` to regenerate.", ""]
    for a in ARMS:
        lines += [f"## {a.name}", "", f"> {a.blazon}", "", "```", image_prompt(a), "```", ""]
    (OUT_DIR / "image_prompts.md").write_text("\n".join(lines), encoding="utf-8")


# --------------------------------------------------------------------------- #
# Main                                                                        #
# --------------------------------------------------------------------------- #
def selected(only: set[str]):
    for a in ARMS:
        if not only or a.name.lower() in only or a.key in only:
            yield a


def main() -> int:
    ap = argparse.ArgumentParser(description="Heraldic crests for Ombreval's families.")
    ap.add_argument("--backend", choices=("svg", "image"), default="svg",
                    help="svg = procedural vector (default); image = gpt-image-2 PNG")
    ap.add_argument("--force", action="store_true", help="redraw even if the file exists")
    ap.add_argument("--only", default="", help="comma-separated family names to (re)draw")
    ap.add_argument("--list", action="store_true", help="print the plan and exit")
    ap.add_argument("--size", default=IMG_SIZE, help=f"image size (default {IMG_SIZE})")
    ap.add_argument("--quality", default=IMG_QUALITY, help=f"image quality (default {IMG_QUALITY})")
    ap.add_argument("--concurrency", type=int, default=4, help="parallel image requests")
    args = ap.parse_args()
    only = {s.strip().lower() for s in args.only.split(",") if s.strip()}

    if args.list:
        print(f"{len(ARMS)} families bear arms:")
        for a in ARMS:
            print(f"  {a.name:9s} [{a.style:6s}] {a.blazon}")
        print(f"\n{len(NO_CREST)} bear none: " + ", ".join(sorted(NO_CREST)))
        return 0

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    ext = "svg" if args.backend == "svg" else "png"
    todo = [(a, OUT_DIR / f"family_{a.key}_crest.{ext}") for a in selected(only)]
    pending = [(a, p) for a, p in todo if args.force or not p.exists()]
    kept = len(todo) - len(pending)

    if args.backend == "svg":
        for a, p in pending:
            p.write_text(build_svg(a), encoding="utf-8")
            print(f"  drew {p.relative_to(REPO_ROOT)}")
    else:
        api_key = load_api_key()
        if not api_key:
            print("OPENAI_API_KEY is not set (shell, .env, or prompt_playgound/.env).",
                  file=sys.stderr)
            return 2
        write_prompt_log()
        if pending:
            print(f"generating {len(pending)} image(s): model={MODEL}, size={args.size}, "
                  f"quality={args.quality}, concurrency={args.concurrency}")
            failures = asyncio.run(generate_images(
                pending, api_key, args.size, args.quality, args.concurrency))
            if failures:
                print(f"\n{len(failures)} failed: " +
                      ", ".join(n for n, _ in failures), file=sys.stderr)

    write_showcase()
    write_readme()
    print(f"\n{args.backend}: {len(pending)} drawn, {kept} kept. "
          f"Output in {OUT_DIR.relative_to(REPO_ROOT)}/.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
