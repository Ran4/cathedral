#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["httpx", "openai", "python-dotenv"]
# ///
"""Generate lore-grounded concept art for Ombreval's canonical places.

The catalog below is distilled from ``lore/core_lore``. Each place gets an
``image_generation_prompt.md`` containing the exact prompt sent to the API and
one or more PNG files. Existing PNGs are kept, so an interrupted run can be
resumed without regenerating completed images. Delete an unwanted image and
rerun the script, or pass ``--force`` to replace every selected image.

``OPENAI_API_KEY`` may be exported by the shell or stored in the repo-root
``.env`` file. The image model is deliberately fixed to ``gpt-image-2``.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import binascii
import os
import sys
from dataclasses import dataclass
from html import escape
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from openai import AsyncOpenAI

REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_ROOT = REPO_ROOT / "lore" / "inspiration_images" / "places"
SHOWCASE_PATH = OUTPUT_ROOT.parent / "showcase_inspirational_images.html"
MODEL = "gpt-image-2"
DEFAULT_SIZE = "1536x1024"
DEFAULT_QUALITY = "medium"
DRY_CITY_MATERIALS = (
    "dry worn limestone and fieldstone, dusty lime plaster, exposed oak framing, "
    "rough cloth, rope, iron, cart-worn timber, terracotta and slate roofs, packed "
    "earth, dry paving, soot, smoke, and visible hand construction"
)
DRY_CUT_RULE = (
    "show present-day F.437, after the Cut was filled and the Serle diverted beyond "
    "the south wall; there is no open river or canal inside the city walls. "
)


@dataclass(frozen=True)
class Variant:
    """One distinct view of a place."""

    scene: str
    composition: str
    lighting: str
    constraints: str = ""
    materials: str = ""


@dataclass(frozen=True)
class Place:
    """A canonical place and the image views requested for it."""

    slug: str
    name: str
    lore: str
    variants: tuple[Variant, ...]


PLACES: tuple[Place, ...] = (
    Place(
        "the_lanthorn",
        "The Lanthorn",
        (
            "The Great Church of Saint Ambrelle, Ombreval's immense cathedral: "
            "a nine-bay aisled nave, crossing, transepts, quire, triforium, "
            "clerestory walks, chapels, towers, chapter house, and generations "
            "of visible building work. Its west front holds the Great Rose; the "
            "unfinished crown of the north tower still carries scaffolding."
        ),
        (
            Variant(
                scene=(
                    "Exterior from the stepped Gradine toward the monumental west "
                    "front. Show the Great Rose as dark glass and tracery from "
                    "outside, working scaffolds on the north-tower crown, masons, "
                    "pilgrims, vergers, carts, and routine maintenance."
                ),
                composition=(
                    "street-level wide establishing view, human figures small "
                    "against the vertical facade, dense city edges framing the "
                    "forecourt"
                ),
                lighting="clear cool morning with one ordinary sun",
                constraints=(
                    "outside the Lanthorn there is exactly one ordinary sun; the "
                    "Great Rose is not glowing and the impossible light is not "
                    "visible"
                ),
            ),
            Variant(
                scene=(
                    "Interior view from the crossing looking west through the nave "
                    "toward the Great Rose. Through the Rose alone, the sky contains "
                    "a cold green-white second sunlike disc; its beams cast faint "
                    "second grave-shadows at an impossible angle beside ordinary "
                    "sun-shadows. Show ordinary cathedral work continuing below."
                ),
                composition=(
                    "grand axial interior, low human eye level, long nave depth, "
                    "the Great Rose as the distant focal point, figures and work "
                    "platforms establishing scale"
                ),
                lighting=(
                    "late-afternoon true sunlight overlapped by cold green-white "
                    "Rose-light; the two kinds of light remain visually distinct"
                ),
                constraints=(
                    "the second disc is visible only through the Great Rose; it "
                    "does not appear in other windows, reflections, or open sky; "
                    "no magical particles, heat, voices, or physical effects"
                ),
            ),
        ),
    ),
    Place(
        "the_wickmarket",
        "The Wickmarket",
        (
            "Ombreval's western chandlers' square: wax, tallow, wicks, honey, "
            "hot fat, candle stalls, public lamps, and the lamplighters' trade, "
            "enclosed by irregular jettied houses and narrow back lanes."
        ),
        (
            Variant(
                scene=(
                    "Highmarket at full business: chandlers dipping candles, blocks "
                    "of wax and tallow, coiled wick, honey pots, smoking cauldrons, "
                    "buyers, apprentices, handcarts, and a lawful lamplighter's ladder."
                ),
                composition=(
                    "busy street-level square seen from one pinched entrance, with "
                    "layered stalls and crooked facades creating deep spatial density"
                ),
                lighting="bright overcast market morning, warm flames against cool stone",
            ),
            Variant(
                scene=(
                    "The market closing at Lamplight after a brief rain: shutters, "
                    "wax-streaked boards, workers lifting awnings, lamps being lit, "
                    "puddles and cart ruts, with one public lamp deliberately left dark."
                ),
                composition=(
                    "lower, more intimate view along the square's uneven edge, "
                    "foreground work details leading toward crowded rooflines"
                ),
                lighting="blue-gray dusk with restrained amber candle and lamp light",
            ),
        ),
    ),
    Place(
        "coswalds_yard",
        "Coswald's Yard",
        (
            "The northern builders' square: dressed and rough stone, timber, lime, "
            "scaffolding, hoists, tracing floors, the masons' lodge, skilled hand "
            "labour, guild pride, and old grievances connected to the Lanthorn's Fabric."
        ),
        (
            Variant(
                scene=(
                    "A working builders' yard crowded with stone blocks, timber racks, "
                    "lime mixing, wooden cranes, templates, chisels, lodge benches, "
                    "apprentices, and masons debating beside an unfinished carving."
                ),
                composition=(
                    "wide three-quarter view from ground level, foreground tools and "
                    "stone chips, scaffold geometry rising behind the lodge"
                ),
                lighting="pale autumn afternoon through limestone dust",
            ),
        ),
    ),
    Place(
        "the_tallage",
        "The Tallage",
        (
            "The customs square on the dry Cut: toll-house, weigh-beams, chained "
            "standards, the Tallage stone, sealed Gaudry weights, pawnshops, clerks, "
            "freight brought by cart and porter from wharves outside the south wall, "
            "careful valuation, and the two-arched overhead Tally Bridge."
        ),
        (
            Variant(
                scene=(
                    "Serle freight arriving by ox cart and porter along the dry Cut: "
                    "clerks at boards, porters at the weigh-beam, chained brass weights "
                    "and stone standards, merchants waiting beside packed bales and salt "
                    "casks, pawnshop fronts, and the overhead Tally Bridge joining the "
                    "toll-house to its bonded warehouse."
                ),
                composition=(
                    "broad street-level establishing view along the dry Cut, with a "
                    "strong diagonal from foreground cart cargo through the weigh station "
                    "to the two-arched overhead bridge"
                ),
                lighting="cool Lowmarket morning with cart dust and muted winter color",
                constraints=(
                    DRY_CUT_RULE
                    + "No visible water, canal, barge, quay, riverbank, mist, reflections, "
                    "or toll-chain; all present-day freight arrives overland by cart and porter"
                ),
                materials=DRY_CITY_MATERIALS,
            ),
        ),
    ),
    Place(
        "marens_green",
        "Maren's Green",
        (
            "The south-eastern fish and boat-families' square on the dry former Cut: "
            "eel smoke, fish carried in from the outer wharves, the Alder Moorings "
            "warehouse yard, the Eel gallery bridge, and the Church of Saint Maren."
        ),
        (
            Variant(
                scene=(
                    "Lowmarket on the dry former Cut: fish slabs, eel-smoking racks, "
                    "baskets and crates brought in by handcart from wharves beyond the "
                    "south wall, boat-families trading and sorting freight, the open gate "
                    "of the Alder Moorings warehouse yard, a laden cart squeezing past "
                    "the Eel gallery bridge's narrow stair, and Saint Maren's church beyond."
                ),
                composition=(
                    "lively eye-level market-street panorama, with the straight dry Cut "
                    "carrying the eye from fish stalls toward the gallery bridge, warehouse "
                    "court, and church"
                ),
                lighting="dry late-summer morning with warm sunlight through eel smoke",
                constraints=(
                    DRY_CUT_RULE
                    + "No visible water, canal, barge, quay, shoreline, mooring posts, "
                    "wet nets, river reflections, or waterfront vista"
                ),
                materials=DRY_CITY_MATERIALS,
            ),
        ),
    ),
    Place(
        "the_bellstand",
        "The Bellstand",
        (
            "The eastern proclamation square behind the Lanthorn's east end, beneath "
            "an old secular watch-bell tower. The unhallowed Scold rings curfew and "
            "summons crowds while edicts are cried and posted."
        ),
        (
            Variant(
                scene=(
                    "A proclamation drawing a mixed city crowd beneath the tall, severe "
                    "watch-bell tower: crier on a worn platform, watch officers, posted "
                    "sheets, people listening from windows and passage mouths, and the "
                    "large secular bell visible overhead."
                ),
                composition=(
                    "low-angle square view that makes the tower and hanging Scold the "
                    "focal point while preserving readable crowd-scale activity"
                ),
                lighting="hard eastern afternoon light with long architectural shadows",
                constraints="posted notices contain no readable words or invented lettering",
            ),
        ),
    ),
    Place(
        "the_gradine",
        "The Gradine",
        (
            "The Lanthorn's shallow stepped ceremonial forecourt, counted apart from "
            "the five squares: processions, petitions, public recantations, licensed "
            "pilgrim sellers, feast crowds, and civic spectacle beneath the west front."
        ),
        (
            Variant(
                scene=(
                    "The steps prepared for an Ambrellestide procession: glassworkers "
                    "carrying wrapped panes from Cinder Row, vergers managing the route, "
                    "licensed sellers, petitioners, guild banners without legible words, "
                    "and townspeople packed into a forecourt too small for the whole city."
                ),
                composition=(
                    "wide view across shallow tiers toward the looming west doors and "
                    "Great Rose, with processional movement cutting across the frame"
                ),
                lighting="clear early-summer daylight with one ordinary sun",
                constraints=(
                    "from outside the Great Rose is only dark glass and tracery; no "
                    "second sun or green light is visible"
                ),
            ),
        ),
    ),
    Place(
        "the_cut",
        "The Cut",
        (
            "The filled former urban reach of the Serle, now an unusually straight "
            "trade street and district across Ombreval's southern third. The river was "
            "diverted outside the south wall in F.363–369, while its bridges and working "
            "river names survived in dry ground."
        ),
        (
            Variant(
                scene=(
                    "A broad, unusually straight trade street cutting through otherwise "
                    "crooked dense quarters: freight carts and porters, warehouse doors, "
                    "hoists, awnings, handbarrows, reused old masonry, and overhead bridges "
                    "compressed into the distance. The former channel is completely filled "
                    "and its dry paving is worn by generations of cart traffic."
                ),
                composition=(
                    "long low street-level perspective, the straight dry roadway forming "
                    "a striking central vanishing corridor beneath crowded irregular rooflines"
                ),
                lighting="soft overcast daylight with dusty cart ruts and deep shop shadows",
                constraints=(
                    DRY_CUT_RULE
                    + "No visible water, canal, boats, barges, quays, riverbanks, retaining "
                    "walls beside water, wet paving, reflections, or water-wall openings"
                ),
                materials=DRY_CITY_MATERIALS,
            ),
        ),
    ),
    Place(
        "the_tally_bridge",
        "The Tally Bridge",
        (
            "A two-arched stone overhead bridge across the dry Cut at the Tallage, first "
            "built for the former channel and rebuilt in stone in F.247. It now joins the "
            "toll-house to its bonded warehouse above the trade street."
        ),
        (
            Variant(
                scene=(
                    "The twin stone arches spanning the busy dry Cut between the toll-house "
                    "and bonded warehouse: laden salt carts and porters pass through below, "
                    "clerks cross within the overhead bridge, and Tallage customs buildings "
                    "crowd both sides of the street."
                ),
                composition=(
                    "low street-level three-quarter architectural view emphasizing the two "
                    "arches overhead, worn masonry, connected buildings, and congested dry "
                    "roadway beneath"
                ),
                lighting="slanting morning light through pale street dust",
                constraints=(
                    DRY_CUT_RULE
                    + "No visible water, canal, barge, boat, riverbank, mist, reflection, "
                    "or toll-chain; carts and porters occupy the former channel"
                ),
                materials=DRY_CITY_MATERIALS,
            ),
        ),
    ),
    Place(
        "the_eel_bridge",
        "The Eel Bridge",
        (
            "A low timber gallery bridge above the dry fish-market lane at Maren's "
            "Green. Its narrow stair constricts the lane, so laden carts make "
            "pedestrians wait."
        ),
        (
            Variant(
                scene=(
                    "A low enclosed timber gallery joining upper storeys above the crowded "
                    "fish-market lane. A cart piled with eel baskets squeezes past the "
                    "gallery's narrow stair while pedestrians wait on its small landing; "
                    "vendors, boat-family porters, and dry market goods fill Maren's Green."
                ),
                composition=(
                    "close human-scale street view, timber gallery crossing the frame above "
                    "the cart, with waiting faces, stair geometry, and joinery clearly visible"
                ),
                lighting="dry gray afternoon with soft light on weathered timber",
                constraints=(
                    DRY_CUT_RULE
                    + "This is a gallery bridge over a street, not a footbridge over water. "
                    "No visible water, canal, barge, boat, mooring posts, wet planks, "
                    "riverbank, or reflections"
                ),
                materials=DRY_CITY_MATERIALS,
            ),
        ),
    ),
    Place(
        "the_sluice",
        "The Old Sluice",
        (
            "The dry stone gatehouse at the east end of the former Cut, where Colm "
            "Attergate drowned while the channel still ran. The river was diverted "
            "outside the south wall in F.363–369, but this historic landmark kept its name."
        ),
        (
            Variant(
                scene=(
                    "The dry old gatehouse closing the eastern end of the filled Cut: heavy "
                    "weathered masonry, blocked former gate apertures, grooves from removed "
                    "mechanisms, remnants of iron lifting gear, pedestrians and a handcart "
                    "using the adjoining dry street, and dense buildings abutting the structure."
                ),
                composition=(
                    "dramatic low street angle toward the dry historic gatehouse, people "
                    "providing scale without turning it into a fantasy fortress"
                ),
                lighting="dry cold late-autumn daylight catching worn stone and iron",
                constraints=(
                    DRY_CUT_RULE
                    + "No visible water, current, canal, river, spray, rain, flood debris, "
                    "wet stone, reflections, active sluice gates, or opening to a waterfront"
                ),
                materials=DRY_CITY_MATERIALS,
            ),
        ),
    ),
    Place(
        "the_alder_moorings",
        "The Alder Moorings",
        (
            "The Alder boat-family's warehouse court at Maren's Green. The river left "
            "the city when the Cut was filled, but the dry storage and transfer yard "
            "kept the inherited name Moorings."
        ),
        (
            Variant(
                scene=(
                    "An enclosed working warehouse court entered through a broad arched gate: "
                    "Alder family members sort fish baskets and freight bales, repair ropes, "
                    "poles and patched canvas from their outer-wharf work, load handcarts, "
                    "keep ledgers at a rough table, and store gear beneath timber galleries."
                ),
                composition=(
                    "intimate eye-level view through the gate into a deep dry courtyard, "
                    "layering family life, storage, and freight work beneath warehouse facades"
                ),
                lighting="quiet dry dawn light beginning on canvas, rope, and dusty paving",
                constraints=(
                    DRY_CUT_RULE
                    + "This is a warehouse yard whose old name survived, not active moorings. "
                    "No visible water, canal, boats, barges, quay, riverbank, mooring posts, "
                    "marina, wet paving, or reflections"
                ),
                materials=DRY_CITY_MATERIALS,
            ),
        ),
    ),
    Place(
        "cinder_row",
        "Cinder Row",
        (
            "The glaziers' street, rebuilt after the fire of F.171 with stone lower "
            "storeys and timber above: furnaces, glassmaking, leading, guild workshops, "
            "old burn memory, and the annual Ambrellestide glass procession."
        ),
        (
            Variant(
                scene=(
                    "A narrow active workshop street with furnace glow behind open shutters, "
                    "glaziers carrying colored panes, apprentices working lead cames, crates "
                    "of sand and fuel, stone shopfronts under rebuilt timber upper floors, "
                    "and practical fire precautions."
                ),
                composition=(
                    "street-level view down the crooked row, hot workshop interiors punctuating "
                    "a cool exterior corridor, overhead jetties compressing the sky"
                ),
                lighting="overcast day contrasted with restrained orange furnace light",
                constraints="colored glass is material craft, not magical glowing crystal",
            ),
        ),
    ),
    Place(
        "the_needle",
        "The Needle",
        (
            "Ombreval's narrowest pinch, made shoulder-wide by encroaching rebuilt houses, "
            "running from the Wickmarket back lanes toward the Tallage. The saying 'past "
            "the Needle' means beyond saving."
        ),
        (
            Variant(
                scene=(
                    "A single porter turning sideways to squeeze through the shoulder-wide "
                    "lane between bowed plaster-and-timber walls, with scraped corners, "
                    "dripping eaves, tiny doors, worn paving, and a brighter market glimpse "
                    "at the far end."
                ),
                composition=(
                    "claustrophobic first-person eye-level passage view, walls nearly filling "
                    "the frame and converging toward a narrow exit"
                ),
                lighting="thin shaft of midday light from far above, cool reflected shadow below",
            ),
        ),
    ),
    Place(
        "the_drapers_reach",
        "The Draper's Reach",
        (
            "The roofed gallery of cloth halls between Coswald's Yard and the Wickmarket, "
            "sheltering wool and finished cloth. Ombreval's standard iron ell is fixed "
            "there for public measurement."
        ),
        (
            Variant(
                scene=(
                    "A long covered cloth gallery during business: bolts of wool cloth, "
                    "merchants measuring against the fixed iron ell, porters, tally boards, "
                    "timber roof trusses, shutters, and rain kept just beyond the arcade."
                ),
                composition=(
                    "strong one-point perspective through the roofed hall, repeating posts "
                    "and fabric bolts leading toward a crowded opening"
                ),
                lighting="soft rainy daylight entering from the sides, rich subdued textile color",
                constraints="no readable modern labels, numerals, or invented signage",
            ),
        ),
    ),
    Place(
        "gaunt_passage",
        "Gaunt Passage",
        (
            "A blind covered passage beneath the old Gaunt house's salt-cellars near "
            "the Tallage. The salt-factor family is extinct, but its name persists in "
            "this dim, practical dead end."
        ),
        (
            Variant(
                scene=(
                    "A dim vaulted passage ending blindly below bulging salt-cellar walls, "
                    "with salt bloom on masonry, leaking sacks overhead, a handcart that "
                    "must be backed out, old doorways, and a narrow slice of Tallage daylight."
                ),
                composition=(
                    "view from inside the dead end toward its only entrance, low ceiling "
                    "and close walls emphasizing awkward working geometry"
                ),
                lighting="cold entrance light fading into warm lantern shadow",
            ),
        ),
    ),
    Place(
        "bellfoot_passage",
        "Bellfoot Passage",
        (
            "The covered way directly beneath the old Bellstand watch-tower stair, shaped "
            "by heavy civic masonry, foot traffic, the tower's secular bell, and the "
            "proclamation square immediately beyond."
        ),
        (
            Variant(
                scene=(
                    "People passing under a steep external tower stair and thick timber "
                    "bracing, with worn civic stonework, a watchman descending, posted but "
                    "unreadable notices, and the brighter Bellstand square at the exit."
                ),
                composition=(
                    "compressed view through the arch, stair diagonals overhead, bright "
                    "square beyond framed as the destination"
                ),
                lighting="late-afternoon light spilling under the tower into deep cool shade",
                constraints="no readable words on notices",
            ),
        ),
    ),
    Place(
        "tanners_slip",
        "Tanners' Slip",
        (
            "A working lane behind Maren's Green, associated with tannery workshops, "
            "hides, bark tannin, closed barrels, hard labour, sharp smells, patched "
            "firebreak masonry, and the remembered tannery fire of F.157."
        ),
        (
            Variant(
                scene=(
                    "A cramped, completely dry service lane where workers in stained aprons "
                    "scrape, trim, stretch, and cure hides at rough benches and timber frames. "
                    "Show bark chips, sacks of ground tannin, folded hides, closed barrels, "
                    "empty covered vats, patched firebreak masonry, and projecting upper floors. "
                    "The lane ends in an enclosed dogleg between workshops."
                ),
                composition=(
                    "ground-level oblique view following dry, worn stone paving toward the "
                    "enclosed dogleg, with work surfaces and projecting upper floors closing "
                    "in on both sides; preserve the strong depth and dense workshop character"
                ),
                lighting=(
                    "dry diffuse morning light with dusty air, restrained earthy color, and "
                    "soft light at the lane's enclosed far end"
                ),
                constraints=(
                    "historically grounded labor, not horror imagery or graphic gore. No "
                    "visible water or liquid anywhere: no river, canal, shoreline, harbor, "
                    "boats, bridge, moorings, stream, drain, gutter, runoff, puddles, rain, "
                    "wet paving, reflections, or liquid in vats, tubs, buckets, or barrels. "
                    "Do not make the lane descend toward water or reveal a waterfront vista"
                ),
                materials=(
                    "dry worn limestone and fieldstone, dusty lime plaster, exposed oak "
                    "framing, dry bark and bark powder, rough cloth sacks, iron hand tools, "
                    "closed wooden casks, empty covered vats, stretched and folded hides, "
                    "terracotta roofs, straw, soot, and visible hand construction"
                ),
            ),
        ),
    ),
    Place(
        "skinners_court",
        "Skinners' Court",
        (
            "A small enclosed working court north of the Gradine, occupied by ordinary "
            "dense-city trade and domestic life, and associated with a persistent but "
            "unproven local tale about light from the Great Rose."
        ),
        (
            Variant(
                scene=(
                    "An irregular court reached through a covered entry: scraping benches, "
                    "bundled pelts, washing lines, children and workers sharing limited space, "
                    "jettied rooms, patched plaster, drains, and only an indirect glimpse "
                    "toward the Lanthorn over rooftops."
                ),
                composition=(
                    "enclosed courtyard view with layered balconies and roof bridges, a "
                    "small off-center opening to the distant cathedral masonry"
                ),
                lighting="ordinary cool daylight broken by warm windows",
                constraints=(
                    "do not confirm the local light tale: no second sun, green beam, ghost, "
                    "or supernatural event; no graphic gore"
                ),
            ),
        ),
    ),
    Place(
        "the_hungry_ox",
        "The Hungry Ox",
        (
            "A boatmen's tavern by Maren's Green, used by working river people and known "
            "for the song 'The Water Knows One': smoky, crowded, practical, affordable, "
            "and tied to the rhythms of barges, fish, freight, and weather."
        ),
        (
            Variant(
                scene=(
                    "A low-ceilinged tavern interior after Lamplight: boatmen and porters "
                    "sharing stew and ale, wet cloaks steaming, a singer leading a table song, "
                    "river gear by the door, scarred tables, rushes, hearth smoke, and no "
                    "romanticized noble clientele."
                ),
                composition=(
                    "immersive eye-level interior from just inside the door, clustered tables "
                    "and rafters creating depth around a central communal song"
                ),
                lighting="warm hearth and tallow candles against cool dusk at the doorway",
                constraints="no readable tavern sign, menus, or modern glass bottles",
            ),
        ),
    ),
    Place(
        "saint_marens_church",
        "The Church of Saint Maren of the Reeds",
        (
            "A low parish church on the old dry Cut by Maren's Green, consecrated for "
            "boatmen and the poor when the channel still ran. Its low crypt has stood "
            "dry since the Serle was diverted; its churchyard charnel-door lintel carries "
            "one newly buried given name in chalk, and Maren Smallvoice rings the name-knell."
        ),
        (
            Variant(
                scene=(
                    "The humble church and its dry churchyard beside the filled former Cut: "
                    "a sexton chalking one short indistinct given name on the charnel-door "
                    "lintel, poor-relief bread being carried inside, boat-family parishioners "
                    "arriving from Maren's Green, and the small passing-bell visible in the "
                    "modest tower."
                ),
                composition=(
                    "street-and-churchyard three-quarter exterior at human scale, with the "
                    "low church, charnel door, dry former Cut, and ordinary parish work sharing "
                    "equal visual importance"
                ),
                lighting="quiet dry cloudy morning with soft light across pale worn stone",
                constraints=(
                    DRY_CUT_RULE
                    + "The crypt and ground are dry. No visible water, flooding, riverbank, "
                    "reeds, boats, mooring posts, wet stone, high-water scene, or reflections. "
                    "Somber but lived-in, not gothic horror; no miracle, ghost, or visible "
                    "second sun; the chalk is a single indistinct medieval given name, with "
                    "no other readable text"
                ),
                materials=DRY_CITY_MATERIALS,
            ),
        ),
    ),
    Place(
        "the_ilvane_chapel",
        "The Ilvane Chapel and Anchorhold",
        (
            "A deconsecrated chapel mortared shut since F.290. Its north-wall anchorhold "
            "remains occupied, and a small squint still opens to the lane so Dame Aldith "
            "can speak, receive alms, and trade verses for honest news."
        ),
        (
            Variant(
                scene=(
                    "A narrow lane beside the sealed chapel: visibly mortared doors and windows, "
                    "weathered sacred stonework, structural cracks and buttressing, the tiny "
                    "open squint of the inhabited anchorhold, a basket of alms, and two ordinary "
                    "neighbors speaking quietly with the unseen anchoress inside."
                ),
                composition=(
                    "intimate street-level architectural study, sealed chapel mass filling the "
                    "frame and the small human squint as the restrained focal point"
                ),
                lighting="thin early-autumn daylight in a shaded lane",
                constraints=(
                    "no apparition, magical glow, second shadow, or confirmed miracle; Dame "
                    "Aldith remains mostly unseen beyond the small squint"
                ),
            ),
        ),
    ),
)


def build_prompt(place: Place, variant: Variant) -> str:
    """Build the exact prompt recorded beside and sent for an image."""

    lines = [
        "Use case: stylized-concept",
        "Asset type: game environment inspiration concept art",
        (
            "Primary request: Create original, richly detailed environment concept art "
            f"for {place.name} in the Cathedral-City of Impossible Light."
        ),
        (
            "World/setting: Ombreval is a dense, free, fortified late-medieval river "
            "city in a realistic world. Streets pinch, widen, and dogleg between "
            "independently built facades; upper storeys are jettied; covered passages, "
            "small courts, and bridges between lofts make the quarters irregular and "
            "walkable. Human labor, weather, fire, flood, debt, trade, and distance matter."
        ),
        f"Canonical location lore: {place.lore}",
        f"Scene/backdrop: {variant.scene}",
        (
            "Style/medium: painterly historical architectural concept art with a "
            "monumental engraved-city sense of scale, cinematic realism, coherent "
            "perspective, and production-minded detail suitable for planning a first-person "
            "3D game environment; original design, not a copy of any existing artwork"
        ),
        f"Composition/framing: {variant.composition}",
        f"Lighting/mood: {variant.lighting}; grounded, inhabited, weathered, and unsentimental",
        "Materials/textures: "
        + (
            variant.materials
            or (
                "weathered limestone and fieldstone, lime plaster, exposed oak framing, "
                "leadwork, rough cloth, rope, iron, worn timber, terracotta and slate "
                "roofs, mud, soot, smoke, river damp, and visible hand construction"
            )
        ),
        (
            "People and technology: ordinary human townspeople in plausible practical "
            "late-medieval clothing; hand tools, animal or human power, period boats, "
            "carts, hoists, lamps, and market equipment only"
        ),
        (
            "World rules: no routine magic, magical species, spell effects, or reliable "
            "miracles. Outside the Lanthorn's Great Rose there is one ordinary sun and "
            "no impossible light."
        ),
    ]
    if variant.constraints:
        lines.append(f"Location-specific constraints: {variant.constraints}")
    lines.extend(
        (
            "Text: no captions, labels, titles, or legible writing in the image",
            (
                "Avoid: generic fantasy spectacle, fairy-tale prettiness, steampunk, "
                "modern objects, electric light, pristine theme-park streets, impossible "
                "mega-architecture unrelated to the lore, dramatic magic, logos, signatures, "
                "frames, borders, and watermarks"
            ),
        )
    )
    return "\n".join(lines)


def write_prompt_file(place: Place, prompts: tuple[str, ...]) -> Path:
    """Write all exact per-image prompts to one reviewable Markdown file."""

    place_dir = OUTPUT_ROOT / place.slug
    place_dir.mkdir(parents=True, exist_ok=True)
    prompt_path = place_dir / "image_generation_prompt.md"
    sections = [
        f"# {place.name}",
        "",
        (
            "Canonical inspiration prompts distilled from `lore/core_lore`. The script "
            f"sends each prompt below to `{MODEL}` without further augmentation."
        ),
        "",
    ]
    for index, prompt in enumerate(prompts, start=1):
        filename = f"{place.slug}_{index:03d}.png"
        sections.extend((f"## `{filename}`", "", "```text", prompt, "```", ""))
    prompt_path.write_text("\n".join(sections), encoding="utf-8")
    return prompt_path


def write_showcase() -> Path:
    """Write a dependency-free gallery for every image in the catalog."""

    cards: list[str] = []
    image_number = 0
    for place in PLACES:
        for index, variant in enumerate(place.variants, start=1):
            image_number += 1
            filename = f"{place.slug}_{index:03d}.png"
            image_path = f"places/{place.slug}/{filename}"
            prompt_path = f"places/{place.slug}/image_generation_prompt.md"
            search_text = " ".join(
                (place.name, place.slug, place.lore, variant.scene)
            ).lower()
            cards.append(
                f"""
        <article class="card" data-search="{escape(search_text, quote=True)}">
          <button class="image-button" type="button"
                  data-image="{escape(image_path, quote=True)}"
                  data-caption="{escape(place.name, quote=True)} — {index:03d}"
                  aria-label="View {escape(place.name, quote=True)} image {index:03d}">
            <span class="missing-copy">Image not generated yet</span>
            <img src="{escape(image_path, quote=True)}"
                 alt="Inspiration art for {escape(place.name, quote=True)}, view {index:03d}"
                 loading="lazy">
          </button>
          <div class="card-copy">
            <div class="eyebrow">Place {image_number:02d} · View {index:03d}</div>
            <h2>{escape(place.name)}</h2>
            <p>{escape(variant.scene)}</p>
            <a href="{escape(prompt_path, quote=True)}">Read generation prompt</a>
          </div>
        </article>"""
            )

    document = f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="dark">
  <title>Ombreval · Inspiration Images</title>
  <style>
    :root {{
      --ink: #e9e2d2;
      --muted: #a9a18f;
      --paper: #151512;
      --panel: #1e1d19;
      --line: #444036;
      --gold: #d3a54b;
      --green: #a9c7b7;
    }}
    * {{ box-sizing: border-box; }}
    html {{ background: var(--paper); color: var(--ink); }}
    body {{
      margin: 0;
      font-family: Georgia, "Times New Roman", serif;
      background:
        radial-gradient(circle at 82% -10%, #34443d 0, transparent 34rem),
        linear-gradient(180deg, #191914, var(--paper) 42rem);
      min-height: 100vh;
    }}
    header {{
      max-width: 92rem;
      margin: 0 auto;
      padding: clamp(3rem, 8vw, 8rem) clamp(1.1rem, 4vw, 4rem) 2.5rem;
    }}
    .kicker, .eyebrow {{
      color: var(--gold);
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: .72rem;
      letter-spacing: .14em;
      text-transform: uppercase;
    }}
    h1 {{
      max-width: 12ch;
      margin: .4rem 0 1rem;
      font-size: clamp(3rem, 9vw, 7.6rem);
      font-weight: 400;
      letter-spacing: -.055em;
      line-height: .88;
    }}
    .intro {{
      max-width: 47rem;
      color: var(--muted);
      font-size: clamp(1rem, 2vw, 1.25rem);
      line-height: 1.55;
    }}
    .toolbar {{
      position: sticky;
      z-index: 5;
      top: 0;
      border-block: 1px solid var(--line);
      background: color-mix(in srgb, var(--paper) 90%, transparent);
      backdrop-filter: blur(14px);
    }}
    .toolbar-inner {{
      display: flex;
      gap: 1rem;
      align-items: center;
      max-width: 92rem;
      margin: auto;
      padding: .8rem clamp(1.1rem, 4vw, 4rem);
    }}
    label {{ color: var(--muted); white-space: nowrap; }}
    input {{
      width: 100%;
      border: 1px solid var(--line);
      border-radius: 999px;
      padding: .75rem 1rem;
      background: #0f0f0d;
      color: var(--ink);
      font: inherit;
    }}
    input:focus {{ outline: 2px solid var(--gold); outline-offset: 2px; }}
    #count {{
      min-width: 7rem;
      color: var(--muted);
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: .75rem;
      text-align: right;
    }}
    main {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(min(100%, 25rem), 1fr));
      gap: 1px;
      max-width: 92rem;
      margin: 3rem auto 7rem;
      padding: 1px;
      background: var(--line);
      border: 1px solid var(--line);
    }}
    .card {{ min-width: 0; background: var(--panel); }}
    .card[hidden] {{ display: none; }}
    .image-button {{
      position: relative;
      display: grid;
      place-items: center;
      width: 100%;
      aspect-ratio: 3 / 2;
      overflow: hidden;
      border: 0;
      border-bottom: 1px solid var(--line);
      padding: 0;
      background: #10100e;
      color: var(--muted);
      cursor: zoom-in;
    }}
    .image-button img {{
      position: absolute;
      inset: 0;
      width: 100%;
      height: 100%;
      object-fit: cover;
      transition: transform 600ms cubic-bezier(.2,.7,.2,1), filter 300ms;
    }}
    .image-button:hover img {{ transform: scale(1.025); filter: brightness(1.06); }}
    .image-button.missing {{ cursor: default; }}
    .image-button.missing img {{ display: none; }}
    .missing-copy {{
      display: none;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: .75rem;
      letter-spacing: .08em;
      text-transform: uppercase;
    }}
    .image-button.missing .missing-copy {{ display: block; }}
    .card-copy {{ padding: 1.35rem 1.45rem 1.65rem; }}
    h2 {{ margin: .35rem 0 .6rem; font-size: 1.75rem; font-weight: 400; }}
    .card p {{
      display: -webkit-box;
      overflow: hidden;
      margin: 0 0 1rem;
      color: var(--muted);
      line-height: 1.48;
      -webkit-box-orient: vertical;
      -webkit-line-clamp: 4;
    }}
    a {{ color: var(--green); text-underline-offset: .2em; }}
    dialog {{
      width: min(96vw, 110rem);
      max-width: none;
      border: 1px solid #5d584d;
      padding: 0;
      background: #090908;
      color: var(--ink);
      box-shadow: 0 2rem 8rem #000;
    }}
    dialog::backdrop {{ background: rgb(0 0 0 / 88%); backdrop-filter: blur(5px); }}
    dialog img {{ display: block; width: 100%; max-height: 88vh; object-fit: contain; }}
    .dialog-bar {{
      display: flex;
      justify-content: space-between;
      gap: 1rem;
      align-items: center;
      padding: .7rem 1rem;
      color: var(--muted);
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: .78rem;
    }}
    .dialog-bar button {{
      border: 1px solid var(--line);
      border-radius: 999px;
      padding: .45rem .8rem;
      background: var(--panel);
      color: var(--ink);
      cursor: pointer;
    }}
    @media (max-width: 38rem) {{
      .toolbar-inner {{ align-items: stretch; flex-wrap: wrap; }}
      label {{ width: 100%; }}
      #count {{ min-width: auto; }}
    }}
  </style>
</head>
<body>
  <header>
    <div class="kicker">The Cathedral-City of Impossible Light · F.437</div>
    <h1>Places of Ombreval</h1>
    <p class="intro">Lore-grounded visual studies of the fortified river city:
      its markets, passages, bridges, working courts, churches, and impossible
      cathedral. Select an image to inspect the full composition.</p>
  </header>
  <div class="toolbar">
    <div class="toolbar-inner">
      <label for="filter">Find a place</label>
      <input id="filter" type="search" placeholder="Try market, river, passage…"
             autocomplete="off">
      <span id="count" aria-live="polite">{image_number} images</span>
    </div>
  </div>
  <main id="gallery">{''.join(cards)}
  </main>
  <dialog id="lightbox">
    <img id="lightbox-image" alt="">
    <div class="dialog-bar">
      <span id="lightbox-caption"></span>
      <button id="close" type="button">Close</button>
    </div>
  </dialog>
  <script>
    const cards = [...document.querySelectorAll('.card')];
    const filter = document.querySelector('#filter');
    const count = document.querySelector('#count');
    const dialog = document.querySelector('#lightbox');
    const dialogImage = document.querySelector('#lightbox-image');
    const caption = document.querySelector('#lightbox-caption');

    document.querySelectorAll('.image-button img').forEach((image) => {{
      image.addEventListener('error', () => {{
        image.parentElement.classList.add('missing');
        image.parentElement.disabled = true;
      }});
    }});

    filter.addEventListener('input', () => {{
      const query = filter.value.trim().toLowerCase();
      let visible = 0;
      cards.forEach((card) => {{
        const matches = !query || card.dataset.search.includes(query);
        card.hidden = !matches;
        if (matches) visible += 1;
      }});
      count.textContent = `${{visible}} image${{visible === 1 ? '' : 's'}}`;
    }});

    document.querySelectorAll('.image-button').forEach((button) => {{
      button.addEventListener('click', () => {{
        if (button.classList.contains('missing')) return;
        dialogImage.src = button.dataset.image;
        dialogImage.alt = button.dataset.caption;
        caption.textContent = button.dataset.caption;
        dialog.showModal();
      }});
    }});
    document.querySelector('#close').addEventListener('click', () => dialog.close());
    dialog.addEventListener('click', (event) => {{
      if (event.target === dialog) dialog.close();
    }});
  </script>
</body>
</html>
"""
    SHOWCASE_PATH.parent.mkdir(parents=True, exist_ok=True)
    SHOWCASE_PATH.write_text(document, encoding="utf-8")
    return SHOWCASE_PATH


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("must be at least 1")
    return parsed


