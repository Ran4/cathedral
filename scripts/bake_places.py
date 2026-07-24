# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Bake ``assets/world/places.json`` — the wayfinding registry for M5's `go_to`.

The LLM never sees a coordinate: it walks to opaque ``place_id`` handles listed
on its sheet under ``places_you_know`` (``features/movement/05_the_llm_seam.md``
§3). This bake gives every nav place that handle, assigns it a planning ward
(so "the places of your own ward" can be seeded), and plants one walkable
anchor per ward (so "Reed Ward" is itself a coarse destination everyone holds).

A place's ward comes first from the authored ward bounds in
``lore/places/00_city_plan.md`` (nearest bound-centre when the rough rectangles
overlap), falling back to the ward of the nearest mapped building — the same
``building.district`` join ``bake_homes.py`` uses — for the odd place outside
every rectangle. A ward's anchor is the nav node nearest the centroid of its
buildings.

Ids are ``pl_`` + four base-36 characters from an FNV-1a hash of the entry's
stable key, bumped deterministically on collision. Opaque on purpose: an id
the model cannot derive from lore text is an id it does not guess at
(05_the_llm_seam.md §3 — we are steering behaviour, not just validating input).

Deterministic and byte-reproducible, like the other bakes.

Run: ``uv run scripts/bake_places.py`` (or plain ``python3``; no dependencies).
"""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILDINGS_PATH = ROOT / "lore" / "places" / "ombreval_buildings.json"
NAV_PATH = ROOT / "assets" / "world" / "navigation.json"
OUT_PATH = ROOT / "assets" / "world" / "places.json"

# building.district spellings → planning ward (the reverse of bake_homes.py's
# map). 'Outer wards' / 'City wall' / 'Parish reserve' belong to no ward and
# never classify a place.
DISTRICT_TO_WARD = {
    "Fabric Ward": "fabric",
    "Wick Ward": "wick",
    "Cloth Ward": "cloth",
    "Wallwright Ward": "wallwright",
    "Cinder Ward": "cinder",
    "Weigh Ward": "weigh",
    "Reed Ward": "reed",
    "Bell and Sluice Wards": "bell_and_sluice",
    "Bell Ward": "bell_and_sluice",
    "Sluice Ward": "bell_and_sluice",
}

# The authored approximate ward bounds (00_city_plan.md "Wards" table), as
# (x0, x1, z0, z1). They overlap — the classifier below breaks ties by
# distance to the bound's centre.
#
# 2026-07 0.7x city shrink (lore/places/shrink_transform.json): these are the
# pre-shrink rectangles scaled uniformly by 0.7 about the origin — they are
# open-ground district envelopes, not building-anchored geometry. Two known
# named-building ward drifts under the scaled boxes (review-verified, both
# consciously accepted):
#   - named_cloth_hall_6 (the hand-authored Draper's Reach re-layout) now
#     lands in the wallwright rectangle instead of cloth;
#   - reserve_church_north_east falls inside NO scaled rectangle and takes
#     the nearest-mapped-building fallback below.
WARD_BOUNDS = {
    "fabric": (-77.0, 126.0, -112.0, 164.5),
    "wick": (-119.0, 91.0, 164.5, 350.0),
    "cloth": (56.0, 192.5, 133.0, 273.0),
    "wallwright": (122.5, 339.5, 28.0, 182.0),
    "cinder": (-189.0, 14.0, 80.5, 220.5),
    "weigh": (-304.5, -122.5, -28.0, 164.5),
    "reed": (-318.5, -112.0, -350.0, -164.5),
    "bell_and_sluice": (-126.0, 231.0, -434.0, -105.0),
}

# Prompt-facing ward names — how people speak of them (00_city_plan.md's table).
WARD_NAMES = {
    "fabric": "Fabric Ward",
    "wick": "Wick Ward",
    "cloth": "Cloth Ward",
    "wallwright": "Wallwright Ward",
    "cinder": "Cinder Ward",
    "weigh": "Weigh Ward",
    "reed": "Reed Ward",
    "bell_and_sluice": "Bell and Sluice Wards",
}

ID_ALPHABET = "0123456789abcdefghijklmnopqrstuvwxyz"


def fnv1a64(text: str) -> int:
    value = 0xCBF29CE484222325
    for byte in text.encode("utf-8"):
        value = ((value ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def opaque_id(key: str, taken: set[str]) -> str:
    """`pl_` + four base-36 chars of the key's hash, salted until unclaimed."""
    salt = 0
    while True:
        value = fnv1a64(f"{key}#{salt}" if salt else key)
        chars = []
        for _ in range(4):
            chars.append(ID_ALPHABET[value % 36])
            value //= 36
        candidate = "pl_" + "".join(chars)
        if candidate not in taken:
            taken.add(candidate)
            return candidate
        salt += 1


