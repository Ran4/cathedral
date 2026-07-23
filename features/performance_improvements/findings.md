# Performance: measurements and fixes (overnight session 2026-07-23)

Goal: rock-solid 60 fps after the first minute, headroom for 1500 NPCs (today ~510).
This file is the measured follow-up to `old_performance_improvement_suggestions.md` —
that document guessed; this one measures. Some of its guesses were right (no spatial
grid, unguarded transforms), some already landed (hot/cold snapshot split, 20 Hz
movement tick, VisibilityRange on NPC parts), and the biggest problems turned out to
be things it never mentioned (a 10 Hz material-invalidation bug, city-wide AABBs on
the batched meshes).

## How to measure (new tooling, kept)

- `CATHEDRAL_PERF=1 cargo run --release` — switches vsync off, records every frame,
  writes `perf_frames.jsonl` (per-second frame-time arrays) and `perf_summary.json`
  (percentiles overall/steady-state, worst frames, all Bevy diagnostics including
  render-pass GPU timings) into the session directory, plus a 5 s percentile line to
  `logs.jsonl` (source `"perf"`). `CATHEDRAL_PERF=vsync` keeps vsync to observe pacing.
- `CATHEDRAL_FAKE_BACKEND=1` — forces the offline deterministic engine without
  editing config.ron.
- `CATHEDRAL_DRIVE_RES=1920x1080` — drive-mode runs at play resolution.
- Standard suite: `scripts/perf_suite.sh` — a 10-vantage tour at 720p and
  1080p, a stand-still run, a walking run (`hold KeyW`), and a vsync pacing
  run; each summarized by `scripts/perf_report.py` (one line of percentiles
  per run).
- Rolling cost attribution in `logs.jsonl`: `[engine pump]`, `[bridge drain]`
  and `[body pose]` avg/max lines every 5 s.

## Baseline (before any fix)

Session 430, 1280×720, fake backend, no vsync, clear weather, RTX 4070 + 20-thread CPU:

| metric (steady state, t>60 s) | value |
|---|---|
| median frame | 13.5 ms |
| p95 | 28.7 ms |
| p99 | 34.9 ms |
| frames over 16.7 ms | **39%** |
| skyline vantage median | ~21 ms (~47 fps) |

The frame-time distribution is bimodal everywhere: quiet frames ~9 ms, spike frames
22–35 ms, ~10 spikes/second, often on consecutive frames — not one periodic system,
but (as it turned out) a 10 Hz invalidation cascade plus unguarded per-frame churn.

GPU pass timings (RenderDiagnosticsPlugin, street level, 720p): early prepass 0.65 ms,
main opaque 0.99 ms, SSAO 0.72 ms, SMAA 0.32 ms, bloom 0.21 ms, transparent 0.13 ms —
the *instrumented* passes total ~3.2 ms, yet frames take 9–13 ms with GPU utilization
57–98%. The unaccounted majority is the shadow passes (4×4096² cascades + ~20 local
shadow views), the atmosphere LUTs, and volumetric fog — none instrumented by Bevy.

## Root causes found (ranked)

(details in the fix log below; file:line references are pre-fix)

1. **`update_wet_materials` inverted throttle** (`src/weather/materials.rs:92`) — the
   guard `if !changed && now < next_update_at` falls through every 100 ms in *steady
   state*, marking the 10 most-shared city materials Modified 10×/s forever. Every
   Modified material re-prepares and re-bins every mega-batch entity bound to it.
   Matches the measured ~10 spikes/second exactly.
2. **City mega-batch meshes have city-wide AABBs** (`src/city/mod.rs:7278`) — ~45
   batches each spanning the full 1.2×1.0 km map. Frustum culling and shadow-view
   culling never cull them: the whole city is vertex-processed in the main pass, in
   all 4 sun cascades (4096², max distance 320 m, sun moves every frame), and in all
   ~20 interior shadow-light views (2 spot + 3 point shadow casters in the cathedral).
3. **Per-frame material invalidation** — six more StandardMaterials marked Modified
   every frame even in clear weather (hidden cloud sheets ×2, rain, impact, puddle,
   rose-window emissive).
4. **Unguarded whole-cast Transform writes** — `reconcile_actor_views` and
   `drive_npc_bodies` rewrite every actor root Transform every frame (and
   MovementInbox entries never expire), so all ~510 puppet subtrees (~9,600 entities,
   18–19 per NPC) re-propagate and re-extract every frame.
