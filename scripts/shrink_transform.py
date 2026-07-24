"""Ombreval 0.7x city-shrink transform (design v2, post-review wf_97253b96).

The authoritative cadastral tables in ``generate_top_down_map.py`` stay in their
**legacy** design coordinates (the 1200x1000 m layout). This module is the
single source of truth for the transform ``T`` that shrinks the *layout* to
~840x700 m at consumption time while keeping every building/prop SIZE unchanged
and leaving the sacred cathedral core (Lanthorn precinct, Gradine, statue
courts, Skinners' Court) exactly where ``scene.rs`` hard-codes it.

Every point is assigned exactly one transform CLASS:

    CORE    -> identity (the fixed core)
    SCALE   -> S * p                       (the ordinary 0.7x shrink)
    COSWALD -> S * p + (dx, 0)             (Coswald's Yard shifted east to
                                            reopen the masons'-lodge gap)
    cluster -> p + delta_c                 (a rigid ensemble translated so its
                                            anchor lands on S * anchor)
    OVERRIDE-> explicit hand-authored coordinates (the Draper's Reach ring, the
               masons/malt corner, the glaziers pair -- geometry T cannot make)

with ``delta_c = S * anchor_c - anchor_c = (S - 1) * anchor_c``.

Class assignment is EXPLICIT for every named building, site, fixture and place
mark (the tables below). Only road-polyline points are classified by a region
rule (``generate_top_down_map.py`` owns that, using ``CLUSTER_MEMBERS`` +
``CORE_BBOXES`` here), and even there the fragile points are pinned by the
explicit ``ROAD_*`` tables.

The module is import-safe from anywhere: ``import shrink_transform`` works
because uv puts the running script's directory on ``sys.path``.
"""

from __future__ import annotations

import math

Point = tuple[float, float]
Polygon = list[Point]

# --------------------------------------------------------------------------- #
#  Scale + local geometry helpers (must match generate_top_down_map.py::rect)  #
# --------------------------------------------------------------------------- #

S = 0.7
# Coswald's Yard scales like everything else, then shifts this far east so the
# masons' lodge fits the gap that reopens east of the fixed Skinners' Court.
COSWALD_SHIFT: Point = (45.0, 0.0)


def rect(cx: float, cz: float, size_x: float, size_z: float, angle_deg: float = 0.0) -> Polygon:
    """Rotated rectangle in world coordinates (identical to the generator's)."""
    hx = size_x / 2.0
    hz = size_z / 2.0
    angle = math.radians(angle_deg)
    ca = math.cos(angle)
    sa = math.sin(angle)
    return [
        (cx + lx * ca - lz * sa, cz + lx * sa + lz * ca)
        for lx, lz in ((-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz))
    ]


def scale(p: Point) -> Point:
    return (S * p[0], S * p[1])


def coswald(p: Point) -> Point:
    return (S * p[0] + COSWALD_SHIFT[0], S * p[1] + COSWALD_SHIFT[1])


def centroid(poly: list[Point]) -> Point:
    return (sum(p[0] for p in poly) / len(poly), sum(p[1] for p in poly) / len(poly))


def scale_about(poly: list[Point], factor: float, about: Point) -> Polygon:
    ax, az = about
    return [(ax + (x - ax) * factor, az + (z - az) * factor) for x, z in poly]


# --------------------------------------------------------------------------- #
#  Clusters: anchor -> delta = (S - 1) * anchor                                #
# --------------------------------------------------------------------------- #

