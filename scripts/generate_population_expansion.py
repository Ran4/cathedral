#!/usr/bin/env python3
"""Build the authored 500-person cast from the preserved original 103.

Run with:
    uv run --no-project python scripts/generate_population_expansion.py

The output is deterministic. Assertions encode the population skeleton from
features/implemented/going_up_to_500_named_characters.md so a later regeneration cannot
quietly turn the cast back into a catalogue of masters.
"""

from __future__ import annotations

import json
import math
import random
import re
from bisect import bisect_left, bisect_right
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHARACTERS = ROOT / "lore/characters"
OCCUPATIONS_PATH = ROOT / "lore/core_lore/occupations.json"
CITY_PLAN_PATH = ROOT / "lore/places/ombreval_buildings.json"

SPAWN_GRID_M = 10.0
SPAWN_CLEARANCE_M = 1.5
NEARBY_RADIUS_M = 20.0
MAX_NEARBY_NPCS = 3
REGION_SIZE_M = 100.0
MAX_NPCS_PER_REGION = 10

MAJOR_IDS = {
    # The twenty canonical dramatis personae.
    "ak3vd", "a9prs", "b4hst", "cj9sp", "dv8ll", "fg2sh", "cf2rr",
    "fl5cp", "fc9rn", "amt4p", "hj6br", "em3rl", "he3nd", "aq7ld",
    "ax5nf", "gw4ld", "az2sm", "gr8tp", "et7rd", "cg6ud",
    # Opening cast and recurring quest/rumour anchors.
    "sv3n1", "cb947", "k0fb1", "ft3tb", "e3cob", "e6ptr", "bm3ot",
    "e9nan", "dtib0", "ar5tl",
}

RESERVED_CANON_NAMES = {
    # Naming-guide examples are canonical lesser lines even when they do not
    # yet have a character sheet; generated ambient people must not claim them.
    "Sibbe Crake",
}

EXISTING_ADDITIONS = {
    "general_labourer": 20, "cargo_worker": 13, "market_seller": 10,
    "scavenger": 5, "tavern_worker": 7, "cook": 6, "baker": 6,
    "brewer": 3, "butcher": 3, "miller": 2, "farmer": 6,
    "carpenter_and_builder": 7, "mason": 5, "cloth_worker": 7,
    "chandler": 4, "smith": 3, "leather_worker": 3, "laundress": 5,
    "roper": 2, "boatworker": 6, "fish_trader": 4, "salt_worker": 3,
    "merchant": 2, "pilgrim": 7, "watchman_and_keeper": 6,
    "court_officer": 2, "revenue_worker": 2, "scribe_and_clerk": 3,
    "healer": 3, "church_attendant": 3, "candor_cleric": 2,
    "funerary_worker": 1, "bell_ringer": 1, "glazier": 2, "draper": 1,
    "lamplighter": 2, "messenger": 3, "guide": 2, "money_dealer": 1,
    "scholar": 1, "painter": 1, "instrument_maker": 1,
    "bellfounder": 1, "freight_broker": 1, "salt_trader": 1,
}

NEW_FAMILY_ADDITIONS = {
    "domestic_servant": 45, "garment_worker": 14, "shoemaker": 12,
    "cooper": 10, "potter": 8, "cartwright_and_wheelwright": 6,
    "fine_metalworker": 8, "animal_worker": 14, "sanitation_worker": 12,
    "water_and_bath_worker": 8, "food_provisioner": 18,
    "grocer_and_spicer": 5, "entertainer": 10, "civic_officer": 7,
    "bailiff_and_gaoler": 8, "militia_and_soldier": 15, "sex_worker": 8,
}

# The first addition in each of these families is a stable supporting anchor.
MINOR_EXISTING_FAMILIES = set(list(EXISTING_ADDITIONS)[:30])
MINOR_NEW_FAMILIES = set(NEW_FAMILY_ADDITIONS)

FEMALE_COUNTS = {
    # Expansion of existing families: 96 women, 83 men.
    "general_labourer": 10, "cargo_worker": 5, "market_seller": 8,
    "scavenger": 3, "tavern_worker": 5, "cook": 4, "baker": 4,
    "brewer": 1, "butcher": 1, "miller": 1, "farmer": 3,
    "carpenter_and_builder": 3, "mason": 2, "cloth_worker": 6,
    "chandler": 2, "smith": 1, "leather_worker": 1, "laundress": 5,
    "roper": 1, "boatworker": 3, "fish_trader": 3, "salt_worker": 1,
    "merchant": 1, "pilgrim": 4, "watchman_and_keeper": 2,
    "court_officer": 1, "revenue_worker": 1, "scribe_and_clerk": 2,
    "healer": 2, "church_attendant": 1, "candor_cleric": 1,
    "funerary_worker": 0, "bell_ringer": 0, "glazier": 1, "draper": 1,
    "lamplighter": 1, "messenger": 1, "guide": 1, "money_dealer": 1,
    "scholar": 0, "painter": 0, "instrument_maker": 1,
    "bellfounder": 0, "freight_broker": 0, "salt_trader": 1,
    # New families: 106 women, 102 men.
    "domestic_servant": 35, "garment_worker": 10, "shoemaker": 4,
    "cooper": 2, "potter": 4, "cartwright_and_wheelwright": 1,
    "fine_metalworker": 2, "animal_worker": 6, "sanitation_worker": 5,
    "water_and_bath_worker": 4, "food_provisioner": 12,
    "grocer_and_spicer": 3, "entertainer": 5, "civic_officer": 3,
    "bailiff_and_gaoler": 2, "militia_and_soldier": 2, "sex_worker": 6,
    # No fixed trade: 6 women, 4 men. New total is 208/189, making 250/250.
    "no_fixed_trade": 6,
}

# Counts assigned to 0-7, 8-11 and 12-15 respectively.
CHILD_ALLOCATION = {
    "no_fixed_trade": (6, 2, 1), "pilgrim": (5, 1, 1),
    "domestic_servant": (8, 8, 6), "animal_worker": (2, 3, 2),
    "market_seller": (1, 2, 0), "food_provisioner": (2, 3, 2),
    "entertainer": (1, 2, 1), "messenger": (0, 2, 1),
    "scavenger": (0, 1, 1), "garment_worker": (0, 0, 2),
    "shoemaker": (0, 0, 1), "farmer": (0, 0, 1),
}

YOUNGEST_AGES = {
    "no_fixed_trade": [2, 3, 4, 5, 6, 7],
    "pilgrim": [2, 3, 4, 5, 6],
    "domestic_servant": [5, 5, 6, 6, 6, 7, 7, 7],
    "animal_worker": [6, 7], "market_seller": [7],
    "food_provisioner": [6, 7], "entertainer": [7],
}

WARD_TARGETS_NEW = {
    # The original feature's near-even counts concentrated people inside the
    # compact named-ward rectangles. These totals follow each ward's share of
    # the full safe city footprint instead.
    "fabric": 25, "wick": 30, "cloth": 38, "wallwright": 19,
    "cinder": 27, "weigh": 51, "reed": 47, "bell_and_sluice": 160,
}

WARD_LABELS = {
    "fabric": "Fabric Ward streets",
    "wick": "Wick Ward streets",
    "cloth": "Cloth Ward streets",
    "wallwright": "Wallwright Ward streets",
    "cinder": "Cinder Ward streets",
    "weigh": "Weigh Ward streets",
    "reed": "Reed Ward streets",
    "bell_and_sluice": "Bell-and-Sluice streets",
}

WARD_BOUNDS = {
    "fabric": (-110.0, 180.0, -160.0, 235.0),
    "wick": (-170.0, 130.0, 235.0, 500.0),
    "cloth": (80.0, 275.0, 190.0, 390.0),
    "wallwright": (175.0, 485.0, 40.0, 260.0),
    "cinder": (-270.0, 20.0, 115.0, 315.0),
    "weigh": (-435.0, -175.0, -40.0, 235.0),
    "reed": (-455.0, -160.0, -500.0, -235.0),
    "bell_and_sluice": (-180.0, 330.0, -620.0, -150.0),
}

ALL_WARDS = list(WARD_TARGETS_NEW)

