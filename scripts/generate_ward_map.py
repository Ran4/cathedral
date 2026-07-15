#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# ///
"""Generate Ombreval's *ward map* — a complementary top-down plan.

The authoritative cadastral plan (``generate_top_down_map.py`` ->
``ombreval_top_down_map.svg``) draws every building. This companion map draws
only the eight planning wards as filled coloured regions over the same walled
enclosure, so the districts themselves are legible at a glance.

The regions are the *real* partition the cadastral generator uses to assign
every building to a ward: ``district_for(x, z)`` — a first-match-wins decision
tree, not the overlapping bounding boxes tabulated in ``00_city_plan.md``. That
partition tiles the whole enclosure with no gaps or overlaps, so each ward's
share of the city is well defined. This script imports the authoritative
generator for its coordinate transform (``screen`` == ``(-z, -x)``), the wall
polygon, the point-in-polygon test, and the parchment palette, so the two maps
stay in exact registration.

Output: ``lore/places/ombreval_ward_map.svg`` (deterministic).

    uv run lore/places/generate_ward_map.py
"""

from __future__ import annotations

import importlib.util
import sys
from html import escape
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Import the authoritative generator (its sibling) as a module so the two maps
# share exactly one coordinate system, wall polygon, ward partition, repo-root
# discovery, and colour register.
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
inline = plan.inline_svg_class_styles

# The ward map lands beside the cadastral plan in lore/places/, wherever this
# script is run from. Its provenance line self-corrects if the file moves.
OUTPUT_PATH = plan.OUTPUT_DIR / "ombreval_ward_map.svg"
GENERATED_BY = Path(__file__).resolve().relative_to(plan.REPO_ROOT).as_posix()


# One entry per ward. Colour, and the short "character" line from
# lore/places/00_city_plan.md. Order controls the legend and the z-order of the
# label draw; Bell-and-Sluice is the highlight ward.
WARDS: list[dict] = [
    {"key": "Bell and Sluice Wards", "short": "BELL & SLUICE", "fill": "#c56a3e",
     "note": "Bellstand, Ilvane Chapel, bellfounding, Old Sluice, eastern housing", "hl": True},
    {"key": "Reed Ward", "short": "REED", "fill": "#5f8f8a",
     "note": "Maren's Green, crypt, fish, Moorings, tanners, boat-families", "hl": False},
    {"key": "Cloth Ward", "short": "CLOTH", "fill": "#9c6e86",
     "note": "Draper's Reach, tentering, cloth halls, merchant lofts", "hl": False},
    {"key": "Fabric Ward", "short": "FABRIC", "fill": "#9aa079",
     "note": "Lanthorn, Gradine, Chapter, pilgrims, fabric workers", "hl": False},
    {"key": "Wick Ward", "short": "WICK", "fill": "#cba64f",
     "note": "Wickmarket, west gate, chandlers, honey, lodging", "hl": False},
    {"key": "Weigh Ward", "short": "WEIGH", "fill": "#b98a4e",
     "note": "Tallage, salt houses, Tally Bridge, customs, pawning", "hl": False},
    {"key": "Cinder Ward", "short": "CINDER", "fill": "#93795f",
     "note": "Cinder Row, fire-conscious workshops, Burnt Court, glass", "hl": False},
    {"key": "Wallwright Ward", "short": "WALLWRIGHT", "fill": "#8b909c",
     "note": "Coswald's Yard, stone, timber, lime, masons, north gate", "hl": False},
    {"key": "Outer wards", "short": "OUTER WARDS", "fill": "#b7a888",
     "note": "wall margins: bakers, brewers, gardens, stables, parish reserves", "hl": False},
]

# Faint orientation anchors reused from the cadastral map (world x, z).
ANCHORS = [
    ("THE LANTHORN", (0, -12), "major"),
    ("THE BELLSTAND", (45, -255), "minor"),
    ("OLD SLUICE", (-305, -610), "minor"),
    ("COSWALD'S YARD", (255, 155), "minor"),
]
GATES = [
    ("WOOL GATE", (-35, 510)), ("STONE GATE", (495, 135)),
    ("HARNE GATE", (15, -665)), ("RIVER GATE", (-505, -135)),
    ("REED POSTERN", (-455, -535)),
]

STEP = 2.0  # sampling / fill-band resolution in metres


