Status: Implemented (2026-10-04). Serde is derived on the real types; restore never seeds anything. Lockstep, negative and round-trip tests are in `crates/cathedral-backends/tests/checkpoints.rs`. F6 quicksaves, `CATHEDRAL_LOAD=<slot>` boots from a slot, and drive `save <slot>` saves. Capture takes ~5–70 ms on the main thread. Encoding takes ~2–4 s on the writer thread under the 40% cap. Files are ~2 MB (1,000 extra residents). Revised as R2, see [PLAN_REVIEW](PLAN_REVIEW.md); load during play is R3.

# M2 — Simulation checkpoints and boot-from-save

Restore the authoritative city, not a picture of it (D06). This milestone builds:
- the pure checkpoint/restore in `cathedral-sim`;
- lockstep tests proving that a restored engine continues like an uninterrupted one;
- a thin real-game slice: a quicksave key and starting the game from a saved slot.

**Load during play** (swapping worlds inside a running game) is the next milestone (R3, the rest of the old M3).
Whether it must be in-process at all is an open question (PLAN_REVIEW Q5).

Entry: M0 closed. M1 is not a dependency.

The checkpoint analysis from the first attempt was right; its implementation was ~10× too big. LESSONS lists what to
port and what not to repeat. `PERSISTENCE_INVENTORY.md` is a useful field checklist, not a gate. The derives, the
exhaustive `Engine { … }` destructure/literal and the lockstep tests are the enforcement.

## 1. Checkpoint and restore (pure, in `cathedral-sim`)

- **Derives on the real runtime types.** Add `Serialize`/`Deserialize` derives to the real types; none derive serde
  today. They include:
  - `World` and its characters/state;
  - inventory jobs and `Round` with its private structs and residents;
  - `Custody`, `Notices`, `Marks`, the animals;
  - `Knowledge` with `Fact`, `Held` and `LearnedHow`;
  - the weather timeline, `WorldClock`;
  - `NpcScheduler` with `InFlight`, `TurnLane`, completions/errors and request ids;
  - `NightOffice`;
  - `ConversationFloor` and its warm-exchange/novelty state;
  - `PlaceRegistry` entries.

  No DTO, mirror, view or builder types. New fields get `#[serde(default)]` only where an older save of the *same
  version* could lack them; otherwise let the version gate it.
- **Serde helpers.** Keep them in one small module:
  - `never`: the `f64::NEG_INFINITY` "never happened" sentinels ↔ `null`;
  - `pairs`: maps with non-string keys (tuple keys) as a sequence of pairs, because `serde_json` rejects them as
    object keys.

  Grep for both; the review found about six saved sentinels and five tuple-key maps. For `Vec3`, either enable
  glam's `serde` feature on the workspace dependency or use the existing `math::vec3_serde`. Glam's feature is on
  today only through bevy's feature unification, so `-p cathedral-sim` builds would miss it. Either way, check that
  every restored float is finite.
- **`FactSource`** stays sealed. `knowledge/source.rs` forbids a `Serialize` impl, so serialize `Fact.source` through
  a private save-only `#[serde(with = …)]` module inside `source.rs`. Never render it anywhere.
- **Skipped and rebound, not saved.** Immutable assets behind `Arc` and service handles are `#[serde(skip)]` and
  rebound on restore:
  - the area map, catalogs (items, sounds, marks, facts), salience, adjacency, shelters and nav;
  - the services (`Box<dyn Cognition>` and similar);
  - scratch buffers;
  - `PlaceRegistry`'s derived indexes, which are rebuilt from the saved entries.
- **`Engine::checkpoint(&self, now) -> Checkpoint`** starts with an exhaustive `let Engine { … } = self;` (no `..`),
  so a new engine field breaks the build until somebody decides whether it is saved. It is read-only: two captures
  with no poll between them encode to identical bytes. Not saved, and reset on restore: presentation and publication
  caches (last snapshot revision, the last-sent hot-channel state, journal caches), the transcript buffer, the speech
  router, startup diagnostics and the services.
- **`Checkpoint`**:
  - `version: u32`;
  - a geometry fingerprint: a cheap hash of the nav/areas/shelters/places data the restored world must match. A
    mismatch refuses the load;
  - the simulation `now`;
  - a small label (game day, office, area name) for slot lists;
  - the engine state.

  Never hash the executable, the source tree or the compiler, and add no build script.
- **`Engine::restore(checkpoint, config, assets, services, …) -> Result<Engine, RestoreError>`** ends in an exhaustive
  `Self { … }` literal, like `Engine::new`. It:
  1. checks the version and geometry fingerprint;
  2. rebinds the assets and services and re-applies the current config's ablation switches;
  3. validates before anything else sees the engine:
     - the player exists;
     - all positions are finite;
     - item and actor references resolve;
     - the world's invariants hold, via a `check_invariants() -> Result` that the existing `assert_invariants` then
       calls, so there is no twin;
  4. **never** runs seeding: no `build_world`, `Round::seed`, fact/case seeding, `NightOffice` seeding or animal
     seeding (D02: the history is installed when a world is created, never on load);
  5. performs the start-up steps `Engine::new` does that aren't seeding (starting the scheduler, warming TTS);
  6. resets `world.spatial_sequence` so the host's next position update is accepted;
  7. clears the conversation floor's "awaiting a reply" state and the speech router.

  The first poll after a restore re-emits `Ready` with a full snapshot and every hot channel, as at startup.