PREFERRED_WARDS = {
    "general_labourer": ["wallwright", "weigh", "fabric", "reed"],
    "cargo_worker": ["weigh", "reed", "wallwright"],
    "market_seller": ["wick", "cloth", "reed", "fabric"],
    "scavenger": ["weigh", "reed", "cinder", "bell_and_sluice"],
    "tavern_worker": ["bell_and_sluice", "wick", "weigh", "reed"],
    "cook": ["wick", "fabric", "bell_and_sluice", "weigh"],
    "baker": ["wick", "wallwright", "bell_and_sluice"],
    "brewer": ["wick", "wallwright", "bell_and_sluice"],
    "butcher": ["weigh", "reed", "wick"], "miller": ["wallwright", "wick"],
    "farmer": ["wallwright", "wick", "reed"],
    "carpenter_and_builder": ["wallwright", "fabric", "cloth"],
    "mason": ["wallwright", "fabric"], "cloth_worker": ["cloth", "cinder"],
    "chandler": ["wick", "fabric"], "smith": ["wallwright", "cinder"],
    "leather_worker": ["reed", "cloth"], "laundress": ["reed", "fabric", "wick"],
    "roper": ["weigh", "reed"], "boatworker": ["reed", "weigh"],
    "fish_trader": ["reed", "weigh"], "salt_worker": ["weigh", "reed"],
    "merchant": ["weigh", "cloth", "wick"], "pilgrim": ["fabric", "wick"],
    "watchman_and_keeper": ["bell_and_sluice", "wick", "wallwright", "reed"],
    "court_officer": ["weigh", "fabric"], "revenue_worker": ["weigh", "wick"],
    "scribe_and_clerk": ["weigh", "fabric", "cloth"],
    "healer": ["fabric", "reed", "wick"], "church_attendant": ["fabric", "reed"],
    "candor_cleric": ["fabric", "reed"], "funerary_worker": ["reed"],
    "bell_ringer": ["fabric", "bell_and_sluice"], "glazier": ["cinder", "fabric"],
    "draper": ["cloth"], "lamplighter": ALL_WARDS,
    "messenger": ALL_WARDS, "guide": ["wick", "fabric", "bell_and_sluice"],
    "money_dealer": ["weigh", "cloth"], "scholar": ["fabric", "cloth"],
    "painter": ["cinder", "fabric"], "instrument_maker": ["cloth", "fabric"],
    "bellfounder": ["bell_and_sluice", "wallwright"],
    "freight_broker": ["weigh", "reed"], "salt_trader": ["weigh", "reed"],
    "domestic_servant": ALL_WARDS, "garment_worker": ["cloth", "wick", "fabric"],
    "shoemaker": ["cloth", "fabric", "cinder"], "cooper": ["weigh", "reed", "wick"],
    "potter": ["cinder", "wallwright", "reed"],
    "cartwright_and_wheelwright": ["wallwright", "weigh", "wick"],
    "fine_metalworker": ["cloth", "fabric", "cinder", "wallwright"],
    "animal_worker": ["wallwright", "wick", "reed", "weigh"],
    "sanitation_worker": ALL_WARDS, "water_and_bath_worker": ALL_WARDS,
    "food_provisioner": ALL_WARDS, "grocer_and_spicer": ["weigh", "wick", "cloth"],
    "entertainer": ["wick", "bell_and_sluice", "fabric", "weigh", "reed"],
    "civic_officer": ["weigh", "fabric", "bell_and_sluice"],
    "bailiff_and_gaoler": ["weigh", "bell_and_sluice", "fabric"],
    "militia_and_soldier": ["wallwright", "bell_and_sluice", "wick", "reed"],
    "sex_worker": ["bell_and_sluice", "weigh", "wick", "cloth"],
    "no_fixed_trade": ["fabric", "wick", "weigh", "reed", "bell_and_sluice"],
}

RANKABLE = {
    "baker", "brewer", "butcher", "miller", "carpenter_and_builder", "mason",
    "cloth_worker", "chandler", "smith", "leather_worker", "laundress", "roper",
    "boatworker", "merchant", "scribe_and_clerk", "healer", "church_attendant",
    "candor_cleric", "glazier", "draper", "painter", "instrument_maker",
    "bellfounder", "garment_worker", "shoemaker", "cooper", "potter",
    "cartwright_and_wheelwright", "fine_metalworker", "grocer_and_spicer",
}

LOW_INCOME = [
    "no_fixed_trade", "pilgrim", "general_labourer", "cargo_worker", "scavenger",
    "sanitation_worker", "domestic_servant", "sex_worker", "market_seller",
    "messenger", "farmer", "laundress", "animal_worker", "water_and_bath_worker",
    "tavern_worker", "cook", "food_provisioner", "salt_worker", "boatworker",
]

CRIME_FAMILIES = [
    "cargo_worker", "market_seller", "tavern_worker", "general_labourer", "merchant",
    "messenger", "guide", "money_dealer", "revenue_worker", "scribe_and_clerk",
    "watchman_and_keeper", "domestic_servant", "food_provisioner", "grocer_and_spicer",
    "entertainer", "civic_officer", "bailiff_and_gaoler", "militia_and_soldier",
    "sex_worker", "animal_worker", "butcher", "pilgrim", "scavenger",
]

ILLEGAL_ACTIVITIES = [
    "pickpocketing", "burglary", "receiving stolen household goods", "cargo pilfering",
    "toll evasion", "filing false manifests", "coin clipping", "using false weights",
    "watering ale", "adulterating food", "unlicensed vending", "guild evasion",
    "running illegal gambling", "rigging games", "keeping an unregistered brothel",
    "protection and intimidation", "taking bribes during inspection",
    "selling counterfeit pilgrim badges", "poaching", "illicit slaughter",
    "violent debt collection", "gate bribery", "warehouse sabotage",
]

CRIME_BY_FAMILY = {
    "cargo_worker": ["cargo pilfering", "filing false manifests"],
    "market_seller": ["using false weights", "unlicensed vending", "pickpocketing"],
    "tavern_worker": ["watering ale", "running illegal gambling"],
    "general_labourer": ["burglary", "protection and intimidation"],
    "merchant": ["toll evasion", "filing false manifests"],
    "messenger": ["pickpocketing", "selling counterfeit pilgrim badges"],
    "guide": ["selling counterfeit pilgrim badges", "pickpocketing"],
    "money_dealer": ["coin clipping", "receiving stolen household goods"],
    "revenue_worker": ["taking bribes during inspection", "filing false manifests"],
    "scribe_and_clerk": ["filing false manifests", "coin clipping"],
    "watchman_and_keeper": ["gate bribery", "protection and intimidation"],
    "domestic_servant": ["burglary", "receiving stolen household goods"],
    "food_provisioner": ["adulterating food", "using false weights"],
    "grocer_and_spicer": ["adulterating food", "guild evasion"],
    "entertainer": ["rigging games", "running illegal gambling"],
    "civic_officer": ["taking bribes during inspection"],
    "bailiff_and_gaoler": ["violent debt collection", "taking bribes during inspection"],
    "militia_and_soldier": ["protection and intimidation", "warehouse sabotage"],
    "sex_worker": ["keeping an unregistered brothel", "protection and intimidation"],
    "animal_worker": ["poaching", "receiving stolen household goods"],
    "butcher": ["illicit slaughter", "adulterating food"],
    "pilgrim": ["selling counterfeit pilgrim badges", "pickpocketing"],
    "scavenger": ["receiving stolen household goods", "burglary"],
}

MEN = ["Aubin", "Colm", "Corin", "Dunstan", "Grigor", "Hamel", "Jos", "Noll",
       "Segwin", "Ansel", "Bertran", "Ewart", "Gile", "Renn", "Tobin", "Warin"]
WOMEN = ["Aldith", "Betriss", "Ede", "Havise", "Idonea", "Jonet", "Lise", "Osanne",
         "Averil", "Clemence", "Petronel", "Rohese", "Sibbe", "Sible", "Renna", "Gude"]
SMALL_NAMES = ["Pin", "Ib", "Cobb", "Sef", "Dob", "Mote", "Nan", "Tib"]
BYNAMES = [
    "Crake", "Hobbe", "Skell", "Tarn", "Ashe", "Marle", "Fitch", "Pike",
    "Brant", "Vell", "Rasp", "Stott", "Rud", "Quern", "Skep", "Rook",
    "Dask", "Clove", "Bram", "Fenn", "Kett", "Mott", "Rill", "Sedge",
    "Thorn", "Wren", "Dunn", "Galt", "Husk", "Kern", "Lark", "Mere",
    "Nett", "Pell", "Rusk", "Sark", "Toll", "Varn", "Wick", "Yew",
]

