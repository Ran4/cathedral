Status: M0–M4 implemented and accepted (2026-09-06). Two normal game days at 1,000/2,000 residents and hidden real-GPU day/rain/evening verification pass. M4 records measured costs and remaining capacity, pacing and visual limits.

# Ambient residents

## Player experience

The extra ambient population should make the buildings and neighbourhoods feel
inhabited. People stand beside a frontage, linger in a court, watch a street,
or spend time near a few neighbours. Occasionally someone changes position or
walks a short distance. The main streets carry a small amount of traffic.

**Generated ambient NPCs have no job by default.** They have identities, homes
or lodging circumstances, ways of supporting themselves, and places they
frequent. An occupation and its work schedule are explicit exceptions.

This document records the implemented replacement for the generated crowd's
former routine. The milestones below describe the changes and their
acceptance evidence.

## Original behaviour and investigation

The investigation started with these captures from session 800:

- [Wickmarket, 15:17:31](../../logs/session_800_2026-09-06_15_02_16/screenshots/cathedral_screenshot_2026-09-06_15_17_31__00.png):
  a long line across the square; generated mason `x00259` is following his
  daily round to the masons' lodge, with 192 m remaining.
- [Wickmarket, 15:17:43](../../logs/session_800_2026-09-06_15_02_16/screenshots/cathedral_screenshot_2026-09-06_15_17_43__00.png):
  another view of the line; generated baker `x00553` is following her daily
  round to the Wickmarket.

The session log records 1,000 generated citizens. The original deterministic
generation recipe gave 756 with jobs and 244 without. The 32
generated trades have only 23 candidate workplaces between them; 464 citizens
share the day-worker template. Ninety-eight have occupations whose only
candidate workplace is the Wickmarket.

There are also mechanical causes beyond the number of commuters:

- Initial placement spreads people around navigation nodes, but wandering
  routes snap their destinations back to nodes. The street graph gradually
  replaces the initial placement beside buildings.
- Eligible NPCs roll a 35% wander chance every 1–6 seconds. This is a chance
  to start a journey, not a bound on the proportion of time spent walking.
- Generated wander destinations can be 15–40 m from a workplace, while the
  daily-round rule recalls a worker beyond 6 m. These rules permit repeated
  outward walks and returns. Their contribution to session 800 has not been
  measured separately.
- Of 4,008 baked route edges, 3,829 have half-width 0.6 m, including the
  connections at the Wickmarket. With agent radius 0.35 m and lane fractions
  0.1–0.7, same-direction lane variation spans only 0.15 m before further
  safety reductions. Many walkers therefore follow almost the same track.
- The census counts a moving person near their post as `at_post`. That
  category cannot establish that the population is mostly stationary.

The earlier [Give the crowd somewhere to be](give_the_crowd_somewhere_to_be.md)
feature improved initial spread, housing, and departure timing. It explicitly
left the small workplace pool unresolved. This spec replaces its default
generated work/wander policy; it does not edit that historical record.

The screenshots use debug acceleration. **Fast-forward changes are outside
this feature.** Behaviour acceptance is measured at normal speed.

## Scope and decisions

The following decisions govern the implementation:

1. The generated population created by `extra_ambient_npcs` defaults entirely
   to residents without jobs. Do not replace the current 75% working share
   with a different automatic working share.
2. Existing authored characters, including authored ambient workers, retain
   their occupations and routines. `Significance::Ambient` alone is not the
   switch for this feature.
3. A resident's routine is local and mostly stationary. An ordinary bell or
   market day does not send them to a city-wide workplace, market, or church.
4. No job does not imply poverty, dependence on alms, or homelessness.
5. Residents remain real, persistent, interactable actors in the authoritative
   simulation. They do not become camera-spawned scenery.
6. Local navigation and the standing/movement distinction are part of the
   feature. Changing only the occupation distribution is insufficient.

The numbers below were initial tuning values for this design. The milestone
evidence records the implemented values and any adjustments against the
intended player experience.

## Identity, housing, and explicit workers

Separate three decisions in generation: **livelihood**, **lodging**, and
**routine**. Neither housing nor behavioural policy may be inferred solely
from whether `occupation_id` is present.

