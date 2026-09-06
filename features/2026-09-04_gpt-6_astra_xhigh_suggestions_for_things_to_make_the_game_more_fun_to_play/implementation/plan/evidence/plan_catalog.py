# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Build/check the plan's traceability tables. This does not run game acceptance."""

from __future__ import annotations

import argparse
from collections import defaultdict
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
PLAN = HERE.parent

# ID, governing decisions, requirement. Milestone coverage derives from scenarios.
REQUIREMENTS = [
    ("R01", "D01", "Prove reusable systems with independent content before authoring the full case."),
    ("R02", "D02", "Install the past incident once in normal city history, without acceptance-triggered spawning."),
    ("R03", "D02", "Provide developer starts through the same production content and movement-faithful progression."),
    ("R04", "D03", "Player and NPC traverse coherent interiors, floors, stairs and real openings."),
    ("R05", "D03", "Support timing claims with complete measured acts, uncertainty and conservative route coverage."),
    ("R06", "D03 D04", "Witnesses see/hear only supported local events, with temporal and identity limits."),
    ("R07", "D01 D04", "Use the shipped shared knowledge system and preserve durable learned evidence references."),
    ("R08", "D01 D03", "Physical access, keys, consent and official scope work as ordinary reusable systems."),
    ("R09", "D01 D06", "Objects, documents, containment, transfers and examinations preserve identity and custody."),
    ("R10", "D01 D05", "Activities and appointments compete with ordinary duties and continue off stage."),
    ("R11", "D01 D04", "Findings evaluate bound, supported propositions and distinguish partial conclusions."),
    ("R12", "D04", "Natural language and explicit controls use equivalent validated actions and truthful receipts."),
    ("R13", "D05", "Reading, conversation, input, menus and provider latency never pause the city."),
    ("R14", "D06", "Checkpoint the complete authoritative simulation, including pending semantic work."),
    ("R15", "D06", "Save anywhere and adopt a coherent host/sim world with durable ordered slot publication."),
    ("R16", "D04 D06", "Fence old-world callbacks and deduplicate committed effects across save/load and retries."),
    ("R17", "D07", "Separate allegations, authority, deliveries, execution and independent custody grounds."),
    ("R18", "D07", "Issued orders reach informed executors through ordinary channels and fair bounded dispatch."),
    ("R19", "D03 D07", "Guards search by learned leads and physically seize, escort and hand over a subject."),
    ("R20", "D07", "Correction, release, property, reputation and other proceedings remain distinct lasting outcomes."),
    ("R21", "D01 D04", "Private admissions and restricted submissions have supported corroboration and truthful privacy."),
    ("R22", "D01", "The early physical route supports the original theft and assault without confession or surveillance."),
    ("R23", "D01 D05", "A late player can complete the physical route without a bargain or the missed retrieval."),
    ("R24", "D01 D03", "Surveillance observes actual object handling and supports only its warranted allegation."),
    ("R25", "D01", "Independent cry witnesses can clear Warin without identifying Corin."),
    ("R26", "D01 D07", "Agreements, sponsorship, confidential terms and one-use services have executable obligations."),
    ("R27", "D01 D07", "Payments, rewards and property deliveries conserve real resources and fulfil once."),
    ("R28", "D01 D03", "Versioned content packs, actual district authoring and stable cast bindings support future authors."),
    ("R29", "D01 D05 D06", "New systems have measured bounded time, memory, IO and retention costs."),
    ("R30", "D01 D04", "Release includes human comprehension/enjoyment and live-language evidence, not only fake traces."),
    ("R31", "D05", "Missed appointments, changing circumstances and absence leave truthful records and continuation."),
    ("R32", "D01 D07", "Migrations preserve a coherent ordinary city and reconcile existing knowledge, keys and law."),
    ("R33", "D04 D05", "Player displays distinguish unknown off-stage outcomes from known scheduled or completed events."),
    ("R34", "D04", "Findings and diagnostics cannot reveal unlearned hidden truth or causal provenance."),
    ("R35", "D01 D05", "One shared operation/duty/resource owner controls progress, interruption and recovery."),
    ("R36", "D06", "Every later subsystem extends persistence and continuation evidence in its own milestone."),
    ("R37", "D01 D04", "The complete case has readable operative documents, independently sourced accounts and clear stakes."),
    ("R38", "D03 D04", "Sealed record access and physically audible speech remain separate during live audience changes."),
    ("R39", "D01 D07", "Forged collection authority, valid pledge debt and actual redemption are distinct property states."),
    ("R40", "D07", "Station capacity, keeper coverage, hold deadlines and actual release egress are authoritative."),
]

