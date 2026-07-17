# Logic bugs found — 2026-07-17 (Claude 5, xhigh)

A whole-repo hunt for logic bugs: seven parallel reviewers, one per area
(sim core, sim actions/round, sim world/nav, sim prompt/parse, backends,
game smart_actors, game core/city), every candidate finding then verified
by hand against the code before making this list. Seven findings survived
(1 medium, 1 low–medium, 5 low). Areas that came back clean are listed at
the bottom together with the candidates that were chased and refuted —
the refutations are the next reviewer's head start.

## 1. Realtime STT: `clear()` desyncs the commit-ack FIFO, misattributing later transcripts

- **File**: `crates/cathedral-backends/src/stt_realtime.rs:278` (interacting with `handle_provider_event` at `stt_realtime.rs:472-495`)
- **Severity**: medium
- **What's wrong**: `pending_commits` is a positional FIFO whose invariant is
  "entries, in order, == unacked `input_audio_buffer.commit` frames on the
  wire, in send order" — each `input_audio_buffer.committed` ack pops the
  front (`stt_realtime.rs:476`) and binds it to the returned `item_id`.
  `clear()` breaks the invariant: it removes a key from the middle of the
  FIFO even though that utterance's commit frame was already sent (a cleared
  committed utterance takes the `was_active == false` branch, so no `clear`
  frame is sent either — the provider's ack is definitely still coming).
  That ack then pops the *wrong* (next) key, and every later utterance's
  `item_id` binds one key off.
- **Failure scenario**: Player commits utterance `a` (pending=`[a]`), commits
  `b` before `a`'s ack arrives (pending=`[a,b]`; `max_in_flight` default 4
  permits it), then `a` is cleared — reachable from
  `speech_router.rs:644` (switching to local STT mid-stream in `Committed`
  phase) and `speech_router.rs:548` (`on_audio_abort`). Pending=`[b]`.
  The ack for `a` arrives → pops `b` → `item_A` binds to key `b`. `a`'s
  audio is transcribed and delivered as `b`'s text; `b`'s real transcript
  finds no waiting key and is silently discarded (`stt_realtime.rs:507`).
  The player's `say` carries the wrong words. Only bites with ≥2 commits in
  flight and a non-tail one cleared — a narrow race, but a real invariant
  break.
- **Proposed fix**: keep the slot as a tombstone so the ack still consumes
  it but binds nothing (keys are `.wav` basenames, so `""` is never real):

  ```rust
  // clear(): instead of pending_commits.remove(position):
  if let Some(slot) = state.pending_commits.iter_mut().find(|entry| **entry == key) {
      slot.clear(); // "" tombstone: its ack must be consumed and dropped
  }

  // handle_provider_event, "input_audio_buffer.committed":
  let Some(key) = state.pending_commits.pop_front() else { return };
  if key.is_empty() {
      return; // ack for a cleared commit: consume the slot, bind nothing
  }
  ```

  The session-failure path that drains the FIFO into failure results
  (`stt_realtime.rs:544`) should then skip empty keys too.

## 2. Office bells have no catch-up cap — a large `now` jump rings them all at once

- **File**: `crates/cathedral-sim/src/engine.rs:1463` (`ring_offices`), driven by `clock.rs` `offices_crossed`
- **Severity**: low–medium (edge-triggered; never fires in normal 1× play)
- **What's wrong**: `ring_offices` enqueues a stroke for *every* office
  boundary in `(last_clock_now, now]` with no cap, and the drain loop
  (`engine.rs:1499-1502`) emits every stroke with `due <= now` in a single
  poll. The function directly below it, `tick_movement`
  (`engine.rs:1527-1538`), explicitly bounds the very same situation with
  `MAX_MOVEMENT_CATCHUP_SLICES` and a snap-forward, with the comment "a huge
  `now` jump snaps forward instead of spinning" — the bell path just lacks
  the same guard.
