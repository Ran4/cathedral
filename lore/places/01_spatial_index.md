# Spatial Index

## Schematic map

This diagram shows relationships, not exact footprints. North is up. West is
left, although west is positive `z` in game coordinates.

For the exact wall, road, site, fixture, and individual-building plan, use the
zoomable [`ombreval_top_down_map.html`](ombreval_top_down_map.html). Its
numbered markers correspond to the complete named-place index printed on the
right side of the map; [`ombreval_buildings.json`](ombreval_buildings.json)
contains the same footprint plan as inspectable data.

```text
                                      NORTH  +X
                                          ^
                                          |
               Seven Lofts          Stone Gate
                     \                  |
                      \        COSWALD'S YARD
                       \        /  Malt Passage
                        DRAPER'S REACH
                    /        \       Crookneck Lane
    Wool Gate--WICKMARKET      \  Skinners' Court   Ilvane Chapel
       |          |             \       |              |
       |       THE NEEDLE     statue--GRADINE--statue  |   BELLSTAND--Harne Gate
       |          |          Cinder Row  LANTHORN------'    Bellfoot
       |          |             |        Ford Well
       |     Burnt Court         |
       |                         |
 Chain Bridge====THE CUT====THE TALLAGE====River Cartway====MAREN'S GREEN====OLD SLUICE
                              Tally Bridge        |       church / Eel Bridge
                              Gaunt Passage       |       Moorings / Hungry Ox
                              Ferrant / Copp      |       Tanners' Slip
                                                 |
                                             River Gate       Reed Postern
                                                 |                /
                                    OUTER WHARVES / THE SERLE

       WEST +Z  <---------------------------------------------------->  -Z EAST
```

## How to read the tables

`Centre` is a useful navigation and authoring anchor. `Envelope` is an
approximate design footprint or length. Irregular streets list a polyline
instead. Neither is permission to create an overlapping box in
`assets/world/areas.json`.

`Planning ID` is the preferred stable ID when the place eventually becomes a
simulation area. Existing IDs in `areas.json` remain unchanged. Some entries,
such as the Great Rose or a public stall, may remain landmarks rather than
location areas.

## Preserved and cathedral precinct anchors

| Planning ID | Display name | Centre `(x, z)` | Envelope or point | Notes |
|---|---|---:|---|---|
| `lanthorn_interior` | Inside the Lanthorn (Great Church of Saint Ambrelle) | `(0, -12)` | existing cruciform footprint | Existing area and geometry; do not move |
| `lanthorn_grounds` | The grounds of the Lanthorn (Great Church of Saint Ambrelle) | `(0, 0)` | existing disjoint perimeter and west apron | Existing area; later precinct work must meet it cleanly |
| `great_rose` | The Great Rose | `(0, 81)` | west-front landmark | Not a ground area; the eye shares its plan point |
| `gradine` | The Gradine | `(0, 131)` | roughly `77 x 52 m` | Rebuildable paving, fixed relationship to west doors |
| `dawn_bearer_vicinity` | Next to the Dawn Bearer statue | `(-72, 190)` | existing `24 x 24 m` area | Existing statue and area; south side of approach |
| `seraph_vicinity` | Next to the Seraph statue | `(72, 190)` | existing `24 x 24 m` area | Existing statue and area; north side of approach |
| `skinners_court` | Skinners' Court | `(108, 122)` | irregular `38 x 34 m` court | North of Gradine, behind street houses |
| `ford_well` | Ford Well | `(88, 35)` | `12 x 10 m` close | New public well beside the old ford ground, outside the preserved footprint |
| `chapter_house` | The Lanthorn chapter house | about `(58, -65)` | part of preserved Fabric | Custody offices and Grey Press are inside; survey exact doors later |

## The five squares

| Planning ID | Display name | Centre `(x, z)` | Approximate envelope | Principal approaches |
|---|---|---:|---|---|
| `wickmarket` | The Wickmarket | `(-17.5, 248.5)` | bent trapezoid, about `57 x 48 m` | Wool Gate road, west approach, Draper's Reach, Needle back lanes |
| `coswalds_yard` | Coswald's Yard | `(223.5, 108.5)` | rough polygon, about `78 x 62 m` | Stone Gate road, Fabric Way, Draper's Reach, Crookneck Lane |
| `tallage` | The Tallage | `(-213.5, 63)` | Cut widening, about `83 x 60 m` | Cut west/east, River Cartway, Needle route, salt lanes |
| `marens_green` | Maren's Green | `(-213.5, -255.5)` | linked market spaces, about `74 x 62 m` | Cut west/east, River Cartway branch, Maren's Slip, Tanners' Slip |
| `bellstand` | The Bellstand | `(31.5, -178.5)` | sloping polygon, about `55 x 46 m` | Bell Way, Bellfoot Passage, Harne Gate road, Crookneck Lane |