CLUSTER_ANCHORS: dict[str, Point] = {
    # Riverside / customs ensembles (explicit anchors verified by review).
    "tallage_bridge": (-305.0, 105.0),   # bridge lands on the scaled Cut x=-213.5
    "gaunt": (-244.0, 33.0),             # 1.5 m toll/salt gap; copp 16.5 m off Cut
    "maren": (-315.0, -388.0),           # ox/moorings clear the scaled Cut channel
    "shambles": (-336.0, 318.0),         # inside the wall, 5 m off the Cut
    # Compact yards + towers.
    "seven_lofts": (360.0, 335.0),
    "bellfounders": (155.0, -485.0),
    "bellstand_tower": (64.0, -270.0),   # anchor = tower centre -> stays embedded
    "ilvane": (175.0, -92.0),
    # Bridges/sluice that span the Cut (single members, anchor = own centre).
    "old_sluice": (-305.0, -610.0),
    "chain_bridge": (-305.0, 425.0),
    # Gate tower pairs: anchor = opening centre so the road endpoint and the
    # opening translate together (0.00 m gap).
    "gate_wool": (-35.0, 510.0),
    "gate_stone": (495.0, 135.0),
    "gate_harne": (15.0, -665.0),
    "gate_river": (-505.0, -135.0),
    "gate_reed": (-455.0, -535.0),
    # Parish reserves (church + reserve rectangle share one delta).
    "parish_west": (105.0, 430.0),
    "parish_north_east": (330.0, -300.0),
    "parish_west_cut": (-220.0, 395.0),
    "parish_river": (-424.0, -35.0),     # nudged to clear river_west_lane
}


# A whole-cluster post-delta shift (applied uniformly to every member, its site,
# fixtures, marks and forced road points). Used where the pure anchor delta puts
# a rigid ensemble a metre too close to the wall/Cut: shambles slides east so
# shambles_1 clears the SW wall and the wall-lane has room to pass west of it.
CLUSTER_NUDGE: dict[str, Point] = {
    # (none currently -- the shambles keeps its review-verified anchor position;
    #  the SW wall-lane and river lane detour around the yard instead.)
}


def cluster_delta(name: str) -> Point:
    ax, az = CLUSTER_ANCHORS[name]
    nx, nz = CLUSTER_NUDGE.get(name, (0.0, 0.0))
    return ((S - 1.0) * ax + nx, (S - 1.0) * az + nz)


CLUSTER_DELTAS: dict[str, Point] = {name: cluster_delta(name) for name in CLUSTER_ANCHORS}


def translate(p: Point, name: str) -> Point:
    dx, dz = CLUSTER_DELTAS[name]
    return (p[0] + dx, p[1] + dz)


# Membership drives (a) rigid entity translation and (b) the road-point region
# rule (generator builds member bboxes from these ids). Site/fixture members are
# included so the region rule sees the full ensemble footprint.
CLUSTER_MEMBERS: dict[str, set[str]] = {
    "tallage_bridge": {"named_toll_house", "named_bonded_warehouse", "named_tally_bridge"},
    "gaunt": {
        "named_gaunt_house", "named_gaunt_salt_1", "named_copp_shop",
        "named_ferrant_house", "gaunt_yard", "chain_well",
    },
    "maren": {
        "named_saint_marens", "named_charnel_house", "named_moorings_1",
        "named_moorings_2", "named_moorings_3", "named_moorings_4",
        "named_eel_bridge", "named_hungry_ox", "named_brine_cellar",
        "saint_marens_yard", "tanners_yard", "alder_moorings", "reed_cistern",
    },
    "shambles": {f"named_shambles_{i}" for i in range(1, 8)} | {"shambles", "shambles_well"},
    "seven_lofts": {f"named_seven_lofts_{i}" for i in range(1, 8)} | {"seven_lofts", "seven_lofts_tanks"},
    "bellfounders": {f"named_bellfounders_{i}" for i in range(1, 6)} | {"bellfounders"},
    "bellstand_tower": {"named_bellstand_tower", "colm_stone", "bellstand_platform", "step_cistern"},
    "ilvane": {"named_ilvane_chapel", "named_anchorhold", "ilvane_plot"},
    "old_sluice": {"named_old_sluice"},
    "chain_bridge": {"named_chain_bridge"},
    "gate_wool": {"gate_wool_1", "gate_wool_2"},
    "gate_stone": {"gate_stone_1", "gate_stone_2"},
    "gate_harne": {"gate_harne_1", "gate_harne_2"},
    "gate_river": {"gate_river_1", "gate_river_2"},
    "gate_reed": {"gate_reed_1"},
    "parish_west": {"reserve_church_west", "west_parish"},
    "parish_north_east": {"reserve_church_north_east", "north_east_parish"},
    "parish_west_cut": {"reserve_church_west_cut", "west_cut_parish"},
    "parish_river": {"reserve_church_river", "river_parish"},
}

