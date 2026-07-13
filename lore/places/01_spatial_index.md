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
| `wickmarket` | The Wickmarket | `(-25, 355)` | bent trapezoid, about `82 x 68 m` | Wool Gate road, west approach, Draper's Reach, Needle back lanes |
| `coswalds_yard` | Coswald's Yard | `(255, 155)` | rough polygon, about `112 x 88 m` | Stone Gate road, Fabric Way, Draper's Reach, Crookneck Lane |
| `tallage` | The Tallage | `(-305, 90)` | Cut widening, about `118 x 86 m` | Cut west/east, River Cartway, Needle route, salt lanes |
| `marens_green` | Maren's Green | `(-305, -365)` | linked market spaces, about `105 x 88 m` | Cut west/east, River Cartway branch, Maren's Slip, Tanners' Slip |
| `bellstand` | The Bellstand | `(45, -255)` | sloping polygon, about `78 x 66 m` | Bell Way, Bellfoot Passage, Harne Gate road, Crookneck Lane |

## The Cut and its places

| Planning ID | Display name | Centre or route `(x, z)` | Envelope | Notes |
|---|---|---:|---|---|
| `the_cut` | The Cut | `(-305, 465)` to `(-305, -605)` | `18..24 m` overall street, `8..12 m` clear cartway | Use multiple disjoint area boxes, excluding named places |
| `chain_bridge` | The Chain Bridge | `(-305, 425)` | bridge/gallery about `42 x 14 m` | Western dry bridge at former chain-house |
| `tally_bridge` | The Tally Bridge | `(-305, 105)` | stone connection about `58 x 18 m` | Two arches over the Cut, toll-house to bonded warehouse |
| `old_sluice` | The Old Sluice | `(-305, -610)` | gatehouse and dry apron about `52 x 44 m` | Closes east end of Cut inside Harne Gate quarter |
| `ropewalk_cut` | The Cut ropewalk | `(-260, 260)` | narrow working strip roughly `18 x 150 m` | Petronel Roper's trade context; not necessarily a prompt area |
| `shambles` | The Shambles | `(-395, 315)` | courts and sheds about `90 x 72 m` | New fixed site down-slope and south of west Cut |

## Tallage and salt-quarter detail

| Planning ID | Display name | Centre `(x, z)` | Envelope | Relationship |
|---|---|---:|---|---|
| `toll_house` | The Tallage toll-house | `(-268, 105)` | about `34 x 42 m` | North bank; joined over Cut to warehouse |
| `bonded_warehouse` | The Tallage bonded warehouse | `(-345, 105)` | about `42 x 52 m` | South bank; controlled freight yard behind |
| `lise_copps_pawnshop` | Lise Copp's pawnshop | `(-262, 43)` | narrow shop and back room | Faces Tally Bridge from east corner |
| `ferrants_house` | Doctor Ferrant's house and study | `(-242, 20)` | town house about `18 x 30 m` | Off east corner, roof and instrument platform screened from square |
| `gaunt_house` | The old Gaunt house | `(-218, 18)` | salt hall and cellars about `48 x 52 m` | Extinct family name, active salt storage |
| `gaunt_passage` | Gaunt Passage | `(-228, 25)` | bent covered route about `55 m` long | Both mouths are hidden by doglegs; grate at midpoint |
| `gaunt_weighing_yard` | The bonded weighing yard | `(-195, -2)` | about `42 x 38 m` | Two cart gates on different lanes; escape from Ashe's cellar |

Gaunt Passage is “blind” in the sense that neither mouth nor its middle can be
seen on a straight sightline. The authoritative Second Sun gazetteer gives it
two mouths and an escape through Ashe's cellar; do not build it as a literal
single-entry dead end.

## Maren's Green and Reed Ward detail