## The Cut and its places

| Planning ID | Display name | Centre or route `(x, z)` | Envelope | Notes |
|---|---|---:|---|---|
| `the_cut` | The Cut | `(-213.5, 325.5)` to `(-213.5, -422)` | `18..24 m` overall street, `8..12 m` clear cartway | Use multiple disjoint area boxes, excluding named places |
| `chain_bridge` | The Chain Bridge | `(-213.5, 297.5)` | bridge/gallery about `42 x 14 m` | Western dry bridge at former chain-house |
| `tally_bridge` | The Tally Bridge | `(-213.5, 73.5)` | stone connection about `58 x 18 m` | Two arches over the Cut, toll-house to bonded warehouse |
| `old_sluice` | The Old Sluice | `(-213.5, -427)` | gatehouse and dry apron about `52 x 44 m` | Closes east end of Cut inside Harne Gate quarter |
| `ropewalk_cut` | The Cut ropewalk | `(-182, 182)` | narrow working strip roughly `18 x 105 m` | Petronel Roper's trade context; not necessarily a prompt area |
| `shambles` | The Shambles | `(-294.2, 219.6)` | courts and sheds about `63 x 50 m` | New fixed site down-slope and south of west Cut |

## Tallage and salt-quarter detail

| Planning ID | Display name | Centre `(x, z)` | Envelope | Relationship |
|---|---|---:|---|---|
| `toll_house` | The Tallage toll-house | `(-176.5, 73.5)` | about `34 x 42 m` | North bank; joined over Cut to warehouse |
| `bonded_warehouse` | The Tallage bonded warehouse | `(-253.5, 73.5)` | about `42 x 52 m` | South bank; controlled freight yard behind |
| `lise_copps_pawnshop` | Lise Copp's pawnshop | `(-188.8, 33.1)` | narrow shop and back room | Faces Tally Bridge from east corner |
| `ferrants_house` | Doctor Ferrant's house and study | `(-168.8, 10.1)` | town house about `18 x 30 m` | Off east corner, roof and instrument platform screened from square |
| `gaunt_house` | The old Gaunt house | `(-144.8, 8.1)` | salt hall and cellars about `48 x 52 m` | Extinct family name, active salt storage |
| `gaunt_passage` | Gaunt Passage | `(-154.8, 15.1)` | bent covered route about `55 m` long | Both mouths are hidden by doglegs; grate at midpoint |
| `gaunt_weighing_yard` | The bonded weighing yard | `(-121.8, -11.9)` | about `42 x 38 m` | Two cart gates on different lanes; escape from Ashe's cellar |

Gaunt Passage is “blind” in the sense that neither mouth nor its middle can be
seen on a straight sightline. The authoritative Second Sun gazetteer gives it
two mouths and an escape through Ashe's cellar; do not build it as a literal
single-entry dead end.

## Maren's Green and Reed Ward detail

