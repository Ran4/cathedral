# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Bake ``assets/world/homes.json`` — the home-binding for the movement round (M4).

Every character who *should* have a bed is bound to one residential building's
baked door, so the daily round (``crates/cathedral-sim/src/round.rs``) can walk
them home at the Snuffing and out again at the Kindling. The join is
``character.planning_ward`` (eight snake_case wards) → ``building.district``
(ward-level strings), per ``features/movement/04_the_round.md`` §3.

The algorithm, deterministic and byte-reproducible like ``bake_navigation.py``:

    for each character, sorted by id:
        skip if a homeless circumstance (pauper / unhoused / insecure_lodging)
        candidates = unclaimed residential buildings in my ward with a baked door
        if the ward is exhausted, fall back to every unclaimed residential door
        home = the nearest candidate door (by XZ to my spawn), tie-broken by id
        claim it

The ~132 people with a homeless circumstance get *no* entry — the absence of a
home is content (``04_the_round.md`` §3): they are the ones still in the street
at the Snuffing, exactly whom the watch stops.

Run: ``uv run scripts/bake_homes.py`` (or plain ``python3``; no dependencies).
"""

from __future__ import annotations

import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHARACTERS_DIR = ROOT / "lore" / "characters"
BUILDINGS_PATH = ROOT / "lore" / "places" / "ombreval_buildings.json"
NAV_PATH = ROOT / "assets" / "world" / "navigation.json"
OUT_PATH = ROOT / "assets" / "world" / "homes.json"

# A homeless circumstance means no bed — and being in the street at curfew is the
# point (04_the_round.md §3/§6). pauper(100)+unhoused(18)+insecure_lodging(14) is
# the ~132 the lore names.
HOMELESS_CIRCUMSTANCES = {"pauper", "unhoused", "insecure_lodging"}

# planning_ward (character) → building.district spellings. bell_and_sluice
# absorbs its sub-spellings; 'Outer wards' / 'City wall' / 'Parish reserve' have
# no ward and are never a ward-local home (they remain in the global fallback).
WARD_TO_DISTRICTS = {
    "fabric": ["Fabric Ward"],
    "wick": ["Wick Ward"],
    "cloth": ["Cloth Ward"],
    "wallwright": ["Wallwright Ward"],
    "cinder": ["Cinder Ward"],
    "weigh": ["Weigh Ward"],
    "reed": ["Reed Ward"],
    "bell_and_sluice": ["Bell and Sluice Wards", "Bell Ward", "Sluice Ward"],
}


def load_characters() -> list[dict]:
    chars = []
    for path in sorted(CHARACTERS_DIR.rglob("*.json")):
        data = json.loads(path.read_text())
        chars.append(data)
    chars.sort(key=lambda c: c["id"])
    return chars


def main() -> None:
    characters = load_characters()
    buildings = json.loads(BUILDINGS_PATH.read_text())["buildings"]
    nav = json.loads(NAV_PATH.read_text())
    nodes = nav["nodes"]
    door_node_by_building = {d["building"]: (d["edge"], d["node"]) for d in nav["doors"]}

    # Residential buildings that actually have a baked (reachable) door.
    residential: list[dict] = []
    for b in buildings:
        if b.get("use") != "residential":
            continue
        door = door_node_by_building.get(b["id"])
        if door is None:
            continue
        edge, node = door
        x, z = nodes[node]
        residential.append(
            {"id": b["id"], "district": b.get("district", ""), "edge": edge, "node": node, "x": x, "z": z}
        )
    # Deterministic candidate order for tie-breaks.
    residential.sort(key=lambda b: b["id"])

    by_district: dict[str, list[dict]] = {}
    for b in residential:
        by_district.setdefault(b["district"], []).append(b)

    claimed: set[str] = set()
    homes: dict[str, dict] = {}
    stats = {"housed": 0, "homeless_by_circumstance": 0, "ward_local": 0, "fallback": 0, "no_candidate": 0}

    def nearest_unclaimed(pool: list[dict], sx: float, sz: float) -> dict | None:
        best = None
        best_key = None
        for b in pool:
            if b["id"] in claimed:
                continue
            d2 = (b["x"] - sx) ** 2 + (b["z"] - sz) ** 2
            key = (d2, b["id"])  # tie-break by building id for determinism
            if best_key is None or key < best_key:
                best_key = key
                best = b
        return best

    for c in characters:
        cid = c["id"]
        if cid == "player":
            continue
        if HOMELESS_CIRCUMSTANCES.intersection(c.get("circumstances", [])):
            stats["homeless_by_circumstance"] += 1
            continue
        spawn = c["spawn_location"]
        sx, sz = spawn["x"], spawn["z"]
        ward = c.get("planning_ward", "")

        ward_pool: list[dict] = []
        for district in WARD_TO_DISTRICTS.get(ward, []):
            ward_pool.extend(by_district.get(district, []))
        home = nearest_unclaimed(ward_pool, sx, sz)
        source = "ward_local"
        if home is None:
            # Ward exhausted (Weigh/Reed/Wick run short): fall back to the whole
            # city so nobody eligible is left bedless by a district deficit.
            home = nearest_unclaimed(residential, sx, sz)
            source = "fallback"
        if home is None:
            stats["no_candidate"] += 1
            continue

        claimed.add(home["id"])
        homes[cid] = {
            "building": home["id"],
            "edge": home["edge"],
            "door_node": home["node"],
            "point": [round(home["x"], 4), round(home["z"], 4)],
        }
        stats["housed"] += 1
        stats[source] += 1

    doc = {
        "schema_version": 1,
        "generated_by": "scripts/bake_homes.py",
        "source_seed": nav.get("source_seed"),
        "homes": dict(sorted(homes.items())),
    }
    OUT_PATH.write_text(json.dumps(doc, indent=1) + "\n")

    print(f"wrote {OUT_PATH.relative_to(ROOT)}")
    print(f"  residential with a door: {len(residential)}")
    print(f"  housed: {stats['housed']} "
          f"(ward-local {stats['ward_local']}, fallback {stats['fallback']})")
    print(f"  homeless by circumstance: {stats['homeless_by_circumstance']}")
    print(f"  eligible but no candidate left: {stats['no_candidate']}")


if __name__ == "__main__":
    main()
