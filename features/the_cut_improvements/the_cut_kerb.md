# Draw the kerb: the Cut's cartway, margin and bank line

**Status:** design, not implemented (2026-07-28). Drawing: `features/the_cut_kerb.html`
(open it in a browser — four plates: the two cross-sections, the 60 m plan, and the long section).

Lay one line of kerbstone down each side of the Cut, five metres off the centreline, and give
the ground outside it a different material. Nothing else. No new sim verb, no new NPC behaviour,
no navigation rebake.

The Cut is currently the widest surface in Ombreval and the only one with nothing drawn on it.
This feature does not fill it. It **divides** it, so that the things we want to put there later
have somewhere to be and somewhere to be out of place.

Related, and deliberately kept separate:

- `features/the_cut_dry_carry.md` — the freight convoy and the porter job. Wants a cartway.
- `features/design_the_cut_game.md` — the parked street game. Wants a course, and a reason to
  be illegal.
- `lore/the_dry_boatmen.md` §"The soundings" — wants the old riverbed to be readable from the
  street. §5 below is how the kerb pays that off.

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
worth doing until something needs it.

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

Margin furniture, repeating along the reaches (all drawn, none collided unless noted):

| Prop | Note |
|---|---|
| Blocked water stair | descends through the kerb line and dies in filled ground — the fossil of the river. The strongest single object here. |
| Mooring ring | iron, set into the kerbstone face, at the head of each blocked stair |
| Cellar hatch / vent | flush in the margin against the façade |
| Bollard | at reach ends and either side of the bridge crossings; **this one is collided** (it is a real obstacle and is short enough to matter) |
| Kerb break | a 3 m flush gap where a warehouse door faces the street, so carts can cross the line lawfully |

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

### M3 — a real step (only if something needs it)

Lift the margin 0.25 m, collide the riser, lift the puppets. Needs the nav bake to keep the margin
connected — the kerb breaks (M1) become load-bearing, not decorative. Do not start this without a
feature that requires it.

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

Invariants worth a test:

1. `collision_footprints.json` is unchanged by M0–M2 — nothing here is a collider except the M1
   bollards, and those must be individually justified.
2. The kerb never crosses a site polygon (`tallage`, `marens_green`) as a ridge.
3. Both kerb lines stay inside the 20 m road ribbon and outside nothing — `|x + 213.5| = 5.0` for
   the full length of each reach.

---

## 7. Lore follow-up (after, not before)

- `lore/places/02_canonical_gazetteer.md` §"The Cut" — say the cartway is kerbed, and where it is
  only marked.
- `lore/the_dry_boatmen.md` §"The soundings" — it currently explains the soundings entirely through
  cellars, foundation trenches and cracked party walls. Add the paragraph it implies: that the
  kerb-line is the one public, unarguable record, which is precisely why Cut landlords hate it.
- `features/design_the_cut_game.md` — if hoop-poling is chosen, the camber and the cartway are its
  course and this doc is its prerequisite.
