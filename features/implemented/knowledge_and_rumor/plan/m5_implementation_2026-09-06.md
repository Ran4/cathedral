# M5 implementation and self-review — 2026-09-06

Implementation owner: `/root/m5_implementation`. Review, full gate, final measured tune,
CPU/RSS, screenshots, and Group G landing belong to the root agent. No commit made here.
Comparison base: checkpoint `1fd8d43` (reviewed M4).

## Implemented seams

- `knowledge/mint.rs`: five-row whitelist; `mint_knell` installs one Blood proposition per
  day/age, with no person and a day-only mask. `mint_stranger_deed` accepts only the actual
  player's `draw_mark`/`scrub_mark` WorldEvents and takes all prose from fixed whitelist
  templates. Event free text and supplied recipient lists cannot become evidence.
- `engine.rs::ring_knell` and `ring_civic_peal`: typed engine commands mint/amplify at the
  tower. Host `soundscape::bell_bridge_command` uses the accepted BellPlan's clamped stroke
  count, tower and clip descriptor. Cue cooldown and curfew scheduling each send once when
  their peal is accepted; the sim does not receive one command per stroke. `drive.rs` no
  longer describes the civic cue as unable to reach the engine.
- `knowledge/pollen.rs::amplify`: existing matching air only, horizontal cell-centre circle
  plus the peal's own ward. Raw heat becomes `max(old, REHEAT_TO)`; only a whole-percent
  change invokes the existing M2 `stir_up`. It neither mints nor creates absent air rows.
- `round.rs::try_purchase`, `service_stalls`, `nearest_open_stall`, and `tick`: warm Coin
  knowledge about the buyer refuses a purchase. Service produces one inbox percept and
  `refused_on_word` trace without a priority nudge. A game-day half-day deadline pauses
  shopping; experienced buyers then avoid currently refusing sellers, and normal home
  meals remain available if every board refuses. Cooling, invalidation and ablation all
  reopen eligibility. Buyer caches are pruned against current presence.
- `knowledge::door_is_shut` and the independent Stage retain in `Engine::poll`: both
  bodies must be within 10 m of the resident's own door; seeded witnesses count too.
  Nonempty inboxes bypass the gate, including with novelty enabled. All mode stays
  ungated. The diagnostic is limited to one resident/player pair per game hour and pruned
  against presence.
- `notices::{Rung, WardNotice, Notices}` and `knowledge::raise_hearsay_words`: Hearsay is
  the first/lowest rung. One daily reading selects a warm Law telling naming the wrong
  present person, preferring fewer removes; absent officers/subjects and unrenderable
  self-garbles cannot win selection. It calls `raise_hearsay` directly and emits no domain
  event or new fact. `(fact, wrong subject)` dedupe survives settlement and is removed
  when its fact dies. Existing summon/settle/warrant paths work. Hearsay never passes the
  fresh own-witness seizure gate. `LawStanding.clears_when` and the custody HUD cover it.
- `prompt/mod.rs`: the already-computed `you_see.people` supplies nearby subjects to
  `relevance_seated_with_present`. Sheet rendering and the keys retained by
  `render_prompt_and_drain` share one selection/neighbor scan. Both canonical and effective
  self-subject guards align with the preexisting final render guards.
- `pollen::sweep_with_invalidations`: source invalidation runs on the existing stir beat,
  before cooling; `Engine::poll` forwards only the invalidated fact id in a diagnostic and
  runs this before the hearsay reading. Public `sweep` retains its original bool signature.
  `FactSource::still_true(ItemWith)` now also requires the actual item to exist.
- `knowledge::record_player_receipt_inner`: distinct mouth ids, bounded by the existing
  64-receipt cap, determine `tellings`; A→B→A is two mouths even across stirs. An
  unattributed/witness arrival counts once. A better/closer receipt still replaces its
  retained word, mouth, time and hearing place. `record_player_deed` provides the player's
  own witnessed “You …” journal entry without enabling self-subject pickup or sheet news.
- `Engine::publish_ward_heat`: all eight wards, maximum quantized heat and row count,
  stable authored-door centroids, zero rows when ablated, exact projection dedupe. Host
  `HotChannels` passes a `ResMut` wrapper; the actual WardHeat arm writes only differences.
  Fullscreen `map::spawn_map_image` adds eight translucent amber dots, with radius/alpha
  tied to heat. The minimap adds none; closed/unchanged maps perform no layout writes.
- `Knowledge::footprint_bytes`: accounts sealed source heap, quiet households, craft ear,
  allocated Holding vector capacity, owned word capacities, receipt mouth sets and hearsay
  dedupe. BTree node overhead remains a documented estimate. Separate
  `Engine::knowledge_auxiliary_bytes` counts Round refusal maps/sets, door throttle and the
  cached WardHeat vector/labels. `cathedral_headless::print_pollen` prints it on a separate
  `[pollen-aux]` line; `PollenCensus.store_bytes` retains its meaning.

## Proved corrections to plan pseudocode and assumptions

1. A buyer-only deadline cannot itself select another seller after expiry: the same
   nearest board would immediately refuse again. The bounded experience set enables the
   explicitly required alternative seller, while retaining the first visible refusal.
