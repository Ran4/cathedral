# Implementation review notes

2026-09-06. Parent coordination notes for the sequential, fresh-context
milestone agents. These are integration findings, not completed acceptance.

- Preserve the developer's root `AGENTS.md` edits and untracked
  `docs/codex_gdd/`. Do not stage or commit unrelated work.
- Current `Presence` is `InCity` or `BeyondTheWalls`. There is no authoritative
  indoors state. The existing home census is a proximity proxy; do not remove
  it from the actual-motion denominator or manufacture indoor counts.
- M0 found that the old headless watch-clock driver can advance 3 seconds per
  poll while movement catches up at most 0.4 seconds. The new motion diagnostic
  opts into 0.05-second polls. Use its documented invocation for comparisons;
  debug fast-forward behaviour is outside the feature.
- Existing `NavData::segment_walkable` samples at cell-sized intervals. M1
  should consider conservative grid traversal for local-path corner checks:
  endpoints alone, or skipping a touched blocked cell, are insufficient.
- `World::step_movement` applies local separation after the tentative step.
  Account for small arrival displacement when defining spot arrival; never
  create perpetual re-routing because a neighbour nudged a body slightly.
- M2 must integrate domestic support with the default routine. Null occupation
  alone still leaves generic hunger, wandering, and night behaviour active.
- `AppearanceSnapshot::compose` currently maps null occupation directly to
  `Poor`, independently of circumstances. Add a generated-resident appearance
  path rather than inventing a fake occupation for clothing or changing every
  authored no-trade character. Existing body presentation already supports
  breathing, weight shifts, and glances when settled; new animations are not
  required to make persistent standing look alive.
- Shared lane-clearance corrections may change authored trajectories, but
  must not alter their occupations, destinations, schedules, or walking speed.

M0 review complete: route causes are observational state; the probe counts
position changes across a poll and retains the full present population. All
1,029 simulation tests and six headless tests passed. Valid settled-daytime
samples were 30.33%/31.65% stationary at 1,000/2,000 extras. See `m0.md` for
the captured causes, endpoint exclusion, and exact reproduction commands.

GPU launch follow-up: use a new detached host tmux session for real NVIDIA
access; the exec tool's device namespace cannot access the GPU. Keep
`CATHEDRAL_HEADLESS=1`, audio disabled, `BEVY_ASSET_ROOT` set, and mapping/focus
checks active. Sessions 804/805 produced genuine RTX captures. However they
were paced at about 1 Hz, so the apparent wall-frame cost is not an ordinary
performance measurement and drive wall waits overstate simulation age. A
separate verification subagent confirmed repeated sleeps inside NVIDIA's
driver, including at zero extras. Safe launch flags did not remove them;
see `gpu_launch.md`. Use verified screenshot HUD times and normal-step
headless runs for formal behaviour. Do not treat drive wall waits as simulation
seconds or report the driver-limited wall frame time as crowd cost.

M1 home integration: validated door paths are access metadata, not resting
positions. Only 176 baked safe spots fall within the old 3 m home-proximity
radius; do not gather residents on their door nodes to satisfy that proxy.
Use reserved, safe frontage resting spots and explicitly describe the resident
home-rest proxy. The final M1 bake has 7,093 home-connected spots across 939
doors; caps of one/two occupants allow 939/1,878 housed residents, enough for
the ordinary-support share at 1,000/2,000 total. Index resident spot/patch
handles at enrollment; convenience metadata lookup methods scan the geometry.

The unchanged pre-feature Bevy executable is preserved at
`/tmp/cathedral-ambient-before` (SHA-256
`d244c26260a172e3b917f370071a6e9865850d8353f87a89f6f88d8e60a81c25`).
This permits additional visual comparisons after the current binary is rebuilt.
It still loads simulation assets from the compiled-in repository path, so any
later run must record the then-current navigation asset and must not silently
claim it reproduces the original route geometry.

M1 review complete: 1,036 simulation tests pass, backend check passes, and the
independent geometry/rebake proof passes. No default routine was changed.

M3 geometry pre-audit: exact supercover validation finds 41 of the 4,008
existing centreline edges touch blocked cells; see
`pre_m3_centreline_audit.json`. Merely validating wider shifted routes and
falling back to those centrelines would not establish swept safety. Keep any
repair local and measured. Preserve existing node indices when inserting
width subdivisions or corner repairs: `assets/world/places.json` and
`assets/world/shelters.json` carry external node references. If any original
node changes, validate/regenerate every dependent reference explicitly.

Longer pre-feature GPU views started in session 810 through the guarded
`capture_views.py` helper, with 1,000/2,000 sequential runs at approximate
30/180/600 simulation seconds. Requested ages are estimates only; record the
HUD times in the actual captures. This is visual evidence, not a frame-cost
benchmark. The second population loads navigation at its own startup, so
coordinate M3 asset mutations or record any geometry difference truthfully.