For a default resident, `occupation_id`, occupation display, title, and rank
are null. Their description answers who they are, why they spend time here,
and how they get by. Use varied, compatible circumstances: a shared household
pot, support from kin, retirement, or the existing hardship circumstances.
Do not invent a named employer, active trade, or daily wage for them.

For this feature, retain the existing approximate quarter of hardship
backgrounds as a separate deterministic demographic draw. The remaining
residents receive ordinary household-supported backgrounds and are eligible
for doors. This preserves the approximate housed population without keeping
jobs as its eligibility test. A future demographic change can alter that mix.

The current no-trade support bank puts `pauper` on every entry. Expand the
generated background bank rather than using it unchanged for everybody.
Existing `dependent` circumstances can describe household support without
`pauper`; any vocabulary additions must be validated consistently. Appearance
must also support ordinary residents without a trade: do not make null
occupation force the entire population into the poorest outfit. Reuse the
existing clothes and body assets.

Assign homes near the resident's daytime patch, using verified reachable
door connections and occupancy limits. A resident's spoken home, ward,
lodging circumstances, appearance, and mechanical home must agree. Residents
without a home receive a suitable local resting/shelter patch instead.

The implementation needs an explicit routine distinction, conceptually
`Resident` and `Worker`. A generated worker is allowed only through an
explicit generation override; the shipped default contains no such overrides.
It must name a compatible occupation and workplace/routine. Do not reintroduce
automatic trade selection because a resident happens to stand near a shop.
No new settings-menu workflow or worker percentage control is required.

## Places to stand

### Persistent neighbourhood patches

Each resident belongs to a small patch containing several verified standing
positions. The patch is tied to a building frontage, court, sheltered edge,
or the side of an open square. It has a stable identifier and a meaningful
local description. A patch is not the single navigation node for a landmark.

Generate candidates from the existing city/building geometry, doors, and
walkable surface. Bake the stable geometry; assign residents and occupancy
in the simulation. No runtime filesystem access belongs in `cathedral-sim`.
The precise asset schema can be chosen during implementation, but it must
carry stable patch/spot ids, positions, facing hints, capacity, and validated
connections sufficient to use the spots safely.

Prefer positions beside buildings and along usable edges. Reject positions
inside walls, in inaccessible pockets, in door approaches, on stairs/bridge
chokes, in existing service queues, or in the main through-route of an alley.
Use door and collision geometry to protect clearance; being on a walkable
bit alone does not establish a good place to stand.

Initial spacing and distribution:

- One resident per reserved standing spot; normally at least 1.2 m between
  occupied spot centres, with room for bodies and nearby walking.
- Small patches normally support 2–6 residents. Several separate patches can
  serve a large square, spread around its usable perimeter.
- A normal patch spans roughly 5–15 m. A resident's home or resting place
  should be reachable within 30 m of walking where suitable geometry exists.
- Spread assignments across residential streets and courts as well as named
  squares. Candidate capacity and occupancy determine allocation; nearest
  landmark alone must not draw the whole neighbourhood to one spot.

Seed daytime residents at their assigned standing spots. Give each a
different initial dwell deadline so they do not all take a first walk
together. Keep identities and patch membership stable while the player
moves around the city.

Slot reservations are simulation state. Reserve a destination before moving,
release the departed slot when it is clear, and release all reservations on
departure, custody, or actor removal. An actor cannot hold every spot in a
patch while waiting for one to become free. If a destination is unavailable,
remaining at the current valid spot is a successful outcome.

Validate capacity at 1,000 and 2,000 generated residents. At larger requested
counts, use additional verified candidates rather than stacking bodies or
shrinking clearance. If valid capacity is exhausted, report requested,
placed, and unplaced counts explicitly and leave excess actors unspawned;
never silently claim the requested population was placed. The existing
20,000 ceiling remains a request ceiling, not a placement/performance promise.

### Facing and social life

At rest, people can look along the street, across their court, or toward a
nearby speaker. Nearby residents may stand in loose small groups; no automatic
following or movement toward strangers is needed to manufacture groups.

Keep existing speech, curiosity, gestures, and interaction rules. Idle
turning and posing must not require new LLM calls. Genuine conversation holds
an NPC's position and takes precedence over an optional local move.

