Status: Planned (2026-09-05).

# M3 — Save-anywhere in the application

Make the M2 checkpoint usable during ordinary play. A save is complete only after durable file publication; a load is complete only after both simulation and host state agree.

## Entry

M2 is accepted. Private checkpoint validation and hydration work with fresh services. M1 provides runtime generations and action high-water marks. Follow the concrete capture/adoption/default-pending-work contract in [CHECKPOINT_PROTOCOL](CHECKPOINT_PROTOCOL.md).

## Host responsibilities

Add a save service to `cathedral-backends` and a small host coordinator in `src/smart_actors/local_engine.rs` or a dedicated sibling module. Keep encoding/disk IO off the simulation thread. Do not move the non-`Send` engine to a worker; pass an owned validated capture value to the worker instead.

Start with at most one retained save payload, one candidate load and one retiring generation under a shared byte budget; queue small request metadata instead of more world copies. Admission precedes allocation/staging. Cancelled loads must release inactive entities/resources before another candidate is admitted. Follow [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md) for incremental host work, bounded callback production/draining and measurements beyond the adoption frame.

Persist the player continuation state alongside the engine: authoritative physical position, velocity, view direction, relevant jump/ground state, fixed-step residual, developer flight flag when applicable and the accepted spatial sample baseline. Preserve player custody strain and its reported latch or move them into the sim; they create commands and are not merely a tether projection. Reset render interpolation to the restored transform so the player and NPCs do not sweep through intervening walls.

Own host-originated semantic events too: vermin swarm announcement/repeat cursors cannot reset on load. Explicitly reinitialize soundscape edge detectors and owed presentation so a first restored clock does not create duplicate civic/well/curfew cues.

Rebuild the mirror, movement inbox, entity bindings, doors, item visuals, lamps, marks, weather and custody projections from the restored world. Clear old-world transient speech/audio and callbacks. Reset request counters/generations consistently, not just the visible scene.

## Save flow

1. The player requests a slot save from normal play. UI says “Saving…” while the city continues.
2. Capture a coherent engine/player boundary and its simulation time. Show the captured time, not an implied guarantee that later actions are already saved.
3. Use [CHECKPOINT_PROTOCOL's generation/reference publication](CHECKPOINT_PROTOCOL.md#6-publishing-slots): encode, validate and durably publish an immutable payload, retain the prior acknowledged reference/payload, then atomically publish the new small slot reference with directory durability. Overwriting the sole payload is insufficient for the promised recovery guarantee.
4. Report success only after publication. On failure, retain the previous save and give a useful message. Do not silently mark a failed write successful because the in-memory capture worked.
5. Bound queued requests by count and retained bytes. Preserve manual save intent; coalesce redundant autosaves where safe. Serialize same-slot publication or supersede older requests so a slow older save cannot overwrite a newer one. Save-operation IDs survive unrelated world loads. A slow disk cannot accumulate an unbounded queue of full-world copies.

## Load flow

Read and validate a candidate without modifying the running world. Check schema, integrity, supported content/geometry manifest and all M2 invariants. Resolve assets and prepare host projection work before committing replacement.

At the replacement boundary, atomically adopt the complete engine/controller/fixed-clock/custody/mirror/input/presentation bundle. Bind saved logical time to host time at adoption, not when reading or staging began. Apply deferred ECS/projection propagation before consumers see the new generation; publish through a dedicated initial-publication API without a poll. Only then activate new service submissions. Do not drain commands queued for the old world into the new one. Saved drafts may return as drafts; they are never automatically sent.

Failure before adoption leaves the current world intact. Prefer an infallible final owned-state swap; otherwise retain and test rollback of the entire bundle, including fixed clocks and pending presentation. Never continue with one world's camera and another world's inventory. Retire old services/worlds outside the critical adoption frame through a designed shared-runtime or bounded retirement path: current backend destruction can wait 500 ms, and the Engine is non-`Send`.

The world resumes at the saved simulation time. There is no offline catch-up while the application was closed. Explain that alongside the slot's saved date/office.

## Player interface

Support named/manual slots, a quick-save slot and visible load/save results. Keep in-game save browsing non-pausing under D05. Confirm replacement of a different manual slot and loading over unsaved play through normal explicit UI; do not use an investigation-specific flow.

Retain document reading position, notebook selection and typed drafts where possible. An interrupted microphone recording gets an honest message. An NPC's already committed speech stays readable without reapplying its actions or announcing it to witnesses again.

Saved metadata must be safe for normal browsing: title, time, location known to the player and capture version. Do not expose the hidden culprit, NPC private plans or unseen evidence locations in the slot preview.

## Tests and operational evidence

- Save during movement, jumping, reading, typing, microphone capture, delayed cognition, an NPC escort and a pending offer.
- Restore these in a fresh process; verify matching positions, possessions, clock, obligations and presentation.
- Simulate truncated files, checksum failure, disk-full/write failure, failed rename and an incompatible content manifest. The prior valid save survives.
- Complete an old cognition/STT/TTS request after loading; no stale effect or voice enters the new world.
- Save repeatedly while disk completion is delayed; memory and queue size remain bounded.
- Repeatedly cancel prepared loads and save during delayed retirement; generation counts, inactive entities, callback cohorts and retained bytes remain within admission limits without losing terminal results.
- Measure main-thread capture/adoption time separately from encoding and disk latency. Include old-world destruction, projection work, and later copy-on-write costs while a snapshot remains retained. Set numerical time/memory budgets from M0's hardware baseline and reject visible save hitches that make “never pause” merely nominal.
- Run hidden-window drive scenarios with `CATHEDRAL_HEADLESS=1`; the player should see no unsolicited test window.

## Completion gate

Save-anywhere works for the current whole city before further systems are added. Every later milestone must extend the schema, restore validation and continuation fixtures as part of its own work. There is no deferred “we will add all persistence at the end” escape hatch.
