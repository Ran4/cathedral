# City Plan of Ombreval

## Planning premise

Ombreval was not laid out once. It is the overlap of three strong geometries:

1. lanes around an old ford-market, older than the Lanthorn;
2. the Lanthorn precinct and the routes built to serve four centuries of
   Fabric work; and
3. the Cut, a straight engineered river reach later filled and converted into
   a freight street.

Those layers explain the city better than a procedural grid. Near the
Lanthorn, roads bend around sacred property, buttresses, chapter walls, and
parcels that predate the church. Across the southern third, the Cut ignores
those alignments. Near the walls, later houses fill defensive margins and
roads converge hard on gates.

The resulting city is comprehensible in the large and surprising in the
small. A porter knows which four routes accept a cart. A child can still lose
an afternoon in the passages between them.

## Fixed anchors and target extent

The Lanthorn keeps its existing footprint and height. For planning purposes:

| Anchor | Target position or envelope |
|---|---|
| Lanthorn interior | Existing cruciform footprint, approximately `x = -67..67`, `z = -104..81` |
| Great Rose and west doors | West front at approximately `z = 81` |
| Gradine | `x = -38.5..38.5`, `z = 105..157`; paving may be rebuilt |
| Dawn Bearer | Existing statue centred at `(-72, 190)` |
| Seraph | Existing statue centred at `(72, 190)` |

The target walled enclosure is an irregular polygon close to these limits:

- north wall: `x = 470..510`;
- south wall: `x = -475..-515`;
- west wall: `z = 490..525`;
- east wall: `z = -640..-680`.

This gives about 1.2 km west to east and 1.0 km north to south. Its geometric
centre is near `(0, -75)`, putting the Lanthorn roughly seventy metres west of
centre as the lore requires.

Corners should be rounded, angled, or stepped around old property and ground;
they should not read as a perfect rectangle from above. The broad planning
polygon is:

```text
NW (475, 485) -> N (505, 130) -> NE (465, -650)
   -> E (330, -675) -> SE (-445, -660)
   -> S (-510, -120) -> SW (-475, 485) -> NW
```

These wall points are targets, not area boxes. Gate openings and towers will
break the lines.

## Ground, water, and drainage

The Lanthorn pavement is the height datum at about `y = 0`. The city should no
longer be a single flat slab.

- The northern ground rises gradually toward `y = 7..10` at the Stone Gate.
- The western gate and Wickmarket sit around `y = 2..4`.
- The old ford ground and Lanthorn precinct remain near `y = 0`.
- The filled Cut is a shallow linear low at roughly `y = -2..-3`.
- The south wall stands around `y = -5..-7`.
- The wharf apron falls beyond it to the Serle's north bank around `y = -10`.

The grades must stay cartable. Short steps belong at courts, churchyards, and
pedestrian passages, not on the River Cartway or Cut.

The Serle flows west to east, therefore from `+z` toward `-z`, beyond the south
wall. Its near bank lies roughly seventy metres outside the wall. The outer
wharves occupy that strip and may be visible through the River Gate, but the
working water is outside the playable walled city unless a future feature
explicitly expands scope.

There is no open watercourse inside the walls. Rain runs in stone or earth
gutters toward settling pits, covered culverts, garden ground, and the south
wall. The Cut is not used as a picturesque drain. In ordinary weather it is
dry, dusty, and cart-worn. After rain it may hold isolated ruts and puddles,
never a continuous reflective channel.

Ford Well at `(88, 35)` is the public reminder of the ground's older water
history. It is a deep covered well, not an opening to the Serle. Other ward
wells and cisterns are placed so households do not depend on cathedral water.

## Historical growth written into the plan

### The ford city

Before the Fabric era, the useful ground was the crossing and market that now
lies beneath and just south of the Lanthorn. The oldest lanes therefore aim
toward the cathedral precinct and then fail to pass through it. They terminate,
split, or slip around the grounds. Vhairé's old crossing is remembered in Ford
Well and in procession, not as an exposed archaeological ruin.

### The Fabric city

Stone came through the north, timber through north and west, glass from Cinder
Row, wax from the Wickmarket, and labour from every ward. Coswald's Yard,
Cinder Row, and the western approach are consequences of the building work.
The Chapter precinct grew against the Lanthorn's north-east side; ordinary
houses pressed close elsewhere. The cathedral never acquired a vast empty
close.

### The water city

The Cut was driven straight across the southern third from about `z = 465` to
`z = -605`, centred close to `x = -305`. Bridges, salt houses, fish markets,
and warehouses attached themselves to its banks. The Tallage grew at its
junction with the customs route. Saint Maren's and the Alder Moorings grew
farther east among boat-families.

### The dry city

