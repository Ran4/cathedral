Status: SPEC ONLY — unimplemented (2026-08-27)

STALE WHERE IT TOUCHES KNOWLEDGE (2026-08-30): `features/knowledge_and_rumor/` is being built
end-to-end **first**, on its own, and this spec will be rewritten against the API that actually
ships. Wherever the text below describes what the knowledge layer owns, when this quest may
start, or a casebook/receipt store of its own, `features/knowledge_and_rumor/README.md` wins.
Do not reconcile this file now.


# Quest: secure nine votes for a drainage-funding plan before the rain

Working title: **Nine Before Rain**

This is a major civic questline inside the larger game. It is **not** the game's premise, main loop, or complete
GDD.

The quest borrows the player's ordinary verbs and the city's ordinary simulation, gives them one five-day
problem, and leaves behind a changed city when it ends.

## One-sentence pitch

An authored downpour is five days away; First Seat Gude Dask asks an unknown but unentangled player to carry a
proposed drainage measure between Ombreval's sixteen benchers, assemble nine compatible conditional pledges,
and bring the motion to its legal hand-count, knowing that every funding base harms a different ward.

## Player-facing promise

The player is a political runner, investigator and fixer—not the person repairing the whole drainage network.

For each useful vote they must:

1. find a bencher on their real daily round;
2. learn what that person or ward needs;
3. do, prove, trade, expose, distort or suppress something concrete;
4. return before the relevant bell or bedtime;
5. secure a named, visible condition on the vote;
6. keep the draft compatible with nine such conditions until the hand-count.

Public works are one kind of leverage. Other routes include carried speech, testimony, accounting, items,
introductions, debt, marks, bribery, concealment, blackmail, curfew-breaking and civic fraud.

## Why this belongs in this game

The quest makes existing simulation state matter at the same time:

- sixteen canonical F.437 benchers already have incompatible material positions;
- nine agreeing votes already make an ordinary rule;
- NPCs already have homes, jobs, routes, needs, memories and changing locations;
- speech already reaches everyone within 20 m and may be remembered;
- items, sparks and two-sided offers already support payment, delivery, debt and bribery;
- chalk marks already alter code-side behavior and can be forged or scrubbed;
- notices, custody, surety, restitution and escape already provide non-combat consequences;
- weather, seven office bells and the Night Office already provide time and overnight change.

Without this quest those systems remain a sandbox. This feature adds a bounded reason to combine them, then
gets out of the way.

## Canon boundary

Canonical and already authored:

- the Drain Question is deadlocked in F.437;
- the three base proposals are a city-wide assessment, a Cut cart fee, and a Reed Postern barrow toll;
- sixteen benchers represent eight wards, and nine agreeing votes pass an ordinary rule;
- Gude Dask is First Seat and controls the docket but has no veto;
- each ward's interests and current benchers are fixed in `lore/core_lore/ward_politics.md`;
- the Common Bench, Common Clerk, Common Chest and Line-keeper are fixed in
  `lore/core_lore/secular_government.md`;
- wells, cisterns, drains, keepers, queues and repair disputes are fixed in `lore/wells_and_water.md`.

Quest additions, not retroactive lore canon:

- a deterministic `Downpour` begins exactly five days after the quest starts;
- Gude gives the assignment and the Common Clerk enters/issues a narrow runner's writ to the player;
- the measure may carry at most two riders from a fixed quest catalog;
- individual benchers expose explicit, machine-readable voting conditions;
- the player may earn deterministic receipts for authored civic tasks;
- the final policy produces one of four authored post-rain world-state packages.

The Common Clerk is an office, not a currently authored NPC. The quest must use the title until a holder is
authored. The runner role and writ are new quest content, not forgotten civic canon.

The quest does not solve the Second Sun, connect it to weather, invoke the Great Rains/Hammering, or imply that
the rain is supernatural. Its immediate damage stays local to the low streets and cellars that canonically flood
in hard rain; it is not an invented F.437 citywide catastrophe.

## Scope boundary: quest versus base game

### This feature owns

- the offer, activation and resolution of this one quest;
- its five-day clock and authored storm deadline;
- the player's runner's writ and civic docket;
- three base funding proposals and a small rider catalog;
- sixteen voter records and their authored conditions;
- eight ward knots and their quest-specific tasks/receipts;
- a final hand-count and four policy outcomes;
- quest-specific morning receipts, rumors and integrity consequences;
- the casebook projection for this quest;
- deterministic fake-backend and headless coverage for the complete quest.

### This feature consumes but does not own

- general player movement and interaction;
- speech, STT/TTS and typed chat;
- inventories, offers, shops, food and money;
- the law ladder, custody and Stone House;
- generic chalk marks;
- NPC rounds, navigation, attention and Night Office;
- weather presentation and bells;
- the larger game's save/load system, quest journal framework and overall progression.

The repository currently has no general save/load or quest framework. This quest may define the pure state and
serialization boundary it needs, but it must not hide an ad-hoc whole-game save system inside `drain_quest.rs`.
Until a base save feature exists, the quest is testable headlessly and playable only in one process lifetime.

Three advertised experiences are foundation dependencies, not current systems:

| Dependency | What exists now | What this quest requires |
|---|---|---|
| Persistence | Settings persistence only; no world/engine checkpoint. | A versioned whole-engine checkpoint. A quest-only save would restore votes while inventory, custody, marks, knowledge and NPC positions reset. |
| Political rumor | A feature design exists, but no runtime pollen propagation. | Bounded typed facts that travel without requiring LLM turns; a temporary quest queue is acceptable only through M2. |
| Player curfew | Snuffing changes NPC rounds, but there is no player curfew offence/detection mechanic. | Watch-witnessed proximity detection before any route promises curfew consequences. |