2. `try_purchase` passes no explicit day. `holds_about` falls back to the actual world's
   clock; otherwise a carried word would appear warm forever at its learning heat.
3. A `holdings_len > 0` door shortcut excludes seeded witnesses because they are synthesized
   at hops zero rather than stored as carried rows. Removed the shortcut.
4. T6's “6 m away is outside” conflicts with its own D59 10 m threshold. The tests use
   exactly 10 m as inside and 10.01 m as outside, for each body independently. T9 similarly
   uses inclusive 20 m hearing and 20.01 m outside.
5. T1c's assertion about *all* authored leashes is false in the untouched shipped JSON:
   day_worker 10, market_trader 12, market_trader_green 12, cleric 10, tavern 8,
   night_watch 24, lamplighter 20, wharf 15, stationary 0 metres. The test pins the authored
   day-worker 10 m value and documents the wider cases. Such residents close the door only
   while actually within 10 m; neither rounds nor D59 was widened.
6. M2 `stir_up` increments stir unconditionally, and raises raw heat only when its percent
   changes. Amplify therefore collects subtarget rows, calls it only on a percent change,
   and updates raw same-percent heat directly. M2's generic method is byte unchanged.
7. A hearsay raiser's newly raised notice must not qualify as their own witnessed breach.
   `fresh_own_notice` explicitly excludes hearsay, including after escalation.
8. Invalidation precedes hearsay so a fact removed on that beat cannot produce a new
   wrongful word first. Diagnostic forwarding returns a local vector rather than retaining
   a queue in Knowledge.
9. The done-when request to leave `source.rs` unchanged would preserve a proved bug:
   removing an item while a hand retains its id kept ItemWith true. Its existence guard
   and sealed `heap_bytes() -> usize` accounting are the only new source access. No payload
   exposure was added.
10. M4 already suppressed a garble naming its holder in final `render_line` and
    `render_plain`. Relevance applies the same guard, preventing a suppressed line's key
    from being retained/reheated. It still checks the canonical subject too.
11. M4 counted arrivals rather than separate mouths, so repeated stirs inflated journal
    “others since.” Its expected counts now reflect unique tellers; closer receipt
    semantics remain intact. Fixed allocations are included in the footprint.
12. The old knowledge test character helper placed equal-length ids at the same point,
    despite promising they were far apart. Greeting relevance correctly exposed that
    fixture error. Distinct deterministic test positions restore the fixture's intended
    isolation; proximity tests place actors explicitly.
13. The carried place-label wart is corrected during composition only: “at In …”,
    “at Inside …”, “at Next to …” and “at At …” become one grammatical locative phrase.
    The same rule applies to journal attribution. No area label, measured hedge,
    ignorance sentence, prompt string or template asset was changed. Standing/receipt
    count prose now uses singular mouth/ward/other/has when appropriate.

## Tests and validation

New test modules:

- `crates/cathedral-sim/tests/knowledge/m5.rs`: T1/T1c/T2/T2b/T3/T6/T7/T8/T9/T10/T21;
  real Knell/CivicPeal commands; nonzero cell coverage with centroid outside; same-percent
  raw heat and warmer rows; actual Stage scheduler/inbox/All behavior; self-garble selection;
  distinct mouth saturation; quantized eight-ward channel; locative area labels.
- `crates/cathedral-sim/src/round/knowledge_tests.rs`: T4/T5 through real tick/service/
  purchase/decision callers, no nudge, repeated queue service and ordinary decision ticks,
  paused game instant, actual deadline prune, actual purchase from the alternative,
  held-food consumption then home meal fallback, ablation/invalidation and cache pruning.
- `crates/cathedral-backends/tests/knowledge_consequences.rs`: T12 shipped population and
  occupation-id assertions plus actual cast salience/household/player precedence.

Extended existing tests:

- `engine_tests`: real `PlayerDrawMark` followed by `PlayerScrubMark`, fixed Stranger
  templates, source classification and own witness journal receipts.
- M4 `raise_word_yields_claimed_and_nothing_else`: both new deed hooks reject event free
  text and produce non-claimed fixed propositions. Existing receipt expectations reflect
  distinct mouths and grammatical place composition.
- `fact_source_reaches_no_projection`: sentinel-bound facts through map rows, census,
  traces, journal, source Debug, snapshots, and all four new diagnostic paths; LawStanding
  prose uses the same no-mechanism-word guard. Refusal percepts have the paired guard.
- `the_store_footprint_is_bounded`: adds a final bound at 256 live facts, saturates all
  64 receipt mouth sets through real learning, and includes an all-20,519-resident bound
  for both refusal caches and one player/resident door pair per resident.
- `world_data::full_roster_prompts_and_public_snapshot_remain_bounded`: saturates the
  longest real sheet with six facts, the longest shipped sentence and unknown trade role,
  third-hand low-band prose and written-out age; checks three selected bullets, 64 KiB
  prompt ceiling and exact original snapshot bytes.
- Host T19 and real cue cooldown routing; T20 ordinary-poll resource flags; fullscreen
  dot count/position/visibility/no quiet Node writes; singular locative journal prose.