# Precedence when a road point sits in two cluster regions (small, specific
# clusters win over sprawling ones). Gates first: their road endpoints must ride
# the gate delta to keep the 0.00 m opening gap.
CLUSTER_PRECEDENCE: list[str] = [
    "gate_wool", "gate_stone", "gate_harne", "gate_river", "gate_reed",
    "bellstand_tower", "ilvane", "old_sluice", "chain_bridge",
    "tallage_bridge", "gaunt", "bellfounders", "seven_lofts",
    "parish_west", "parish_north_east", "parish_west_cut", "parish_river",
    "maren", "shambles",
]

# --------------------------------------------------------------------------- #
#  Core (identity) footprints                                                  #
# --------------------------------------------------------------------------- #

# Named buildings and sites that DO NOT MOVE (scene.rs hard-codes the 3D core).
CORE_SITE_IDS: set[str] = {
    "lanthorn_precinct", "gradine", "dawn_court", "seraph_court", "skinners_court",
}
CORE_NAMED_IDS: set[str] = {"named_lanthorn", "named_chapter_house"}
CORE_FIXTURE_IDS: set[str] = {"dawn_bearer", "seraph", "ford_well"}

# Padded bounding boxes (minx, maxx, minz, maxz) used ONLY to classify road
# polyline points as CORE. The forecourt box bridges the Gradine to the statue
# courts so ceremonial-strip road points stay put instead of scaling into the
# precinct. (The projection post-pass uses the real core polygons, not these.)
CORE_BBOXES: list[tuple[float, float, float, float]] = [
    (-81.0, 81.0, -118.0, 95.0),    # tight cathedral apron +8 (close-tightening)
    (-47.0, 47.0, 97.0, 216.0),     # Gradine + ceremonial forecourt
    (52.0, 92.0, 170.0, 210.0),     # Seraph court (r 14) +6
    (-92.0, -52.0, 170.0, 210.0),   # Dawn Bearer court (r 14) +6
    (81.1, 134.9, 96.8, 147.2),     # Skinners' Court +6
]

# 2026-07 close-tightening: the precinct site is no longer the 158x225 m rect —
# it is a 6 m paved apron following the cathedral's cross outline (west side
# held at z=104 so the Gradine still meets open steps at the west doors), and
# the statue courts shrink from r 20 to r 14. Ordinary fabric fills the rest.
PRECINCT_APRON: Polygon = [
    (-50.0, 104.0), (50.0, 104.0), (50.0, -1.0), (73.0, -1.0), (73.0, -45.0),
    (50.0, -45.0), (50.0, -110.0), (-50.0, -110.0), (-50.0, -45.0),
    (-73.0, -45.0), (-73.0, -1.0), (-50.0, -1.0),
]


def _court_circle(cx: float, cz: float, radius: float, steps: int = 20) -> Polygon:
    return [
        (cx + math.cos(i * math.tau / steps) * radius,
         cz + math.sin(i * math.tau / steps) * radius)
        for i in range(steps)
    ]


OVERRIDE_SITE_POLYGON: dict[str, Polygon] = {
    "lanthorn_precinct": PRECINCT_APRON,
    "dawn_court": _court_circle(-72.0, 190.0, 14.0),
    "seraph_court": _court_circle(72.0, 190.0, 14.0),
}


def in_core_bbox(p: Point) -> bool:
    x, z = p
    return any(minx <= x <= maxx and minz <= z <= maxz for minx, maxx, minz, maxz in CORE_BBOXES)


# --------------------------------------------------------------------------- #
#  Explicit entity class tables                                               #
# --------------------------------------------------------------------------- #