- **Failure scenario**: The host stops polling for ~20 min wall-clock
  (window minimized, laptop suspend), then resumes. At the default
  3600 s/day that span is ~8 game-days ≈ 56 office crossings, every stroke
  already past due → one poll pushes dozens of simultaneous town-bell
  `EngineMessage::Sound`s. Instead of a countable hour-bell the player gets
  a wall of bells — defeating the count-the-strokes design the clock
  documents. Also reachable via a multi-second hitch at 60× debug speed.
- **Proposed fix**: bound the catch-up like movement does — on a large span
  ring only the most recent office(s):

  ```rust
  let mut crossed = self.clock.offices_crossed(self.last_clock_now, now);
  const MAX_BELL_OFFICES_PER_POLL: usize = 2;
  if crossed.len() > MAX_BELL_OFFICES_PER_POLL {
      crossed = crossed.split_off(crossed.len() - MAX_BELL_OFFICES_PER_POLL);
  }
  for (instant, office) in crossed {
      for stroke_at in stroke_times(office, instant) {
          self.bell_strokes.push_back(stroke_at);
          queued_any = true;
      }
  }
  ```

## 3. Fixture-clearance test inverts the rotation the wrong way — oblique fixtures are checked in mirror orientation

- **File**: `src/city/plan.rs:410-414`
- **Severity**: low (a guardrail that silently under-tests, not runtime behavior)
- **What's wrong**: The NPC-clearance test maps a world point into a
  fixture's local frame to test it against the half-extents. Fixtures are
  spawned with `Quat::from_rotation_y(+angle)` (`src/city/mod.rs:1959` →
  `spawn_rotated_box_named`, `mod.rs:3730`), so the inverse mapping is
  `R_y(−θ)`: `local_x = dx·cosθ − dz·sinθ`, `local_z = dx·sinθ + dz·cosθ`.
  The test negates the angle *and* uses the transposed formula — the two
  compose into `R_y(+θ)`, i.e. it checks the point against the fixture's
  **mirror** (−θ) footprint. Concretely at θ=90°: a world offset `(0,−1)`
  (the fixture's local +X) maps to `(−1,0)` instead of `(1,0)`. Harmless
  for axis-aligned and 90°-multiple fixtures; wrong for the plan's ~70
  oblique ones (−19.5°, +13.7°, …).
- **Failure scenario**: An NPC authored 1.4 m off the long side of a stall
  rotated +18°: the true footprint overlaps the clearance disc but the
  mirror footprint doesn't → the test passes and the NPC ships embedded in
  the stall — exactly the condition the invariant exists to catch.
- **Proposed fix**: drop the negation so the formula is the true inverse:

  ```rust
  let angle = fixture.angle_deg.to_radians();       // was: (-fixture.angle_deg)
  let dx = point[0] - fixture.position[0];
  let dz = point[1] - fixture.position[1];
  let local_x = dx * angle.cos() - dz * angle.sin();
  let local_z = dx * angle.sin() + dz * angle.cos();
  ```

## 4. `add_tube` end caps drop two wedges — a hole in every tube end

- **File**: `src/city/monuments.rs:249-253`
- **Severity**: low (visual)
- **What's wrong**: The end caps fan from the tube's centreline point
  (`path[0]` / `path[last]`) but iterate `1..sides-1`, which is the idiom
  for fanning from *ring vertex 0* (a polygon triangulated by `sides−2`
  triangles). With a centre apex you need all `sides` wedges with
  wrap-around. As written the wedges `(C, r0, r1)` and `(C, r_last, r0)`
  are never emitted — an uncapped sector of `2/sides` of the disc (≈80° on
  the 9-sided arms, ≈120° on 6-sided tubes). The neighbouring `add_loft`
  caps do it correctly (`monuments.rs:206-214`, `(segment + 1) % segments`).
- **Failure scenario**: The Dawn-Bearer's balancing arm ends free in
  mid-air; its exposed end cap has an ~80° hollow slice you can see into
  up close.
- **Proposed fix**:

  ```rust
  for side in 0..sides {
      let next = (side + 1) % sides;
      self.triangle(path[0], rings[0][side], rings[0][next]);
      let last = rings.len() - 1;
      self.triangle(path[last], rings[last][next], rings[last][side]);
  }
  ```

## 5. `stop_npc_speech_for_capture` leaks `pcm_streams` entries

- **File**: `src/smart_actors/speech.rs:976-1011`
- **Severity**: low (bounded leak + wasted work; no replay possible)
- **What's wrong**: This is the one terminal audio path that clears
  `ready_audio` and drains `audio_order` but never touches
  `state.pcm_streams`. Every sibling terminal path removes and/or
  `finish()`es the stream: the normal end-of-voice (`speech.rs:676`), TTS
  failure (`:507`), invalid chunk (`:562`), stale entry (`:707`), timeout
  (`:724`), inaudible (`:739`), suspension failure (`:762`).
- **Failure scenario**: Player speaks mid-NPC-voice → `StopNpcSpeech` cuts
  the active voice and drains the queue, but the cut events' streams stay
  in the map un-finished. Later `TtsChunk` messages for those events still
  pass the `stream_is_live` gate (`speech.rs:530`) and are pushed into a
  consumer-less buffer, and the orphaned entries linger until the next
  `ClearSpeechPresentation` (engine disconnect). Bounded, but a real leak
  and inconsistent with every sibling handler.
- **Proposed fix**: after `state.ready_audio.clear();` add

  ```rust
  for stream in state.pcm_streams.values() {
      stream.source.finish();
  }
  state.pcm_streams.clear();
  ```

  Cutting NPC voice for capture is a full reset of the audio queues (the
  same thing `SpeechPresentationState::clear` does), so clearing every
  pending stream is correct here.

## 6. `RealtimeSettings` doc says every field is env-overridable — `max_in_flight` is not

- **File**: `crates/cathedral-backends/src/config.rs:456` (vs. the doc comment at `config.rs:344-346`)
- **Severity**: low (doc/behavior mismatch)
- **What's wrong**: The struct doc says "Every field is env-overridable
  because the API shape is documented as volatile (R16)." `url`, `model`,
  `delay`, `language` and `idle_close_seconds` all have env lookups
  (`STT_REALTIME_URL`, `STT_REALTIME_MODEL`, `STT_REALTIME_DELAY`,
  `STT_LANGUAGE`, `STT_STREAM_IDLE_CLOSE_S`) — `max_in_flight` alone is
  hardcoded to `DEFAULT_REALTIME_MAX_IN_FLIGHT`.
- **Failure scenario**: An operator sets an env var to tune the commit
  gate (`stt_realtime.rs:252`); it silently has no effect.
- **Proposed fix**: read it from env like its siblings —

  ```rust
  max_in_flight: environment
      .trimmed("STT_REALTIME_MAX_IN_FLIGHT")
      .and_then(|value| value.parse::<usize>().ok())
      .filter(|count| *count > 0)
      .unwrap_or(DEFAULT_REALTIME_MAX_IN_FLIGHT),
  ```

  — or soften the doc comment to name the exception.

## 7. `faction_role` prose is hardcoded against the prompt module's own doctrine

- **File**: `crates/cathedral-sim/src/prompt/mod.rs:760-761` (vs. the module doc at `mod.rs:4-13`)
- **Severity**: low
- **What's wrong**: The module doc asserts "**all** LLM-visible prose lives
  in `assets/prompts/`", with section labels as the one exception. In
  `you_line`, the introducers for `home` and `illegal_activity` obey it
  (`strings.home_label`, `strings.illegal_activity_label`) — but between
  them, `faction_role` renders a hardcoded `" Faction role: {role}."` (and
  the `Family:`-style connective prose below is likewise inline). An
  incremental-migration leftover: LLM-visible prose that can't be edited
  via the data files.
- **Failure scenario**: Any lore NPC with `faction_role` set gets prose in
  its sheet that `strings.toml` cannot touch; the doc's categorical claim
  is false.
- **Proposed fix**: mirror the existing pattern —

  ```toml
  # assets/prompts/strings.toml
  # Introduces the lore profile's `faction_role` on the `**you**` line.
  faction_role_label = "Faction role:"
  ```

  ```rust
  if let Some(role) = lore.faction_role {
      line.push_str(&format!(" {} {role}.", strings.faction_role_label));
  }
  ```

  — or narrow the module doc to say only some micro-strings are
  externalized.

---

## Notes — flagged, but not asserted as bugs

- **Mic-always-on mode never interrupts NPC voices**
  (`src/smart_actors/speech.rs:988`, `interaction.rs:868-873`): with
  `pause_microphone_during_npc_voice = false`, `poll_microphone` still
  sends `StopNpcSpeech` when the player speaks, but the handler returns
  immediately — the NPC keeps talking into the open recording. Plausibly
  intended (that config opts into feedback), but if the intent is "always
  cut NPC voice when the player speaks", the early-return guard is wrong.
- **Sentinel collision in `history_section`**
  (`crates/cathedral-sim/src/prompt/mod.rs:848-853`): a genuine lone event
  whose text literally equals `"nothing"`/`"nothing yet"` would render as
  the empty-form instead of a bullet. Effectively unreachable with the
  current line formats; inherent to the sentinel design.
- **Dormant offset bug in `cistern_vault`** (`src/city/water.rs:1505`):
  the draw-hatch offset uses `+sin` where the `right` basis uses `−sin`,
  so it would be misplaced for a rotated vault — but every call site
  passes `angle = 0.0`. Worth a comment if the function ever gets a
  nonzero angle.

## Clean areas — and the candidates chased and refuted

**Sim world/nav/areas/places/floor/seed** — no defensible bugs. Refuted:
`floor.rs::acquire_scoped` evicting before the existing-id check at the
32-cap (faithful `OrderedDict` port, commented approximation);
`step_movement` popping ≤1 waypoint per tick (paths are node-spaced;
distance conservation is pinned); 3-D vs planar-XZ distance between
`characters_within` and the stage gate (identical while everyone shares
`WALK_Y`; diverges only in dev-fly); the unused-looking `movement.speed`
input (it's an output for gait, not an input — the mover walks at
`WALK_SPEED_MPS` by design). A* optimality without a closed set holds
because the Euclidean heuristic is consistent.

**Sim actions/round/speech_router/perception** — no defensible bugs.
Refuted: stale `spatial_seq` rejecting a whole recording (pinned as "the
monotonicity guard is not negotiable"); `go_to {person}` gating on hearing
radius rather than the view cone (matches what `you_see` itself shows);
the run_ladder stale-`phase` read after `end_intent` (every reachable
combination still applies the right decision); curfew emitting both the
turn-back and excuse percepts in one tick (two distinct facts, both true);
`resolve_arrivals`' Approaching-but-not-at-curb branch (draw points are
nav nodes, so routes end within the arrive radius — defensive dead
branch); double-enqueue/double-nudge at wells (phase guards prevent it).

**Sim scheduler/attention/clock/engine** (beyond finding #2) — clean:
office-crossing arithmetic, backoff, the three turn lanes, the novelty
state machine, `context_hash`, and the settled-only mirroring of
`you_see` are all correct and heavily pinned.

**Sim prompt/parse/lore + the fresh c0bd554 commit** (beyond finding #7)
— clean: the new `on_your_way`/`the_day` sheet lines match their tests
and the `Patrol`/`IntentTarget` semantics; every `PromptStrings` field
maps to `strings.toml` and every `%s` template is validated at load;
`parse.rs`'s verb regex and JSON-shape guard hold up.

**Backends** (beyond findings #1 and #6) — clean: WAV header
repair/duration math, the incremental PCM16 decoder, downmix, cost/cache
accounting, Retry-After clamping, the worker protocol teardown, env
precedence, the byte-identical prompt archive, and the headless runner.

**Game smart_actors** (beyond finding #5) — clean: movement
interpolation/seq handling, yaw round-trips, event ordering
(`PlayerAudioEnd` before `PlayerRecording`), revision-gated command
resolution, snapshot validation, targeting ray math, VAD/resampler math,
chat cursor handling, config-menu/chat Escape exclusion.

**Game core/city** (beyond findings #3 and #4) — clean: water, scene,
controller (coyote/jump), drive scheduler, session-log symlinks,
screenshots, offset-polygon miters, stair/wall yaw conventions.
