# The Places of Ombreval

> Authoritative spatial guide for the F.437 city. This directory decides where
> places are, how they connect, what surrounds them, and what a future world
> builder must preserve.

## Status and authority

This guide establishes a new city plan from scratch. The present procedural
city outside the Lanthorn is a prototype, not a constraint. Its repeated
blocks, roads, generic towers, square positions, canal, and walls may all be
removed or replaced.

Only these pieces of current game geometry are fixed:

- the Lanthorn and its orientation;
- the Great Rose in the west front;
- the two large approach statues, the Dawn Bearer and the Seraph; and
- the coordinate convention already declared in `assets/world/areas.json`.

The Gradine remains directly outside the west doors because that relationship
is canon, but its paving and edges may be rebuilt. Everything else in this
guide is the target arrangement for later authored world work.

For facts of history, religion, named people, and the impossible light,
`lore/core_lore/` and `lore/second_sun/00_canon.md` remain senior. This guide
does not rewrite those facts. It makes their shared geography concrete. If an
inspiration image disagrees with written lore, the writing wins.

The directory `lore/wip_lore_please_ignore_this_is_NOT_canon/` was not used and
must never be used to interpret this plan.

## Coordinate convention

All plan coordinates are in metres and written `(x, z)`:

- north is `+x`;
- south is `-x`;
- west is `+z`;
- east is `-z`; and
- height is `+y`, with the Lanthorn pavement close to `y = 0`.

Coordinates are design targets, normally the centre of a place or the points
along a route. They are not ready-made `areas.json` boxes. A later build must
survey the finished walkable geometry and divide it into non-overlapping area
boxes. Districts, buildings, squares, and rooms must not be entered as nested
overlapping areas merely because this guide discusses them together.

## The plan in one paragraph

Ombreval is an irregular walled city about 1.2 km west to east and 1.0 km north
to south. The Lanthorn stands a little west of its geometric centre on the old
ford ground. Crooked streets radiate from that older centre, while the filled
Cut runs conspicuously straight across the southern third. The Serle itself
flows west to east outside the south wall. Freight lands at the outer wharves,
passes through the River Gate, and divides along the dry Cut toward the Tallage
and Maren's Green. Wool enters at the western Wool Gate, stone and long timber
at the northern Stone Gate, and downstream road traffic at the eastern Harne
Gate. No open river or canal exists inside the walls.

The city is not arranged as five equivalent plazas around the cathedral. Each
square is the widened working part of a route:

- the Wickmarket catches western wool-road and pilgrim traffic;
- Coswald's Yard receives stone and timber from the north;
- the Tallage controls freight where the River Cartway meets the Cut;
- Maren's Green serves the boat-families between the River Gate and the east
  Cut; and
- the Bellstand gathers the eastern wards beneath the secular watch bell.

The Gradine is smaller, more formal, and dangerously inadequate for a whole-city
crowd.

## Guide files

- [`00_city_plan.md`](00_city_plan.md) — the whole city: land, walls, wards,
  routes, building grammar, skyline, and implementation rules.
- [`01_spatial_index.md`](01_spatial_index.md) — the map, coordinates, extents,
  adjacencies, and stable planning IDs.
- [`02_canonical_gazetteer.md`](02_canonical_gazetteer.md) — detailed treatment
  of every established or previously approved named place.
- [`03_new_places_and_infrastructure.md`](03_new_places_and_infrastructure.md)
  — the additional gates, service places, parish reserves, utilities, and
  ordinary institutions needed to make the city function.
- [`04_routes_and_sightlines.md`](04_routes_and_sightlines.md) — daily freight,
  market, feast, pilgrimage, and covert routes, plus the visual reveals that
  make the plan legible in first person.

## Source ledger

The principal written sources behind the plan are:

- [`../core_lore/places.md`](../core_lore/places.md) for established names and
  relationships;
- [`../core_lore/setting_and_geography.md`](../core_lore/setting_and_geography.md)
  for scale, orientation, river, density, and wider routes;
- [`../core_lore/candor_and_churches.md`](../core_lore/candor_and_churches.md)
  for the three established sacred places and institutional uses;
