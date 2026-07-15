#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# ///
"""Generate Ombreval's *ward map* — the cadastral plan with a ward overlay.

This is the authoritative top-down map (``generate_top_down_map.py`` ->
``ombreval_top_down_map.svg``: every building footprint, road, wall, gate and
label) with a translucent colour wash laid over the eight planning wards, so the
districts read at a glance while the building contours stay visible underneath.
It is written as a separate file, ``lore/places/ombreval_ward_map.svg``.

The wards are the *real* partition the cadastral generator uses to assign every
building to a district: ``district_for(x, z)`` — a first-match-wins decision
tree, not the overlapping bounding boxes tabulated in ``00_city_plan.md``. That
partition tiles the whole enclosure with no gaps or overlaps, so each ward's
share of the city is well defined.

This script imports the authoritative generator, renders the base map through
it (guaranteeing exact registration), then splices in the overlay:

- the ward colour wash goes *inside* ``<g id="map-art">``, right before
  ``<g id="direct-labels">`` — over the buildings, under the map's own labels;
- bold ward names, area figures and a ward key go on top, before ``</svg>``.

    uv run scripts/generate_ward_map.py
"""

from __future__ import annotations

import importlib.util
import sys
from html import escape
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Import the authoritative generator (its sibling) so the two maps share exactly
# one coordinate system, wall polygon, ward partition, repo-root discovery, and
# rendered base image.
_spec = importlib.util.spec_from_file_location("_ombreval_plan", HERE / "generate_top_down_map.py")
assert _spec and _spec.loader
plan = importlib.util.module_from_spec(_spec)
sys.modules["_ombreval_plan"] = plan
_spec.loader.exec_module(plan)

WALL = plan.WALL
screen = plan.screen
svg_points = plan.svg_points
district_for = plan.district_for
point_in_polygon = plan.point_in_polygon

# The ward map lands beside the cadastral plan in lore/places/, wherever this
# script is run from. Its provenance line self-corrects if the file moves.
BASE_SVG_PATH = plan.SVG_PATH
OUTPUT_PATH = plan.OUTPUT_DIR / "ombreval_ward_map.svg"
GENERATED_BY = Path(__file__).resolve().relative_to(plan.REPO_ROOT).as_posix()

# One entry per ward: colour, short label, and the "character" line from
# lore/places/00_city_plan.md. Bell-and-Sluice is the highlight ward.
WARDS: list[dict] = [
    {"key": "Bell and Sluice Wards", "short": "BELL & SLUICE", "fill": "#c1502a",
     "note": "Bellstand, Ilvane, bellfounding, Old Sluice", "hl": True},
    {"key": "Reed Ward", "short": "REED", "fill": "#2f7d78",
     "note": "Maren's Green, crypt, fish, moorings, boat-families", "hl": False},
    {"key": "Cloth Ward", "short": "CLOTH", "fill": "#8f4f74",
     "note": "Draper's Reach, tentering, cloth halls", "hl": False},
    {"key": "Fabric Ward", "short": "FABRIC", "fill": "#6f8a3f",
     "note": "Lanthorn, Gradine, Chapter, pilgrims", "hl": False},
    {"key": "Wick Ward", "short": "WICK", "fill": "#d19a1f",
     "note": "Wickmarket, west gate, chandlers, lodging", "hl": False},
    {"key": "Weigh Ward", "short": "WEIGH", "fill": "#b06a1f",
     "note": "Tallage, salt, Tally Bridge, customs, pawning", "hl": False},
    {"key": "Cinder Ward", "short": "CINDER", "fill": "#7a5a3a",
     "note": "Cinder Row, fire-conscious workshops, glass", "hl": False},
    {"key": "Wallwright Ward", "short": "WALLWRIGHT", "fill": "#556072",
     "note": "Coswald's Yard, stone, timber, lime, masons", "hl": False},
    {"key": "Outer wards", "short": "OUTER WARDS", "fill": "#8a7a4a",
     "note": "wall margins: bakers, brewers, gardens, reserves", "hl": False},
]

STEP = 2.0            # sampling / fill-band resolution in metres
WASH_OPACITY = 0.34   # translucency of the ordinary ward wash
HL_OPACITY = 0.44     # the highlight ward, a touch stronger

# String anchors that must exist in the cadastral output for the splice to work.
ANCHOR_LAYER = '<g id="direct-labels">'
ANCHOR_END = "</svg>"


