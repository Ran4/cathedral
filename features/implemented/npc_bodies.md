SUMMARY:

Implement NPC bodies — the city looks 2011 now, and the lilac capsules stand out more with every scene upgrade.

Even simple articulated primitives + the not-yet-animated gait from the movement feature would close the gap. Pairs naturally with finishing gait animation.

We want to target graphics that's around Fallout 3 level, possibly slightly below.

DETAIL SPEC:

Bodies are only half the feature. The other half is *what the body can say*: how a vendor
holds out a loaf, how a stranger waves you over, how a "no" reads from across the lane, how
a drunk walks. So this spec covers three things that ship together:

1. **The puppet** — an articulated body replacing the capsule.
2. **The pose pipeline** — the host-side animation layers that drive it.
3. **The mind→body contract** — which motions the LLM commands, which the sim produces
   without an LLM turn, and which are pure host-side reflex.

## 0. What exists today (the seams we build on)

| Seam | Where | State |
|---|---|---|
| Body = capsule(0.40, 0.82) + head sphere(0.27) + nose cone | `src/smart_actors/actors.rs:121-159`, spawn at `:319-447` | the thing we replace |
| Hot pose channel: `ActorMotion { position_m, facing_yaw, speed, gait_phase }` @ 20 Hz | `crates/cathedral-sim/src/engine.rs:401-412`, host `MotionSample` `model.rs:52-59` | **plumbed end-to-end, deliberately unread** (`actors.rs:256-259`) |
| Gait accumulator `gait_phase += speed*dt*GAIT_CADENCE`, stride-continuous across reroutes | `world.rs:34-37,535`, `round.rs:3769` | ready to render |
| Cold snapshot per NPC: `id, name_for_player, control, position_m, facing_yaw, appearance, holds` | `snapshot.rs` / `model.rs` | **no activity, mood, pose, or gesture field**; `appearance` is the structured `AppearanceSnapshot` (§2 seam, shipped) |
| Offered items = bobbing props above the head (`OfferAnchor` y=2.02) | `actors.rs:607-830` | **retired in M2** — props live in the hands (`src/smart_actors/hands.rs`) |
| Speech presentation + per-NPC voice sink (`NpcVoice`, bubble lifetime) | `speech.rs:281-460,638,777-812` | the talk-animation hook |
| Action verbs: 14, no gesture/emote; `dance {}` has a test asserting it's unknown | `actions.rs:72-101`, test `:2277` | greenfield |
| `make_sound` + injected `emittable_sounds` prompt list | `actions.rs:1118`, `turn.j2` | the template for a gesture verb |
| Statuses axis (drunkenness etc.) | sketched in `features/movement/03_the_ladder.md:63-90` | **does not exist in code** |
| Animation infra | `bevy_animation`/`bevy_gltf` compiled in (via the `3d` feature), zero usage; no model assets anywhere | greenfield |

Constants we inherit: `WALK_SPEED_MPS = 1.8` (the only speed), `SETTLED_SPEED_MPS = 0.15`
(the walk/idle line), `MOVEMENT_TICK_SECONDS = 0.05`, `HEARING_RADIUS_M = 20` (the social
radius), VisibilityRange fade 120–150 m, stage < 32 m.

## 1. Goal and aesthetic

Fallout 3 or slightly below, filtered through this game's actual origin: a monumental
engraving.

- Overall silhouette height stays ≈ the current 1.7 m so doors, bridges and streets keep scale.
- The body is **textured**, like everything else in the scene: generated clothing/cloth
  artwork for the outfit classes and painted faces on the heads. Include faces! There's an
  OPENAI_API_KEY — feel free to use it with gpt-image-2 to generate a few dozen faces (and
  outfit base textures).
- **Readability targets** (these are the acceptance criteria for the whole feature):
  at 30 m you can tell *walking vs standing*, *which way they face*, *carrying vs
  empty-handed*, and *rough occupation* (headgear/palette). At 8 m you can tell *talking*,
  *offering you something*, *waving/nodding/refusing*. At 3 m a handover reads as a handover.

**Non-goals:** player body/hands (first-person stays disembodied), cloth sim,
ragdoll, foot IK (some foot slide is accepted), children/animal builds,
and the 20–40 non-sim "crowd shell" wanderers (that idea stays in
`its_at_least_2011_now_baby.md` §5.8, out of scope here).

## 2. The puppet

*(M0 shipped — `src/smart_actors/body.rs`. As-built notes, where they diverge from the text
below: 11 mesh parts + optional headgear rather than 13 — each headgear variant is a single
merged lathe mesh (Brim's disc+crown, KettleHelm's dome+brim), and hand props stay M2. All
limb/head meshes are authored with their pivot baked at the joint (thigh origin = hip, head
origin = neck) so pose systems rotate part `Transform`s in place. The head is a custom UV
sphere (r 0.24) whose UVs azimuthal-project a face texture onto the front hemisphere;
everything past the rim runs off `[0,1]` and a clamp-to-edge sampler paints it in the
image's uniform skin-edge tone — 24 shared face materials, picked from a rehash of
`palette_seed` so face, tint (bits 16–17) and headgear (bit 7, sim-side) stay uncorrelated.
Outfits: 7 textured cloth bands × 4 quantized tints + 3 bespoke = 31 shared materials; the
majors' legacy colors are lifted so the brightest channel is 0.85 because they now multiply
a textured albedo instead of standing alone. The hood wears its own neutral dark
double-sided shell material, not the wearer's outfit band (open shells need double-sided;
a per-class hood would have doubled the band). Build scaling: hips scale the pelvis, and
shoulders+height scale the torso — legs keep authored length so every foot stays planted on
the walk plane; stature reads in torso/head (M 1.03/1.05/0.96, F 0.97/0.94/1.05).
`ActorOutfit` now tags all ten cloth parts and a new `ActorFace` tags the head; reconcile
hot-swaps both materials on appearance change, while headgear mesh and build are spawn-time
(appearances never restructure today). `BodyRig` + left/right `HandAnchor`s are in place
for M1/M2. Heads are painted only on the front hemisphere — the clamped rear is bare skin,
so uncovered heads read bald from behind; acceptable under the engraving stylization, and
most classes wear headgear.)*