SPEECH_MANNERS = [
    "You answer strangers in clipped practical phrases", "You speak slowly and test every claim",
    "You are quick with dry jokes but slow with trust", "You talk warmly until money is mentioned",
    "You use few words and watch hands more than faces", "You chatter when nervous and laugh at yourself",
    "You are courteous, direct and hard to hurry", "You bargain cheerfully and remember exact sums",
    "You speak softly but refuse to be talked over", "You ask one blunt question before offering help",
    "You meet strangers with guarded good manners", "You tell small stories instead of giving straight answers",
]
CONCERNS = [
    "recover a disputed penny", "dry a blanket before curfew", "find the owner of a missing hen",
    "secure tomorrow's place in the hiring line", "replace a cracked cup", "collect a late wage",
    "mend a leaking shoe", "keep a sleeping place for tonight", "buy lamp oil before dusk",
    "learn why a promised cart is late", "get a splinter from your palm", "find two clean bandages",
    "settle whose turn it is at the well", "return a borrowed knife", "sell the last bruised apple",
    "keep rain off a bundle of kindling", "locate a younger sibling", "stretch supper to one more bowl",
    "have a bent buckle straightened", "avoid losing a rented stool", "deliver a message before the next bell",
    "find a quiet corner to sleep", "collect kindling without a fine", "trade for a sound length of cord",
    "get an employer to count the full day", "clear mud from your only coat", "borrow a handcart for an hour",
    "have a sore heel looked at", "find the key to a shared chest", "learn who moved your market basket",
]
VISIBLE_FACTS = [
    "Your left cuff is carefully patched in red", "A loop of blue cord holds your hair back",
    "You carry a horn spoon tucked through your belt", "One shoe has a new wooden sole",
    "Your knuckles are stained with soot", "A chipped green bead hangs at your throat",
    "You keep counting on the fingers of one hand", "Your apron bears three square repairs",
    "A reed whistle sticks from your sleeve", "Your broad hat has lost half its brim",
    "White lime dust sits in the seams of your coat", "You wear mismatched woollen gloves",
    "A burn mark crosses the back of your right hand", "Your belt is tied with bright sail thread",
    "You carry a basket with one broken handle", "Your hair is cropped close on one side",
    "A brass button replaces the clasp at your neck", "You smell faintly of woodsmoke and mint",
    "Your sleeves are rolled to different heights", "A tiny clay bird rides in your pocket",
    "Your nose has been broken and set slightly sideways", "You have ink on one thumb",
    "Three keys knock together at your belt", "Your cap is pinned with a goose feather",
    "A pale scar notches your lower lip", "Your coat is too large and belted twice",
    "You carry a smooth tally stick", "A string of nutshells circles one wrist",
    "Your basket is lined with striped cloth", "Your boots are polished only across the toes",
]
CONDITIONS = [
    "blind in one eye", "deaf in the left ear", "missing two fingers", "chronic cough",
    "lame", "failing sight", "burn-scarred forearm", "stiff right knee",
    "tremor in both hands", "partial hearing loss", "club foot", "weak left arm",
    "recurring fever", "crooked spine", "old shoulder injury", "tooth pain",
    "arthritic hands", "shortness of breath", "speech impediment", "one hand",
]

MINOR_TITLES = {
    "civic_officer": "First Seat", "bailiff_and_gaoler": "Stone keeper",
    "militia_and_soldier": "Wall captain", "sex_worker": "Brothel keeper",
    "entertainer": "Gaming-room keeper", "domestic_servant": "Housekeeper",
    "garment_worker": "Tailor", "shoemaker": "Cordwainer", "cooper": "Cooper",
    "potter": "Potter", "cartwright_and_wheelwright": "Wheelwright",
    "fine_metalworker": "Locksmith", "animal_worker": "Ostler",
    "sanitation_worker": "Dung carter", "water_and_bath_worker": "Well keeper",
    "food_provisioner": "Grain dealer", "grocer_and_spicer": "Spicer",
}

# Ambient people may belong to civic and licensed institutions, but they must not
# silently multiply the singular offices that anchor those institutions in canon.
AMBIENT_INSTITUTION_TITLES = {
    "entertainer": [
        "Musician", "Singer", "Storyteller", "Juggler", "Dancer", "Puppeteer", "Tumbler",
    ],
    "civic_officer": ["Bencher", "Ward hand", "Measure clerk"],
    "bailiff_and_gaoler": [
        "Bench sergeant", "Gaoler", "Prison guard", "Debt officer", "Court usher",
    ],
    "militia_and_soldier": ["Militiaman", "Wall guard", "Hired soldier", "File leader"],
    "sex_worker": [
        "Sex worker", "Independent companion", "Registered-house worker", "House keeper",
    ],
}

MINOR_ROLE_DETAILS = {
    "domestic_servant": "You coordinate beds, keys and food stores for a large household without confusing service with ownership.",
    "garment_worker": "Your fitting bench trains apprentices and takes the ward's difficult alterations.",
    "shoemaker": "Your shop makes new shoes but keeps a cheap repair stool for neighbours with little coin.",
    "cooper": "Brewers and fish traders rely on your mark when a cask must hold through winter.",
    "potter": "You schedule the shared kiln and refuse loads that would make its firing unsafe.",
    "cartwright_and_wheelwright": "Gate carriers bring you wheels that must be sound before the next freight bell.",
    "fine_metalworker": "Householders trust your locks, and the Bench sometimes asks you to examine a forced one.",
    "animal_worker": "You allot stable space at the gate yard and know which drovers leave their animals hungry.",
    "sanitation_worker": "You set the crew's route from market gutters toward the gate carts and argue for full wages.",
    "water_and_bath_worker": "You keep a ward well's rope, cover and turn-list rather than claiming a public bathhouse exists.",
    "food_provisioner": "You buy grain in bulk, sell it honestly by small measure and extend short credit to known households.",
    "grocer_and_spicer": "You keep costly oils and spices under seal and arbitrate small disputes over adulteration.",
    "entertainer": "You run a licensed gaming room, check the pieces nightly and bar lenders who wager against a person's freedom.",
    "civic_officer": "You chair the Common Bench for one year, arrange its docket and speak only for rules it has actually voted.",
    "bailiff_and_gaoler": "You keep the Stone House, publish its fixed fees and answer to the Full Measure for every prisoner.",
    "militia_and_soldier": "You command the Wall Muster for a three-year term but cannot levy tax or keep it armed after dismissal.",
    "sex_worker": "You keep a registered house, enforce adult consent and debt limits, and know exactly what the licence permits.",
}

MINOR_ROLE_CONCERNS = {
    "domestic_servant": "account for a missing pantry key",
    "garment_worker": "finish a disputed fitting before dusk",
    "shoemaker": "find leather for two promised repairs",
    "cooper": "replace a warped hoop before the brewer arrives",
    "potter": "keep damp fuel out of tomorrow's firing",
    "cartwright_and_wheelwright": "straighten a freight wheel before the gate closes",
    "fine_metalworker": "identify who filed a copied key",
    "animal_worker": "find fodder for a drover's underfed mare",
    "sanitation_worker": "get the ward hand to count a full crew day",
    "water_and_bath_worker": "replace the fraying well rope",
    "food_provisioner": "recover two sacks sent to the wrong loft",
    "grocer_and_spicer": "prove an oil jar has not been watered",
    "entertainer": "replace a marked gaming piece",
    "civic_officer": "have two expenditure warrants signed",
    "bailiff_and_gaoler": "replace a broken Stone House lock",
    "militia_and_soldier": "find three missing muster spearheads",
    "sex_worker": "settle a licence inspection before curfew",
}


@dataclass
class Draft:
    occupation_id: str | None
    local_index: int
    id: str
    significance: str
    gender: str
    age: int = 0
    ward: str = ""
    name: str = ""
    title: str | None = None
    rank: str | None = None
    statuses: list[str] = field(default_factory=list)
    conditions: list[str] = field(default_factory=list)
    illegal_activity: str | None = None
    knows: list[str] = field(default_factory=list)
    father: str | None = None
    mother: str | None = None
    children: list[str] = field(default_factory=list)
    cohort: str = ""
    concern: str = ""
    visible: str = ""
    speech: str = ""

    @property
    def family(self) -> str:
        return self.occupation_id or "no_fixed_trade"


def generated_id(index: int) -> str:
    digits = "0123456789abcdefghijklmnopqrstuvwxyz"
    value = index
    tail = ""
    for _ in range(4):
        tail = digits[value % 36] + tail
        value //= 36
    return "p" + tail