Timing correction after inspecting session 810: the controller overrides
Bevy's virtual max delta to **100 ms** (`MAX_FRAME_CATCHUP`), so 1 Hz means
approximately **10 wall seconds per simulation second**, not four. The
initial GPU report missed this override and is being corrected. The running
`ambient_before` captures retain their original `approx30/180/600` filenames
and ratio 4 metadata; those names overstate actual simulation age. Inspect
HUD times (first Wickmarket view is 07:05, about 12.5 simulation seconds), label
the recorded times truthfully, and use the helper's corrected ratio 10 default for
new runs. Neither the controller nor the debug clock was changed.

M2 review complete: full sim and six headless tests pass, real startup places
all 1,000/2,000 requests with zero workers, and two-day support keeps hunger
above the famished threshold. The initial meal-boundary floating-point bug
is fixed. The first M2 agent encountered service-capacity failures; a fresh
replacement completed review/tests and `m2.md`. Parent updated the current
CLAUDE crowd documentation. M3 owns the explicit integration issues listed
in `m2.md` before M4 full-engine acceptance.

The first long pre-feature GPU run completed in session 810 with 24 captures
and 2,299 unmapped/unfocused checks. The second began in session 811 at
17:29:55 CEST and loaded the M1 asset, so M3 can now modify geometry. Note
the subsequent recovery to normal frame throughput documented in
`gpu_launch.md`; fixed-ratio screenshot ages remain unreliable.

Both long pre-feature GPU runs are now complete: sessions 810/811 each saved
24 images, with 2,299/2,208 map/focus observations, all unmapped and unfocused.
Actual screenshot ages and visual qualifications are in `m4_camera_notes.md`;
`inspect_gpu_timing.py` and the two `ambient_before_*_timing.json` files retain
the frame/drive timing audit. In particular, both populations visibly form a
long Wickmarket procession at normal 1×. Later 2,000-extra frames include a
fake-transcription conversation; disable microphone input using the existing
V toggle in controlled final GPU runs.

M3 review addendum: strict shared movement checks expose invalid imported
authored starting positions. The initial parent raw-bitset audit mistakenly
read little-endian bits; the corrected MSB-first report is
`m3_authored_spawn_audit.json` (23 blocked/out-of-grid and 11 unsafe boundary
points among 519 sheets, before road/custody enrollment). M3 must use actual
post-seed presence and navigation checks. A bounded correction before the
first world publication is authorized for mobile present authored actors,
respecting already allocated bodies and preserving lore/jobs/destinations.
The normal bound is 20m. Two starts inside the excluded Lanthorn nave have
existing outward Lanthorn work legs and need a precise same-building 40m
initial exception; leaving them unchanged would freeze those existing trips.
Absent road parties, custody, and immovable authored actors retain their
representation. No unsafe runtime escape or interior navigation is added.

M3 review complete: 1,046 simulation, 174 backend and 500 app tests pass.
The latest app failure was an interaction fixture standing 4.843m from Ilse
after the measured lane shift, outside the unchanged 4m transfer range. The
fixture now approaches the actual bodies and preserves the full exchange and
cleanup checks. This was a diagnosed geometry effect, not a flaky timeout.
All three startup populations have 28 safe initial corrections and zero
unresolved eligible actors; no overlap with allocated residents is introduced.
The final bake has 9,232 frontage spots, with 9,072 placed at a 20,000 request.
The route proof preserves original references, finds no unsafe final segment,
and verifies rebake equality and refinement idempotence. See `m3.md`.

M4 publication review: resident phase/claim changes initially bumped the cold
world revision, repeatedly cloning a full public snapshot during local
movement. The final bridge carries changed resident records separately.
Initial Ready and subsequent full snapshots include and supersede these
records; absent/removed actors are filtered, and the mirror updates only
existing entries without changing position, inventory or revision. Countdown
alone causes no publication. Actor sheets and optional screenshot evidence
read the live engine state, including current dwell, rather than treating a
cached countdown as current. Round and mirror regressions pass. The isolated
600-second comparison at 1,000 extras reduced pump/report cost from 5.733 to
4.556 ms per poll (20.5%); final two-day measurements remain the acceptance
record. Behavioural rules were unchanged by this publication fix.

Final diff cleanup: repository-wide formatting had changed unrelated files.
The parent restored 23 files only after proving their HEAD and current source
produce identical rustfmt output. Three actor-rendering files retain only
their required new fixture fields, with the same canonical-output proof.
`formatting_cleanup.json` records the paths and canonical hashes. No semantic
change or initial user edit was removed. No build or test was run alongside
the formal population measurements.