def parse_args() -> argparse.Namespace:
    place_slugs = tuple(place.slug for place in PLACES)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--only",
        action="extend",
        nargs="+",
        choices=place_slugs,
        metavar="PLACE",
        help="generate only one or more place slugs (repeatable)",
    )
    parser.add_argument(
        "--size",
        default=DEFAULT_SIZE,
        choices=(
            "1024x1024",
            "1536x1024",
            "1024x1536",
            "2048x2048",
            "2048x1152",
            "3840x2160",
            "2160x3840",
            "auto",
        ),
        help=f"gpt-image-2 output size (default: {DEFAULT_SIZE})",
    )
    parser.add_argument(
        "--quality",
        default=DEFAULT_QUALITY,
        choices=("low", "medium", "high", "auto"),
        help=f"gpt-image-2 quality (default: {DEFAULT_QUALITY})",
    )
    parser.add_argument(
        "--concurrency",
        type=positive_int,
        default=5,
        help="maximum simultaneous API calls (default: 5)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="replace existing PNGs instead of keeping them",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="write prompt files and report work without calling the API",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="list cataloged place slugs and exit",
    )
    return parser.parse_args()


async def generate_image(
    client: AsyncOpenAI,
    prompt: str,
    target: Path,
    *,
    size: str,
    quality: str,
) -> None:
    """Generate one PNG and install it atomically at target."""

    response = await client.images.generate(
        model=MODEL,
        prompt=prompt,
        n=1,
        size=size,
        quality=quality,
        output_format="png",
    )
    if not response.data or not response.data[0].b64_json:
        raise RuntimeError("Image API response did not contain base64 image data")

    try:
        png_bytes = base64.b64decode(response.data[0].b64_json, validate=True)
    except (binascii.Error, ValueError) as error:
        raise RuntimeError("Image API returned invalid base64 data") from error
    if not png_bytes.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError("Image API response was not a PNG")

    temporary = target.with_name(f".{target.name}.tmp")
    temporary.write_bytes(png_bytes)
    temporary.replace(target)


