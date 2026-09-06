### The crowd knob

`config.ron: smart_actors.extra_ambient_npcs` (0..=20000, default 0,
`CATHEDRAL_EXTRA_NPCS=n` for one run) requests extra persistent ambient
residents. Generation lives in `crates/cathedral-sim/src/crowd.rs`; their local
routine lives in `round/residents.rs`. They have six-character ids
(`x00000`…), personal identities, inventories and knowledge, and remain
interactable strangers. They have no authored character file.

**Generated residents have no job by default.** Occupation, title and rank
are null; housing and livelihood are separate choices. About three quarters
belong to supported households with nearby doors, while the existing
approximate hardship quarter has local support and no settled household
door. Ordinary residents use varied civilian clothes. A generated worker
requires an explicit occupation-and-workplace override through the generation
API; shipped hosts provide no worker overrides. Authored actors, including
authored ambient workers, retain their jobs and daily rounds.

Residents start at individually reserved standing spots beside building
frontages, courts and square edges. These positions are baked into
`navigation.json: resident_places`, with verified local routes and protection
for walls, doors, service areas and through-routes. Allocation balances
patches and household doors and excludes existing bodies. The generator
reports **requested, placed and unplaced** counts; the request ceiling is not
a promise that 20,000 people fit. Exhausting safe capacity leaves excess
actors unspawned. The current bake places all 1,000/2,000 requests and 9,072
of a 20,000 request after accounting for the authored starting bodies.

A resident normally lingers for 45–150 elapsed simulation seconds and may
then change position within the same frontage. Optional walks use exact
destinations, reservations and a 15 m routed budget, with at most 10% of
eligible residents admitted at once and one optional departure per small
patch. Arrival starts another dwell. A failed route or unavailable place
means waiting. Residents do not inherit the occupational recall radius,
generic wander roll or a city-wide workplace commute.

Household or neighbourhood provision satisfies needs locally at meal
offices, including breakfast at the Kindling. Hunger remains real, held food
can still be eaten, and missed meals have bounded local recovery. This is
abstract hearth support: it creates no saleable items, vendor transactions
or wages. Default residents do not join public food or water queues or take
the cast's well-keeper and vendor posts.

At night residents rest at safe places beside their household or familiar
frontage, with individual morning/evening timing. A home connector is an
access route, not a shared stopping coordinate. There is no authoritative
indoors state: residents remain present bodies, and resting is reported as
an outdoor proxy. The generated routine does not roll an evening destination
at a distant tavern or ward sign.

Standing residents retain existing speech, curiosity, gestures and
interaction rules. Explicit travel, conversation, weather and custody use
the simulation's ownership rules. Their presence does not add an LLM
scheduler lane: stage capacity and the existing cognition slots still bound
provider work.

Local rain shelter uses individually reserved positions under actual nearby
roofs. Residents without available local cover remain exposed; a household
assignment does not count as being indoors. Street lanes use measured
clearance and checked connections. Invalid imported starts for mobile authored
actors are corrected to nearby safe ground before the world is first shown;
their jobs, rounds and destinations remain intact.

`cathedral-headless --trace-motion` measures actual displacement and its
cause separately for generated and authored populations, including queues,
hunger and the full present-population denominator. It uses 0.05-second
watch-clock polls so movement, needs and calendar time advance consistently.
The older `at_post` census is not a stationary count. The ambient residents
two-day probes at 1,000 and 2,000 residents average about 99.06% stationary in
settled daytime samples, with the full populations present. Mean pump/report
cost is 4.52/8.97 ms per normal poll; the earlier one-day baseline was
2.90/6.55 ms. The [implemented spec](features/implemented/ambient_residents.md)
and [acceptance evidence](features/implemented/ambient_residents_evidence/m4.md)
record geometry, behaviour and frame measurements, including that CPU tradeoff.