- [`../core_lore/calendar_and_history.md`](../core_lore/calendar_and_history.md)
  for the historical layers that shaped streets;
- [`../core_lore/trade_and_daily_life.md`](../core_lore/trade_and_daily_life.md)
  for freight, markets, measures, and material dependencies;
- [`../core_lore/naming_language.md`](../core_lore/naming_language.md) for new
  names and the formerly optional place-name examples;
- [`../second_sun/00_canon.md`](../second_sun/00_canon.md) as the senior Second
  Sun source;
- [`../second_sun/10_gazetteer_of_the_second_sun.md`](../second_sun/10_gazetteer_of_the_second_sun.md)
  for secret-site geometry and public associations; and
- [`../second_sun/11_glossary_and_naming.md`](../second_sun/11_glossary_and_naming.md)
  for street, bridge, passage, and ropewalk details.

The prompts under `lore/inspiration_images/places/` inform material and street-
level atmosphere. They do not control position, and written lore wins any
conflict.

## Binding city rules

Later lore and construction should preserve all of these:

1. The Lanthorn and the two statues stay where they are.
2. The Lanthorn's quire is east, its west doors and Great Rose are west, and
   the Gradine touches those west doors.
3. The Lanthorn is a little west of the walled city's geometric centre, not at
   its western edge.
4. The Serle is outside the south wall. There is no open in-city canal, active
   quay, barge basin, or waterfront.
5. The Cut is a completely dry, filled, unusually straight trade street across
   the southern third.
6. The five squares and the Gradine occupy the relationships fixed in core
   lore. The Gradine is never counted as a sixth town square.
7. Carts can travel from the outer wharves through the River Gate to both the
   Tallage and Maren's Green without crossing the cathedral precinct.
8. The Draper's Reach lies between Coswald's Yard and the Wickmarket. The
   Needle runs from Wickmarket back lanes toward the Tallage.
9. The Bellstand is east of and behind the Lanthorn's east end. Skinners'
   Court is north of the Gradine. Gaunt Passage is near the Tallage. Tanners'
   Slip is behind Maren's Green.
10. Saint Maren's church, crypt, churchyard, charnel door, Alder Moorings, Eel
    Bridge, and Hungry Ox form one walkable neighbourhood, not scattered props.
11. The Old Sluice closes the east end of the Cut and is dry.
12. The city is dense but not a maze of repeated blocks. Major routes have a
    material reason; minor routes dogleg and pinch because parcels accumulated
    independently.
13. The Lanthorn dominates the skyline. Ordinary houses do not become towers
    merely to add silhouettes.
14. The impossible light is visible only through the Great Rose from inside
    the Lanthorn. No exterior layout, statue, reflective surface, or sightline
    extends it into the city.
15. The walls and streets must show four centuries of adaptation: old ford
    lanes, the imposed Cut, post-diversion encroachment, fires, flood repairs,
    guild wealth, and ordinary maintenance.

## What has become fixed here

The optional in-register examples from `lore/core_lore/places.md` are assigned
real sites in this plan: Tenterhook Lane, Burnt Court, Maren's Slip, Crookneck
Lane, Eelback Alley, Malt Passage, and the Chain Bridge are now places in
Ombreval. Their established kinds have not changed.

This guide also establishes a small number of practical names needed to make
the city navigable: the Wool, Stone, Harne, and River gates; the Reed Postern;
the Shambles; Seven Lofts; Bellfounders' Yard; and Ford Well. These additions
are deliberately workmanlike. They do not compete with the older lore's named
places or turn every street into cathedral imagery.

Additional parish churches are necessary at this scale, but this guide reserves
their sites without naming their dedications. Naming them belongs to later
parish lore and should be done sparingly.

## Reading the plan at street level

A useful test is whether a resident can give directions without coordinates:

> From the River Gate, follow the carts to the dry Cut. West under the Tally
> Bridge is the Tallage; east through the eel smoke is Maren's Green. For the
> Lanthorn, turn north at the weigh-beam, pass the mouth of the Needle, and keep
> the north tower scaffold ahead until the street spills onto the Gradine.

If a later change makes directions like that impossible, it has probably
weakened the geography even if the individual building looks attractive.