# id -> transform class for the 65 named buildings. "core"/"override"/a cluster.
NAMED_CLASS: dict[str, str] = {
    "named_lanthorn": "core",
    "named_chapter_house": "core",
    "named_toll_house": "tallage_bridge",
    "named_bonded_warehouse": "tallage_bridge",
    "named_tally_bridge": "tallage_bridge",
    "named_copp_shop": "gaunt",
    "named_ferrant_house": "gaunt",
    "named_gaunt_house": "gaunt",
    "named_gaunt_salt_1": "gaunt",
    "named_saint_marens": "maren",
    "named_charnel_house": "maren",
    "named_moorings_1": "maren",
    "named_moorings_2": "maren",
    "named_moorings_3": "maren",
    "named_moorings_4": "maren",
    "named_hungry_ox": "maren",
    "named_brine_cellar": "maren",
    "named_eel_bridge": "maren",
    "named_ilvane_chapel": "ilvane",
    "named_anchorhold": "ilvane",
    "named_bellstand_tower": "bellstand_tower",
    "named_old_sluice": "old_sluice",
    "named_chain_bridge": "chain_bridge",
    "reserve_church_west": "parish_west",
    "reserve_church_north_east": "parish_north_east",
    "reserve_church_west_cut": "parish_west_cut",
    "reserve_church_river": "parish_river",
    "gate_wool_1": "gate_wool",
    "gate_wool_2": "gate_wool",
    "gate_stone_1": "gate_stone",
    "gate_stone_2": "gate_stone",
    "gate_harne_1": "gate_harne",
    "gate_harne_2": "gate_harne",
    "gate_river_1": "gate_river",
    "gate_river_2": "gate_river",
    "gate_reed_1": "gate_reed",
    # Hand-authored overrides (see OVERRIDE_RECT).
    "named_masons_lodge": "override",
    "named_malt_house": "override",
    "named_glaziers_guild": "override",
    "named_sparr_workshop": "override",
    **{f"named_cloth_hall_{i}": "override" for i in range(1, 7)},
    **{f"named_seven_lofts_{i}": "seven_lofts" for i in range(1, 8)},
    **{f"named_shambles_{i}": "shambles" for i in range(1, 8)},
    **{f"named_bellfounders_{i}": "bellfounders" for i in range(1, 6)},
}

# Post-cluster nudges (applied AFTER the cluster delta) for individual members
# that the uniform delta leaves a metre too close to a road/wall.
ENTITY_NUDGE: dict[str, Point] = {
    "named_brine_cellar": (5.0, 0.0),   # clear the SW wall-lane and Tanners' Slip
    "named_moorings_1": (2.0, 0.0),     # open room east so the brine cellar fits
    "named_hungry_ox": (-2.0, 0.0),     # keep the tavern 1.5 m off the dry Cut
}

# id -> class for the 23 sites. "scale_centroid:<cluster>" scales the open yard
# about its own centroid THEN rides the cluster delta (rigid buildings, shrunk
# yard). "coswald" = scale + east shift. "burnt" = scale + small authored nudge.
SITE_CLASS: dict[str, str] = {
    "lanthorn_precinct": "override_polygon",
    "gradine": "core",
    "dawn_court": "override_polygon",
    "seraph_court": "override_polygon",
    "skinners_court": "core",
    "wickmarket": "scale",
    "coswalds_yard": "coswald",
    "tallage": "scale",
    "marens_green": "scale",
    "bellstand": "scale",
    "burnt_court": "burnt",
    "gaunt_yard": "gaunt",
    "saint_marens_yard": "maren",
    "alder_moorings": "scale_centroid:maren",
    "tanners_yard": "scale_centroid:maren",
    "ilvane_plot": "ilvane",
    "seven_lofts": "seven_lofts",
    "shambles": "scale_centroid:shambles",
    "bellfounders": "bellfounders",
    "west_parish": "parish_west",
    "north_east_parish": "parish_north_east",
    "west_cut_parish": "parish_west_cut",
    "river_parish": "parish_river",
}

# Small authored nudge for the scaled Burnt Court (west/north, clears the
# re-placed glaziers pair and the scaled Needle).
BURNT_COURT_NUDGE: Point = (-3.0, 4.0)

