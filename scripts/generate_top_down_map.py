#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# ///
"""Generate Ombreval's authoritative cadastral plan as SVG and JSON.

The output is deterministic. Changing the seed, generation rules, or source
geometry is a map revision: individual unnamed building IDs and footprints are
otherwise intended to remain stable for later world-building work.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass
from html import escape
import json
import math
from pathlib import Path
import random
import re
from typing import Iterable, Sequence


Point = tuple[float, float]  # world (x, z): north +x, west +z
Polygon = list[Point]
SEED = 0x0B437A1


def _repo_root() -> Path:
    """Locate the repository root so this script can move without breaking paths."""
    here = Path(__file__).resolve()
    for parent in (here, *here.parents):
        if (parent / ".git").exists():
            return parent
    return here.parent


REPO_ROOT = _repo_root()
# This generator's path relative to the repo root, stamped into every output so a
# reader can always find what produced them. Computed dynamically, so it stays
# correct if the script is moved (e.g. lore/places/ -> scripts/).
GENERATED_BY = Path(__file__).resolve().relative_to(REPO_ROOT).as_posix()
# The plan always lives in lore/places/, wherever this script is run from.
OUTPUT_DIR = REPO_ROOT / "lore" / "places"
SVG_PATH = OUTPUT_DIR / "ombreval_top_down_map.svg"
JSON_PATH = OUTPUT_DIR / "ombreval_buildings.json"
HTML_PATH = OUTPUT_DIR / "ombreval_top_down_map.html"


@dataclass(frozen=True)
class Road:
    id: str
    name: str
    points: list[Point]
    width_m: float
    tier: str
    label: bool = False


@dataclass(frozen=True)
class Site:
    id: str
    name: str
    polygon: Polygon
    kind: str


@dataclass
class Building:
    id: str
    name: str | None
    polygon: Polygon
    use: str
    material: str
    levels: int
    named: bool
    district: str


@dataclass(frozen=True)
class PlaceMark:
    number: int
    name: str
    anchor: Point
    kind: str


@dataclass(frozen=True)
class Fixture:
    id: str
    kind: str
    position: Point
    size: tuple[float, float]
    angle_deg: float = 0.0
    label: str | None = None


def rect(cx: float, cz: float, size_x: float, size_z: float, angle_deg: float = 0.0) -> Polygon:
    """Return a rotated rectangle in world coordinates."""
    hx = size_x / 2.0
    hz = size_z / 2.0
    angle = math.radians(angle_deg)
    ca = math.cos(angle)
    sa = math.sin(angle)
    result: Polygon = []
    for lx, lz in [(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)]:
        result.append((cx + lx * ca - lz * sa, cz + lx * sa + lz * ca))
    return result


def circle_poly(cx: float, cz: float, radius: float, steps: int = 20) -> Polygon:
    return [
        (
            cx + math.cos(index * math.tau / steps) * radius,
            cz + math.sin(index * math.tau / steps) * radius,
        )
        for index in range(steps)
    ]


WALL: Polygon = [
    (475, 485),
    (505, 130),
    (465, -650),
    (330, -675),
    (-445, -660),
    (-510, -120),
    (-475, 485),
]


ROADS: list[Road] = [
    Road("cut", "THE CUT", [(-305, 465), (-305, -605)], 20.0, "cut", True),
    Road(
        "west_approach",
        "WEST APPROACH",
        [(-35, 510), (-25, 420), (-25, 355), (-10, 285), (0, 225), (0, 157)],
        7.0,
        "major",
    ),
    Road(
        "fabric_way",
        "FABRIC WAY",
        [(495, 135), (360, 145), (255, 155), (180, 120), (115, 95), (72, 80)],
        7.0,
        "major",
        True,
    ),
    Road("river_cartway", "RIVER CARTWAY", [(-505, -135), (-415, -130), (-305, -125)], 8.5, "major", True),
    Road("bell_way", "BELL WAY", [(78, -112), (105, -165), (70, -205), (45, -255)], 5.5, "major", True),
    Road("harne_road", "HARNE ROAD", [(45, -255), (80, -360), (35, -505), (15, -665)], 6.5, "major", True),
    Road("reed_route", "MAREN'S SLIP", [(-455, -535), (-410, -485), (-375, -435), (-320, -390)], 3.5, "minor", True),
    Road("drapers_reach", "THE DRAPER'S REACH", [(35, 320), (100, 270), (175, 215), (210, 185)], 5.0, "passage", True),
    Road("tenterhook", "TENTERHOOK LANE", [(95, 220), (30, 195), (-35, 185)], 4.5, "minor", True),
    Road("needle", "THE NEEDLE", [(-75, 310), (-130, 270), (-162, 230), (-225, 160), (-270, 120)], 1.2, "alley", True),
    Road("cinder_row", "CINDER ROW", [(-80, 285), (-125, 240), (-135, 165), (-105, 115), (-52, 122)], 6.0, "minor", True),
    Road("crookneck", "CROOKNECK LANE", [(205, 60), (165, -15), (105, -85)], 4.2, "minor", True),
    Road("tanners_slip", "TANNERS' SLIP", [(-340, -300), (-390, -330), (-405, -390), (-380, -425)], 3.5, "minor", True),
    Road("eelback", "EELBACK ALLEY", [(-350, -410), (-370, -445), (-345, -482)], 2.4, "alley", True),
    Road("tallage_to_gradine", "", [(-270, 120), (-220, 135), (-160, 140), (-100, 150), (-52, 155)], 5.5, "major"),
    Road("east_cut_to_bell", "", [(-305, -220), (-220, -215), (-140, -205), (-60, -230), (5, -255)], 5.0, "minor"),
    Road("maren_to_bell", "", [(-250, -365), (-165, -340), (-80, -320), (5, -275)], 4.5, "minor"),
    Road("north_service", "", [(78, 95), (88, 60), (83, 5), (90, -70), (78, -112)], 4.5, "service"),
    Road("south_service", "", [(-78, 95), (-88, 55), (-83, 0), (-90, -70), (-78, -112)], 4.5, "service"),
    Road("apse_lane", "", [(-78, -112), (0, -130), (78, -112)], 4.0, "service"),
    Road("wick_north", "", [(-20, 390), (55, 375), (125, 335), (190, 270), (230, 205)], 4.5, "minor"),
    Road("loft_lane", "", [(-35, 450), (80, 445), (200, 420), (360, 335), (450, 205)], 5.0, "minor"),
    Road("stone_west_spur", "", [(465, 175), (400, 245), (360, 335), (300, 405), (220, 455)], 4.5, "minor"),
    Road("north_east_arc", "", [(440, 20), (365, 5), (300, -30), (245, -82), (205, -150)], 4.5, "minor"),
    Road("east_wall_road", "", [(420, -150), (360, -225), (330, -300), (350, -430), (410, -560)], 4.5, "minor"),
    Road("bellfounder_spur", "", [(80, -360), (155, -485), (260, -555), (385, -590)], 5.0, "minor"),
    Road("west_cut_link", "", [(-25, 420), (-120, 425), (-220, 410), (-305, 390), (-405, 390)], 4.5, "minor"),
    Road("shambles_lane", "", [(-210, 330), (-305, 320), (-395, 315), (-450, 300)], 5.0, "minor"),
    Road("river_west_lane", "", [(-455, 375), (-430, 260), (-420, 140), (-430, 20), (-475, -80)], 4.5, "minor"),
    Road("river_east_lane", "", [(-470, -180), (-430, -270), (-390, -345), (-410, -485)], 4.0, "minor"),
    Road("west_residential_1", "", [(145, 480), (135, 410), (150, 345), (135, 280)], 3.8, "minor"),
    Road("west_residential_2", "", [(275, 470), (260, 390), (265, 310), (245, 225)], 3.8, "minor"),
    Road("north_cross_1", "", [(440, 400), (360, 360), (280, 330), (210, 300)], 3.8, "minor"),
    Road("north_cross_2", "", [(450, 80), (365, 105), (315, 80), (260, 45)], 3.8, "minor"),
    Road("east_residential_1", "", [(290, -100), (235, -175), (210, -255), (220, -340)], 3.8, "minor"),
    Road("east_residential_2", "", [(425, -350), (335, -365), (260, -390), (185, -420)], 3.8, "minor"),
    Road("fabric_south_cross", "", [(-55, 240), (-20, 210), (35, 190), (100, 170)], 3.6, "minor"),
    Road("cinder_west_cross", "", [(-65, 330), (-145, 315), (-220, 285), (-305, 260)], 4.0, "minor"),
    Road("weigh_north_lane", "", [(-180, 210), (-190, 130), (-195, 55), (-218, 18), (-230, -45)], 4.0, "minor"),
    Road("river_ward_cross", "", [(-455, 80), (-390, 40), (-320, 15), (-250, -5), (-180, -30)], 4.0, "minor"),
    Road("cut_south_west", "", [(-360, 455), (-385, 350), (-400, 250), (-405, 150)], 3.8, "minor"),
    Road("cut_south_mid", "", [(-350, 80), (-385, 25), (-415, -40), (-460, -95)], 3.8, "minor"),
    Road("reed_cross", "", [(-440, -250), (-380, -260), (-320, -275), (-250, -290)], 3.8, "minor"),
    Road("maren_north_lane", "", [(-205, -250), (-210, -320), (-235, -382), (-210, -455)], 3.8, "minor"),
    Road("sluice_lane", "", [(-250, -520), (-305, -570), (-305, -610), (-245, -640)], 4.0, "minor"),
    Road("west_inner_wall", "", [(430, 455), (250, 485), (50, 490), (-170, 485), (-390, 455)], 4.0, "wall_lane"),
    Road("north_inner_wall", "", [(455, 430), (475, 220), (475, 10), (450, -220), (435, -485)], 4.0, "wall_lane"),
    Road("east_inner_wall", "", [(415, -610), (220, -635), (20, -640), (-190, -635), (-390, -620)], 4.0, "wall_lane"),
    Road("south_inner_wall", "", [(-445, -590), (-470, -420), (-480, -220), (-480, -20), (-450, 210), (-430, 430)], 4.0, "wall_lane"),
]


SITES: list[Site] = [
    Site("lanthorn_precinct", "The Lanthorn precinct", rect(0, -12, 158, 225), "precinct"),
    Site("gradine", "The Gradine", [(-39, 105), (39, 105), (39, 157), (-39, 157)], "square"),
    Site("dawn_court", "Dawn Bearer court", circle_poly(-72, 190, 20), "monument"),
    Site("seraph_court", "Seraph court", circle_poly(72, 190, 20), "monument"),
    Site("skinners_court", "Skinners' Court", rect(108, 122, 38, 34, -7), "court"),
    Site("wickmarket", "The Wickmarket", [(-66, 388), (22, 394), (30, 344), (12, 318), (-58, 321), (-70, 352)], "square"),
    Site("coswalds_yard", "Coswald's Yard", [(195, 112), (310, 108), (319, 170), (292, 203), (214, 199), (193, 166)], "square"),
    Site("tallage", "The Tallage", [(-359, 145), (-250, 150), (-245, 78), (-268, 34), (-350, 38), (-366, 92)], "square"),
    Site("marens_green", "Maren's Green", [(-361, -315), (-252, -309), (-245, -375), (-269, -421), (-351, -420), (-365, -370)], "square"),
    Site("bellstand", "The Bellstand", [(2, -219), (87, -217), (94, -270), (72, -297), (12, -292), (-1, -256)], "square"),
    Site("burnt_court", "Burnt Court", rect(-172, 232, 36, 32, 5), "court"),
    Site("gaunt_yard", "Gaunt weighing yard", rect(-195, -2, 46, 42, 4), "yard"),
    Site("saint_marens_yard", "Saint Maren's churchyard", rect(-267, -360, 48, 55, -5), "churchyard"),
    Site("alder_moorings", "The Alder Moorings", rect(-367, -407, 70, 62, 3), "yard"),
    Site("tanners_yard", "Tanners' work yards", rect(-400, -360, 70, 125, 5), "yard"),
    Site("ilvane_plot", "The Ilvane Chapel plot", rect(175, -92, 43, 61, 2), "churchyard"),
    Site("seven_lofts", "Seven Lofts", rect(360, 335, 110, 88, -5), "yard"),
    Site("shambles", "The Shambles", rect(-395, 315, 94, 76, 2), "yard"),
    Site("bellfounders", "Bellfounders' Yard", rect(155, -485, 82, 68, -4), "yard"),
    Site("west_parish", "West parish reserve", rect(105, 430, 48, 58, -3), "parish_reserve"),
    Site("north_east_parish", "North-east parish reserve", rect(330, -300, 50, 60, 5), "parish_reserve"),
    Site("west_cut_parish", "West Cut parish reserve", rect(-220, 395, 48, 58, -4), "parish_reserve"),
    Site("river_parish", "River Ward parish reserve", rect(-420, -35, 50, 60, 3), "parish_reserve"),
]


def add_named_buildings() -> list[Building]:
    buildings: list[Building] = []

    def add(
        key: str,
        name: str,
        polygon: Polygon,
        use: str,
        material: str,
        levels: int,
        district: str,
    ) -> None:
        buildings.append(Building(key, name, polygon, use, material, levels, True, district))

    lanthorn = [
        (-44, 81), (44, 81), (44, -7), (67, -7), (67, -39), (44, -39),
        (44, -104), (-44, -104), (-44, -39), (-67, -39), (-67, -7), (-44, -7),
    ]
    add("named_lanthorn", "The Lanthorn", lanthorn, "ecclesiastical", "limestone", 6, "Fabric Ward")
    add("named_chapter_house", "Chapter house and Grey Press", rect(70, -69, 28, 43, 3), "ecclesiastical", "limestone", 3, "Fabric Ward")
    add("named_masons_lodge", "The masons' lodge", rect(207, 151, 42, 31, -2), "guild", "limestone", 3, "Wallwright Ward")
    add("named_malt_house", "The malt-house over Malt Passage", rect(234, 111, 27, 45, 5), "trade", "half_timber", 3, "Wallwright Ward")
    add("named_glaziers_guild", "The glaziers' guildhall", rect(-141, 237, 26, 37, -6), "guild", "stone_timber", 3, "Cinder Ward")
    add("named_sparr_workshop", "The Sparr workshop", rect(-115, 250, 18, 31, -7), "workshop", "stone_timber", 3, "Cinder Ward")

    # Draper's Reach: six individually placed cloth-hall ranges flanking the covered way.
    for index, (cx, cz, angle, sx, sz) in enumerate([
        (48, 334, -34, 24, 15), (78, 306, -38, 27, 16), (111, 278, -41, 28, 16),
        (143, 247, -42, 29, 16), (175, 218, -39, 27, 16), (204, 194, -34, 23, 15),
    ], 1):
        add(f"named_cloth_hall_{index}", f"Draper's Reach cloth hall {index}", rect(cx, cz, sx, sz, angle), "trade", "stone_timber", 3, "Cloth Ward")

    add("named_toll_house", "The Tallage toll-house", rect(-268, 105, 33, 41, 0), "civic", "limestone", 3, "Weigh Ward")
    add("named_bonded_warehouse", "The Tallage bonded warehouse", rect(-345, 105, 42, 51, 0), "storage", "fieldstone", 4, "Weigh Ward")
    add("named_copp_shop", "Lise Copp's pawnshop", rect(-262, 43, 15, 27, 3), "trade", "stone_timber", 3, "Weigh Ward")
    add("named_ferrant_house", "Doctor Ferrant's house and study", rect(-242, 20, 18, 30, -4), "trade", "plaster", 3, "Weigh Ward")
    gaunt_house = [
        (-240, -6), (-195, -6), (-195, 42), (-240, 42),
        (-240, 36), (-231, 36), (-231, 4), (-240, 4),
    ]
    add("named_gaunt_house", "The old Gaunt house", gaunt_house, "storage", "fieldstone", 4, "Weigh Ward")
    add("named_gaunt_salt_1", "Gaunt salt cellar west range", rect(-220, 58, 22, 27, 3), "storage", "fieldstone", 2, "Weigh Ward")

    maren_church = [
        (-258, -353), (-213, -353), (-213, -374), (-201, -374), (-201, -393),
        (-213, -393), (-213, -415), (-258, -415), (-258, -397), (-270, -397),
        (-270, -371), (-258, -371),
    ]
    add("named_saint_marens", "The Church of Saint Maren of the Reeds", maren_church, "ecclesiastical", "fieldstone", 3, "Reed Ward")
    add("named_charnel_house", "Saint Maren's charnel house", rect(-278, -365, 12, 18, 0), "ecclesiastical", "fieldstone", 1, "Reed Ward")
    for index, (cx, cz, sx, sz, angle) in enumerate([
        (-392, -407, 20, 54, 2), (-343, -430, 46, 18, 2), (-342, -388, 20, 29, 2), (-370, -371.5, 33, 15, 2),
    ], 1):
        add(f"named_moorings_{index}", f"Alder Moorings warehouse range {index}", rect(cx, cz, sx, sz, angle), "storage", "half_timber", 3, "Reed Ward")
    add("named_hungry_ox", "The Hungry Ox", rect(-330, -455, 20, 28, -5), "tavern", "half_timber", 3, "Reed Ward")
    add("named_brine_cellar", "The empty brine-rotted cellar", rect(-415, -405, 18, 21, 4), "industrial", "fieldstone", 1, "Reed Ward")

    add("named_ilvane_chapel", "The Ilvane Chapel", rect(175, -92, 34, 51, 2), "ecclesiastical", "fieldstone", 2, "Bell Ward")
    add("named_anchorhold", "The Ilvane anchorhold", rect(194, -92, 8, 19, 2), "ecclesiastical", "fieldstone", 1, "Bell Ward")
    add("named_bellstand_tower", "The Bellstand watch-bell tower", rect(64, -270, 22, 25, 0), "civic", "limestone", 5, "Bell Ward")

    # Seven Lofts: seven separately roofed granary bays around the yard.
    for index, (cx, cz, sx, sz, angle) in enumerate([
        (392, 358, 21, 48, -5), (366, 370, 20, 45, -5), (338, 372, 21, 42, -5),
        (320, 338, 42, 18, -5), (332, 306, 20, 43, -5), (362, 299, 21, 45, -5),
        (395, 310, 22, 47, -5),
    ], 1):
        add(f"named_seven_lofts_{index}", f"Seven Lofts bay {index}", rect(cx, cz, sx, sz, angle), "storage", "fieldstone", 4, "Cloth Ward")

    # The Shambles is a complex, not one implausibly huge building.
    for index, (cx, cz, sx, sz, angle, use) in enumerate([
        (-427, 339, 18, 47, 2, "industrial"), (-405, 348, 17, 42, 2, "trade"),
        (-378, 348, 22, 43, 2, "trade"), (-356, 329, 17, 34, 2, "industrial"),
        (-350, 294, 41, 15, 2, "industrial"), (-395, 284, 44, 16, 2, "industrial"),
        (-428, 293, 18, 27, 2, "storage"),
    ], 1):
        add(f"named_shambles_{index}", f"Shambles range {index}", rect(cx, cz, sx, sz, angle), use, "fieldstone", 2, "Cinder Ward")

    for index, (cx, cz, sx, sz, angle, use) in enumerate([
        (181, -501, 22, 47, -4, "industrial"), (151, -510, 25, 38, -4, "industrial"),
        (123, -499, 20, 43, -4, "storage"), (126, -463, 38, 17, -4, "workshop"),
        (169, -458, 43, 16, -4, "storage"),
    ], 1):
        add(f"named_bellfounders_{index}", f"Bellfounders' Yard range {index}", rect(cx, cz, sx, sz, angle), use, "fieldstone", 2, "Bell Ward")

    # Dry hydraulic structures and overhead bridges.
    add("named_old_sluice", "The Old Sluice", rect(-305, -610, 50, 42, 0), "civic", "limestone", 3, "Sluice Ward")
    add("named_chain_bridge", "The Chain Bridge upper store", rect(-305, 425, 44, 12, 0), "bridge", "stone_timber", 2, "Cinder Ward")
    add("named_tally_bridge", "The Tally Bridge upper passage", rect(-305, 105, 61, 14, 0), "bridge", "limestone", 2, "Weigh Ward")
    add("named_eel_bridge", "The Eel Bridge gallery", rect(-343, -374, 12, 31, 0), "bridge", "half_timber", 2, "Reed Ward")

    # Reserved but deliberately unnamed parish churches.
    for key, name, cx, cz, angle in [
        ("west", "West parish church reserve", 105, 430, -3),
        ("north_east", "North-east parish church reserve", 330, -300, 5),
        ("west_cut", "West Cut parish church reserve", -220, 395, -4),
        ("river", "River Ward parish church reserve", -420, -35, 3),
    ]:
        add(f"reserve_church_{key}", name, rect(cx, cz, 24, 38, angle), "ecclesiastical", "fieldstone", 2, "Parish reserve")

    # Gate towers are buildings and receive stable IDs even when the gate is one named place.
    for key, name, tower_specs in [
        ("wool", "Wool Gate", [(-17, 510, 22, 25, 0), (-53, 510, 22, 25, 0)]),
        ("stone", "Stone Gate", [(495, 112, 24, 22, 0), (495, 158, 24, 22, 0)]),
        ("harne", "Harne Gate", [(33, -665, 22, 25, 0), (-3, -665, 22, 25, 0)]),
        ("river", "River Gate", [(-505, -164, 24, 25, 0), (-505, -106, 24, 25, 0)]),
    ]:
        for index, (cx, cz, sx, sz, angle) in enumerate(tower_specs, 1):
            add(f"gate_{key}_{index}", f"{name} tower {index}", rect(cx, cz, sx, sz, angle), "fortification", "limestone", 4, "City wall")
    add("gate_reed_1", "Reed Postern tower", rect(-455, -535, 20, 24, -7), "fortification", "limestone", 3, "City wall")

    return buildings


NAMED_BUILDINGS = add_named_buildings()


FIXTURES: list[Fixture] = [
    Fixture("dawn_bearer", "statue", (-72, 190), (10, 10), label="Dawn Bearer"),
    Fixture("seraph", "statue", (72, 190), (10, 10), label="Seraph"),
    Fixture("ford_well", "well", (88, 35), (12, 10), label="Ford Well"),
    Fixture("colm_stone", "stone", (52, -258), (3, 3), label="Colm's stone"),
    Fixture("bellstand_platform", "platform", (36, -248), (13, 8), label="Crier's platform"),
    Fixture("tallage_weighbeam", "weighbeam", (-306, 65), (17, 5), label="Weigh-beam"),
    Fixture("tallage_stone", "stone", (-290, 74), (4, 4), label="Tallage stone"),
    Fixture("coswald_tracing", "tracing", (258, 168), (32, 21), label="Tracing floor"),
    Fixture("coswald_crane", "crane", (285, 142), (8, 8), label="Yard crane"),
    # The water network of `lore/wells_and_water.md`. Ford Well above is the
    # ninth and best-known source; a well is a lined shaft into groundwater, a
    # cistern is a roof-fed store, and the Shambles and Seven Lofts keep their
    # own work and fire supplies.
    Fixture("slate_cistern", "cistern", (51, 372), (9, 7), label="Slate Cistern"),
    Fixture("tenter_cistern", "cistern", (94, 223), (10, 7), label="Tenter Cistern"),
    Fixture("lodge_well", "lodge_well", (239, 143), (10, 8), label="Lodge Well"),
    Fixture("three_curb", "three_curb_well", (-131, 166), (10, 10), label="Three-Curb"),
    Fixture("chain_well", "chain_well", (-197, 73), (8, 8), label="Chain Well"),
    Fixture("reed_cistern", "cistern", (-291, -345), (10, 8), label="Reed Cistern"),
    Fixture("step_cistern", "step_cistern", (55, -300), (8, 7), label="Step Cistern"),
    Fixture("bitter_well", "well", (101, -401), (7, 6), label="Bitter Well"),
    Fixture("shambles_well", "well", (-340, 315), (9, 8), label="The Shambles well"),
    Fixture("seven_lofts_tanks", "fire_tanks", (385, 388), (10, 6), label="Seven Lofts fire tanks"),
]


def add_market_fixtures() -> None:
    rng = random.Random(SEED ^ 0xF17E)
    groups = [
        ("wick", -25, 355, 48, 34, 18),
        ("coswald", 255, 155, 55, 38, 12),
        ("tallage", -305, 90, 54, 35, 14),
        ("maren", -305, -365, 48, 36, 18),
        ("gradine", 0, 131, 32, 20, 10),
    ]
    for group, cx, cz, radius_x, radius_z, count in groups:
        for index in range(count):
            angle = rng.random() * math.tau
            radius = math.sqrt(rng.random())
            x = cx + math.cos(angle) * radius_x * radius
            z = cz + math.sin(angle) * radius_z * radius
            kind = "stone_stack" if group == "coswald" and index < 7 else "stall"
            if group == "maren" and index < 5:
                kind = "smoke_rack"
            FIXTURES.append(Fixture(f"{group}_fixture_{index + 1:02d}", kind, (x, z), (5.0, 2.8), rng.uniform(-20, 20)))


add_market_fixtures()


PLACE_MARKS: list[PlaceMark] = []


def mark(name: str, anchor: Point, kind: str = "place") -> None:
    PLACE_MARKS.append(PlaceMark(len(PLACE_MARKS) + 1, name, anchor, kind))


for _name, _anchor, _kind in [
    ("The Lanthorn", (0, -12), "major"),
    ("The Great Rose and eye", (0, 81), "landmark"),
    ("The Gradine", (0, 131), "major"),
    ("Dawn Bearer statue", (-72, 190), "landmark"),
    ("Seraph statue", (72, 190), "landmark"),
    ("Skinners' Court", (108, 122), "place"),
    ("Ford Well", (88, 35), "landmark"),
    ("Chapter house and Grey Press", (70, -69), "building"),
    ("The Wickmarket", (-25, 355), "major"),
    ("Coswald's Yard", (255, 155), "major"),
    ("The Tallage", (-305, 90), "major"),
    ("Maren's Green", (-305, -365), "major"),
    ("The Bellstand", (45, -255), "major"),
    ("The Cut", (-305, -90), "route"),
    ("The Chain Bridge", (-305, 425), "bridge"),
    ("The Tally Bridge", (-305, 105), "bridge"),
    ("The Old Sluice", (-305, -610), "building"),
    ("The Cut ropewalk", (-260, 260), "place"),
    ("The Shambles", (-395, 315), "place"),
    ("Tallage toll-house", (-268, 105), "building"),
    ("Bonded warehouse", (-345, 105), "building"),
    ("Lise Copp's pawnshop", (-262, 43), "building"),
    ("Doctor Ferrant's house", (-242, 20), "building"),
    ("The old Gaunt house", (-218, 18), "building"),
    ("Gaunt Passage", (-228, 25), "route"),
    ("Bonded weighing yard", (-195, -2), "place"),
    ("Saint Maren's church", (-235, -382), "building"),
    ("Saint Maren's churchyard", (-267, -360), "place"),
    ("The charnel door and crypt", (-276, -365), "landmark"),
    ("The Alder Moorings", (-367, -407), "place"),
    ("The Eel Bridge", (-343, -374), "bridge"),
    ("The Hungry Ox", (-330, -447), "building"),
    ("Tanners' Slip", (-390, -345), "route"),
    ("Eelback Alley", (-370, -445), "route"),
    ("Maren's Slip", (-410, -485), "route"),
    ("Empty brine-rotted cellar", (-415, -405), "building"),
    ("The Draper's Reach", (120, 260), "route"),
    ("Tenterhook Lane", (30, 195), "route"),
    ("The Needle", (-162, 230), "route"),
    ("Cinder Row", (-130, 205), "route"),
    ("Burnt Court", (-172, 232), "place"),
    ("The glaziers' guildhall", (-141, 237), "building"),
    ("The masons' lodge", (207, 151), "building"),
    ("Malt Passage", (232, 112), "route"),
    ("Crookneck Lane", (165, -15), "route"),
    ("Osanne Vell's stall", (18, 350), "landmark"),
    ("The Ilvane Chapel", (175, -92), "building"),
    ("The Ilvane anchorhold", (194, -92), "building"),
    ("Bellfoot Passage", (55, -248), "route"),
    ("Bellstand watch-bell tower", (64, -270), "building"),
    ("Colm's stone", (52, -258), "landmark"),
    ("Bellfounders' Yard", (155, -485), "place"),
    ("The Wool Gate", (-35, 510), "gate"),
    ("The Stone Gate", (495, 135), "gate"),
    ("The Harne Gate", (15, -665), "gate"),
    ("The River Gate", (-505, -135), "gate"),
    ("The Reed Postern", (-455, -535), "gate"),
    ("Seven Lofts", (360, 335), "place"),
    ("Outer Serle wharves", (-570, -165), "outside"),
    # The named ward water sources. Ford Well (7) is already indexed above.
    ("Slate Cistern", (51, 372), "landmark"),
    ("Tenter Cistern", (94, 223), "landmark"),
    ("Lodge Well", (239, 143), "landmark"),
    ("Three-Curb", (-131, 166), "landmark"),
    ("Chain Well", (-197, 73), "landmark"),
    ("Reed Cistern", (-291, -345), "landmark"),
    ("Step Cistern", (55, -300), "landmark"),
    ("Bitter Well", (101, -401), "landmark"),
    ("The Shambles well", (-340, 315), "landmark"),
    ("Seven Lofts fire tanks", (385, 388), "landmark"),
]:
    mark(_name, _anchor, _kind)


def point_in_polygon(point: Point, polygon: Sequence[Point]) -> bool:
    x, z = point
    inside = False
    previous = polygon[-1]
    for current in polygon:
        x1, z1 = previous
        x2, z2 = current
        if (z1 > z) != (z2 > z):
            crossing_x = (x2 - x1) * (z - z1) / (z2 - z1) + x1
            if x < crossing_x:
                inside = not inside
        previous = current
    return inside


def orientation(a: Point, b: Point, c: Point) -> float:
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def point_on_segment(point: Point, start: Point, end: Point, epsilon: float = 1e-9) -> bool:
    return (
        min(start[0], end[0]) - epsilon <= point[0] <= max(start[0], end[0]) + epsilon
        and min(start[1], end[1]) - epsilon <= point[1] <= max(start[1], end[1]) + epsilon
    )


def segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool:
    o1 = orientation(a, b, c)
    o2 = orientation(a, b, d)
    o3 = orientation(c, d, a)
    o4 = orientation(c, d, b)
    epsilon = 1e-9
    ab_straddles = (o1 > epsilon and o2 < -epsilon) or (o1 < -epsilon and o2 > epsilon)
    cd_straddles = (o3 > epsilon and o4 < -epsilon) or (o3 < -epsilon and o4 > epsilon)
    if ab_straddles and cd_straddles:
        return True
    return (
        (abs(o1) <= epsilon and point_on_segment(c, a, b, epsilon))
        or (abs(o2) <= epsilon and point_on_segment(d, a, b, epsilon))
        or (abs(o3) <= epsilon and point_on_segment(a, c, d, epsilon))
        or (abs(o4) <= epsilon and point_on_segment(b, c, d, epsilon))
    )


def point_segment_distance(point: Point, a: Point, b: Point) -> float:
    px, pz = point
    ax, az = a
    bx, bz = b
    dx = bx - ax
    dz = bz - az
    length_sq = dx * dx + dz * dz
    if length_sq == 0:
        return math.hypot(px - ax, pz - az)
    t = max(0.0, min(1.0, ((px - ax) * dx + (pz - az) * dz) / length_sq))
    return math.hypot(px - (ax + t * dx), pz - (az + t * dz))


def segment_distance(a: Point, b: Point, c: Point, d: Point) -> float:
    if segments_intersect(a, b, c, d):
        return 0.0
    return min(
        point_segment_distance(a, c, d),
        point_segment_distance(b, c, d),
        point_segment_distance(c, a, b),
        point_segment_distance(d, a, b),
    )


def polygon_edges(poly: Sequence[Point]) -> Iterable[tuple[Point, Point]]:
    for index, start in enumerate(poly):
        yield start, poly[(index + 1) % len(poly)]


def polygons_intersect(a: Sequence[Point], b: Sequence[Point]) -> bool:
    if any(point_in_polygon(point, b) for point in a):
        return True
    if any(point_in_polygon(point, a) for point in b):
        return True
    return any(segments_intersect(a1, a2, b1, b2) for a1, a2 in polygon_edges(a) for b1, b2 in polygon_edges(b))


def polygon_distance(a: Sequence[Point], b: Sequence[Point]) -> float:
    if polygons_intersect(a, b):
        return 0.0
    return min(segment_distance(a1, a2, b1, b2) for a1, a2 in polygon_edges(a) for b1, b2 in polygon_edges(b))


def road_clear(poly: Sequence[Point], road: Road) -> bool:
    clearance = road.width_m / 2.0 + (0.8 if road.tier in {"alley", "passage"} else 1.5)
    for edge_a, edge_b in polygon_edges(poly):
        for start, end in zip(road.points, road.points[1:]):
            if segment_distance(edge_a, edge_b, start, end) < clearance:
                return False
    return True


def min_wall_distance(point: Point) -> float:
    return min(point_segment_distance(point, start, end) for start, end in polygon_edges(WALL))


def district_for(x: float, z: float) -> str:
    if -170 <= x <= 130 and z >= 235:
        return "Wick Ward"
    if x >= 120 and z >= 175:
        return "Cloth Ward"
    if x >= 175 and z >= 35:
        return "Wallwright Ward"
    if -280 <= x <= 30 and 110 <= z <= 320:
        return "Cinder Ward"
    if -440 <= x <= -170 and -40 <= z <= 240:
        return "Weigh Ward"
    if x <= -160 and z <= -235:
        return "Reed Ward"
    if z <= -145:
        return "Bell and Sluice Wards"
    if -120 <= x <= 190 and -170 <= z <= 235:
        return "Fabric Ward"
    return "Outer wards"


DISTRICT_ANGLES = {
    "Wick Ward": -4.0,
    "Cloth Ward": -9.0,
    "Wallwright Ward": 4.0,
    "Cinder Ward": -6.0,
    "Weigh Ward": 2.0,
    "Reed Ward": 5.0,
    "Bell and Sluice Wards": -3.0,
    "Fabric Ward": 1.0,
    "Outer wards": 0.0,
}


USE_WEIGHTS = {
    "Wick Ward": ["residential", "trade", "trade", "workshop", "tavern"],
    "Cloth Ward": ["residential", "trade", "trade", "storage", "storage"],
    "Wallwright Ward": ["residential", "workshop", "workshop", "storage", "trade"],
    "Cinder Ward": ["residential", "workshop", "workshop", "trade", "storage"],
    "Weigh Ward": ["residential", "trade", "trade", "storage", "storage"],
    "Reed Ward": ["residential", "trade", "industrial", "storage", "tavern"],
    "Bell and Sluice Wards": ["residential", "residential", "trade", "workshop", "storage"],
    "Fabric Ward": ["residential", "trade", "lodging", "workshop", "residential"],
    "Outer wards": ["residential", "residential", "workshop", "storage", "trade"],
}


MATERIALS_BY_USE = {
    "residential": ["plaster", "half_timber", "half_timber", "stone_timber"],
    "trade": ["stone_timber", "plaster", "half_timber"],
    "workshop": ["stone_timber", "fieldstone", "half_timber"],
    "industrial": ["fieldstone", "stone_timber"],
    "storage": ["fieldstone", "half_timber"],
    "tavern": ["half_timber", "plaster"],
    "lodging": ["plaster", "half_timber"],
}


def generate_buildings() -> list[Building]:
    buildings = list(NAMED_BUILDINGS)
    occupancy: dict[tuple[int, int], list[Building]] = {}
    road_bounds = [
        (
            min(point[0] for point in road.points) - road.width_m / 2 - 2,
            max(point[0] for point in road.points) + road.width_m / 2 + 2,
            min(point[1] for point in road.points) - road.width_m / 2 - 2,
            max(point[1] for point in road.points) + road.width_m / 2 + 2,
        )
        for road in ROADS
    ]
    site_bounds = [
        (
            min(point[0] for point in site.polygon) - 1.2,
            max(point[0] for point in site.polygon) + 1.2,
            min(point[1] for point in site.polygon) - 1.2,
            max(point[1] for point in site.polygon) + 1.2,
        )
        for site in SITES
    ]

    def bins_for(poly: Sequence[Point]) -> set[tuple[int, int]]:
        min_x = math.floor(min(point[0] for point in poly) / 30)
        max_x = math.floor(max(point[0] for point in poly) / 30)
        min_z = math.floor(min(point[1] for point in poly) / 30)
        max_z = math.floor(max(point[1] for point in poly) / 30)
        return {(x, z) for x in range(min_x, max_x + 1) for z in range(min_z, max_z + 1)}

    for building in NAMED_BUILDINGS:
        for key in bins_for(building.polygon):
            occupancy.setdefault(key, []).append(building)

    def accept_building(building: Building, gap_m: float) -> bool:
        polygon = building.polygon
        centre = (
            sum(point[0] for point in polygon) / len(polygon),
            sum(point[1] for point in polygon) / len(polygon),
        )
        bounds = (
            min(point[0] for point in polygon),
            max(point[0] for point in polygon),
            min(point[1] for point in polygon),
            max(point[1] for point in polygon),
        )

        def overlaps(candidate_bounds: tuple[float, float, float, float]) -> bool:
            return not (
                bounds[1] < candidate_bounds[0]
                or bounds[0] > candidate_bounds[1]
                or bounds[3] < candidate_bounds[2]
                or bounds[2] > candidate_bounds[3]
            )

        if not all(point_in_polygon(point, WALL) for point in polygon):
            return False
        if min_wall_distance(centre) < 15.0:
            return False

        nearby: dict[str, Building] = {}
        for key in bins_for(polygon):
            for other in occupancy.get(key, []):
                nearby[other.id] = other
        if any(polygon_distance(polygon, other.polygon) < gap_m for other in nearby.values()):
            return False
        if any(
            not road_clear(polygon, road)
            for road, road_bound in zip(ROADS, road_bounds)
            if overlaps(road_bound)
        ):
            return False
        if any(
            polygon_distance(polygon, site.polygon) < 1.2
            for site, site_bound in zip(SITES, site_bounds)
            if overlaps(site_bound)
        ):
            return False

        buildings.append(building)
        for key in bins_for(polygon):
            occupancy.setdefault(key, []).append(building)
        return True

    def ordinary_building(
        building_id: str,
        cx: float,
        cz: float,
        size_x: float,
        size_z: float,
        angle: float,
        local: random.Random,
        street_facing: bool,
    ) -> Building:
        district = district_for(cx, cz)
        choices = USE_WEIGHTS[district]
        use = local.choice(choices)
        if street_facing and use == "residential" and local.random() < 0.38:
            use = "trade"
        material = local.choice(MATERIALS_BY_USE[use])
        radial = math.hypot(cx, cz + 50)
        base_levels = 3 if radial < 360 else 2
        if street_facing and radial < 470:
            base_levels += 1 if local.random() < 0.22 else 0
        levels = max(1, min(4, base_levels + local.choice([-1, 0, 0, 0, 1])))
        return Building(
            building_id,
            None,
            rect(cx, cz, size_x, size_z, angle),
            use,
            material,
            levels,
            False,
            district,
        )

    # First establish crooked street walls. Each accepted range has its long
    # frontage aligned to one real road segment rather than to a global grid.
    frontage_number = 1
    frontage_roads = [
        road for road in ROADS if road.tier not in {"service", "wall_lane"}
    ]
    for road_index, road in enumerate(frontage_roads):
        for segment_index, (start, end) in enumerate(zip(road.points, road.points[1:])):
            dx = end[0] - start[0]
            dz = end[1] - start[1]
            length = math.hypot(dx, dz)
            if length < 18:
                continue
            tangent_x = dx / length
            tangent_z = dz / length
            normal_x = -tangent_z
            normal_z = tangent_x
            angle = math.degrees(math.atan2(tangent_z, tangent_x))
            for side_index, side in enumerate((-1.0, 1.0)):
                local = random.Random(
                    SEED
                    ^ ((road_index + 17) * 100_003)
                    ^ ((segment_index + 31) * 7_919)
                    ^ ((side_index + 5) * 65_537)
                )
                cursor = local.uniform(4.0, 9.0)
                while cursor < length - 4:
                    frontage = local.uniform(7.0, 13.5)
                    if cursor + frontage > length - 3:
                        break
                    depth = local.uniform(10.0, 17.5)
                    if road.tier == "cut":
                        depth += local.uniform(1.5, 5.0)
                    elif road.tier in {"alley", "passage"}:
                        depth = local.uniform(7.5, 12.0)
                    along = cursor + frontage / 2
                    setback = 1.0 if road.tier in {"alley", "passage"} else 1.7
                    offset = road.width_m / 2 + depth / 2 + setback
                    cx = start[0] + tangent_x * along + normal_x * offset * side
                    cz = start[1] + tangent_z * along + normal_z * offset * side
                    building = ordinary_building(
                        f"omb_f{frontage_number:04d}",
                        cx,
                        cz,
                        frontage,
                        depth,
                        angle,
                        local,
                        True,
                    )
                    if accept_building(building, 0.45):
                        frontage_number += 1
                    cursor += frontage + local.uniform(0.45, 1.4)

    # Then pack the interiors with irregular back houses, workshops, stables,
    # and storage ranges. Their orientation follows the nearest street, but
    # candidate positions are a deterministic random field, so independent
    # parcels form courts and doglegs instead of another hidden lattice.
    infill_rng = random.Random(SEED ^ 0x1AF111)
    orientation_cache: dict[tuple[int, int], float] = {}
    infill_number = 1
    for _attempt in range(30_000):
        if infill_number > 1_750:
            break
        cx = infill_rng.uniform(-475, 475)
        cz = infill_rng.uniform(-645, 470)
        if not point_in_polygon((cx, cz), WALL):
            continue

        orientation_key = (math.floor(cx / 45), math.floor(cz / 45))
        if orientation_key not in orientation_cache:
            sample = (orientation_key[0] * 45 + 22.5, orientation_key[1] * 45 + 22.5)
            nearest_angle = DISTRICT_ANGLES[district_for(*sample)]
            nearest_distance = math.inf
            for road in ROADS:
                if road.tier == "wall_lane":
                    continue
                for start, end in zip(road.points, road.points[1:]):
                    distance = point_segment_distance(sample, start, end)
                    if distance < nearest_distance:
                        nearest_distance = distance
                        nearest_angle = math.degrees(
                            math.atan2(end[1] - start[1], end[0] - start[0])
                        )
            orientation_cache[orientation_key] = nearest_angle
        nearest_angle = orientation_cache[orientation_key]

        size_x = infill_rng.uniform(7.0, 13.0)
        size_z = infill_rng.uniform(9.0, 17.0)
        district = district_for(cx, cz)
        if district in {"Weigh Ward", "Cloth Ward", "Reed Ward"} and infill_rng.random() < 0.25:
            size_x += infill_rng.uniform(1.0, 4.0)
            size_z += infill_rng.uniform(2.0, 6.0)
        angle = nearest_angle + infill_rng.uniform(-9.0, 9.0)
        building = ordinary_building(
            f"omb_i{infill_number:04d}",
            cx,
            cz,
            size_x,
            size_z,
            angle,
            infill_rng,
            False,
        )
        if accept_building(building, 0.8):
            infill_number += 1

    return buildings


BUILDINGS = generate_buildings()


INTENTIONAL_BUILDING_OVERLAPS = {
    frozenset(("named_toll_house", "named_tally_bridge")),
    frozenset(("named_bonded_warehouse", "named_tally_bridge")),
    frozenset(("named_moorings_3", "named_eel_bridge")),
    frozenset(("named_ilvane_chapel", "named_anchorhold")),
}


def validate_plan() -> None:
    """Reject unstable IDs and accidental building-footprint collisions."""
    collections = {
        "road": [road.id for road in ROADS],
        "site": [site.id for site in SITES],
        "fixture": [fixture.id for fixture in FIXTURES],
        "building": [building.id for building in BUILDINGS],
    }
    for kind, ids in collections.items():
        if len(ids) != len(set(ids)):
            raise ValueError(f"Duplicate {kind} ID in city plan")
    if [place.number for place in PLACE_MARKS] != list(range(1, len(PLACE_MARKS) + 1)):
        raise ValueError("Named-place index numbers are not contiguous")
    if len({place.name for place in PLACE_MARKS}) != len(PLACE_MARKS):
        raise ValueError("Named-place index contains a duplicate display name")

    for building in BUILDINGS:
        if not building.named and not all(point_in_polygon(point, WALL) for point in building.polygon):
            raise ValueError(f"Urban-fabric building {building.id} crosses the city wall")

    bins: dict[tuple[int, int], list[Building]] = {}
    checked: set[frozenset[str]] = set()
    for building in BUILDINGS:
        min_x = math.floor(min(point[0] for point in building.polygon) / 40)
        max_x = math.floor(max(point[0] for point in building.polygon) / 40)
        min_z = math.floor(min(point[1] for point in building.polygon) / 40)
        max_z = math.floor(max(point[1] for point in building.polygon) / 40)
        keys = {
            (x, z)
            for x in range(min_x, max_x + 1)
            for z in range(min_z, max_z + 1)
        }
        for key in keys:
            for other in bins.get(key, []):
                pair = frozenset((building.id, other.id))
                if pair in checked:
                    continue
                checked.add(pair)
                if polygons_intersect(building.polygon, other.polygon) and pair not in INTENTIONAL_BUILDING_OVERLAPS:
                    raise ValueError(
                        f"Accidental building overlap: {building.id} and {other.id}"
                    )
        for key in keys:
            bins.setdefault(key, []).append(building)


def screen(point: Point) -> Point:
    """Map world (x,z) to SVG (east-right, north-up)."""
    x, z = point
    return (-z, -x)


def svg_points(poly: Sequence[Point]) -> str:
    return " ".join(f"{sx:.2f},{sy:.2f}" for sx, sy in map(screen, poly))


def svg_path(points: Sequence[Point]) -> str:
    transformed = [screen(point) for point in points]
    return "M " + " L ".join(f"{x:.2f} {y:.2f}" for x, y in transformed)


def polygon_centroid(poly: Sequence[Point]) -> Point:
    return (
        sum(point[0] for point in poly) / len(poly),
        sum(point[1] for point in poly) / len(poly),
    )


def ridge_for(poly: Sequence[Point]) -> tuple[Point, Point]:
    # For rectangles, join the midpoints of the shorter pair of opposing edges.
    if len(poly) != 4:
        centre = polygon_centroid(poly)
        return (centre[0], min(p[1] for p in poly)), (centre[0], max(p[1] for p in poly))
    lengths = [math.dist(poly[i], poly[(i + 1) % 4]) for i in range(4)]
    if lengths[0] < lengths[1]:
        return (
            ((poly[0][0] + poly[1][0]) / 2, (poly[0][1] + poly[1][1]) / 2),
            ((poly[2][0] + poly[3][0]) / 2, (poly[2][1] + poly[3][1]) / 2),
        )
    return (
        ((poly[1][0] + poly[2][0]) / 2, (poly[1][1] + poly[2][1]) / 2),
        ((poly[3][0] + poly[0][0]) / 2, (poly[3][1] + poly[0][1]) / 2),
    )


def svg_header() -> str:
    return """<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
  width="9000" height="7200" viewBox="-690 -610 1810 1440" role="img"
  aria-labelledby="map-title map-desc">
