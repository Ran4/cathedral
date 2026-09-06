Status: All scenarios below are planned (2026-09-05); none is certified by generating this file.

# Verification by milestone

These are behavioral acceptance contracts for implementation. They deliberately include adversarial and partial-result cases. Exact future test/CLI names are assigned when the owning milestone implements them; this document does not advertise nonexistent commands.

The [checkpoint protocol](CHECKPOINT_PROTOCOL.md), [spatial proof protocol](SPATIAL_PROOF_PROTOCOL.md) and [law protocol](LAW_PROTOCOL.md) supply the detailed cross-module invariants. Strict continuation equality uses held/recorded completions with controlled timing; intentional requeue of unfinished live requests instead proves conserved obligations and exactly-once effects.

Current baseline/tool results belong in [REVIEW_LOG.md](REVIEW_LOG.md). Automated traces do not certify human enjoyment or a successful visual check.

## [M0](M0_baseline_and_prerequisites.md)

<a id="v01"></a>
**V01 — Knowledge prerequisite** · source + existing feature acceptance · [R01](REQUIREMENTS.md#r01), [R07](REQUIREMENTS.md#r07), [R32](REQUIREMENTS.md#r32)

Reconcile the completed knowledge M0–M5 against current source and its own acceptance record.

Acceptance: The plan uses shipped holders, provenance, player-learning and retention APIs; no guessed duplicate store or stale quest stub remains.

<a id="v02"></a>
**V02 — Authority inventory** · source audit · [R14](REQUIREMENTS.md#r14), [R32](REQUIREMENTS.md#r32), [R35](REQUIREMENTS.md#r35)

Enumerate mutable sim/host owners, capabilities and migration seams on the actual implementation baseline.

Acceptance: Every future-affecting field has a checkpoint policy; every new shared service has one producer and explicit consumers.

<a id="v03"></a>
**V03 — Reproducible baseline** · tests + measurement · [R29](REQUIREMENTS.md#r29), [R32](REQUIREMENTS.md#r32)

Run accepted baseline suites and profile movement-faithful default/2,000-citizen runs, with 20,000 separately labeled stress.

Acceptance: Record commit/tree, hardware, poll pattern, CPU/frame/memory figures and numeric budgets. A coarse clock watcher is not movement evidence.

## [M1](M1_live_time_and_transactions.md)

<a id="v04"></a>
**V04 — Live overlays and conversation** · sim + host · [R13](REQUIREMENTS.md#r13), [R35](REQUIREMENTS.md#r35)

Keep a conversation/floor hold and each existing menu open across movement, need and calendar transitions.

Acceptance: Simulation events and ordinary duties continue; only input capture or a particular person's permitted courtesy changes.

<a id="v05"></a>
**V05 — Clock crossing ownership** · deterministic sim · [R13](REQUIREMENTS.md#r13), [R14](REQUIREMENTS.md#r14), [R16](REQUIREMENTS.md#r16)

Change 1×/60× immediately after bedtime and midnight while Round skips a cadence tick.

Acceptance: One office, production/settlement edge and ambient/night duty is processed; historical cursors are not reinterpreted through the new rate.

<a id="v06"></a>
**V06 — No partial or duplicate action** · deterministic sim · [R12](REQUIREMENTS.md#r12), [R16](REQUIREMENTS.md#r16), [R29](REQUIREMENTS.md#r29)

Repeat commands, conflict their payloads, exhaust the bounded replay window and inject failed route/authority/resource validation.

Acceptance: One effect/receipt at most; rejection leaves no partial custody/inventory mutation, and compacted IDs including never-accepted requests cannot replay. M2 extends the same fixture through restore.

<a id="v07"></a>
**V07 — Minimal operation kernel** · deterministic sim · [R10](REQUIREMENTS.md#r10), [R35](REQUIREMENTS.md#r35)

Two duties claim one actor/resource; interrupt and replan during bounded in-memory work.

Acceptance: One owner, conserved completed steps and operation-wide recovery budget; replanning does not reset exhaustion. M2 adds the checkpoint continuation in V09.

<a id="v08"></a>
**V08 — World and actor generations** · sim + host · [R16](REQUIREMENTS.md#r16)

Deliver a result for an old world and a departed incarnation with reused numeric request/actor IDs.

Acceptance: Both identity fences work independently; no current-world effect or presentation acknowledgement is accepted.

## [M2](M2_simulation_checkpoints.md)

<a id="v09"></a>
**V09 — Populated checkpoint** · deterministic continuation · [R14](REQUIREMENTS.md#r14), [R16](REQUIREMENTS.md#r16), [R35](REQUIREMENTS.md#r35), [R36](REQUIREMENTS.md#r36)

Capture M1 kernel ownership/progress and exhausted replay windows plus ordinary queues, production, offers, road parties, weather, lamps, knowledge, custody and needs mid-operation.

Acceptance: Fresh hydration resumes canonical state/events and replay rejection with controlled subsequent inputs, without new/seed paths or omitted subsystem state.

<a id="v10"></a>
**V10 — Pending cognition categories** · deterministic continuation · [R14](REQUIREMENTS.md#r14), [R16](REQUIREMENTS.md#r16)

Capture an empty-inbox idle submission and a held completion followed by newer player speech.

Acceptance: Unfinished work restores one owed retry; held results apply once without resubmission; old drained history and newer obligations remain distinct.

<a id="v11"></a>
**V11 — Night obligation identity** · deterministic continuation · [R14](REQUIREMENTS.md#r14), [R16](REQUIREMENTS.md#r16)

Capture queued/submitted/completed Night duties around midnight and actor departure/re-entry.

Acceptance: Owed day, incarnation and spent/completed state survive; no doubled reflection or ambient reroll, and valid retries still yield to the player.

<a id="v12"></a>
**V12 — Validation and compatibility** · DTO validation · [R14](REQUIREMENTS.md#r14), [R15](REQUIREMENTS.md#r15)

Load valid Never sentinels, invalid floats, duplicate ownership, overflow, missing adapters and incompatible content/generator versions.

Acceptance: Virgin worlds work; invalid candidates fail with subsystem diagnostics before adoption; no defaults are silently reseeded.

<a id="v13"></a>
**V13 — Capture is observation** · sim + host · [R14](REQUIREMENTS.md#r14), [R16](REQUIREMENTS.md#r16)

Request repeated saves with no elapsed time, including committed receipts not yet consumed by the host.

Acceptance: No extra poll, cognition, event or physical change; all committed readable effects at the watermark belong to the capture.

## [M3](M3_save_load_application.md)

<a id="v14"></a>
**V14 — Matched physical capture** · fresh-process host continuation · [R14](REQUIREMENTS.md#r14), [R15](REQUIREMENTS.md#r15), [R16](REQUIREMENTS.md#r16)

Save mid-jump at nonzero host/sim residual and ordinary pre-capture wall debt while a custody threshold command is queued.

Acceptance: Accepted sim pose equals the authoritative body; residuals/debt remain distinct, no offline/preparation debt is added and resistance emits its threshold effect once.

<a id="v15"></a>
**V15 — Late callback storm** · host/backend integration · [R15](REQUIREMENTS.md#r15), [R16](REQUIREMENTS.md#r16)

After adoption deliver old cognition, Night, STT, TTS, PCM, microphone/backend-status and SpeechPresented callbacks.

Acceptance: No old voice, automatic utterance or domain effect enters the loaded generation, including reused IDs.

<a id="v16"></a>
**V16 — Durable ordered saves** · backend fault injection · [R15](REQUIREMENTS.md#r15), [R29](REQUIREMENTS.md#r29)

Reverse two same-slot write completions; inject IO failures and process death after payload, recovery-reference and active-reference publication/flush steps.

Acceptance: The latest authorized durable publication wins; an acknowledged prior reference/payload remains recoverable, uncertain publication is not reported as success and retention stays bounded.

<a id="v17"></a>
**V17 — Prepared adoption and retirement** · host integration + measurement · [R13](REQUIREMENTS.md#r13), [R15](REQUIREMENTS.md#r15), [R29](REQUIREMENTS.md#r29)

Delay load preparation while the city runs, then inject staging/adoption failure and expensive old-world retirement.

Acceptance: Adoption resumes at the saved instant and swaps one coherent bundle; rollback includes clocks/UI; retirement does not create an unbudgeted frame stall.

<a id="v18"></a>
**V18 — Presentation continuation** · fresh-process host continuation · [R15](REQUIREMENTS.md#r15), [R16](REQUIREMENTS.md#r16), [R33](REQUIREMENTS.md#r33)

Save during speech/recording and on both sides of rat-percept and civic/well cue deadlines; later M10 extends this fixture with explicit dictation purposes and cancelled drafts.

Acceptance: Committed lines/history remain without repeated speech, unsent text stays a draft and late callbacks cannot broadcast it; semantic rat events and presented cues do not duplicate or vanish.

## [M4](M4_layered_navigation.md)

<a id="v19"></a>
**V19 — Layered body parity** · sim + hidden-window drive · [R04](REQUIREMENTS.md#r04), [R08](REQUIREMENTS.md#r08), [R36](REQUIREMENTS.md#r36)

Walk player and NPC through stairs, L-corners, landings, low headroom, closed portals and vertically overlapping rooms.

Acceptance: Both use actual supported routes and consistent collision/topology; no ground flattening, wall crossing or accidental cross-floor interaction.

<a id="v20"></a>
**V20 — Conservative route coverage** · geometry analysis · [R04](REQUIREMENTS.md#r04), [R05](REQUIREMENTS.md#r05)

Add a usable roof/drop/jump or boundary shortcut absent from the ordinary NPC graph.

Acceptance: The permissive lower-bound model includes it or declines certification; graph shortest path is never mislabeled universal physical minimum.

<a id="v21"></a>
**V21 — Movement-faithful advancement** · deterministic replay · [R03](REQUIREMENTS.md#r03), [R04](REQUIREMENTS.md#r04), [R05](REQUIREMENTS.md#r05), [R13](REQUIREMENTS.md#r13)

Replay equal elapsed time with regular/jittered polling and a large requested development advance.

Acceptance: All accepted spans are spent in ordered substeps; route/custody/object results agree within stated tolerance and dropped spans are reported.

## [M5](M5_perception_and_observations.md)

<a id="v22"></a>
**V22 — Local perception** · sim + visual/audio evidence · [R06](REQUIREMENTS.md#r06), [R38](REQUIREMENTS.md#r38)

Put listeners/observers across walls, floors and changing doors, with vertical gaze/fog/darkness; vary arrivals during capture versus STT latency.

Acceptance: Perception follows the declared policy without radius-only witnesses; utterance-time coverage controls delayed speech, partial hearing grants no whole proposition and badges disclose no unseen audience count.

<a id="v23"></a>
**V23 — Temporal sight gap** · deterministic spatial replay · [R06](REQUIREMENTS.md#r06), [R24](REQUIREMENTS.md#r24)

Use identical visible endpoints with an intermediate pillar occlusion, door closure or hidden exchange.

Acceptance: Continuous observation breaks; catch-up endpoints cannot grant unseen intermediate acts or restore continuity retroactively.

<a id="v24"></a>
**V24 — Observable object identity** · sim + host presentation · [R06](REQUIREMENTS.md#r06), [R09](REQUIREMENTS.md#r09), [R24](REQUIREMENTS.md#r24)

Show two similar bundles, pocket the packet or hold it as an unrendered second item.

Acceptance: A visible person does not expose hidden inventory IDs; observation tracks strengthen only through supported features/handling and later examination.

<a id="v25"></a>
**V25 — Last known information** · knowledge-boundary tests · [R06](REQUIREMENTS.md#r06), [R07](REQUIREMENTS.md#r07), [R33](REQUIREMENTS.md#r33), [R34](REQUIREMENTS.md#r34)

Move a person/object unseen while the player and an officer consult existing records.

Acceptance: Both retain last learned locations/times; no hidden transform or movement task leaks into their displays or leads.

## [M6](M6_access_and_permissions.md)

<a id="v26"></a>
**V26 — Keys and thresholds** · sim + host · [R08](REQUIREMENTS.md#r08), [R09](REQUIREMENTS.md#r09), [R36](REQUIREMENTS.md#r36)

Use a wrong, borrowed, copied-pattern, lost or swallowed unique key against current lock/bar states.

Acceptance: Physical match and lawful permission remain separate; identity and actual threshold state survive transfer/recovery/save.

<a id="v27"></a>
**V27 — Scoped entry** · permission integration · [R08](REQUIREMENTS.md#r08), [R17](REQUIREMENTS.md#r17)

Accept loft consent, then attempt rack/desk entry outside its scope or after revocation.

Acceptance: Only covered current acts proceed; later order-backed adapters use real authority and delivery rather than a warrant boolean.

<a id="v28"></a>
**V28 — Keeper and loan availability** · sim + interface · [R08](REQUIREMENTS.md#r08), [R10](REQUIREMENTS.md#r10), [R26](REQUIREMENTS.md#r26)

Request entry/key loan while its keeper is away, occupied or relieved.

Acceptance: A real undertaking, journey and bounded availability policy handles it; the key/keeper is not duplicated or teleported.

<a id="v29"></a>
**V29 — Door operation interruption** · deterministic continuation · [R04](REQUIREMENTS.md#r04), [R08](REQUIREMENTS.md#r08), [R35](REQUIREMENTS.md#r35)

Close/bar a portal during traversal or interrupt an opening while another operation claims it.

Acceptance: Physical state, nav revision, access receipt and resource ownership remain coherent; retries retain overall progress budgets.

## [M7](M7_objects_and_examinations.md)

<a id="v30"></a>
**V30 — Unique-object lineage** · inventory continuation · [R09](REQUIREMENTS.md#r09), [R14](REQUIREMENTS.md#r14), [R36](REQUIREMENTS.md#r36)

Transfer, pocket, wet, transform, swallow and recover a specimen while saving across each boundary.

Acceptance: One physical identity or explicit lineage persists; neither quantity conservation nor custody history silently resets.

<a id="v31"></a>
**V31 — Examination semantics** · sim + interface · [R09](REQUIREMENTS.md#r09), [R11](REQUIREMENTS.md#r11), [R35](REQUIREMENTS.md#r35)

Interrupt a fracture/document comparison, swap an object revision and attempt to remove its reserved original.

Acceptance: Results identify actual method/objects/revisions; incomplete work produces no completed match, and invalidated drafts remain understandable.

<a id="v32"></a>
**V32 — Readable originals and copies** · authoring + sim · [R09](REQUIREMENTS.md#r09), [R11](REQUIREMENTS.md#r11), [R37](REQUIREMENTS.md#r37)

Read an operative document, make an authenticated copy, then alter or lose the original.

Acceptance: The record distinguishes author, authenticity, visible text, copy act and source; possession or handwriting resemblance alone is insufficient.

## [M8](M8_activities_and_appointments.md)

<a id="v33"></a>
**V33 — Duty priority and cover** · deterministic sim · [R10](REQUIREMENTS.md#r10), [R35](REQUIREMENTS.md#r35), [R40](REQUIREMENTS.md#r40)

Combine urgent needs, conversations, curfew, examinations, escort and post relief.

Acceptance: One arbiter owns routes/resources, maintains required cover and reports bounded interruption without inventing staff.

<a id="v34"></a>
**V34 — Appointment without player** · sim + interface · [R10](REQUIREMENTS.md#r10), [R13](REQUIREMENTS.md#r13), [R31](REQUIREMENTS.md#r31), [R33](REQUIREMENTS.md#r33)

A necessary attendee is delayed while the player reads elsewhere through the meeting window.

Acceptance: The meeting uses only permitted present/recorded material, records its actual outcome and supports later business without pausing or teleporting.

<a id="v35"></a>
**V35 — Complete timed demonstration** · replay + measurement · [R05](REQUIREMENTS.md#r05), [R06](REQUIREMENTS.md#r06), [R10](REQUIREMENTS.md#r10)

Observe departure/return but have the stand-in skip hidden work; vary mechanism load and historical sighting travel.

Acceptance: Only covered acts count; endpoints do not certify hidden steps, and the sighting interval includes actual mechanism/travel uncertainty.

<a id="v36"></a>
**V36 — Finite agreements and services** · deterministic continuation · [R26](REQUIREMENTS.md#r26), [R27](REQUIREMENTS.md#r27), [R36](REQUIREMENTS.md#r36)

Let two agreements and ordinary spending compete for the same funds; accept a calendar undertaking and single-use cargo task, interrupt and reload.

Acceptance: Actual resources fund at most one effect; outstanding obligations, finite terms and redemption persist. M9 later integrates formal disclosure/verification, exercised by V40/V58.

<a id="v37"></a>
**V37 — Learned strategy triggers** · knowledge + activity integration · [R10](REQUIREMENTS.md#r10), [R24](REQUIREMENTS.md#r24), [R34](REQUIREMENTS.md#r34)

Send one warning to an actor and keep an identical warning only in the player's private notes.

Acceptance: Only the actually received fact changes an allowed bounded activity strategy; off-stage deterministic duties remain independent of cognition.

## [M9](M9_inquiries_and_evidence.md)

<a id="v38"></a>
**V38 — Minimal sufficient evidence** · data-driven rule tests · [R07](REQUIREMENTS.md#r07), [R11](REQUIREMENTS.md#r11), [R22](REQUIREMENTS.md#r22), [R25](REQUIREMENTS.md#r25)

Remove each essential support, add duplicate retellings, and distinguish physical, admission, recovery and clearance requests.

Acceptance: Each finding downgrades correctly; opportunity, recovery, theft, assault and uncorroborated continuous presence remain distinct.

<a id="v39"></a>
**V39 — Cross-case binding attack** · adversarial rule tests · [R11](REQUIREMENTS.md#r11), [R34](REQUIREMENTS.md#r34)

Mix valid packets, wallet dates, statements and intervals from different incidents or named holders.

Acceptance: Shared typed actor/object/event/interval bindings prevent unrelated valid facts satisfying one conjunction.

<a id="v40"></a>
**V40 — Hidden-cause noninterference** · paired-world rule tests · [R11](REQUIREMENTS.md#r11), [R21](REQUIREMENTS.md#r21), [R34](REQUIREMENTS.md#r34)

Keep admissible inputs identical but change unlearned planting, whispers and retelling histories.

Acceptance: Official findings and diagnostics stay identical until new evidence establishes the difference; no private provenance oracle appears.

<a id="v41"></a>
**V41 — Autonomous supported account** · off-stage deterministic sim · [R07](REQUIREMENTS.md#r07), [R10](REQUIREMENTS.md#r10), [R11](REQUIREMENTS.md#r11), [R31](REQUIREMENTS.md#r31)

With player/cognition absent, an informed NPC lodges their own account and an uninformed NPC attempts the same.

Acceptance: Only actual holdings/authorised records can be submitted; confirmation and recipients are recorded without granting hidden history.

<a id="v42"></a>
**V42 — Amended findings** · deterministic continuation · [R11](REQUIREMENTS.md#r11), [R16](REQUIREMENTS.md#r16), [R20](REQUIREMENTS.md#r20), [R36](REQUIREMENTS.md#r36)

Confirm a false account, later correct it and reopen after save/load.

Acceptance: Old submissions stay immutable, new grounds form an amendment and downstream authority receives one explicit change event.

## [M10](M10_conversation_and_interface.md)

<a id="v43"></a>
**V43 — Input parity and comprehension** · host + human + live language · [R12](REQUIREMENTS.md#r12), [R13](REQUIREMENTS.md#r13), [R30](REQUIREMENTS.md#r30), [R34](REQUIREMENTS.md#r34)

Perform the same confirmed action/channel by voice, typing and controls; mis-transcribe a name and alter only hidden picker/claim data.

Acceptance: Formal actions use the same validated service, unconfirmed accusations do not lodge, already spoken words stay heard and hidden state cannot change learned choices or resolution diagnostics.

<a id="v44"></a>
**V44 — Stale live draft** · hidden-window drive + human · [R12](REQUIREMENTS.md#r12), [R13](REQUIREMENTS.md#r13), [R31](REQUIREMENTS.md#r31)

Keep a selected bundle open while its recipient leaves, item moves or grant changes.

Acceptance: Commit-time validation is specific, preserves drafts and does not invalidate on unrelated crowd revisions or pause the city.

<a id="v45"></a>
**V45 — Sensitive audience change** · perception + interface integration · [R16](REQUIREMENTS.md#r16), [R21](REQUIREMENTS.md#r21), [R34](REQUIREMENTS.md#r34), [R38](REQUIREMENTS.md#r38)

Compare a perceived door opening with paired worlds differing only by an undetected listener; cancel/restore explicit dictation while its transcript is delayed.

Acceptance: Only perceived/reported changes interrupt unemitted presentation; hidden listeners do not change UI or submission outcome. Sealed delivery emits no speech, old spoken effects persist and drafts never fall back to public chat.

<a id="v46"></a>
**V46 — Unknown off-stage result** · host + knowledge boundary · [R13](REQUIREMENTS.md#r13), [R31](REQUIREMENTS.md#r31), [R33](REQUIREMENTS.md#r33)

The notebook stays open past review time while the reviewer is blocked elsewhere.

Acceptance: Display says the scheduled time passed with outcome unconfirmed; held/cancelled/postponed updates require a real received report.

## [M11](M11_orders_and_dispositions.md)

<a id="v47"></a>
**V47 — Independent authority and grounds** · deterministic law · [R11](REQUIREMENTS.md#r11), [R17](REQUIREMENTS.md#r17), [R20](REQUIREMENTS.md#r20)

Return property, centrally recall a mandate still in transit, deliver recall before an older copy and request a named second valid ground.

Acceptance: Registry issuance, finite local grants, received tombstones and holds remain distinct; no psychic cancellation, wrong-order lookup, assault erasure, revived execution or deadline reset occurs.

<a id="v48"></a>
**V48 — Station and hold declarations** · data validation + law · [R17](REQUIREMENTS.md#r17), [R26](REQUIREMENTS.md#r26), [R40](REQUIREMENTS.md#r40)

Validate occupied stations, legacy inmates, sponsorship, deferred review and intake/handover/extension just before, exactly at and after an office cutoff.

Acceptance: Capacity/care/egress and pre-delivered defaults are explicit; exclusive cutoff is deterministic, handover preserves received recall and no delay or exact-boundary extension renews expired authority.

<a id="v49"></a>
**V49 — Ordinary law migration** · base-game regression · [R07](REQUIREMENTS.md#r07), [R17](REQUIREMENTS.md#r17), [R32](REQUIREMENTS.md#r32)

Disable the quest and exercise garbled-hearsay summons, witnessed watch intervention, posted fees, debt and authored inmates.

Acceptance: Each uses its proper capability/provenance; watch cause and prisoner handover meet the declared next-bell procedure; no invented finding or accidental release.

## [M12](M12_dynamic_enforcement.md)

<a id="v50"></a>
**V50 — Issuance to actual arrest** · off-stage end-to-end · [R10](REQUIREMENTS.md#r10), [R18](REQUIREMENTS.md#r18), [R19](REQUIREMENTS.md#r19)

Issue an order with no informed executor, distant player and unavailable cognition; provide a supported lead and feasible resources.

Acceptance: Ordinary checking/delivery, assignment, search, local seizure, route escort and handover all occur through production services.

<a id="v51"></a>
**V51 — Fair blocked dispatch** · deterministic continuation · [R18](REQUIREMENTS.md#r18), [R29](REQUIREMENTS.md#r29), [R35](REQUIREMENTS.md#r35)

Queue three orders including an impossible oldest lead, a feasible later one and a stream of new ordinary work.

Acceptance: Stable age/priority and persisted retry/parking prevent starvation or monopolisation; exhausted pre-seizure claims are released.

<a id="v52"></a>
**V52 — Physical pursuit and escort** · sim + hidden-window drive · [R04](REQUIREMENTS.md#r04), [R06](REQUIREMENTS.md#r06), [R19](REQUIREMENTS.md#r19), [R35](REQUIREMENTS.md#r35)

Lose sight, turn L-corners, climb stairs and meet opposing escorts/temporary or permanent blockers.

Acceptance: Search uses learned leads; both bodies move within speed/collision limits, preserve overall recovery budgets and never snap through walls.

<a id="v53"></a>
**V53 — Capacity and overnight cover** · sim + content acceptance · [R18](REQUIREMENTS.md#r18), [R19](REQUIREMENTS.md#r19), [R40](REQUIREMENTS.md#r40)

Two escorts compete for one intake slot while multiple arrests and Snuffing threaten keeper/post coverage.

Acceptance: Slot reservation/handover is unique, physical inmates count, available relief is real and occupied stations retain the declared care policy.

<a id="v54"></a>
**V54 — Coordination races** · deterministic continuation · [R16](REQUIREMENTS.md#r16), [R17](REQUIREMENTS.md#r17), [R19](REQUIREMENTS.md#r19)

Separate central recall from actual officer/keeper receipt during search/escort/intake; attach a second ground, race autonomous seizure and reload.

Acceptance: Distant behavior changes only after available causes; local tombstones/cutoffs remain binding, one shared execution/custody persists and independent grounds keep original deadlines.

<a id="v55"></a>
**V55 — Release and booked property** · sim + host · [R09](REQUIREMENTS.md#r09), [R20](REQUIREMENTS.md#r20), [R40](REQUIREMENTS.md#r40)

Expire a hold while a door is blocked and an offered packet's booking transfer is pending; replace its keeper.

Acceptance: Legal release, restraint removal and actual egress are distinct; property custody/ownership/return claims persist with explicit failed or completed receipts.

## [M13](M13_authoring_and_foundation_acceptance.md)

<a id="v56"></a>
**V56 — Independent author proves reuse** · author demonstration · [R01](REQUIREMENTS.md#r01), [R11](REQUIREMENTS.md#r11), [R28](REQUIREMENTS.md#r28)

An author builds two differently structured disputes, including one with no offender, plus pursuit and agreement fixtures.

Acceptance: Normal UI/headless services work from data; no branch in the engine, evaluator or guard service checks a case/actor ID to make them function.

<a id="v57"></a>
**V57 — Pack validation and tooling** · authoring validation · [R01](REQUIREMENTS.md#r01), [R03](REQUIREMENTS.md#r03), [R28](REQUIREMENTS.md#r28), [R34](REQUIREMENTS.md#r34)

Load packs with dangling roles, dates, references, essential inaccessible locations, missing adapters or private-fact leaks.

Acceptance: Actionable source diagnostics appear before partial installation; documented shortcuts use real services and do not patch clues into saves.

<a id="v58"></a>
**V58 — Foundation continuation gate** · fresh-process integrated acceptance · [R01](REQUIREMENTS.md#r01), [R07](REQUIREMENTS.md#r07), [R14](REQUIREMENTS.md#r14), [R15](REQUIREMENTS.md#r15), [R29](REQUIREMENTS.md#r29), [R36](REQUIREMENTS.md#r36)

Save/restore independent fixtures across every new subsystem, after more than 256 mints/six holdings and under declared archive-quota saturation and closed/reopened inquiries.

Acceptance: All owners extend continuation; essential/transitive evidence remains addressable without unlimited hot news, admission fails before unsupported installation and full foundation acceptance precedes Tallage content.

## [M14](M14_tallage_district.md)

<a id="v59"></a>
**V59 — A convincing ordinary district** · site review + human + drive · [R03](REQUIREMENTS.md#r03), [R04](REQUIREMENTS.md#r04), [R28](REQUIREMENTS.md#r28), [R30](REQUIREMENTS.md#r30)

Compare candidate plans/sections and traverse ordinary work, cart and pedestrian paths with the quest disabled.

Acceptance: Interiors and private links have mundane purposes; entrances/nav/collision/housing agree and the detour does not depend on invisible restrictions.

<a id="v60"></a>
**V60 — Historical route certificate** · geometry + temporal evidence · [R05](REQUIREMENTS.md#r05), [R06](REQUIREMENTS.md#r06), [R28](REQUIREMENTS.md#r28)

Measure complete private work, conservative public alternatives, Lise's sighting endpoints and Warin's cry-alignment coverage.

Acceptance: The report meets the advance-selected margin/uncertainty relation under established historical conditions, or the strong claim remains unaccepted.

## [M15](M15_incident_and_cast.md)

<a id="v61"></a>
**V61 — One established history** · content continuation · [R02](REQUIREMENTS.md#r02), [R03](REQUIREMENTS.md#r03), [R24](REQUIREMENTS.md#r24), [R36](REQUIREMENTS.md#r36)

Open different fresh civil dates/offices, then advance a late developer start from the same anchor, reload and discover out of order.

Acceptance: Past history and resolved future offices install once; hold expiry is independent, basic retrieval moves real property, and late/load operations never re-anchor deadlines.

<a id="v62"></a>
**V62 — Operative papers and accounts** · content/schema review · [R09](REQUIREMENTS.md#r09), [R21](REQUIREMENTS.md#r21), [R23](REQUIREMENTS.md#r23), [R37](REQUIREMENTS.md#r37), [R39](REQUIREMENTS.md#r39)

Inspect the genuine pledge, forged release, counterfoil, lodged Lise account and restricted Odo detail.

Acceptance: Exact text, parties, wallet identity/dating, unpaid balance and disclosure provenance support the promised noncircular routes without extra mandatory witnesses.

<a id="v63"></a>
**V63 — Stable cast in the city** · lore validation + sim · [R02](REQUIREMENTS.md#r02), [R28](REQUIREMENTS.md#r28), [R32](REQUIREMENTS.md#r32), [R37](REQUIREMENTS.md#r37)

Install Mott's stable tier/duties and the existing seven-person cast with their normal homes/roles.

Acceptance: No required ambient-only reference remains, cast knowledge is intentionally seeded and ordinary occupations continue around the incident.

## [M16](M16_investigation_and_public_review.md)

<a id="v64"></a>
**V64 — Early physical full case** · full content playthrough · [R11](REQUIREMENTS.md#r11), [R22](REQUIREMENTS.md#r22), [R37](REQUIREMENTS.md#r37)

Recover the packet early; obtain origin, fracture match and the same wallet's dating, with no confession or surveillance.

Acceptance: E01's sole continuous encounter and bound authenticated chain support original theft and assault; opportunity alone is insufficient.

<a id="v65"></a>
**V65 — Warin-only success** · full content playthrough · [R06](REQUIREMENTS.md#r06), [R11](REQUIREMENTS.md#r11), [R25](REQUIREMENTS.md#r25)

Establish Averil/Gile's independent continued observation and cry relation, then stop investigating Corin.

Acceptance: Warin can be formally cleared under the supported procedure, and the game recognises a legitimate partial achievement without inventing a culprit.

<a id="v66"></a>
**V66 — Public review while life continues** · off-stage content playthrough · [R10](REQUIREMENTS.md#r10), [R13](REQUIREMENTS.md#r13), [R25](REQUIREMENTS.md#r25), [R31](REQUIREMENTS.md#r31), [R33](REQUIREMENTS.md#r33)

Lodge partial/full bundles or leave NPCs to submit supported material while the player misses High Wick.

Acceptance: The actual review can be limited, delayed or successful; only lodged evidence counts and later records report its real outcome.

## [M17](M17_alternatives_and_recovery.md)

<a id="v67"></a>
**V67 — Late physical recovery** · full content playthrough · [R08](REQUIREMENTS.md#r08), [R11](REQUIREMENTS.md#r11), [R23](REQUIREMENTS.md#r23), [R31](REQUIREMENTS.md#r31)

Miss retrieval/review; Corin refuses wallet comparison and the bargain, Lise is absent and the rack is empty.

Acceptance: The logged fragment and retrievable wallet deposition permit comparison; its match plus Lise's prior identification of the particular desk and failed rack search permit scoped desk access before packet recovery.

<a id="v68"></a>
**V68 — Private admission** · full content playthrough · [R11](REQUIREMENTS.md#r11), [R21](REQUIREMENTS.md#r21), [R34](REQUIREMENTS.md#r34), [R38](REQUIREMENTS.md#r38)

Confirm named acts and independently supplied detail; contrast an established leading leak with an unlearned secret leak.

Acceptance: Only supported corroboration is certified; sealed delivery is actual, hidden histories do not become diagnostics and later established contamination can amend findings.

<a id="v69"></a>
**V69 — Watch the moving packet** · spatial content playthrough · [R06](REQUIREMENTS.md#r06), [R11](REQUIREMENTS.md#r11), [R24](REQUIREMENTS.md#r24), [R31](REQUIREMENTS.md#r31)

Follow actual retrieval, warn Corin through a real recipient, lose sight, or observe a planted/handed-over similar bundle.

Acceptance: Bounded strategy changes only from learned warnings; continuity and identity support handling at most unless further evidence establishes the original acts.

<a id="v70"></a>
**V70 — Hush and sponsorship terms** · full content playthrough · [R20](REQUIREMENTS.md#r20), [R21](REQUIREMENTS.md#r21), [R26](REQUIREMENTS.md#r26), [R27](REQUIREMENTS.md#r27)

Contrast legitimate sealed restitution with a hush bundle withheld even from sealed institutional delivery; let another witness disclose and miss a sponsored report.

Acceptance: Counterparties, real payments and finite terms remain distinct; third-party speech is not player breach, forbidden hush terms gain no automatic legal remedy, and sponsorship never becomes innocence or a renewed hold.

<a id="v71"></a>
**V71 — Exploration and interruption** · slow/exploratory human + replay · [R02](REQUIREMENTS.md#r02), [R13](REQUIREMENTS.md#r13), [R23](REQUIREMENTS.md#r23), [R31](REQUIREMENTS.md#r31)

Find the passage/packet first, read through events, interrupt a search and return after a day.

Acceptance: Discovery is retained without reseeding, missed observations remain missed and at least one supported complete route still exists.

## [M18](M18_lasting_consequences.md)

<a id="v72"></a>
**V72 — Consequences happen elsewhere** · longitudinal content replay · [R17](REQUIREMENTS.md#r17), [R18](REQUIREMENTS.md#r18), [R19](REQUIREMENTS.md#r19), [R20](REQUIREMENTS.md#r20)

Issue/amend orders while Corin, Warin and the player are in different districts; revisit after an office/day.

Acceptance: Actual dispatch, custody, release and corrections occur or remain honestly blocked; learned records distinguish orders from completed arrests.

<a id="v73"></a>
**V73 — Redemption and earned service** · content economy continuation · [R09](REQUIREMENTS.md#r09), [R26](REQUIREMENTS.md#r26), [R27](REQUIREMENTS.md#r27), [R39](REQUIREMENTS.md#r39)

Reject forged collection while the pledge remains unpaid, then supply genuine payment and redeem Warin's cargo favour twice.

Acceptance: No free return follows forgery alone; authorised delivery conserves tools/funds and one-use service requires real permission/resources and fulfils once.

<a id="v74"></a>
**V74 — Aftermath and reopening** · multi-day continuation · [R07](REQUIREMENTS.md#r07), [R08](REQUIREMENTS.md#r08), [R20](REQUIREMENTS.md#r20), [R33](REQUIREMENTS.md#r33), [R36](REQUIREMENTS.md#r36)

Let gossip/night work run, change who learns the correction, revisit Lise's conditional passage grant/restriction and reopen after loading.

Acceptance: Actual knowledge drives responses and scoped access; learned geography persists, and amendments do not reset evidence, money, actors or completed orders.

## [M19](M19_acceptance_and_release.md)

<a id="v75"></a>
**V75 — Whole delivery endurance** · release integration · [R13](REQUIREMENTS.md#r13), [R14](REQUIREMENTS.md#r14), [R15](REQUIREMENTS.md#r15), [R16](REQUIREMENTS.md#r16), [R29](REQUIREMENTS.md#r29), [R36](REQUIREMENTS.md#r36)

Run representative complete paths over multiple game days and save before/after every decisive boundary.

Acceptance: Full suites and fresh-process continuation pass; retention stays bounded without dropping referenced evidence, renewing duties or losing city state.

<a id="v76"></a>
**V76 — Human investigation** · human formative sessions · [R04](REQUIREMENTS.md#r04), [R12](REQUIREMENTS.md#r12), [R30](REQUIREMENTS.md#r30), [R31](REQUIREMENTS.md#r31), [R37](REQUIREMENTS.md#r37)

Observe five to eight formative testers across voice/keyboard, familiarity and slow/exploratory styles.

Acceptance: They explain their reconstruction, distinguish opportunity from identification, understand pending actions and find the spatial/social work worth doing; concrete failures drive revision.

<a id="v77"></a>
**V77 — Live language boundaries** · live-provider evidence · [R07](REQUIREMENTS.md#r07), [R12](REQUIREMENTS.md#r12), [R21](REQUIREMENTS.md#r21), [R30](REQUIREMENTS.md#r30), [R34](REQUIREMENTS.md#r34)

Exercise holders/non-holders, evasions, specific admissions, false accusations and disclosure with live providers.

Acceptance: Models neither invent physical results nor leak sealed history; deterministic confirmation/receipts preserve mechanics when prose varies or requests fail.

<a id="v78"></a>
**V78 — Performance and compatibility release** · measurement + release audit · [R13](REQUIREMENTS.md#r13), [R15](REQUIREMENTS.md#r15), [R29](REQUIREMENTS.md#r29), [R32](REQUIREMENTS.md#r32)

Compare accepted reference runs at default/2,000 citizens and separate 20,000 stress, including saves, retirement and concurrent ordinary work.

Acceptance: Agreed numeric time/memory budgets and declared behavior/content compatibility pass; known baseline stress cost is separated from added multiplicative cost.