Arrival presentation review: the existing body renderer treats a movement
sample older than 0.18 seconds as stopped and blends walking down over 0.25
seconds. Its established idle layers provide breathing, weight shifts and
glances. The resident feature therefore needs no new standing animation or
continuous fake movement update; final captures still verify actual arrivals.

The parent independently validated both final two-day summaries; all 101
settled daytime samples pass at each population. The full 240 post-initial
observations retain 1,000/2,000 present residents, with zero generated jobs,
round legs, public queues, famished actors or displacement beyond 15 m from
initial positions. `acceptance_review.json` records the checks and input
hashes. The stress run also confirms 9,072 safe placements from 20,000 requests.

Initial final GPU review, session 812: the parent viewed Wickmarket ground
and elevated, Tenterhook ground, Burnt Court ground, Needle ground, and the
east frontage at 07:13–07:17. The HUD reads 1× and MIC OFF. Frontage bodies
are visible, the court and passage remain open, and the square is sparse.
At 08:17–08:21 the court and east frontage remain occupied. Resident x00412
is lingering at both east-frontage requests, moving from (9.125,277.125) to
(11.125,277.375) between them. The later Wickmarket ground frame honestly
retains a small group of named passers-by; the generated residents in the
surrounding sidecar are lingering. Appearance or nameplate alone must not
be used to assign a generated identity or movement cause. These early images
do not substitute for the later, weather and second-population checks.

Later independent GPU review: session 812's selected x00590 has an actual
optional route in the clean moving sidecar, arrives before the first debug
capture, and stays at the same position while its dwell decreases. Its rain
follow-up x00153 remains at the same roof slot through the later capture.
The parent also viewed the main final Wickmarket/east-frontage images; their
17:43–17:46 HUD times are late afternoon, despite the `evening` filenames.
The additional Lamplight fixture addresses that gap explicitly.

For session 813, the parent compared Wickmarket ground at 08:17 with the
original session 811 ground view at 08:10. The original long procession is
gone; a smaller named group remains visible. Tenterhook at 08:18, Wickmarket
elevated at 11:10 and Burnt Court ground at 11:12 show occupied building edges
and usable open space. The parent inspected authoritative x01193 state in
the preceding complete sidecar: a local route and speed 2.1 m/s become no
route at the destination by the clean close-up. Subsequent debug records
retain that position with dwell decreasing from 86.32 to 70.80 seconds.
Thus the close-up filename is not used to claim it was still walking.

The parent viewed session 813's rain debug and later clean images at
12:17 and 12:42. Both sidecars place x00446 at (-31.375,326.625), with the
same reserved Wool Gate roof slot, Weather cause, sheltered=true and no
route. Each retains all 2,000 generated residents. The visible persistent
body and authoritative roof membership establish the local shelter result;
rain pixels alone would not establish cover. The older debug phrase
“never walks” is a presentation defect addressed in the final UI build.

Main session 813 completed all 61 captures with every window check unmapped
and unfocused. The parent viewed its final Wickmarket and east-frontage
frames at 17:43 and 17:47: a small named market group remains, while two
generated bodies occupy the east frontage. The final full-population sidecar
also returns x00446 from shelter to its own patch `rp_omb_i0541_e0_c0`, at
(-55.375,326.625), lingering without a route and with hunger 255.0. This
independently confirms that local shelter did not become a new permanent
market or roof destination. The same-camera frame comparison retains all
frames in its selected complete minutes, including ordinary large spikes;
baseline concurrency and microphone differences prevent a causal speedup claim.

Final evening review: both separately initialized Lamplight fixtures completed
24 PNG/sidecar pairs, exits 0, and 235/225 unmapped/unfocused checks at
1,000/2,000 residents. The parent viewed early court/frontage/lane images,
the resting debug close-ups at 19:22, and their clean follow-ups at 19:35.
The debug label now correctly reads “Stationary (no active route)”. The
late body is shadowed; the earlier frontage view establishes its visible
placement, while authoritative state and the debug panel establish resting.

In both fixtures x00412 changes from lingering at 18:17 to household-frontage
resting at 19:22, at the identical position (9.125,277.125). It stays there
through the later follow-up, with no route or target and hunger 255. The
two debug records are about 30.59 simulation seconds apart; dwell decreases
from 86.20 to 55.62 seconds at 1,000 and 86.49 to 55.88 at 2,000. All capture
records retain their full populations at scale 1, with indoors unavailable.
These fixtures establish evening presentation and resting behaviour; they
do not pretend to be uninterrupted morning-to-evening GPU sessions. The
formal full-engine two-day traces supply that continuous transition evidence.

The parent accepts M4 with the recorded CPU increase, shadowed late close-up,
and hidden-driver timing limitations. No outstanding implementation or visual
acceptance issue remains. Final documentation and backlog archival follow.