def existing_ward(sheet: dict) -> str:
    # The opening trio stand on the Gradine even when they work elsewhere.
    if sheet["id"] in {"sv3n1", "cb947", "k0fb1"}:
        return "fabric"
    district = sheet["district"]
    z = sheet["spawn_location"]["z"]
    if district.startswith(("The Lanthorn", "The chapter house", "Skinners' Court", "The Gradine")):
        return "fabric"
    if "Wickmarket" in district:
        return "wick"
    if district in {"The Draper's Reach", "Tenterhook Lane"}:
        return "cloth"
    if district.startswith("Coswald's Yard") or district == "Malt Passage":
        return "wallwright"
    if district.startswith("Cinder Row") or district.startswith("The Needle"):
        return "cinder"
    if district.startswith(("The Tallage", "A tall house off the Tallage", "Gaunt Passage", "The Tally Bridge", "The shambles")):
        return "weigh"
    if district.startswith("The Cut"):
        return "weigh" if z > -200 else "reed"
    if district.startswith(("Maren", "Saint Maren", "Tanners' Slip", "The Alder Moorings", "The Hungry Ox")):
        return "reed"
    if district.startswith(("The Bellstand", "Bellfoot Passage", "Lodgings off the Bellstand", "The Ilvane", "The Old Sluice")):
        return "bell_and_sluice"
    raise AssertionError(f"unassigned existing district: {district}")


def ordered_sheet(sheet: dict) -> dict:
    keys = [
        "id", "name", "significance", "planning_ward", "age", "gender",
        "occupation_id", "title", "rank", "faction_role", "illegal_activity",
        "district", "knows", "father", "mother", "children", "spawn_location",
        "statuses", "conditions", "memories", "core_character_description",
        "extended_character_description", "appearance_key", "voice_key", "holds", "goal",
    ]
    return {key: sheet[key] for key in keys if key in sheet}


def migrate_existing() -> list[dict]:
    status_map = {
        "widow": "widow", "pauper": "pauper", "orphan": "orphan",
        "enclosed": "enclosed_religious", "recanted heretic": "recanted_heretic",
        "one of the Spared": "spared", "illiterate": "illiterate",
    }
    sheets = []
    for path in sorted(CHARACTERS.rglob("*.json")):
        raw = json.loads(path.read_text())
        if re.fullmatch(r"p[0-9a-z]{4}", raw["id"]):
            path.unlink()
            continue
        raw["significance"] = "major" if raw["id"] in MAJOR_IDS else "minor"
        raw["planning_ward"] = existing_ward(raw)
        statuses = list(raw.get("statuses", []))
        kept_conditions = []
        for condition in raw.get("conditions", []):
            if condition in status_map:
                statuses.append(status_map[condition])
            else:
                kept_conditions.append(condition)
        raw["statuses"] = sorted(set(statuses))
        raw["conditions"] = kept_conditions
        raw = ordered_sheet(raw)
        path.write_text(json.dumps(raw, indent=2, ensure_ascii=False) + "\n")
        sheets.append(raw)
    assert len(sheets) == 103
    assert len(MAJOR_IDS) == 30 and MAJOR_IDS <= {sheet["id"] for sheet in sheets}
    return sheets


def make_drafts() -> list[Draft]:
    additions = {**EXISTING_ADDITIONS, **NEW_FAMILY_ADDITIONS, "no_fixed_trade": 10}
    assert sum(EXISTING_ADDITIONS.values()) == 179
    assert sum(NEW_FAMILY_ADDITIONS.values()) == 208
    assert sum(additions.values()) == 397
    assert sum(FEMALE_COUNTS.values()) == 208
    drafts = []
    index = 0
    for family, count in additions.items():
        for local_index in range(count):
            minor = local_index == 0 and (
                family in MINOR_EXISTING_FAMILIES or family in MINOR_NEW_FAMILIES
            )
            drafts.append(Draft(
                occupation_id=None if family == "no_fixed_trade" else family,
                local_index=local_index,
                id=generated_id(index),
                significance="minor" if minor else "ambient",
                gender="f" if local_index < FEMALE_COUNTS[family] else "m",
            ))
            index += 1
    assert Counter(d.significance for d in drafts) == {"ambient": 350, "minor": 47}
    return drafts


def assign_ages(drafts: list[Draft]) -> None:
    rng = random.Random(437)
    bins = {
        0: [2 + i % 6 for i in range(25)], 1: [8 + i % 4 for i in range(24)],
        2: [12 + i % 4 for i in range(19)], 3: [16 + i % 4 for i in range(46)],
        4: [20 + i % 20 for i in range(144)], 5: [40 + i % 20 for i in range(87)],
        6: [60 + i % 22 for i in range(52)],
    }
    for values in bins.values():
        rng.shuffle(values)
    by_family = defaultdict(list)
    for draft in drafts:
        by_family[draft.family].append(draft)
    assigned = set()
    for family, counts in CHILD_ALLOCATION.items():
        total_children = sum(counts)
        child_slots = (
            by_family[family]
            if family == "no_fixed_trade"
            else by_family[family][-total_children:]
        )
        offset = 0
        for bin_id, count in enumerate(counts):
            selected = child_slots[offset:offset + count]
            for child_index, draft in enumerate(selected):
                draft.age = (
                    YOUNGEST_AGES[family][child_index]
                    if bin_id == 0
                    else bins[bin_id].pop()
                )
                assigned.add(draft.id)
            offset += count
    bins[0].clear()
    adult_ages = bins[3] + bins[4] + bins[5] + bins[6]
    rng.shuffle(adult_ages)
    remaining = [draft for draft in drafts if draft.id not in assigned]
    assert len(adult_ages) == len(remaining)
    for draft, age in zip(remaining, adult_ages, strict=True):
        draft.age = age
    assert not any(bins[i] for i in range(3))


def assign_wards(drafts: list[Draft]) -> None:
    remaining = dict(WARD_TARGETS_NEW)
    # Interleave occupation families instead of exhausting one trade at a time.
    by_family = defaultdict(list)
    for draft in drafts:
        by_family[draft.family].append(draft)
    ordered = []
    for local_index in range(max(map(len, by_family.values()))):
        for family in by_family:
            if local_index < len(by_family[family]):
                ordered.append(by_family[family][local_index])
    ward_order = {ward: index for index, ward in enumerate(ALL_WARDS)}
    for draft in ordered:
        preferred = PREFERRED_WARDS[draft.family]
        candidates = [ward for ward in preferred if remaining[ward] > 0]
        if not candidates:
            candidates = [ward for ward in ALL_WARDS if remaining[ward] > 0]
        ward = max(
            candidates,
            key=lambda value: (remaining[value] / WARD_TARGETS_NEW[value], -ward_order[value]),
        )
        draft.ward = ward
        remaining[ward] -= 1
    assert not any(remaining.values()), remaining


def round_robin(drafts: list[Draft], families: list[str], predicate) -> list[Draft]:
    by_family = defaultdict(list)
    for draft in drafts:
        if draft.family in families and predicate(draft):
            by_family[draft.family].append(draft)
    output = []
    for index in range(max([len(values) for values in by_family.values()] or [0])):
        for family in families:
            if index < len(by_family[family]):
                output.append(by_family[family][index])
    return output


def assign_ranks(drafts: list[Draft]) -> None:
    leaders = round_robin(drafts, sorted(RANKABLE), lambda d: d.age >= 30)[:29]
    assert len(leaders) == 29
    for index, draft in enumerate(leaders):
        if index in {8, 18, 28}:
            draft.rank = "warden"
        else:
            draft.rank = "mistress" if draft.gender == "f" else "master"
    apprentices = round_robin(
        drafts, sorted(RANKABLE), lambda d: 12 <= d.age <= 19 and d.rank is None
    )[:25]
    assert len(apprentices) == 25
    for draft in apprentices:
        draft.rank = "apprentice"
    journeymen = round_robin(
        drafts, sorted(RANKABLE), lambda d: 20 <= d.age <= 39 and d.rank is None
    )[:40]
    assert len(journeymen) == 40
    for draft in journeymen:
        draft.rank = "journeyman"


def add_status(draft: Draft, *statuses: str) -> None:
    for status in statuses:
        if status not in draft.statuses:
            draft.statuses.append(status)