After F.363–369, the channel was filled but property did not turn around. Old
bank doors, hoist beams, gallery bridges, cellars, retaining stone, and the
names of water work still face the dry street. The centre of the former bed is
now the easiest east–west cart road in Ombreval. New stalls and shallow shops
have encroached on its margins, but its straightness remains unmistakable.

The new cartage enriched some carriers, toll interests, and large boat houses,
but it did not restore every Cut warehouse or river household. The dry city's
odd mixture of valuable frontage, obsolete bank-side rooms, and cheap upper
lodging begins with that uneven recovery.

### The city after the Hammering

The Great Hail of F.415 struck a city whose flood-weakened foundations carried
generations of jetties, lofts, tile, and piecemeal repair. Present F.437
geometry must therefore show survival and contraction together. Sound ground
floors can remain in use beneath stripped, braced, storage-only, or empty upper
storeys. A repaired roof may cover two formerly separate houses. A shortened
jetty, rebuilt party wall, blocked stair, or roof material changing halfway
across a frontage should have a parcel-level reason.

Approximately 5,000 people occupy a city built for roughly 15,000. Major work
routes and markets remain busy, but density is uneven: quiet courts and vacant
upper windows can sit one turn from a crowded square without turning the city
into a ruin.

## The eight planning wards

Wards are broad authoring districts. They are not `areas.json` entries and may
overlap the everyday catchment of a square.

| Ward | Approximate bounds | Character and reason |
|---|---|---|
| Fabric Ward | `x = -110..180`, `z = -160..235` | Lanthorn, Gradine, Chapter, pilgrims, fabric workers, Skinners' Court, Ford Well |
| Wick Ward | `x = -170..130`, `z = 235..500` | Wickmarket, west gate traffic, chandlers, honey, lodging, back lanes |
| Cloth Ward | `x = 80..275`, `z = 190..390` | Draper's Reach, tentering, cloth halls, merchant lofts |
| Wallwright Ward | `x = 175..485`, `z = 40..260` | Coswald's Yard, stone, timber, lime, masons, northern gate road |
| Cinder Ward | `x = -270..20`, `z = 115..315` | Cinder Row, fire-conscious workshops, Burnt Court, the Needle's west approaches |
| Weigh Ward | `x = -435..-175`, `z = -40..235` | Tallage, salt houses, Tally Bridge, Gaunt Passage, customs and pawning |
| Reed Ward | `x = -455..-160`, `z = -500..-235` | Maren's Green, church and crypt, fish, Moorings, tanners, boat-families |
| Bell and Sluice Wards | `x = -180..330`, `z = -620..-150` | Bellstand, Ilvane Chapel, eastern housing, bellfounding, Old Sluice |

The far north-east and far west margins contain dense but less famous
residential and food-storage streets. They are not empty filler: they hold
bakers, brewers, small gardens, stables, parish reserves, and people who do not
work in the named trades.

## Walls, gates, and roads beyond

### Wool Gate

The western gate is centred near `(-35, 510)`. The upstream road toward Brede
and the Combs enters here with wool, hides, honey, and pilgrims. Inside, the
street bends through the Wickmarket before aligning with the west approach.
The bend prevents an implausible half-kilometre ceremonial boulevard and means
the Lanthorn is revealed in stages.

### Stone Gate

The northern gate is centred near `(495, 135)`. Stone, lime, long timber, and
Fabric carts descend from it to Coswald's Yard. The Lantern Road to Ostrelle
also leaves by this gate before separating from the quarry and timber roads
beyond the wall. This fixes the city end of the six-week journey without
filling in the distant geography.

### Harne Gate

The eastern gate is centred near `(15, -665)`. The dry road toward Harne and
the downstream country enters here. Inside, it reaches the Old Sluice road and
the Bellstand by doglegs rather than looking axially through the whole city.

### River Gate

The main southern gate is centred near `(-505, -135)`. It is the broadest
working gate, with paired leaves, a porter wicket, toll shelter, dung ruts, and
room for one cart to wait while another passes. The River Cartway reaches the
Cut near `(-305, -125)` and splits west to the Tallage and east to Maren's
Green. This route carries freight; pilgrims do not get a picturesque private
gate to the water.

### Reed Postern

The foot and handcart postern near `(-455, -535)` serves eastern wharf hands,
fish carriers, funerals, and the Reed Ward. It cannot admit an ox cart. Maren's
Slip climbs from it toward the Green. The postern gives boat-families a short
working route without weakening the River Gate's customs role.

It also has two roles the plan should not sand off. Once a year, at
Vhairestide, it is opened wide for a crowd — the Walking Out — and the whole
Reed Ward passes through it to see the river. And because handbarrows pass it
tolled lightly or not at all, it is the ward's untolled barrow route: a
working door that is pointedly not a customs house, and the substance of the
Reed Ward's postern politics at every reckoning.