**Representation: articulated primitives, not skinned glTF — for v1.** Everything visual in
this project is procedural primitives authored in code; there is no asset pipeline, and the
summary above is right that articulated primitives + real gait closes most of the gap. A
low-poly skinned glTF (the `its_at_least_2011_now_baby.md` §5.8 suggestion) remains the
upgrade path, so the pose math must stay representation-independent (see §4): the pose
layer computes joint angles; a writer applies them to part `Transform`s today and could
apply them to skinned bones tomorrow. Do not touch `AnimationPlayer`/`AnimationGraph` —
our animation is a handful of `sin()` calls and slerps, and clip infrastructure would be
more code than the animation.

**Part list (13 mesh parts per actor), all shared-handle primitives so batching holds:**

- pelvis (small cuboid/rounded box), torso (tapered — Capsule3d or scaled cuboid), head
  (the existing 0.27 sphere, slightly reduced)
- per side: upper arm, forearm (thin capsules), thigh, shin (thin capsules) — 8 parts
- optional headgear (1 part, per-occupation mesh: hood, coif, brimmed hat, watch kettle-helm)
- optional held-item prop slot on each hand (see §6)

Joints are just entity parenting: root → pelvis → torso → head; pelvis → thighs → shins;
torso → upper arms → forearms. A `BodyRig` component on the root stores the part
`Entity` ids so pose systems never do name lookups. Hands are empty anchor children on the
forearms (`HandAnchor`, left/right) — the successors of today's `OfferAnchor`.

**The nose cone dies.** It existed only to make the capsule's facing readable
(`actors.rs:364-375`); an asymmetric body with headgear reads facing on its own.
`NameAnchor`/`SpeechAnchor` stay as-is (labels and bubbles are untouched by this feature).
Every mesh part gets the same cloned `VisibilityRange` (120–150 m fade) the three current
parts get. The `Indoors`/sleep hiding rule keeps working because it hides the root.

**Appearance.** *(Seam shipped.)* The stringly `appearance_key` is gone: `ActorSnapshot`
carries a structured, still-public `appearance: AppearanceSnapshot`
(`crates/cathedral-sim/src/appearance.rs`), composed **once in the sim** at character
creation from sheet facts (gender, occupation class via `occupation_id`, rank,
circumstances — district is deliberately unused, and `faction_role` too: every authored
faction is a secret society, and a faction outfit would leak it at 30 m) plus a
deterministic per-id FNV-1a seed:

```
AppearanceSnapshot {
    build: Build,          // Female | Male (shoulder/hip/height ±5% scaling)
    outfit: OutfitClass,   // Cleric | Merchant | Craftsman | Laborer | Watch | Notable | Poor
    headgear: Headgear,    // None | Hood | Coif | Brim | KettleHelm
    palette_seed: u32,     // deterministic tint within the outfit's palette band
    bespoke: Option<String>, // the named majors' fixed-look override ("sven"/"conny"/"ilse"),
                             // authored as `bespoke_appearance` in their lore JSON — never
                             // keyed on name_for_player (the unknown-people rule rewrites it)
}
```

The host maps `OutfitClass` to a small band of **textured materials** — original generated
clothing artwork (wool, linen, leather, vestments) in muted engraving tones, like every
other surface in the project — and tints them per seed (the three named majors keep their
exact current colors as fixed tint overrides). Heads get **face textures**: a few dozen
generated faces (gpt-image-2, see §1) shared across the cast and picked deterministically
from the per-id seed. This is the whole variety system: ~7 outfit texture bands × headgear
× tint seed × a few dozen faces is enough for 500 actors to stop looking cloned, while
every material stays a shared handle so batching holds.

## 3. The mind→body contract

The user-facing question of this feature: *who moves the body?* Three tiers, with one hard
rule separating them.

**Tier 1 — Reflex (host-only, cosmetic, invisible to minds).** Breathing, idle weight
shifts, gaze/head-tracking, talk gesticulation, blink-rate equivalents. These live entirely
in Bevy systems, read only what already crosses the boundary (motion samples, speech
events, sound events), and **never** produce sim state or percepts. No LLM cost, no sim
churn.

**Tier 2 — Habit (sim-driven, no LLM turn).** The autonomous round already makes bodies do
things: walk legs, queue at stalls, serve customers, light lamps, draw water. The body
layer *dresses* these existing sim facts and events: `holds` → carry pose,
`OfferSnapshot` → arm extended, `WorldEvent::{Accepted,Declined}` → recipient nod /
head-shake, the silent stall purchase → a hand-over motion between vendor and buyer.
No new sim decisions — only new presentation of decisions the sim already makes.

**Tier 3 — Deliberate (an LLM action line).** A new `gesture` verb (§7) for motions that
*are* communication: wave, beckon, point, bow, refuse, dance. These cost an action line in
a turn, exactly like `say`/`make_sound`.