5. **Chimney smoke** (`src/city/smoke.rs:178`) — up to ~7,200 camera-facing quads
   rebuilt, distance-sorted and re-uploaded (0.5–1 MB fresh allocations) every frame,
   `NoFrustumCulling`.
6. **Engine::poll on the main thread** — the whole sim (round::tick ~13 O(N) passes,
   ~7 brute-force O(N) proximity scans, 510-key clone per frame) runs inside the
   frame; movement catch-up may run up to 64 O(N) slices in one poll after a hitch
   (hitch amplification). Player walking republishes the full cold snapshot at 10 Hz
   (thousands of allocations per bump).
7. **Synchronous file IO on the main thread** — prompt archive (~130 KB .md+.json per
   LLM turn) and session log (write+flush syscall per line under a shared mutex).
8. **Small steady leaks of change-detection discipline** — well/gate mechanism
   transforms written at rest, `WellMechanismActivity` resource written unconditionally,
   HUD clock/connection Text rewritten per frame, soundscape full-cast scans with
   per-actor String clones (~5 passes/frame), 7 weather stem sinks volume-set per
   frame, map markers dirtying UI layout per frame.
9. **120 Hz fixed-step player collision** against 3,335 colliders with no broad-phase
   (2 full scans/frame steady, more during catch-up — amplifies any hitch).
10. **Volumetric fog toggling** compiles its pipeline on first fog onset (~100 ms+
    hitch mid-session) and steps to a 64-step raymarch over two city-scale volumes.

## Fix log

### Round 1 — invalidation & culling (landed)

- `update_wet_materials` guard inverted → fixed (`weather/materials.rs`): steady
  state no longer touches any material; transitions capped at 10 Hz.
- `animate_clouds` / `update_weather_materials` / `update_lightning_flashes`:
  materials written only when the computed value differs; hidden cloud sheets
  skip everything.
- `reconcile_actor_views` gated on mirror change (≤10 Hz), Transform
  compare-before-assign; `drive_npc_bodies` stops rewriting arrived walkers;
  `drive_gesture_pose` snapshot sync + `reconcile_hand_props` gated on mirror
  change (+ handover-flight edges).
- `update_actor_focus` distance-culls before collecting/sorting.
- Well/gate mechanisms stop rewriting rest transforms; `WellMechanismActivity`
  uses `set_if_neq`; NPC body-sound scheduler runs at 4 Hz; soundscape
  occupancy counts cached at 2 Hz; HUD/voice-panel/map-marker writes
  compare-guarded (including the `Mut` auto-deref trap in `set_optional_text`).
- Cathedral interior shadow lights (2 spot + 3 point = 20 shadow views ≈ 5 ms
  render-thread CPU) gated by player distance (`scene.rs`,
  `InteriorShadowLight`).
- **City mega-batches split into 128 m ground tiles** (`city/mod.rs
  spawn_batch`) — culling finally works. Detail tiles (props, laundry, hoists,
  awnings, balconies…) additionally get `VisibilityRange` fades inside the fog
  distance. City Mesh3d entities: ~700 → 2,604.
- Chimney smoke: per-plume distance fade/cull at 340–450 m (~600 plumes → the
  ~100 the fog leaves visible at street level; the two smoke tests now pin the
  culled contract). A 30 Hz rebuild throttle was tried and dropped — it broke
  test determinism and the CPU rebuild was never the measured cost; the cull
  shrinks the sorted/uploaded batch, which was.

**Tour numbers, 720p no-vsync fake-backend (steady state t>60 s):**

| metric | before | after round 1 |
|---|---|---|
| median | 13.5 ms | **7.0 ms** |
| mean | 15.1 ms | 8.6 ms |
| p95 | 28.7 ms | **17.1 ms** |
| p99 | 34.9 ms | 21.9 ms |
| frames > 16.7 ms | 39% | **5.6%** |
| prepass+opaque vertex invocations | 6.8 M | **0.84 M** |
| main opaque GPU | 0.99 ms | 0.54 ms |

Remaining spikes cluster at teleports (relocation cost: tile prep, audio
reseed, snapshot) plus a smaller continuous residue — ablation quadrant
running to attribute it.

### Round 2 — IO off the frame, message coalescing (landed)

- Prompt archive (`cathedral-backends/prompt_log.rs`): `.md`/`.json` writes
  ride a dedicated writer thread (FIFO keeps the filename contract; `flush()`
  + `Drop` keep the archive complete; tests flush before reading).
- Session log (`session_log.rs`): INFO lines buffered, flushed by a 250 ms
  ticker + atexit hook; WARN/ERROR still flush inline.
