## It's at least 2011 now, baby

*Why Ombreval looks like 1996, whether Bevy or the GPU is to blame (spoiler: neither),
and a concrete, ordered plan to drag it to 2011 — with a marked trail onward to 2020.*

Reference targets, all in `lore/inspiration_images/places/`:
- `the_cut/the_cut_001.png` — a street that is a *canyon*: 4–5 storey jettied façades, beams and
  hoists crossing overhead, awnings, carts, sacks, mud-worn cobbles, overcast sky, distant spires.
- `bellfoot_passage/bellfoot_passage_001.png` — a covered passage *under* a building, stair climbing
  the wall beside it, lantern, posted notices, vendor clutter in the dark, bright square framed in the arch.
- `the_bellstand/the_bellstand_001.png` — an open belfry with the **bell visible in its arch**, a
  timber gallery bridging to the neighbouring roofs at third-floor height, balconies full of people,
  chimney smoke, layered rooflines.

Current state (session 202 screenshot): single-to-two-storey textured prisms under a naive blue sky,
windows that are pale stickers, one cobbled ribbon in a sea of bare dirt, empty skyline. The gap is
real but it is *not* an engine gap. It decomposes into five very fixable deficits.

## Today

**Nothing lives in the space.** The mid-ground of the reference is *stuff*: carts, awnings,
hoists, laundry, signage, barrels, crowds, smoke. Ombreval's street furniture inventory for a
1.2 × 1.0 km city is: 91 plan fixtures (mostly wells, stalls, cranes), one ropewalk, three wharf
cranes, 15 wharf sheds. The skyline inventory is: zero chimneys, five bare tower prisms, no smoke, no
birds, no banners. Verticality inventory: one external stair (the Bellstand's, mod.rs:1262) and the
plan's covered bridges rendered as floating slabs on two piers (mod.rs:1460). There is no overhead
layer at all: no jetties, no spanning galleries, no arches over streets — the exact thing all three
inspiration images are *about*.

**The city is low.** The plan generator (scripts/generate_top_down_map.py:732) rolls
`3±1 storeys inside the 360 m radial, 2±1 outside, clamped to 4`. Histogram over the shipped JSON:
280 × 1-storey, 1,116 × 2, 883 × 3, 285 × 4, 2 × 5+. Median street width 4 m against median eave
~6.75 m gives a height:width ratio of ~1.7 — the references live at 3–4. Even with perfect materials
the space cannot canyon. (Roads *do* pinch to 1.2 m, so the plan's horizontal fabric is right; it's
the vertical dimension that's timid.)

## Status: implemented (2026-07-18)

Everything below is in `src/city/mod.rs` on `develop`; 199 tests green, both guard rails
(`no_walkable_cell_is_solid`, `batched_city_keeps_render_entity_count_bounded`) hold.

- **Real timber frames**: `WallKind::HalfTimber` storeys (plain, jettied, and the elevated
  bridge/malt-house shells) now render as plain plaster with a geometric skeleton batched into
  one dark-wood mesh (`add_timber_framing`): corner posts, sill/storey/plate rails threaded
  between the window rows, 1.2–1.8 m hash-jittered studs that skip openings, diagonal corner
  braces (two orientations + both, by hash). Members ride 0.12–0.13 m proud with buried backs;
  gables went to plaster with the storeys. The painted `half_timber` grid texture is no longer
  used on buildings.
- **Covered passages** (`build_covered_passages`): the three bridge upper storeys and the
  malt-house get a joist-and-board ceiling, a fascia over each mouth, 2–3 always-burning hanging
  lanterns (batched meshes + shadowless `PointLight`s, emissive `lantern_glass` material), posted
  notices on both faces of the spine piers, and limestone doorstep strips. (The Tally Bridge's
  ends are buried inside the bonded warehouse and toll-house — it really is an "upper passage"
  between them — so its mouth dressing is hidden; Chain, Eel and Malt Passage read fully.)
- **Arcades** (`build_square_arcades`): 19 strips on buildings fronting the six `square` sites —
  posts at ~2.4 m spacing 1.35 m out (skipping doorways), a beam, and a slate pentice from the
  façade at 3.78 m down over the walk. *Deviation from the sketch:* the posts carry a pentice
  hood rather than the jettied upper wall, and they do **not** collide — the baked navigation
  predates them (prop precedent), and the hood clears head height, so nav/doors/colliders are
  untouched. The toll-house's west arcade runs *inside* the Tally passage, which is accidental
  and great.
- **External stairs & balconies** (`build_yard_stairs`): ~36 flights on a hash-picked tenth of
  the 2+-storey ordinary fabric, each placed only where eight probe points clear every other
  footprint, every road (half-width + 0.8 m), fixtures, squares, and the curtain wall. Stringers,
  treads, handrail, balusters, railed landing, full-height posts, and a dark first-floor door.
  *Deviation:* the flight is non-colliding scenery like the props; only the landing platform
  (3.1 m up, above the walk band) gets a collider so a flying player can perch.

Still open, tracked in `features/its_at_least_2011_now_baby.md`: plan-generator storey increase
(a map revision), awnings/laundry, smoke, AutoExposure, volumetric fog.

## What was done earlier (the more-verticality branch)

- **Jetties**: convex half-timber quads of 2+ storeys (~80% of them) step out 0.34 m per storey
  (capped at two steps), with mitred per-storey footprints, dark soffit rings underneath, and
  bressummer beams on every step face. Openings re-addressed per storey band so windows sit in the
  jettied walls correctly. Roof rides the top band's footprint. Ground floor stays exactly on the
  cadastral line — nav, doors, colliders untouched.
- **The Bellstand belfry** (§5.5 as promised): shaft stops at 23.5 m, then a walkable stage
  (landable, with parapet colliders), corner + mid piers, entablature, corner pinnacles, **a great
  bronze bell you can see from half the city and stand under** (crown/shoulder/waist/flare/lip/
  clapper from primitives, headstock beam), slate spire with finial to ~44 m. `exp6_on_belfry` is
  taken standing on the stage.
- Saint Maren's and the four parish reserve towers got small open bell stages with visible bells —
  five believable landmarks instead of five prisms.
- **23 street galleries**: half-timbered bridges with slate hoods spanning 2–5.5 m streets at
  4.55 m clearance, spawned only where 2+-storey buildings flank both sides (probed at three
  setbacks and seated into the façades). Walkable on top. `exp6_gallery_bellway` shows one casting
  its shadow across Bell Way.
- **2,380 doorway prop clusters**: barrels (coopered, iron hoops), crate stacks, grain sacks,
  firewood ricks, hanging trade signs on iron brackets — all *batched into five meshes* (one per
  material) via new in-mesh lathe/box emitters, not entities. No colliders on props: the baked
  navigation predates them, so they are scenery exactly the way NPCs already are.

**Guard rails respected** (and they caught real mistakes):
- `no_walkable_cell_is_solid` failed when props had colliders → props are now non-blocking, nav
  stays honest. The *right* long-term fix is promoting key props to plan fixtures + re-bake.
- `batched_city_keeps_render_entity_count_bounded` (< 1,500 entities) failed when props were
  spawned as ~7k primitive clones → rewrote them as batched MeshData. The discipline held.

**What I deliberately did not do**: geometric timber framing over plaster (see "Real timber frames" below), gable-to-street ridge orientation, storey
count increase in the plan generator, awnings/laundry,
smoke, AutoExposure (recommended; needs a compensation curve so night stays night), volumetric fog
(compiled in, one component away), NPC bodies.

## Things before verticality

Real timber frames + per-building variation (kill the wallpaper)

For `WallKind::HalfTimber` buildings, stop relying on the painted grid:
- Swap the upper-storey wall material to **plain plaster**;
- Overlay geometric members (instanced dark-wood boxes, 12–16 cm proud of the wall): corner posts,
  storey rails at each 3.15 m line, studs every 1.2–1.8 m (hash-jittered), diagonal braces at corners
  and beside openings (the classic K/Z patterns, 2–3 variants by hash), all snapped to actual storey
  heights and actual window positions — structure, not stencil.
- **Per-building tint variation** for *all* wall kinds: multiply the batched vertices' colour by a
  hash-derived ±8% value / ±4° hue jitter via `ATTRIBUTE_COLOR` (StandardMaterial multiplies vertex
  colour) — one line in `add_extruded_walls`, breaks the 1,100-clones monotony at zero material cost.
- **Grime**: darken the vertex colour toward the ground (y=0 → ×0.72, y≥3 → ×1.0) and slightly darken
  the top 0.3 m under the eaves. Vertex-level fake AO/weathering, free, and exactly what the
  references show (dark bases, streaked walls).

## Verticality: the overhead layer

This is the heart of my ask — all three inspiration images are about **occupied space above the
street**:

- **Jetties**: for half-timber/plaster buildings of 2+ storeys on streets ≤ 8 m wide, step each upper
  storey out 0.30–0.45 m over the street face(s) (the façades that border a road). Underside gets
  joist ends (small instanced boxes) every ~0.8 m. Two 3-storey jettied rows over a 4 m lane turn
  "corridor between boxes" into *the cut*. Ground-floor footprint (nav, doors, colliders at street
  level) is untouched — the cantilever starts at 3.15 m, above head height. Collision: extend the
  building collider only above the jetty base so flying players can perch.
- **Spanning galleries/bridges**: find street segments ≤ 4.5 m wide flanked by 3-storey buildings
  (the road polylines + building edge adjacency are all in the plan), and with ~1/20 hash odds span a
  **timber gallery** at 2nd-floor height: floor slab, side rails or half-timbered walls with window
  modules, shed or gable roof, 2–3 m long. The Bellstand reference shows exactly this. Clearance
  ≥ 3.2 m so nothing at street level cares; add a collider so it's walkable-on when flying.
- **Covered passages**: where the plan already routes roads *under* "bridge"-use buildings
  (base_y 4.25, mod.rs:728) dress the underside: arch face boards, 2–3 hanging lanterns
  (PointLight, shadow off), posted-notice quads on the flanking walls, stone doorstep strips. That's
  bellfoot_passage_001 nearly verbatim.
- **Arcades**: buildings fronting the five squares get a ground-floor arcade strip: posts at 2.4 m
  spacing carrying the (jettied) upper wall, walkable colonnade behind. (Collision: posts only.)
- **External stairs & balconies**: on 2+ storey buildings in yards/wide spots, a hash-picked 10%
  get a straight timber stair up the gable side to a first-floor landing/balcony with rails
  (the Bellstand image, left edge). Colliders: yes (they're in yards, not on nav routes — verify
  against `navigation.json` clearance the way fixtures are tested in plan.rs tests).
- **THE BELLSTAND** (named_bellstand_tower, 22×25 m footprint, eave 31.5 m): rebuild the top as an
  **open belfry** — from ~24 m: corner piers + arched openings on all four faces (reuse the
  `half_arch` torus trick from scene.rs:113), a **real bell** hanging in the opening (lathe profile:
  stacked cylinder segments or a revolved mesh, bronze material, 1.6 m mouth), headstock beam and
  wheel, a timber gallery balcony ringing the stage at 24 m (rails + brackets), pyramidal spire with
  four lucarnes above, finial. The existing external stair already climbs to it. Sight-line pay-off
  from half the city; and the sim's `town_bell` sound gets a *visible source* (later: swing the bell
  transform when it rings — the hook already exists as the `sound town_bell` drive action).
- **Parish towers & Saint Maren's**: same treatment at smaller scale — open stage, visible bell,
  slate pyramid. Five believable landmarks instead of five prisms.