Weather is implemented and deterministic, but its current seeded timeline does not guarantee a promised storm
at one exact quest boundary. Nine Before Rain therefore owns an authored weather episode layered over that
timeline; quest resolution keys off game time, not an audible bell or sampled weather kind.

### Explicit non-goals

- making this the game's opening premise or mandatory main story;
- a generic procedural quest generator;
- a generic relationship/reputation-number system;
- free-form laws authored by the LLM;
- simulating every pipe, culvert volume or tax receipt;
- repairing the entire drainage network by hand;
- combat, health, skill trees or boss encounters;
- full building interiors;
- proving the Serle tun true or false in canon;
- producing a perfect compromise that harms nobody;
- resolving the origin or nature of the impossible light.

## Quest availability and entry

This is a midgame or early-midgame civic quest, available after the player has completed the base game's
movement/speech tutorial and the game can persist world state.

Activation predicates:

- the world year is F.437 and the Drain Question is unresolved;
- the quest has not already resolved;
- the calendar is late spring, shortly before Ambrellestide, so the authored voter deadlines align;
- no other world event owns the same downpour window;
- Gude Dask can give the assignment and the Common Clerk can enter the writ;
- the final hand-count can be scheduled five game days later at High Wick;
- the authored downpour can begin after Lamplight on that fifth day.

The player does **not** need ward standing. Their lack of ward, guild and creditor ties is why Gude can use them.
The writ gives permission to carry the draft, request a formal condition and inspect named public works. It does
not make the player a bencher, officer, voter, constable or person anybody must trust.

### Offer scene

At the Bellstand, Gude explains only what the player needs:

- the low streets and Bell-and-Sluice/Reed cellars flood in hard rain;
- three proposals are blocked;
- nine votes pass a rule;
- Gude may arrange the docket but may not manufacture agreement;
- the player may carry the Common Clerk's copy and return conditions;
- the hand-count will occur at High Wick on day five whether the player is ready or not.

Accepting gives:

- `quest_drain_writ`, a non-transferable quest document;
- `quest_drain_docket`, the diegetic casebook root;
- the three canonical base proposals;
- Gude as the first lead;
- a first appointment with one Wick or Bell-and-Sluice bencher.

Declining does not freeze the world. The hand-count still occurs without the player and defaults to deadlock
unless other simulation work eventually supplies a lawful majority.

## Completion and duration

- **Expected play time:** 4–6 hours.
- **World time:** five game days, from the offer to the High Wick hand-count.
- **Success condition:** at least nine valid `For` votes when the hand-count commits.
- **Failure condition:** fewer than nine valid `For` votes. This resolves to deadlock rather than a game-over.
- **Postcondition:** the authored downpour occurs and the selected policy—or deadlock—becomes visible world
  state.
- **Continuation:** the player keeps playing on the same save after every outcome.

The quest never waits for the player. Missing the vote, being in custody, or refusing to attend still produces an
outcome from the valid recorded votes.

## The player's daily loop

Each day supports one or two serious political leads:

1. **Read the city.** Check the forecast, bells, docket, overnight withdrawals and public changes.
2. **Choose.** Select a bencher, condition, rider or piece of leverage worth pursuing.
3. **Find.** Learn the person's schedule and reach their actual current location.
4. **Listen.** Ask what would move them and who else must witness it.
5. **Act.** Carry, inspect, trade, prove, expose, mark, bribe, trespass or arrange a meeting.
6. **Return.** Bring the result back before the person's next leg or bedtime.
7. **Record.** Receive an explicit conditional pledge or a refusal with a legible next lead.
8. **Choose the night.** Return to a safe roof, land a last word, or exploit curfew at legal risk.
9. **Receive the morning.** Learn what the Night Office, rumor propagation and changed draft did to the coalition.

The player should always be able to name at least one next lead and why it matters. They need not always know
whether following it is wise.

## Five-day dramatic structure

| Day | Quest beat | Expected coalition | New pressure |
|---|---|---:|---|
| 1 — The Writ | Learn one ward knot, perform one witnessed task, obtain the first seal. | 1–2 | Being unknown; learning office time and the docket. |
| 2 — The Price | Conditions begin to conflict; introduce riders and a second route to each task. | 3–4 | Work versus travel; clean route versus debt. |
| 3 — The Turn | The first player claim returns as rumor; one pledge may withdraw with a receipt. | 5–6 | Overnight memory and public interpretation. |
| 4 — The Shortfall | Honest obvious routes no longer cover every missing vote. | 7–8 | Bribery, leverage, forged marks, curfew and custody. |
| 5 — The Hand-count | Last words, final draft, invalidated seals, public vote, then rain. | 9+ or deadlock | The docket closes at High Wick; no hidden grace period. |

This structure is authored. The exact bencher order is not. Schedules, arrests, player choices and the draft
determine the route through it.

## Proposal model

The player edits the draft through the docket. Free speech may negotiate the contents but never silently mutates
the legal measure.

### Base proposals