def assign_statuses_and_conditions(drafts: list[Draft]) -> None:
    no_trade = [draft for draft in drafts if draft.occupation_id is None]
    poor_minors = round_robin(
        drafts, LOW_INCOME,
        lambda d: d.significance == "minor" and d.age >= 16,
    )[:5]
    poverty_candidates = round_robin(
        drafts, LOW_INCOME,
        lambda d: d.occupation_id is not None and d.significance == "ambient",
    )
    poor = no_trade + poor_minors + poverty_candidates[:92 - len(no_trade) - len(poor_minors)]
    assert len(poor) == 92
    for draft in poor:
        add_status(draft, "pauper")
    child_beggars = [draft for draft in poor if draft.age < 16 and draft.significance == "ambient"][:4]
    adult_beggars = [
        draft for draft in poor
        if draft.age >= 16 and draft.significance == "ambient" and draft not in child_beggars
    ][:31]
    beggars = poor_minors[:3] + child_beggars + adult_beggars
    assert len(beggars) == 38
    for draft in beggars:
        add_status(draft, "alms_dependent", "begs_regularly")
    housing = poor.copy()
    random.Random(244).shuffle(housing)
    for draft in housing[:18]:
        add_status(draft, "unhoused")
    for draft in housing[18:32]:
        add_status(draft, "insecure_lodging")
    precarious = round_robin(
        drafts,
        ["general_labourer", "cargo_worker", "market_seller", "scavenger",
         "sanitation_worker", "domestic_servant", "animal_worker", "tavern_worker",
         "food_provisioner", "laundress", "boatworker", "farmer"],
        lambda d: d.age >= 12,
    )[:60]
    assert len(precarious) == 60
    for draft in precarious:
        add_status(draft, "intermittently_employed")
    for draft in drafts:
        if draft.age < 12:
            add_status(draft, "dependent")
    children = [draft for draft in drafts if draft.age < 16]
    random.Random(301).shuffle(children)
    for draft in children[:15]:
        add_status(draft, "orphan")
    migrants = sorted(
        [draft for draft in drafts if draft.age >= 12 and draft.significance == "ambient"],
        key=lambda draft: (len(draft.statuses), draft.id),
    )[:35]
    assert len(migrants) == 35
    for index, draft in enumerate(migrants):
        add_status(draft, "recent_migrant")
        if index < 25:
            add_status(draft, "noncitizen")
    unemployed = sorted(
        [draft for draft in poor if draft.age >= 16 and "prisoner" not in draft.statuses],
        key=lambda draft: (len(draft.statuses), draft.id),
    )[:20]
    for draft in unemployed:
        add_status(draft, "unemployed")
    elders = sorted(
        [draft for draft in drafts if draft.age >= 60],
        key=lambda draft: (len(draft.statuses), draft.id),
    )[:20]
    for draft in elders:
        add_status(draft, "retired")
    widowed = sorted(
        [draft for draft in drafts if draft.age >= 35],
        key=lambda draft: (len(draft.statuses), draft.id),
    )[:18]
    for draft in widowed:
        add_status(draft, "widow" if draft.gender == "f" else "widower")
    prison_candidates = [draft for draft in drafts if draft.age >= 16 and draft.family in {
        "no_fixed_trade", "general_labourer", "cargo_worker", "domestic_servant",
        "militia_and_soldier", "sex_worker", "market_seller", "animal_worker",
    }]
    prison_candidates.sort(key=lambda draft: (len(draft.statuses), draft.significance != "ambient", draft.id))
    no_trade_adult = next(draft for draft in prison_candidates if draft.occupation_id is None)
    prisoners = [no_trade_adult] + [draft for draft in prison_candidates if draft is not no_trade_adult][:7]
    assert len(prisoners) == 8
    for draft in prisoners:
        add_status(draft, "prisoner")
    # Every null occupation explicitly records a material means of support.
    for draft in drafts:
        if draft.occupation_id is None and not set(draft.statuses) & {
            "dependent", "alms_dependent", "pauper", "prisoner"
        }:
            add_status(draft, "pauper", "alms_dependent")

    disabled = sorted(
        [draft for draft in drafts if draft.age >= 16],
        key=lambda draft: (len(draft.statuses) + (draft.illegal_activity is not None), draft.id),
    )[:20]
    for draft, condition in zip(disabled, CONDITIONS, strict=True):
        draft.conditions.append(condition)
    pregnant = sorted(
        [draft for draft in drafts if draft.gender == "f" and 20 <= draft.age <= 39],
        key=lambda draft: (len(draft.statuses), draft.id),
    )[:6]
    for draft in pregnant:
        draft.conditions.append("pregnant")
    nursing = sorted(
        [draft for draft in drafts if draft.gender == "f" and 20 <= draft.age <= 39 and draft not in pregnant],
        key=lambda draft: (len(draft.statuses), draft.id),
    )[:4]
    for draft in nursing:
        draft.conditions.append("nursing an infant")
    for draft in drafts:
        draft.statuses.sort()


def assign_crime(drafts: list[Draft]) -> None:
    minor_crime_families = {
        "cargo_worker", "tavern_worker", "watchman_and_keeper",
        "revenue_worker", "bailiff_and_gaoler",
    }
    minor_candidates = [
        draft for draft in drafts
        if draft.significance == "minor" and draft.family in minor_crime_families and draft.age >= 16
    ]
    assert len(minor_candidates) == 5
    by_family = defaultdict(list)
    for draft in drafts:
        if draft.significance == "ambient" and draft.family in CRIME_FAMILIES and draft.age >= 16:
            by_family[draft.family].append(draft)
    for values in by_family.values():
        values.sort(key=lambda draft: (len(draft.statuses), draft.id))
    ambient_candidates = []
    for index in range(max(map(len, by_family.values()))):
        for family in CRIME_FAMILIES:
            if index < len(by_family[family]):
                ambient_candidates.append(by_family[family][index])
    candidates = minor_candidates + ambient_candidates[:37]
    assert len(candidates) == 42
    used_by_family = Counter()
    for draft in candidates:
        choices = CRIME_BY_FAMILY[draft.family]
        draft.illegal_activity = choices[used_by_family[draft.family] % len(choices)]
        used_by_family[draft.family] += 1


def choose_title(draft: Draft, titles: list[str]) -> str | None:
    if draft.occupation_id is None:
        return None
    if draft.significance == "minor" and draft.family in MINOR_TITLES:
        return MINOR_TITLES[draft.family]
    if draft.significance == "ambient" and draft.family in AMBIENT_INSTITUTION_TITLES:
        allowed = AMBIENT_INSTITUTION_TITLES[draft.family]
        assert set(allowed) <= set(titles)
        return allowed[(draft.local_index * 3 + draft.age) % len(allowed)]
    young = {
        "domestic_servant": "Errand child", "animal_worker": "Stable child",
        "food_provisioner": "Food-stall helper", "entertainer": "Tumbler",
    }
    if draft.age < 12 and young.get(draft.family) in titles:
        return young[draft.family]
    if draft.rank == "apprentice":
        apprentice = next((title for title in titles if "apprentice" in title.lower() or title == "Novice"), None)
        if apprentice:
            return apprentice
    if draft.rank in {"master", "mistress"}:
        needle = "mistress" if draft.rank == "mistress" else "master"
        ranked = next((title for title in titles if needle in title.lower()), None)
        if ranked:
            return ranked
    return titles[(draft.local_index * 3 + draft.age) % len(titles)]


def assign_titles(drafts: list[Draft], occupation_catalog: dict[str, dict]) -> None:
    for draft in drafts:
        if draft.occupation_id is None:
            draft.title = None
        else:
            titles = occupation_catalog[draft.occupation_id]["alternative_titles"]
            draft.title = choose_title(draft, titles)
            assert draft.title in titles


def assign_names_and_relationships(drafts: list[Draft]) -> None:
    used_names = {
        json.loads(path.read_text())["name"]
        for path in CHARACTERS.rglob("*.json")
        if not re.fullmatch(r"p[0-9a-z]{4}", json.loads(path.read_text())["id"])
    }
    used_names.update(RESERVED_CANON_NAMES)
    original_name_count = len(used_names)
    cohort_number = 0

    def name_person(draft: Draft, byname: str) -> None:
        pool = SMALL_NAMES + (WOMEN if draft.gender == "f" else MEN) if draft.age < 12 else (
            WOMEN if draft.gender == "f" else MEN
        )
        start = (int(draft.id[1:], 36) * 7 + draft.age) % len(pool)
        for offset in range(len(pool)):
            candidate = f"{pool[(start + offset) % len(pool)]} {byname}"
            if candidate not in used_names:
                draft.name = candidate
                used_names.add(candidate)
                return
        # Extremely defensive; current cohort cycling never reaches this.
        candidate = f"{pool[start]} {byname} of {draft.ward.replace('_', ' ').title()}"
        assert candidate not in used_names
        draft.name = candidate
        used_names.add(candidate)

    for ward in ALL_WARDS:
        ambient = [draft for draft in drafts if draft.ward == ward and draft.significance == "ambient"]
        minors = [draft for draft in drafts if draft.ward == ward and draft.significance == "minor"]
        children = [draft for draft in ambient if draft.age < 16]
        adults = sorted(
            [draft for draft in ambient if draft.age >= 25],
            key=lambda draft: (-draft.age, draft.id),
        )
        grouped = set()
        groups: list[tuple[list[Draft], bool]] = []
        while children and adults:
            child_group = children[:2]
            del children[:len(child_group)]
            adult_group = adults[:2]
            del adults[:len(adult_group)]
            group = adult_group + child_group
            grouped.update(draft.id for draft in group)
            groups.append((group, True))
        rest = [draft for draft in ambient if draft.id not in grouped]
        groups.extend((rest[index:index + 4], False) for index in range(0, len(rest), 4))

        for group, family_group in groups:
            byname = BYNAMES[cohort_number % len(BYNAMES)]
            cohort_number += 1
            label = f"the {byname} household" if family_group else f"a shared {ward.replace('_', '-')} work-and-lodging group"
            for draft in group:
                name_person(draft, byname if family_group else BYNAMES[(cohort_number + draft.local_index) % len(BYNAMES)])
                draft.cohort = label
            for index, draft in enumerate(group):
                peers = [peer.id for peer in group if peer.id != draft.id]
                draft.knows = peers[:2]
            if family_group:
                parents = [draft for draft in group if draft.age >= 25]
                kids = [draft for draft in group if draft.age < 16]
                mother = next((draft for draft in parents if draft.gender == "f"), None)
                father = next((draft for draft in parents if draft.gender == "m"), None)
                for child in kids:
                    if "orphan" in child.statuses:
                        continue
                    if mother:
                        child.mother = mother.id
                        mother.children.append(child.id)
                    if father:
                        child.father = father.id
                        father.children.append(child.id)

        if minors:
            label = f"the established {ward.replace('_', '-')} ward circle"
            for index, draft in enumerate(minors):
                name_person(draft, BYNAMES[(cohort_number + index) % len(BYNAMES)])
                draft.cohort = label
                others = [other.id for other in minors if other.id != draft.id]
                draft.knows = others[:3]
            cohort_number += len(minors)
    assert len(used_names) == original_name_count + 397


