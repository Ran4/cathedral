# It's at least 2011 now, baby

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

---

## 1. Diagnosis — what the screenshot actually shows

Everything below is verifiable in `src/city/mod.rs` (the whole outdoor city is ~2,500 lines there
plus `monuments.rs`/`water.rs`).

**D1. Buildings are extruded footprints and nothing else.**
`add_extruded_walls()` (mod.rs:782) walls straight up from the cadastral polygon to
`levels × 3.15 m + 0.45` (`building_verticals`, mod.rs:727), then `add_building_roof()` (mod.rs:818)
puts a gable exactly flush with the wall plane. There is no eaves overhang, no plinth (except under
plaster/half-timber), no cornice, no quoins, no chimney, no dormer, nothing breaking the silhouette.
A building's entire geometric vocabulary is: prism + wedge.

**D2. Windows are stickers.** `add_facade_openings()` (mod.rs:894) pastes 1.0×1.35 m quads 3.5 cm
in front of the wall, in a uniform grid (same count per edge, same height per floor), all sharing one
dark-glass material whose `perceptual_roughness: 0.3` mirror-reflects the bright sky — which is why
they read as *pale blue decals* in the screenshot. Real windows are **holes**: a dark reveal, a frame,
a sill shadow. The eye keys on that depth instantly, at any distance, and its absence is the single
biggest "1996" tell in the image.

**D3. One texture per material for the whole city, and the timber is painted on.**
Every wall of a kind shares one 1024² tile repeated every 7 m with a single global tint
(mod.rs:258–401). No per-building hue/value variation, no grime gradient at the base, no streaking
under sills — so 1,100 plaster buildings are *the same wall*. Worse, the half-timber frame is baked
into the albedo: at any glancing angle the flatness is obvious, and the painted timber grid (~2.3 m
bays) doesn't align with storeys, corners, or openings. In the reference, timber framing is
*structure*: posts at corners, rails at floor lines, braces around openings, and it casts shadows.

**D4. Lighting is physically bright but perceptually flat.**
The good news: the game already runs Bevy's physical `Atmosphere` sky with `RAW_SUNLIGHT`
(scene.rs:1101), ACES tonemapping, EV 12.8, an atmosphere-driven environment map
(controller.rs:450–462), and squared-falloff distance fog (scene.rs:1200). That's why it doesn't look
*wrong*, just dead:
- **No ambient occlusion of any kind.** No SSAO on the camera, no baked AO, no vertex AO, and a flat
  `GlobalAmbientLight` of 300 lux (scene.rs:93) that fills every crevice with identical light. Corners,
  eaves, alley mouths, door reveals — all render at the same brightness as open wall. AO is *the*
  cheap realism multiplier and we have zero sources of it.
- **The sun passes through the zenith at noon** (`drive_sun`, smart_actors/clock.rs:88-95: elevation
  = cos of day fraction → 90° at 12:00). Midday screenshots get flat top-down light with 1-metre
  shadows, the worst possible modelling light. Real cities at 45–50° latitude never see the sun above
  ~65°; artists shoot at 20–35°.
- **Shadow resolution is spent on a 520 m range from a 2048² map** (scene.rs:1113, Bevy default
  size). At street distances the cascade texel density is mush; small geometry couldn't cast crisp
  shadows even if it existed.
- 4× MSAA (Bevy default) and nothing else: no bloom, so emissives and the sky never bleed; no SMAA/TAA
  option.

