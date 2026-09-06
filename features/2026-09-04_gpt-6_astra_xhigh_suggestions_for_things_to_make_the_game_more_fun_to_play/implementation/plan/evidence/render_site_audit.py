# /// script
# requires-python = ">=3.11"
# dependencies = ["matplotlib==3.10.8"]
# ///
"""Render existing cadastral data for the plan. This does not survey a new route."""

from __future__ import annotations

import hashlib
import json
import math
import os
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", "/tmp/cathedral-plan-matplotlib")
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection
from matplotlib.patches import Patch, Polygon

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
PLAN_PATH = ROOT / "lore/places/ombreval_buildings.json"
NAV_PATH = ROOT / "assets/world/navigation.json"


def point(xz):
    """East right, north up: the authored world declares north=+x, east=-z."""
    x, z = xz
    return -(z - 63.0), x + 213.5


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    plan = json.loads(PLAN_PATH.read_text())
    nav = json.loads(NAV_PATH.read_text())
    assert plan["coordinate_system"]["north"] == "+x"
    assert plan["coordinate_system"]["east"] == "-z"
    buildings = {row["id"]: row for row in plan["buildings"]}
    door = next(row for row in nav["doors"] if row["building"] == "named_copp_shop")
    place = next(row for row in nav["places"] if row["name"] == "Lise Copp's pawnshop")
    door_xz = nav["nodes"][door["node"]]
    place_xz = nav["nodes"][place["node"]]
    mismatch = math.dist(door_xz, place_xz)

    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10})
    fig, ax = plt.subplots(figsize=(12, 11))
    fig.subplots_adjust(left=0.10, right=0.97, bottom=0.12, top=0.93)
    fig.patch.set_facecolor("#fbf8f0")
    ax.set_facecolor("#fbf8f0")

    selected = {
        "named_copp_shop": ("Pawnshop", "#cba35e"),
        "named_toll_house": ("Toll-house", "#91b3ad"),
        "named_bonded_warehouse": ("Bonded warehouse", "#91b3ad"),
        "named_tally_bridge": ("Tally Bridge\nupper passage", "#b6d0d6"),
    }
    for row in plan["buildings"]:
        points = [point(p) for p in row["polygon"]]
        if not any(-100 <= e <= 90 and -95 <= n <= 95 for e, n in points):
            continue
        highlight = selected.get(row["id"])
        bridge = row.get("use") == "bridge"
        poly = Polygon(
            points, closed=True,
            facecolor=highlight[1] if highlight else "#dedbd3",
            edgecolor="#6c746e" if highlight else "#aaa9a1",
            linewidth=1.2 if highlight else 0.5,
            alpha=0.58 if bridge else 0.92,
            hatch="///" if bridge else None,
            zorder=3 if highlight else 1,
        )
        ax.add_patch(poly)
        if highlight:
            center = tuple(sum(p[i] for p in points) / len(points) for i in range(2))
            ax.text(*center, highlight[0], ha="center", va="center", weight="bold", fontsize=9, zorder=6)

    segments = []
    for a, b, _half_width in nav["edges"]:
        p, q = point(nav["nodes"][a]), point(nav["nodes"][b])
        if any(-100 <= e <= 90 and -95 <= n <= 95 for e, n in (p, q)):
            segments.append((p, q))
    ax.add_collection(LineCollection(segments, colors="#779495", linewidths=0.65, alpha=0.7, zorder=2))

    de, dn = point(door_xz)
    pe, pn = point(place_xz)
    ax.plot([de, pe], [dn, pn], "--", color="#9c3e36", linewidth=1.6, zorder=7)
    ax.scatter([de], [dn], marker="s", s=65, color="#9c3e36", zorder=8)
    ax.scatter([pe], [pn], marker="o", s=55, color="#5a3d78", zorder=8)
    ax.annotate("Façade door destination", (de, dn), xytext=(55, -8),
                arrowprops={"arrowstyle": "->", "color": "#9c3e36"},
                color="#9c3e36", fontsize=9)
    ax.annotate("Named-place destination", (pe, pn), xytext=(57, 42),
                arrowprops={"arrowstyle": "->", "color": "#5a3d78"},
                color="#5a3d78", fontsize=9)

    fixture = next(row for row in plan["fixtures"] if row["id"] == "tallage_weighbeam")
    fixture_xz = fixture.get("point", fixture.get("position", fixture.get("center")))
    if fixture_xz is None:
        # This audited schema uses x/z fields for point fixtures.
        fixture_xz = [fixture["x"], fixture["z"]]
    fe, fn = point(fixture_xz)
    ax.scatter([fe], [fn], marker="D", s=45, color="#314d4d", zorder=8)
    ax.annotate("Weigh-beam", (fe, fn), xytext=(40, -27),
                arrowprops={"arrowstyle": "->", "color": "#314d4d"}, fontsize=9)

    ax.axhline(0, color="#d1caba", linewidth=0.6, zorder=0)
    ax.axvline(0, color="#d1caba", linewidth=0.6, zorder=0)
    ax.set_xlim(-100, 90)
    ax.set_ylim(-95, 95)
    ax.set_aspect("equal")
    ax.set_xlabel("East / metres from the Tallage anchor")
    ax.set_ylabel("North / metres from the Tallage anchor")
    ax.set_title("Existing Tallage: footprints, ground graph and two pawnshop destinations", loc="left", pad=18, weight="bold")
    ax.legend(handles=[
        Patch(facecolor="#dedbd3", edgecolor="#aaa9a1", label="Existing building footprint"),
        Patch(facecolor="#b6d0d6", hatch="///", edgecolor="#6c746e", label="Overhead bridge footprint"),
        plt.Line2D([0], [0], color="#779495", lw=1, label="Existing ground navigation graph"),
    ], loc="lower left", fontsize=8, framealpha=0.95)
    fig.text(0.06, 0.025,
             f"Door/place separation: {mismatch:.2f} m straight line. No proposed comparison room or private circuit is surveyed here.\n"
             "Sources: ombreval_buildings.json and navigation.json. A graph route is not a lower bound on all player movement.",
             fontsize=8, color="#515950")
    for extension in ("png", "svg"):
        fig.savefig(HERE / f"tallage_existing.{extension}", dpi=170, bbox_inches="tight")
    plt.close(fig)
    report = {
        "scope": "Existing source-data illustration only; no new geometry or route feasibility validated.",
        "sources": {str(p.relative_to(ROOT)): digest(p) for p in (PLAN_PATH, NAV_PATH)},
        "north": "+x", "east": "-z",
        "pawnshop_door_node": door["node"], "pawnshop_door_xz": door_xz,
        "pawnshop_place_node": place["node"], "pawnshop_place_xz": place_xz,
        "door_place_straight_distance_m": mismatch,
        "navigation_nodes": len(nav["nodes"]), "navigation_edges": len(nav["edges"]),
    }
    (HERE / "site_audit.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
