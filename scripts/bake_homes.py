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

The ~132 people with a homeless circumstance get no ``homes`` entry — the
absence of a home is content (``04_the_round.md`` §3): they are the ones still
in the street at the Snuffing, exactly whom the watch stops.

Each entry also carries a **readable place**, so the prompt can tell the model
where it lives without ids or coordinates
(``features/npc_knows_where_it_lives__inject_home_into_prompt.md``): the
building's ward-level ``district`` plus the nearest named place from the
wayfinding bake (``assets/world/places.json``), folded into a spoken
``place_description`` ("a house in the Cinder Ward, near the Shambles well").
The bedless get the same treatment under ``bedless``: an explicit no-fixed-bed
framing anchored to their spawn ("no fixed bed — you sleep rough in the Reed
Ward, near the Alder Moorings, wherever the watch will let you lie"), so the
LLM can *play* the circumstance instead of silently improvising a cottage.

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
PLACES_PATH = ROOT / "assets" / "world" / "places.json"
ROUNDS_PATH = ROOT / "assets" / "world" / "rounds.json"
OUT_PATH = ROOT / "assets" / "world" / "homes.json"

# A homeless circumstance means no bed — and being in the street at curfew is the
# point (04_the_round.md §3/§6). pauper(100)+unhoused(18)+insecure_lodging(14) is
# the ~132 the lore names. `enclosed_religious` is Dame Aldith the anchoress:
# not homeless, but her bed is the anchorhold cell she is bricked into, never a
# residential door — binding her to a house would give the curfew rung somewhere
# to march her (04_the_round.md §1: her Round has zero legs).
HOMELESS_CIRCUMSTANCES = {"pauper", "unhoused", "insecure_lodging", "enclosed_religious"}

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


def spoken(name: str) -> str:
    """A place name as it sits mid-sentence: 'The Tallage' → 'the Tallage'."""
    return "the " + name[4:] if name.startswith("The ") else name


# Beyond this, the nearest named place is a direction, not a neighbour:
# "toward the Ilvane anchorhold" instead of a misleading "near".
NEARBY_M = 120.0


def location_phrase(place: dict, distance: float) -> str:
    """How a resident hangs a spot on the nearest named place: 'near the
    Tallage', 'off Cinder Row', 'by the Wool Gate' — or, out in the unnamed
    streets, 'toward' whatever is closest."""
    if distance > NEARBY_M:
        return f"toward {spoken(place['name'])}"
    preposition = {"route": "off", "gate": "by", "bridge": "by"}.get(place["kind"], "near")
    return f"{preposition} {spoken(place['name'])}"


def bedless_description(circumstances: set[str], ward: str, place: dict, distance: float) -> str:
    """The explicit no-fixed-bed framing (the feature's 'the homeless are
    content here too'): the model should *know* the circumstance, not stare at
    an empty field. The anchoress is bedless in the bake but not homeless —
    her bed is the cell she is bricked into."""
    if "enclosed_religious" in circumstances:
        return f"the anchorhold cell at {spoken(place['name'])}; you are enclosed there for life"
    phrase = location_phrase(place, distance)
    where = f"in the {ward}, {phrase}" if ward else phrase
    if "insecure_lodging" in circumstances:
        return f"no bed of your own — a night-to-night lodging {where}, easily lost"
    return f"no fixed bed — you sleep rough {where}, wherever the watch will let you lie"


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

    # The wayfinding bake's named places (the speakable city) and ward names.
    places_doc = json.loads(PLACES_PATH.read_text())
    named_places = [
        {**place, "x": nodes[place["node"]][0], "z": nodes[place["node"]][1]}
        for place in places_doc["places"]
    ]
    ward_names = {ward["ward"]: ward["name"] for ward in places_doc["wards"]}

    def nearest_place(x: float, z: float, skip_names: str | None = None) -> tuple[dict, float]:
        best = None
        best_key = None
        for place in named_places:
            if skip_names and skip_names in place["name"].lower():
                continue
            d2 = (place["x"] - x) ** 2 + (place["z"] - z) ** 2
            key = (d2, place["id"])  # tie-break by place id for determinism
            if best_key is None or key < best_key:
                best_key = key
                best = place
        assert best is not None and best_key is not None
        return best, math.sqrt(best_key[0])

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
    bedless: dict[str, dict] = {}
    landmark_distances: list[float] = []
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

    # M5's off-map road-party actors live on the road between cities: they get
    # neither a home nor a bedless entry (round/tests.rs pins this contract).
    # Pre-shrink they fell out emergently; at 0.7x their gate-side spawns would
    # otherwise bind to city doors.
    road_members = {
        member
        for party in json.loads(ROUNDS_PATH.read_text())["road_parties"]
        for member in party["members"]
    }

    for c in characters:
        cid = c["id"]
        if cid == "player" or cid in road_members:
            continue
        spawn = c["spawn_location"]
        sx, sz = spawn["x"], spawn["z"]
        ward = c.get("planning_ward", "")

        circumstances = HOMELESS_CIRCUMSTANCES.intersection(c.get("circumstances", []))
        if circumstances:
            stats["homeless_by_circumstance"] += 1
            # The framing is anchored to the spawn: where they actually are is
            # where they actually sleep. The anchoress skips her own cell so
            # "the anchorhold cell at the anchorhold" never bakes.
            skip = "anchorhold" if "enclosed_religious" in circumstances else None
            place, distance = nearest_place(sx, sz, skip_names=skip)
            landmark_distances.append(distance)
            bedless[cid] = {
                "ward": ward_names.get(ward, ""),
                "landmark": place["name"],
                "place_description": bedless_description(
                    circumstances, ward_names.get(ward, ""), place, distance
                ),
            }
            continue

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
        place, distance = nearest_place(home["x"], home["z"])
        landmark_distances.append(distance)
        district_phrase = f"a house in the {home['district']}" if home["district"] else "a house"
        homes[cid] = {
            "building": home["id"],
            "edge": home["edge"],
            "door_node": home["node"],
            "point": [round(home["x"], 4), round(home["z"], 4)],
            "ward": home["district"],
            "landmark": place["name"],
            "place_description": f"{district_phrase}, {location_phrase(place, distance)}",
        }
        stats["housed"] += 1
        stats[source] += 1

    doc = {
        "schema_version": 2,
        "generated_by": "scripts/bake_homes.py",
        "source_seed": nav.get("source_seed"),
        "homes": dict(sorted(homes.items())),
        "bedless": dict(sorted(bedless.items())),
    }
    OUT_PATH.write_text(json.dumps(doc, indent=1) + "\n")

    print(f"wrote {OUT_PATH.relative_to(ROOT)}")
    print(f"  residential with a door: {len(residential)}")
    print(f"  housed: {stats['housed']} "
          f"(ward-local {stats['ward_local']}, fallback {stats['fallback']})")
    print(f"  homeless by circumstance: {stats['homeless_by_circumstance']}")
    print(f"  eligible but no candidate left: {stats['no_candidate']}")
    print(f"  nearest-landmark distance: "
          f"max {max(landmark_distances):.0f} m, "
          f"mean {sum(landmark_distances) / len(landmark_distances):.0f} m")


if __name__ == "__main__":
    main()