| ID | Public effect | Primary opposition | Default visible consequence |
|---|---|---|---|
| `city_assessment` | Every taxable household contributes to low-street drainage work. | Wallwright; poor households without an exemption. | Household purses fall; broad repair crews appear; Wallwright cooperation cools. |
| `cut_cart_fee` | Freight using the Cut cartway pays for the culvert work. | Cloth, carriers and market trades. | Cart queues grow; cloth/food prices rise; Cut enforcement increases. |
| `reed_postern_toll` | Reed Postern barrows fund the work. | Reed, fish runs, poor carriers and wharf households. | Gate queues grow; fish arrives later; smuggling and postern patrols increase. |

Each base funds the minimum repair package on its own. This quest is not a budget spreadsheet.

### Fixed rider catalog

The draft may contain zero, one or two riders. Riders are authored legal clauses, not LLM text.

| ID | Compatible base | Effect / prerequisite |
|---|---|---|
| `poor_house_exemption` | `city_assessment` | Exempts pauper/no-fixed-trade households; shifts support toward low wards and away from strict revenue voters. |
| `chapter_composition` | `city_assessment` | The Chapter voluntarily pays for pilgrim street wear; requires a witnessed Chapter commitment. The Bench cannot compel it. |
| `sunset_clause` | `cut_cart_fee` | Ends the fee after the repair term; reduces long-term opposition but does not erase immediate prices. |
| `capped_freight_loads` | `cut_cart_fee` | Caps assessment per load; helps honest bulk carriers and creates an incentive to split manifests. |
| `handbarrow_exemption` | `reed_postern_toll` | Preserves the poor man's door for defined hand loads; creates a classification/enforcement problem. |
| `public_tun_audit` | any | Requires a public proving at the next Gaudry's Audit; it never canonically settles whether the tun is honest. |
| `sealed_fire_reserve` | any | Dedicates part of the repair work to sealed cistern fire reserves; buys Cinder support and angers current queues. |
| `step_keeper_from_chest` | any | Pays the Step Cistern keeper from the Common Chest; helps Bell-and-Sluice and consumes visible civic money. |

The rider catalog should remain small. If playtesting needs more combinations, add authored riders; do not let
the model write arbitrary law.

### Draft revision

Every edit increments `draft_revision`. A pledge records the revision and its condition predicates. After an
edit, the sim immediately re-evaluates all pledges:

- still satisfied: remains sealed;
- temporarily incompatible: shown grey with the broken predicate;
- impossible under the new base: withdrawn with a deterministic reason;
- requiring an unfulfilled external commitment: pending, never counted.

There is no invisible persuasion total and no off-screen random vote flip.

## Voter roster and ward knots

The active Bench is the fourteen `bn*` character sheets plus Gude Dask and Aubin Marle. Betriss Clove and
Osanne Stott keep the courtesy title from former service but are viewers, not current voters.

All fourteen `bn*` benchers and Gude are currently Minor actors. Aubin Marle is currently Ambient. Because a
named decisive voter needs authored cognition, quest knowledge and individual handling, implementation must
promote/deepen Aubin before shipping rather than quietly treating an Ambient actor as a Major quest subject.

Every ward knot has one public material dispute, two current voters, at least one clean route, one compromise
route and one unlawful or personally costly route. A route creates receipts and facts; it does not award generic
`+10 reputation`.

| Ward | Current voters | Public knot | Examples of authored leverage |
|---|---|---|---|
| Fabric | Segwin Hobbe (`bn3sg`); Clemence Pike (`bn4cp`) | Pilgrim traffic wears civic streets while consecrated ground pays no ward tax; unlicensed beds undercut licensed lodging. | Count feast wear; secure a voluntary Chapter composition; inspect illegal upper rooms; threaten Segwin's Chapter cartage or Clemence's competitors. |
| Wick | Jonet Kett (`bn5jk`); Gile Bram (`bn6gb`) | Households queue while Slate Cistern keeps a third sealed for fire; Cinder's furnace rules threaten food/tallow trades. | Operate Slate's first-wash board and make a witnessed tank/queue count; add a seasonal lower fire-line rider; inspect a covered hearth and fire buckets with a Cinder witness. The seal is not canonically cracked. |
| Cloth | Averil Nett (`bn7an`); Jos Mere (`bn8jm`) | A Cut fee tolls every bale twice; Cloth is blamed for foul drain return. | Trace Tenter Cistern's turned lead elbow and controlled outfall; kill/cap the cart fee; obtain a mismatched manifest and expose or exploit the ambiguity. Do not canonize Jos as its forger. |
| Wallwright | Ewart Toll (`bn9et`); Bertran Crake (`bnbcr`) | High ground rejects a city assessment; Lodge Well's public shaft and lodge windlass split responsibility. | Obtain a witnessed allocation of shaft versus windlass costs; add beneficiary funding/high-ground exemption; recover evidence and secure Bertran a lawful hearing date, never a promised verdict. Lodge Well is not the moving head. |
| Cinder | Petronel of the Row (`bnpro`); Dunstan Hook (`bndhk`) | Cinder wants sealed reserves, inspected furnaces, Three-Curb's moving head repaired and counted fire gear. | Carry the three courts' emergency petition; order lifting/resetting before the debt hearing; complete the hook/ladder/bucket tally; add the fire-reserve rider. |
| Weigh | Rohese Sedge (`bnrse`); Aubin Wren (`bnawr`) | Weigh wants the postern tolled and its measures trusted; Aubin must recuse on the tun. | Add a publicly accounted barrow toll; arrange a neutral proving procedure; omit/separate the tun clause so Aubin can recuse cleanly. The quest never returns the tun's truth. |
| Reed | Idonea Tarn (`bn1id`); Hamel of the Reach (`bn2hm`) | Reed defends long postern hours and no handcart toll; it distrusts the Serle tun. | Run a timed fish barrow; witness gate delay; add the handbarrow exemption; promise a neutral public proving procedure. |
| Bell-and-Sluice | Gude Dask (`p009p`); Aubin Marle (`p009s`) | The poorest ward wants Step Cistern's keeper paid by the Common Chest and lower streets repaired. | Use Gude's two-different-ward expenditure warrants as the procedure tutorial; cost the keeper stipend; carry an audited household count; disclose/remove any Marle interest so Aubin can vote cleanly. |