Standing, looking, and existing gestures are sufficient for the first release.
Sitting, leaning animations, chairs, and new occupational tasks are outside
this spec. Do not claim someone is sitting or doing a job the scene does not
actually show.

## Routine: linger, move locally, settle

Use an explicit resident routine instead of falling through the existing
trade/social/wander ladder. Conceptually it has these states:

| State | Behaviour | Ends when |
|---|---|---|
| Lingering | Stay at a valid spot; face and interact naturally | A dwell deadline passes and a suitable move is admitted |
| Moving locally | Walk to one reserved nearby spot | Arrived, interrupted, or route failed |
| Visiting nearby | Walk to an admitted neighbouring patch and linger there | Its visit ends, followed by a long dwell at the next spot |
| Resting at home/shelter | Use the existing domestic resting representation | The resident's next local daytime routine begins |
| Interrupted | Existing conversation, explicit intent, weather, or custody owns the actor | That activity releases control |

Initial tuning:

- After settling, dwell for a deterministic per-actor 45–150 seconds on the
  existing elapsed-time basis used for movement/ladder deadlines. These are
  not game-calendar minutes. This does not change clock scaling or pause
  semantics.
- At a dwell deadline, the NPC may stay for another dwell. Do not perform a
  fresh per-frame wander roll or accumulate missed moves as catch-up work.
- A local change of spot normally travels 2–8 m and must fit within a 15 m
  routed distance. A route that exceeds the budget is rejected before motion.
- A less frequent visit can use a neighbouring patch within 30 m of routed
  travel. It is optional and must not become a scheduled city-wide errand.
- At most 10% of eligible outdoor residents may be taking optional walks at
  once, rounded down with one slot available for populations below ten.
  Admission must be deterministic and fair over time, not actor-id starvation.
  Start at most one optional walk at a time from an ordinary small patch.

Do not retarget a committed optional walk every ladder poll. Arrival ends
walking and starts a new dwell; it does not immediately select another path.
Failed routes release reservations and incur a dwell/backoff before retry.
No free slot or no useful route means standing still, not a fallback march
to the nearest square.

There must be one coherent definition of belonging to a patch. Being at a
valid spot in it satisfies the routine. The old 6 m workplace recall and
15–40 m generated wander leash do not apply to residents.

An explicit `go_to`, custody, or a necessary shelter movement can leave the
patch and bypass the optional-walk limit. Record those reasons separately.
An interrupted resident retains their identity and home; after release they
settle or return once by a valid path. There is no immediate leash snap,
teleport, or conflict with an ongoing conversation or explicit intent.

## Meals, evenings, and other sources of traffic

Removing work schedules must not simply replace commuters with hundreds of
hungry market customers. Give the resident routine an explicit support policy.

Use the simulation's existing abstract hearth support as the basis. A resident
can receive that support while settled in their assigned household patch at
meal offices. A resident with an appropriate hardship background has a local
support/resting patch with matching prose. This is background household or
community provision, not a new shop, item transfer, or delivery NPC.

Requirements:

- Retain actual hunger values and normal consumption of held/offered food.
  Define resident meal eligibility explicitly; do not rely on the accidental
  `legs.is_empty()` hearth fallback and its distance to a snapped base node.
- Support uses the existing game-time meal/refill accounting, not a refill on
  each ladder poll. Standing in the patch qualifies; passing through it does
  not repeatedly manufacture meals.
- Default residents do not automatically join public food or water queues.
  Ordinary support needs resolve locally. Explicit player/NPC transactions
  continue through the real inventory and economic rules.
- The abstraction changes need satisfaction only. It must not mint saleable
  food, take stock from an unrelated vendor, invent wage transactions, or
  change the existing household money settlement.
- Verify ordinary residents remain fed across multiple days. If they miss
  support because they are away on an explicit activity, preserve truthful
  needs and support a local recovery when that activity ends; do not send all
  hungry residents to the same global stall.

Housed residents still retire at night and return to their nearby daytime
patch. Use individual departure choices within the existing evening/morning
windows, and local paths, so this provides residential activity without a
cross-city march. Preserve the existing curfew and custody priorities.
Unhoused residents use local shelter/resting spots without being represented
as having entered somebody else's house.