async def generate_jobs(
    jobs: list[tuple[str, Path]],
    *,
    api_key: str,
    size: str,
    quality: str,
    concurrency: int,
) -> list[tuple[Path, Exception]]:
    """Run image jobs concurrently while letting independent failures finish."""

    from openai import AsyncOpenAI
    from httpx import Timeout

    semaphore = asyncio.Semaphore(concurrency)
    failures: list[tuple[Path, Exception]] = []
    timeout = Timeout(connect=15.0, read=600.0, write=60.0, pool=60.0)
    client = AsyncOpenAI(api_key=api_key, timeout=timeout, max_retries=3)

    async def run_one(prompt: str, target: Path) -> None:
        async with semaphore:
            print(f"  generate {target.relative_to(REPO_ROOT)}", flush=True)
            try:
                await generate_image(
                    client,
                    prompt,
                    target,
                    size=size,
                    quality=quality,
                )
            except Exception as error:  # Keep unrelated jobs resumable.
                failures.append((target, error))
                print(
                    f"  failed   {target.relative_to(REPO_ROOT)}: {error}",
                    file=sys.stderr,
                    flush=True,
                )
            else:
                print(f"  wrote    {target.relative_to(REPO_ROOT)}", flush=True)

    try:
        await asyncio.gather(*(run_one(prompt, target) for prompt, target in jobs))
    finally:
        await client.close()
    return failures


