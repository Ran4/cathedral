# Draw the kerb: the Cut's cartway, margin and bank line

**Status:** M0–M3 implemented and the lore written back (2026-07-30). Nothing here is open; the
file stays in `features/the_cut_improvements/` because its siblings are unbuilt and
`index.md` points at this path. Drawing: `features/the_cut_improvements/the_cut_kerb.html`
(open it in a browser — four plates: the two cross-sections, the 60 m plan, and the long section).

Lay one line of kerbstone down each side of the Cut, five metres off the centreline, and give
the ground outside it a different material. Nothing else. No new sim verb, no new NPC behaviour,
no navigation rebake.

The Cut is currently the widest surface in Ombreval and the only one with nothing drawn on it.
This feature does not fill it. It **divides** it, so that the things we want to put there later
have somewhere to be and somewhere to be out of place.

Related, and deliberately kept separate:

- `features/the_cut_improvements/the_cut_dry_carry.md` — the freight convoy and the porter job.
  Wants a cartway.
- `features/the_cut_improvements/poling_dry_game.md` — the street game (it resolves the parked
  `design_the_cut_game.md`). Wants a course, and a reason to be illegal.
- `lore/the_dry_boatmen.md` §"The soundings" — wants the old riverbed to be readable from the
  street. §5 below is how the kerb pays that off, and §7 is the paragraph that was written back.

---

## 1. Why the Cut reads as empty

It is not short of buildings. It is short of *distinctions*. Measured off the shipped data:

| | |
|---|---|
| `lore/places/ombreval_buildings.json` road `cut` | one segment, `(-213.5, 325.5)` → `(-213.5, -422.0)`, `width_m: 20.0` |
| Length | ~748 m — the longest single thing in the city |
| Every other road tier | `major` 5.5–8.5 m (6 of them), `minor` 3.5–6.0 m (32), `passage` 5.0, `service` 4.0–4.5, `wall_lane` 4.0, `alley` 1.2–2.4 |
| Façade line along the Cut | west `x ≈ -225.2`, east `x ≈ -201.8` — **23.4 m housefront to housefront** |
| Authored furniture on it | one: `build_ropewalk` — 6 posts and 3 wires, and they stand at `x = -182`, thirty metres off the street behind the housefronts |
| Actors with a leg *at* "The Cut" in `rounds.json` | one: Tam Rud, fuller |
| Sound bed | `AreaBed { area_id: "the_cut", sound: CutFreightCorridor }`, doc-commented *"Carts and porters along the old river bed"* |

The last row is the whole problem stated by the codebase itself. The soundscape already plays
freight over a street that has no freight, no kerb, no margin and no cart. `build_sites_and_roads`
draws the Cut as a single 20 m ribbon of `materials.dry_cut` and stops.

`lore/places/02_canonical_gazetteer.md` says the Cut's **working cartway is 8–12 m** and that the
rest is old bank, carrying "warehouse doors, blocked water stairs, cellar vents, hoists, awnings,
counting rooms". Roughly half the street is supposed to be a different kind of place. Right now it
is the same texture.

The gap between "20 m of identical ground" and "a 10 m cartway with 6 m of bank either side" is
this feature.

---

## 2. Binding decisions

### 2.1 The kerb is drawn, never collided

`src/controller.rs`:

```rust
pub const WALK_BAND_LO: f32 = 0.01;
```

`scripts/bake_navigation.py::build_walkable` erodes **every exported collider footprint** by the
agent radius and then keeps only the single largest connected component. Colliders are exported by
`solid_footprints_in_band(WALK_BAND_LO, WALK_BAND_HI)`, which takes any solid whose `max.y >= 0.01`
— i.e. anything at all standing on the ground.

So a kerb *with a collider* would carve two 748 m lines out of the walkable surface, erode them
wider than they are, and sever the six-metre margin from the cartway along the entire street. The
margin would become its own component and be dropped. That strands Tam Rud, every housefront door
on the Cut, the ropewalk approach, and every stall we ever pitch there.