Implementation audit: the current simulation has no authoritative indoors
state. Its home behaviour is proximity to a door. Retain that representation
for this feature, using valid resting positions beside the home approach;
do not add hiding or teleportation to simulate an interior. Report a
home-resting proxy separately while retaining those bodies in the present
population. A real indoors state belongs to future interior work.

Default residents must not inherit the current random evening trip to any
tavern or chalked destination in the city. A nearby visit can fit the same
local visit rules and movement budget. Authored routines, explicit workers,
and explicit `go_to` requests retain their existing semantics.

Weather shelter may interrupt lingering. Prefer reachable shelter in or
beside the patch and spread occupants among valid positions. Do not reuse a
single destination coordinate for everyone. Keep emergency/custody behaviour
authoritative; calm-weather stationary targets are not hard movement bans.

## Navigation changes

### Reach the chosen standing point

Resident local movement must reach its actual spot. Routing to the nearest
street node and declaring arrival is not acceptable.

Prefer a validated direct segment between nearby spots. When blocked, use a
bounded local search on the existing walkable surface, followed by validated
path simplification. Reuse graph connections where helpful, with validated
start and end connections. Both endpoints being walkable is insufficient:
every traversed segment must respect collision clearance, including corners.

If local search cannot find a route within its spatial/work budget, stay put.
Do not call the current `route_path_to_point` as an unchecked final-stride
shortcut. Compute routes on admitted transitions, not for every standing NPC
on every frame. This feature does not require a replacement city-wide navmesh.

### Give remaining traffic room

Audit the 0.6 m edge widths against actual walkable clearance. In the baker,
detours and connecting edges can receive a conservative 0.6 m width even
where some of the route has more room. Preserve genuinely narrow passages.

Measure safe clearance along an edge, subdividing where needed so one pinch
does not erase usable width across an entire open square. Sideways shifts and
their connecting segments must remain validated. Do not globally raise the
minimum width or remove collision checks to obtain a prettier line.

Retain stable per-actor lane variation and existing local separation. Where
the square supports it, walkers should use visibly different paths. Everyone
arriving at a resident patch ends at their own reserved spot rather than
converging on its central node.

Shared navigation corrections can alter authored actors' exact trajectories.
That is an explicit geometry effect of this milestone; it does not authorize
changes to their jobs, schedules, destinations, or movement speed.

## Ownership and integration

| Area | Responsibility |
|---|---|
| [crowd.rs](../../crates/cathedral-sim/src/crowd.rs), [lore.rs](../../crates/cathedral-sim/src/lore.rs), [appearance.rs](../../crates/cathedral-sim/src/appearance.rs) | Independent background, lodging, and routine selection; no jobs by default |
| [bake_navigation.py](../../scripts/bake_navigation.py) and geometry assets | Stable valid patches/spots and their connections; measured route clearance |
| [sim navigation](../../crates/cathedral-sim/src/nav/mod.rs) | Exact local destinations and bounded collision-safe paths |
| [round.rs](../../crates/cathedral-sim/src/round.rs) or an extracted resident module | Dwell state, reservations, optional-walk admission, domestic support, interruption/resume |
| [sim prompt](../../crates/cathedral-sim/src/prompt/mod.rs), [actor sheet](../../src/smart_actors/actor_sheet.rs) and snapshot projection | Truthful local routine, destination and movement reason |
| [body.rs](../../src/smart_actors/body.rs) and actor presentation | Existing poses/gestures and accurate standing/walking display |

Key existing integration points are `extra_ambient_sheets`/`lodgings`,
`Round::seed`/`build_legs`, `run_ladder`/`decide`/`apply_decision`,
`decay_needs`, `reroll_ambient_evenings`, `resolve_arrivals`, `route_path`,
`NavData::offset_route`, and `World::step_movement`. Recheck these against
current code when implementing; earlier milestone write-ups are historical.

The pure simulation owns behaviour in both Bevy and `cathedral-headless`.
Patch allocation and randomness are deterministic. Introduce no new LLM
scheduler lane or per-frame all-pairs population search. Standing residents
should need only cheap deadline/state checks; occupancy and patch neighbours
should be indexed.