The wall itself is about twelve to fifteen metres high, with towers at gates,
corners, and a few vulnerable turns. Wall walks and roofs must not rival the
Lanthorn towers. Houses may back onto the inner lane but do not casually punch
private doors through the wall.

## Primary movement network

### West approach

The approach is a sequence, not a boulevard:

```text
Wool Gate -> Wickmarket -> Pilgrim Bend -> statue courts -> Gradine -> west doors
```

From the Wool Gate to the Wickmarket, the road is six to seven metres wide and
commercial. East of the Wickmarket it narrows, turns around a lodging block,
and only then finds the Lanthorn axis. Near `z = 225` it opens enough for the
Dawn Bearer and Seraph to stand in separate side courts at their fixed
positions. The final approach funnels to the Gradine rather than expanding
into a second square.

The statues remain visible above roofs before their bases are visible. Dense
buildings behind and beside them make their scale legible. Their courts are
ordinary stone and market-edge space, not magical sanctuaries.

### The Cut

The Cut centreline stays close to `x = -305`. Its ordinary clear cart width is
eight to twelve metres, widening at the Tallage and Maren's Green and narrowing
under the Tally and Chain bridges. Bank-front buildings face it on slightly
raised thresholds. At long range, its straight run is visible in fragments
under awnings, bridges, dust, and traffic. Houses over the filled channel
settle where houses on the old banks do not — “built on the bed” — so cracked
party walls, doors out of true, and shored fronts may follow an invisible line
along the route (see `wells_and_water.md`).

### River Cartway

This route climbs from the River Gate to the Cut junction at `(-305, -125)`.
It has a hard-wearing central strip, wheel ruts, porter margins, dung collection,
and no stairs. Westbound freight reaches the Tallage in a few minutes;
eastbound fish and boat gear reach Maren's Green without crossing Tallage
weighing space unless customs requires it.

### Fabric Way

The northern working route joins Coswald's Yard to the Lanthorn's north side
and west approach. It carries stone sledges, scaffold timber, lime carts, and
processions at different hours. It does not run straight into a cathedral
door. At the precinct it divides into service lanes around the north tower and
the public route to the Gradine.

### Bell Way

A crooked eastward street leaves the Lanthorn's apse neighbourhood, passes the
Ilvane lane junction, and enters the Bellstand through Bellfoot Passage. From
the square another road reaches Harne Gate. The apse should appear suddenly
when walking west; the Bellstand tower should replace it as the eastward
landmark after the second turn.

### The market web

The Draper's Reach and Tenterhook Lane connect the northern and western trade
districts. The Needle is the tempting pedestrian shortcut from the Wickmarket
back lanes toward the Tallage, unusable by carts and uncomfortable even with a
basket. Cinder Row provides a second working connection toward the Gradine.
Together these routes prevent every trip from collapsing onto the west
approach.

## Streets and blocks

The city should use parcels rather than uniform blocks.

- Major cart streets are normally `5.5..7.5 m` clear, with the Cut wider.
- Ordinary streets vary between `3.0..5.0 m` and should change width within a
  single view.
- Alleys and covered passages are `1.4..2.5 m` except the Needle, whose pinch
  is about `1.05..1.2 m`.
- A typical facade is `4..9 m` wide. Several facades make one apparent house
  row; a single procedural building should not occupy a whole block.
- Buildings are usually two or three storeys. Four is exceptional and should
  mark wealth, storage, or an inherited constraint.
- Jetties project `0.4..1.0 m`, never identically on both sides.
- Roof bridges occur where one owner or guild controls both buildings. They
  need doors, structure, and a reason, not random skyline decoration.
- Courts sit behind street houses and are reached through passages. Their
  owners, drains, work, and escape routes matter.

There should be no recurring pattern in which every block contains the same
central dogleg, two lateral alleys, and one bridge. Those elements remain common,
but each arises from a local parcel story.

## Materials and fire history

Ground floors on principal streets are stone or heavy timber framed with
infill. Upper floors mix lime plaster and exposed oak. Slate is concentrated
on the Lanthorn, churches, wealthy halls, and fire-conscious guild roofs;
terracotta tile marks rebuilt commercial districts; cheaper timber shingles
and limited thatch survive toward the walls.

Cinder Row's F.171 rebuild has continuous stone lower storeys, party-wall
firebreaks, covered sand tubs, shutterable furnaces, and timber above. Tanners'
Slip shows patched masonry after F.157. The Wickmarket's fire risk produces
open cauldron stands, hooks, sand, and gaps around the hottest work, not a
modern safety code. Burnt Court preserves smoke-dark stone without becoming a
ruin.