| Planning ID | Display name | Centre `(x, z)` | Envelope | Relationship |
|---|---|---:|---|---|
| `saint_marens_church` | The Church of Saint Maren of the Reeds | `(-235, -382)` | low church about `52 x 64 m` | North side of old Cut, visible through market smoke |
| `saint_marens_churchyard` | Saint Maren's churchyard | `(-267, -360)` | irregular about `45 x 52 m` | Between church and Green; dry, crowded by old graves |
| `charnel_door` | Saint Maren's charnel door | `(-276, -365)` | small threshold vicinity | Faces Green and is legible from Eel Bridge rail |
| `saint_marens_crypt` | Saint Maren's crypt | `(-242, -385)` | beneath eastern church/yard edge | Separate interior only if made accessible |
| `alder_moorings` | The Alder Moorings | `(-367, -407)` | warehouse court about `66 x 58 m` | South side of Green; wholly dry |
| `eel_bridge` | The Eel Bridge | `(-343, -374)` | timber gallery about `12 x 30 m` | Crosses fish-market lane east–west; narrow stair constricts traffic |
| `hungry_ox` | The Hungry Ox | `(-330, -447)` | tavern about `20 x 28 m` | East/south edge of Green, song audible into nearby lane |
| `tanners_slip` | Tanners' Slip | `(-390, -345)` | dogleg lane about `135 m` long | Behind/south of Green, enclosed and dry |
| `eelback_alley` | Eelback Alley | `(-370, -445)` | humped alley about `70 m` long | Behind smokehouses and tavern service yards |
| `marens_slip` | Maren's Slip | `(-375, -435)` to `(-455, -535)` | stepped foot/handcart way | Connects Green to Reed Postern, no waterfront vista |
| `brine_cellar` | The empty brine-rotted cellar | `(-415, -405)` | cellar off Tanners' Slip | False meeting site; nobody loved by the cell comes here |

## Northern and western trade web

| Planning ID | Display name | Centre or route `(x, z)` | Envelope | Notes |
|---|---|---:|---|---|
| `drapers_reach` | The Draper's Reach | `(35, 320)` to `(210, 185)` | covered gallery, about `190 m` along a bent hall frontage | Runs between Wickmarket and Coswald's Yard; tenter loft above |
| `tenterhook_lane` | Tenterhook Lane | `(95, 220)` to `(-35, 185)` | open working lane, `4..5 m` clear | Fullers' frames receive sun and moving air south of cloth halls |
| `the_needle` | The Needle | `(-75, 310)` to `(-270, 120)` | crooked pedestrian route; midpoint pinch `1.05..1.2 m` | Wickmarket back lanes toward Tallage |
| `cinder_row` | Cinder Row | `(-80, 285)` to `(-105, 115)` via `(-135, 205)` | fire-rebuilt street, `5..7 m` | South-west approach to Gradine |
| `burnt_court` | Burnt Court | `(-172, 232)` | court about `34 x 30 m` | Reached from Cinder Row through fire-scarred passage |
| `glaziers_guildhall` | The glaziers' guildhall | `(-140, 235)` | workshop hall about `26 x 38 m` | On Cinder Row; Sparr workshop adjacent |
| `masons_lodge` | The masons' lodge | `(205, 150)` | hall and porch about `46 x 34 m` | South edge of Coswald's Yard, facing work ground and tower view |
| `malt_passage` | Malt Passage | `(232, 112)` | covered passage about `42 m` | Under malt-house at Coswald's Yard |
| `crookneck_lane` | Crookneck Lane | `(205, 60)` via `(165, -15)` to `(105, -85)` | two hard bends, about `175 m` | North-east route from builders' quarter toward Ilvane lane/Bell ward |
| `osanne_vells_stall` | Osanne Vell's stall | `(18, 350)` | north row of Wickmarket | Landmark and NPC station, normally not its own area |

## Eastern places

| Planning ID | Display name | Centre or route `(x, z)` | Envelope | Notes |
|---|---|---:|---|---|
| `ilvane_chapel` | The Ilvane Chapel | `(175, -92)` | chapel about `34 x 52 m` | Mortared building with lane along north wall |
| `ilvane_anchorhold` | The Ilvane anchorhold | `(193, -92)` | cell in north wall | Squint opens north onto lane; Dame Aldith remains mostly unseen |
| `bellfoot_passage` | Bellfoot Passage | `(55, -248)` | passage beneath external tower stair | Opens immediately into Bellstand |
| `bellstand_tower` | The Bellstand watch-bell tower | `(62, -270)` | base about `22 x 25 m` | Off-centre on north/east edge, not in square centre |
| `colm_stone` | Colm's stone | `(52, -258)` | worn stone at tower door | Unattested tale; not a supernatural prop |
| `bellfounders_yard` | Bellfounders' Yard | `(155, -485)` | new industrial yard about `78 x 64 m` | Smoke/noise close to east wall and Harne road |