Preserve generated ids, interaction reachability, personal knowledge, rumor
participation, inventories, and player-facing names. The movement routine
must not grant residents knowledge of everyone in their patch. Do not change
authored lore sheets to accommodate generation.

Less ambient travel will also reduce incidental meetings and rumor transport
between neighbourhoods. Accept that consequence of local residents; retain
authored travellers and deliberate visits. Do not silently restore long
ambient errands or increase LLM turn rates to compensate.

Prompts must describe a local resident routine rather than a nonexistent work
round. Do not fabricate a workplace, advertise every internal spot id as a
public place, or describe a moving actor as stationary. Debug views should
show routine kind, patch/spot, dwell remaining, and the actual movement reason.

This spec targets fresh session generation. Do not add a save system. If
checkpoint persistence has shipped before implementation reaches this work,
integrate routine state, deadlines, and reservations with its existing
protocol, including migration of older generated routines.

## Milestones

Implemented sequentially, with review and evidence per milestone. M0–M4 are
complete; the final acceptance record distinguishes measured results from
remaining capacity and presentation limits.

### M0 — Characterize the baseline and count actual motion

**Implemented 2026-09-06.** The opt-in headless `--trace-motion` diagnostic
compares actual positions and pre-poll movement causes, keeps generated and
authored populations separate, and reports queues, sampled time away, stable
20 m spatial observation cells, and the truthful home-resting proxy. These
cells are baseline observations, not the resident patches M1 will bake.

One complete normal-speed day at 1,000 / 2,000 extras, with 3,600-second days,
0.05-second polls, clear weather and fake cognition, measured **30.3% / 31.7%**
stationary across settled daytime samples. Neither run reached 85% in any of
its 45 valid settled daytime samples. The tests and detailed measurements,
including the excluded terminal float-boundary sample, are in
[M0 evidence](ambient_residents_evidence/m0.md). Deterministic camera and
geometry probes are saved there. Detached host tmux launches subsequently
produced real RTX 4070 captures in sessions 804/805; the earlier software
renderer attempts remain excluded. Those early hidden-window sessions waited
about one second per frame inside the NVIDIA driver, so their frame times
do not establish crowd rendering cost. M4 subsequently completed repeated
visual evidence at verified simulation times and two-day behaviour acceptance.
No default crowd behaviour changes in M0.

Add or extend a headless diagnostic for generated residents separately from
authored actors. Count stationary, optional walking, domestic walking,
explicit-intent walking, and emergency/custody movement directly from motion
and its cause. Report patch occupancy, time away, and queue membership.
Do not substitute the existing `at_post` census for stationary counts.

Record a normal-speed baseline at 1,000 and 2,000 extras using the current
world. Preserve the session-800 captures as motivating evidence, not a
normal-speed benchmark. Identify deterministic probe cameras and spatial
patch fixtures for later comparisons.

### M1 — Places and paths

**Implemented 2026-09-06.** The optional navigation catalogue has 2,665
patches and 9,171 individually spaced standing spots, with verified graph and
nearby-home connections. Exact bounded local routes and two-claim spot
reservations are implemented; actual mover tests cover corner safety and
settling after avoidance. Existing crowd policy, graph edges and widths stay
unchanged until the next milestones. See [M1 evidence and API handoff](ambient_residents_evidence/m1.md)
for capacity/door limits, independent clearance measurements and 1,036 passing
simulation tests.

Bake patches and individual standing slots, validate capacity/door clearance,
and implement bounded exact local movement. Prove a route reaches an off-graph
spot around an obstacle and rejects unreachable or over-budget destinations.
Implement destination reservation/release before enrolling the full crowd.

### M2 — Residents by default