def sample() -> tuple[dict[str, list[tuple[float, float, float]]], dict[str, list], float]:
    """Return per-ward horizontal fill runs, centroids, and total cell count.

    Runs are ``(x, z_lo, z_hi)`` bands of constant world-x. Centroid data is a
    ``[sum_x, sum_z, count]`` accumulator per ward.
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


def render() -> None:
    runs, accum, total = sample()
    cell = STEP * STEP
    enclosure = total * cell

    # Reuse the cadastral header verbatim (same viewBox, palette, fonts), then
    # append ward-specific style and geometry before the closing tag.
    header = plan.svg_header().rstrip()
    # Splice ward styles in just before the stylesheet close.
    ward_css = "\n".join(
        f"    .fill-{i} {{ fill: {w['fill']}; }}" for i, w in enumerate(WARDS)
    )
    extra_css = (
        ward_css
        + "\n    .ward-fill { stroke: none; opacity: 0.52; }"
        + "\n    .ward-fill.hl { opacity: 0.66; }"
        + "\n    .ward-outline { fill: none; stroke: #6f5c3f; stroke-width: 1.1; opacity: 0.5; }"
        + "\n    .ward-name { font-family: Georgia, serif; font-weight: bold; letter-spacing: 1.3px;"
        " fill: #2c2318; text-anchor: middle; paint-order: stroke; stroke: #f4ecd7; stroke-width: 1.1px;"
        " stroke-linejoin: round; }"
        + "\n    .ward-name-hl { font-size: 15px; }"
        + "\n    .ward-name-sm { font-size: 11px; }"
        + "\n    .ward-stat { font-family: Arial, sans-serif; fill: #3a2f20; text-anchor: middle;"
        " paint-order: stroke; stroke: #f4ecd7; stroke-width: 0.9px; }"
        + "\n    .ward-stat-hl { font-size: 11px; }"
        + "\n    .ward-stat-sm { font-size: 7px; }"
        + "\n    .gate-mark { fill: #3b3328; stroke: #f4ecd7; stroke-width: 1; }"
    )
    header = header.replace("  ]]></style>", extra_css + "\n  ]]></style>")
    # New title/desc.
    header = header.replace(
        "<title id=\"map-title\">Authoritative top-down building plan of Ombreval, F.437</title>",
        "<title id=\"map-title\">Ward map of Ombreval, F.437</title>",
    ).replace(
        "<desc id=\"map-desc\">A detailed cadastral city map showing every planned building"
        " footprint, streets, walls, gates, the dry Cut, the Serle outside the south wall, and"
        " numbered locations for all named places.</desc>",
        "<desc id=\"map-desc\">The eight planning wards of Ombreval drawn as filled regions over"
        " the walled enclosure, using the cadastral generator's own district partition.</desc>",
    )

    parts: list[str] = [header]
    parts.append(
        f"<!-- Generated by {GENERATED_BY}. Do not edit by hand; re-run the script to regenerate. -->"
    )
    parts.append('<rect class="map-bg" x="-690" y="-610" width="1810" height="1440"/>')

    # Terrain / river context, dimmed, purely for orientation.
    parts.append('<rect class="outer-ground" x="-650" y="-545" width="1370" height="1250" opacity="0.6"/>')
    river_poly = [(-575, 640), (-575, -735), (-690, -735), (-690, 640)]
    parts.append(f'<polygon class="river" points="{svg_points(river_poly)}" opacity="0.7"/>')
    parts.append(f'<polygon class="city-ground" points="{svg_points(WALL)}"/>')

    # Ward fills (horizontal run bands per ward).
    parts.append('<g id="ward-fills">')
    half = STEP / 2.0
    for index, ward in enumerate(WARDS):
        hl = " hl" if ward["hl"] else ""
        parts.append(f'<g class="ward-fill fill-{index}{hl}" data-ward="{escape(ward["key"])}">')
        for x, z_lo, z_hi in runs[ward["key"]]:
            sx = -z_hi - half           # svg_x = -z; leftmost is the largest z
            width = (z_hi - z_lo) + STEP
            sy = -x - half              # svg_y = -x
            parts.append(
                f'<rect x="{sx:.1f}" y="{sy:.1f}" width="{width:.1f}" height="{STEP:.1f}"/>'
            )
        parts.append('</g>')
    parts.append('</g>')

    # Crisp wall outline on top so the ragged fill edge reads as intentional.
    parts.append(f'<polygon class="wall-casing" points="{svg_points(WALL)}"/>')
    parts.append(f'<polygon class="wall" points="{svg_points(WALL)}"/>')

    # Orientation anchors and gates.
    parts.append('<g id="anchors">')
    for text_value, point, weight in ANCHORS:
        sx, sy = screen(point)
        parts.append(f'<text class="direct-{weight}" x="{sx:.1f}" y="{sy:.1f}" opacity="0.72">{escape(text_value)}</text>')
    for text_value, point in GATES:
        sx, sy = screen(point)
        parts.append(f'<rect class="gate-mark" x="{sx - 4:.1f}" y="{sy - 4:.1f}" width="8" height="8"/>')
        parts.append(f'<text class="direct-minor" x="{sx:.1f}" y="{sy - 9:.1f}" opacity="0.8">{escape(text_value)}</text>')
    parts.append('</g>')

    # Ward name + area + share, centred on each ward's centroid.
    parts.append('<g id="ward-labels">')
    stats: list[tuple[dict, float, float]] = []  # (ward, area, pct) for the legend
    for ward in WARDS:
        sx_sum, sz_sum, count = accum[ward["key"]]
        if count == 0:
            continue
        area = count * cell
        pct = 100.0 * count / total
        stats.append((ward, area, pct))
        cx, cz = sx_sum / count, sz_sum / count
        sx, sy = screen((cx, cz))
        size = 15 if ward["hl"] else 11
        name_class = "ward-name ward-name-hl" if ward["hl"] else "ward-name ward-name-sm"
        stat_class = "ward-stat ward-stat-hl" if ward["hl"] else "ward-stat ward-stat-sm"
        parts.append(f'<text class="{name_class}" x="{sx:.1f}" y="{sy:.1f}">{escape(ward["short"])}</text>')
        parts.append(f'<text class="{stat_class}" x="{sx:.1f}" y="{sy + size + 1:.1f}">{area / 1000:.0f}k m² · {pct:.1f}%</text>')
    parts.append('</g>')

    # Title block, compass, scale (reuse the cadastral idiom).
    parts.append('<g id="map-information">')
    parts.append('<text class="map-title" x="-640" y="-566">OMBREVAL — THE WARDS</text>')
    parts.append('<text class="map-subtitle" x="-638" y="-548">EIGHT PLANNING WARDS · F.437 · NORTH +X · EAST −Z · REGIONS = district_for() PARTITION</text>')
    parts.append('<g transform="translate(-610,-505)"><path d="M 0 35 L 0 -20 L -7 -6 L 0 -28 L 7 -6 L 0 -20" fill="#3b3328"/><text class="panel-text" x="0" y="-34" text-anchor="middle">N +X</text><text class="panel-small" x="0" y="48" text-anchor="middle">south −X</text><text class="panel-small" x="-30" y="8" text-anchor="middle">W +Z</text><text class="panel-small" x="31" y="8" text-anchor="middle">E −Z</text></g>')
    parts.append('<g transform="translate(-620,690)"><path d="M 0 0 L 200 0" stroke="#30291f" stroke-width="3"/><path d="M 0 -5 L 0 5 M 50 -5 L 50 5 M 100 -5 L 100 5 M 150 -5 L 150 5 M 200 -5 L 200 5" stroke="#30291f" stroke-width="2"/><text class="panel-text" x="0" y="16">0</text><text class="panel-text" x="100" y="16" text-anchor="middle">100 m</text><text class="panel-text" x="200" y="16" text-anchor="end">200 m</text></g>')
    parts.append(f'<text class="panel-small" x="-620" y="730">Generated by {escape(GENERATED_BY)} · 1 SVG unit = 1 m</text>')

    # Legend / area table, sorted by share.
    stats.sort(key=lambda item: -item[1])
    panel_h = 60 + len(stats) * 30 + 60
    parts.append(f'<rect class="panel" x="735" y="-535" width="350" height="{panel_h}" rx="5"/>')
    parts.append('<text class="panel-title" x="750" y="-510">WARD AREAS · SHARE OF ENCLOSURE</text>')
    for row, (ward, area, pct) in enumerate(stats):
        y = -483 + row * 30
        parts.append(f'<rect x="752" y="{y - 9:.0f}" width="20" height="14" rx="2" fill="{ward["fill"]}" opacity="{0.66 if ward["hl"] else 0.52}" stroke="#6f5c3f" stroke-width="0.8"/>')
        parts.append(f'<text class="panel-text" x="780" y="{y:.0f}"><tspan font-weight="bold">{escape(ward["short"])}</tspan>  ·  {area / 1000:.0f}k m²  ·  {pct:.1f}%</text>')
        parts.append(f'<text class="panel-small" x="780" y="{y + 10:.0f}">{escape(ward["note"])}</text>')
    note_y = -483 + len(stats) * 30 + 6
    parts.append(f'<text class="panel-small" x="750" y="{note_y:.0f}">Enclosure ≈ {enclosure / 1e6:.2f} km². Regions are the cadastral generator\'s non-overlapping</text>')
    parts.append(f'<text class="panel-small" x="750" y="{note_y + 11:.0f}">district_for() partition (first-match-wins), NOT the overlapping boxes in 00_city_plan.md,</text>')
    parts.append(f'<text class="panel-small" x="750" y="{note_y + 22:.0f}">which give Bell &amp; Sluice a smaller 21% by bounding box.</text>')
    parts.append('</g>')

    parts.append('</svg>')
    svg = "\n".join(parts) + "\n"
    OUTPUT_PATH.write_text(inline(svg), encoding="utf-8")
    print(f"Wrote {OUTPUT_PATH.name}: enclosure {enclosure:,.0f} m², "
          + ", ".join(f"{w['short']} {p:.1f}%" for w, _, p in stats))


if __name__ == "__main__":
    render()