# id -> class for the 19 static fixtures (market-stall groups are regenerated).
FIXTURE_CLASS: dict[str, str] = {
    "dawn_bearer": "core",
    "seraph": "core",
    "ford_well": "core",
    "colm_stone": "bellstand_tower",
    "bellstand_platform": "bellstand_tower",
    "step_cistern": "bellstand_tower",
    "tallage_weighbeam": "scale",
    "tallage_stone": "scale",
    "coswald_tracing": "coswald",
    "coswald_crane": "coswald",
    "slate_cistern": "override",   # placed inside the scaled Wickmarket, off hall 1
    "tenter_cistern": "override",
    "lodge_well": "override",
    "three_curb": "scale",
    "chain_well": "gaunt",
    "reed_cistern": "maren",
    "bitter_well": "scale",
    "shambles_well": "shambles",
    "seven_lofts_tanks": "seven_lofts",
}

# --------------------------------------------------------------------------- #
#  Hand-authored OVERRIDE geometry (the three re-laid-out corners)            #
# --------------------------------------------------------------------------- #
#
# The Draper's Reach ring, the masons/malt corner and the glaziers pair cannot
# be produced by any single T delta (they land on the fixed seraph/dawn courts
# or crush against Skinners' Court). They are authored directly in the new,
# shrunk frame. Sizes/angles are kept from the legacy tables; only centres move.

# id -> (cx, cz, size_x, size_z, angle_deg) rebuilt in the shrunk frame.
OVERRIDE_RECT: dict[str, tuple[float, float, float, float, float]] = {
    # Six cloth halls flanking the re-routed covered way, NORTH of seraph court,
    # from the scaled Wickmarket SE corner down toward the shifted Coswald's
    # Yard. >=2 m gaps between halls; all clear of the fixed statue court.
    "named_cloth_hall_1": (37.0, 256.0, 24.0, 15.0, -34.0),
    "named_cloth_hall_2": (66.0, 246.0, 27.0, 16.0, -38.0),
    "named_cloth_hall_3": (96.0, 234.0, 28.0, 16.0, -41.0),
    "named_cloth_hall_4": (126.0, 220.0, 29.0, 16.0, -42.0),
    "named_cloth_hall_5": (154.0, 204.0, 27.0, 16.0, -39.0),
    "named_cloth_hall_6": (176.0, 186.0, 23.0, 15.0, -34.0),
    # Masons' lodge in the reopened gap east of Skinners' Court (west edge >=135),
    # with the malt-house stacked south of it (both clear of the shifted yard).
    "named_masons_lodge": (156.0, 150.0, 42.0, 31.0, -2.0),
    "named_malt_house": (150.0, 104.0, 27.0, 45.0, 5.0),
    # Glaziers' guild NORTH of the fixed Dawn Bearer court, Sparr workshop behind
    # it (north) so it clears the West Cut parish reserve to the west.
    "named_glaziers_guild": (-104.0, 232.0, 26.0, 37.0, -6.0),
    "named_sparr_workshop": (-112.0, 270.0, 18.0, 31.0, -7.0),
}

# Override fixture positions (kept size). Follow the re-authored corners.
OVERRIDE_FIXTURE: dict[str, Point] = {
    "tenter_cistern": (135.0, 188.0),   # tentering ground south of the covered way
    "lodge_well": (172.0, 120.0),       # Malt Passage, east of lodge/malt
    "slate_cistern": (8.0, 258.0),      # inside the scaled Wickmarket, west of hall 1
}

# Override place-mark anchors (mark follows the object it labels).
OVERRIDE_MARK: dict[int, Point] = {
    37: (100.0, 214.0),   # The Draper's Reach (mid covered way)
    38: (124.5, 196.0),   # Tenterhook Lane (mid-lane, clear of the Tenter Cistern's area)
    42: (-104.0, 232.0),  # The glaziers' guildhall
    43: (156.0, 150.0),   # The masons' lodge
    44: (170.0, 118.0),   # Malt Passage
    60: (8.0, 258.0),     # Slate Cistern
    61: (135.0, 188.0),   # Tenter Cistern
    62: (172.0, 120.0),   # Lodge Well
}

# --------------------------------------------------------------------------- #
#  Place-mark class table (mark follows the class of the object it labels)     #
# --------------------------------------------------------------------------- #