| Planning ID | Display name | Centre `(x, z)` | Envelope | Relationship |
|---|---|---:|---|---|
| `saint_marens_church` | The Church of Saint Maren of the Reeds | `(-140.5, -265.6)` | low church about `52 x 64 m` | North side of old Cut, visible through market smoke |
| `saint_marens_churchyard` | Saint Maren's churchyard | `(-172.5, -243.6)` | irregular about `45 x 52 m` | Between church and Green; dry, crowded by old graves |
| `charnel_door` | Saint Maren's charnel door | `(-181.5, -248.6)` | small threshold vicinity | Faces Green and is legible from Eel Bridge rail |
| `saint_marens_crypt` | Saint Maren's crypt | `(-147.5, -268.6)` | beneath eastern church/yard edge | Separate interior only if made accessible |
| `alder_moorings` | The Alder Moorings | `(-272.5, -290.6)` | warehouse court about `46 x 41 m` | South side of Green; wholly dry |
| `eel_bridge` | The Eel Bridge | `(-248.5, -257.6)` | timber gallery about `12 x 30 m` | Crosses fish-market lane east–west; narrow stair constricts traffic |
| `hungry_ox` | The Hungry Ox | `(-235.5, -330.6)` | tavern about `20 x 28 m` | East/south edge of Green, song audible into nearby lane |
| `tanners_slip` | Tanners' Slip | `(-295.5, -228.6)` | dogleg lane about `135 m` long | Behind/south of Green, enclosed and dry |
| `eelback_alley` | Eelback Alley | `(-275.5, -328.6)` | humped alley about `70 m` long | Behind smokehouses and tavern service yards |
| `marens_slip` | Maren's Slip | `(-280.5, -318.6)` to `(-318.5, -374.5)` | stepped foot/handcart way | Connects Green to Reed Postern, no waterfront vista |
| `brine_cellar` | The empty brine-rotted cellar | `(-315.5, -288.6)` | cellar off Tanners' Slip | False meeting site; nobody loved by the cell comes here |

## Northern and western trade web

| Planning ID | Display name | Centre or route `(x, z)` | Envelope | Notes |
|---|---|---:|---|---|
| `drapers_reach` | The Draper's Reach | `(22, 241)` to `(170, 174)` | covered gallery, about `165 m` along a bent hall frontage | Runs between Wickmarket and Coswald's Yard, bending north of the fixed Seraph court; tenter loft above |
| `tenterhook_lane` | Tenterhook Lane | `(152, 172)` to `(114, 204)` | open working lane, `4..5 m` clear | Fullers' frames receive sun and moving air on the halls' open south-east side |
| `the_needle` | The Needle | `(-52.5, 217)` to `(-178.5, 88.5)` | crooked pedestrian route; midpoint pinch `1.05..1.2 m` | Wickmarket back lanes toward Tallage |
| `cinder_row` | Cinder Row | `(-48, 204)` to `(-87, 84)` via `(-91, 144)` | fire-rebuilt street, `5..7 m` | South-west approach to Gradine |
| `burnt_court` | Burnt Court | `(-123.4, 166.4)` | court about `24 x 21 m` | Reached from Cinder Row through fire-scarred passage |
| `glaziers_guildhall` | The glaziers' guildhall | `(-104, 232)` | workshop hall about `26 x 38 m` | At Cinder Row's upper end, north of the Dawn Bearer court; Sparr workshop behind it to the north |
| `masons_lodge` | The masons' lodge | `(156, 150)` | hall and porch about `46 x 34 m` | Off Coswald's Yard's south-west corner, facing work ground and tower view |
| `malt_passage` | Malt Passage | `(170, 118)` | covered passage about `42 m` | Under the malt-house south of the masons' lodge |
| `crookneck_lane` | Crookneck Lane | `(188.5, 42)` via `(100, -14)` to `(95, -80)` | two hard bends, about `170 m` | North-east route from builders' quarter toward Ilvane lane/Bell ward |
| `osanne_vells_stall` | Osanne Vell's stall | `(12.6, 245)` | north row of Wickmarket | Landmark and NPC station, normally not its own area |

## Eastern places

| Planning ID | Display name | Centre or route `(x, z)` | Envelope | Notes |
|---|---|---:|---|---|
| `ilvane_chapel` | The Ilvane Chapel | `(122.5, -64.4)` | chapel about `34 x 52 m` | Mortared building with lane along north wall |
| `ilvane_anchorhold` | The Ilvane anchorhold | `(141.5, -64.4)` | cell in north wall | Squint opens north onto lane; Dame Aldith remains mostly unseen |
| `bellfoot_passage` | Bellfoot Passage | `(35.8, -167)` | passage beneath external tower stair | Opens immediately into Bellstand |
| `bellstand_tower` | The Bellstand watch-bell tower | `(44.8, -189)` | base about `22 x 25 m` | Off-centre on north/east edge, not in square centre |
| `colm_stone` | Colm's stone | `(32.8, -177)` | worn stone at tower door | Unattested tale; not a supernatural prop |
| `bellfounders_yard` | Bellfounders' Yard | `(108.5, -339.5)` | new industrial yard about `78 x 64 m` | Smoke/noise close to east wall and Harne road |

## Gates, storage, and outside anchors