def centroid(polygon: list[list[float]]) -> tuple[float, float]:
    n = len(polygon)
    return (sum(p[0] for p in polygon) / n, sum(p[1] for p in polygon) / n)


def main() -> None:
    nav = json.loads(NAV_PATH.read_text())
    nodes = nav["nodes"]
    buildings = json.loads(BUILDINGS_PATH.read_text())["buildings"]

    # The mapped buildings, as (ward, cx, cz), in file order (deterministic).
    mapped: list[tuple[str, float, float]] = []
    for b in buildings:
        ward = DISTRICT_TO_WARD.get(b.get("district", ""))
        if ward is None:
            continue
        cx, cz = centroid(b["polygon"])
        mapped.append((ward, cx, cz))

    def ward_of(x: float, z: float) -> str:
        # The authored bounds first: among the rectangles containing the point,
        # the one whose centre is nearest (they overlap on purpose).
        best_ward, best_d2 = "", float("inf")
        for ward, (x0, x1, z0, z1) in WARD_BOUNDS.items():
            if not (x0 <= x <= x1 and z0 <= z <= z1):
                continue
            cx, cz = (x0 + x1) / 2, (z0 + z1) / 2
            d2 = (cx - x) ** 2 + (cz - z) ** 2
            if d2 < best_d2:
                best_d2, best_ward = d2, ward
        if best_ward:
            return best_ward
        # Outside every bound (the gates, the wall line): the nearest mapped
        # building decides.
        for ward, cx, cz in mapped:
            d2 = (cx - x) ** 2 + (cz - z) ** 2
            if d2 < best_d2:
                best_d2, best_ward = d2, ward
        return best_ward

    def nearest_node(x: float, z: float) -> int:
        best, best_d2 = 0, float("inf")
        for index, (nx, nz) in enumerate(nodes):
            d2 = (nx - x) ** 2 + (nz - z) ** 2
            if d2 < best_d2:
                best_d2, best = d2, index
        return best

    taken: set[str] = set()
    places = []
    for place in nav["places"]:
        x, z = nodes[place["node"]]
        places.append(
            {
                "id": opaque_id(f"place:{place['name']}", taken),
                "name": place["name"],
                "node": place["node"],
                "kind": place["kind"],
                "ward": ward_of(x, z),
            }
        )

    wards = []
    for ward in WARD_NAMES:  # dict order is authoring order, fixed above
        members = [(cx, cz) for w, cx, cz in mapped if w == ward]
        cx = sum(p[0] for p in members) / len(members)
        cz = sum(p[1] for p in members) / len(members)
        wards.append(
            {
                "id": opaque_id(f"ward:{ward}", taken),
                "ward": ward,
                "name": WARD_NAMES[ward],
                "node": nearest_node(cx, cz),
            }
        )

    doc = {
        "schema_version": 1,
        "generated_by": "scripts/bake_places.py",
        "source_seed": nav.get("source_seed"),
        "places": places,
        "wards": wards,
    }
    OUT_PATH.write_text(json.dumps(doc, indent=1) + "\n")

    from collections import Counter

    by_ward = Counter(p["ward"] for p in places)
    print(f"wrote {OUT_PATH.relative_to(ROOT)}")
    print(f"  places: {len(places)}, wards: {len(wards)}, ids: {len(taken)}")
    print(f"  places per ward: {dict(sorted(by_ward.items()))}")


if __name__ == "__main__":
    main()