**The rule that keeps this honest:** *anything another mind could react to must originate
in the sim and produce percepts; anything that never enters the sim must stay cosmetic.*
A wave the LLM performs is real — nearby NPCs get "Ilse waves at you" in their inboxes and
can answer it. A breathing cycle is not — no mind ever hears about it. There is no third
category ("visible to the player but unknown to NPCs") because that desynchronizes what the
player sees from what NPCs know, which is precisely the kind of inconsistency the sim
exists to prevent. Gesture percepts follow the standard unknown-people naming rule
("A stranger waves at you").

## 4. The pose pipeline (host)

One new module, `src/smart_actors/body.rs` (splitting from `actors.rs`, which keeps
spawn/labels/reconcile). Pose evaluation is layered; later layers override or add onto
earlier ones per joint:

- **L0 Locomotion** — walk cycle from `MotionSample.speed` + `gait_phase` (§5). Owns legs,
  arm swing, torso bob, turn lean.
- **L1 Idle life** — when settled: breathing (2–4 s torso scale/pitch, phase from actor-id
  hash so crowds don't sync), occasional weight shift, rare head glance.
- **L2 Activity** — carry pose, offer pose, vendor-serving pose. Keyed on snapshot state
  (`holds`, offers) and `WorldEvent`s. Owns arms when active.
- **L3 Gesture** — one-shots and loops from `EngineMessage::Gesture` (§7). Owns arms +
  head for the duration; upper-body-only, so a walking wave works.
- **L4 Speech & gaze** — while this actor's `NpcVoice` sink is live (or its bubble
  unexpired): small head bob + forearm gesticulation with amplitude ramped by an id-seeded
  noise; gaze slerps the head (yaw clamp ±70°) toward the conversation partner, the player
  when near and addressed, or briefly toward a loud `Sound` position.

Blending is deliberately dumb: each layer has per-joint weights; a gesture ramps its weight
in/out over ~0.15 s. No graph, no state machine beyond "active gesture + expiry".

**Carriage modifiers (§8)** are not a layer — they are parameters (sway amplitude, phase
noise, cadence multiplier, lean bias) read by L0/L1.

## 5. Locomotion (finishing movement M7's gait)

Both signals arrive per tick and are currently discarded in `drive_npc_bodies`
(`actors.rs:260-305`). That system keeps owning root translation/yaw interpolation
untouched; the new pose system reads the same interpolated `MotionSample`:

- **walk factor** `w = smoothstep(SETTLED_SPEED_MPS, 0.6, speed)` blends idle↔walk over
  ~0.25 s so starts/stops don't pop.
- Legs: thigh pitch `± A_leg · sin(2π · gait_phase)`, opposite phase per side; shin adds a
  lagged counter-bend. Arms: counter-phase swing at `~0.5 · A_leg` (suppressed on any arm
  owned by L2/L3). Torso: bob `A_bob · |sin|` at double frequency, slight lateral roll.
  Start values `A_leg ≈ 30°`, `A_bob ≈ 0.03 m`; tune by eye against stride length — if feet
  visibly skate, adjust `GAIT_CADENCE` (`world.rs:34`), not the visuals, so the phase stays
  the sim's single source of stride truth.
- Turn lean: roll the torso a few degrees against yaw rate (derivable from the two motion
  samples already held in `NpcMotion`).
- `gait_phase` is stride-continuous across reroutes by sim guarantee — never reset it
  host-side.

## 6. Hands: carrying and offering

The direct answer to "not just above their head surely" — the above-head offer fan
(`OfferAnchor`, `reconcile_offered_item_views`, bob+spin) is **retired** and replaced by:

- **Carrying (habit tier):** an actor with a non-empty `holds` renders the first item's
  `visual_key` prop in the left `HandAnchor`, arm relaxed at the side (basket-carry pitch).
  This alone makes the bread round legible: buyers walk home visibly holding a loaf.
- **Offering (habit tier, existing consent dance):** while an `OfferSnapshot` exists, the
  giver's right arm extends toward the recipient (L2 owns the arm; gaze follows), the item
  prop sits in the offered hand. `WorldEvent::Accepted` → item prop transfers to the
  recipient's hand (a 0.3 s lerp between anchors), recipient nods. Declined → giver's arm
  retracts, decliner shakes head. Retracted/withdrawn → arm retracts. All keyed on events
  that already flow (`mod.rs:871-897`); the HUD toast text stays.
- **Stall vending (habit tier):** the silent auto-purchase already emits self-percepts and
  the `coin_clink` sound; add a `WorldEvent::StallSale { vendor, buyer, item }` so the
  host can play the same hand-over choreography between vendor and buyer. The
  bread-board-stock dressing from `04_the_bread_round.md` §7 stays in that feature; this
  spec only needs the hand-over.
- Multiple simultaneous offers (the current fan handles them) are rare; the one visible
  in-hand offer is the oldest, the rest exist only in text. Acceptable loss.

## 7. Gestures: the deliberate body

**Verb.** One new verb, mirroring `make_sound`'s shape:

```
gesture {"kind": "wave", "to": "Sven"}     # "to" optional: a person name you can see
```

**Catalog** — a `const` table in the sim (not a data file: every kind needs bespoke pose
code anyway, so a toml row without matching animation is a lie; adding a gesture = one
const row + one pose function):