def main() -> int:
    args = parse_args()
    if args.list:
        for place in PLACES:
            print(f"{place.slug:<24} {len(place.variants)} image(s)  {place.name}")
        return 0

    selected_slugs = set(args.only or ())
    selected = tuple(
        place for place in PLACES if not selected_slugs or place.slug in selected_slugs
    )

    jobs: list[tuple[str, Path]] = []
    kept = 0
    for place in selected:
        prompts = tuple(build_prompt(place, variant) for variant in place.variants)
        prompt_path = write_prompt_file(place, prompts)
        print(f"  prompt   {prompt_path.relative_to(REPO_ROOT)}")
        for index, prompt in enumerate(prompts, start=1):
            target = prompt_path.parent / f"{place.slug}_{index:03d}.png"
            if target.exists() and not args.force:
                print(f"  keep     {target.relative_to(REPO_ROOT)}")
                kept += 1
            else:
                jobs.append((prompt, target))

    showcase_path = write_showcase()
    print(f"  showcase {showcase_path.relative_to(REPO_ROOT)}")

    if args.dry_run:
        print(
            f"dry run: {len(jobs)} image(s) would be generated, {kept} kept; "
            f"model={MODEL}, size={args.size}, quality={args.quality}"
        )
        return 0
    if not jobs:
        print(f"done: 0 generated, {kept} kept; catalog satisfied")
        return 0

    from dotenv import load_dotenv

    load_dotenv(REPO_ROOT / ".env")
    api_key = os.environ.get("OPENAI_API_KEY", "").strip()
    if not api_key:
        print(
            "OPENAI_API_KEY is not set (shell environment or repo-root .env)",
            file=sys.stderr,
        )
        return 2

    print(
        f"generating {len(jobs)} image(s) with model={MODEL}, size={args.size}, "
        f"quality={args.quality}, concurrency={args.concurrency}"
    )
    failures = asyncio.run(
        generate_jobs(
            jobs,
            api_key=api_key,
            size=args.size,
            quality=args.quality,
            concurrency=args.concurrency,
        )
    )
    if failures:
        print(
            f"incomplete: {len(jobs) - len(failures)} generated, {kept} kept, "
            f"{len(failures)} failed; rerun to retry missing files",
            file=sys.stderr,
        )
        return 1

    print(f"done: {len(jobs)} generated, {kept} kept; catalog satisfied")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