**Therefore: the kerb has no entry in `CollisionWorld`.** It never reaches
`collision_footprints.json`, the bake is byte-identical, and there is nothing to re-pin. This is
the same class of trap as the Stone House door leaf (`build_stone_house`'s doc comment): a solid
where the world needs a line.

*(M3 has since collided the riser walls and the sixteen bollards — 58 footprints, the four-step
nav chain re-run and `shelters.json` re-pinned by hand. The arithmetic above did not change and is
still exactly why the margin **surface** is never a solid: a floor slab would carve the whole
margin out of the walkable grid. What M3 showed is that thin walls with authored gaps survive it —
the main component kept 100.0% of the walkable surface and all 1101 doors. The live invariant is
`city::tests::the_cut_collides_exactly_the_riser_and_the_bollards`; see §4 M3 and §6.1.)*

### 2.2 Nothing changes height for feet

The plate draws a 0.25 m step because that is what a kerb *is*. Do not ship 0.25 m. The player
resolves only against `CollisionWorld`, and NPC puppets stand on the flat nav graph, so any drawn
riser is a surface both of them glide through.

**Ship the kerbstone as a 0.10 m ridge and keep every walkable surface at grade.** At 10 cm nobody
notices they did not step up; at 25 cm everybody does. The margin is read by *material and the
line*, not by level:

- cartway — `materials.dry_cut` as today: rutted, dusty, cart-worn
- margin — a distinct drier, flagged/gravelled treatment
- kerbstone — `materials.limestone`, a continuous 0.30 m wide ridge 0.10 m proud

A real step, with the margin lifted and the puppets lifted onto it, is a possible M3 and is not
worth doing until something needs it. *(M3 has since shipped the real step, gate waived by the
repo owner — see its "How it shipped" under §4; this section stands as the record of why M0–M2
did not.)*

### 2.3 The camber belongs to the hoop game, not here

A cambered cartway is correct and is what a rolled hoop needs. It is also the one part of this that
makes surfaces non-flat. Leave the crown out; `design_the_cut_game.md` can add it when it needs it,
and it will want to choose its own profile anyway.

### 2.4 Inside the squares the line is marked, not built

Two sites interrupt the ribbon:

- **The Tallage** — `z 23.8 … 105.0`
- **Maren's Green** — `z -294.7 … -216.3`

The gazetteer is specific: *"On Lowmarket the Cut stays open to through carts down its middle.
Stalls use marked margins and must clear for a bell."* In the squares the boundary is a rule the
Bench asserts, not a stone somebody laid. Draw it there as flush marker blocks at intervals — same
line, no ridge — so that crossing from street to square you can see the law get weaker.

This also gives three natural authored reaches instead of one 748 m extrusion:

| Reach | z range | Length | Character |
|---|---|---|---|
| North | +325.5 → +105.0 | 220 m | Chain Bridge quarter to the Tallage — rope, wool, hides, the ropewalk |
| Middle | +23.8 → -216.3 | 240 m | Tallage to Maren's Green — the longest empty stretch in the game today |
| South | -294.7 → -422.0 | 127 m | Maren's Green to the Old Sluice — poorer, quieter |

---

## 3. Geometry

Centreline `x = -213.5`. Kerb faces at **`x = -218.5` and `x = -208.5`**.

```
 west facade                                                        east facade
 x -225.2                          x -213.5                            x -201.8
    │                                  │                                   │
    │◄────── margin 6.7 m ──────►│◄────── cartway 10 m ──────►│◄─ margin 6.7 m ─►│
    │                          -218.5                        -208.5             │
    │                            ▓▓                             ▓▓              │
    │                          kerbstone                     kerbstone          │
    │                       0.30 m wide, 0.10 m proud, no collider              │
```

- **Cartway 10 m**, mid-range of the lore's 8–12 m, kept clear.
- **Margin 6.7 m** each side to the façades — everything the street's life needs, and wide enough
  for a stall pitch plus passage behind it.
- Worth noting: `assets/world/areas.json`'s `the_cut` boxes are `x -219.1 … -207.9`, i.e. ±5.6 m.
  The sound bed already treats almost exactly this middle strip as "the Cut". The kerb makes visible
  the boundary the audio has been using all along.

Margin furniture, repeating along the reaches — **all of it drawn, none of it collided**:

| Prop | Note |
|---|---|
| Blocked water stair | descends through the kerb line and dies in filled ground — the fossil of the river. The strongest single object here. |
| Mooring ring | iron, set into the kerbstone face, at the head of each blocked stair |
| Cellar hatch / vent | flush in the margin against the façade |
| Bollard | at reach ends, at the two squares' mouths and either side of the bridge crossings. This was drafted as the one collided exception (a real obstacle, short enough to matter); **M1 shipped it drawn like the rest** — see below — **and M3 collided it** as promised, riding the riser's own rebake |
| Kerb break | a 3 m flush gap where a warehouse door faces the street, so carts can cross the line lawfully |

**Why the bollards ended up drawn too** (decided when M1 shipped, 2026-07-29; the argument is kept
in full on `add_bollard`'s doc comment in `src/city/mod.rs`). Colliding sixteen posts costs the
whole four-step nav chain — `export_collision_footprints` → `bake_navigation.py` →
`bake_places.py` → `bake_homes.py` → a hand-edit of `shelters.json`, which has no script — and that
bake renumbers the street graph while `places.json`, `homes.json` and `shelters.json` pin bare node
indices that stay *valid* and silently mean somewhere else. That is a poor trade for sixteen
ornaments. It is also the consistent answer: the player already walks through a four-metre stone
water stair, a 0.10 m kerb and a cellar hatch on this street, so one solid post would teach "some
of this is real" rather than "posts are solid". The bollards get their colliders **with M3**, when
the margin is lifted, the kerb breaks become load-bearing and the bake has to be re-run anyway.
**That is what happened** (2026-07-30): M3 collided all sixteen with the riser, on the riser's own
rebake — both of M1's arguments having expired at once, exactly as anticipated.

---

## 4. Milestones

### M0 — the line

Kerbstone ridges along the three reaches, flush marker blocks through the two squares, margin
material either side. `build_kerb` called from `build_named_details`. No colliders. No nav rebake
(assert this: `collision_footprints.json` must be unchanged after the build).

This alone is the feature. Everything below is optional.

### M1 — the margin furniture

Blocked water stairs, mooring rings, cellar hatches, bollards, kerb breaks at warehouse doors.
Hand-placed per reach rather than procedurally spaced — the reaches have different characters and
a regular rhythm would read as wallpaper.

### M2 — the sag (the soundings)

Vary the kerbstone's `y` along its length: true over the old bank, dipping 0.15–0.25 m over the
filled channel, in three or four authored places. Pure geometry, no collider, no sim.

This is the payoff and it deserves stating plainly: **a single leaning house tells the player
nothing, because houses lean everywhere in this city.** 748 m of dead-straight stone that dips in
three specific places tells them exactly where the river was. The kerb is the only object in
Ombreval long enough and straight enough to publish the soundings — the thing the Alders sell
quietly, the Cut landlords deny, and the masons' lodge disputes the fee for and then pays.

Nothing needs to explain it. A player who walks the Cut and notices can carry it into a
conversation with a mason, a landlord or Wyn Alder and be *right about something the city argues
over*, using only `say` and `remember`.

Where the sag goes must agree with whatever `wells_and_water.md` and the soundings already fix; it
is law on what is under the street.

**How it shipped** (2026-07-29; `CUT_SOUNDINGS` and `cut_sounding_sag` in `src/city/mod.rs`). Four
soundings, one in the north reach's wool quarter, one at the harbour head under the Chain Bridge,
the longest below the Tallage, and the deepest in the scour pool above the Old Sluice; each takes
the two lines down by a different amount, because the channel was narrower than the street and did
not run down its middle. `wells_and_water.md` turns out to fix *nothing* positional — *"No plan of
the old channel was ever drawn"* is the point of the soundings — so the four are argued from what
the street itself fixes: a gate scours a pool above it, a harbour chain closes deep water, and M1's
blocked water stairs were built where the channel came alongside a bank, so all four stairs that
fall on a sounding stand on its shoulder rather than in its trough. §7's follow-up (writing the
kerb-line into `the_dry_boatmen.md`) is still open, and that is where the claim becomes lore.

**M2 ships the soundings' positions and their relative depths, not their absolute figures, and the
milestone's headline 0.15–0.25 m dip is deferred to M3.** Stated plainly because the number in the
milestone above is not what is on the ground. The cartway is one flat quad at `y = 0.024` spanning
both kerb lines, so a stone taken the full 0.20 m down is not a dip but a hole — occluded by the road
itself and indistinguishable from a junction mouth — and the ribbon cannot dip with it while the
player and the puppets stand on the flat nav graph. Nor can the ridge simply be built taller: §2.2
pins it at 0.10 m. That leaves the whole budget between the true line and the trace height
(`CUT_KERB_DROWNED_Y = 0.036`) at about six centimetres, and the drawn soundings come out **0.035 m
(the 0.16 m one) to 0.052 m (the 0.24 m one)** deep.

`cut_kerbstone_top` **scales** the authored profile into that budget rather than clamping it against
the floor, which is the one thing worth being careful about: a clamp draws every sounding deeper than
the budget at the same flush floor, so all four become the same object and the west/east asymmetry —
the actual editorial claim, that the channel did not run down the street's middle — stops existing as
geometry. Scaled, the deepest sounding is drawn visibly lower than the shallowest and each keeps its
raised-cosine shape. Two further consequences of a kerb that has gone down to street level carry what
six centimetres cannot: the drowned stone **heaves** out of true (a two-sided scatter about the
profile, not a lift off it) and is **dirtier**.

What that buys and what it does not: from a low camera sighted along the line the drowned reaches are
legibly grey, broken and flush against a bright, chunky true line, and from above the street is not
comical. At standing eye height, walking the Cut, it is at the edge of noticeable. **The milestone's
payoff — a player noticing unprompted and carrying it to a mason — is therefore not yet delivered.**
Giving the sag somewhere real to go means cutting the cartway ribbon back to the kerb lines and
carrying the margin's ground down with the stone, which needs a riser where the two surfaces part
company and puppets that stand on it: that is M3, and it is a wholesale change to this street's
surfaces rather than a patch on M2. When it lands, the profile is already the full 0.15–0.25 m and
the tests already read it — only the budget in `cut_kerbstone_top` has to go.

### M3 — a real step (only if something needs it)

Lift the margin 0.25 m, collide the riser, lift the puppets. Needs the nav bake to keep the margin
connected — the kerb breaks (M1) become load-bearing, not decorative. Do not start this without a
feature that requires it.

**How it shipped** (2026-07-30, at the repo owner's explicit request — the gate above was waived,
not satisfied; no feature yet *requires* the step). `CUT_STEP_M = 0.25` in `src/city/mod.rs`.

- **The margin is a real quarter-metre bank.** The flags are edged slabs at `y 0.28`
  (`add_margin_slab`), the kerbstones carry their whole height stack up with their seats still at
  `y −0.42`, and the hatches, vents, kerb-break aprons, door thresholds and doorside clutter ride
  up with the ground they stand on — the 22 Cut-facing doorways keep their sills the same 0.065 m
  proud of the flags the gazetteer's *"slightly raised thresholds"* always meant
  (`add_door_module`'s `ground_lift`, guarded by
  `the_cut_facing_doors_keep_their_thresholds_proud_of_the_flags`), and `build_street_props`
  seats its ricks, sacks and barrels on the sampled ground rather than at grade.
  The water stairs and mooring stones keep their absolute heights — they are the *old* bank, which
  the made ground has merely caught up with — so a flight now genuinely descends through a trench
  the flags part around (`cut_furniture_flag_gaps`), and each kerb break grew a drawn four-course
  stone pitch a cart could take.
- **Only the riser and the bollards are collided** — one thin wall per laid run, topping a
  centimetre *below* the step so leaving the margin never snags, plus the sixteen posts M1
  deferred. The margin surface itself is **never** a collider: the bake exports every solid
  topping `y ≥ 0.01`, so a floor slab would carve the whole margin out of the walkable surface.
  From the cartway the kerb is a wall (a hop clears it — the controller has no step-up); the
  lawful ways up are the kerb breaks' ramps, the water stairs and the junction mouths.
- **The step under feet is `CutMarginProfile`** — a pure XZ→height function built from the same
  geometry the renderer draws (flags flat, ramps inclined, stair treads exact). The player's
  controller seats on it as a virtual floor (`fixed_player_movement`); the puppets get it applied
  at presentation (`reconcile_actor_views` / `drive_npc_bodies`), so the sim stays flat and
  authoritative while every body stands on the drawn stone. Open strip *edges* feather the lift
  over 0.45 m — the z-ends at junction mouths and reach ends, and the x-edges left standing open
  where a road along the back of the margin (the wall lane behind the west bank) strips the outer
  lanes; edges that abut a stair, a ramp or the kerb line deliberately do not.
- **The sag is finally real relief.** M2's six-centimetre budget (`CUT_KERB_DROWNED_Y`) is
  deleted; `cut_kerbstone_top` draws the full authored 0.15–0.25 m, and the step beneath keeps
  the deepest stone ~0.09 m proud of the cartway — a dip in a standing line, sunk against the
  flags' exposed edge faces behind it, never a hole in the road.
- **The nav chain was re-run whole** — export → `bake_navigation.py` → `bake_places.py` →
  `bake_homes.py` → `shelters.json` re-pinned by hand (a pure renumber: all 28 pins resolve to
  their exact old points). The margin survived exactly as §2.1's arithmetic said it must: the
  main component keeps 100.0% of the walkable surface and all 1101 doors. The gaps do the work —
  a 3 m break erodes to a 2.3 m crossing (nine cells), a stair gap to 6.5 m, a junction mouth to
  a street's width; the ~0.7 m bollard slots seal, harmlessly.
  `the_cut_margin_stays_connected_to_its_cartway` proves it against the committed bitset, and
  `the_cut_collides_exactly_the_riser_and_the_bollards` (successor of
  `the_cut_kerb_adds_nothing_to_the_collision_world`) pins the collider set to exactly these
  boxes and nothing more.

---

## 5. What it unlocks

- **`the_cut_dry_carry.md`** — the convoy holds the cartway, the porters pass in the margin, both
  lawful and visibly different. Blocking a lane means something because the lane exists.
- **Obstruction, and the beat that already walks here.** Segwin Mott's south beat is
  `Maren's Green → The Tallage → Maren's Green` (`rounds.json`) — twice a day down this street,
  doing nothing. With a kerb, goods over the line are a legible offence and `raise_notice`
  (`crates/cathedral-sim/src/notices.rs`, law-only) finally has something to point at. The
  gazetteer already says the Bench issues obstruction notices on the Cut; this is what one looks
  like.
- **`design_the_cut_game.md`** — a course, a lane, onlookers who are standing somewhere specific,
  and the reason the game is marginal: it is played in the cartway. Enforcement of the Cut game and
  the obstruction notice become the same mechanic pointed two ways, which is exactly why it is the
  kind of thing a ward election can swing.
- **The soundings**, per M2.
- **The street stops reading as a 20 m car park**, which is the observation that started this.

---

## 6. Acceptance

The claim is visual, so the evidence is screenshots. From the middle reach, which is the emptiest
stretch in the game today:

```sh
CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE='\
  tp -213.5 4 -180 180; shot kerb_middle_reach; \
  tp -213.5 1.7 -180 90; shot kerb_across; \
  tp -213.5 4 120 180; shot kerb_north_reach; \
  tp -213.5 4 60 180; shot kerb_into_tallage; \
  tp -213.5 45 -100 180 -30; shot kerb_from_above; quit' cargo run
```

Compare `kerb_middle_reach` against the pre-change shot of the same camera
(`logs/session_569_2026-07-28_22_34_17/screenshots/cut_zneg180_south.png`).

**Two corrections to that script, learned the hard way across all four milestones** (final
acceptance run 2026-07-30, `accept_*.png`):

- **Two actions have to come first or the whole set is unreadable**: `key KeyT; sleep 55;`
  (`config.ron`'s clock already runs at 24×, so one press is 240× and 55 s reaches late morning
  from the Dayspring session start) and `weather clear 0`. At Dayspring the Cut lies in the west
  housefronts' shadow and the cartway/margin distinction does not survive it — several rounds of
  earlier "evidence" on this feature are dark frames for that reason alone. Put `click Continue`
  before every `shot`, because the startup settings panel sits over the view.
- **`tp -213.5 4 60 180` is inside the Tallage market hall** and always was. The camera that
  actually shows the line going flush is `tp -213.5 1.7 14 180 -5`, standing in the mouth of the
  square with the bollards either side. Two more worth keeping: `tp -207.5 1.7 -180 90 -8` is the
  step from across the cartway, and `tp -218.5 0.6 -95 0 -2` (0.6 m, sighted *along* the west
  line into the middle sounding) is the only camera that reads the sag — with
  `tp -218.5 0.6 3.5 0 -2` as its dead-true control. Eye height shows the street; it does not
  show the sag.

Invariants worth a test:

1. `collision_footprints.json` is unchanged by M0–M2 — **nothing on this street is a collider**,
   the M1 bollards included (§3). Anything that does want a collider here belongs to M3, with the
   rebake. **Since M3 the invariant is exact rather than empty**: the street collides its riser
   walls and its sixteen bollards and nothing else, the margin surface is never a solid, and the
   shipped test is `city::tests::the_cut_collides_exactly_the_riser_and_the_bollards` (successor
   of `the_cut_kerb_adds_nothing_to_the_collision_world`), with
   `the_cut_margin_stays_connected_to_its_cartway` proving the bake kept the margin in the main
   component.
2. The kerb never crosses a site polygon (`tallage`, `marens_green`) as a ridge.
3. Both kerb lines stay inside the 20 m road ribbon and outside nothing — `|x + 213.5| = 5.0` for
   the full length of each reach.

---

## 7. Lore follow-up (after, not before)

**Done 2026-07-30**, both edits short and in place rather than rewrites:

- `lore/places/02_canonical_gazetteer.md` §"The Cut" — the cartway is now described as bounded:
  the two lines five metres off centre, the flagged banks a hand's depth up, the three-metre flush
  break at a warehouse door, and the squares where the line is marker blocks and not stone,
  "so crossing in from the street you can watch the law get weaker".
- `lore/the_dry_boatmen.md` §"The soundings" — one paragraph, placed before the true arch of the
  Old Sluice: the Bench has written a little of the soundings down without meaning to, because the
  kerb sinks over the filled bed and comes up again on the far bank; nobody drew it and nobody can
  be sued for it, which is why a Cut landlord hates it and why the Alders still charge the same fee
  (a dip says the channel was there and nothing else — not the depth, not the sills, not which arch
  is Colm's).

Still conditional, not owed by this feature:

- `features/the_cut_improvements/poling_dry_game.md` (which resolves the parked
  `design_the_cut_game.md`) — if hoop-poling is chosen, the camber and the cartway are its course
  and this doc is its prerequisite. §2.3 still holds: the camber belongs to the game, not here.