def sample() -> tuple[dict[str, list[tuple[float, float, float]]], dict[str, list], float]:
    """Return per-ward horizontal fill runs, centroid accumulators, and cell count.

    Runs are ``(x, z_lo, z_hi)`` bands of constant world-x. The accumulator is
    ``[sum_x, sum_z, count]`` per ward, for centroid labels and areas.
    """
    xs = [p[0] for p in WALL]
    zs = [p[1] for p in WALL]
    x_min, x_max = min(xs), max(xs)
    z_min, z_max = min(zs), max(zs)

    runs: dict[str, list[tuple[float, float, float]]] = {w["key"]: [] for w in WARDS}
    accum: dict[str, list] = {w["key"]: [0.0, 0.0, 0] for w in WARDS}
    total = 0

    x = x_min
    while x <= x_max:
        run_ward: str | None = None
        run_lo = z_min
        prev_z = z_min
        z = z_min
        while z <= z_max:
            here = district_for(x, z) if point_in_polygon((x, z), WALL) else None
            if here is not None:
                total += 1
                a = accum[here]
                a[0] += x
                a[1] += z
                a[2] += 1
            if here != run_ward:
                if run_ward is not None:
                    runs[run_ward].append((x, run_lo, prev_z))
                run_ward = here
                run_lo = z
            prev_z = z
            z += STEP
        if run_ward is not None:
            runs[run_ward].append((x, run_lo, prev_z))
        x += STEP

    return runs, accum, float(total)


def overlay_group(runs) -> str:
    """The translucent ward wash, plus a crisp wall outline over it."""
    half = STEP / 2.0
    lines = ['<g id="ward-overlay">']
    for ward in WARDS:
        opacity = HL_OPACITY if ward["hl"] else WASH_OPACITY
        lines.append(f'<g fill="{ward["fill"]}" opacity="{opacity}" stroke="none" data-ward="{escape(ward["key"])}">')
        for x, z_lo, z_hi in runs[ward["key"]]:
            sx = -z_hi - half           # svg_x = -z; leftmost is the largest z
            width = (z_hi - z_lo) + STEP
            sy = -x - half              # svg_y = -x
            lines.append(f'<rect x="{sx:.1f}" y="{sy:.1f}" width="{width:.1f}" height="{STEP:.1f}"/>')
        lines.append('</g>')
    # Redraw the wall so the wash does not muddy the fortification line.
    lines.append(f'<polygon points="{svg_points(WALL)}" fill="none" stroke="#3d382e" stroke-width="5" opacity="0.55"/>')
    lines.append('</g>')
    return "\n".join(lines)


def ward_names(accum, total) -> tuple[str, list]:
    """Bold ward names + area/share, centred on each ward. Returns (svg, stats)."""
    cell = STEP * STEP
    lines = ['<g id="ward-names">']
    stats: list[tuple[dict, float, float]] = []
    for ward in WARDS:
        sum_x, sum_z, count = accum[ward["key"]]
        if count == 0:
            continue
        area = count * cell
        pct = 100.0 * count / total
        stats.append((ward, area, pct))
        cx, cz = sum_x / count, sum_z / count
        sx, sy = screen((cx, cz))
        size = 17 if ward["hl"] else 12
        name_style = (
            "font-family:Georgia,serif;font-weight:bold;letter-spacing:1.4px;fill:#241b12;"
            "text-anchor:middle;paint-order:stroke;stroke:#f6efdb;stroke-width:2.2px;"
            f"stroke-linejoin:round;font-size:{size}px"
        )
        stat_style = (
            "font-family:Arial,sans-serif;font-weight:bold;fill:#3a2a18;text-anchor:middle;"
            f"paint-order:stroke;stroke:#f6efdb;stroke-width:1.6px;font-size:{size - 4}px"
        )
        lines.append(f'<text x="{sx:.1f}" y="{sy:.1f}" style="{name_style}">{escape(ward["short"])}</text>')
        lines.append(f'<text x="{sx:.1f}" y="{sy + size + 1:.1f}" style="{stat_style}">{area / 1000:.0f}k m² · {pct:.1f}%</text>')
    lines.append('</g>')
    return "\n".join(lines), stats