def point_in_polygon(point: tuple[float, float], polygon: list[list[float]]) -> bool:
    x, z = point
    inside = False
    for a, b in zip(polygon, polygon[1:] + polygon[:1]):
        if (a[1] > z) != (b[1] > z):
            edge_x = (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0]
            if x < edge_x:
                inside = not inside
    return inside


def point_segment_distance_squared(
    point: tuple[float, float],
    a: list[float],
    b: list[float],
) -> float:
    x, z = point
    dx = b[0] - a[0]
    dz = b[1] - a[1]
    length_squared = dx * dx + dz * dz
    if length_squared == 0:
        return (x - a[0]) ** 2 + (z - a[1]) ** 2
    along = ((x - a[0]) * dx + (z - a[1]) * dz) / length_squared
    along = max(0.0, min(1.0, along))
    return (x - (a[0] + along * dx)) ** 2 + (z - (a[1] + along * dz)) ** 2


def polygon_distance_squared(point: tuple[float, float], polygon: list[list[float]]) -> float:
    return min(
        point_segment_distance_squared(point, a, b)
        for a, b in zip(polygon, polygon[1:] + polygon[:1])
    )


def prepare_spawn_geometry(city_plan: dict) -> dict:
    buildings = []
    for building in city_plan["buildings"]:
        polygon = building["polygon"]
        xs = [vertex[0] for vertex in polygon]
        zs = [vertex[1] for vertex in polygon]
        buildings.append((min(xs), max(xs), min(zs), max(zs), polygon))
    fixtures = []
    for fixture in city_plan["fixtures"]:
        angle = math.radians(-fixture["angle_deg"])
        fixtures.append((
            fixture["position"][0],
            fixture["position"][1],
            math.cos(angle),
            math.sin(angle),
            fixture["size"][0] * 0.5 + SPAWN_CLEARANCE_M,
            fixture["size"][1] * 0.5 + SPAWN_CLEARANCE_M,
        ))
    return {
        "wall": city_plan["wall_polygon_xz"],
        "buildings": buildings,
        "fixtures": fixtures,
    }


def safe_spawn_candidate(point: tuple[float, float], geometry: dict) -> bool:
    wall = geometry["wall"]
    clearance_squared = SPAWN_CLEARANCE_M * SPAWN_CLEARANCE_M
    if not point_in_polygon(point, wall) or polygon_distance_squared(point, wall) < clearance_squared:
        return False

    x, z = point
    for min_x, max_x, min_z, max_z, polygon in geometry["buildings"]:
        if not (
            min_x - SPAWN_CLEARANCE_M <= x <= max_x + SPAWN_CLEARANCE_M
            and min_z - SPAWN_CLEARANCE_M <= z <= max_z + SPAWN_CLEARANCE_M
        ):
            continue
        if point_in_polygon(point, polygon) or polygon_distance_squared(point, polygon) < clearance_squared:
            return False

    for center_x, center_z, cosine, sine, half_x, half_z in geometry["fixtures"]:
        local_x = (x - center_x) * cosine - (z - center_z) * sine
        local_z = (x - center_x) * sine + (z - center_z) * cosine
        if abs(local_x) < half_x and abs(local_z) < half_z:
            return False
    return True


def planning_ward_for_position(x: float, z: float) -> str:
    """The cadastral map's district rules, with outer fabric assigned by proximity."""
    if -170 <= x <= 130 and z >= 235:
        return "wick"
    if x >= 120 and z >= 175:
        return "cloth"
    if x >= 175 and z >= 35:
        return "wallwright"
    if -280 <= x <= 30 and 110 <= z <= 320:
        return "cinder"
    if -440 <= x <= -170 and -40 <= z <= 240:
        return "weigh"
    if x <= -160 and z <= -235:
        return "reed"
    if z <= -145:
        return "bell_and_sluice"
    if -120 <= x <= 190 and -170 <= z <= 235:
        return "fabric"

    def distance_squared(bounds: tuple[float, float, float, float]) -> float:
        min_x, max_x, min_z, max_z = bounds
        dx = max(min_x - x, 0.0, x - max_x)
        dz = max(min_z - z, 0.0, z - max_z)
        return dx * dx + dz * dz

    return min(
        ALL_WARDS,
        key=lambda ward: (distance_squared(WARD_BOUNDS[ward]), ALL_WARDS.index(ward)),
    )


@dataclass
class SpawnCandidate:
    x: float
    z: float
    ward: str
    minimum_distance_squared: float = math.inf
    unavailable: bool = False


def full_city_spawn_candidates(geometry: dict) -> list[SpawnCandidate]:
    wall = geometry["wall"]
    min_x = math.floor(min(vertex[0] for vertex in wall) / SPAWN_GRID_M)
    max_x = math.ceil(max(vertex[0] for vertex in wall) / SPAWN_GRID_M)
    min_z = math.floor(min(vertex[1] for vertex in wall) / SPAWN_GRID_M)
    max_z = math.ceil(max(vertex[1] for vertex in wall) / SPAWN_GRID_M)
    offset = SPAWN_GRID_M * 0.25
    candidates = []
    for x_index in range(min_x, max_x + 1):
        for z_index in range(min_z, max_z + 1):
            x = x_index * SPAWN_GRID_M + offset
            z = z_index * SPAWN_GRID_M + offset
            if safe_spawn_candidate((x, z), geometry):
                candidates.append(SpawnCandidate(x, z, planning_ward_for_position(x, z)))
    return candidates


def maximum_region_count_with_candidate(
    candidate: tuple[float, float],
    placed: list[tuple[float, float]],
) -> int:
    """Maximum population in any 100 m square that contains `candidate`."""
    x, z = candidate
    relevant = [
        point for point in placed
        if x - REGION_SIZE_M <= point[0] <= x + REGION_SIZE_M
        and z - REGION_SIZE_M <= point[1] <= z + REGION_SIZE_M
    ]
    relevant.append(candidate)
    x_starts = {x - REGION_SIZE_M}
    x_starts.update(
        point[0] for point in relevant if x - REGION_SIZE_M <= point[0] <= x
    )
    maximum = 0
    for start_x in x_starts:
        zs = sorted(
            point[1] for point in relevant
            if start_x <= point[0] <= start_x + REGION_SIZE_M
        )
        z_starts = {z - REGION_SIZE_M}
        z_starts.update(value for value in zs if z - REGION_SIZE_M <= value <= z)
        for start_z in z_starts:
            count = bisect_right(zs, start_z + REGION_SIZE_M) - bisect_left(zs, start_z)
            maximum = max(maximum, count)
    return maximum


def maximum_region_occupancy(points: list[tuple[float, float]]) -> int:
    """Exact maximum for any axis-aligned 100 x 100 m sliding window."""
    by_x = sorted(points)
    maximum = 0
    for left_index, left in enumerate(by_x):
        zs = sorted(
            point[1] for point in by_x[left_index:]
            if point[0] <= left[0] + REGION_SIZE_M
        )
        low = 0
        for high, z in enumerate(zs):
            while zs[low] < z - REGION_SIZE_M:
                low += 1
            maximum = max(maximum, high - low + 1)
    return maximum


