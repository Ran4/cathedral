# M2a15 actual host ownership and evidence

The exhaustive owner inventory and explicit transient/rebuild policies are in
[OWNER_DESIGN.md](OWNER_DESIGN.md). This table connects the implemented boundary
to meaningful consumers. All restore helpers below are private tests: they write
only covered fields into deliberately scrambled prepared owners. There is no
production partial World/Engine install, checkpoint-triggered poll or extra
input drain.

| Authority | Encoded state/binding | Consumer evidence |
|---|---|---|
| Physical controller | flying, velocity, yaw/pitch, grounded, coyote/jump buffer, previous/current position; same single ECS owner; exact sim pose/sequence | `host_owner_scrambled_physics_restores_exact_next_fixed_step` compares exact bits immediately after prepared restoration and after the actual fixed solver. Split controller/body owner is refused. Finite extreme velocity and concrete setter bounds are corrupted independently. |
| Accepted time | elapsed, ordinary wall debt/wall, accepted virtual elapsed/delta, last accepted frame, fixed elapsed/delta/timestep/residual and sim movement residual | `host_owner_debt_and_fixed_residual_survive_scrambled_prepared_clock` creates an ordinary 503 ms stall, captures nonzero debt/residual, compares all accepted/published/generic clocks and physical bits before a new step, then compares the real First/RunFixedMainLoop consumers. |
| Command/consumer boundary | completed generation/H/physical sample, independent HOST_PRODUCER issued allocator, message sequence, actual speech/cue/intent read cursors | Root actual-boundary tests preserve four post-H worker PCM arrivals, committed unread speech and real PreUpdate chat. `host_owner_keeps_issued_authority_after_sim_refuses_an_explicit_gap` preserves successful enqueue authority despite sim receipt refusal. Root empty-presentation corruption rejects last speech beyond message sequence. |
| Unsubmitted UI actions | closed typed variants with original request, pose, target/item/quantity and order; interrupted recording metadata has no raw audio | Real chat enters pending state before capture and commits on the next ordinary update. Ordinary forwarding assigns a fresh spatial sequence to the still-unsubmitted action's frozen position; saved/read-only operations neither forward nor reissue it. Existing generation tests reject stale UI before command identity allocation. |
| Installed physical definitions | ordered boxes, convex planes/footprints, CutMargin rectangles/feathers/ramps/stairs; actual gate kind/half-size/translation; source algorithms | Actual renderer-free CityPlugin capture covers both physical collider families. Prepared feather mutation changes identity; swapped gate kind changes identity; wrong active and unowned DynamicBarrier refuse observation. Root corrupts prepared definition identity at decode/candidate boundary. |
| Gates | previous schedule sample, initialized/target and each motion's value/start/target/elapsed/duration | `host_gate_fields_restore_mid_motion_and_the_next_schedule_edge` uses actual motion/schedule consumers after closed wire round trip; pure `blocks` exposes threshold without animation or cue generation. Mandatory installed presence rejects null removal/spurious continuation. |
| Custody | sampled complete standing/holders/notices plus strain and failed-send latch | Bound to actual Engine last-law publication. `host_restored_strain_keeps_failed_attempt_latch_and_break_threshold` fills the command queue, preserves failed struggle latch, compares next strain progression and exact escape command threshold. |
| Vermin | installed nav/seed/density/swarm flag/ordered colonies and rat legs; announced boil night and last percept minute | Actual city has eight colonies/150 rats. `host_vermin_gates_preserve_refused_send_and_repeat_boundary` checks failed-send stamp, inclusive repeat deadline and empty colonies. Null/config override corruption fails against saved prepared definitions. Rat animation/scatter is cosmetic. |
| Sampled clock/soundscape | exact published calendar/brightness/scale; cooldown map/prune anchor, observed office/curfew day/rope, flour day, well mechanism times, work position/active and unread CivicBell | Bound clock equals actual Engine publication. `host_restored_curfew_stamp_and_rope_do_not_repeat_a_refused_semantic_bell` preserves a claimed day/rope through failed enqueue and suppresses overlapping semantic bell after prepared restore. Old scheduled audio is cleared explicitly; capture itself does not clear it. |
| Interaction/editor/overlays | selected item/index, coin count, pending request correlation/revision/result, next request, dismissed broadcasts, active offer; map/inventory/chat/journal open state; exact draft character cursor and journal scroll | Root opens actual chat/journal with ordinary keys and checks Unicode combining/CJK/trailing-space draft and nonzero scroll without auto-submission. `host_restored_menu_keeps_its_original_spit_target_after_focus_changes` proves actual inventory action uses the stored historical target after prepared focus changes. |
| Chalk | exact selected choice, half-finished intent/progress and sampled pen/ordered anchors/kinds | Standing binds actual saved marks publication; kinds are strict and ordered, with no gaps/duplicates. Production load policy interrupts a held C gesture until new input; this component records it without completing the gesture. Full load gesture publication remains M3. |
| Journal/HUD projections | resolved ordered journal and standing, sampled custody/notices/chalk, exact readable HUD text/timers | Source formatter is shared without changing ordinary text. Expected publication comparison is bounded; historical IDs may outlive current actors/items. No omniscient transcript reconstruction. Root checks unknown/missing nested fields and actual saved-candidate context coherence. |
| Owed readable speech | original event seq/ID/speaker/label/target/words/position/recipient count/expect-audio; unread messages at actual reader cursor; separate subtitle formatted text/minimum/start/audio flag, bubble text/anchor/expiry and player receipt | `host_restored_readable_receipts_keep_partial_and_not_yet_started_minima` compares exact originals before/after prepared restore and advances actual presentation at 4, 9.999, 10, 12.999 and 13 seconds. Queued line gets its full minimum only on becoming front. Root captures unread speech and intentional chat together. |
| Receipt lifetime | shared original allocation across subtitle/bubble/caption; no raw TTS bytes/handles | `host_receipt_lease_follows_last_bubble_owner_and_precedes_clone` exhausts bounded storage and checks final-owner release/oversized refusal. `host_player_receipt_does_not_attach_to_provisional_replacement_or_expiry` covers committed A -> provisional B and ordinary expiry/disconnect. Original positions compare bitwise. |