| kind | target | dur | loop | percept template |
|---|---|---|---|---|
| `wave` | optional | 1.5 s | no | "{A} waves at {B}." / "{A} waves." |
| `beckon` | yes | 1.5 s | no | "{A} beckons {B} closer." |
| `nod` | optional | 0.8 s | no | "{A} nods to {B}." |
| `shake_head` | optional | 0.9 s | no | "{A} shakes their head at {B}." |
| `shrug` | no | 1.0 s | no | "{A} shrugs." |
| `point` | yes (person or known place handle) | 1.2 s | no | "{A} points toward {B}." |
| `bow` | optional | 1.8 s | no | "{A} bows to {B}." |
| `dance` | no | loop | yes | "{A} is dancing." |

`point` accepting a place handle pairs with `tell_way` ("The well is that way" + arm).
`dance` loops until the actor's next non-`wait` action or 60 s of sim time, whichever
first.

**Sim mechanics.** Parse in the normal grammar (`parse.rs` needs nothing new); dispatch arm
in `actions.rs`; unknown kind → the standard action-error percept. Delete/invert the
`dance {}` unknown-verb test at `actions.rs:2277`. Witnesses = `characters_within(origin,
HEARING_RADIUS_M)` — sight reuses the 20 m social radius; consistent with `say`, and
occlusion is ignored exactly as it is for speech. Percepts to witnesses use the templates
above with unknown-people naming; self-percept "You wave at Sven." enters the actor's own
history. `world_revision` bumps.

**Boundary crossing.** Two additions:
- `EngineMessage::Gesture { actor_id, kind, target_id, recipient_ids }` — the transient
  trigger, presented like `Speech` (player included only if within radius).
- `ActorSnapshot.active_gesture: Option<GestureKind>` — for loopers, so a player walking up
  mid-`dance` sees the dance (host starts the loop from snapshot state, not just events).