# mark number -> class token. "override" marks read OVERRIDE_MARK.
MARK_CLASS: dict[int, str] = {
    1: "core", 2: "core", 3: "core", 4: "core", 5: "core", 6: "core", 7: "core", 8: "core",
    9: "scale",             # The Wickmarket
    10: "coswald",          # Coswald's Yard
    11: "scale",            # The Tallage
    12: "scale",            # Maren's Green (the square scales)
    13: "scale",            # The Bellstand (the square scales)
    14: "scale",            # The Cut
    15: "chain_bridge",     # The Chain Bridge
    16: "tallage_bridge",   # The Tally Bridge
    17: "old_sluice",       # The Old Sluice
    18: "scale",            # The Cut ropewalk
    19: "shambles",         # The Shambles
    20: "tallage_bridge",   # Tallage toll-house
    21: "tallage_bridge",   # Bonded warehouse
    22: "gaunt",            # Lise Copp's pawnshop
    23: "gaunt",            # Doctor Ferrant's house
    24: "gaunt",            # The old Gaunt house
    25: "gaunt",            # Gaunt Passage
    26: "gaunt",            # Bonded weighing yard
    27: "maren",            # Saint Maren's church
    28: "maren",            # Saint Maren's churchyard
    29: "maren",            # The charnel door and crypt
    30: "maren",            # The Alder Moorings
    31: "maren",            # The Eel Bridge
    32: "maren",            # The Hungry Ox
    33: "maren",            # Tanners' Slip (road pinned to maren)
    34: "maren",            # Eelback Alley (road pinned to maren)
    35: "scale",            # Maren's Slip (verified: area+mark SCALE)
    36: "maren",            # Empty brine-rotted cellar
    37: "override", 38: "override",
    39: "scale",            # The Needle
    40: "scale",            # Cinder Row
    41: "burnt",            # Burnt Court
    42: "override", 43: "override", 44: "override",
    45: "scale",            # Crookneck Lane
    46: "scale",            # Osanne Vell's stall
    47: "ilvane",           # The Ilvane Chapel
    48: "ilvane",           # The Ilvane anchorhold
    49: "bellstand_tower",  # Bellfoot Passage
    50: "bellstand_tower",  # Bellstand watch-bell tower
    51: "bellstand_tower",  # Colm's stone
    52: "bellfounders",     # Bellfounders' Yard
    53: "gate_wool",        # The Wool Gate
    54: "gate_stone",       # The Stone Gate
    55: "gate_harne",       # The Harne Gate
    56: "gate_river",       # The River Gate
    57: "gate_reed",        # The Reed Postern
    58: "seven_lofts",      # Seven Lofts
    59: "scale",            # Outer Serle wharves
    60: "override",         # Slate Cistern (placed in the scaled Wickmarket)
    61: "override", 62: "override",
    63: "scale",            # Three-Curb
    64: "gaunt",            # Chain Well
    65: "maren",            # Reed Cistern
    66: "bellstand_tower",  # Step Cistern
    67: "scale",            # Bitter Well
    68: "shambles",         # The Shambles well
    69: "seven_lofts",      # Seven Lofts fire tanks
}

# --------------------------------------------------------------------------- #
#  Road-point overrides / pins                                                 #
# --------------------------------------------------------------------------- #
#
# ROAD_POINT_OVERRIDES: {road_id: {index: (x, z)}} explicit new coordinates.
# ROAD_FORCED_IDENTITY: {road_id: {indices}} keep the legacy point (no move).
# ROAD_FORCED_CLASS:    {road_id: {index: class}} force a class for one point.
# ROAD_FORCED_CLUSTER_WHOLE: {road_id: cluster} force a whole road into a cluster
#                           (points that miss the fragile region bbox by <1 m).