Root's independent `tests_public.rs` also covers all observed nested-object
unknown/missing fields, strict unit/map spellings, duplicate keys, raw padding,
shared lease exhaustion, changed source bytes/row stride between measurement and
export, wrong physical/H/issued/spatial bindings, and coherent saved component
contexts without constructing Engine or World. Owner pure wire tests cover
explicit null, Duration and mixed `Never` closure and the largest row stride.

## Dispositions that are intentionally separate

Device-held keys, mouse deltas, microphone capture, audio sinks/streams, old
backend jobs, thread/channel capabilities, debug scripts and wall/process clocks
are released or rebound. Config/provider/device preferences are user-owned.
Body reflexes read PresentSpeech independently and are cosmetic; that cursor is
not the speech reader's acknowledgement. Other sound cues and scheduled audio
are cosmetic interruption, while unread CivicBell remains semantic work.

WorldMirror/render body/camera/layout/assets and compatible static definitions
rebuild. Pending historical item/actor references are retained as historical
choices, not silently retargeted to a current nearby actor. Generation and old
message cursors are saved boundary fences; future M3 rebinds fresh runtime
capabilities and republishes only from the admitted complete state.

Complete root/category receipt linkage, M2b hydration, exact M2c execution retry
and SpeechRouter interruption adoption, dedicated M3 publication and actual
Running/Save/Load/retiring lifetimes remain pending. The private prepared tests
are narrow continuation evidence, not a claim of full save/load adoption.

The added receipt-limit regressions run actual `receive_speech_events` with a
full receipt allowance, preserving subtitle, bubble, player caption and audio
expectation without an early SpeechPresented command. The actual host fixture
then refuses capture separately for missing player and NPC originals and regains
eligibility after ordinary readable expiry. The unavailable player flag clears
on provisional replacement, expiry, disconnect and generation reset.

## Persisted component compatibility — 2026-09-14

`checkpoint_host/initial-v1.json` and `active-v1.json` under the sim test fixtures
preserve exact actual CityPlugin host bytes. Three independent fresh writers
produce identical payloads. The ordinary owner loader test executes at the real
capture barrier with compatible prepared context, including active committed
unread speech and unforwarded intentional chat. It compares canonical decoded
and candidate bytes, and all fresh scalar/row bytes after changing only a
detached copy of the replacement generation fence. An extra initial app and the
full workspace's earlier tests establish that loading does not depend on the
process-global generation allocator starting at one. No live owner is restored
or changed for this fixture comparison.
