# Lore fixes due to the 0.7× city resize

Status: backlog. Audited 2026-07-24 against the shipped post-resize plan.

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
spawn positions, and cross-city walking estimates were updated. The remaining
problems are below.

## 1. The old city dimensions survive in paces

Two current sources still describe Ombreval as twelve hundred by a thousand
paces:

- `lore/core_lore/setting_and_geography.md:47-48`
- `lore/second_sun/10_gazetteer_of_the_second_sun.md:10`

That wording predates the resize. It originally sat beside the old 1.2 km by
1.0 km metric dimensions, so it was plainly the diegetic rendering of the old
footprint. Only the metric half of the first passage was changed.

Choose and document one fix, then keep both sources identical:

- update the pace counts to describe the new footprint; or
- explicitly define an Ombreval pace as about 0.7 m if the old counts are meant
  to survive.

The first option is less surprising because no current measures document
defines such a short local pace.

## 2. Reed Ward's repeated "half a mile" is no longer spatially possible

Resize-sensitive half-mile claims remain in:

- `lore/the_dry_boatmen.md:24-25,458`
- `lore/families/family_alder.md:26,236,499,627,855`

They cover two different relationships and both need rewording:

1. The Alder table versus Ewart speaking at the Hungry Ox. The current Alder
   Moorings and Hungry Ox anchors are only about **54.5 m** apart and the
   gazetteer deliberately makes them one neighbourhood. "Across the Green" or
   similarly local wording fits; half a mile does not.
2. The Moorings/boat quarter versus the outer Serle wharves. The current
   anchors are about **216 m** apart in a straight line. The authored street
   estimate from River Gate to Maren's Green is about **295 m**, with the bank
   and yard approaches added on either end. The real route should be measured,
   but it is nowhere near an 805 m half-mile.

Keep the emotional and economic point—the wall and dry carry separate home
from water—without preserving the obsolete number.

Do **not** mechanically replace every mile fraction in the corpus. In
`lore/families/family_rud.md:866`, the Lanthorn and Saint Maren's are called a
quarter-mile apart. The current spatial index gives Gradine to Maren's Green as
about 390 m by street, almost exactly a quarter-mile, so that line still works.

## 3. Two route endpoints in the spatial index missed authored overrides

`lore/places/01_spatial_index.md` mostly follows the transformed JSON, but two
points in its principal-route table are still legacy values:

- At line 184, Stone/Fabric Way ends at `(72,80)`; the generated `fabric_way`
  ends at **`(74,91)`**.
- At line 186, Bell Way starts at `(78,-112)`; the generated `bell_way` starts
  at **`(72,-108)`**.

Use `lore/places/ombreval_buildings.json` (or the resolved entries in
`lore/places/shrink_transform.json`) as authority and compare the complete
polylines when fixing these rows.

## 4. The places README reports stale generated inventory

`lore/places/README.md` disagrees with its own current generated JSON/SVG:

- lines 101-102 say **59** named places; the JSON contains **69**;
- lines 108-110 say **1,030** total buildings and **965** ordinary urban-fabric
  buildings;
- the current JSON and both generated SVG maps report **1,108** total,
  **1,043** ordinary urban-fabric, and **65** named/reserved buildings.

Update the prose from `ombreval_buildings.json.statistics` after the final map
generation. Do not hand-edit generated JSON or SVG output.

## 5. The ward-share caveat describes a regeneration that already happened

`lore/places/README.md:139-144` says the ward shares predate the shrink and may
have drifted. The current post-resize ward map has already recomputed the real
`district_for()` partition as **32.7%** Bell-and-Sluice. The current scaled
authoring box is 117,453 m² against a 548,242.6 m² wall polygon, or about
**21.42%**, so the rounded `~33%` and `~21%` claims remain correct.

Replace the uncertainty note with post-resize figures/provenance, or simply
remove it. Regeneration is still the right verification command, but it is no
longer pending work.

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

## Acceptance criteria

- Canonical prose has one unambiguous current city footprint, in metric and any
  diegetic units.
- The Reed/Alder distance language agrees with current routes while preserving
  the dry-carry grievance.
- The two principal-route rows exactly match the generated road polylines.
- The places README inventory matches `ombreval_buildings.json.statistics` and
  its ward-share text describes the post-resize map.
- A final search for `1.2 km`, `1.0 km`, twelve-hundred/thousand pace city
  dimensions, and location-specific half-mile claims produces no canonical
  resize leaks (excluding the explicitly ignored material above).