def key_panel(stats, enclosure) -> str:
    """A compact ward colour key, below the cadastral map's own panels."""
    stats = sorted(stats, key=lambda item: -item[1])
    px, py, pw, ph = 735, 723, 350, 104
    panel_style = "fill:#f4ecd7;stroke:#574b39;stroke-width:1.2"
    title_style = "font-family:Georgia,serif;font-size:11px;font-weight:bold;fill:#30291f;letter-spacing:1px"
    text_style = "font-family:Arial,sans-serif;font-size:6.4px;fill:#30291f"
    small_style = "font-family:Arial,sans-serif;font-size:5.6px;fill:#514636"

    lines = [f'<rect x="{px}" y="{py}" width="{pw}" height="{ph}" rx="5" style="{panel_style}"/>']
    lines.append(f'<text x="{px + 15}" y="{py + 20}" style="{title_style}">PLANNING WARDS · SHARE OF ENCLOSURE</text>')
    # Two columns of swatch + label.
    col_x = [px + 15, px + 185]
    per_col = (len(stats) + 1) // 2
    for index, (ward, area, pct) in enumerate(stats):
        column = index // per_col
        row = index % per_col
        x = col_x[column]
        y = py + 36 + row * 14
        lines.append(f'<rect x="{x}" y="{y - 8}" width="16" height="10" rx="2" fill="{ward["fill"]}" opacity="{HL_OPACITY if ward["hl"] else WASH_OPACITY}" stroke="#6f5c3f" stroke-width="0.7"/>')
        lines.append(f'<text x="{x + 22}" y="{y}" style="{text_style}"><tspan font-weight="bold">{escape(ward["short"])}</tspan>  {area / 1000:.0f}k m² · {pct:.1f}%</text>')
    lines.append(f'<text x="{px + 15}" y="{py + ph - 8}" style="{small_style}">Regions = district_for() partition (non-overlapping); the overlapping boxes in 00_city_plan.md give Bell &amp; Sluice a smaller ~21%.</text>')
    return "\n".join(lines)


def retitle(svg: str) -> str:
    """Rename the cadastral map's title/subtitle to name the ward map."""
    replacements = [
        (
            "<title id=\"map-title\">Authoritative top-down building plan of Ombreval, F.437</title>",
            "<title id=\"map-title\">Ward map of Ombreval, F.437</title>",
        ),
        (
            "<desc id=\"map-desc\">A detailed cadastral city map showing every planned building"
            " footprint, streets, walls, gates, the dry Cut, the Serle outside the south wall, and"
            " numbered locations for all named places.</desc>",
            "<desc id=\"map-desc\">The authoritative cadastral plan of Ombreval with a translucent"
            " colour wash over its eight planning wards (the district_for() partition).</desc>",
        ),
        ("-566\">OMBREVAL</text>", "-566\">OMBREVAL — THE WARDS</text>"),
        ("AUTHORITATIVE TOP-DOWN BUILDING PLAN", "PLANNING WARDS OVER THE BUILDING PLAN"),
    ]
    for old, new in replacements:
        if old not in svg:
            raise ValueError(f"cadastral base map is missing an expected string to retitle: {old[:60]}...")
        svg = svg.replace(old, new, 1)
    return svg


def render() -> None:
    runs, accum, total = sample()
    enclosure = total * STEP * STEP

    # Render the authoritative base map, then read it back to compose onto.
    plan.render_svg()
    base = BASE_SVG_PATH.read_text(encoding="utf-8")
    if ANCHOR_LAYER not in base or not base.rstrip().endswith(ANCHOR_END):
        raise ValueError("cadastral base map does not have the expected layer structure to splice into")

    names_svg, stats = ward_names(accum, total)
    provenance = f"<!-- Ward overlay generated by {GENERATED_BY} over the cadastral base map. Re-run the script to regenerate. -->"

    out = base
    out = out.replace('<g id="map-art">', provenance + '\n<g id="map-art">', 1)
    out = out.replace(ANCHOR_LAYER, overlay_group(runs) + "\n" + ANCHOR_LAYER, 1)
    out = out.replace(ANCHOR_END, names_svg + "\n" + key_panel(stats, enclosure) + "\n" + ANCHOR_END, 1)
    out = retitle(out)

    OUTPUT_PATH.write_text(out, encoding="utf-8")
    ordered = ", ".join(f"{w['short']} {p:.1f}%" for w, _, p in sorted(stats, key=lambda i: -i[1]))
    print(f"Wrote {OUTPUT_PATH.name}: cadastral base + ward overlay · enclosure {enclosure:,.0f} m² · {ordered}")


if __name__ == "__main__":
    render()
