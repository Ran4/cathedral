# Lore fixes due to the 0.7× city resize

Status: implemented 2026-07-24 against the shipped post-resize plan.

## Scope and baseline

This audit covers canonical material under `lore/`. It excludes
`lore/wip_lore_please_ignore_this_is_NOT_canon/` and the two Second Sun HTML
files that `lore/AGENTS.md` explicitly says to ignore.

The authoritative current headline is roughly **840 m west-to-east by 700 m
north-to-south**. The generated wall polygon in
`lore/places/ombreval_buildings.json` spans about 812 m on the west/east `z`
axis and 710.5 m on the north/south `x` axis, which is consistent with that
rounded headline. `scripts/shrink_transform.py` is the transform authority:
ordinary positions moved by 0.7×, with explicit core/cluster overrides, while
building sizes and street widths generally did not shrink.

The main canon, cadastral maps, most coordinates, square extents, character
spawn positions, and cross-city walking estimates were already updated. The
issues found by this audit and their completed corrections are below.

## 1. The old city dimensions survived in paces

Two current sources described Ombreval as twelve hundred by a thousand
paces:

- `lore/core_lore/setting_and_geography.md:47-48`
- `lore/second_sun/10_gazetteer_of_the_second_sun.md:10`

That wording predated the resize and originally sat beside the old 1.2 km by
1.0 km metric dimensions. Both sources now use eight hundred and forty by
seven hundred paces, matching the new 840 m by 700 m headline without inventing
a special short local pace.

## 2. Reed Ward's repeated "half a mile" was no longer spatially possible

Resize-sensitive half-mile claims remained in:

- `lore/the_dry_boatmen.md:24-25,458`
- `lore/families/family_alder.md:26,236,499,627,855`

They covered two different relationships:

1. The Alder table versus Ewart speaking at the Hungry Ox. The current Alder
   Moorings and Hungry Ox anchors are only about **54.5 m** apart and the
   gazetteer deliberately makes them one neighbourhood. "Across the Green" or
   similarly local wording fits; half a mile does not.
2. The Moorings/boat quarter versus the outer Serle wharves. The current
   anchors are about **216 m** apart in a straight line. The authored street
   estimate from River Gate to Maren's Green is about **295 m**, with the bank
   and yard approaches added on either end. The real route should be measured,
   but it is nowhere near an 805 m half-mile.

The family conflict now uses "across the Green" and the dry-carry passages use
the wall, gate, and cart road as their landmarks. This preserves the emotional
and economic point—the wall and dry carry separate home from water—without the
obsolete number.

Do **not** mechanically replace every mile fraction in the corpus. In
`lore/families/family_rud.md:866`, the Lanthorn and Saint Maren's are called a
quarter-mile apart. The current spatial index gives Gradine to Maren's Green as
about 390 m by street, almost exactly a quarter-mile, so that line still works.

## 3. Two route endpoints in the spatial index had missed authored overrides

`lore/places/01_spatial_index.md` mostly followed the transformed JSON, but two
points in its principal-route table were legacy values:

- Stone/Fabric Way now ends at **`(74,91)`**, matching `fabric_way`.
- Bell Way now starts at **`(72,-108)`**, matching `bell_way`.

The complete table rows were compared with
`lore/places/ombreval_buildings.json`, not only the changed endpoints.

## 4. The places README reported stale generated inventory

`lore/places/README.md` disagreed with its own current generated JSON/SVG:

- lines 101-102 say **59** named places; the JSON contains **69**;
- lines 108-110 say **1,030** total buildings and **965** ordinary urban-fabric
  buildings;
- the current JSON and both generated SVG maps report **1,108** total,
  **1,043** ordinary urban-fabric, and **65** named/reserved buildings.

The prose now reports the generated statistics: **69** named places, **1,108**
total buildings, **1,043** ordinary urban-fabric buildings, and **65**
named/reserved buildings. Generated JSON and SVG output were not hand-edited.

## 5. The ward-share caveat described a regeneration that already happened

`lore/places/README.md:139-144` said the ward shares predated the shrink and may
have drifted. The current post-resize ward map had already recomputed the real
`district_for()` partition as **32.7%** Bell-and-Sluice. The current scaled
authoring box is 117,453 m² against a 548,242.6 m² wall polygon, or about
**21.42%**, so the rounded `~33%` and `~21%` claims remain correct.

The README now gives the post-resize figures directly: **32.7%** for the
generated partition and **21.4%** for the scaled authoring boxes. It retains
the ward-map regeneration command as the verification path.

## Already correct; preserve these decisions

- `lore/core_lore/core_lore.md`, `lore/core_lore/setting_and_geography.md`,
  `lore/second_sun/00_canon.md`, and the main `lore/places/` plan now use the
  840 × 700 m headline.
- `lore/places/04_routes_and_sightlines.md` correctly reduced the west-doors to
  Tally Bridge walk from three-to-four minutes to two-to-three minutes.
- Building dimensions, street widths, hearing radii, the Lanthorn, and other
  fixed/core geometry must not be blindly multiplied by 0.7.
- Harne, Brede, Salorge, Ostrelle, and other off-map journey lengths were not
  made closer by shrinking the playable city.
- Do not alter protected inconsistencies listed in `lore/CONSISTENCY.md`.
- Do not use or update the ignored WIP folder to establish canon. In particular,
  `lore/second_sun/index.html` embeds an old snapshot but is explicitly ignored
  by `lore/AGENTS.md`.
- Historical records under `features/implemented/` intentionally retain
  pre-shrink coordinates per `features/AGENTS.md`.

## Verification

- [x] Canonical prose has one unambiguous current city footprint, in metric and any
  diegetic units.
- [x] The Reed/Alder distance language agrees with current routes while preserving
  the dry-carry grievance.
- [x] The two principal-route rows exactly match the generated road polylines.
- [x] The places README inventory matches `ombreval_buildings.json.statistics` and
  its ward-share text describes the post-resize map.
- [x] A final search for `1.2 km`, `1.0 km`, twelve-hundred/thousand pace city
  dimensions, and location-specific half-mile claims produces no canonical
  resize leaks (excluding the explicitly ignored material above).