### Per-voter condition brief

These are the initial clean/public condition sets. Data may also expose explicitly marked compromise and
coercive variants, but may not reverse a sheet's public position merely to complete a coalition.

| Voter | Initial authored conditions to expose |
|---|---|
| Segwin Hobbe | Witnessed voluntary Chapter composition; Line-keeper/ward-hand count of pilgrim and works-cart wear. |
| Clemence Pike | Lawful inspection of unlicensed upper rooms; Chapter composition or equivalent lodging/street-wear contribution. |
| Jonet Kett | Witnessed first-wash/tank/queue count; seasonal lower sealed fire line. |
| Gile Bram | Covered-hearth/fire-bucket inspection with a Cinder witness; rider distinguishing covered hearths from high-risk furnace/render premises. |
| Averil Nett | No Cut cart fee; traced Tenter elbow/outfall with costs assigned to the real defect rather than blanket Cloth blame. |
| Jos Mere | No Cut cart fee. A mismatched manifest may enable leverage, but his personal authorship/guilt remains unresolved. |
| Ewart Toll | Beneficiary/user funding or high-ground exemption; witnessed allocation of Lodge Well's civic shaft and lodge windlass costs. |
| Bertran Crake | Beneficiary-street funding/high-ground exemption; deed/witness and a lawful date for his Hammering claim hearing, never a promised judgment. |
| Petronel of the Row | Three courts' emergency petition and lift/reset order for Three-Curb before the debt hearing; fire-reserve/inspection rider. |
| Dunstan Hook | Completed citywide hook/ladder/bucket tally; spending earmarked only for documented deficits and sealed fire reserves. |
| Rohese Sedge | Publicly accounted Reed Postern barrow toll; neutral proving procedure that protects Weigh's public exactness. |
| Aubin Wren | Barrow-toll motion with the tun clause omitted/separated; neutral prover so his mandatory tun recusal is clean. |
| Idonea Tarn | Long postern hours plus handbarrow/fish/funeral exemption; witnessed lawful fish/barrow journey showing delay. |
| Hamel of the Reach | Independent public Gaudry proving procedure; long hours and handcart access preserved. |
| Gude Dask | Two different-ward signatures on the separate expenditure warrants as procedure tutorial; an authored cross-ward/Step-keeper condition for her ordinary vote. |
| Aubin Marle | Audited household count and costed Step keeper stipend; any Marle interest disclosed and removed before he votes. |

Detailed per-voter conditions belong in data, not hard-coded match arms. The table above is the content brief,
not permission to contradict a character sheet.

Direct personal favors in exchange for votes are corruption/coercion routes, not the neutral default. The clean
routes produce public evidence, lawful inspections, hearings, voluntary composition, transparent accounting or
measure riders. Preserve canonically unresolved matters—especially the Serle tun and wharf-born residency.

## Conditional vote rules

### What a pledge means

A pledge is a state transition, not sentiment:

```text
voter + motion + draft_revision + authored condition ids + evidence snapshot -> Pledged
```

It counts only when:

- the voter is a current bencher and has not recused;
- the quest is active and the docket is open;
- the draft is legally valid;
- every hard condition is currently true;
- the pledge was made explicitly by that voter;
- no deterministic withdrawal fact has fired since the pledge.

Friendly speech without `pledge_vote` is not a vote. Hostile speech does not prevent a vote whose conditions the
actor deliberately accepted.

The seal/pledge is quest UI for declared intent; it is not the legal act. The finale always performs an actual
public hand-count. First Seat may arrange the business and casts an ordinary bencher's vote, but has no veto or
unilateral lawmaking power. Nine are both quorum and the required agreeing count.

### Actor/simulation contract

The LLM owns:

- the wording and performance of the negotiation;
- whether it trusts a claim not yet mechanically proved;
- which authored condition it chooses from its currently available set;
- refusal, anger, mercy, gossip and interpretation;
- optional suggestions and introductions.

The sim owns:

- who is legally a voter;
- current draft and rider legality;
- task, item, place, time and witness receipts;
- recusal and conflicts of interest;
- condition availability and truth;
- pledge validity, withdrawal reasons and the final arithmetic;
- quest timing, outcome and post-rain state.

The model may say no. It may never invent a seventeenth voter, a fourth base proposal, a hidden rider, a tax
amount, a completed task, a vote count or a deadline extension.

### Proposed action seams

Add only typed actions whose arguments are validated against quest state:

```text
propose_vote_condition {"condition_id":"..."}
pledge_vote            {"condition_id":"...","draft_revision":7}
withdraw_vote          {"reason_id":"..."}
record_testimony       {"fact_id":"...","person":"..."}
```

`withdraw_vote` is available only when the sim exposes an authored/current reason. A draft edit may invalidate a
pledge without waiting for cognition; the next actor turn performs the reaction, not the state transition.

## Deterministic quest state

The authoritative state belongs in `cathedral-sim`, because the deadline, fake backend and final vote must be
deterministic and headless. Suggested shape:

```rust
struct DrainQuest {
    phase: DrainQuestPhase,
    accepted_at: Option<WorldTime>,
    hand_count_at: WorldTime,
    downpour_at: WorldTime,
    draft: DrainDraft,
    draft_revision: u32,
    voters: BTreeMap<ActorId, DrainVoterState>,
    knots: BTreeMap<WardId, WardKnotState>,
    tasks: BTreeMap<QuestTaskId, QuestTaskState>,
    receipts: Vec<QuestReceipt>,
    known_leads: BTreeSet<QuestLeadId>,
    integrity: DrainIntegrityLedger,
    outcome: Option<DrainOutcome>,
}
```

Suggested phase machine:

```text
Dormant -> Offered -> Active -> DocketClosed -> HandCount -> Resolved
                  \-> Declined ----------------------------^ (default deadlock)
```

All collections that enter snapshots, prompts or fixtures use stable ordering. No `HashMap` order may leak into
goldens.

### Receipts

Receipts are append-only facts with stable ids. Examples:

- item delivered by actor A to actor B before the Waning;
- player operated fixture X during an allowed office;
- named witnesses were within the event's real perception radius;
- a public mark existed, was scrubbed, or was proved forged;
- a message was handed to the player and a later transcript was judged faithful/distorted;
- a Chapter commitment, surety, bribe or notice was accepted;
- a voter pledged, recused or withdrew for a named reason.

Receipts are not all shown. The casebook projects only what the player has learned or directly caused.

## Civic tasks: embodied proof, not repair grinding

The quest needs a small generic interaction seam for authored public-work fixtures. It does not need a repair
crafting game.

Task kinds:

- `CarryBetween`: possess/carry a specified stack or prop between two anchors before an office;
- `OperateFixture`: wind, lift, sound, clear or brace at a named fixture for a bounded duration;
- `InspectWith`: use a required tool/item at an anchor and emit an observation;
- `DeliverTo`: complete a two-sided item handoff to a specified person or occupation;
- `AssembleWitnesses`: have required people present when a domain event occurs;
- `FollowAndObserve`: remain close enough through a route segment and witness a delay/obstruction;
- `RecordPublicCount`: visit a fixed set of fixtures and return the tally before a bell.

Every task specifies:

- start/finish anchors;
- required item or empty-hand state;
- allowed offices;
- completion predicate;
- relevant witness policy;
- interruption/lapse behavior;
- receipt(s) emitted;
- one terse in-world prompt and one accessible text fallback.

No rapid-input QTE, hidden dice roll or model-authored completion is permitted. The player's skill is choosing the
route, timing, witnesses, inventory and social framing.

## Casebook / docket

The quest gets one diegetic status surface. It is a projection of known state, not omniscience.

Always visible after acceptance:

- time/office of the final hand-count;
- forecast hard-rain window;
- current base and riders;
- number of currently valid pledged votes;
- known invalid/withdrawn votes and named reasons;
- the latest one or two leads the player chose to pin.

Per voter, once learned:

- name, ward and current public stance;
- last known schedule/location clue, not live wallhacks;
- offered condition;
- fulfilled/missing predicate;
- pledge state and draft revision;
- withdrawal/recusal receipt.

The docket must be fully usable with typed chat and screen-reader-friendly text. Voice is expressive, never a
critical accessibility gate.

## Rumor and Night Office integration

Full `Rumor Pollen` is a preferred integration, not an excuse to block M0–M2.

Quest facts eligible to become bounded rumor tokens:

- a public pledge or withdrawal;
- a large bribe, exposed forgery or custody commit;
- a manipulated public proving;
- the player publicly contradicting their own carried words;
- a named ward service performed before witnesses.

Until generic rumor propagation exists, M2 may use an authored quest fact queue that transfers only at fixed
public gatherings and the Night Office. Replace it rather than maintaining two permanent rumor systems.

The Night Office may:

- settle a witnessed event into a Major bencher's memory;
- expose one morning reaction/receipt;
- carry the hottest relevant fact into a ward mood;
- cause a lawful withdrawal only when a condition or authored trust predicate changed.

It may not roll votes randomly or silently.

Today, every `bn*` bencher and Gude is Minor, while Aubin Marle is Ambient; the Night Office does not give them
sixteen dependable individual reflections. Mandatory predicate reevaluation and dawn withdrawal receipts must
therefore run deterministically even with the Night Office disabled or all night work dropped. Night cognition
may color memory, ward mood and performance only.

## Law and custody integration

The quest creates opportunities for existing law, not a parallel punishment system.

Potential offences:

- bribing a bencher or falsifying Bench business;
- forging/scrubbing a civic mark;
- toll fraud or manifest fraud;
- trespass after curfew, once the missing player-curfew witness mechanic exists;
- false peals;
- stealing or concealing a weight, writ or public tally;
- breaking custody.

Arrest is fail-forward:

- the quest clock continues;
- benchers may visit or send word to the Stone House;
- surety, restitution, serving time and escape remain valid routes;
- the player can lose offices and votes but never receives a generic quest-failed screen;
- the hand-count proceeds in their absence.

## Writ rules

The writ is a quest capability, not a universal key.

It permits the player to:

- carry the Common Clerk's draft;
- request a formal condition from a current bencher;
- inspect only fixtures named by an active quest task;
- ask a ward hand or keeper for lawful access while on that task.

It does not permit:

