Status: Planned (2026-09-05).

# M11 — Orders and lawful dispositions

Separate public accusation from institutional authority. A supported finding can create a scoped order; returning an object or cooling a rumour cannot silently erase an unrelated proceeding.

## Entry

M9 evidence/findings, M8 activities and M10 player feedback are accepted. Review the actual `notices.rs`, `custody.rs`, `actions.rs` and `Engine::tick_custody` behaviour before migration.

[LAW_PROTOCOL](LAW_PROTOCOL.md) specifies the cross-milestone record, briefing, coverage, station, release and migration contracts. This milestone owns legal declarations and defaults; M12 executes their physical work.

## Existing behaviour that must change

Current `WardNotice` combines an accusation, optional item, summons and warrant boolean. All broad law occupations carry every notice. A recent report raised by an officer can count as their own witnessed authority. Notices expire even when warranted. Settlement can erase a notice independently of physical release, and ordinary custody has short real-time/next-bell release ceilings.

These are a working base-game loop, not an evidence-backed institutional process. Keep its useful reports, notifications and custody machinery while replacing the inappropriate authority shortcuts.

## Records

Introduce stable matter/allegation, authority-decision, order and disposition records in a shared law module. Reference M9 submissions/findings rather than copying their evidence store.

An order records issuer/capability, supported grounds, subject or supported description, permitted action, place/search scope, issue time and review/expiry policy. Keep authority lifecycle/version separate from per-recipient delivery versions, assignment attempts and execution receipts. An order may be active with several failed attempts, or recalled while a stale delivery is in transit. Execution history remains after authority ends.