## Gates, storage, and outside anchors

| Planning ID | Display name | Centre `(x, z)` | Envelope | Direction and use |
|---|---|---:|---|---|
| `wool_gate` | The Wool Gate | `(-35, 510)` | west gate complex | Brede, Combs, wool, honey, hides, pilgrims |
| `stone_gate` | The Stone Gate | `(495, 135)` | north gate complex | stone, lime, timber; Lantern Road to Ostrelle branches beyond |
| `harne_gate` | The Harne Gate | `(15, -665)` | east gate complex | Harne and downstream road traffic |
| `river_gate` | The River Gate | `(-505, -135)` | main south gate complex | outer wharves and all heavy river freight |
| `reed_postern` | The Reed Postern | `(-455, -535)` | narrow south-east postern | wharf hands, fish carriers, funerals, handcarts only |
| `seven_lofts` | Seven Lofts | `(360, 335)` | granary compound about `105 x 82 m` | Near Wool and Stone gate routes; defended food storage |
| `outer_wharves` | The outer Serle wharves | `(-570, -165)` | off-wall strip from about `z = 120..-450` | Outside playable wall; cranes, quays, barges, one sun |
| `serle` | The Serle | about `x = -620` | river runs west `+z` to east `-z` | Never enters the present city |

## Reserved ordinary parish sites

The city needs more than three congregations, but core lore deliberately leaves
the other parishes unnamed. These footprints are reserved so later lore can
add dedications without displacing established places.

| Reserve | Centre `(x, z)` | Congregation and economy to develop later |
|---|---:|---|
| West parish reserve | `(105, 430)` | gate households, chandlers, wool carriers, lodging keepers |
| North-east parish reserve | `(330, -300)` | wall households, carters, small metal trades |
| West Cut parish reserve | `(-220, 395)` | rope-makers, shambles workers, porters, poor tenants |
| River Ward parish reserve | `(-420, -35)` | gate workers, wharf porters, stable hands, freight families |

They are authoring reservations, not named places and not simulation areas.
Their eventual churches should be smaller than Saint Maren's unless later lore
provides a material reason otherwise.

## Principal route polylines

| Route | Authoring points `(x, z)` | Clear width and traffic |
|---|---|---|
| West gate approach | `(-35,510) -> (-25,420) -> (-25,355) -> (-10,285) -> (0,225) -> (0,157)` | `6..8 m`, narrowing to final processional funnel |
| Stone/Fabric Way | `(495,135) -> (360,145) -> (255,155) -> (180,120) -> (115,95) -> (72,80)` | `6..7 m`, heavy building carts to service lane |
| River Cartway | `(-505,-135) -> (-415,-130) -> (-305,-125)` | `7..9 m`, no steps; splits onto Cut |
| Bell Way | `(78,-112) -> (105,-165) -> (70,-205) -> (45,-255)` | `4.5..6 m`, two reveals and Bellfoot entrance |
| Harne Gate road | `(45,-255) -> (80,-360) -> (35,-505) -> (15,-665)` | `5.5..7 m`, road traffic and Bellfounders spur |
| Reed route | `(-455,-535) -> (-410,-485) -> (-375,-435) -> (-320,-390)` | `3..4 m`, foot and handcart traffic |

## Approximate walks

Times assume an unhurried resident in ordinary traffic, not a running player.

| From | To | Distance | Street time |
|---|---|---:|---:|
| Gradine | Wickmarket | about `240 m` | `4 min` |
| Gradine | Coswald's Yard | about `300 m` | `5 min` |
| Gradine | Bellstand | about `330 m` | `5..6 min` |
| Gradine | Tallage | about `390 m` | `6..7 min` |
| Gradine | Maren's Green | about `560 m` | `9 min` |
| Tallage | Maren's Green along Cut | about `455 m` | `7 min` |
| River Gate | Tallage | about `360 m` | `6 min` with no load |
| River Gate | Maren's Green | about `420 m` | `7 min` with no load |
| Wool Gate | Harne Gate | about `1.25 km` by streets | `19..22 min` |
| Stone Gate | River Gate | about `1.1 km` by streets | `17..19 min` |

Loaded carts, crowds, bridge constrictions, market days, and processions can
double these times. That friction is part of the city's scale.