ROAD_POINT_OVERRIDES: dict[str, dict[int, Point]] = {
    "cinder_row": {0: (-48.0, 204.0), 3: (-74.6, 80.0)},
    "bell_way": {0: (72.0, -108.0), 1: (84.0, -118.0)},
    # Crookneck bends west of the relocated Ilvane chapel; the terminal LANDS ON
    # the north service ring's (90,-70) vertex so the graph keeps the pre-shrink
    # north-east crossing past the cathedral (an un-roaded gap here gets packed
    # with infill and severs Wool Gate -> Harne Gate to a 2.1x detour).
    "crookneck": {1: (100.0, -14.0), 2: (90.0, -70.0)},
    "west_approach": {4: (0.0, 178.0)},          # avoid a 0.5 m stub above (0,157)
    # The forecourt crossing must END on real junctions: west approach between
    # the statue courts, and the service ring's start. Scaling left both ends
    # floating 25-40 m short, killing the Wickmarket -> Bellstand center route.
    "fabric_south_cross": {0: (-7.0, 199.5), 3: (74.0, 91.0)},
    "river_east_lane": {2: (-300.0, -236.0)},    # pass west of moorings_4
    # A scaled cross-lane clipped the malt-house SE corner; drop it below the yard.
    # The terminal joins the service ring AT its start vertex — the old
    # 16 m open-ground gap is not bridged by the nav bake and infill may block it.
    "fabric_way": {3: (168.0, 72.0), 5: (74.0, 91.0)},
    # North-side cross-lanes skirt the re-authored ring and the relocated Lofts.
    "north_cross_1": {2: (205.0, 258.0), 3: (158.0, 236.0)},
    "cinder_west_cross": {1: (-104.0, 203.0)},   # south of the glaziers' guild
    # West Cut cross-link stops east of the Shambles (it can no longer pass west).
    "west_cut_link": {4: (-256.0, 268.0)},
    # Lift Tanners' Slip clear of the nudged brine cellar (rest rides maren).
    "tanners_slip": {2: (-303.0, -266.0)},
}

ROAD_FORCED_IDENTITY: dict[str, set[int]] = {
    "cinder_row": {4},           # (-52,122): stays out of the Gradine
    "tallage_to_gradine": {4},   # (-52,155): stays out of the Gradine
}

# Serve-Coswald road points get the same scale+east-shift as the yard.
ROAD_FORCED_CLASS: dict[str, dict[int, str]] = {
    "fabric_way": {2: "coswald"},   # index 3 is explicitly overridden below
    "crookneck": {0: "coswald"},
    "north_cross_2": {3: "coswald"},
}

# eelback / tanners_slip / reed_route tail ride the relocated Maren ensemble
# even though their end points miss the region bbox by a fraction of a metre.
ROAD_FORCED_CLUSTER_WHOLE: dict[str, str] = {
    "eelback": "maren",
    "tanners_slip": "maren",
}
# reed_route[2:] pinned to maren; [0] is the Reed gate, [1] is Maren's Slip (scale).
ROAD_FORCED_CLUSTER_POINT: dict[str, dict[int, str]] = {
    "reed_route": {2: "maren", 3: "maren"},
}

# Whole roads re-authored in the shrunk frame (the ring's covered way + lane).
ROAD_OVERRIDE_POINTS: dict[str, list[Point]] = {
    # 2026-07 close-tightening: the service ring hugs the 6 m cathedral apron
    # instead of the old 158x225 m precinct rect (the east lane still bulges to
    # (90,-70) around the chapter house at x 56..84). Ordinary fabric now packs
    # the freed close; connectors (fabric_way, fabric_south_cross, crookneck,
    # bell_way, cinder_row) share the new ring vertices below.
    "north_service": [(74.0, 91.0), (76.0, 55.0), (74.0, 0.0), (88.0, -48.0), (90.0, -70.0), (72.0, -108.0)],
    "south_service": [(-74.0, 91.0), (-76.0, 55.0), (-74.0, 5.0), (-76.0, -70.0), (-72.0, -108.0)],
    "apse_lane": [(-72.0, -108.0), (0.0, -118.0), (72.0, -108.0)],
    # Covered way flanked by the six halls, north of the fixed seraph court,
    # bending from the scaled Wickmarket corner toward the masons' corner.
    "drapers_reach": [(22.0, 241.0), (58.0, 232.0), (102.0, 212.0), (140.0, 192.0), (170.0, 174.0)],
    # Short tentering lane past the Tenter Cistern, between the halls and the lodge.
    "tenterhook": [(152.0, 172.0), (135.0, 188.0), (114.0, 204.0)],
    # These scaled minor roads would bulldoze the authored cloth-hall ranges, so
    # they route along the ring's north edge / end short of the relocated Lofts.
    "wick_north": [(-14.0, 272.0), (45.0, 278.0), (100.0, 266.0), (140.0, 246.0), (175.0, 214.0)],
    "west_residential_1": [(101.5, 336.0), (103.5, 281.0), (100.0, 258.0)],
    "west_residential_2": [(192.5, 329.0), (188.0, 285.0), (193.0, 258.0)],
    # The Shambles yard blocks the SW wall-to-Cut corridor at z 191..206, so the
    # river lane serves only south of the yard and the wall-lane detours inland,
    # down the Cut's west bank, around the yard (per review_geometry).
    "river_west_lane": [(-322.0, 182.0), (-300.0, 98.0), (-301.0, 14.0), (-332.5, -56.0)],
    "south_inner_wall": [
        (-311.5, -413.0), (-329.0, -294.0), (-336.0, -154.0), (-336.0, -10.0),
        (-270.0, 176.0), (-224.0, 188.0), (-224.0, 212.0), (-242.0, 258.0), (-272.0, 295.0),
    ],
}