**Implemented 2026-09-06.** Both hosts generate jobless supported residents
on reserved safe frontages, with housing independent of jobs, varied civilian
clothes, 45–150-second dwells, bounded optional walks and individual local
night/morning rest. Meal support includes Kindling breakfast and bounded
missed-meal recovery; a two-day normal-clock needs test remains above famished
without inventory or money changes. Explicit compatible worker overrides
retain their named workplaces and ordinary archetypes. Actual startup places
all 1,000/2,000 requested residents; at 20,000 it places 9,011 after authored
spawn blockers and reports 10,989 unplaced. All 1,036 simulation tests pass.
See [M2 evidence and integration handoff](ambient_residents_evidence/m2.md).
M3 completed the integration below; M4 completed full-population behaviour
and visual acceptance.

Separate livelihood, lodging and routine; add ordinary supported backgrounds
and appearance variation. Enable local lingering with dwell deadlines and
optional-walk limits, replacing generic wander/recall for residents. Include
meal support and local morning/evening behaviour in this milestone: shipping
jobless actors who immediately starve or queue is not a completed default.

Keep the explicit worker path opt-in and retain authored occupations.
Update old generated-crowd tests that deliberately pin the replaced policy;
preserve unrelated authored-cast and economic guarantees.

### M3 — Integration and remaining route geometry

**Implemented 2026-09-06.** Resident claims now hand off safely to explicit
travel, conversations, local weather shelter, custody and departure. Necessary
long returns use checked local connectors and the shared graph. Settled
conversations remain eligible for local meals. Snapshot, prompt and actor
sheet distinguish local routine, claims, exposure, dwell and movement cause;
internal spot IDs are confined to diagnostics, never public wayfinding.

Measured subdivision repairs all 41 unsafe original centreline edges while
preserving all 3,972 original node indices and existing place/door references.
The final graph has 10,026 nodes and 10,062 edges, at most 4 m long, with measured
half-widths of 0.35–4 m. The rebake provides 9,232 safe frontage spots and 159
independent positions under actual roof polygons. Authored movement and
separation now check complete swept segments too.

This exposed invalid authored starting positions. Mobile present actors now
receive an initial safe surface correction before first publication, avoiding
existing bodies and keeping their authored jobs, destinations and schedules.
The ordinary bound is 20 m; the reviewed Lanthorn exception permits 40 m only
inside its known nave for an actor with an authored Lanthorn leg. Dunstan Pike
and Betriss Marle needed 36.875 m and 36.625 m respectively to reach the same
building's navigable apron. Absent, confined and immovable actors retain their
representation. Every correction or unresolved start is diagnosed. The eight
other larger adjustments and the full checks are recorded in
[M3 evidence](ambient_residents_evidence/m3.md).

Drive `sleep-sim` now waits on existing virtual time for M4 captures, without
changing clock, speed, pause or the existing real-time `sleep` command.

Complete conversation, explicit intent, weather, custody, evening-roll,
prompt/debug, and reservation cleanup checks. Correct measured route widths
and preserve corner/door safety. Verify existing authored movement scenarios
alongside resident paths; no broader navigation redesign is required.

### M4 — Visual acceptance and tuning

**Implemented and accepted 2026-09-06.** Two complete normal game days at each
of 1,000 and 2,000 extras retain every resident, with **99.055% / 99.061%**
mean stationary across 101 settled daytime samples each. All samples meet
the mandatory ≥85% threshold; the quiet result is accepted above the soft
85–95% aim because frontages remain visibly inhabited and short local walks
still settle. Generated jobs, round legs, public food/water queues, famished
residents and sampled >15 m drift are zero across both clear-weather runs.
Matching first-day means improve from M0's 30.333% / 31.654% to
99.040% / 99.053%. Patch and independent spatial occupancy remain distributed.

Typed sparse resident-state publication avoids cloning the whole population
for each local arrival and preserves identical sampled behaviour. Final
pump/report cost is **4.522 / 8.970 ms per poll**, still about 56% / 37% above
the original one-day M0 measurements. A 20,000-request stress observation
places **9,072**, explicitly leaving **10,928** unplaced, at 43.952 ms per poll.
This is a measured capacity/CPU limit, not a 20,000-visible-crowd promise.