Flood history belongs below waist height: mismatched lower masonry, blocked
bank doors, old iron rings, salt bloom, filled steps, and cellars that remain
unpopular. It must not imply that present streets are wet.

Hammering history belongs above eye level: mismatched roof patches, shortened
jetties, replaced braces, pitted soft stone, flattened metalwork, and upper
openings closed with boards or reused shutters. Slate, tile, shingle, timber,
and scavenged lead may meet on one repaired roof. This damage must not make
every building derelict; ordinary maintenance has continued for twenty-two
years.

## Squares as working spaces

No major square is a clean ninety-two-metre paved tile with a central fountain.
Water is valuable, geometry is inherited, and stalls occupy good ground.

| Place | Target centre | Usable open character |
|---|---:|---|
| Gradine | `(0, 131)` | `77 x 52 m`, shallow stepped forecourt, axial but crowded |
| Wickmarket | `(-25, 355)` | about `82 x 68 m`, bent trapezoid with hot-work islands |
| Coswald's Yard | `(255, 155)` | about `112 x 88 m`, rough working yard with tracing floor |
| Tallage | `(-305, 90)` | about `118 x 86 m`, elongated along the Cut and broken by weighing equipment |
| Maren's Green | `(-305, -365)` | about `105 x 88 m`, market street, churchyard edge, smoke and bridge constriction |
| Bellstand | `(45, -255)` | about `78 x 66 m`, sloped civic square dominated off-centre by tower and platform |

The dimensions describe the overall breathing room, not a single empty
rectangle. Arcades, stairs, stalls, trees, wells, walls, and traffic reduce the
walkable interior.

## Skyline and visual hierarchy

The Lanthorn is the city. Its crossing, west towers, dome or lantern, Great
Rose facade, and scaffolded north crown dominate long views. The preserved
thirty-metre statues are the next most unusual silhouettes, revealed from the
west and from nearby rooflines.

Below them:

- the Bellstand watch tower reaches roughly `32..36 m`;
- Saint Maren's modest bell tower reaches roughly `20..24 m`;
- gate and wall towers reach roughly `18..24 m`;
- unnamed parish reserves may eventually carry `16..22 m` towers;
- guild halls and warehouses use roof mass, hoists, and chimneys rather than
  arbitrary spires; and
- ordinary houses remain mostly `9..16 m` to the ridge.

Five generic landmark towers must not be retained simply because there are
five squares. Each square's identity comes from its work and its one justified
vertical element, if any.

## Density, food, and uncelebrated land

A believable city cannot consist only of monuments and named quest sites.
Between them are bakehouses, brewers, lodging houses, stables, yards, kitchens,
small gardens, dung carts, wood stacks, wells, cisterns, laundries, schools,
copyists, sheds, and rented rooms. Seven Lofts near the north-west wall stores
grain. The Shambles near the south-west working route keeps slaughter away
from the pilgrimage precinct. Bellfounders' Yard puts smoke and testing noise
east of the Bellstand.

Small productive gardens survive along the inner wall where defense and poor
ground prevented full building. They are narrow leases, not pastoral parks.
No broad ornamental greenbelt rings the cathedral.

## Area-map implementation rules

When a place is built and added to `assets/world/areas.json`:

1. Survey the final collision and walkable geometry; do not copy a planning
   rectangle blindly.
2. Give ownership to the most specific public place. For example, the Tallage
   boxes must exclude the Tally Bridge passage, Lise Copp's shop, and any
   separately named interior if those receive their own areas.
3. Do not create overlapping district areas behind those places. Wards remain
   metadata and prompt lore unless the area model later gains hierarchy.
4. Use multiple disjoint boxes for bent streets and irregular squares.
5. Keep upper rooms distinct only when the player can reach them and their
   location name matters.
6. Retain exterior ground boxes around a building without swallowing its
   interior, as already done for the Lanthorn.
7. Test every shared face. Inclusive-minimum, exclusive-maximum containment
   makes a boundary deterministic only when the boxes meet exactly.

## Things the redesign must remove

- the active canal currently generated inside the western city;
- five interchangeable square tiles with identical fountains and stall rings;
- generic landmark towers assigned one per square;
- the uniform 120-metre road and block grid;
- repeated block-internal doglegs generated from one template;
- walls without usable gates or a relationship to the Serle;
- roads that pass through the Lanthorn's footprint or reserve an implausibly
  empty cathedral superblock; and
- any exterior manifestation of the impossible light.

The target is not disorder for its own sake. It is accumulated, legible cause:
water made one line, worship another, freight a third, and four centuries of
householders made everything between them difficult.