**Prompt.** One conditional line in `turn.j2`'s fenced action block (next to `make_sound`),
listing the kinds inline: `gesture {"kind": "wave|beckon|nod|shake_head|shrug|point|bow|dance", "to": "optional name"}`
plus a one-clause hint ("small body language; use it instead of narrating gestures in
say"). Re-bless `golden_prompts.rs` fixtures. This also gives the "no inventing verbs"
rule a pressure valve — models that want to *do* something physical now have a legal verb.

**Autonomic gesture reuse (habit tier).** The same pose functions are triggered without the
LLM where the sim already decides: accept → `nod`, decline → `shake_head` (§6). Keep this
list short and mechanical; anything with social meaning beyond the event itself belongs to
the LLM.

## 8. Carriage: drunk swaying and the statuses seam

There is **no intoxication state in the sim today** — no ale item, no drunkenness field;
taverns are just workplaces and hearths. `movement/03_the_ladder.md:63-90` already sketches
the `statuses` axis this belongs to. This feature builds the *seam and the body language*,
not the gameplay:

- Sim: `statuses: BTreeMap<StatusKind, f64>` on `CharacterState`; a small `StatusKind` enum
  starting with `Drunkenness`, `Weariness`. Publicly visible kinds (both of these) cross on
  `ActorSnapshot` as `statuses: Vec<(StatusKind, f32)>`. Nothing in the sim *sets* them yet
  except tests and a debug hook (a `cathedral-headless` flag like
  `--status Ilse=drunkenness:0.8`, and/or a drive-mode action) for eyeballing.
- Host carriage mapping: `drunkenness` scales L0 phase noise + lateral torso sway + a
  wandering lean bias and multiplies cadence irregularity; `weariness` drops arm swing and
  adds forward stoop. Because it modulates the walk the sim already computed, the actor's
  *position* stays honest — witnesses and collisions are unaffected.
- Optional, later, sim-side: a drunkenness multiplier on `LANE_JITTER_FRACTION` so the
  *path* genuinely weaves. Flagged separately because it changes witnessable positions.
- Ale, tavern serving, and how anyone *gets* drunk: explicitly out of scope — that is
  food_and_items M5+ material. When it lands, it writes one float and the body already
  knows what to do.

## 9. Performance

- ~13 mesh parts × 500 actors ≈ 6.5 k entities exists but is mostly culled; parts share
  mesh+material handles so draw batching holds, and every part carries the VisibilityRange
  fade (shadow-pass drop included, per `performance_improvements.md` item 8).
- **Pose LOD:** Tier A (< 40 m, cap ~64 actors): all layers. Tier B (40–120 m): L0 only,
  evaluated every other frame. Tier C (> 120 m): no pose writes (fade region). Gestures
  and gaze simply don't evaluate outside Tier A — at those distances they fail the §1
  readability test anyway.
- Budget: pose systems ≤ 0.5 ms/frame at Tier A cap; measure with the usual frame timings
  before/after on the forecourt (worst crowd).
- Sim-side cost of this feature is ~zero: gestures are turn-rate events; statuses are a
  cold snapshot field.

## 10. Testing

- **Sim (pure, fast):** parse/dispatch tests per gesture kind incl. unknown-kind error and
  target resolution; percept-text goldens (known/unknown observer); `active_gesture`
  set/expiry across `poll`s; statuses serialization + snapshot exposure; `StallSale` event
  emission. Golden prompt fixtures re-blessed once (`turn.j2` change).
- **Headless:** transcript prints gesture lines (`* Ilse waves at Sven`); fake backend's
  deterministic script gains a gesture emission so offline integration runs exercise the
  path end-to-end.
- **Host/visual:** drive-mode scripts per milestone —
  `tp` to a stall at bread hour, `shot` the queue/handover; `tp` to a square, `shot` crowd
  variety; `--status`-style drunk actor, `shot` sequence for sway. Existing entity/nav test
  contracts must stay green (body is visual-only; sim geometry stays point+radius).

## 11. Milestones

- **M0 The puppet.** Articulated body replaces capsule+nose; `AppearanceSnapshot` seam;
  textured outfit bands + generated face textures + headgear + tint; VisibilityRange on all
  parts; neutral standing pose.
  *Done when:* a forecourt screenshot shows a varied, occupation-readable crowd; all tests
  green.
  *(Shipped — see the §2 as-built note. Tests: rig part/material-bound unit tests +
  face-projection orientation contract in body.rs/actors.rs; the 3-root and 514-root
  ActorView contracts hold. One honesty note on the acceptance shot: the sim never
  gathers more than ~5 bodies in one square — vendors at pitches, well queues, the
  majors' bell cluster — so "crowd" screenshots show a handful of varied figures, not a
  throng; that is world density, not a body-rendering limit.)*
- **M1 The gait.** L0 walk cycle from `speed`/`gait_phase` + L1 idle life + turn lean +
  pose LOD tiers. Closes movement M7's "carried but unbuilt" note.
  *Done when:* walking vs standing reads at 30 m in a screenshot pair; frame budget held.
  *(Shipped — `body.rs::animate_body_pose`, one system in the `Present` set. As-built notes,
  where they diverge from the text above: pose = per-joint `JointDelta` (rot/trans/scale)
  composed absolutely over a `RestPose` captured at spawn (build scaling folded in), layers
  blending over the accumulator at their own weight exactly per §4 — L2/L3/L4 slot in as
  further `apply_*` calls after `apply_idle`, Tier A only. The gait keeps its own two-sample
  history in `BodyPoseState` (`NpcMotion`'s fields are private, and it also yields the
  yaw *rate* for turn lean, low-passed at 8/s); a sample older than 0.18 s counts as
  speed 0 because an arrived mover just leaves the hot channel — no "stopped" tick exists.
  `A_leg` landed at 27° (not 30°) and `GAIT_CADENCE` was retuned 1.4 → 0.67 in `world.rs`
  (one stride cycle per ~1.5 m matches the authored 0.82 m leg at that swing, so feet
  don't skate); it stays private — nothing host-side reads it. Knee fold is a
  `max(0, cos)`-gated counter-bend peaking early-swing. L1 periods/phases all hash off
  FNV-1a(actor id): breath 2.6–3.8 s, weight shift 9–15 s alternating sides, glance
  12–20 s. LOD per §9, plus: Tier B parity is seeded so the skipped half varies, Tier C
  writes rest once on transition, and an `at_rest` flag makes far idle bodies cost zero
  transform writes. Budget: release measures 30 µs/frame at a synthetic Tier-A cap (514
  walking actors, 100 candidates capped to 64 — the live world never gathers that many;
  ignored test `pose_cost_at_tier_a_cap`) and 56 µs avg / 96 µs max on the live forecourt
  crowd — ~6–11% of the 0.5 ms budget. A `[body pose]` diagnostic logs avg/max/tier counts
  every 5 s. Goldens untouched. Screenshot honesty note: the acceptance pair reads
  clearly, but "crowd" frames show the world's real density — one to five figures, walkers
  desynchronized, not a throng (same caveat as M0's).)*
- **M2 Hands.** `HandAnchor`s; carry-from-`holds`; offer/accept/decline/retract
  choreography; above-head offer fan deleted; `StallSale` event + vendor handover.
  *Done when:* a bread-round purchase reads as a handover in a drive-mode shot sequence.
  *(Shipped — new module `src/smart_actors/hands.rs` (prop reconcile + hand-over flights,
  the fan's prop vocabulary and Create/Keep/Replace disposition moved there as
  `ItemPropAssets`/`prop_disposition`); L2 activity + the nod/head-shake one-shots live in
  `body.rs` (`apply_carry`/`apply_offer`/`apply_one_shot`, blends and `OneShotGesture` state
  on `BodyPoseState` — `start_gesture(kind, face, now)` is the entry point M4's catalog
  reuses). As-built notes, where they diverge from §6's text: the carry prop is the first
  held item that is neither a spark stack nor the in-hand offer — the whole cast carries a
  spark wallet, and rendering it would put a coin in every fist in the city; the sim event
  kind is the string `"stall_sale"` (there is no sim WorldEvent enum), emitted with empty
  recipients from `round.rs::service_stalls` next to the coin_clink, and
  `engine.rs::is_handoff_kind` keeps it out of the offer-handoff hold — tests pin that no
  mind, inbox or golden prompt sees it. Host wiring: the WorldEvent drain arm forwards
  accept/decline/stall_sale as a `HandoverFeedback` message; accept/stall_sale launch a
  0.3 s eased prop flight giver-right-hand → recipient-left-hand (arced, suppressing the
  recipient's carry prop while airborne; a bodiless recipient — the player — catches at
  chest height over the mirror position) plus a recipient nod; the stall vendor gets a
  0.9 s `pulse_offer` arm extension since no OfferSnapshot backs a silent sale; decline
  plays the decliner's head-shake, and every retraction is just the offer leaving the
  snapshot ramping the L2 blend out over ~0.22 s. Screenshot honesty: the offer arm +
  in-hand prop, a mid-walk carried item (Sven's fish), and the accept→retract sequence
  were all captured live in fake mode; a live silent stall purchase never landed on
  camera during the drive sessions — bread-round buyers only carry home on a Waning home
  leg, and no famished buyer queued while watched — so the vendor→buyer flight itself is
  covered by the shared code path (same `launch_flight`/nod as accept) and by tests, not
  by a live shot.)*
- **M3 Reflex.** Gaze/head-tracking (conversation, player, loud sounds); talk gesticulation
  keyed on `NpcVoice`/bubble lifetime; autonomic nod/head-shake on accept/decline events.
  *(Shipped — L4 lives in `body.rs`: a `ReflexState` resource (speaker → talk deadline +
  addressee, plus a bounded 8-entry recent-sound ring) fed by `track_reflex_signals` in the
  `Present` chain right before `animate_body_pose`. As-built notes, where they diverge from
  the text above: the talk signal keys on the `PresentSpeech` message, not `NpcVoice` —
  `active_voice` carries no actor id — with the deadline shared verbatim with the bubbles
  (`speech_text_seconds`, made `pub(super)`); `PresentSpeech` grew a `target_id` (the sim's
  `say` target, previously discarded in the Speech drain arm) so the speaker's gaze knows
  its partner. Gaze priority per actor, Tier A only (`ReflexState::gaze_point`): own
  conversation partner while talking (the player resolves to the live camera, an NPC to
  its live root) > listener glance at the nearest live speaker within the 20 m social
  radius > own standing offer / stall pulse aim (the §6 "gaze follows" deferred from M2) >
  a recent sound, idle actors only (walk_blend < 0.5), within the sound's own
  `audible_distance` — no loudness threshold needed — held 2.2–3.6 s per (actor, sound)
  hash, self-sounds under 1 m excluded. The head slerps at ~7/s with weight ramps
  (~0.25–0.3 s); yaw clamps at ±70° off the root facing (= torso: no layer yaws the
  torso), pitch ±0.5 rad, and a target beyond ~137° drops the gaze entirely rather than
  pinning a craned neck. Talk gesticulation: forearms lift 0.43–0.85 rad and wave with
  sides out of phase, upper arms pitch forward and drift off the ribs (silhouette
  readability at 8 m — the first cut read only in profile and was amplified), head bob is
  composed *over* the gaze rotation, and everything scales on an energy noise of two
  seeded slow sines in [0.1, 1] so no two speakers gesture alike; per-arm weights
  `1 − carry_blend` / `1 − offer_blend` keep L2-owned arms out of it. One deliberate §4
  ordering deviation: L4 evaluates *before* the L3 one-shot, so an accept-nod or
  decline-shake — a communicative beat — plays over the ambient tracking and its closed
  envelope hands the head back to the gaze smoothly; the M2 wiring itself
  (WorldEvent drain → `HandoverFeedback` → `start_gesture`) was verified complete, no gap.
  Nothing sim-side changed; goldens untouched. Tests: talk-deadline/prune bookkeeping,
  gaze priority chain incl. idle gating and earshot, yaw clamp + behind give-up +
  torso-relative frame, gesticulation lift/suppression/per-speaker divergence, and an
  app-level manual-clock test that a `PresentSpeech` turns the speaker's head+talk layers
  on (head visibly toward the camera) and both decay past the deadline. Screenshot
  honesty: all captures are the fake-mode Ilse exchange — the flank shot shows her head
  turned ~70° off her torso toward a camera that teleported mid-bubble (live tracking),
  and the bell glance is proven as a before/after pair (head in profile at rest → facing
  the drive-injected `sound town_bell` origin with the toast in frame); one drive session
  was discarded after desktop keyboard focus leaked into the window mid-run.)*
- **M4 The deliberate body.** `gesture` verb: catalog, dispatch, percepts,
  `EngineMessage::Gesture`, `active_gesture`, prompt block + golden re-bless, headless
  transcript, fake-mode script, pose functions for all 8 kinds.
  *Done when:* in a live run, asking an NPC to wave produces a wave the player sees and a
  percept a bystander NPC reacts to.
  *(Shipped — sim `crates/cathedral-sim/src/gesture.rs` (`GestureKind` ×8, serde
  snake_case so `shake_head` is the wire form; a `const GESTURES` catalog of
  `GestureSpec` rows — verb string, `GestureTarget` rule, duration, loop flag,
  percept templates; `DANCE_MAX_SECONDS = 60`) + the `gesture` verb in
  `actions.rs`; host in `body.rs`/`mod.rs`. As-built notes, where they diverge
  from §7's text: **each catalog row carries four percept templates, not one** —
  witness (3rd-person) and own (2nd-person) × targeted and untargeted — because
  English needs distinct "waves"/"wave" wording, a "you" when the witness *is*
  the target, and a separate self line; the sim's stranger form keeps the id it
  always has, so a bystander reads `A stranger (id cb947) waves at you.`, not
  the spec's shorthand `A stranger waves at you.` Witnesses are
  `characters_within(origin, HEARING_RADIUS_M)` exactly like `say` (occlusion
  ignored), percepts route through `perception::identify`, the self line enters
  `recent_history`, and an unknown kind is a new `ActionErrorCode::UnknownGesture`
  (mirrors `make_sound`'s `unknown_sound`). Target resolution: no-target kinds
  reject a `to`, required kinds demand it, and `point` resolves a known place
  handle first (like `tell_way`, gated on `places_known`) before falling back to
  a visible person (like `say`); a place-pointed gesture names the place for all
  observers and carries **no** `target_id` on the event (the host cannot aim at a
  place). **`active_gesture`** lives as `CharacterState.active_gesture:
  Option<ActiveGesture{kind, deadline}>` and crosses as
  `ActorSnapshot.active_gesture: Option<GestureKind>`, `skip_serializing_if`
  `None` in both the sim and host snapshot — without that skip the
  `"active_gesture":null` on 500 actors tipped the public snapshot past its
  128 KiB test cap. Only `dance` (the sole looper) sets it; it clears two ways —
  the actor's next successful non-`wait` action, enforced in `dispatch` (the
  `gesture` verb excepted, since it sets its own), and the engine's
  `expire_gestures` after the 60 s cap (deadline stamped on the first poll that
  sees the loop, exactly like a `TravelIntent`'s, since the action layer has no
  clock). Both clears bump `world_revision`, so the snapshot drops it and the
  host stops the dance. **`EngineMessage::Gesture`** rides a new
  `EventType::Gesture` + `DomainEvent::gesture` + `flush_gesture` (which reparses
  the verb string back to a `GestureKind`); it is presented like `Speech` (player
  in `recipient_ids` only within radius). **Prompt:** one *unconditional* line
  after `make_sound` in `turn.j2` (gestures are not gated by `sounds_enabled`,
  so it is not wrapped in the `{% if %}`), kinds inline, one-clause hint;
  re-blessed via the ignored regenerate test — the diff is exactly the one added
  line in all 22 sounds-bearing fixtures, and the sounds-off fixture places it
  right after `tell_way`. **fake.rs** keys on `wave`/`dance` in the fresh
  percepts for *any* name (not Ilse-scoped, so whichever nearest listener the
  reaction lane picks answers): a wave greets and waves at the player, a dance
  says and starts the loop; the dance-unknown-verb test at `actions.rs` was
  inverted (a bare `dance` verb is still `UnknownVerb`, but `dance` is now
  reachable as `gesture {"kind": "dance"}`), and the headless bin gained a
  `--say TEXT` flag (broadcast one utterance from the spawn) so an offline run
  prints the transcript line. **Host poses:** `OneShotGesture` grew Wave / Beckon
  / Shrug / Point / Bow beside M2's Nod / ShakeHead, durations matched to the sim
  catalog, `from_kind` mapping `Dance → None`; `apply_one_shot` now poses arms /
  torso / head per kind, **upper-body only** (legs untouched, so a walking wave
  still walks), wave/beckon/point swinging the shoulder by the clamped
  target-aim `face_yaw`, and bow folding the torso forward with a *negative*
  X-rotation (the upright torso pivots opposite a down-hanging arm). A new
  `apply_dance` looping envelope (torso roll+twist, pelvis bob+sway, both arms up
  swinging in opposition, head bob, per-actor phase) is driven by
  `BodyPoseState.dance`/`dance_blend`. **Wiring:** a `body::PresentGesture`
  message + a `drive_gesture_pose` system in `ReconcileMirror` — it starts
  one-shots from the triggers (aiming at the target's live mirror position) and,
  every frame, sets each body's dance flag from the snapshot's `active_gesture`,
  so a player who arrives mid-loop sees the dance and it stops the frame the flag
  clears; `mod.rs`'s `Gesture` arm writes the message and nothing else (no
  toast); `animate_body_pose` applies the dance *before* the one-shot so a wave —
  which ends the loop sim-side — reads over the brief blend-out. Tests: sim
  per-kind parse/dispatch/percept goldens (known + unknown observer, "you" for
  the target), target resolution and errors, `active_gesture` set + next-action
  clear, place-`point`; `e2e_fake` through the real `Engine` — a player "wave"
  emits an `EngineMessage::Gesture` aimed at the player with a bystander NPC in
  `recipient_ids`, and a `dance` rides the snapshot and is expired after the cap;
  host unit tests for the mapping, the upper-body poses, bow-forward, and the
  dance's sway-over-time; and an app-level test that a dance snapshot drives the
  loop (blend up, stop on clear) while a `PresentGesture` starts a one-shot.
  Zero new clippy warnings; goldens re-blessed once, deliberately. Screenshot
  honesty: the wave (Conny's right arm up, Ilse and Sven behind) and the dance
  (Conny both arms up mid-loop, then arms down once the loop ended on his next
  turn) were captured live in fake mode; all three majors waved from one
  broadcast because every witness within 20 m gets the percept and the scripted
  cast all answers it, and the dawn market square is dim — the poses read, the
  crowd is the world's real handful, not a throng.)*
- **M5 Carriage.** Statuses seam + drunk/weary carriage mapping + debug hook.
  *Done when:* `--status`-flagged actor visibly sways on a walk in a shot sequence, with
  zero change to its path coordinates.
  *(Shipped — sim `StatusKind` (`Drunkenness`, `Weariness`; serde snake_case,
  `Ord` for the map key) lives in `crates/cathedral-sim/src/character.rs` next
  to `Needs`, with `CharacterState.statuses: BTreeMap<StatusKind, f64>` (seeded
  empty in `from_sheet`, exactly like `Needs`) and `Character::statuses() ->
  Vec<(StatusKind, f32)>` clamping each to a finite `0..=1`. Both kinds are
  public: they cross on `ActorSnapshot.statuses: Vec<(StatusKind, f32)>` in all
  three places (sim `snapshot.rs`, host `model.rs` `From`, the `model.rs`
  validator), `skip_serializing_if = "Vec::is_empty"` so the universal empty
  case keeps the 500-actor snapshot byte-identical; the validator rejects any
  value outside a finite `0..=1` (`SnapshotError::InvalidStatus`), while the sim
  and the `From` clamp on the way through. As-built notes, where they diverge
  from §8's text: **statuses persist nowhere** — `CharacterState` derives no
  `Serialize` (only the seed `CharacterSheet` does, and statuses are runtime
  like `Needs`), so there is no save/load to round-trip; the serialization
  contract that *is* pinned is the `PublicSnapshot` one (a status set, exposed,
  and round-tripped through serde, empty stays absent). **Nothing sim-side sets
  a status** except the tests and the two debug hooks, which share one sim entry
  point `World::debug_set_status(name, kind, value)` (case-insensitive name
  match, clamps, bumps `world_revision`) reached through a new
  `EngineCommand::DebugSetStatus`: the `cathedral-headless --status
  Name=kind:value` flag (repeatable, applied after world load, beside `--say`)
  and the drive-mode `status <name> <kind> <value>` action (parsed in
  `src/drive.rs`, carried by a new `BridgeCommand::DebugStatus` down the exact
  path the `sound` action's `DebugSound` uses). **Carriage is parameters on
  L0/L1, not a layer** (§4): `body.rs` grew a private `Carriage {drunkenness,
  weariness}` mirrored onto `BodyPoseState` each frame by `drive_gesture_pose`'s
  snapshot loop (one mirror lookup now feeds both the dance flag and the
  statuses, so `animate_body_pose` still never reads the mirror). Drunkenness
  `d` staggers the *visual* gait phase (a fast jitter + a slow cadence drift, in
  stride-cycle units), adds a lateral torso sway plus a slowly-wandering seeded
  lean (both roll, clamped together to ±0.35 rad), all read by `apply_locomotion`
  and — so a *standing* drunk still sways — by `apply_idle`. Weariness `w` scales
  the arm swing to `1 − 0.7w` (→ 0.3× at `w=1`) and folds the torso forward into
  a stoop (a negative X-rotation, like the bow). Every term is scaled by its
  status, so a default `Carriage` is byte-for-byte identity — the existing L0/L1
  tests, called with `Carriage::default()`, still pass unchanged. The actor's
  path/position is untouched: carriage only reshapes the walk the sim already
  computed, so witnesses and collisions are unaffected, and the LANE_JITTER
  path-weave (§8) stays unimplemented. Because L0 runs in Tier B and idle life
  only in Tier A, a *walking* drunk sways down to 120 m while a *standing* one
  sways only inside 40 m — acceptable LOD, the far figure fails §1's readability
  anyway. Tests: sim `StatusKind` snake_case serde round-trip, `statuses()`
  clamp/order/exposure, `debug_set_status` by-name/clamp/revision-bump/unknown,
  and `public_snapshot` exposure + serde round-trip; a prompt test asserting a
  set status changes *neither* the rendered markdown nor the structured sheet
  (statuses never enter prompts — so **goldens were NOT re-blessed**, they stay
  M4's blessed bytes); host `--status`/drive parse tests (multi-word names,
  bad kinds, out-of-range); and host mapping units — `d=0` identity (torso
  offset exactly `(0,0)`, full arm swing, no stoop), `d=0.8` sway wanders yet
  stays inside the clamp and the phase stagger moves the legs, `w` drops the arm
  swing toward 0.3× and stoops the torso forward, `Carriage::from_statuses`
  mapping. Zero new clippy warnings. Screenshot honesty: captured live in fake
  mode on Ilse (a standing major in the dawn market) — a sober back view
  (upright), then drunkenness 0.8 with the torso visibly leaning one way and, a
  frame later, the other (the sway oscillation; her head also turns because a
  fake-mode exchange started — that is L4 gaze, not the carriage, which only
  rolls/pitches the torso), and an elevated rear pair showing weariness 1.0
  fold the head forward and down over the feet versus the upright baseline. A
  *walking* drunk was not the captured subject — market majors stand at dawn and
  following a walker with static teleports is unreliable — but a standing drunk
  exercises the identical `apply_idle` carriage the walk uses in `apply_locomotion`,
  the leg-stagger is pinned by test, and Ilse's coordinates never moved between
  frames (the §8 "zero change to its path" guarantee). **Ale, tavern serving,
  and how anyone actually gets drunk stay food_and_items M5+ material** — when
  they land it is one float write into `statuses` per §8, and the body already
  knows what to do.)*

M0→M1→M2 are strictly ordered; M3/M4/M5 are independent after M2 (M4 only needs M0's arms).

## 12. Risks & open questions

- **Stiff-puppet uncanny valley.** Mitigated by the carved-figure stylization, muted
  textured palette and painterly generated faces — we promise engraving, not human. If it
  still reads wrong, the glTF swap path (§2) exists and the pose math carries over.
- **Gesture spam / prompt regressions.** A gesture costs an action line like any verb; if
  models over-gesture, tighten the prompt hint before adding rate limits. Golden-prompt
  churn is a one-time re-bless.
- **`appearance_key` migration.** *(Done.)* The three majors keep bespoke looks via the
  `bespoke` override; every snapshot construction site produces an `AppearanceSnapshot`,
  and the host maps `OutfitClass` to placeholder flat colors until the textured bands land.
- **Open:** do beggars/notables warrant extra outfit classes beyond the seven? Does `point`
  need target facing-direction data in the percept ("points north")? Decide after M0
  screenshots. (Resolved: bodies are textured — generated clothing artwork and gpt-image-2
  faces ship with M0.)