- entry into arbitrary private interiors;
- ordering citizens or officers;
- taking public items without a receipt;
- ignoring queues, curfew, tolls or custody;
- voting;
- speaking for Gude, the Common Clerk or the Bench.

If stolen, destroyed or surrendered, the legal record remains at the Bench House but the player loses portable
access until Gude/Common Clerk reissues it or a forged replacement is accepted.

## Final hand-count

At High Wick on day five:

1. the docket closes and receives a final `draft_revision`;
2. all conditions are evaluated in stable voter-id order;
3. recused voters are shown separately;
4. each valid bencher vote is committed exactly once;
5. nine `For` votes pass; anything below nine deadlocks;
6. the result emits a citywide domain event and bell/crier presentation;
7. emergency authority, closures, clearing/shoring and any expenditure warrants are scheduled before the
   downpour; long-term culvert work is not completed instantly;
8. the quest resolves after the first morning receipt following the rain.

The public scene may use LLM speech for arguments and reaction, but the hand-count cannot wait indefinitely for
sixteen provider calls. The sim commits the arithmetic; available actors voice a bounded selection around it.

The funding rule and an expenditure warrant are separate instruments. Nine agreeing votes pass the ordinary
rule. Major spending still needs signatures from benchers of two different wards. Gude's existing warrant errand
is the procedural tutorial and a post-vote execution gate; it never substitutes for the nine-vote hand-count.

## Outcomes

### Assessment passes

- repair crews appear broadly in low streets;
- taxable household purses/settlement reflect the assessment;
- poor households are spared only if that rider passed;
- Wallwright and any broken assessment promises carry resentment/memory;
- Chapter participation is visible only if its voluntary commitment was secured.

### Cut fee passes

- toll/enforcement anchors appear on the Cut;
- cart routes gain delay and some market costs increase by authored values;
- sunset/cap riders change the rule visibly;
- manifest splitting and evasion become follow-on play;
- Cloth reactions reflect which concessions or betrayals occurred.

### Reed Postern toll passes

- the postern queue and watch behavior change;
- fish/handbarrow timing and selected prices change by authored values;
- exemption classification becomes a follow-on enforcement problem;
- Reed smuggling/evasion grows rather than disappearing;
- the public tun audit is scheduled if attached.

### Deadlock

- the authored downpour floods selected low-street/cellar anchors and closes selected routes;
- affected wells/cisterns receive temporary availability/contamination state;
- emergency work and orders replace the planned repair;
- ward moods and memories assign blame from actual quest facts;
- the Drain Question remains available as a later civic problem, not an identical immediate replay.

### Integrity overlay

The policy outcome and the player's method are orthogonal. Track authored facts, not morality points:

- promises kept/broken;
- bribes offered/accepted/exposed;
- forged evidence/marks used and later discovered;
- carried words faithfully delivered or altered;
- lawful notices settled or evaded;
- people materially harmed/helped by task choices.

The overlay affects reactions, law and follow-up content. It never changes the nine-vote arithmetic after the
hand-count.

## Failure matrix

| Failure | Immediate result | New route |
|---|---|---|
| Miss a bencher before their next leg | No conversation/pledge now. | Learn tomorrow's route, intercept at a public gathering, or use an intermediary. |
| Change the draft and invalidate seals | Seals grey with broken predicates. | Restore the clause, renegotiate, or replace the voter. |
| Lose/consume required evidence | Task receipt remains incomplete. | Borrow, buy, steal, substitute testimony, or expose the loss honestly. |
| Lie and get caught | Trust predicate/rumor/law fact changes. | Use leverage, restitution, unlawful contacts or another coalition. |
| Bencher recuses | Their vote cannot count on that motion. | Change rider/base or find a different ninth vote; recusal is never persuadable. |
| Get arrested | Offices pass while custody play continues. | Surety, fee, service, escape, prison messenger or accept a smaller coalition. |
| Miss the hand-count | Valid recorded votes are counted without the player. | Play the resulting policy/deadlock and its aftermath. |
| Provider/STT/TTS failure | Performance degrades. | Typed prompts, deterministic action fallback and casebook remain sufficient. |

## Prompt surface

Only current benchers receive the quest section, and only while the quest is active/recently resolved.

Suggested rendered block:

```text
**drain_motion**:
- hand_count: Day 5, High Wick
- current_draft: cut_cart_fee + sunset_clause
- draft_revision: 7
- your_legal_status: current bencher for Cloth Ward
- your_public_position: oppose a Cut fee
- conditions_you_may_offer_now:
  - cloth_outfall_traced
  - capped_freight_loads
- receipts_you_know:
  - player delivered the Tallage manifest before the Waning
- your_recorded_vote: none
```

Do not inject all sixteen voters, all tasks or hidden receipts into every prompt.

## Data ownership

Prefer data for quest content and Rust for rules.

Suggested files:

```text
assets/world/drain_quest.json # catalog: timing, voters, predicates, tasks, fixtures, strings
```

Suggested code:

```text
crates/cathedral-sim/src/drain_quest.rs
src/smart_actors/quest_ui.rs
src/smart_actors/quest_interaction.rs
```

The host loads/validates the asset and passes plain catalog values into the IO-free sim. Mutable `DrainQuestState`
belongs on `World`, not only `Engine`, because NPC actions receive `&mut World`. Validate exactly sixteen unique
current voter ids and every condition, fact, fixture, item, place and operation reference at construction. With
no catalog, old prompt/snapshot behavior remains byte-identical. If a generic quest module exists by then, use
it rather than preserving these provisional paths.

## Engine and projection seams