# ID, owner milestone, covered requirements, title, adversarial setup/action,
# observable acceptance, evidence kind. All are PLANNED, never execution claims.
SCENARIOS = [
    ("V01", 0, "R01 R07 R32", "Knowledge prerequisite", "Reconcile the completed knowledge M0–M5 against current source and its own acceptance record.", "The plan uses shipped holders, provenance, player-learning and retention APIs; no guessed duplicate store or stale quest stub remains.", "source + existing feature acceptance"),
    ("V02", 0, "R14 R32 R35", "Authority inventory", "Enumerate mutable sim/host owners, capabilities and migration seams on the actual implementation baseline.", "Every future-affecting field has a checkpoint policy; every new shared service has one producer and explicit consumers.", "source audit"),
    ("V03", 0, "R29 R32", "Reproducible baseline", "Run accepted baseline suites and profile movement-faithful default/2,000-citizen runs, with 20,000 separately labeled stress.", "Record commit/tree, hardware, poll pattern, CPU/frame/memory figures and numeric budgets. A coarse clock watcher is not movement evidence.", "tests + measurement"),
    ("V04", 1, "R13 R35", "Live overlays and conversation", "Keep a conversation/floor hold and each existing menu open across movement, need and calendar transitions.", "Simulation events and ordinary duties continue; only input capture or a particular person's permitted courtesy changes.", "sim + host"),
    ("V05", 1, "R13 R14 R16", "Clock crossing ownership", "Change 1×/60× immediately after bedtime and midnight while Round skips a cadence tick.", "One office, production/settlement edge and ambient/night duty is processed; historical cursors are not reinterpreted through the new rate.", "deterministic sim"),
    ("V06", 1, "R12 R16 R29", "No partial or duplicate action", "Repeat commands, conflict their payloads, exhaust the bounded replay window and inject failed route/authority/resource validation.", "One effect/receipt at most; rejection leaves no partial custody/inventory mutation, and compacted IDs including never-accepted requests cannot replay. M2 extends the same fixture through restore.", "deterministic sim"),
    ("V07", 1, "R10 R35", "Minimal operation kernel", "Two duties claim one actor/resource; interrupt and replan during bounded in-memory work.", "One owner, conserved completed steps and operation-wide recovery budget; replanning does not reset exhaustion. M2 adds the checkpoint continuation in V09.", "deterministic sim"),
    ("V08", 1, "R16", "World and actor generations", "Deliver a result for an old world and a departed incarnation with reused numeric request/actor IDs.", "Both identity fences work independently; no current-world effect or presentation acknowledgement is accepted.", "sim + host"),
    ("V09", 2, "R14 R16 R35 R36", "Populated checkpoint", "Capture M1 kernel ownership/progress and exhausted replay windows plus ordinary queues, production, offers, road parties, weather, lamps, knowledge, custody and needs mid-operation.", "Fresh hydration resumes canonical state/events and replay rejection with controlled subsequent inputs, without new/seed paths or omitted subsystem state.", "deterministic continuation"),
    ("V10", 2, "R14 R16", "Pending cognition categories", "Capture an empty-inbox idle submission and a held completion followed by newer player speech.", "Unfinished work restores one owed retry; held results apply once without resubmission; old drained history and newer obligations remain distinct.", "deterministic continuation"),
    ("V11", 2, "R14 R16", "Night obligation identity", "Capture queued/submitted/completed Night duties around midnight and actor departure/re-entry.", "Owed day, incarnation and spent/completed state survive; no doubled reflection or ambient reroll, and valid retries still yield to the player.", "deterministic continuation"),
    ("V12", 2, "R14 R15", "Validation and compatibility", "Load valid Never sentinels, invalid floats, duplicate ownership, overflow, missing adapters and incompatible content/generator versions.", "Virgin worlds work; invalid candidates fail with subsystem diagnostics before adoption; no defaults are silently reseeded.", "DTO validation"),
    ("V13", 2, "R14 R16", "Capture is observation", "Request repeated saves with no elapsed time, including committed receipts not yet consumed by the host.", "No extra poll, cognition, event or physical change; all committed readable effects at the watermark belong to the capture.", "sim + host"),
    ("V14", 3, "R14 R15 R16", "Matched physical capture", "Save mid-jump at nonzero host/sim residual and ordinary pre-capture wall debt while a custody threshold command is queued.", "Accepted sim pose equals the authoritative body; residuals/debt remain distinct, no offline/preparation debt is added and resistance emits its threshold effect once.", "fresh-process host continuation"),
    ("V15", 3, "R15 R16", "Late callback storm", "After adoption deliver old cognition, Night, STT, TTS, PCM, microphone/backend-status and SpeechPresented callbacks.", "No old voice, automatic utterance or domain effect enters the loaded generation, including reused IDs.", "host/backend integration"),
    ("V16", 3, "R15 R29", "Durable ordered saves", "Reverse two same-slot write completions; inject IO failures and process death after payload, recovery-reference and active-reference publication/flush steps.", "The latest authorized durable publication wins; an acknowledged prior reference/payload remains recoverable, uncertain publication is not reported as success and retention stays bounded.", "backend fault injection"),
    ("V17", 3, "R13 R15 R29", "Prepared adoption and retirement", "Delay load preparation while the city runs, then inject staging/adoption failure and expensive old-world retirement.", "Adoption resumes at the saved instant and swaps one coherent bundle; rollback includes clocks/UI; retirement does not create an unbudgeted frame stall.", "host integration + measurement"),
    ("V18", 3, "R15 R16 R33", "Presentation continuation", "Save during speech/recording and on both sides of rat-percept and civic/well cue deadlines; later M10 extends this fixture with explicit dictation purposes and cancelled drafts.", "Committed lines/history remain without repeated speech, unsent text stays a draft and late callbacks cannot broadcast it; semantic rat events and presented cues do not duplicate or vanish.", "fresh-process host continuation"),
    ("V19", 4, "R04 R08 R36", "Layered body parity", "Walk player and NPC through stairs, L-corners, landings, low headroom, closed portals and vertically overlapping rooms.", "Both use actual supported routes and consistent collision/topology; no ground flattening, wall crossing or accidental cross-floor interaction.", "sim + hidden-window drive"),
    ("V20", 4, "R04 R05", "Conservative route coverage", "Add a usable roof/drop/jump or boundary shortcut absent from the ordinary NPC graph.", "The permissive lower-bound model includes it or declines certification; graph shortest path is never mislabeled universal physical minimum.", "geometry analysis"),
    ("V21", 4, "R03 R04 R05 R13", "Movement-faithful advancement", "Replay equal elapsed time with regular/jittered polling and a large requested development advance.", "All accepted spans are spent in ordered substeps; route/custody/object results agree within stated tolerance and dropped spans are reported.", "deterministic replay"),
    ("V22", 5, "R06 R38", "Local perception", "Put listeners/observers across walls, floors and changing doors, with vertical gaze/fog/darkness; vary arrivals during capture versus STT latency.", "Perception follows the declared policy without radius-only witnesses; utterance-time coverage controls delayed speech, partial hearing grants no whole proposition and badges disclose no unseen audience count.", "sim + visual/audio evidence"),
    ("V23", 5, "R06 R24", "Temporal sight gap", "Use identical visible endpoints with an intermediate pillar occlusion, door closure or hidden exchange.", "Continuous observation breaks; catch-up endpoints cannot grant unseen intermediate acts or restore continuity retroactively.", "deterministic spatial replay"),
    ("V24", 5, "R06 R09 R24", "Observable object identity", "Show two similar bundles, pocket the packet or hold it as an unrendered second item.", "A visible person does not expose hidden inventory IDs; observation tracks strengthen only through supported features/handling and later examination.", "sim + host presentation"),
    ("V25", 5, "R06 R07 R33 R34", "Last known information", "Move a person/object unseen while the player and an officer consult existing records.", "Both retain last learned locations/times; no hidden transform or movement task leaks into their displays or leads.", "knowledge-boundary tests"),
    ("V26", 6, "R08 R09 R36", "Keys and thresholds", "Use a wrong, borrowed, copied-pattern, lost or swallowed unique key against current lock/bar states.", "Physical match and lawful permission remain separate; identity and actual threshold state survive transfer/recovery/save.", "sim + host"),
    ("V27", 6, "R08 R17", "Scoped entry", "Accept loft consent, then attempt rack/desk entry outside its scope or after revocation.", "Only covered current acts proceed; later order-backed adapters use real authority and delivery rather than a warrant boolean.", "permission integration"),
    ("V28", 6, "R08 R10 R26", "Keeper and loan availability", "Request entry/key loan while its keeper is away, occupied or relieved.", "A real undertaking, journey and bounded availability policy handles it; the key/keeper is not duplicated or teleported.", "sim + interface"),
    ("V29", 6, "R04 R08 R35", "Door operation interruption", "Close/bar a portal during traversal or interrupt an opening while another operation claims it.", "Physical state, nav revision, access receipt and resource ownership remain coherent; retries retain overall progress budgets.", "deterministic continuation"),
    ("V30", 7, "R09 R14 R36", "Unique-object lineage", "Transfer, pocket, wet, transform, swallow and recover a specimen while saving across each boundary.", "One physical identity or explicit lineage persists; neither quantity conservation nor custody history silently resets.", "inventory continuation"),
    ("V31", 7, "R09 R11 R35", "Examination semantics", "Interrupt a fracture/document comparison, swap an object revision and attempt to remove its reserved original.", "Results identify actual method/objects/revisions; incomplete work produces no completed match, and invalidated drafts remain understandable.", "sim + interface"),
    ("V32", 7, "R09 R11 R37", "Readable originals and copies", "Read an operative document, make an authenticated copy, then alter or lose the original.", "The record distinguishes author, authenticity, visible text, copy act and source; possession or handwriting resemblance alone is insufficient.", "authoring + sim"),
    ("V33", 8, "R10 R35 R40", "Duty priority and cover", "Combine urgent needs, conversations, curfew, examinations, escort and post relief.", "One arbiter owns routes/resources, maintains required cover and reports bounded interruption without inventing staff.", "deterministic sim"),
    ("V34", 8, "R10 R13 R31 R33", "Appointment without player", "A necessary attendee is delayed while the player reads elsewhere through the meeting window.", "The meeting uses only permitted present/recorded material, records its actual outcome and supports later business without pausing or teleporting.", "sim + interface"),
    ("V35", 8, "R05 R06 R10", "Complete timed demonstration", "Observe departure/return but have the stand-in skip hidden work; vary mechanism load and historical sighting travel.", "Only covered acts count; endpoints do not certify hidden steps, and the sighting interval includes actual mechanism/travel uncertainty.", "replay + measurement"),
    ("V36", 8, "R26 R27 R36", "Finite agreements and services", "Let two agreements and ordinary spending compete for the same funds; accept a calendar undertaking and single-use cargo task, interrupt and reload.", "Actual resources fund at most one effect; outstanding obligations, finite terms and redemption persist. M9 later integrates formal disclosure/verification, exercised by V40/V58.", "deterministic continuation"),
    ("V37", 8, "R10 R24 R34", "Learned strategy triggers", "Send one warning to an actor and keep an identical warning only in the player's private notes.", "Only the actually received fact changes an allowed bounded activity strategy; off-stage deterministic duties remain independent of cognition.", "knowledge + activity integration"),
    ("V38", 9, "R07 R11 R22 R25", "Minimal sufficient evidence", "Remove each essential support, add duplicate retellings, and distinguish physical, admission, recovery and clearance requests.", "Each finding downgrades correctly; opportunity, recovery, theft, assault and uncorroborated continuous presence remain distinct.", "data-driven rule tests"),
    ("V39", 9, "R11 R34", "Cross-case binding attack", "Mix valid packets, wallet dates, statements and intervals from different incidents or named holders.", "Shared typed actor/object/event/interval bindings prevent unrelated valid facts satisfying one conjunction.", "adversarial rule tests"),
    ("V40", 9, "R11 R21 R34", "Hidden-cause noninterference", "Keep admissible inputs identical but change unlearned planting, whispers and retelling histories.", "Official findings and diagnostics stay identical until new evidence establishes the difference; no private provenance oracle appears.", "paired-world rule tests"),
    ("V41", 9, "R07 R10 R11 R31", "Autonomous supported account", "With player/cognition absent, an informed NPC lodges their own account and an uninformed NPC attempts the same.", "Only actual holdings/authorised records can be submitted; confirmation and recipients are recorded without granting hidden history.", "off-stage deterministic sim"),
    ("V42", 9, "R11 R16 R20 R36", "Amended findings", "Confirm a false account, later correct it and reopen after save/load.", "Old submissions stay immutable, new grounds form an amendment and downstream authority receives one explicit change event.", "deterministic continuation"),
    ("V43", 10, "R12 R13 R30 R34", "Input parity and comprehension", "Perform the same confirmed action/channel by voice, typing and controls; mis-transcribe a name and alter only hidden picker/claim data.", "Formal actions use the same validated service, unconfirmed accusations do not lodge, already spoken words stay heard and hidden state cannot change learned choices or resolution diagnostics.", "host + human + live language"),
    ("V44", 10, "R12 R13 R31", "Stale live draft", "Keep a selected bundle open while its recipient leaves, item moves or grant changes.", "Commit-time validation is specific, preserves drafts and does not invalidate on unrelated crowd revisions or pause the city.", "hidden-window drive + human"),
    ("V45", 10, "R16 R21 R34 R38", "Sensitive audience change", "Compare a perceived door opening with paired worlds differing only by an undetected listener; cancel/restore explicit dictation while its transcript is delayed.", "Only perceived/reported changes interrupt unemitted presentation; hidden listeners do not change UI or submission outcome. Sealed delivery emits no speech, old spoken effects persist and drafts never fall back to public chat.", "perception + interface integration"),
    ("V46", 10, "R13 R31 R33", "Unknown off-stage result", "The notebook stays open past review time while the reviewer is blocked elsewhere.", "Display says the scheduled time passed with outcome unconfirmed; held/cancelled/postponed updates require a real received report.", "host + knowledge boundary"),
    ("V47", 11, "R11 R17 R20", "Independent authority and grounds", "Return property, centrally recall a mandate still in transit, deliver recall before an older copy and request a named second valid ground.", "Registry issuance, finite local grants, received tombstones and holds remain distinct; no psychic cancellation, wrong-order lookup, assault erasure, revived execution or deadline reset occurs.", "deterministic law"),
    ("V48", 11, "R17 R26 R40", "Station and hold declarations", "Validate occupied stations, legacy inmates, sponsorship, deferred review and intake/handover/extension just before, exactly at and after an office cutoff.", "Capacity/care/egress and pre-delivered defaults are explicit; exclusive cutoff is deterministic, handover preserves received recall and no delay or exact-boundary extension renews expired authority.", "data validation + law"),
    ("V49", 11, "R07 R17 R32", "Ordinary law migration", "Disable the quest and exercise garbled-hearsay summons, witnessed watch intervention, posted fees, debt and authored inmates.", "Each uses its proper capability/provenance; watch cause and prisoner handover meet the declared next-bell procedure; no invented finding or accidental release.", "base-game regression"),
    ("V50", 12, "R10 R18 R19", "Issuance to actual arrest", "Issue an order with no informed executor, distant player and unavailable cognition; provide a supported lead and feasible resources.", "Ordinary checking/delivery, assignment, search, local seizure, route escort and handover all occur through production services.", "off-stage end-to-end"),
    ("V51", 12, "R18 R29 R35", "Fair blocked dispatch", "Queue three orders including an impossible oldest lead, a feasible later one and a stream of new ordinary work.", "Stable age/priority and persisted retry/parking prevent starvation or monopolisation; exhausted pre-seizure claims are released.", "deterministic continuation"),
    ("V52", 12, "R04 R06 R19 R35", "Physical pursuit and escort", "Lose sight, turn L-corners, climb stairs and meet opposing escorts/temporary or permanent blockers.", "Search uses learned leads; both bodies move within speed/collision limits, preserve overall recovery budgets and never snap through walls.", "sim + hidden-window drive"),
    ("V53", 12, "R18 R19 R40", "Capacity and overnight cover", "Two escorts compete for one intake slot while multiple arrests and Snuffing threaten keeper/post coverage.", "Slot reservation/handover is unique, physical inmates count, available relief is real and occupied stations retain the declared care policy.", "sim + content acceptance"),
    ("V54", 12, "R16 R17 R19", "Coordination races", "Separate central recall from actual officer/keeper receipt during search/escort/intake; attach a second ground, race autonomous seizure and reload.", "Distant behavior changes only after available causes; local tombstones/cutoffs remain binding, one shared execution/custody persists and independent grounds keep original deadlines.", "deterministic continuation"),
    ("V55", 12, "R09 R20 R40", "Release and booked property", "Expire a hold while a door is blocked and an offered packet's booking transfer is pending; replace its keeper.", "Legal release, restraint removal and actual egress are distinct; property custody/ownership/return claims persist with explicit failed or completed receipts.", "sim + host"),
    ("V56", 13, "R01 R11 R28", "Independent author proves reuse", "An author builds two differently structured disputes, including one with no offender, plus pursuit and agreement fixtures.", "Normal UI/headless services work from data; no branch in the engine, evaluator or guard service checks a case/actor ID to make them function.", "author demonstration"),
    ("V57", 13, "R01 R03 R28 R34", "Pack validation and tooling", "Load packs with dangling roles, dates, references, essential inaccessible locations, missing adapters or private-fact leaks.", "Actionable source diagnostics appear before partial installation; documented shortcuts use real services and do not patch clues into saves.", "authoring validation"),
    ("V58", 13, "R01 R07 R14 R15 R29 R36", "Foundation continuation gate", "Save/restore independent fixtures across every new subsystem, after more than 256 mints/six holdings and under declared archive-quota saturation and closed/reopened inquiries.", "All owners extend continuation; essential/transitive evidence remains addressable without unlimited hot news, admission fails before unsupported installation and full foundation acceptance precedes Tallage content.", "fresh-process integrated acceptance"),
    ("V59", 14, "R03 R04 R28 R30", "A convincing ordinary district", "Compare candidate plans/sections and traverse ordinary work, cart and pedestrian paths with the quest disabled.", "Interiors and private links have mundane purposes; entrances/nav/collision/housing agree and the detour does not depend on invisible restrictions.", "site review + human + drive"),
    ("V60", 14, "R05 R06 R28", "Historical route certificate", "Measure complete private work, conservative public alternatives, Lise's sighting endpoints and Warin's cry-alignment coverage.", "The report meets the advance-selected margin/uncertainty relation under established historical conditions, or the strong claim remains unaccepted.", "geometry + temporal evidence"),
    ("V61", 15, "R02 R03 R24 R36", "One established history", "Open different fresh civil dates/offices, then advance a late developer start from the same anchor, reload and discover out of order.", "Past history and resolved future offices install once; hold expiry is independent, basic retrieval moves real property, and late/load operations never re-anchor deadlines.", "content continuation"),
    ("V62", 15, "R09 R21 R23 R37 R39", "Operative papers and accounts", "Inspect the genuine pledge, forged release, counterfoil, lodged Lise account and restricted Odo detail.", "Exact text, parties, wallet identity/dating, unpaid balance and disclosure provenance support the promised noncircular routes without extra mandatory witnesses.", "content/schema review"),
    ("V63", 15, "R02 R28 R32 R37", "Stable cast in the city", "Install Mott's stable tier/duties and the existing seven-person cast with their normal homes/roles.", "No required ambient-only reference remains, cast knowledge is intentionally seeded and ordinary occupations continue around the incident.", "lore validation + sim"),
    ("V64", 16, "R11 R22 R37", "Early physical full case", "Recover the packet early; obtain origin, fracture match and the same wallet's dating, with no confession or surveillance.", "E01's sole continuous encounter and bound authenticated chain support original theft and assault; opportunity alone is insufficient.", "full content playthrough"),
    ("V65", 16, "R06 R11 R25", "Warin-only success", "Establish Averil/Gile's independent continued observation and cry relation, then stop investigating Corin.", "Warin can be formally cleared under the supported procedure, and the game recognises a legitimate partial achievement without inventing a culprit.", "full content playthrough"),
    ("V66", 16, "R10 R13 R25 R31 R33", "Public review while life continues", "Lodge partial/full bundles or leave NPCs to submit supported material while the player misses High Wick.", "The actual review can be limited, delayed or successful; only lodged evidence counts and later records report its real outcome.", "off-stage content playthrough"),
    ("V67", 17, "R08 R11 R23 R31", "Late physical recovery", "Miss retrieval/review; Corin refuses wallet comparison and the bargain, Lise is absent and the rack is empty.", "The logged fragment and retrievable wallet deposition permit comparison; its match plus Lise's prior identification of the particular desk and failed rack search permit scoped desk access before packet recovery.", "full content playthrough"),
    ("V68", 17, "R11 R21 R34 R38", "Private admission", "Confirm named acts and independently supplied detail; contrast an established leading leak with an unlearned secret leak.", "Only supported corroboration is certified; sealed delivery is actual, hidden histories do not become diagnostics and later established contamination can amend findings.", "full content playthrough"),
    ("V69", 17, "R06 R11 R24 R31", "Watch the moving packet", "Follow actual retrieval, warn Corin through a real recipient, lose sight, or observe a planted/handed-over similar bundle.", "Bounded strategy changes only from learned warnings; continuity and identity support handling at most unless further evidence establishes the original acts.", "spatial content playthrough"),
    ("V70", 17, "R20 R21 R26 R27", "Hush and sponsorship terms", "Contrast legitimate sealed restitution with a hush bundle withheld even from sealed institutional delivery; let another witness disclose and miss a sponsored report.", "Counterparties, real payments and finite terms remain distinct; third-party speech is not player breach, forbidden hush terms gain no automatic legal remedy, and sponsorship never becomes innocence or a renewed hold.", "full content playthrough"),
    ("V71", 17, "R02 R13 R23 R31", "Exploration and interruption", "Find the passage/packet first, read through events, interrupt a search and return after a day.", "Discovery is retained without reseeding, missed observations remain missed and at least one supported complete route still exists.", "slow/exploratory human + replay"),
    ("V72", 18, "R17 R18 R19 R20", "Consequences happen elsewhere", "Issue/amend orders while Corin, Warin and the player are in different districts; revisit after an office/day.", "Actual dispatch, custody, release and corrections occur or remain honestly blocked; learned records distinguish orders from completed arrests.", "longitudinal content replay"),
    ("V73", 18, "R09 R26 R27 R39", "Redemption and earned service", "Reject forged collection while the pledge remains unpaid, then supply genuine payment and redeem Warin's cargo favour twice.", "No free return follows forgery alone; authorised delivery conserves tools/funds and one-use service requires real permission/resources and fulfils once.", "content economy continuation"),
    ("V74", 18, "R07 R08 R20 R33 R36", "Aftermath and reopening", "Let gossip/night work run, change who learns the correction, revisit Lise's conditional passage grant/restriction and reopen after loading.", "Actual knowledge drives responses and scoped access; learned geography persists, and amendments do not reset evidence, money, actors or completed orders.", "multi-day continuation"),
    ("V75", 19, "R13 R14 R15 R16 R29 R36", "Whole delivery endurance", "Run representative complete paths over multiple game days and save before/after every decisive boundary.", "Full suites and fresh-process continuation pass; retention stays bounded without dropping referenced evidence, renewing duties or losing city state.", "release integration"),
    ("V76", 19, "R04 R12 R30 R31 R37", "Human investigation", "Observe five to eight formative testers across voice/keyboard, familiarity and slow/exploratory styles.", "They explain their reconstruction, distinguish opportunity from identification, understand pending actions and find the spatial/social work worth doing; concrete failures drive revision.", "human formative sessions"),
    ("V77", 19, "R07 R12 R21 R30 R34", "Live language boundaries", "Exercise holders/non-holders, evasions, specific admissions, false accusations and disclosure with live providers.", "Models neither invent physical results nor leak sealed history; deterministic confirmation/receipts preserve mechanics when prose varies or requests fail.", "live-provider evidence"),
    ("V78", 19, "R13 R15 R29 R32", "Performance and compatibility release", "Compare accepted reference runs at default/2,000 citizens and separate 20,000 stress, including saves, retirement and concurrent ordinary work.", "Agreed numeric time/memory budgets and declared behavior/content compatibility pass; known baseline stress cost is separated from added multiplicative cost.", "measurement + release audit"),
]