**D5. Nothing lives in the space.** The mid-ground of the reference is *stuff*: carts, awnings,
hoists, laundry, signage, barrels, crowds, smoke. Ombreval's street furniture inventory for a
1.2 × 1.0 km city is: 91 plan fixtures (mostly wells, stalls, cranes), one ropewalk, three wharf
cranes, 15 wharf sheds. The skyline inventory is: zero chimneys, five bare tower prisms, no smoke, no
birds, no banners. Verticality inventory: one external stair (the Bellstand's, mod.rs:1262) and the
plan's covered bridges rendered as floating slabs on two piers (mod.rs:1460). There is no overhead
layer at all: no jetties, no spanning galleries, no arches over streets — the exact thing all three
inspiration images are *about*.

**D6. The city is low.** The plan generator (scripts/generate_top_down_map.py:732) rolls
`3±1 storeys inside the 360 m radial, 2±1 outside, clamped to 4`. Histogram over the shipped JSON:
280 × 1-storey, 1,116 × 2, 883 × 3, 285 × 4, 2 × 5+. Median street width 4 m against median eave
~6.75 m gives a height:width ratio of ~1.7 — the references live at 3–4. Even with perfect materials
the space cannot canyon. (Roads *do* pinch to 1.2 m, so the plan's horizontal fabric is right; it's
the vertical dimension that's timid.)

That's the whole illness. Note what is *not* on the list: triangle counts, texture resolution,
draw-call budgets, Rust, or Bevy.

---

## 2. Is Bevy the limitation?

**No — not between here and 2011, and mostly not between here and 2020.** Receipts:

Already compiled into the shipped binary (the `3d` umbrella feature pulls `bevy_pbr`,
`bevy_core_pipeline`, `bevy_anti_alias`, `bevy_post_process`, `smaa_luts`, `tonemapping_luts`) and
simply **unused**:

| Feature | Bevy component | Cost to adopt |
|---|---|---|
| SSAO | `ScreenSpaceAmbientOcclusion` on the camera (+ `Msaa::Off`) | one line + retune ambient |
| Bloom | `Bloom::NATURAL` on the camera | one line |
| SMAA / TAA / FXAA | `Smaa`, `TemporalAntiAliasing`, `Fxaa` components | one line (needed once MSAA goes off for SSAO) |
| Volumetric fog + god rays | `VolumetricFog` on camera, `VolumetricLight` on the sun | two lines + tuning |
| Screen-space reflections | `ScreenSpaceReflections` (experimental) | wet streets, canal |
| Higher-res shadows | `DirectionalLightShadowMap { size: 4096 }` resource | one line |
| Better cascade split | `CascadeShadowConfigBuilder` already used (scene.rs:1113) | retune 3 numbers |
| Parallax mapping | `StandardMaterial::depth_map` | needs height maps authored |
| Normal/roughness/AO maps | plain `StandardMaterial` fields | needs maps authored (§5.3) |

Opt-in cargo features when we want them: `experimental_pbr_pcss` (soft penumbra shadows),
`bevy_solari` (real raytraced GI — needs an RT GPU), `meshlet` (Nanite-style virtual geometry, wants
preprocessed meshes), `dlss` (NVIDIA upscaling). None are needed for 2011.

What Bevy genuinely does **not** give you (and how much it matters here):

- **No baked lighting pipeline.** No lightmapper, no light probes, no precomputed GI. The 2011 look
  (Skyrim) leaned on baked/hemisphere ambience; we approximate with atmosphere env-map + SSAO +
  emissive cheats, which is honestly enough for exteriors. Interiors at night will feel it. (Bevy has
  a `Lightmap` component — it can *consume* lightmaps if we ever bake them externally, e.g. via
  Blender for hero interiors like the cathedral.)
- **No artist-in-the-loop editor.** All set dressing is code. For a procedural city that's fine — we
  were never going to hand-place 2,500 houses — but it means every visual idea must become a *rule*,
  not a brushstroke. (This is also the project's superpower: one good rule fixes the whole city.)
- **No built-in LOD/HLOD/impostor generation.** `VisibilityRange` (dither-fade by distance) exists
  per-entity; grouping and impostors are ours to build. Matters only once prop counts get silly.
- **No decal system worth leaning on yet** — `ForwardDecal` exists but is limited; grime is better
  done in-mesh (vertex colour / second UV tint) for our use.
- **Skinned crowds are DIY.** Capsule-people are ours to improve; Bevy will happily skin a rigged
  glTF but ships no crowd system. (2020 concern, not 2011.)

Verdict: the engine has been *ahead* of this codebase the whole time. We are using maybe 30% of its
renderer.

---

## 3. Is performance the limitation?

No. The current scene is **absurdly cheap** and structured exactly the way you'd want for 100× more
detail:

- The whole 2,566-building fabric is batched by material into **about a dozen giant meshes**
  (`spawn_batch` per `WallKind`/`RoofKind`, windows, doors — mod.rs:643-674). A few dozen draw calls
  for the entire city. The flip side: no culling granularity — the GPU rasterises the far half of the
  city through the near half every frame — and it's *still* fast, which tells you how much headroom
  exists.
- Everything else is clones of five primitive meshes (`CityMeshes`, mod.rs:64) sharing a handful of
  `StandardMaterial`s. Bevy auto-instances identical mesh+material pairs: a thousand more barrels is
  one instanced draw.
- Textures: twelve 1024² albedos ≈ 70 MB VRAM raw, mip-mapped. A 2080-class card yawns.

Budget math for everything §5 proposes (worst case, whole city, no culling):
- Window modules: ~2,500 buildings × ~14 openings × ~6 cuboids ≈ **210 k instances** of one unit-cube
  mesh in ~3 materials. Bevy's GPU-instanced cuboids at this count are a few ms — and `VisibilityRange`
  at 120 m cuts 90% of them (you cannot resolve a 6 cm frame at 120 m anyway).
- Timber frames: ~800 half-timber buildings × ~40 members ≈ 32 k instances, same story.
- Chimneys/eaves/ridge caps: < 20 k instances.
- Props: 5–10 k.
- SSAO ≈ 0.5–1.5 ms @1080p; volumetric fog ≈ ~1 ms at default res; 4096² cascade re-render is the
  priciest item and still comfortable on your box.

The one real perf rule for this codebase: **never give each building its own `Mesh` asset for
detail**. Either merge per-district into the existing `MeshData` batches (walls do this today) or
instance shared primitives (props/frames/windows should do this). Both paths are already idiomatic
here. 60 fps at 1080p is not in danger before ~1 M instances, and `VisibilityRange` exists long
before that.

---

## 4. What separates 1996 from 2011 (the perceptual checklist)

Worth stating explicitly, because it's the rubric every experiment should be scored against.
Human "realism" perception of an urban scene keys, in roughly this order, on:

1. **Occlusion gradients** — darkening in every concavity: under eaves, in window reveals, where wall
   meets ground, in alley mouths. (1996 has none; 2011 = SSAO + baked AO; 2020 = GI.)
2. **Silhouette complexity** — rooflines broken by chimneys, dormers, towers, jetties; street walls
   broken by overhangs, signs, awnings. Boxes read instantly as boxes at any texture quality.
3. **Contact detail at hand range** — frames, sills, hinges, steps, doorstones; texture response
   (normal maps) under grazing light.
4. **Value variation at street range** — no two adjacent façades identical in hue/brightness/grime.
5. **Depth cues in air** — aerial perspective (have it!), light shafts, smoke, haze layering.
6. **Evidence of life** — clutter, wear paths, laundry, crowds, birds. (Stuff.)
7. Only *then*: texture resolution, polygon roundness, shader sophistication.

The screenshot fails 1, 2, 3, 4, 6 outright and half-passes 5. That is *why* it says 1996 even
though the renderer is doing physically-based everything. The good news: 1–4 and 6 are geometry and
rules, not engine work.

---

## 5. The road to 2011, ordered by visible-return-per-effort

### 5.1 Light & air (one evening, transforms every screenshot)

Camera (controller.rs:439-464): add `ScreenSpaceAmbientOcclusion` (quality High) + `Msaa::Off` +
`Smaa` (or TAA) + `Bloom::NATURAL`. Resource: `DirectionalLightShadowMap { size: 4096 }`.
Sun cascades (scene.rs:1113): pull `maximum_distance` to ~180 m, `first_cascade_far_bound` ~12 m —
street-range crispness; the fog hides the far cutoff. Drop `GlobalAmbientLight` 300 → ~80–120 lux
once SSAO exists (flat ambient is currently doing "fake GI" duty and fighting every crevice).
Fix the sun's arc (clock.rs:88): cap noon elevation at ~52° (`elevation.cos` → scale by ~0.79 and
re-normalise) so midday keeps 1.3:1 shadows; keep the ecliptic tilt so morning/evening rake the
façades. Consider `VolumetricFog` + `VolumetricLight` on the sun for morning shafts (tune density
low; it's a mood dial, not a default).
Also worth testing: EV 12.8 → ~13.3 with the brighter ambient sum, and FOV 70° → 60° vertical —
the reference images are 45–55° shots; narrower FOV makes façades loom.

### 5.2 Windows that are holes, not stickers (the single biggest façade win)

**Tonight's key lesson: you cannot fake the recess.** The first attempt sank the glass behind the
wall plane and relied on the solid wall to "intersect" it — and the wall, being opaque, simply hid
every window (§8, exp2→exp3). The correct move, now implemented: the wall face is emitted as the
**rectangle complement of its openings** (`add_wall_face_with_holes`, a ~40-line scanline over the
opening rects per edge) and each hole is then lined with a module:
- glass pane recessed 15 cm, slightly oversized so no slit into the hollow shell survives;
- reveal returns (the four faces connecting wall plane to glass) — this is what reads as "hole";
- projecting sill below, proud lintel above (the under-sill shadow is very visible in raking light);
- mullion cross on the glass plane;
- shutters folded back against the wall on ~42% of lower-floor plaster/timber windows;
- doors: recessed leaf, reveal, lintel, threshold slab.
Openings are *planned first* (`plan_facade_openings`), then walls are cut around them, then modules
line them — one source of truth, no drift. Per-building hash drives skips (11%), x-jitter (±0.35 m),
and smaller ground-floor openings (0.78×1.02 m vs 1.0×1.35 m) so façades stop being punch cards.
Remaining from the original wish list: per-building width variation, shopfronts, dusk emissive
(easy follow-ups — the planner is the one place to touch).

### 5.3 Roofscape (skyline + the eaves shadow line)

- **Eaves overhang**: extend both roof planes 0.45–0.7 m past the wall plane and 0.25 m past the
  gables (add_building_roof already knows the eave rectangle; push the two eave verts outward along
  the plane). The dark AO+shadow line under the eaves is one of the strongest "real building" cues.
- **Ridge caps**: a 0.18 m half-round (cylinder) along each ridge, terracotta-dark.
- **Chimneys**: 1–3 per building by hash (bigger houses more), fieldstone boxes 0.8×0.8 m rising
  0.8–1.6 m over the ridge with a cap slab; place on the ridge line at hash positions. This alone
  fixes the naked skyline. Add a *smoke* system later (billboard quads, §6-lite: even 40 lazy smoke
  columns over the city reads as morning life).
- **Dormers** on 3+ storey buildings with steep roofs (~20%): small gabled boxes with a window module.
- **Roof UVs along the slope** instead of the current top-down planar map (mod.rs:873) so tile
  courses run with the pitch and don't stretch on steep roofs.
- Landmarks: finials/weathervanes on towers and gates (cone + rod + pennant quad).

### 5.4 Real timber frames + per-building variation (kill the wallpaper)

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

### 5.5 Verticality: the overhead layer (the actual brief)

This is the heart of your ask — all three inspiration images are about **occupied space above the
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

### 5.6 Clutter: the prop kit (evidence of life)

One module, ~10 parametric props from existing primitives, instanced and hash-scattered with the
same discipline as fixtures (respect nav clearance = the 1.49 m rule the population test uses):
barrel, crate stack, sack pile, handcart (the_cut foreground), firewood stack, bench, water trough,
hanging sign (bracket + board, per shopfront-y door on named streets), awning (cloth quad, slight
sag, over ~25% of street-facing doors), lantern-on-bracket (emissive + tiny PointLight budget: only
nearest N lit via distance system), laundry line between jetties (catenary of small quads), posted
notices (paper quads on walls near squares/gates). Density keyed by district: thick in Wickmarket/
Shambles/wharves, sparse in Drapers' Reach. ~5–10 k instances city-wide ≈ free (§3).

### 5.7 Ground contact & wear

- Wear strips: a 0.5 m darkened band (vertex colour on the ground/site meshes, or thin decal quads)
  along every building front — kills the "buildings placed on a billiard table" look.
- Gutter: a 0.3 m darker channel along street centrelines (the road ribbon already exists —
  just add a centre strip with the dry_cut material darkened).
- Puddles after "rain": a few dozen low-roughness ellipse discs in hash spots (2020: SSR makes them
  mirrors; 2011: high reflectance + env map is already convincing).
- Doorsteps: a stone slab proud of the ground at every door (they exist in nav!), 20 cm tall.

### 5.8 People (flagged, not planned tonight)

Capsules with name tags are a bigger 1996 tell than any wall. 2011 fix: a single low-poly rigged
villager glTF (or 3 variants), vertex-tinted per NPC, simple walk cycle; Bevy's skinned mesh + the
existing movement lerp carries it. Crowd *ambience* (non-sim extras) is a separate cheap win:
20–40 wandering shells with no LLM turn, culled by district. Needs its own feature doc.

---

## 6. The stretch to 2020

Do §5 first; every item below multiplies it.

1. **Texture set v2**: for the twelve albedos, add normal + roughness (+ small height for cobbles/
   timber) maps — generate from the existing albedos (Materialize/awesomebump/SD-based pipelines) or
   regenerate as proper PBR sets; load as KTX2+zstd with mips (features already enabled). Parallax
   (`depth_map`) on cobbles and fieldstone. This is the biggest pure-look jump after §5.1/§5.2.
2. **PCSS** (`experimental_pbr_pcss` feature): contact-hardening soft shadows; the eaves/jetty
   shadows go from crisp-everywhere to photographic.
3. **SSR** on wet streets and the canal; a real water shader (normal-scrolled ripples) for the Serle.
4. **Volumetrics as weather**: fog density driven by the world clock (dawn mist over the canal
   burning off by Terce), god rays through the Bellstand arches; light shafts in the cathedral
   already halfway exist via the spot lights.
5. **GI**: if an RT GPU is present, `bevy_solari` (raytraced direct+GI) is the honest 2020 answer.
   Fallback ladder: irradiance-volume-style hand-placed probes per district (Bevy has
   `EnvironmentMapLight`/`LightProbe`) → per-district baked cube maps for alleys vs squares.
6. **Meshlet + authored hero props**: once props go from parametric cuboids to sculpted meshes
   (cart, bell, saints, market goods), the `meshlet` path removes the LOD question entirely.
7. **Skinned crowds + cloth**: villager rigs, banner/awning vertex-wave, laundry sway.
8. **Birds**: 3–5 boid flocks circling towers, scatter on the bell. Comically high mood-per-triangle.
9. **HLOD/impostors** if 1+2 push instance counts past comfort: bake distant district cards.

---

## 7. Performance engineering notes (for whoever implements)

- Keep the two batching disciplines separate and deliberate: **merge** static per-district geometry
  into the existing `MeshData` batches (walls/roofs/wear strips/timber frames can all go this way —
  but merge per *district*, not city-wide, so frustum culling gets its granularity back); **instance**
  repeated identical shapes (window modules, props, joists, balusters — one mesh asset + one material
  each). Never per-building unique mesh assets for detail.
- Split the city fabric batches by district anyway (a `district` field is already on every building):
  today's city-wide batches defeat frustum *and* occlusion culling; ~20 district batches × 8 materials
  is still trivially few draws but lets the far side of Ombreval actually cull.
- `VisibilityRange` bands: window modules/props/frames fade at ~120–150 m; chimneys/dormers at 400 m;
  jetties/bridges/towers never (silhouette features!).
- Lights: hard budget. Sun + ≤8 shadowless point lights near the camera (lanterns swap on/off by
  distance, the cathedral interior already rations shadow casters this way, scene.rs:1149).
- Measure before/after with `bevy diagnostic` FrameTimeDiagnosticsPlugin dumped into logs.jsonl —
  the session-log infra is already there (`source:"rust"`), so numbers land next to screenshots.
- The `door_edges`/nav contract: **nothing in §5 moves a ground-level footprint or blocks a route.**
  Jetties/galleries live above 3.15 m; props obey the 1.49 m clearance rule; stairs/arcade posts must
  be validated against `assets/world/navigation.json` reachability like fixtures are (plan.rs test
  `distributed_population_spawns_clear_city_buildings_walls_and_fixtures` is the pattern to copy).

## 8. What actually changed tonight (experiment log)

Everything below is implemented and committed on the **`more-verticality`** branch, verified with
in-game screenshots via the (new) `tp` drive action, and green on the full test suite
(195 passed). Screenshot evidence lives in `logs/session_207…214/screenshots/` (`exp1_*` … `exp6_*`,
`gold_*`); each batch reused the same camera poses so the befores/afters line up. My runs used
`fake_backend: true` and a `high_wick`/`waning` start in a temporary config.ron — your original
config.ron was restored afterwards.

**Batch 1 — light & air (exp1b_)**
- Camera: `Msaa::Off` + `Smaa` + `ScreenSpaceAmbientOcclusion` (High) + `Bloom::NATURAL`
  (controller.rs). 4096² sun shadow map; cascades retuned 14 m/320 m (scene.rs).
- Flat ambient 300 → 110 lux day (14 night), both in scene.rs and clock.rs, now that SSAO and the
  atmosphere env-map carry the fill.
- **Sun arc capped at ~50° noon altitude** (clock.rs) — the old arc hit the zenith and flattened
  everything; now even High Wick rakes the façades. Verified: the noon lane shot casts storey-long
  shadows across the street.
- New drive action: `tp x y z [yaw [pitch]]` (drive.rs + a `TeleportPlayer` message in
  controller.rs) — deterministic screenshot poses, and honestly just a good debug tool.

**Batch 2 — the façade is real now (exp2/exp3)**
- Walls: per-building tint jitter + ground-grime vertex gradient (`ATTRIBUTE_COLOR`; the knee at
  2.8 m), openings **actually cut through the wall mesh**, storey-aware (see §5.2 for the failed
  first attempt — instructive).
- Window/door modules: recess, reveals, sills, lintels, mullions, shutters, thresholds.
- Roofs: eaves overhang 0.55 m following the pitch (the under-eave shadow line!), verge overhangs,
  ridge caps, slope-aligned UVs (tiles no longer stretch), and **chimneys** (1–2 per gabled
  building, on the ridge, with cap slabs) — the skyline went from bald to inhabited in one change.
- Glass material: roughness 0.3 → 0.55, reflectance 0.32 — panes stopped mirroring the sky as
  cyan stickers.
- Found & fixed a latent bug: `dark_wood` was single-sided, so door leaves/shutters had been
  backface-culled on half the façades (pale "empty doorway" look). Now double-sided.

**Batch 3 — verticality (exp4/exp5/exp6)**
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

**Perf posture after all of the above**: the whole city is still ~a few dozen batched meshes plus
~1.3k entities; drive runs sustain screenshot cadence at 1280×720 without hitching. No LOD needed
yet. (Vertex count grew to roughly 2–3 M across the city batches — trivial for any 2015+ GPU. The
next big additive feature should start splitting batches per district, §7.)

**What I deliberately did not do tonight**: geometric timber framing over plaster (§5.4 — biggest
remaining façade win, needs the member-layout rules), gable-to-street ridge orientation, storey
count increase in the plan generator (§1 D6 — *your* call, it's a map revision), awnings/laundry,
smoke, AutoExposure (recommended; needs a compensation curve so night stays night), volumetric fog
(compiled in, one component away), NPC bodies.

### 8.0 Cross-checks from a parallel code survey

A second, independent code-mapping pass (separate agent, same night) confirmed the diagnosis and
added three things this doc should own:

- **The cathedral interior has its own texture bug**: `scene.rs:153` loads `limestone.png` with the
  default *clamp* sampler and the interior is built from scaled unit cuboids (0..1 UVs per face), so
  one texture tile stretches across every wall slab. The city avoided this via
  `load_repeating_texture` + real UVs; the cathedral needs the same treatment — but note a repeating
  sampler alone won't fix it, since the scaled-cuboid UVs never leave 0..1: either author UVs in a
  custom mesh (like `cathedral_floor_mesh` already does) or set `StandardMaterial::uv_transform`
  per broad face class. Untouched tonight; the interior deserves its own evening.
- **Existing docs to fold in rather than duplicate**: `features/more_interesting_houses.md` is the
  *gameplay* half of §5.6 (shopfronts, signs, functions, sounds — my hanging-sign props are its
  visual advance guard) and `features/performance_improvements.md` (1,500 walking actors) already
  plans `VisibilityRange` fades, shorter cascades and an optional depth prepass + occlusion culling
  — §7 here should be executed together with it.
- **Backface culling**: every `textured()` city material is `double_sided + cull_mode: None`. The
  survey flags reclaiming it as a free perf win — *partially true now*: since tonight, roof
  undersides at the eaves and the hollow shells behind punched windows genuinely need the inner
  faces on walls and roofs; but ground/road/site surfaces and most props don't. Reclaim it
  per-material, not wholesale.

### 8.1 Compare for yourself

| Claim | Before | After |
|---|---|---|
| Street canyon | your session-202 screenshot | `exp6_east_lane`, `gold_east_lane` |
| Sticker windows → holes | session-202 | `exp3_facade_frontal` (mullions, reveals) |
| Bald skyline → chimneys | session-202 | `exp2_skyline`, `exp6_skyline` |
| Tower prism → belfry | `exp1b_bellstand` | `exp4_belfry_close`, `exp6_on_belfry` |
| Nothing overhead → galleries | any before | `exp6_gallery_bellway`, `exp6_gallery_crookneck` |
| Golden hour | — | `gold_*` (the Waning, 15:00 — the sun cap at work) |

## 9. Knob map (quick reference)

| Thing | Where |
|---|---|
| Camera / post components | `src/controller.rs:439` (`spawn_player`) |
| Fog | `src/scene.rs:1200` (`add_fog_to_new_cameras`) |
| Sun spawn + cascades | `src/scene.rs:1101` (`build_lighting`) |
| Sun day arc + ambient swing | `src/smart_actors/clock.rs:74` (`drive_sun`) |
| Flat ambient | `src/scene.rs:93` (`GlobalAmbientLight`) |
| City materials/tints/roughness | `src/city/mod.rs:258` (`create_materials`) |
| Wall extrusion + UV scale (7 m) | `src/city/mod.rs:782` (`add_extruded_walls`) |
| Roof shape/pitch/UVs | `src/city/mod.rs:818` (`add_building_roof`) |
| Storey height (3.15 m) + special eaves | `src/city/mod.rs:37,727` |
| Windows/doors | `src/city/mod.rs:894` (`add_facade_openings`) |
| Fixtures (from the plan) | `src/city/mod.rs` (`build_fixtures`) |
| Bellstand/towers/named details | `src/city/mod.rs` (`build_named_details`, `build_bellstand_belfry`, `add_open_bell_stage`, `spawn_bell`) |
| City levels distribution (map revision!) | `scripts/generate_top_down_map.py:732` |
| Plan data (2,566 buildings) | `lore/places/ombreval_buildings.json` |
| Texture assets (1024², albedo-only) | `assets/textures/ombreval_*.png` |
| *New:* opening planner / hole cutter | `src/city/mod.rs` (`plan_facade_openings`, `add_wall_face_with_holes`) |
| *New:* window/door modules | `src/city/mod.rs` (`add_window_module`, `add_door_module`, `add_reveal`) |
| *New:* jetty step size / eligibility | `src/city/mod.rs` (`JETTY_STEP`, `jetty_bands`) |
| *New:* eaves/verge overhang, ridge caps, chimneys | `src/city/mod.rs` (`EAVES_OVERHANG`, `VERGE_OVERHANG`, `add_chimneys`) |
| *New:* per-building tint + grime | `src/city/mod.rs` (`building_tint`, `grime_shade`, `MeshData::set_brush`) |
| *New:* street galleries | `src/city/mod.rs` (`build_street_galleries`) |
| *New:* doorway prop kit (batched) | `src/city/mod.rs` (`build_street_props`, `add_barrel`, `add_drum`, `add_log`, `add_sack`) |
| *New:* screenshot teleport | `CATHEDRAL_DRIVE='tp x y z [yaw [pitch]]'` (`src/drive.rs`, `TeleportPlayer` in `src/controller.rs`) |