Likely new player commands:

- `AcceptQuest { quest_id }`
- `PlayerSetDrainDraft { request_id, base_id, rider_ids }`
- `PinQuestLead { lead_id }`
- `PlayerWork { request_id, fixture_id, operation_id, item_id, position_m, spatial_seq }`
- `SleepAtRoof { door_id }` only if the base game still lacks a sleep seam and this dependency is approved.

Likely new messages/snapshot projections:

- `EngineMessage::DrainQuest(DrainQuestView)` on activation and whenever a dedicated monotonic quest revision
  changes;
- small request-result/receipt messages for immediate feedback;
- quest work progress only when it changes, not every frame.

Do not place the casebook inside the actor/item `PublicSnapshot` or touch the whole public-state revision for
each lead; that republishes the full cast and configured crowd. Project `DrainQuestView` into a dedicated Bevy
`QuestCasebookState`. The Bevy side renders player-safe typed state and does not re-derive vote legality or count
votes. Hidden trust, private objections, unknown receipts and secret facts never enter the view.

## Milestones

Each milestone is independently playable and testable.

### M0 — Pure quest state and one clock

- Add the validated catalog and `DrainQuestState` on the pure sim's `World`, behind an absent/default-off data
  gate.
- Implement offer/accept/decline, five-day deadline, authored weather boundary and idempotent phase transitions.
- Implement three bases, draft revisions and final deterministic hand-count with seeded voter fixtures.
- Publish a minimal snapshot/diagnostic; no LLM actions or polished UI.
- Headless test: accept, advance five days, observe default deadlock and resolution.
- Quest-disabled prompts/snapshots remain byte-identical.

### M1 — Four-voter vertical slice

- Reed + Cloth only: Idonea Tarn, Hamel of the Reach, Averil Nett and Jos Mere.
- One base motion (`cut_cart_fee`), one rider, four authored conditions.
- Add `pledge_vote`, invalidation receipts and a four-person mini hand-count requiring three.
- Add one spoken-message task, one timed barrow task and one mark route.
- The result changes one Cut enforcement behavior the next morning.
- This is the go/no-go test for the whole feature.

### M2 — Docket and all sixteen voters

- Add all three bases, fixed rider catalog and sixteen voter records.
- Implement the quest casebook/docket and accessible text controls.
- Add the eight ward knots with at least two routes each.
- Add deterministic fake-backend quest decisions.
- Validate that at least three distinct nine-vote coalitions exist.

### M3 — Consequence routes

- Integrate quest facts with Night Office morning receipts.
- Integrate bounded rumor propagation or the temporary authored fact queue.
- Exercise existing law/custody with at least one route through arrest, surety and escape.
- Implement explicit withdrawal/recusal presentation.
- Add the quest integrity ledger.

Correctness must pass with Night Office disabled/dropped. Player curfew is not promised until watch-witnessed
detection exists.

### M4 — Hand-count, rain and aftermath

- Build the bounded public hand-count presentation.
- Add the deterministic quest weather overlay and test time jumps across multiple offices/days.
- Apply four post-rain outcome packages and rider variations.
- Ensure the same world continues after resolution.
- Add follow-up lines/tasks that make the new rule legible without an ending card.

### M5 — Persistence foundation

- Add explicit, versioned engine/world checkpoint DTOs; the sim produces/consumes values and the host performs
  atomic file IO.
- Preserve clock/boundary cursors, world, rounds, inventory, marks, law/custody, knowledge, rumor and quest
  state together; intentionally discard/restart in-flight cognition and speech work.
- Round-trip and corrupt/version-rejection tests prove no receipt, pledge, storm transition, hand-count or rumor
  duplicates after load.
- Capture a checkpoint immediately before each explicit pledge mutation and at every office boundary. Document
  whether disk durability is same-poll or requires a two-phase host acknowledgement.

### M6 — Full content, polish and ship

- Author all sixteen voters/eight ward knots and promote/deepen Aubin Marle.
- Voice/typed accessibility parity and provider-failure fallback.
- Audio/visual receipts: seals, bell/crier, forecast, queues, work crews and flooded anchors.
- Balance travel/office windows for 4–6 hours.
- Performance test with authored cast and configured crowds.
- Complete headless, Bevy-host and one real drive-mode acceptance run with `CATHEDRAL_HEADLESS=1`.

## Acceptance criteria

### Deterministic sim

- A fixed seed and command transcript produce byte-stable quest state and vote result.
- Nine valid votes pass; eight do not; recused/invalid votes never count.
- Editing a draft invalidates/restores the correct seals immediately and reports why.
- The hand-count fires exactly once at the authored game-time boundary even if bell audio is disabled, the
  player is absent/in custody, or one pump crosses several offices.
- The authored downpour and resolution fire exactly once, independently of developer weather presentation
  overrides.
- A failed LLM call cannot delete a pledge, receipt, deadline or outcome.
- Stable ordering holds across voters, tasks, receipts, prompts and snapshots.

### Content

- Every current bencher has at least two authored conditions compatible with their sheet.
- At least three materially different nine-vote coalitions are valid.
- No coalition requires one exact model phrase or microphone recognition.
- No single item, task or NPC failure hard-locks the quest.
- The public tun remains unresolved in every ending.
- No outcome is presented as the morally correct solution.

### Player comprehension

- After acceptance, a first-time player can state the goal, count and deadline.
- At any ordinary point before docket close, the casebook exposes at least one actionable learned lead.
- A grey/withdrawn seal names the broken condition.
- The player can distinguish proposal effects before committing the draft.
- The morning after the vote visibly communicates which policy passed without relying on prose alone.