Validation completed before final root gate:

- `cargo check --workspace --all-targets`: passed; only existing `perf::Probe` warning.
- Backend lib: **148 passed**, including saturated prompt/snapshot canary; new backend
  integration module: **2 passed**.
- Host binary unit suite with headless/fake environment: **498 passed, 0 failed, 3 ignored**,
  5.11 s. No app window, focus change, GPU run or audio was requested.
- Engine integration suite: **64 passed**.
- Final focused sim results and saturated byte totals are appended below after completion.

Frozen files were checked against M4: prompt fixtures, every `assets/prompts` file and
`pollen_cadence.rs` have zero diff. No probability, salience band, affinity, cadence
assertion or measured prose changed. The only salience asset edit is `_no_trade_why` prose
explaining the already-derived curiosity. Per-file rustfmt used explicit paths and
`skip_children=true`; `cargo fmt` was never run. All builds used `CARGO_BUILD_JOBS=1`.

Root retains final tuning and its measurements, full workspace/lint delta gate, visual
evidence and landing. This document is an implementation self-review, not their approval.

## Final implementation freeze

- Final real Round regressions: **3 passed, 0 failed**, 0.03 s
  (`/tmp/knowledge-m5-round-final.log`).
- Final knowledge integration suite: **103 passed, 0 failed**, 0.22 s
  (`/tmp/knowledge-m5-final.log`). The engine suite above remains **64/64**.
- The memory test was then rerun alone after dropping its earlier clone timing probes.
  Keeping those probes alive caused Arc COW to clone six-row vectors at capacity six,
  understating the original allocated capacity eight. The corrected test preserves that
  capacity and passed (`/tmp/knowledge-m5-memory-final.log`):

  | Accounting stage | Bytes |
  | --- | ---: |
  | 20,519 bodies × 6 carried holdings, initial six facts | 19,742,942 |
  | 256 facts plus all 64 receipt sets at 64 distinct mouths | 20,123,494 |
  | All-resident refusal and door auxiliary upper bound, with actual cached projection | 5,746,199 |
  | Combined saturated bound | **25,869,693** |

  Combined is below 32 MiB. At 256 facts, the measured store clone took 0.1825 ms and
  the full 20,521-character world clone took 11.180 ms in this fixture; root owns the
  representative timing/RSS runs and their interpretation.
- Fresh host evidence: `/tmp/knowledge-m5-host.log`; backend/canary evidence:
  `/tmp/knowledge-m5-backend.log`; engine evidence: `/tmp/knowledge-m5-engine.log`;
  workspace check evidence: `/tmp/knowledge-m5-check.log`.
- `git diff --check` passed. A final comparison against checkpoint M4 again found no
  change to cadence tests, prompt fixtures or prompt assets.

Production code and implementation tests are frozen for root review/gate. Cargo is
released; no tool session is doing a build or application run. No agent-owned change
touches the root's documentation/evidence files outside this self-review.

## Silent-plugin correction after root runtime review

Root's unmapped UI session exposed a real registration bug: `SoundscapePlugin::build`
returned when `AudioPlugin` was absent, before installing cue ingestion and the curfew
clock edge. The previous mapping/cooldown unit tests installed those handlers directly,
so they did not cover the plugin's silent branch. A drive cue could therefore be logged
without producing any bell bridge command in the actual headless host.

`src/soundscape.rs` now registers the existing cue ingestion, well activity projection
and curfew scheduling independently of playback. The audio branch keeps its original
ordering through explicit dependencies: cart cues → ingestion → activity projection →
clock sounds → curfew → footsteps → the remaining playback systems. No second bell cue
reader or separate bell policy was introduced. With no audio plugin, a final
`discard_muted_sounds` clears every scheduled playback request, including future strokes,
after cue/curfew policy completes. Cooldown occupancy remains intact.

The new `the_silent_soundscape_plugin_forwards_bells_and_discards_playback` regression
installs the **real** `SoundscapePlugin` with `MinimalPlugins`, controlled time, a bridge
receiver and a projected world clock. It checks one accepted 17-stroke Knell from duplicate
cues, one Curfew on entering Snuffing, no startup or repeated-office peal, and no repeated
Knell or Summons while the ropes are occupied. Sixty-four frames of repeated cues plus
ordinary baking/delayed gate sounds leave the scheduled queue empty with stable capacity.
The app has no AudioPlugin, audio assets, soundscape playback assets or AudioPlayer entity.

This follow-up changes only `src/soundscape.rs` and this self-review. Root retains the
runtime rerun, gate and final tuning.

Validation: affected soundscape suite **35 passed, 0 failed** in 0.08 s
(`/tmp/knowledge-m5-silent-bells.log`); fresh whole host suite **499 passed, 0 failed,
3 ignored** in 4.07 s (`/tmp/knowledge-m5-host-silent-final.log`). Both ran with
`CARGO_BUILD_JOBS=1 CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1`. Only the existing
`perf::Probe` warning appeared. Explicit-file rustfmt and `git diff --check` passed.
The correction is frozen and cargo released for root's gate and runtime rerun.