def milestone_files():
    result = {}
    for path in PLAN.glob("M[0-9]*_*.md"):
        match = re.match(r"M(\d+)_", path.name)
        assert match
        number = int(match.group(1))
        assert number not in result, f"Duplicate milestone M{number}"
        result[number] = path.name
    assert set(result) == set(range(20)), "Expected exactly M0 through M19"
    return result


def outputs():
    milestones = milestone_files()
    req_ids = {row[0] for row in REQUIREMENTS}
    assert len(req_ids) == len(REQUIREMENTS)
    seen_scenarios = set()
    coverage = defaultdict(list)
    by_milestone = defaultdict(list)
    scenario_values = []
    for sid, owner, covered, title, action, expected, kind in SCENARIOS:
        assert sid not in seen_scenarios
        seen_scenarios.add(sid)
        assert owner in milestones
        requirements = covered.split()
        assert set(requirements) <= req_ids, f"Unknown requirement in {sid}"
        assert action and expected and kind
        for rid in requirements:
            coverage[rid].append(sid)
        row = dict(id=sid, owner=f"M{owner}", requirements=requirements,
                   title=title, setup_and_action=action, acceptance=expected,
                   evidence_kind=kind, status="planned")
        by_milestone[owner].append(row)
        scenario_values.append(row)
    assert set(coverage) == req_ids, "Every requirement needs acceptance coverage"
    assert set(by_milestone) == set(milestones), "Every milestone needs a gate scenario"

    requirement_values = [dict(id=rid, decisions=decisions.split(), requirement=body,
                               scenarios=coverage[rid])
                          for rid, decisions, body in REQUIREMENTS]
    catalog = dict(schema=1, scope="Planning traceability only; no scenario has run by this catalog.",
                   milestones=[dict(id=f"M{n}", file=milestones[n],
                                    sequential_predecessor=f"M{n-1}" if n else None,
                                    scenarios=[v["id"] for v in by_milestone[n]])
                               for n in sorted(milestones)],
                   requirements=requirement_values, scenarios=scenario_values)

    req = ["Status: Planned acceptance coverage (2026-09-05); generated from evidence/plan_catalog.py.",
           "", "# Requirement traceability", "",
           "The seven [authoritative decisions](DECISIONS.md) become the requirements below. "
           "Coverage names planned acceptance scenarios, not completed tests. "
           "[VERIFICATION.md](VERIFICATION.md) gives each trigger and observable result. "
           "Current execution evidence is recorded separately in [REVIEW_LOG.md](REVIEW_LOG.md).", "",
           "| ID | Decision | Required result | Acceptance coverage |",
           "|---|---|---|---|"]
    for row in requirement_values:
        links = ", ".join(f"[{sid}](VERIFICATION.md#{sid.lower()})" for sid in row["scenarios"])
        req.append(f'| <a id="{row["id"].lower()}"></a>{row["id"]} | '
                   f'{", ".join(row["decisions"])} | {row["requirement"]} | {links} |')
    req.extend(["", "## Maintaining the record", "",
                "Edit the catalog source when requirements or acceptance ownership change, then run "
                "`uv run evidence/plan_catalog.py --write` from this plan directory. "
                "Run without `--write` to check that the generated files match. "
                "An executed scenario needs its actual command/content version/result in the milestone's "
                "acceptance record; changing this table never certifies implementation.", ""])

    verification = ["Status: All scenarios below are planned (2026-09-05); none is certified by generating this file.",
                    "", "# Verification by milestone", "",
                    "These are behavioral acceptance contracts for implementation. They deliberately include "
                    "adversarial and partial-result cases. Exact future test/CLI names are assigned when the "
                    "owning milestone implements them; this document does not advertise nonexistent commands.", "",
                    "The [checkpoint protocol](CHECKPOINT_PROTOCOL.md), [spatial proof protocol](SPATIAL_PROOF_PROTOCOL.md) "
                    "and [law protocol](LAW_PROTOCOL.md) supply the detailed cross-module invariants. "
                    "Strict continuation equality uses held/recorded completions with controlled timing; "
                    "intentional requeue of unfinished live requests instead proves conserved obligations and exactly-once effects.", "",
                    "Current baseline/tool results belong in [REVIEW_LOG.md](REVIEW_LOG.md). "
                    "Automated traces do not certify human enjoyment or a successful visual check.", ""]
    for owner in sorted(by_milestone):
        verification.extend([f"## [M{owner}]({milestones[owner]})", ""])
        for row in by_milestone[owner]:
            refs = ", ".join(f"[{rid}](REQUIREMENTS.md#{rid.lower()})" for rid in row["requirements"])
            verification.extend([f'<a id="{row["id"].lower()}"></a>',
                                 f'**{row["id"]} — {row["title"]}** · {row["evidence_kind"]} · {refs}', "",
                                 row["setup_and_action"], "",
                                 f'Acceptance: {row["acceptance"]}', ""])
    return {
        PLAN / "REQUIREMENTS.md": "\n".join(req),
        PLAN / "VERIFICATION.md": "\n".join(verification),
        HERE / "plan_catalog.json": json.dumps(catalog, indent=2) + "\n",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    generated = outputs()
    for path, value in generated.items():
        if args.write:
            path.write_text(value)
        else:
            assert path.exists() and path.read_text() == value, f"Regenerate {path.name}"
    print(json.dumps(dict(mode="write" if args.write else "check",
                          requirements=len(REQUIREMENTS), scenarios=len(SCENARIOS),
                          milestones=20, game_acceptance="not run by this tool")))


if __name__ == "__main__":
    main()