# --------------------------------------------------------------------------- #
#  Point-class application                                                     #
# --------------------------------------------------------------------------- #

def apply_class(p: Point, cls: str) -> Point:
    """Transform a single point by a class token (not for OVERRIDE ids)."""
    if cls == "core":
        return p
    if cls == "scale":
        return scale(p)
    if cls == "coswald":
        return coswald(p)
    if cls == "burnt":
        sx, sz = scale(p)
        return (sx + BURNT_COURT_NUDGE[0], sz + BURNT_COURT_NUDGE[1])
    if cls in CLUSTER_DELTAS:
        return translate(p, cls)
    raise ValueError(f"unknown transform class {cls!r}")


def named_new_polygon(building_id: str, legacy_polygon: Polygon) -> Polygon:
    """New footprint for a named building: rigid translate, identity, or a
    re-authored rectangle for the OVERRIDE ids."""
    cls = NAMED_CLASS[building_id]
    if cls == "override":
        return rect(*OVERRIDE_RECT[building_id])
    if cls == "core":
        return list(legacy_polygon)
    dx, dz = CLUSTER_DELTAS[cls]
    nx, nz = ENTITY_NUDGE.get(building_id, (0.0, 0.0))
    return [(x + dx + nx, z + dz + nz) for x, z in legacy_polygon]


def site_new_polygon(site_id: str, legacy_polygon: Polygon) -> Polygon:
    """New footprint for a site by its explicit class."""
    cls = SITE_CLASS[site_id]
    if cls == "override_polygon":
        return list(OVERRIDE_SITE_POLYGON[site_id])
    if cls == "core":
        return list(legacy_polygon)
    if cls == "scale":
        return [scale(p) for p in legacy_polygon]
    if cls == "coswald":
        return [coswald(p) for p in legacy_polygon]
    if cls == "burnt":
        return [
            (sx + BURNT_COURT_NUDGE[0], sz + BURNT_COURT_NUDGE[1])
            for sx, sz in (scale(p) for p in legacy_polygon)
        ]
    if cls.startswith("scale_centroid:"):
        cluster = cls.split(":", 1)[1]
        shrunk = scale_about(legacy_polygon, S, centroid(legacy_polygon))
        return [translate(p, cluster) for p in shrunk]
    # plain cluster: rigid translate
    return [translate(p, cls) for p in legacy_polygon]


def fixture_new_position(fixture_id: str, legacy_position: Point) -> Point:
    cls = FIXTURE_CLASS[fixture_id]
    if cls == "override":
        return OVERRIDE_FIXTURE[fixture_id]
    return apply_class(legacy_position, cls)


def mark_new_anchor(number: int, legacy_anchor: Point) -> Point:
    cls = MARK_CLASS[number]
    if cls == "override":
        return OVERRIDE_MARK[number]
    return apply_class(legacy_anchor, cls)