| Planning ID | Display name | Centre `(x, z)` | Envelope | Direction and use |
|---|---|---:|---|---|
| `wool_gate` | The Wool Gate | `(-24.5, 357)` | west gate complex | Brede, Combs, wool, honey, hides, pilgrims |
| `stone_gate` | The Stone Gate | `(346.5, 94.5)` | north gate complex | stone, lime, timber; Lantern Road to Ostrelle branches beyond |
| `harne_gate` | The Harne Gate | `(10.5, -465.5)` | east gate complex | Harne and downstream road traffic |
| `river_gate` | The River Gate | `(-353.5, -94.5)` | main south gate complex | outer wharves and all heavy river freight |
| `reed_postern` | The Reed Postern | `(-318.5, -374.5)` | narrow south-east postern | wharf hands, fish carriers, funerals, handcarts only |
| `seven_lofts` | Seven Lofts | `(252, 234.5)` | granary compound about `105 x 82 m` | Near Wool and Stone gate routes; defended food storage |
| `outer_wharves` | The outer Serle wharves | `(-399, -115.5)` | off-wall strip from about `z = 84..-315` | Outside playable wall; cranes, quays, barges, one sun |
| `serle` | The Serle | about `x = -434` | river runs west `+z` to east `-z` | Never enters the present city |

## Reserved ordinary parish sites

The city needs more than three congregations, but core lore deliberately leaves
the other parishes unnamed. These footprints are reserved so later lore can
add dedications without displacing established places.

| Reserve | Centre `(x, z)` | Congregation and economy to develop later |
|---|---:|---|
| West parish reserve | `(73.5, 301)` | gate households, chandlers, wool carriers, lodging keepers |
| North-east parish reserve | `(231, -210)` | wall households, carters, small metal trades |
| West Cut parish reserve | `(-154, 276.5)` | rope-makers, shambles workers, porters, poor tenants |
| River Ward parish reserve | `(-292.8, -24.5)` | gate workers, wharf porters, stable hands, freight families |

They are authoring reservations, not named places and not simulation areas.
Their eventual churches should be smaller than Saint Maren's unless later lore
provides a material reason otherwise.

## Principal route polylines

| Route | Authoring points `(x, z)` | Clear width and traffic |
|---|---|---|
| West gate approach | `(-24.5,357) -> (-17.5,294) -> (-17.5,248.5) -> (-7,199.5) -> (0,178) -> (0,157)` | `6..8 m`, narrowing to final processional funnel |
| Stone/Fabric Way | `(346.5,94.5) -> (252,101.5) -> (223.5,108.5) -> (168,72) -> (80.5,66.5) -> (72,80)` | `6..7 m`, heavy building carts to service lane |
| River Cartway | `(-353.5,-94.5) -> (-290.5,-91) -> (-213.5,-87.5)` | `7..9 m`, no steps; splits onto Cut |
| Bell Way | `(78,-112) -> (84,-118) -> (49,-143.5) -> (25.8,-174)` | `4.5..6 m`, two reveals and Bellfoot entrance |
| Harne Gate road | `(25.8,-174) -> (56,-252) -> (24.5,-353.5) -> (10.5,-465.5)` | `5.5..7 m`, road traffic and Bellfounders spur |
| Reed route | `(-318.5,-374.5) -> (-287,-339.5) -> (-280.5,-318.6) -> (-225.5,-273.6)` | `3..4 m`, foot and handcart traffic |

## Approximate walks

Times assume an unhurried resident in ordinary traffic, not a running player.

| From | To | Distance | Street time |
|---|---|---:|---:|
| Gradine | Wickmarket | about `170 m` | `3 min` |
| Gradine | Coswald's Yard | about `210 m` | `3..4 min` |
| Gradine | Bellstand | about `230 m` | `4 min` |
| Gradine | Tallage | about `275 m` | `4..5 min` |
| Gradine | Maren's Green | about `390 m` | `6 min` |
| Tallage | Maren's Green along Cut | about `320 m` | `5 min` |
| River Gate | Tallage | about `250 m` | `4 min` with no load |
| River Gate | Maren's Green | about `295 m` | `5 min` with no load |
| Wool Gate | Harne Gate | about `870 m` by streets | `13..15 min` |
| Stone Gate | River Gate | about `770 m` by streets | `12..13 min` |

Loaded carts, crowds, bridge constrictions, market days, and processions can
double these times. That friction is part of the city's scale.