<title id="map-title">Authoritative top-down building plan of Ombreval, F.437</title>
<desc id="map-desc">A detailed cadastral city map showing every planned building footprint, streets, walls, gates, the dry Cut, the Serle outside the south wall, and numbered locations for all named places.</desc>
<defs>
  <style><![CDATA[
    .map-bg { fill: #efe4c9; }
    .outer-ground { fill: #d7c797; stroke: #8d7c55; stroke-width: 1; }
    .contour { fill: none; stroke: #ae9c72; stroke-width: 0.7; opacity: 0.36; stroke-dasharray: 5 7; }
    .river { fill: #8eb0af; stroke: #587c7d; stroke-width: 2.2; }
    .river-current { fill: none; stroke: #d5e0d9; stroke-width: 1; opacity: 0.75; }
    .wharf { fill: #9b8060; stroke: #493e32; stroke-width: 0.8; }
    .city-ground { fill: #d8ca9f; stroke: none; }
    .site-square { fill: #d8c19b; stroke: #8e7653; stroke-width: 0.8; }
    .site-court, .site-yard { fill: #cbb98f; stroke: #806f50; stroke-width: 0.7; }
    .site-churchyard { fill: #b9bd91; stroke: #6e7350; stroke-width: 0.8; }
    .site-parish_reserve { fill: #cad0a1; stroke: #6e7350; stroke-width: 1; stroke-dasharray: 4 3; }
    .site-monument, .site-precinct { fill: #d5c49d; stroke: #94815d; stroke-width: 0.7; }
    .road-under { fill: none; stroke: #7d6849; stroke-linecap: round; stroke-linejoin: round; opacity: 0.45; }
    .road { fill: none; stroke: #c8ad81; stroke-linecap: round; stroke-linejoin: round; }
    .road-cut { stroke: #b99e72; }
    .road-service, .road-wall_lane { stroke: #bda57d; }
    .road-alley, .road-passage { stroke: #baa077; }
    .building { stroke: #4a4135; stroke-width: 0.72; stroke-linejoin: round; }
    .material-plaster { fill: #d6b98f; }
    .material-half_timber { fill: #b88760; }
    .material-stone_timber { fill: #b7a17f; }
    .material-fieldstone { fill: #8f8a78; }
    .material-limestone { fill: #c9c2a4; }
    .roof-ridge { stroke: #5e5140; stroke-width: 0.42; opacity: 0.65; }
    .named-building { stroke: #211f1b; stroke-width: 1.25; }
    .use-ecclesiastical { fill: #aeb39b; }
    .use-civic, .use-fortification { fill: #aaa891; }
    .use-guild { fill: #b49d7c; }
    .use-storage { fill: #9e9176; }
    .use-industrial { fill: #9a7e69; }
    .use-bridge { fill: #766a57; }
    .wall-casing { fill: none; stroke: #3d382e; stroke-width: 13; stroke-linejoin: round; }
    .wall { fill: none; stroke: #9f967c; stroke-width: 8; stroke-linejoin: round; }
    .wall-walk { fill: none; stroke: #443d32; stroke-width: 1.1; stroke-dasharray: 5 5; }
    .wall-tower { fill: #928b76; stroke: #302c25; stroke-width: 1.2; }
    .fixture { stroke: #3d372e; stroke-width: 0.6; }
    .fixture-stall { fill: #c27855; }
    .fixture-stone_stack { fill: #a7a38f; }
    .fixture-smoke_rack { fill: #826b54; }
    .fixture-platform, .fixture-tracing { fill: #c0ab82; }
    .fixture-well, .fixture-chain_well, .fixture-three_curb_well, .fixture-lodge_well { fill: #7e9fa0; }
    .fixture-cistern, .fixture-step_cistern, .fixture-fire_tanks { fill: #94b0ac; }
    .fixture-statue { fill: #5c766f; }
    .fixture-crane, .fixture-weighbeam { fill: #76583f; }
    .fixture-stone { fill: #aaa48d; }
    .direct-major { font-family: Georgia, serif; font-size: 15px; font-weight: bold; letter-spacing: 1.4px; fill: #29251e; text-anchor: middle; paint-order: stroke; stroke: #efe4c9; stroke-width: 0.9px; stroke-linejoin: round; }
    .direct-minor { font-family: Georgia, serif; font-size: 8px; font-weight: bold; letter-spacing: 0.8px; fill: #393126; text-anchor: middle; paint-order: stroke; stroke: #efe4c9; stroke-width: 0.6px; }
    .road-label { font-family: Georgia, serif; font-size: 7.5px; font-weight: bold; letter-spacing: 1.1px; fill: #554630; paint-order: stroke; stroke: #e8dbbd; stroke-width: 0.6px; }
    .place-marker { fill: #302a22; stroke: #f4ecd7; stroke-width: 1.3; }
    .place-number { fill: #fffaf0; font-family: Arial, sans-serif; font-size: 5.4px; font-weight: bold; text-anchor: middle; dominant-baseline: central; }
    .panel { fill: #f4ecd7; stroke: #574b39; stroke-width: 1.2; }
    .panel-title { font-family: Georgia, serif; font-size: 12px; font-weight: bold; fill: #30291f; letter-spacing: 1px; }
    .panel-text { font-family: Arial, sans-serif; font-size: 6.7px; fill: #30291f; }
    .panel-small { font-family: Arial, sans-serif; font-size: 5.8px; fill: #514636; }
    .map-title { font-family: Georgia, serif; font-size: 25px; font-weight: bold; fill: #2f291f; letter-spacing: 2.5px; }
    .map-subtitle { font-family: Arial, sans-serif; font-size: 8.5px; fill: #5e503c; letter-spacing: 0.6px; }
    .ward-label { font-family: Georgia, serif; font-size: 13px; font-style: italic; fill: #6b5a43; opacity: 0.24; text-anchor: middle; letter-spacing: 2px; }
  ]]></style>
</defs>
"""


def inline_svg_class_styles(source: str) -> str:
    """Inline the map stylesheet for strict and CSS-light SVG renderers.

    The class names remain in the file for editing and layer inspection.  The
    duplicated inline declarations make the plan portable to command-line SVG
    renderers, document converters, and game tooling that ignore embedded CSS.
    """
    style_match = re.search(r"<style><!\[CDATA\[(.*?)\]\]></style>", source, re.DOTALL)
    if style_match is None:
        raise ValueError("SVG stylesheet block is missing")

    declarations_by_class: dict[str, str] = {}
    for selector_group, declarations in re.findall(
        r"([^{}]+)\{([^{}]+)\}", style_match.group(1)
    ):
        compact = " ".join(declarations.split())
        for selector in selector_group.split(","):
            selector = selector.strip()
            if re.fullmatch(r"\.[A-Za-z0-9_-]+", selector):
                declarations_by_class[selector[1:]] = compact

    def replace_class(match: re.Match[str]) -> str:
        class_names = match.group(1).split()
        declarations = [
            declarations_by_class[class_name]
            for class_name in class_names
            if class_name in declarations_by_class
        ]
        if not declarations:
            return match.group(0)
        return f'class="{match.group(1)}" style="{" ".join(declarations)}"'

    return re.sub(r'class="([^"]+)"', replace_class, source)


def render_svg() -> None:
    parts: list[str] = [svg_header()]
    parts.append(
        f"<!-- Generated by {GENERATED_BY} (seed 0x{SEED:X}). "
        "Do not edit by hand; re-run the script to regenerate. -->"
    )
    parts.append('<rect class="map-bg" x="-690" y="-610" width="1810" height="1440"/>')
    parts.append('<g id="map-art">')

    # Terrain and river context.
    parts.append('<rect class="outer-ground" x="-650" y="-545" width="1370" height="1250"/>')
    city_points = svg_points(WALL)
    parts.append(f'<polygon id="city-ground" class="city-ground" points="{city_points}"/>')
    for y in [-420, -260, -100, 80, 245, 410]:
        parts.append(f'<path class="contour" d="M -560 {y} C -250 {y - 22}, 100 {y + 18}, 690 {y - 8}"/>')

    river_poly: Polygon = [(-575, 640), (-575, -735), (-690, -735), (-690, 640)]
    parts.append(f'<polygon class="river" points="{svg_points(river_poly)}"/>')
    for x in [-605, -635, -666]:
        start = screen((x, 625))
        end = screen((x, -720))
        parts.append(f'<path class="river-current" d="M {start[0]} {start[1]} C -150 {start[1] - 8}, 250 {end[1] + 9}, {end[0]} {end[1]}"/>')

    # Outer wharf aprons and individual sheds.
    for index, z in enumerate(range(95, -451, -38), 1):
        wharf = rect(-568, float(z), 24, 27, 0)
        parts.append(f'<polygon id="wharf-shed-{index:02d}" class="wharf" points="{svg_points(wharf)}"><title>Outer wharf shed {index}</title></polygon>')
        quay_a = screen((-590, z + 14))
        quay_b = screen((-590, z - 14))
        parts.append(f'<path class="wharf" d="M {quay_a[0]} {quay_a[1]} L {quay_b[0]} {quay_b[1]}" stroke-width="4"/>')

    # Site grounds beneath roads and buildings.
    parts.append('<g id="sites">')
    for site in SITES:
        parts.append(f'<polygon id="site-{site.id}" class="site-{site.kind}" points="{svg_points(site.polygon)}"><title>{escape(site.name)}</title></polygon>')
    parts.append('</g>')

    # Roads, with casing to preserve clear boundaries at full-map scale.
    parts.append('<g id="roads">')
    for road in ROADS:
        path = svg_path(road.points)
        parts.append(f'<path class="road-under" d="{path}" stroke-width="{road.width_m + 2.2:.1f}"/>')
        parts.append(f'<path id="road-{road.id}" class="road road-{road.tier}" d="{path}" stroke-width="{road.width_m:.1f}"><title>{escape(road.name or "Unnamed street")}</title></path>')
    parts.append('</g>')

    # All building footprints, named and unnamed, with stable IDs and metadata.
    parts.append('<g id="buildings">')
    for building in BUILDINGS:
        classes = ["building", f"material-{building.material}", f"use-{building.use}"]
        if building.named:
            classes.append("named-building")
        data_name = escape(building.name or "Unnamed building", quote=True)
        parts.append(
            f'<polygon id="{building.id}" class="{" ".join(classes)}" '
            f'data-name="{data_name}" data-use="{building.use}" data-levels="{building.levels}" '
            f'data-district="{escape(building.district, quote=True)}" points="{svg_points(building.polygon)}">'
            f'<title>{data_name} — {building.use}, {building.levels} storey</title></polygon>'
        )
        ridge_a, ridge_b = ridge_for(building.polygon)
        ra = screen(ridge_a)
        rb = screen(ridge_b)
        parts.append(f'<line class="roof-ridge" x1="{ra[0]:.2f}" y1="{ra[1]:.2f}" x2="{rb[0]:.2f}" y2="{rb[1]:.2f}"/>')
    parts.append('</g>')

    # Wall and towers sit over the urban fabric.
    closed_wall = WALL + [WALL[0]]
    wall_path = svg_path(closed_wall)
    parts.append('<g id="fortifications">')
    parts.append(f'<path class="wall-casing" d="{wall_path}"/>')
    parts.append(f'<path class="wall" d="{wall_path}"/>')
    parts.append(f'<path class="wall-walk" d="{wall_path}"/>')
    tower_points: list[Point] = list(WALL)
    for start, end in polygon_edges(WALL):
        length = math.dist(start, end)
        for step in range(1, int(length // 115) + 1):
            t = step / (int(length // 115) + 1)
            tower_points.append((start[0] + (end[0] - start[0]) * t, start[1] + (end[1] - start[1]) * t))
    for index, point in enumerate(tower_points, 1):
        sx, sy = screen(point)
        parts.append(f'<rect id="wall-tower-{index:02d}" class="wall-tower" x="{sx - 6:.2f}" y="{sy - 6:.2f}" width="12" height="12" transform="rotate(45 {sx:.2f} {sy:.2f})"><title>Wall tower {index}</title></rect>')
    parts.append('</g>')

    # Market and civic fixtures.
    parts.append('<g id="fixtures">')
    for fixture in FIXTURES:
        sx, sy = screen(fixture.position)
        width, height = fixture.size[1], fixture.size[0]
        title = escape(fixture.label or fixture.kind)
        if fixture.kind in {"well", "chain_well", "three_curb_well", "lodge_well", "statue", "stone", "crane"}:
            radius = max(width, height) / 2
            parts.append(f'<circle id="fixture-{fixture.id}" class="fixture fixture-{fixture.kind}" cx="{sx:.2f}" cy="{sy:.2f}" r="{radius:.2f}"><title>{title}</title></circle>')
        else:
            parts.append(f'<rect id="fixture-{fixture.id}" class="fixture fixture-{fixture.kind}" x="{sx - width / 2:.2f}" y="{sy - height / 2:.2f}" width="{width:.2f}" height="{height:.2f}" transform="rotate({-fixture.angle_deg:.2f} {sx:.2f} {sy:.2f})"><title>{title}</title></rect>')
    parts.append('</g>')

    # Ward names sit beneath specific place labels.
    ward_labels = [
        ("WICK WARD", (-45, 420)), ("CLOTH WARD", (170, 320)), ("WALLWRIGHT WARD", (345, 120)),
        ("FABRIC WARD", (20, 15)), ("CINDER WARD", (-185, 220)), ("WEIGH WARD", (-260, 5)),
        ("REED WARD", (-365, -300)), ("BELL WARD", (205, -285)), ("SLUICE WARD", (-120, -540)),
    ]
    for label, point in ward_labels:
        sx, sy = screen(point)
        parts.append(f'<text class="ward-label" x="{sx}" y="{sy}">{label}</text>')

    # Direct labels for the principal anchors.
    direct_labels = [
        ("THE LANTHORN", (0, -12), "major"), ("THE GRADINE", (0, 132), "minor"),
        ("THE WICKMARKET", (-25, 355), "major"), ("COSWALD'S YARD", (255, 155), "major"),
        ("THE TALLAGE", (-305, 90), "major"), ("MAREN'S GREEN", (-305, -365), "major"),
        ("THE BELLSTAND", (45, -255), "major"), ("SEVEN LOFTS", (360, 335), "minor"),
        ("THE SHAMBLES", (-395, 315), "minor"), ("BELLFOUNDERS' YARD", (155, -485), "minor"),
        ("OLD SLUICE", (-305, -610), "minor"), ("SAINT MAREN'S", (-225, -390), "minor"),
        ("ALDER MOORINGS", (-380, -418), "minor"), ("ILVANE CHAPEL", (175, -92), "minor"),
        ("WOOL GATE", (-35, 530), "minor"), ("STONE GATE", (515, 135), "minor"),
        ("HARNE GATE", (15, -687), "minor"), ("RIVER GATE", (-527, -135), "minor"),
        ("REED POSTERN", (-474, -535), "minor"),
    ]
    parts.append('<g id="direct-labels">')
    for text_value, point, weight in direct_labels:
        sx, sy = screen(point)
        parts.append(f'<text class="direct-{weight}" x="{sx:.2f}" y="{sy:.2f}">{escape(text_value)}</text>')
    parts.append('</g>')

    # Numbered markers connect the exhaustive name index to exact anchors.
    parts.append('<g id="place-markers">')
    for place in PLACE_MARKS:
        sx, sy = screen(place.anchor)
        parts.append(f'<circle class="place-marker" cx="{sx:.2f}" cy="{sy:.2f}" r="5.8"><title>{place.number}. {escape(place.name)}</title></circle>')
        parts.append(f'<text class="place-number" x="{sx:.2f}" y="{sy:.2f}">{place.number}</text>')
    parts.append('</g>')
    parts.append('</g>')  # map-art

    # Title, orientation, scale, legends, and complete numbered index.
    parts.append('<g id="map-information">')
    parts.append('<text class="map-title" x="-640" y="-566">OMBREVAL</text>')
    parts.append('<text class="map-subtitle" x="-638" y="-548">AUTHORITATIVE TOP-DOWN BUILDING PLAN · F.437 · NORTH +X · EAST −Z</text>')

    # Compass and scale bar.
    parts.append('<g transform="translate(-610,-505)"><path d="M 0 35 L 0 -20 L -7 -6 L 0 -28 L 7 -6 L 0 -20" fill="#3b3328"/><text class="panel-text" x="0" y="-34" text-anchor="middle">N +X</text><text class="panel-small" x="0" y="48" text-anchor="middle">south −X</text><text class="panel-small" x="-30" y="8" text-anchor="middle">W +Z</text><text class="panel-small" x="31" y="8" text-anchor="middle">E −Z</text></g>')
    parts.append('<g transform="translate(-620,690)"><path d="M 0 0 L 200 0" stroke="#30291f" stroke-width="3"/><path d="M 0 -5 L 0 5 M 50 -5 L 50 5 M 100 -5 L 100 5 M 150 -5 L 150 5 M 200 -5 L 200 5" stroke="#30291f" stroke-width="2"/><text class="panel-text" x="0" y="16">0</text><text class="panel-text" x="100" y="16" text-anchor="middle">100 m</text><text class="panel-text" x="200" y="16" text-anchor="end">200 m</text></g>')

    # Building legend.
    parts.append('<rect class="panel" x="735" y="-535" width="350" height="150" rx="5"/>')
    parts.append('<text class="panel-title" x="750" y="-512">MAP KEY</text>')
    legend_items = [
        ("plaster", "Plaster / residential fabric"), ("half_timber", "Half-timber fabric"),
        ("fieldstone", "Fieldstone / storage / industry"), ("limestone", "Civic, church, fortification"),
    ]
    for index, (material, label) in enumerate(legend_items):
        y = -490 + index * 20
        parts.append(f'<rect class="building material-{material}" x="752" y="{y - 8}" width="18" height="12"/>')
        parts.append(f'<text class="panel-text" x="778" y="{y + 1}">{escape(label)}</text>')
    parts.append('<circle class="place-marker" cx="760" cy="-405" r="5.8"/><text class="place-number" x="760" y="-405">#</text><text class="panel-text" x="778" y="-402">Numbered named-place anchor</text>')
    parts.append('<text class="panel-small" x="750" y="-388">Every footprint has an SVG/JSON ID, use, material, and storey count.</text>')

    # Exhaustive place index in two columns.
    panel_y = -370
    panel_height = 965
    parts.append(f'<rect class="panel" x="735" y="{panel_y}" width="350" height="{panel_height}" rx="5"/>')
    parts.append('<text class="panel-title" x="750" y="-345">NAMED PLACE INDEX</text>')
    columns = [PLACE_MARKS[:30], PLACE_MARKS[30:]]
    for column_index, places in enumerate(columns):
        x = 750 + column_index * 170
        for row, place in enumerate(places):
            y = -323 + row * 26.5
            parts.append(f'<text class="panel-text" x="{x}" y="{y}"><tspan font-weight="bold">{place.number:02d}</tspan>  {escape(place.name)}</text>')
            parts.append(f'<text class="panel-small" x="{x + 17}" y="{y + 9}">({place.anchor[0]:g}, {place.anchor[1]:g}) · {escape(place.kind)}</text>')

    unnamed_count = sum(not building.named for building in BUILDINGS)
    named_count = sum(building.named for building in BUILDINGS)
    parts.append('<rect class="panel" x="735" y="610" width="350" height="105" rx="5"/>')
    parts.append('<text class="panel-title" x="750" y="636">PLAN INVENTORY</text>')
    parts.append(f'<text class="panel-text" x="750" y="657">{len(BUILDINGS):,} total building footprints</text>')
    parts.append(f'<text class="panel-text" x="750" y="674">{unnamed_count:,} individually placed urban-fabric buildings</text>')
    parts.append(f'<text class="panel-text" x="750" y="691">{named_count:,} named, reserved, bridge, gate, and complex buildings</text>')
    parts.append(f'<text class="panel-small" x="750" y="707">Generated by {escape(GENERATED_BY)} · seed 0x{SEED:X} · 1 SVG unit = 1 m</text>')
    parts.append('</g>')
    parts.append('</svg>')

    svg = "\n".join(parts) + "\n"
    SVG_PATH.write_text(inline_svg_class_styles(svg), encoding="utf-8")


def write_json() -> None:
    payload = {
        "schema_version": 1,
        "title": "Authoritative top-down building plan of Ombreval, F.437",
        "generated_by": GENERATED_BY,
        "seed": SEED,
        "coordinate_system": {
            "units": "meters",
            "north": "+x",
            "east": "-z",
            "up": "+y",
        },
        "wall_polygon_xz": WALL,
        "roads": [asdict(road) for road in ROADS],
        "sites": [asdict(site) for site in SITES],
        "fixtures": [asdict(fixture) for fixture in FIXTURES],
        "named_place_index": [asdict(place) for place in PLACE_MARKS],
        "buildings": [asdict(building) for building in BUILDINGS],
        "statistics": {
            "total_buildings": len(BUILDINGS),
            "named_or_reserved_buildings": sum(building.named for building in BUILDINGS),
            "unnamed_urban_fabric_buildings": sum(not building.named for building in BUILDINGS),
            "roads": len(ROADS),
            "named_places": len(PLACE_MARKS),
            "fixtures": len(FIXTURES),
        },
    }
    JSON_PATH.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def write_html() -> None:
    HTML_PATH.write_text(
        """<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Ombreval — authoritative top-down building plan</title>
  <style>
    :root { color-scheme: dark; }
    * { box-sizing: border-box; }
    html, body { margin: 0; height: 100%; background: #29251f; overflow: hidden; }
    #viewport { position: fixed; inset: 0; overflow: hidden; touch-action: none; }
    #map {
      width: 100vw; height: 100vh; object-fit: contain; display: block;
      transform-origin: 0 0; cursor: grab; user-select: none; will-change: transform;
    }
    #map.dragging { cursor: grabbing; }
    #controls {
      position: fixed; left: 16px; bottom: 16px; display: flex; align-items: center;
      gap: 7px; padding: 8px; border: 1px solid #82755f; border-radius: 8px;
      background: rgb(34 31 26 / 88%); box-shadow: 0 3px 16px rgb(0 0 0 / 35%);
      font: 13px/1.2 system-ui, sans-serif; color: #efe4c9;
    }
    button, a {
      min-width: 34px; height: 32px; padding: 0 10px; border: 1px solid #93846b;
      border-radius: 5px; background: #4b4337; color: #fff6df; font: inherit;
      text-decoration: none; display: inline-grid; place-items: center; cursor: pointer;
    }
    button:hover, a:hover { background: #625746; }
    #zoom-readout { min-width: 46px; text-align: center; font-variant-numeric: tabular-nums; }
    #help { opacity: .78; margin-left: 3px; }
    @media (max-width: 720px) { #help { display: none; } }
  </style>
</head>
<body>
  <div id="viewport">
    <img id="map" draggable="false" src="ombreval_top_down_map.svg" alt="Authoritative top-down building plan of Ombreval, F.437">
  </div>
  <div id="controls" aria-label="Map controls">
    <button id="zoom-out" type="button" aria-label="Zoom out">−</button>
    <span id="zoom-readout">100%</span>
    <button id="zoom-in" type="button" aria-label="Zoom in">+</button>
    <button id="reset" type="button">Fit</button>
    <a href="ombreval_top_down_map.svg" target="_blank" title="Open the raw vector map">SVG</a>
    <span id="help">wheel/double-click to zoom · drag to pan · 0 to fit</span>
  </div>
  <script>
    const viewport = document.querySelector('#viewport');
    const map = document.querySelector('#map');
    const readout = document.querySelector('#zoom-readout');
    let scale = 1;
    let offsetX = 0;
    let offsetY = 0;
    let dragging = false;
    let previousX = 0;
    let previousY = 0;

    function render() {
      map.style.transform = `translate(${offsetX}px, ${offsetY}px) scale(${scale})`;
      readout.textContent = `${Math.round(scale * 100)}%`;
    }

    function zoomAt(clientX, clientY, factor) {
      const next = Math.min(24, Math.max(0.5, scale * factor));
      const localX = (clientX - offsetX) / scale;
      const localY = (clientY - offsetY) / scale;
      offsetX = clientX - localX * next;
      offsetY = clientY - localY * next;
      scale = next;
      render();
    }

    function reset() {
      scale = 1;
      offsetX = 0;
      offsetY = 0;
      render();
    }

    viewport.addEventListener('wheel', event => {
      event.preventDefault();
      zoomAt(event.clientX, event.clientY, Math.exp(-event.deltaY * 0.0015));
    }, { passive: false });
    viewport.addEventListener('dblclick', event => zoomAt(event.clientX, event.clientY, 2));
    viewport.addEventListener('pointerdown', event => {
      dragging = true;
      previousX = event.clientX;
      previousY = event.clientY;
      map.classList.add('dragging');
      viewport.setPointerCapture(event.pointerId);
    });
    viewport.addEventListener('pointermove', event => {
      if (!dragging) return;
      offsetX += event.clientX - previousX;
      offsetY += event.clientY - previousY;
      previousX = event.clientX;
      previousY = event.clientY;
      render();
    });
    viewport.addEventListener('pointerup', event => {
      dragging = false;
      map.classList.remove('dragging');
      viewport.releasePointerCapture(event.pointerId);
    });
    document.querySelector('#zoom-in').addEventListener('click', () => zoomAt(innerWidth / 2, innerHeight / 2, 1.5));
    document.querySelector('#zoom-out').addEventListener('click', () => zoomAt(innerWidth / 2, innerHeight / 2, 1 / 1.5));
    document.querySelector('#reset').addEventListener('click', reset);
    addEventListener('keydown', event => {
      if (event.key === '0') reset();
      if (event.key === '+' || event.key === '=') zoomAt(innerWidth / 2, innerHeight / 2, 1.5);
      if (event.key === '-') zoomAt(innerWidth / 2, innerHeight / 2, 1 / 1.5);
    });
    render();
  </script>
</body>
</html>
""",
        encoding="utf-8",
    )


def main() -> None:
    validate_plan()
    render_svg()
    write_json()
    write_html()
    print(
        f"Wrote {SVG_PATH.name}, {JSON_PATH.name}, and {HTML_PATH.name}: "
        f"{len(BUILDINGS)} buildings, {len(ROADS)} roads, {len(PLACE_MARKS)} named places."
    )


if __name__ == "__main__":
    main()