- **In-flight LLM work** (the rules from LESSONS):
  - a completion already received but not yet applied is saved and applied exactly once after restore;
  - a request that was submitted but has no result is re-requested once from its saved prompt;
  - nothing is both restored to its inbox and applied;
  - with no cognition available, the turn is unwound the way a provider failure unwinds it, but without the
    system line or backoff;
  - Night Office: an in-flight reflection goes back to the front of its queue, and the per-day guards stay as
    saved, so a load never grants a second reflection.
- **Time.** The sim keeps its own `now` (host seconds since start). The host adds one `time_offset` so the restored
  engine sees a continuous clock: `offset = checkpoint.now - host_elapsed_at_adoption`. No sim field is rebased.
  Loading resumes the saved simulation time; time with the game closed does not pass (D06).

## 2. Save file, quicksave and boot-from-slot (host)

- **`crates/cathedral-backends/src/save.rs`.**
  - One file per slot under `saves/` (gitignored). The first line is a small JSON header (version, label, wall-clock
    save time), followed by gzip'd JSON (`flate2` is already in `Cargo.lock`).
  - Writes run on one worker thread: encode, decode-check, write `slot.tmp` with fsync, keep the previous file as
    `slot.bak`, then rename. There is a `list_slots` that reads headers only.
- **Capture** happens in the host right after `engine.poll` in `LocalEngine::pump`: clone the checkpoint on the main
  thread and hand it to the worker. No extra poll and no waiting on providers. Disk work never happens in the pump
  (D05). Log capture time, encode time and size once per save. They are measured, not gated.
- **A quicksave key.** Check the existing bindings for a free one; F5 is the screenshot key. It shows a short HUD
  toast with the result.
- **`CATHEDRAL_LOAD=<slot>`.** It reads and decodes the slot at engine build time, outside the pump. `LocalEngine`
  then calls `Engine::restore` instead of `Engine::new`, and places the player controller at the saved player
  position and yaw before the first spatial update. Host-only player state (pitch, flying) travels in a small
  `HostSave` beside the checkpoint. An unreadable or incompatible slot logs an error and starts a fresh world.
- **Drive action `save <slot>`.** Then a two-process headless run can prove it:

  ```sh
  CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE='wait-online; tp …; sleep 5; save smoke; sleep 2; quit' cargo run
  CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_LOAD=smoke CATHEDRAL_DRIVE='wait-online; sleep 3; shot loaded; quit' cargo run
  ```

## 3. Lockstep tests (`crates/cathedral-backends/tests/`)

Run engine A uninterrupted. Run B to a capture point, encode it to bytes, drop B and restore C from the bytes with
fresh services. Feed A and C identical steps and compare:
- the domain `EngineMessage`s per poll, ignoring C's first-poll republish;
- every N polls, the re-encoded checkpoints, with request ids masked.

Use the real city data and the deterministic fake cognition, with the Night Office, weather and knowledge on. Where
work was in flight at capture, assert conservation instead of identical timing: exactly one re-request, applied
exactly once.

Scenarios (keep each to a few hundred polls; the suite runs under a 40%-of-one-core cap):
- an idle city at 60× across an office;
- an NPC turn in flight at capture;
- a received-but-unapplied result at capture;
- a player-speech reaction in flight;
- a pending NPC offer accepted after load;
- a custody seizure mid-escort, then the commit;
- a night reflection across bedtime;
- a seeded ward-air fact spreading;
- a weather change during owed bell strokes;
- two captures with no poll between them give identical bytes.

Negative tests: wrong version, wrong geometry fingerprint and a NaN position are all refused, and the caller keeps
its current engine.

One `#[ignore]`d run at +2,000 residents prints capture/encode/restore milliseconds and bytes.

**Standing rule for every later milestone:** add at least one lockstep scenario for the new state you introduce
(D06).

## Do not build

- DTO, wire-twin, view or builder mirrors of the state;
- byte/count admission, allocation meters, per-frame save/load budgets or memory ceilings;
- slot generation/reference publication protocols, directory durability or crash-at-every-step tests;
- staged adoption with rollback;
- saved velocity, fixed-step residuals, wall-time debt, input watermarks or physical-sample sequences;
- calendar or deadline rebasing;
- durable Night Office duty identities;
- exe/source/compiler hashing or a build script;
- a field-index script;
- a 20,000-resident gate;
- offline catch-up;
- save migrations.

## Done when

- The lockstep and negative tests pass. `cargo test -p cathedral-sim -p cathedral-backends` passes through the capped
  worker.
- The game builds.
- The two-process headless drive run saves in one process and starts the next one from the slot. The city and
  player are where they were, and the shot renders.