def assign_population_spawns(existing: list[dict], new: list[dict], geometry: dict) -> None:
    sheets = existing + new
    fixed = [sheet for sheet in sheets if sheet["significance"] == "major"]
    movable = [sheet for sheet in sheets if sheet["significance"] != "major"]
    assert len(fixed) == 30 and len(movable) == 470
    placed = [
        (sheet["spawn_location"]["x"], sheet["spawn_location"]["z"])
        for sheet in fixed
    ]
    radius_squared = NEARBY_RADIUS_M * NEARBY_RADIUS_M
    nearby_counts = [
        sum(
            (left[0] - right[0]) ** 2 + (left[1] - right[1]) ** 2 <= radius_squared
            for other_index, right in enumerate(placed)
            if index != other_index
        )
        for index, left in enumerate(placed)
    ]
    assert max(nearby_counts) <= MAX_NEARBY_NPCS - 1
    required_by_ward = Counter(sheet["planning_ward"] for sheet in movable)
    candidates = full_city_spawn_candidates(geometry)
    for candidate in candidates:
        candidate.minimum_distance_squared = min(
            (candidate.x - x) ** 2 + (candidate.z - z) ** 2 for x, z in placed
        )
    selected_by_ward = defaultdict(list)
    while sum(len(points) for points in selected_by_ward.values()) < len(movable):
        eligible = [
            index for index, candidate in enumerate(candidates)
            if not candidate.unavailable
        ]
        assert eligible
        chosen_index = max(
            eligible,
            key=lambda index: (candidates[index].minimum_distance_squared, -index),
        )
        chosen = candidates[chosen_index]
        point = (chosen.x, chosen.z)
        nearby = [
            index for index, other in enumerate(placed)
            if (point[0] - other[0]) ** 2 + (point[1] - other[1]) ** 2 <= radius_squared
        ]
        if len(nearby) >= MAX_NEARBY_NPCS or any(
            nearby_counts[index] >= MAX_NEARBY_NPCS - 1 for index in nearby
        ):
            chosen.unavailable = True
            continue
        if maximum_region_count_with_candidate(point, placed) > MAX_NPCS_PER_REGION:
            chosen.unavailable = True
            continue

        chosen.unavailable = True
        for index in nearby:
            nearby_counts[index] += 1
        placed.append(point)
        nearby_counts.append(len(nearby))
        selected_by_ward[chosen.ward].append(point)
        for candidate in candidates:
            if candidate.unavailable:
                continue
            distance_squared = (candidate.x - chosen.x) ** 2 + (candidate.z - chosen.z) ** 2
            candidate.minimum_distance_squared = min(
                candidate.minimum_distance_squared,
                distance_squared,
            )

    assert Counter({ward: len(points) for ward, points in selected_by_ward.items()}) == required_by_ward

    for ward in ALL_WARDS:
        ward_sheets = sorted(
            (sheet for sheet in movable if sheet["planning_ward"] == ward),
            key=lambda sheet: sheet["id"],
        )
        positions = sorted(selected_by_ward[ward])
        assert len(ward_sheets) == len(positions)
        for sheet, (x, z) in zip(ward_sheets, positions, strict=True):
            facing_seed = sum(
                (index + 1) * ord(character)
                for index, character in enumerate(sheet["id"])
            )
            sheet["spawn_location"] = {
                "x": round(x, 2),
                "y": 0.91,
                "z": round(z, 2),
                "facing": round(((facing_seed * 0.731) % math.tau) - math.pi, 4),
            }


def material_sentence(draft: Draft) -> str:
    statuses = set(draft.statuses)
    if "prisoner" in statuses:
        return "Stone House rations and food carried in by kin are your present support."
    if "dependent" in statuses:
        return "You eat from a shared household pot and earn only an occasional penny."
    if "begs_regularly" in statuses:
        return "Irregular work, doorstep alms and a regular begging pitch keep you fed."
    if "pauper" in statuses:
        return "Irregular work and neighbours' small alms barely keep you fed."
    if "intermittently_employed" in statuses:
        return "Day wages and a place in the hiring line keep your household going."
    return "Your wages, barter and place in the shared household pot keep you fed."


def description(draft: Draft, index: int) -> str:
    district = WARD_LABELS[draft.ward]
    role = "person with no fixed trade" if draft.title is None else draft.title.lower()
    article = "an" if role[0] in "aeiou" else "a"
    if draft.title and "prisoner" in draft.statuses:
        action = f"You worked as {article} {role} and are now held from {district}, still tied to {draft.cohort}."
    elif draft.title and "unemployed" in draft.statuses:
        action = f"You are an out-of-work {role} in {district}, relying on {draft.cohort} while seeking hire."
    elif draft.title and "retired" in draft.statuses:
        action = f"You are a retired {role} in {district}, still part of {draft.cohort}."
    elif draft.title:
        action = f"You are {article} {role} in {district}, working beside {draft.cohort}."
    else:
        action = f"You have no fixed trade and pass your days in {district} with {draft.cohort}."
    sentences = [action, material_sentence(draft), f"{draft.speech}.",
                 f"Today you need to {draft.concern}.", f"{draft.visible}."]
    if draft.conditions:
        if draft.conditions[0] == "pregnant":
            sentences.append("You are visibly pregnant and ration your strength through the day.")
        elif draft.conditions[0] == "nursing an infant":
            sentences.append("You are nursing an infant and keep listening for the child's cry.")
        else:
            sentences.append(f"The visible bodily condition you live with is {draft.conditions[0]}.")
    if draft.illegal_activity:
        sentences.append(f"You quietly supplement that living through {draft.illegal_activity}.")
    if "recent_migrant" in draft.statuses:
        sentences.append("You arrived recently and still measure local speech before trusting it.")
    if draft.significance == "minor":
        if draft.family in MINOR_ROLE_DETAILS:
            sentences.append(MINOR_ROLE_DETAILS[draft.family])
        sentences.extend([
            "People in this ward return to you because you remember agreements and faces.",
            "You mean to keep your place without pretending that every local quarrel concerns the Great Rose.",
        ])
    text = " ".join(sentences)
    words = len(text.split())
    if draft.significance == "ambient":
        assert 40 <= words <= 105, (draft.id, words, text)
    else:
        assert 75 <= words <= 180, (draft.id, words, text)
    assert not re.search(r"\b(?:major|minor|ambient)\b", text, re.IGNORECASE)
    return text


def make_new_sheets(drafts: list[Draft]) -> list[dict]:
    sheets = []
    for index, draft in enumerate(drafts):
        if draft.age < 8:
            child_concerns = [
                "find the familiar adult you lost sight of", "keep your bread from a bold pigeon",
                "recover a dropped wooden button", "get your blanket back before dark",
                "find where the older children hid your cup", "carry a small bundle without dropping it",
            ]
            draft.concern = child_concerns[(index + draft.age) % len(child_concerns)]
            draft.speech = "You use short, earnest sentences and stay close to familiar adults"
        elif draft.age < 12:
            draft.concern = CONCERNS[(index * 7 + draft.age) % len(CONCERNS)]
            draft.speech = "You answer in quick, literal phrases and grow wary when adults crowd you"
        else:
            draft.concern = CONCERNS[(index * 7 + draft.age) % len(CONCERNS)]
            draft.speech = SPEECH_MANNERS[(index * 5 + draft.age) % len(SPEECH_MANNERS)]
        if draft.significance == "minor" and draft.family in MINOR_ROLE_CONCERNS:
            draft.concern = MINOR_ROLE_CONCERNS[draft.family]
        draft.visible = VISIBLE_FACTS[(index * 11 + draft.local_index) % len(VISIBLE_FACTS)]
        sheet = {
            "id": draft.id, "name": draft.name, "significance": draft.significance,
            "planning_ward": draft.ward, "age": draft.age, "gender": draft.gender,
            "occupation_id": draft.occupation_id, "title": draft.title, "rank": draft.rank,
            "faction_role": None, "illegal_activity": draft.illegal_activity,
            "district": WARD_LABELS[draft.ward], "knows": draft.knows,
            "father": draft.father, "mother": draft.mother, "children": draft.children,
            "spawn_location": {
                "x": 0.0, "y": 0.91, "z": 0.0,
                "facing": round(((index * 0.731) % math.tau) - math.pi, 4),
            },
            "statuses": draft.statuses, "conditions": draft.conditions, "memories": [],
            "core_character_description": description(draft, index),
            "extended_character_description": "", "goal": draft.concern.capitalize(),
        }
        sheets.append(ordered_sheet(sheet))
    return sheets


def slug(name: str) -> str:
    value = re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")
    return value or "unnamed"