### Integration

- Typed chat can complete every critical path without STT/TTS.
- Fake backend can complete at least one clean and one dirty route.
- Custody consumes time but does not freeze the quest.
- Marks, offers, items and schedules remain authoritative; quest code does not duplicate them.
- No Bevy-only state determines a vote or outcome.
- Quest correctness holds with Night Office disabled and every scheduled night reflection dropped.
- Whole-engine checkpoint round trips do not duplicate or lose quest/world consequences.

## Vertical-slice acceptance scenario

Two game days, Reed and Cloth, four voters:

1. Gude issues the writ and a `cut_cart_fee` draft.
2. Idonea refuses unless handbarrows are exempted.
3. Hamel asks for a neutral public proving procedure and a carried message; the slice never reveals the tun's
   truth.
4. Averil wants the fee dead or capped after an outfall inspection.
5. Jos offers help if the player returns one manifest unopened; exposing a mismatch supplies a different route
   without canonizing who falsified it.
6. A timed barrow job shows the postern delay.
7. One spoken claim crosses two wards by the next morning.
8. Night travel creates a legal/illegal route split through an existing offence; use curfew only after the
   watch-witnessed player-curfew dependency exists.
9. One route may enter the Stone House and still reach the vote.
10. A three-of-four mini hand-count changes Cut enforcement the next day.

If this is not fun and legible at four voters, do not author the remaining twelve.

## Risks and decisions still required

1. **Quest length.** Five one-hour game days implies up to five real hours before optional sleep skips. Playtest
   whether this is satisfying or exhausting; do not change the deadline before the vertical slice.
2. **Authority fantasy.** The writ must open a narrow civic conversation without making an unknown player
   implausibly powerful. NPC prompt copy and access checks carry this risk.
3. **Task feel.** `OperateFixture` must feel embodied, not like holding E on eight reskinned progress bars. The
   route/time/witness choices must carry the play.
4. **Coalition legibility.** Conditional seals can become spreadsheet UI. Keep predicates concrete and named in
   ordinary language.
5. **LLM latency.** Sixteen serial conversations cannot be required during the public vote. Commit in code and
   voice only a bounded dramatic subset.
6. **Speech occlusion.** Hearing currently ignores walls. Do not build critical eavesdropping around closed rooms
   until perception gains occlusion; use squares, passages and open stalls.
7. **Persistence.** The quest is too long to ship responsibly without base save/load. Keep sim state serializable
   and treat disk persistence as a hard ship dependency.
8. **Rumor dependency.** Full walking-speed rumor is desirable but not M1-critical. The temporary queue must be
   designed for deletion.
9. **Weather ownership.** The quest must reserve or discover a deterministic storm window without fighting a
   future general weather calendar.
10. **Ambient public office.** Aubin Marle is a named current voter but presently Ambient. Promote and author
    him before full content; do not rely on ambient prompting or Night Office behavior.
11. **Immediate versus long-term work.** A vote just before rain can authorize clearing, shoring, closures,
    labor and warrants. Presentation must not imply that decades-old culverts are rebuilt overnight.

## Open design questions

- Which publicly defensible condition earns Gude's ordinary Bell-and-Sluice vote, without inventing a First
  Seat veto or neutrality rule?
- May a voter condition refer to another voter's public pledge, or only to draft/task/world facts?
- Should the player be able to submit a knowingly illegal draft and force the Common Clerk to reject it before
  the hand-count?
- Which two unlawful routes belong in M1: manifest theft, forged mark, bribery, or curfew trespass?
- Is `public_tun_audit` compatible with every base, or only a Postern-toll compromise?
- How are market price changes capped so the quest consequence is visible without destabilizing the entire food
  economy?
- Which base-game feature owns the safe roof and sleep-to-Kindling interaction?
- Does a pre-pledge autosave mean an in-memory checkpoint written in the same poll, or crash-durable two-phase
  acknowledgement before the mutation commits?

## Source references

- `lore/core_lore/ward_politics.md` — wards, current benchers and the Drain Question.
- `lore/core_lore/secular_government.md` — sixteen seats, nine votes, Bench officers and jurisdiction.
- `lore/wells_and_water.md` — water infrastructure, labor, queues, repairs and shortages.
- `lore/the_dry_boatmen.md` — Reed Postern, dry carry, the tun and Reed interests.
- `features/lore_ward_politics.md` — open gameplay layer this quest concretizes.
- `features/knowledge_and_rumor/` — the shared knowledge layer (facts + rumour propagation); was `features/rumors.md`. As of 2026-08-30 it also owns quest receipts, the player's casebook and the journal, which this spec currently proposes for itself.
- `features/false_peals__ring_the_bells_manually.md` — optional high-risk sequence break.
- `features/implemented/law_and_order.md` — notices, custody, surety and escape.
- `features/implemented/chalking_the_walls.md` — authoritative forged/scrubbed marks.
- `features/implemented/movement/` — schedules, travel intent and navigation seams.
- `crates/cathedral-sim/AGENTS.md` — authoritative sim, prompt/action and scheduling boundaries.

## Companion quest overview

The visual quest overview copied and reframed from `docs/codex_gdd/` lives beside this spec:

- `nine_before_rain_quest_overview.docx`
- `nine_before_rain_quest_overview.pdf`
- `generate_quest_overview.py`

The overview communicates the player experience. This markdown file is authoritative for feature scope and
implementation.