- Bridge drain (`smart_actors/mod.rs`): when several full snapshots arrive in
  one frame (post-hitch recovery), only the newest replaces the mirror —
  earlier ones each cost an O(cast) validation for dead state.

### Round 3 — attribution of the residual spikes

Stand-still ablation (90 s at the wickmarket, 720p, no vsync, after rounds
1–2; `CATHEDRAL_NO_ACTORS` / `CATHEDRAL_NO_WEATHER`):

| configuration | p95 | p99 | frames >16.7 ms | >22 ms |
|---|---|---|---|---|
| full game | 15.9 | 20.4 | 398 | 41 |
| **no actors** | 13.7 | 15.7 | **27** | **0** |
| no weather | 17.7 | 22.0 | 631 | 95 |
| neither | 13.3 | 16.1 | 89 | 15 |

Weather is innocent. **Every remaining spike is the smart-actor stack**, and
the new rolling timers (`[engine pump]`, `[bridge drain]` in `logs.jsonl`)
narrow it further: the bridge drain is trivial (max ~0.5 ms) while
`Engine::poll` spikes at **5–7.7 ms on single frames** — the sim's own
whole-cast passes (round ladder, attention scans, movement) plus fake-mode
turn processing, all on the main thread. That is what the cathedral-sim
changes attack:

- movement catch-up cap 64 → 8 slices (0.4 s; the remainder already snapped
  `movement_now` to `now`, so only the constant moved) — a hitch no longer
  amplifies itself with up to 64 whole-cast movement slices in one poll;
- `round::tick` gated to the 20 Hz movement cadence instead of every render
  frame (its dt/office math reads its own stored anchors, so cadence changes
  when it runs, never how much time it accounts), plus a reused scratch
  buffer for `run_ladder`'s per-tick id list;
- player spatial updates no longer call `touch_public_state()`: the 10 Hz
  walking republish of the whole cold `PublicSnapshot` (every actor + item
  cloned and revalidated) is gone. The snapshot is output-only — the sim
  never reads its own emitted player position — so sheets/percepts are
  unaffected; NPC position writes through `update_positions` (teleports,
  tests) still bump. Three sim tests that pinned "a player move bumps the
  revision" were updated to the new contract (they now pin monotonicity via
  an NPC move), and a new test pins both directions. All 554 sim tests green;
  golden prompts untouched. Additional Round-3 host fixes: name-label/thinking-
indicator systems stopped rebuilding N-entry String maps per frame and
stopped re-flagging ~1,000 hidden UI nodes per frame; clock HUD text guarded;
volumetric fog/light pipelines warm at startup (the first mid-play fog onset
used to compile them — a one-time ~100 ms hitch) with on/off hysteresis;
lightning flash range trimmed 1,400→600 m (reverted in round 4 — see below).

### Round 4 — adversarial review of the night's own diff

A 13-agent review/verify pass over the diff confirmed and led to fixing:

- **The mirror gates were inert**: `process_engine_message` took
  `&mut WorldMirror`, so passing the `ResMut` deref-flagged the mirror on
  *every* engine message (Clock/Weather arrive every frame) and
  `mirror.is_changed()` never skipped. Fixed by passing the `ResMut` wrapper
  and deref-muting only in the two snapshot arms. (The same `Mut`
  auto-deref-coercion trap as the HUD helper — it is worth grepping for
  `&mut <resmut>` arguments in any future change-detection work.)
- Lightning flash range reverted to 1,400 m — strike origins sit at
  y ∈ [360, 680] m, so a 600 m range meant most flashes never reached the
  ground at all.
- Detail-tile fade bands widened (`use_aabb` gauges the AABB *center*, up to
  ~90 m behind a 128 m tile's nearest corner).
- Interior shadow gates widened past the nave's own 152 m sightline so the
  toggle can never pop in view.
- Drive `hold` re-asserts the key every frame (a window focus loss would
  silently release it while the evidence log claimed a full walk).
- Prompt-archive writer got an atexit flush (the drive watchdog exits without
  destructors); drive/session log lines flush inline again (their durability
  is a documented contract).
- The perf recorder's own 5 s summary (an O(n log n) sort over the full
  history) moved off the main thread — on long runs it was writing phantom
  5 s-period spikes into its own data.

Rejected after verification: the snapshot-coalescing ordering hazard
(unreachable given the 1:1 pump→drain chaining; the coalescing was
nonetheless tightened to strictly-consecutive snapshots).

