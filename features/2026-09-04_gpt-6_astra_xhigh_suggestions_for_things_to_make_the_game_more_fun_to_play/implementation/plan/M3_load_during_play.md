Status: Implemented (2026-10-07). F9, drive `load <slot>` and the Esc menu's slot list load in the running game. Each adopted engine gets a fresh cognition channel; the carried-across STT/TTS services tag their work with the engine's generation, so late voices and transcripts from the replaced world are dropped. Commands queued for the replaced world are discarded at the swap. The five R2 follow-ups are done. Host tests (save, keep playing, load; a stale voice) and a headless drive run (save, play on at 60×, refused bad slot, load, menu load) passed under the capped worker. Load cost: read+decode ~0.5–0.7 s on the save thread; on the main thread verify ~100 ms, old-world drop 7–90 ms, restore ~1 ms (Q6: reported, not optimised).

# M3 — Load during play

From: the rest of the old M3 ([archive/M3_save_load_application.md](archive/M3_save_load_application.md), an edge-case
source only). R3 in [PLAN_REVIEW](PLAN_REVIEW.md). Entry: M2.

M2 can save the running city and start a new process from a slot. This milestone loads a slot **inside the running
game** (D08, Q5): the world swaps, nothing from the replaced world leaks into the new one, and the player sees a
coherent city at the saved time.

## Scope

- **Triggers.** A quickload key (F9; F6 quicksaves) loads the `quick` slot. A drive action `load <slot>` loads any
  slot. The Esc menu lists the slots (label = saved date and office) with a load button each, and says that time does
  not pass while the game is closed (D06).
- **Reuse M2's restore path.** Read and decode the slot off the main thread (D05), then `verify` and
  `Engine::restore` at a pump boundary, exactly as boot-from-slot does. The new engine continues from the saved
  simulation time through `LocalEngine.time_offset`. A slot that fails to read or verify leaves the current world
  running and says why.
- **Old callbacks are inert.** Each adopted engine gets a fresh cognition backend channel (and, in fake mode, a fresh
  fake cognition), so a reply to a request of the replaced engine has nowhere to land. Request ids restart freely.
- **Speech services carry across.** The STT/TTS services move from the old engine into the new one, so local speech
  workers do not restart. Their late results for the old world's requests are ignored, not applied.
- **Host reset.** Clear and rebuild from the new world: HUD (toasts, transcript, notices, pending replies), puppets
  and their interpolation (nobody sweeps through a wall to their new spot), player position/facing/pitch/flight (M2's
  `Restored` event), speech presentation and queued audio, soundscape edge detectors (no duplicate bell, well or
  curfew cue on the first restored frame) and an open microphone recording (stopped, with an honest message).

## R2 follow-ups

1. **Saved idle-turn hashes.** `Novelty`'s saved context hashes use `DefaultHasher` (`attention.rs`), which may change
   with the toolchain. Switch them to a fixed hash such as the FNV in `checkpoint.rs`.
2. **Turn order.** The scheduler's turn `order` (from `idle_cognition.mode`) comes from the save; take it from
   `config.ron`, like the pacing already is.
3. **Restore errors.** A restore error after `verify` passed calls `LocalEngine::fail` and kills the cast. At boot it
   starts a fresh world instead; during play it keeps the current world.
4. **Local speech workers** are carried across (above), so only cognition is fenced.
5. **Test isolation.** Host tests honour `CATHEDRAL_LOAD`. They must not: read it only in the real game's startup.

Q6 (D08): saving keeps the 100 ms hitch clamp. Measure and report the main-thread cost of capture and of adoption.

## Not in scope

The ≤2 ms/frame budget, retention cohorts, the slot publication protocol, staged adoption with whole-bundle
rollback, bounded retirement generations, named-slot management beyond the list, drafts and reading positions
(they arrive with the panels in M4/M6).

## Acceptance (player-visible)

1. F6, walk away and let the clock run, F9: the player is back where they saved, the clock reads the saved time and
   the people are where they were.
2. A drive run saves, keeps playing, runs `load <slot>`, and its screenshot shows the restored city with no stale
   HUD lines or doubled puppets.
3. A reply or voice still in flight from before the load never speaks or acts in the loaded world.
4. With local speech configured, a load does not restart the speech worker processes.
5. Loading across a bell or curfew boundary rings nothing twice.
6. Loading a missing or corrupt slot shows a reason and the current city keeps running.
7. The Esc menu lists the slots and states that time does not pass while the game is closed.

Ships with: a host test that saves, keeps playing, loads in-process and checks the world, HUD and puppets were
replaced, and that a late completion from the old engine is ignored. The lockstep tests in
`crates/cathedral-backends/tests/checkpoints.rs` cover the sim side; extend them only if the restore path changes.