Hidden RTX 4070 runs produced **170 final screenshot/state pairs**, all with
the full requested population and normal 1× clock. Repeated ground/elevated
Wickmarket, Tenterhook, Burnt Court and Needle views show occupied frontages,
clear approaches and no recurring generated procession. Paired live states
establish local arrival/dwell, limited nearby rain shelter and clear return;
separate normal-speed Lamplight fixtures establish visible evening frontages
and real outdoor resting residents. Authored groups remain legitimate activity.
Same-camera daytime frame means are 11.083 / 12.913 ms, including ordinary
spikes. Earlier baseline concurrency/microphone/geometry differences prevent
a controlled causal GPU speedup claim. Intermittent long frames and shadowed
late-evening close-ups remain documented limitations. Exact commands, hashes,
sample definitions, actual capture times, screenshots, checks and independent
review are in [M4 evidence](ambient_residents_evidence/m4.md).

The acceptance scenarios and required checks follow.

## Acceptance and verification

### Behaviour measurements

At `extra_ambient_npcs: 1000` and `2000`, normal debug scale `1×` and the
shipped `seconds_per_day: 3600`, run at least two game days with the fake
backend. A headless driver may advance synthetic elapsed time faster than
wall time, but must preserve the same movement, needs and clock relationship.
Do not replace this with a shorter game day.

In clear daytime conditions after initial settling:

- Aim for 85–95% of outdoor default residents stationary across sampled time;
  require at least 85% stationary in 90% of samples. A quiet patch may be
  entirely stationary. Standing includes turning/gesturing without walking.
- The denominator includes all present outdoor default residents, including
  those travelling or queueing; do not hide walkers by calling them `at_post`.
  With the current presence model this means all `InCity` default residents;
  report home-resting as an overlapping proxy and indoors as unavailable.
  Report domestic/explicit/emergency episodes separately as well as in the
  full population totals.
- Optional movement respects the 10% admission limit; ordinary local changes
  respect the 15 m routed budget and nearby visits the 30 m budget.
- No default resident has an automatically assigned occupational round, well
  duty, vendor job, or public food/water queue membership.
- Patch occupancy remains distributed throughout the two-day run. No slow
  migration from residential patches onto landmark nodes or markets.
- Hunger recovers through specified support across meal offices; no persistent
  city-wide famished cohort caused by disabling their work rounds.

### Meaningful mechanical checks

Cover housed/no-job and unhoused/no-job identities; explicit worker opt-in;
deterministic assignment; spot reservation under simultaneous arrivals;
blocked/failed routes; no 6 m recall inside a valid patch; and no immediate
walk loop after arrival. Vary poll cadence to ensure missed deadlines do not
create a burst of catch-up trips.

Exercise conversations during dwell and movement, explicit travel outside the
patch followed by release, meal and night transitions, heavy rain, arrest,
and actor departure. Verify reservations are released and residents neither
teleport nor fight the controlling activity. Keep real food transactions and
household currency accounting intact.

For route geometry, include an open square, a narrow alley, a door approach,
and a wall corner. Check the whole swept path rather than endpoints alone.
At zero generated extras, run relevant authored movement, round, interaction,
and prompt tests. Shared width fixes may change a path; unexplained authored
behaviour changes are failures.

### Visual evidence

Use `CATHEDRAL_HEADLESS=1` for every Bevy verification run, following the
existing hidden-window screenshot instructions. No visible window, focus
grab, cursor grab, or audio.

Record ground-level and elevated views of the Wickmarket, a residential lane,
a court, and a narrow passage. Capture repeated views after 30 seconds and
several minutes, plus day/evening views. Show both clean views and debug views
that identify movement reasons. Single startup screenshots do not establish
persistent placement.

The evidence should show occupied frontages and courts, usable streets and
doors, small irregular groups, occasional distinct walkers, and arrivals that
settle. A line disappearing only because residents were hidden indoors or
fewer than the requested 1,000/2,000 were spawned is a failure. Smaller groups
of authored travellers remain legitimate activity.

Measure engine-pump and frame cost at the same supported populations and
settings as the baseline. Repeated routing of stationary residents or a
population-wide neighbour scan in each frame is unacceptable. Report results
at 20,000 as a stress/capacity observation, without making that count a new
frame-rate promise.

## Exclusions

This feature does not include debug fast-forward fixes, a new economy/supply chain, authored-cast
job removal, playable house interiors, new sitting/working animations, a
camera-driven population system, or a general city navigation rewrite.