## Final numbers (all rounds, including the sim changes)

`scripts/perf_suite.sh`, fake backend, steady state (t>20 s per run), after
the round-4 review fixes (sessions 446–450):

| run | p50 | p95 | p99 | max | frames >16.7 ms |
|---|---|---|---|---|---|
| tour 720p | 7.1 | 15.4 | 18.3 | 29.9 | 2.4% |
| tour 1080p | 7.0 | 15.5 | 17.7 | 33.5 | 2.2% |
| stand-still 1080p | 7.1 | 15.0 | 17.2 | 22.9 | 1.4% |
| **walking 1080p** | 5.7 | 13.4 | **16.0** | 24.4 | **0.6%** |
| vsync 1080p | 16.7 (locked) | 20.1 | 21.8 | 26.1 | — |

Against the baseline tour (13.5 / 28.7 / 34.9 ms, 39% over budget): medians
halved, the tail cut by ~17 ms, over-budget frames down 20–60×. **Walking at
1080p — the way the game is actually played — has p99 under the 16.7 ms
budget.** The vsync run's worst frame in 90 s is 26 ms: not one true
missed-vblank double (33 ms) anywhere, including the teleport settle. 1080p
costs the same as 720p — the GPU has large headroom; the frame is CPU-shaped.
(The tours teleport every 8 s, so their tails measure relocation cost, not
play.)

The residual 17–22 ms tail (~1–2 events/min) is `Engine::poll` turn
processing: the pump timer still reports 5–6 ms single-poll maxima when the
fake backend churns turns at the stage — prompt rendering and turn fan-out,
inside the frame. Live play triggers these only when an LLM turn actually
starts/lands, so fake mode overstates their frequency. Next lever if they
bother: render the prompt off-thread (architecture change — the sim is
deliberately synchronous), or budget the scheduler to open turns only on
frames with headroom.

## The path to 1500 NPCs

What tonight bought: everything render-side that used to be O(cast) *per
frame* is now O(visible) or event-driven. What still scales with N, in the
order it will hurt:

1. **`Engine::poll` whole-cast passes** — round::tick now runs at 20 Hz (not
   per frame) and player walking no longer republishes the snapshot, but the
   attention/audibility scans and movement slices are still brute-force O(N).
   At 1500 expect roughly 3× today's pump cost. The right fix is the shared
   spatial grid inside the sim (one coarse cell index rebuilt per movement
   tick, consumed by `characters_within`, neighbours, and sound fan-out) —
   `old_performance_improvement_suggestions.md` item 4 remains correct and
   remains undone.
2. **18–19 entities per NPC** (≈28k entities, ≈17k draw meshes at 1500) —
   per-entity visibility/extraction overhead. The old plan's item 3 (flatten
   anchors, merge the puppet's static parts) is the lever; it was too invasive
   for one night against a cast that now renders cheaply at 510.
3. **Two resident UI text nodes per NPC** (≈3k nodes at 1500) — pool
   `MAX_VISIBLE_NAME_LABELS`+1 nodes instead (old plan item 5). The label
   *systems* are already O(nearby); only taffy's resident-tree size still
   grows with N.
4. **Cold snapshot validation** — O(N) per revision bump. Bumps are now rare
   (real state changes only), but at 1500 each one costs 3×; if it shows up,
   diff-validate against the previous snapshot instead of revalidating the
   world.

Explicitly not done, measured as not-worth-it tonight: a broad-phase for
player collision (3,335 colliders scanned at 120 Hz measured ~0.2 ms/frame,
constant in N), animate_body_pose changes (tiering already caps it at
~100 µs), skinned-mesh puppets (visual-quality decision, not a perf one).

### Environment note (important for anyone measuring)

X11 blanks the screen after 10 minutes (`xset q`: timeout 600); an occluded
window is throttled to exactly 1000 ms/frame, which reads like a catastrophic
regression. Measurement sessions must `xset s off -dpms` first (perf tour
scripts do not do this globally — it re-enables on reboot).

Running `./target/release/cathedralbevy` directly (instead of `cargo run`)
loses the asset root — Bevy falls back to the executable's directory and every
texture/sound/font 404s. Export `BEVY_ASSET_ROOT=<repo>` for direct-binary
runs; the perf scripts now do.

### Pre-existing failures (not from this work)

- `cathedral-backends/tests/curiosity_walk.rs::one_person_in_five_speaks_first…`
  fails on the untouched tree (24.4% vs ~20% target) — reproduced via
  `git stash` before any of tonight's changes were in play.