Represent the finite execution mandate separately, under [LAW_PROTOCOL's causal recall policy](LAW_PROTOCOL.md#finite-authority-and-causal-recall). Registry recall stops new issue/renewal; an already-issued grant remains usable by its named qualified recipient after actual receipt until their received recall or its original fixed cutoff. This includes a physical mandate already in transit. A received recall tombstone rejects later stale delivery. Do not silently stop a distant uninformed officer or create fresh authority from a generic order notice.

A custody disposition states the legal basis for holding/releasing a person, the keeper, review conditions and applicable obligations. One physical custody state references multiple valid hold grounds. Attaching a second ground cannot replace the escort or reset earlier deadlines. Discharging one matter cannot release a person who remains validly held under another; the interface must explain that distinction.

Declare stations with real intake/egress locations, permitted hold types, physical capacity including authored inmates, keeper coverage and review/release defaults. Current city-wide max-four new arrests is a separate safeguard, not station capacity. Define immediate watch intervention and written-cause delivery, legacy inmate grounds and ordinary debt/fee treatment before replacing the old helpers.

Separate restitution, recovery, evidential referral, physical custody, public reputation and final adjudication. The required procedure supports investigation, referral and bounded review/release. It does not claim to implement a complete criminal court or a sentencing simulation.

## Authority capabilities

Extend M6's common capability registry with legal issue/recall, enforcement, keeper and disposition roles; M7/M9 already supply examiner/reviewer roles. Author capabilities in data: report a wrong, receive a complaint, authenticate a document, record findings, issue/recall a scoped order, execute an order, keep a prisoner and execute release. The current broad `is_law` predicate is insufficient for these distinctions.

Mott can be given an explicit delegated Bench role for this procedure. Odo can authenticate papers and provide testimony without judging his own assault. Averil's revenue occupation does not automatically give her every arrest power. Existing station/keeper roles remain meaningful.

Validate authority at issue and validate the actual acquired mandate, capability, scope, received-version state, cutoff and execution history at execution. Resolve an explicitly named order/mandate directly; do not find the first order against a person and then reject a different valid requested one. Grant validity follows the declared procedure, not an omniscient remote-recall shortcut.

Register legal order/review/disposition adapters with M8 only now that their service exists. Complete the order-backed entry/search integration deferred from M6, using an actual scoped order and delivery receipt rather than an unchecked permission flag.

## Information and delivery

The institutional registry holds an order. An individual officer gains actionable knowledge through a modeled briefing, direct handoff or station/notice-board check. A public rumour can point to a record but is not the record's authority.

Initial institutional delivery uses ordinary station visits and explicit local handoffs. Declare the registry/check site, eligible roles and a shift-start/once-per-office check duty through M8 so a newly issued order has a guaranteed ordinary delivery opportunity when resources are available. If an urgent relay service is added, it must have a named available carrier and a travel/receipt operation. Do not solve reliability by granting every guard global knowledge immediately.

Durable orders survive ordinary gossip caps and cooling. Public notices can reference them, but deleting a chalk mark or losing a carried rumour cannot annul them.

A held rumour may still motivate a report or lower-authority summons under the shipped knowledge mechanic. Preserve its claimed/hearsay provenance. Such intake is not firsthand evidence, and an ignored hearsay summons must not automatically become search/arrest authority without the relevant independent decision. Keep an explicit regression fixture for this intentionally weaker social consequence.

## Disposition and correction

An accepted clearance creates a specific correction/release instruction. The keeper receives and executes it through ordinary activities. The earlier false accusation remains in history with its correction; the world does not pretend no harm occurred.

Every hold policy has a review deadline, supported extensions and a deterministic fallback if the reviewer is absent. Deliver that deadline/default with initial custody and every handover, so scheduled expiry needs no new remote message. An unscheduled early discharge requires actual service. Deferral does not extend detention. Distinguish the end of the relevant authority, received instruction, removal of restraint and usable physical egress. M6/M12 must supply exit assistance/relief without teleportation or a fictional renewed hold. Preserve prisoner feeding/watering.

An arrest order that has been executed is not an endless instruction to rearrest after lawful release. Breach of a later condition, a new matter or a new decision requires its own valid transition.

Preserve controller-safety behaviour for player grips separately from legal disposition. A stalled provider may end a physical grip safely, but should not determine the legal status of a case. Use deterministic review/release rules with a visible next step; no person is held forever because an LLM never took a turn.

Implement sponsorship/provisional release as a bounded procedure now: an eligible identified sponsor voluntarily accepts specific obligations, an authorised reviewer decides, and the keeper executes the resulting disposition. It changes hold conditions, not the truth of an allegation. Use M8 agreements for actual accepted obligations. Select alternate officers/keepers only from existing capability-qualified available actors; otherwise reschedule or report blockage.

Ship versioned legal declarations, role bindings, procedures and load validation here. M15–M18 only author the real case's participants and terms.

## Tests

- A repeated report cannot masquerade as firsthand observation or an issued order.
- Only an actor with the relevant capability can issue, recall, enforce or release under that scope.
- A named valid second order is usable even when another order names the same subject.
- Property return resolves the named recovery/restitution while an assault matter continues.
- Gossip expiry/cap pressure does not cancel institutional authority.
- Clearance and correction propagate to the appropriate keeper without clearing unrelated custody grounds.
- Executed/recalled orders cannot produce repeated arrest loops.
- Save/load retains decisions, delivery, status, review times and references without reseeding them.
- Legacy notices, authored inmates and ordinary petty-wrong flows have explicit reviewed migration tests.
- A stale issuance received after that recipient's recall receipt does not reactivate authority. An already-issued mandate still in transit when central recall occurs follows its original finite grant policy, without learning the remote change. One served recall among two hold grounds leaves only the other ground and its original deadline.
- Gate/night-watch intervention delivers prisoner and written cause by the canon's next-office deadline or follows its explicit fallback, with the investigation pack disabled.

## Completion gate

An independent dispute can produce, deliver, amend and discharge a lawful order with clear records. It does not yet claim the suspect has been caught. M12 receives a stable execution interface and ordinary citizens who have actually learned their duties.