def validate(
    existing: list[dict],
    new: list[dict],
    catalog: dict[str, dict],
    geometry: dict,
) -> None:
    sheets = existing + new
    assert len(sheets) == 500
    assert len({sheet["id"] for sheet in sheets}) == 500
    assert all(len(sheet["id"]) == 5 for sheet in sheets)
    assert len({sheet["name"] for sheet in sheets}) == 500
    assert Counter(sheet["significance"] for sheet in sheets) == {
        "major": 30, "minor": 120, "ambient": 350,
    }
    assert Counter(sheet["gender"] for sheet in sheets) == {"m": 250, "f": 250}
    age_bins = Counter(
        "0-7" if sheet["age"] <= 7 else "8-11" if sheet["age"] <= 11
        else "12-15" if sheet["age"] <= 15 else "16-19" if sheet["age"] <= 19
        else "20-39" if sheet["age"] <= 39 else "40-59" if sheet["age"] <= 59 else "60+"
        for sheet in sheets
    )
    assert age_bins == {"0-7": 25, "8-11": 25, "12-15": 25, "16-19": 55,
                        "20-39": 180, "40-59": 125, "60+": 65}
    assert Counter(sheet["planning_ward"] for sheet in new) == WARD_TARGETS_NEW
    assert Counter(sheet["planning_ward"] for sheet in sheets) == {
        "fabric": 42, "wick": 40, "cloth": 43, "wallwright": 31,
        "cinder": 37, "weigh": 74, "reed": 64, "bell_and_sluice": 169,
    }

    positions = [
        (sheet["spawn_location"]["x"], sheet["spawn_location"]["z"])
        for sheet in sheets
    ]
    movable_positions = [
        (sheet["spawn_location"]["x"], sheet["spawn_location"]["z"])
        for sheet in sheets if sheet["significance"] != "major"
    ]
    assert all(safe_spawn_candidate(position, geometry) for position in movable_positions)
    for sheet in sheets:
        if sheet["significance"] == "major":
            continue
        location = sheet["spawn_location"]
        assert planning_ward_for_position(location["x"], location["z"]) == sheet["planning_ward"]
    for index, left in enumerate(positions):
        for right in positions[index + 1:]:
            assert left != right
    neighbor_counts = [
        sum(
            (left[0] - right[0]) ** 2 + (left[1] - right[1]) ** 2
            <= NEARBY_RADIUS_M**2
            for right in positions
        )
        for left in positions
    ]
    assert max(neighbor_counts) <= MAX_NEARBY_NPCS, max(neighbor_counts)
    assert maximum_region_occupancy(positions) <= MAX_NPCS_PER_REGION

    wall = geometry["wall"]
    city_x_span = max(vertex[0] for vertex in wall) - min(vertex[0] for vertex in wall)
    city_z_span = max(vertex[1] for vertex in wall) - min(vertex[1] for vertex in wall)
    occupied_x_span = max(point[0] for point in positions) - min(point[0] for point in positions)
    occupied_z_span = max(point[1] for point in positions) - min(point[1] for point in positions)
    assert occupied_x_span >= city_x_span * 0.80, occupied_x_span
    assert occupied_z_span >= city_z_span * 0.80, occupied_z_span
    safe_cells = {
        (math.floor(candidate.x / REGION_SIZE_M), math.floor(candidate.z / REGION_SIZE_M))
        for candidate in full_city_spawn_candidates(geometry)
    }
    occupied_cells = {
        (math.floor(x / REGION_SIZE_M), math.floor(z / REGION_SIZE_M))
        for x, z in positions
    }
    assert len(safe_cells - occupied_cells) <= 1, safe_cells - occupied_cells

    assert sum("pauper" in sheet["statuses"] for sheet in sheets) == 100
    assert sum("begs_regularly" in sheet["statuses"] for sheet in sheets) == 38
    assert sum(bool({"unhoused", "insecure_lodging"} & set(sheet["statuses"])) for sheet in sheets) == 32
    assert sum("intermittently_employed" in sheet["statuses"] for sheet in sheets) == 60
    conventional_existing = sum(
        sheet["illegal_activity"] is not None and "heresy" not in sheet["illegal_activity"]
        for sheet in existing
    )
    assert conventional_existing == 8
    assert sum(sheet["illegal_activity"] is not None for sheet in new) == 42
    assert conventional_existing + 42 == 50
    assert sum(sheet.get("rank") in {"master", "mistress", "warden"} for sheet in sheets) == 55

    original_counts = Counter(sheet["occupation_id"] for sheet in existing)
    final_counts = Counter(sheet["occupation_id"] for sheet in sheets)
    for family, count in EXISTING_ADDITIONS.items():
        assert final_counts[family] - original_counts[family] == count
    for family, count in NEW_FAMILY_ADDITIONS.items():
        assert original_counts[family] == 0 and final_counts[family] == count
    assert final_counts[None] == 10
    ids = {sheet["id"] for sheet in sheets}
    significance = {sheet["id"]: sheet["significance"] for sheet in sheets}
    for sheet in sheets:
        assert set(sheet["knows"]) <= ids
        if sheet["significance"] != "ambient":
            assert all(significance[known] != "ambient" for known in sheet["knows"])
        if sheet["father"]:
            assert sheet["id"] in next(item for item in sheets if item["id"] == sheet["father"])["children"]
        if sheet["mother"]:
            assert sheet["id"] in next(item for item in sheets if item["id"] == sheet["mother"])["children"]
        if sheet["occupation_id"] is None:
            assert sheet["title"] is None and sheet["rank"] is None
        else:
            assert sheet["title"] in catalog[sheet["occupation_id"]]["alternative_titles"]

    ambient = [sheet for sheet in sheets if sheet["significance"] == "ambient"]
    for sheet in ambient:
        allowed_titles = AMBIENT_INSTITUTION_TITLES.get(sheet["occupation_id"])
        if allowed_titles is not None:
            assert sheet["title"] in allowed_titles
    for family in AMBIENT_INSTITUTION_TITLES:
        anchors = [
            sheet for sheet in sheets
            if sheet["occupation_id"] == family and sheet["title"] == MINOR_TITLES[family]
        ]
        assert len(anchors) == 1 and anchors[0]["significance"] == "minor"

    stable_text = "\n".join(
        sheet["core_character_description"] + "\n" + sheet["extended_character_description"]
        for sheet in sheets if sheet["significance"] != "ambient"
    )
    canon_text_parts = []
    for directory in [ROOT / "lore/core_lore", ROOT / "lore/second_sun", ROOT / "features"]:
        for path in directory.rglob("*.md"):
            if "wip_lore_please_ignore_this_is_NOT_canon" not in str(path):
                canon_text_parts.append(path.read_text())
    canon_text = "\n".join(canon_text_parts)
    for sheet in ambient:
        assert sheet["id"] not in stable_text and sheet["name"] not in stable_text
        assert sheet["id"] not in canon_text and sheet["name"] not in canon_text, (
            sheet["id"], sheet["name"]
        )


def write_new(sheets: list[dict]) -> None:
    for sheet in sheets:
        folder = sheet["occupation_id"] or "no_fixed_trade"
        path = CHARACTERS / folder / f"{sheet['id']}_{slug(sheet['name'])}.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(sheet, indent=2, ensure_ascii=False) + "\n")


def write_existing(sheets: list[dict]) -> None:
    paths = {}
    for path in CHARACTERS.rglob("*.json"):
        raw = json.loads(path.read_text())
        if not re.fullmatch(r"p[0-9a-z]{4}", raw["id"]):
            paths[raw["id"]] = path
    assert set(paths) == {sheet["id"] for sheet in sheets}
    for sheet in sheets:
        paths[sheet["id"]].write_text(json.dumps(sheet, indent=2, ensure_ascii=False) + "\n")


def main() -> None:
    occupations = json.loads(OCCUPATIONS_PATH.read_text())
    city_plan = json.loads(CITY_PLAN_PATH.read_text())
    geometry = prepare_spawn_geometry(city_plan)
    catalog = {entry["occupation_id"]: entry for entry in occupations}
    assert len(catalog) == 65
    assert set(NEW_FAMILY_ADDITIONS) <= set(catalog)
    existing = migrate_existing()
    drafts = make_drafts()
    assign_ages(drafts)
    assign_wards(drafts)
    assign_ranks(drafts)
    assign_statuses_and_conditions(drafts)
    assign_crime(drafts)
    assign_titles(drafts, catalog)
    assign_names_and_relationships(drafts)
    new = make_new_sheets(drafts)
    assign_population_spawns(existing, new, geometry)
    validate(existing, new, catalog, geometry)
    write_existing(existing)
    write_new(new)
    print("wrote 397 new sheets; validated 500 people (30 major / 120 minor / 350 ambient)")


if __name__ == "__main__":
    main()
