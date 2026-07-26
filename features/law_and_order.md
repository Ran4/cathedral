# Law and order

What the city does when someone is wronged. Motivated by a real playtest
(session 472, 2026-07-24, `logs/session_472_2026-07-24_20_33_19/`) in which the
player robbed two people in three minutes and the city's response was equal
parts wonderful and hollow.

## The incident, in detail

All of this is reconstructed from the session's `prompts/` archive; timestamps
are the prompt archive's (when the answer arrived). The player is `id player`
throughout; nobody in the cast knows their name.

**Theft 1 — the badge (~20:34, the Wickmarket).** The player asked Ede-Crake
(p0012, badge seller) "whats in your hand" → "Sparks, and pilgrim badges for a
spark apiece." Player: "can i have one? i am religious and love em". Ede-Crake
held one out — `offer_item {"item_id":"pmhcm"}`, saying "A spark apiece,
friend. Here is one." — the player **accepted the badge and walked off without
paying**. By Ede-Crake's next turn the player was already >20 m away and his
"There you are. Thank you, friend." failed with a range error. Worse, that turn
recorded `remember {"memory":"I sold the player one pilgrim badge for one
spark."}` — **a completed sale that never completed**. The victim of theft 1
does not know he was robbed, and if ever questioned would testify the player
paid. (The prompt explicitly asks actors to record half-done deals as open
obligations; the model recorded a finished one instead.)

**Theft 2 — the spark (~20:35, Tenterhook Lane / Tenter Cistern).** The player
walked to Cobb (e3cob, 14, tenter boy) and reversed the con: "please buy a
pilgrim badge from me for just 1 spark". Cobb: "Aye. One spark." +
`offer_item {"item_id":"w_e3cob","quantity":1}`. The player accepted — Cobb's
`you_hold` really drops spark ×5 → ×4 — said "thanks", and then: "what badge" /
"nah" / "na" / "nope, both are mine" / "haha".

**Escalation.** Cobb, over six turns: demand → "You took my spark. Hand over
the pilgrim badge, or return it." → "Else I call the reeve." Player: "or
what?", "reeve?". Cobb: "Aye. The reeve, for this theft. Tell me where he is."
Note: **there is no reeve** — the word appears nowhere in `lore/`; Cobb
invented an office his world plausibly ought to have. Bystander Bertran-Lark
(p002c) had already joined in unprompted ("Mockery will not settle it") and
answered with a graceful save: "I do not know the reeve's seat, but the Tallage
is the likeliest place to seek civic justice" + `tell_way pl_35z0` (redundant —
Cobb already knew the Tallage — but harmless).

**Confession and the report.** Player: "no i wont give it to you! i stole it
fucker". Cobb's turn is textbook memory hygiene, all in one reply:

```
say      "You stole my spark and gave no badge. I'm taking this to the Tallage."
forget   "I paid the stranger one spark for a pilgrim badge, but they have not yet given me the badge."
remember "The stranger took one spark for a pilgrim badge, refused to give the badge or return the spark, and I am reporting the theft."
set_goal "Report the stranger's theft at The Tallage"
go_to    pl_35z0
```

**The town splits believably.** Jonet-Pike (p002g) counsels settling on the
spot — "Best return the spark and settle it here. The Tallage will make a small
theft a heavy matter" — and when Cobb refuses ("He confessed"), she limits her
testimony: "tell them exactly what you saw. I'll not swear to more than that."
Bertran-Lark issues a final ultimatum to the player. Gude-Marle and Gile-Fitch
overhear and correctly `wait {}`.

**The hollow ending.** Cobb set off (last seen at x 140, z 182, ~20 m into the
walk). The game was closed ~100 s later; he never arrived. And had he arrived,
**nothing is there to receive him** — the report would have been improvised at
whoever happened to stand within 20 m, remembered in their heads, and that is
the end of the causal chain. The whole "report it to the Tallage" thread is
emergent speech; the sim has no law transaction of any kind.

## What the city already has (facts, post-shrink coordinates)

- **The Tallage** (`pl_35z0`, weigh ward): a Cut widening at `(-213.5, 63)`,
  ~83×60 m, with the toll-house (`pl_8so5`), bonded warehouse (`pl_5vy5`), and
  weighing yard around it. It is the civic/toll heart of the city, and it *is*
  the right lore answer to "where does civic justice live".
- **Odo Trask** (fo6gl, notary, minor, district "The Tallage") — "an
  unwitnessed word is only weather", incorruptible, fee book open on the
  counter. A purpose-built receiver of sworn complaints who already exists.
- **A whole law bench in `lore/characters/bailiff_and_gaoler/`** (all ambient,
  district "Bell-and-Sluice streets", spawns x≈226–244): Havise Ashe, Jos
  Fitch, Segwin Mott (bench sergeants), Tobin Marle, Ewart Rasp (prison
  guards), Segwin Vell (court usher), Noll Brant (debt officer), Ede Clove
  (stone keeper). Plus `civic_officer/` and `court_officer/` folders. None of
  them has a law *function* in the sim — they are villagers with stern job
  titles.
- **The Bellstand watch-bell tower** (`pl_5wkc`) and the Scold's civic peals
  (curfew has a real trigger; `summons` exists as a drive stand-in).
- **Five gates** with named keepers (the no-procedural-characters rule already
  made gate people real and recurring).
- `LoreProfile` already carries an `illegal_activity` field — the seam for
  "this character is known to the law" exists.

Distance check: the incident was at `(125, 195)`. The Tallage is 364 m away
straight-line — roughly half the city — and ~450–500 m by street (via the
Wickmarket and the Needle). At `WALK_SPEED_MPS = 1.8` that is **4–5 minutes of
real time**, during which the sim produces zero feedback about the errand. It
feels like ten.

## Problem 1 — reporting crime is a single cross-town pilgrimage

One law site for an 840×700 m city means the average victim walks minutes to
report a stolen coin, and the thief watches them go. But the medieval answer —
and the answer this playtest already demonstrated — is that **crime reporting
is not a building, it is a social act**: the hue and cry. Cobb cried theft;
two strangers joined immediately; that part worked without any system at all.

What is missing is not more buildings but **law presence where people are**:

- Give the existing bench cast stations and beats at places that already
  exist: a bench sergeant whose round walks the market squares (Wickmarket,
  Coswald's Yard, the Gradine, Maren's Green), gate keepers who can take a
  report at the five gates, the watch anchored at the Bellstand watch-bell
  tower, Odo Trask at the Tallage toll-house for the sworn/recorded version.
  That yields ~8 reporting points and a longest walk of ~150–200 m instead of
  ~450, purely by staffing rounds — no new geometry.
- The distinction writes itself in lore terms: any sergeant or gate keeper can
  *hear* a hue and cry; only the Tallage can *record* it (Odo Trask: an
  unwitnessed word is only weather). Small wrongs get a sergeant's talking-to;
  recorded wrongs get consequences. Cobb's cross-town march becomes the
  *escalated* path, chosen because the boy wanted it on parchment — which is
  exactly the character he is.

## Problem 2 — nothing happens at the end of the walk

What would happen today: `go_to` arrival gives Cobb a priority turn
(engine.rs' arrival nudge), he says "I wish to report a theft" to whoever is
within 20 m — possibly nobody — they `remember` it, and the world never touches
the player again unless those NPCs happen to gossip within the player's
hearing. Medieval-honest, but reads as /dev/null.

What the LLM *should* be able to do about it, in cost order:

1. **Nothing new (baseline, acceptable short-term).** The report lives as
   memories in law-cast heads. It only surfaces if the player later talks to
   them. Cheap, already true, invisible 95% of the time.
2. **Make the accusation travel (recommended core).** A recorded report at the
   Tallage (or a sergeant hearing a hue and cry) injects a short-lived **ward
   notice** into the prompts of the law cast and, diluted, of talkative
   citizens: "word in the ward: an outland stranger took a boy's spark at the
   tenter-frames on Highmarket morning". This is the gossip network the sim
   already implies, made into a world fact with a decay clock. Strangers have
   no names, so notices carry *descriptions and places*, not ids — which is
   both period-correct and exactly what the "unknown people" rule supports.
   The player experiences it as the city cooling toward them: stallholders
   hesitate to offer, sergeants ask pointed questions, children repeat the
   story.
3. **Confrontation, not courts.** If a bench sergeant shares a stage with the
   player while a notice matching them is live, they get the percept and act
   as their character — question, demand restitution, threaten the stone. An
   `accuse`/`demand` verb is not needed; speech plus the notice percept is
   enough. Restitution (hand the spark back / pay a fine to the sergeant) uses
   the existing offer/accept machinery and *clears the notice*, which gives
   the player an actual interactive loop: steal → be named → be found → pay up
   or stay hot.
4. **Not now:** warrants, arrest, gaol time, trials. The gaol cast exists in
   lore when we ever want it (npc bench + "the stone" + Ede Clove the stone
   keeper), but a working notice/restitution loop is the 80%.

Also worth fixing regardless of the above: the **false-sale memory** from
theft 1. One sharpened line in the prompt's memory guidance — record payment
only when the counter-item is in `you_hold` / visibly received; an accepted
offer is not yet a paid one — would have made Ede-Crake the second accuser
instead of a character witness for the thief.

## Problem 3 — BUG: a silently accepted offer never wakes the offerer

Confirmed in code. When the player accepts an NPC's offer, the transfer
happens and the percept "A stranger (id player) accepted the spark (id
w_e3cob) you offered" lands in the offerer's inbox — but **nothing schedules
them a turn**:

- `EngineCommand::PlayerAccept` (crates/cathedral-sim/src/engine.rs:1213) goes
  through the plain `player_action` path — no `prioritize` call. Same for
  `PlayerDecline`.
- Contrast `PlayerSay` (engine.rs:1514–1520), which hands the addressed NPC a
  priority slot, and the `go_to` arrival nudge (engine.rs:943–951), whose
  comment states the exact principle: *off stage there is no idle rotation to
  render the percept, and without the nudge the chain dies silently*.

In session 472 it was masked twice by luck: Cobb's reaction turn (20:34:57)
was triggered by the player saying "thanks" (player-reaction lane), and
Ede-Crake's by the stray STT artifact "Transcript". Had the player accepted
in silence and sprinted, then with `idle_cognition.mode: "stage"` (the current
config: radius 32 m, max 6) the offerer goes off stage carrying an unread
acceptance percept and **never thinks again** — no reaction, no memory, no
theft ever registered — until the player happens to wander back within 32 m or
someone addresses them, at which point the stale percept finally renders and
the LLM does conclude, correctly, "they ran off with my spark". So the
intuition in the playtest report is right on both halves: yes he *would*
eventually think that — but "eventually" can be never in a session.

**Fix (small):** in the `PlayerAccept` / `PlayerDecline` handlers, after a
successful apply, call `self.scheduler.prioritize(&self.world, &offerer_id,
false, now)` — the offerer id is the item's holder, already resolved by the
apply. The priority-handoff lane is deliberately ungated by proximity, so this
works off stage, matches the arrival-nudge rationale word for word, and keeps
timing governed by the inter-turn delay and the floor. Add a scheduler-level
test: player accepts a silent offer, no speech — the offerer must be the next
selected actor even under `IdleGate::Stage(&empty)`.

(While there: an NPC accepting the *player's* offer already yields feedback in
the HUD, and NPC↔NPC acceptance happens inside the acceptor's own turn whose
`say` handoff usually covers the offerer — the player-accept path is the one
with no natural wake-up at all.)

## Problem 4 — settlement is a side effect of the item plumbing, not a choice

Found by reading M3 back, 2026-07-26. Restitution was deliberately built on the
existing offer/accept machinery so M3 would need no verb but `raise_notice`
(see item 3 under Problem 2). The result is blunter than intended:
`accept_offered_item` calls `notices::settle_on_transfer` unconditionally after
the transfer (`crates/cathedral-sim/src/actions.rs:850`), and that function
(`notices.rs:226`) tests exactly two things — is the giver the accused, and is
the acceptor the wronged party or *any* law officer. It never looks at what
moved, how much it was worth, or whether the transfer had anything to do with
the wrong.

So today:

- **Ordinary commerce launders a theft.** A sergeant buying a loaf from the
  accused clears the notice. So does the robbed boy accepting a rope he asked
  for an hour later. Neither party said anything about the wrong; neither knows
  the word just died.
- **It is value-blind.** A crust settles a stolen spark.
- **One accepted item is a general amnesty**: `settle_on_transfer` clears
  *every* live notice naming the giver, including ones the acceptor never heard
  of and the fouling notices raised automatically by
  `raise_ward_notice_for` (actions.rs:1812), which carry no wronged party at all.
- **It takes the judgment away from the actor the feature exists to give it
  to.** M3's whole thesis is that confrontation is character, not procedure —
  and then the one genuinely characterful call (*is this enough? do I take it as
  restitution or keep the word alive?*) is decided by the transfer plumbing.
- **A corrupt officer cannot be corrupt.** The lore stocks the law bench with
  bribe-takers — Havise Ashe, a bench sergeant with a real M2 beat, is "taking
  bribes during inspection"; Betriss Pell and Averil Stott are "gate bribery";
  Odo Trask is the deliberate incorruptible. None of it can express itself:
  taking the purse *is* absolution, and refusing is the only lever there is.

### M3.5 — settlement as an act

**DONE 2026-07-26.** Built as scoped below; the three places the implementation
departs from it are marked *as built* inline.

Make the clearing a verb the accepting character chooses, and make the transfer
reputation-neutral.

- **`settle_notice {"notice_id": 3}`** — callable by the law cast (as
  `raise_notice` is) *and* by the wronged party named on that notice, whether or
  not they serve the law, so the boy can forgive his own spark. Per-notice, never
  a blanket clear. Not settling is refusing; no counter-verb is needed.
- **Remove the call at actions.rs:850.** A transfer is then just a transfer.
- **Guard the dropped-verb failure mode with the idiom the codebase already
  leans on.** The bad trade here is real: the player pays, the sergeant says "we
  are square", the model never emits the verb, and the word stays live —
  indistinguishable from being cheated. So on a transfer *from* an accused *to*
  a law officer or the wronged party, hand the acceptor a percept ("this may be
  the restitution the ward's word wants — settle_notice if it answers it, or say
  why not") and a priority turn, exactly as `notices::confront` and the M0
  accept nudge do. With the prompt in front of them, an officer who then keeps
  the word alive is a story, not a bug.
- **Keep two narrow mechanical paths**, because a verb cannot cover them:
  1. The accused returning *the very item named in the notice* to the wronged
     party. This needs `WardNotice` to record what was taken — an optional
     `taken: Option<ItemId>` set from a new optional `raise_notice` arg (and
     from `raise_ward_notice_for`, which knows) — plus a line in the law
     paragraph: pass `taken` when you know what was taken.
     *As built:* `raise_ward_notice_for` passes `None` and gained no parameter.
     Neither wrong it raises *takes* anything — a spitter leaves the mouthful
     behind, a fouler leaves worse — so there is nothing whose return could
     settle those words, and a parameter both callers pass `None` to would be
     a seam with nothing on the other side. `taken` comes only from
     `raise_notice`.
  2. **The player as acceptor.** The player has no verbs. When an NPC accused
     hands the player-as-wronged the taking, nothing would ever settle it, so
     that acceptance must still clear mechanically — and it is the one case
     where the transfer really is unambiguous. *As built:* not restricted to
     the `taken` item — for the player any transfer from the accused settles,
     since no verb of his could ever distinguish them.
- **Prompt cost:** one verb line and a sentence in the law paragraph, both
  already gated on `has_law_verbs` (prompt/mod.rs:472) except for the
  wronged-party case, which needs the verb listed when a live notice names you
  as wronged. Non-law, notice-less prompts stay byte-identical; the law fixtures
  are regenerated with the ignored `regenerate_golden_fixtures` test.
  *As built:* two further prompt changes were needed, both on carrier sheets
  only — the golden fixtures carry no notices and did not move a byte.
  1. `word_in_the_ward` bullets are **numbered** (`- notice 3 — …`), exactly as
     `your_round`'s legs are and for the same reason: `settle_notice` names a
     notice by its number, and the sheet is the only place the model can read
     one off.
  2. The wronged party gets their own short paragraph (the law's is written for
     officers), and `notices::carries` now always carries a notice to the person
     it names as wronged — a taciturn victim would otherwise hold the verb with
     no number to give it.
- **Tests:** an unrelated purchase from an accused no longer settles anything;
  the officer's `settle_notice` clears exactly one notice and the carriers hear
  it die; a non-law non-wronged caller is refused; the wronged party may settle
  their own; returning the named `taken` item settles without the verb; the
  player accepting restitution settles; the restitution percept lands and the
  acceptor is the next selected actor even off stage.

Bribery falls out of this for free, which is the point: the sergeant takes the
purse and simply does not call `settle_notice`, and Odo Trask takes nothing at
all.

## Problem 5 — refusal has no floor

M3.5 gave settlement a chooser. It gave refusal nothing. A player who never
settles simply keeps walking: officers get the `confront` percept and demand
restitution, forever, and the word decays after `NOTICE_LIFE_GAME_DAYS`
whether or not anyone answered it. There is no rung below "asked again more
sternly". Worse, the wrongs that *cannot* be answered by restitution already
exist in the tree — `raise_ward_notice_for` (actions.rs:1809) raises fouling
notices with no `wronged` and no `taken`, so nothing but a law officer's
`settle_notice` can ever end them — and the sim's only response to one is more
talking.

## M4 — taking hold

Custody. Designed 2026-07-26 and revised the same day against the shipped code,
which moved five things: how close an officer has to get and how they get there,
`notices::confront`'s once-ever `served` set, which side of the sweep the tether
clamps on, what "a breach the officer witnessed" can actually be checked
against, and where the grab reflex lives. The decisions below are settled, not
options.

### The constraint that shapes all of it

`src/controller.rs:63-64`: the player walks at 8 m/s and runs at 12. Every NPC
in the city walks at `WALK_SPEED_MPS = 1.8` (`lib.rs:169`). **The player is
4.4× faster than any officer**, so arrest can never be pursuit — a chase you
always win is worse than no chase at all.

Second: the sim has no authority over the player's body. It *reads* the player
through `EngineCommand::SpatialUpdate` and never writes. Holding the player is
a genuinely new host capability (drive's `tp` proves it is possible), and an
LLM is what pulls the trigger. That is the largest single thing this milestone
introduces and everything below is shaped by keeping it safe.

### What the lore already decided

`lore/core_lore/secular_government.md` is unusually specific, and it is the
design:

- **Two doors.** Gate and night watchmen *may stop an immediate breach of the
  peace* with no warrant, but must bring the prisoner and written cause to the
  Stone House by the next office bell. Bench sergeants otherwise make **arrests
  ordered by a court**, serve summonses and escort prisoners; ordinary debt
  collectors have no arrest power.
- **The Stone House** by the River Gate is the civic gaol; its Stone keeper is
  Ede Clove (`p009w`, 22, whose own `illegal_activity` is *violent debt
  collection*). It holds the accused awaiting hearing, short sentences and
  committed debtors. Families bring food and bedding.
- **"Gaol fees are fixed publicly; inventing a fee is extortion."** That one
  line is the exit design: custody is not a timer, it is a price.

**The court is out of scope.** Three benchers sitting as the Civic Measure
Court is a whole second system, so in M4 the warrant comes from an ignored
summons rather than a bench. Leave a TODO in the custody module's doc comment
beside the release paths — "committed to await a hearing" is exactly where a
bench slots in later — and nowhere else.

### The ladder

Rungs 1–2 shipped in M3/M3.5; every rung is an act some character chooses, and
nothing escalates on a timer alone.

1. **word** — the live notice; carriers cool toward you.
2. **demand** — `notices::confront` plus speech.
3. **`summon {"notice_id": 3}`** — an officer out of patience calls you to
   answer by a named office bell. The Scold's `summons` peal exists today only
   as a drive stand-in with no real trigger; this gives it one.
4. **warrant** — the summons ignored past its bell. Any law-cast actor may then
   take you, anywhere in the city.
5. **`seize`** — into custody, ≤ `ITEM_INTERACTION_RADIUS_M` (4 m), on a live
   warrant **or** on a notice *this same officer raised within the last game
   hour*. The lore's two doors exactly — see below for why the second one is
   written that way.
6. **the escort** — walked to the nearest station.
7. **`release`** — available on every turn, to the holder and to the keeper.

**Gate keepers refusing passage is deliberately not a rung.** It reads well and
it is not buildable in M4: only the Stone and River gates have working leaves
(`src/city/gates.rs`), those leaves are driven by the office clock rather than
by the law, and the Harne Gate, the Wool Gate and the Reed Postern are open
holes in the wall. A law override of the gate schedule is a host-side feature
with no home in M4a–M4e; a gate keeper who is *told* to stop you gets what every
other officer gets, which is `seize`.

### What answering means, and three ways a rung dies quietly

Four decisions the ladder needs that the shipped code does not make. Each is
small, and leaving any of the first three out makes the ladder stall *silently*
rather than visibly — which is exactly the failure mode M3 was written to end.

**What answering a summons *is*.** The notice being settled — `settle_notice` by
an officer or by the wronged party, or `settle_on_return` — and nothing else.
There is no separate "presented myself" state to track: going and dealing with
it *is* restitution, or it is talking the officer round until they call the verb.
If the notice is still live when the named bell rings, the warrant issues on the
clock edge. That makes M4a testable — the deadline has exactly one thing that
clears it — and it means the M3.5 machinery is what discharges every rung.

**`notices::confront` must re-arm.** It serves each (notice, officer) pair once
and never again: `served` (`notices.rs:117`) is written at `:372` and cleared
nowhere, so it dies only with the notice. Every rung above 2 assumes the officer
keeps being told the accused is standing in front of them — and with
`require_news: true` (`config.ron:39`) an officer whose inbox is drained gets no
idle turn at all, so after one encounter the ladder stops dead. **Clear `served`
whenever the notice changes rung.** A summons, and then a warrant, each re-arm
the face-to-face percept for every officer, which is precisely the moment they
should look at you again.

**A warrant must not be evicted by gossip.** `Notices::raise` drops the oldest
live notice past `NOTICES_MAX_LIVE = 8` (`notices.rs:81,176`), so in a talkative
ward a warrant can vanish without anyone deciding anything — and M4d's escape
notice can evict the very notice it escalated from. **Oldest-out skips warranted
notices**; if all eight carry warrants the raise is refused instead. A warrant
ends because somebody ended it, never because the ward changed the subject.

**The brand is long, and that is the choice.** `NOTICE_LIFE_GAME_DAYS` is 20
game days, which at `seconds_per_day: 3600` is 20 real hours — so a warrant does
not expire inside any real session, and M4d's escape notice (no `wronged`, no
`taken`) cannot be answered by restitution either. A player who runs is wanted
for the rest of the run and the only exit is a law officer choosing
`settle_notice`. That is deliberate: it is what makes escaping a real cost
rather than a delay. It is only acceptable because the exit is a person you can
go and talk to, so — as in M5 — **the HUD must always name what would clear it**.
A brand with a visible door is a story; a brand with no door is a bug.

### Custody is a state; the grab is its enforcement

`seize` does **not** take hold of you. It puts you in custody and names a
destination; you are free to walk. The hand comes onto your arm only when you
break the arrangement.

```
Free ──seize──► In charge ──break──► Held ──arrive──► Committed ──release──► Free
                    │                   │                  │
                    └──── release ──────┴──────────────────┘
```

Compliance is the pleasant path and must stay that way: a 100–200 m walk beside
an LLM sergeant who talks to you the whole distance and can let you go at any
point. **The escort is the content, not the cell** — being marched in public is
the punishment, and it is a conversation you can still win.

The same diagram is the NPC's, with two boxes greyed out: an NPC prisoner is
simply led (the sim moves both of them), so *In charge* and *Held* differ only
in whether `struggle` can work, and nothing between them touches the host. Every
state below is written for the player because the player is the hard case; where
it says "you", an NPC gets the cheap version of the same thing.

### Arm's reach, and the walk that gets there

A grab cannot be a lasso: the officer is slow and an LLM turn costs seconds, so
by the time a prompt returns a sprinting player is 30 m gone. So the grab is a
reflex at contact range, and everything else follows from that.

- **`CUSTODY_REACH_M = 3.0`** — the same order as `WELL_ARRIVE_RADIUS_M` and
  `STALL_ARRIVE_RADIUS_M`, and inside the 4 m offer radius.
- **Pre-authorized and host-side.** `seize` is the officer declaring intent;
  the grab that enforces it fires mechanically and instantly, with no provider
  round trip. Same split as the npc_bodies gaze reflex — reflexes are code,
  decisions are prompts.
- **`CUSTODY_LEASH_M = 8.0`**, deliberately `SOCIAL_PULL_RADIUS_M`: while
  merely in charge you may step aside, look at a stall, talk to someone you
  pass. It is not enough to drift out of by accident.
- **The escort keeps contact**, walking at ~1.5 m, so you are normally within
  reach without being held.
- Cross the leash and the officer closes; reach 3 m while you are outside it or
  moving away and the grab fires.

**The approach is the tell — that is the skill, not the reach.** It would be
convenient if standing in conversation meant standing in reach, and it does not:
`ACTOR_FOCUS_RADIUS_M` is 20 m (`src/smart_actors/targeting.rs:15`), so the
player aims at, talks to and is talked to by anyone within a wide square, and
has no reason to come inside 4 m unless they want to trade. Nothing about
ordinary conversation puts you within arm's length of anybody.

So an officer who means to take you has to **close first**, on their own two
feet, with `go_to {"person": "player"}` — 1.8 m/s, in the open, walking straight
at you and usually talking while they do it. That walk is the whole warning
system, and reading it is what the player learns:

- **Let a sergeant close, and bolting is too late** — they are already at 1.5 m
  and the grab is a reflex with no provider call in it.
- **Move while they are still crossing the square and you are simply gone** —
  8 m/s against 1.8 needs no cleverness at all.

This is the better version anyway, because it is legible: nobody is ever grabbed
out of nowhere, and every seizure is preceded by several seconds of a person
visibly walking at you. It is what makes M2's beats and postings tactically real
— an officer already standing where you want to be is the threat, not an officer
who spots you across a square — and it makes the 4.4× speed disparity stop
mattering without pretending it is not there. The player always *can* run. The
game's only job is to make sure they knew they had to.

### What being held feels like

- **The camera is never taken.** Losing your view is nausea; losing your feet is
  drama. Look anywhere, including at the hand on your arm.
- **Movement is tethered, not disabled.** Input still runs, clamped to ~1.5 m
  around the grip point: turn, face them, face away, circle. You cannot leave.
  When the officer walks, the anchor moves and you go at their 1.8 m/s.
- **Collision wins over the tether, and the clamp goes *before* the sweep.**
  Clamp the **desired** position — the wish, the delta this tick — and hand that
  to `move_aabb` (`src/controller.rs:690-698`) so the existing swept solve is
  what actually writes the position. Never clamp the position the sweep already
  returned: that is a raw write into a resolved result, it can push the player
  through a wall, and it would drag them back through the very stall that ought
  to have saved them. Done in the right order, putting a market stall between you
  and the officer breaks the grip by itself — a free, physical, discoverable
  escape that costs nothing to build.
- **If geometry beats the anchor, the grip loses.** The officer is the mover, so
  they can walk somewhere the tether cannot follow: a corner, a stall, a doorway
  the player is wedged against. The sweep stops the player, the separation grows,
  and once it passes `CUSTODY_LEASH_M` the hold simply ends — no teleport, no
  rubber band, no pinning a player inside a wall to preserve a state flag. It is
  the same escape as the stall, arrived at from the other side, and the officer
  gets the ordinary broke-free percept and turn.
- **A visible hand**: npc_bodies' articulated arms already do offer
  choreography; the arm goes to the upper arm and stays.
- **Sound is already catalogued** — `features/more_sounds/more_sounds.json`
  has the keys-jingle tagged "watchmen, gatekeepers, and prisoner escorts", and
  the gaol door reserved for "the future Stone House near River Gate".
- **HUD**: a standing line on the offer-lapse notice precedent — *"Havise Ashe
  has taken you in charge — the Tallage toll-house"*, then *"Held by Havise
  Ashe"*. The leash is explained once, the first time it is ever drawn.

### The struggle-out

Pull, don't mash: hold a movement direction away from the officer. A **strain**
meter fills over **~5 s of continuous pull** and drains ~3× as fast the moment
you stop. Escape is meant to be easy enough to be a real choice — what should
make you hesitate is the consequence, not the difficulty.

Modifiers, all on seams that already exist:

- **Drunkenness and weariness** (npc_bodies M5 statuses, drive's `status`
  action) slow the fill.
- **Two holders is much worse**, near-impossible. The escort's right move is
  therefore `say` — "help me hold this one" — and nearby law `go_to
  {"person": …}`. No new verb, and being *dragged* by two people is what the
  word actually means.
- **Grip by occupation**: `bailiff_and_gaoler` / `militia_and_soldier` hold
  harder than `revenue_worker`. Ede Clove should be worse to be held by than
  Odo Trask, and her sheet already says why.

Struggling is loud and public — but it is **two percepts, not a stream**:

- The holder is told **once when the pulling starts** and **once if it succeeds**,
  and gets a priority turn on each, so the LLM is in the loop for the interesting
  call — tighten, shout for help, or let go in character ("Run then. The word
  will find you.").
- Bystanders within `HEARING_RADIUS_M` get the same two; a hue and cry raises
  itself.

The obvious version of this — a percept about once a second while the meter
fills — does not survive contact with the scheduler. There is **exactly one LLM
turn in flight at a time** (`in_flight: Option<InFlight>`, `scheduler.rs:137`),
so across a ~5 s struggle the holder gets one turn no matter how often they are
poked; the priority deque de-duplicates the rest, and the only thing a
per-second percept actually achieves is five near-identical lines competing for
one `since_your_last_turn` and crowding a 64-entry inbox. One at the start and
one at the end says everything the model needs and costs what it is worth.
- **Breaking free auto-raises a notice** with no `wronged` and no `taken`, the
  same shape `raise_ward_notice_for` already makes for fouling — structurally
  unanswerable by restitution. Escape closes the "you could have just paid the
  fee" door, and that is the cost that makes the choice a choice.

### Confinement without geometry

Custody goes to the **nearest station**, not always the Stone House: the
Tallage toll-house counter (`pl_8so5`, Odo Trask), the five gates and the Reed
Postern, the Bellstand watch-bell tower (`pl_5wkc`). Wickmarket → River Gate is
~300 m, i.e. **three minutes pinned at 1.8 m/s**, which is too long; the
nearest station is 50–150 m. This is M2's argument repeated ("law presence
where people are, not one building"), the station list falls out of M2's
existing postings and `workplaces["bailiff_and_gaoler"]`, and **M4 therefore
needs no new geometry at all**.

Most of those are not lockable rooms, so: **you are confined by a person, not a
door.** The keeper holds the threshold; walking out is an escape attempt, and
they are within 3 m, so it is the same reflex grab. Custody-by-supervision is
period-honest and free. The Stone House, whenever it is built, becomes the one
place with a real door — and the only place a grave matter is committed to.

### Exits

All on machinery that already exists:

1. **Pay the posted fee** — offer coins to the keeper; `restitution_candidates`
   already earns them the percept and the turn, and they `settle_notice` and
   `release`.
2. **Surety** — someone vouches for you. Pure speech; the keeper releases.
3. **Talk your way out** — `release` is on every holder's turn.
4. **Wait it out** — a hard cap, no exceptions. At `seconds_per_day: 3600` an
   office bell is ~8.5 real minutes, which is far too long to stare at a wall,
   so the keeper releases after **4 real minutes** regardless of what the
   models do. This is the *station* cap, and it is short on purpose: a gate
   arch or a toll-house counter has nobody to talk to but the keeper. M5's
   Stone House sets its own, longer ceiling of 6 minutes, because there the
   waiting is the content — eight inmates in one room — rather than the price
   of it. Two numbers, two rooms, and the difference is the point.
5. **Break out** — struggle past the keeper. Compounds, as above.

### Verbs

Small, and gated exactly as `has_law_verbs` is (`prompt/mod.rs:472`), so a
sheet that is neither law's nor a prisoner's does not render a byte of this:

```
seize {"person": "4bfk4", "notice_id": 3}    # Law only: take someone in charge for the nearest station
grab {"person": "4bfk4"}                     # Take hold of someone you have in charge
release {"person": "4bfk4"}                  # Let them go
struggle {}                                  # Held only: try to pull free of whoever has hold of you
```

- `seize` requires law occupation, ≤4 m, and a live warrant **or** a notice this
  officer raised themselves within the last game hour — **plus a `say` in the
  same turn**, the rule turn.j2 already applies to a silent `go_to`. A wordless
  seizure reads as the game stealing the controller.
- `grab` is usually the reflex against the player; it is exposed so an officer
  *can* take hold deliberately, and rendered only while they hold someone or
  have just seized. Against an NPC prisoner it is the only way hold is taken —
  there is no reflex, because there is no one to outrun anybody.
- `struggle` renders only to someone actually held, and is the NPC's whole
  resistance move; the player's equivalent is a key they hold down.
- Calling for help needs no verb: speech plus `go_to {"person": …}` covers it.

**Why the second door is "a notice you raised yourself, recently".** The lore's
wording is *an immediate breach of the peace the watchman witnessed*, and the
sim has no such thing to check: percepts are prose, and there is no structured
record anywhere that says this officer saw that happen. Written literally the
precondition is untestable, and the test "refused with no warrant and no
witnessed breach" cannot be authored at all.

But witnessing is already modelled — just under a different name.
`raise_ward_notice_for` (`actions.rs:1809`) is exactly a breach an officer saw
with their own eyes, raised mechanically by the nearest law-cast character
within earshot, and `WardNotice` already carries both fields the check needs:
`raised_by` and `raised_game_days` (`notices.rs:103-104`). So the second door is
**a notice whose `raised_by` is this officer and whose `raised_game_days` is
within the last game hour** — no new state, no new verb, and the same meaning:
you saw it, it was just now, you may take them without waiting for a bench. A
notice somebody *else* raised, or one this officer raised yesterday, needs the
warrant like everything else.

### The cast arrests each other too

`seize` takes `{"person": …}` and means it. The law may take **anyone** —
which is the half of this milestone that costs almost nothing and pays the most.

**There is already an open loop it closes.** `raise_ward_notice_for`
(`actions.rs:1809`) raises notices against *NPCs* today: a townsman who fouls
the street in front of a sergeant gets a word on the ward's tongues, and then
absolutely nothing happens to him, ever. The whole ladder above rung 1 has been
written as if the player were the only person who can be wanted, and they are
not. A city whose law only ever activates against the one outlander in it is a
theme-park; a city where you round a corner and find Havise Ashe walking some
baker to the Bellstand is a place that was already running before you got there.

**And it is far cheaper than the player case.** Nearly everything hard about M4
is hard because of the player specifically: the 4.4× speed gap, the host owning
the player's feet, the tether, the reflex, the strain meter, the dead-man timer.
None of it applies here. The sim is already the authoritative mover for NPCs, so
an NPC in custody is *slaved to the escort* — no clamp, no host message, no
`controller.rs` at all, and both parties walk at 1.8 m/s so there is no
disparity to engineer around. **NPC custody is M4b plus a sheet section.** The
grab (M4c) and the strain meter (M4d) stay what they always were: the player's.

What it does need:

- **The prisoner has to know.** A short `you_are_held` sheet section — who has
  you, where you are being taken, and what would end it — plus `struggle` on
  their verb list. Gated on the state, exactly as `has_notices`, `has_pockets`
  and `has_frontbutt` already gate theirs (`prompt/mod.rs:478-503`).
- **So restate the budget promise honestly.** It was never "non-law prompts are
  byte-identical"; it is *"a prompt renders nothing for a situation you are not
  in"*, which is the idiom the whole sheet is built on. Someone not in custody
  and not law pays zero bytes, and the golden fixtures do not move. A person
  being marched across the city by a sergeant can afford four lines about it.
- **`struggle {}`** — the NPC's counterpart to the player's held movement key,
  and the reason the player's strain meter has no equivalent here: an NPC's
  attempt resolves in the sim on the spot. Same modifiers as the player's meter
  (the holder's occupation grip, the prisoner's drunkenness and weariness, two
  holders being near-hopeless) applied as one deterministic roll on the
  `attention::opens_first` hash idiom rather than a 20 Hz meter, so a scripted
  run reproduces. Success raises M4d's unanswerable escape notice — theirs is
  the same brand yours is.
- **A confinement cap, because the city has to keep working.** Nothing else in
  the sim removes a person from the world, and the economy is made of named
  individuals: gaol the wrong baker and the bread round stops. Cap the total in
  custody city-wide (the eight authored inmates do not count against it), refuse
  `seize` past it, and lean on the release paths — a station's 4 minutes, the
  Stone House's 6 — so custody is always draining. A sergeant having a bad day
  must not be able to empty the Wickmarket.
- **The player is never the arresting officer.** They have no verbs and no
  occupation. Watching is the whole point.

The reward for all of it is that M5's cell stops being a fixed cast of eight and
becomes a room with **traffic** — someone new in the corner tonight, someone
gone tomorrow, and the possibility that the person you get talking to in there
is somebody you got put there.

### The seam the sim does not have

1. **Authority over the player's feet.** A new hot `EngineMessage` carrying
   holder, anchor and radius — hot like `Movement`/`Clock`, never bumping
   `world_revision` — applied in `controller.rs` by clamping the **desired**
   position and letting the existing swept solve resolve it (never the position
   the sweep returned; see the tether bullet above).
2. **The reflex lives in the host, and the sim is told.** `seize` is the sim's
   decision and it publishes custody; the *grab* is host-side code that watches
   the real distance every frame and fires with no round trip. The sim learns
   about it from a `PlayerGrabbed { holder }` command alongside
   `PlayerStruggling { holder }` and `PlayerBrokeFree`, and it is that command —
   not a sim-side distance check — that earns the holder their percept and
   priority turn. **This is not a free choice.** The sim reads the player at
   `POSITION_UPDATE_HZ = 10` (`src/smart_actors/mod.rs:60`), which is 1.2 m of
   travel per sample at run speed, before the return trip; a 3 m reflex decided
   sim-side would be wrong by most of its own radius. Host-side it is exact and
   instant, which is the entire promise of the mechanic.
3. **Strain stays host-side.** It is a 20 Hz input meter and the sim has no
   clock by design; the sim hears only the throttled commands above.
4. **A dead-man timer, non-negotiable.** If the holder takes no turn for 60 s —
   provider outage, lane starvation, a killed process — the hold releases
   itself. A player must never be pinned by an API failure. Note that with one
   turn in flight across the whole cast, a busy scene can starve a holder past
   60 s with nothing broken at all; releasing then is correct, not a false
   positive, and the officer can always take hold again.
5. **Fly mode ignores custody** (developer flying is not a jailbreak).
6. **Offers keep working while held**, since paying the fee is the main exit.
7. **Refcount the hold** — two officers, one lets go, you are still held.

### Sub-milestones

- **M4a — summons and warrant.** `summon` verb, the deadline on the notice
  (discharged by settlement, nothing else), the warrant flag, the `served`
  re-arm on every rung change, the warrant exemption from oldest-out eviction,
  the Scold's peal wired to a real trigger, the HUD rung display. No physical
  contact yet; entirely testable headlessly.
- **M4b — custody without a grab.** `seize` / `release`, the station picker,
  the escort walk, the leash, the HUD lines. Compliance path only: walking away
  just ends custody. This is already a complete, shippable scene.
- **M4b′ — the cast arrests each other.** The `you_are_held` sheet section, the
  prisoner slaved to their escort, the confinement cap. Rides on M4b's escort
  and station picker and touches no host code at all, so it lands in the same
  breath — and it is the sub-milestone that makes the law visible to a player
  who has done nothing wrong. `struggle` waits for M4d.
- **M4c — the grab.** The player's case only: the hot custody message, the
  pre-sweep tether clamp in `controller.rs`, the host-side reflex and its three
  commands back to the sim, the hand, the sounds, the dead-man timer.
- **M4d — the struggle.** The player's strain meter and its modifiers, the NPC's
  `struggle` verb and its one deterministic roll, the two percepts and their
  priority turns, the escape notice both paths raise.
- **M4e — committed.** The keeper at the threshold, the posted fee, surety, the
  4-minute cap at a station — for whoever is standing there, cast or player.

### Tests

Sim-side (`cathedral-sim`, headless, no host):

- A non-law actor's `seize` is refused; so is one at 5 m and one with no `say`
  in the same turn.
- An officer seizes an NPC on that NPC's own fouling notice — the whole
  `raise_ward_notice_for` → `seize` → escort → station chain runs with the
  player absent from the world entirely.
- A seized NPC's sheet grows a `you_are_held` section and a `struggle` verb; the
  same character's sheet before the seizure and after their release is
  byte-identical, and an unrelated bystander's never moves.
- A held NPC follows their escort without a `go_to` of their own, and their
  `go_to` is refused while held.
- `struggle` is refused to someone not held; it succeeds against one holder and
  reliably fails against two; the roll is deterministic across identical runs.
- `seize` past the confinement cap is refused, and the eight authored inmates do
  not count toward it.
- `seize` on a warranted notice is allowed; on an unwarranted notice this
  officer raised minutes ago it is allowed; on one *another* officer raised it
  is refused; on one this officer raised a game day ago it is refused.
- A summons is discharged by `settle_notice` and by nothing else; a summons
  still live when its named bell rings raises the warrant on the clock edge.
- Every rung change clears `served`, so an officer already confronted about a
  notice is confronted again once it carries a summons, and again on a warrant.
- Oldest-out never evicts a warranted notice; with all eight warranted, the
  raise is refused instead.
- The station picker returns the nearest posting, never the Stone House by
  default.
- `PlayerGrabbed` earns the holder the priority turn; `PlayerStruggling` and
  `PlayerBrokeFree` each earn one, and a struggle produces exactly two percepts
  however long it lasts.
- Breaking free raises an unanswerable notice (no `wronged`, no `taken`) that
  `settle_on_return` cannot clear.
- The dead-man timer releases a held player when the holder is starved.
- Paying the keeper earns the restitution percept and priority turn (the M3.5
  path, unchanged) and `settle_notice` + `release` frees you.
- The 4-minute station cap releases regardless of the models.

Host-side (`controller.rs`, no provider in the loop at all):

- Leash: inside 8 m nothing happens; outside it the officer closes; at 3 m
  while outside it or moving away the grab fires in the same frame.
- The tether clamps the desired position, not the resolved one: a held player
  pushed at a wall ends up outside it, never inside it.
- A solid between the player and a walking anchor breaks the grip rather than
  dragging them through it, and the separation passing `CUSTODY_LEASH_M` while
  blocked ends the hold.
- Struggling ~5 s breaks the hold; stopping for a second loses most of the
  meter; two holders do not break in 5 s; drunkenness and weariness slow the
  fill.
- Flying is not custody: the clamp does not apply.

## M5 — the Stone House

The gaol. Designed 2026-07-26, after M4 and deliberately not inside it: M4 ships
complete on stations alone, and this is geometry plus a cast relocation plus a
new confinement state, which wants its own milestone. Revised the same day
against the shipped code, which sharpened three things: the Bellstand site's
real argument (M2 already posts the watch there), the single-lane scheduler the
room has to live inside, and the second place `confined` has to be enforced.

### The finding that changes the scope

Eight characters already carry the `prisoner` circumstance
(`features/implemented/movement/03_the_ladder.md` §2), and their sheets are not
ambiguous about where they are:

> *"You worked as an errand child and are **now held** from Bell-and-Sluice
> streets, still tied to a shared bell-and-sluice work-and-lodging group.
> **Stone House rations** and food carried in by kin are your present
> support."* — Lise Skell, `p0056`

Betriss Skell (`p005c`), Lise Skell (`p0056`), Aldith Hobbe (`p0055`), Sible
Rud (`p005f`), Jonet Marle (`p0059`), Havise Pike (`p005a`), Osanne Tarn
(`p0057`), Aubin Clove (`p00b0`). Seven are young household servants — errand
child, kitchen maid, housemaid, chamber servant — which is exactly who a
medieval city held on a petty-theft accusation. **All eight are currently
spawned walking around Bell-and-Sluice streets**, because there is nowhere to
hold them. That is a live world-consistency bug, and it is also the gaol's
entire population, already written.

Three hooks sit in the same data and cost nothing to use:

- **Ede Clove** (`p009w`) keeps the Stone House; **Aubin Clove** (`p00b0`) is
  inside it. This world's surnames are family-coded (`lore/families/`).
- **Tobin Marle** (`p009y`) is a prison guard; **Jonet Marle** (`p0059`) is an
  inmate.
- Ede Clove's authored goal is **"Replace a broken stone house lock."** The lock
  is broken in the shipped world state, right now.

### Why a gaol is not a lockout here

Lockouts are anti-gameplay, which is why every other game fades to black and
says *three hours later*. This game's content is talking to LLM people, and the
Stone House is **the only place in the city with a captive cast that has nothing
to do but talk to you**. Everywhere else people are on rounds, walking to work,
leaving mid-sentence. In here: eight strangers, one room, and time.

So the goal is not to make the gaol short. It is to make it the densest social
scene in the game and let the player choose when to leave. The cap exists to
prevent a soft-lock, not as the intended exit. **No fade to black, ever.**

And with M4b′ the eight are only the *standing* population. The law arrests the
cast too, so the cell has traffic: a face in the corner tonight that was not
there yesterday, an inmate gone by morning because somebody paid for them, and —
the good one — the occasional person in here because of something you did. A
fixed cast of eight is a diorama you visit once. A room whose occupants change
is somewhere you check on.

### The room is denser than the lane

The one thing that can wreck this: eleven people in one room and **one LLM turn
in flight across the entire city** (`in_flight: Option<InFlight>`,
`scheduler.rs:137`), behind a stage that admits at most six
(`config.ron: idle_cognition.max_actors: 6`). Left alone, the densest social
scene in the game is also the one where each person speaks least often — a room
of eight who each get a turn every eighth turn is a room of people ignoring you.

Three things make it work, none of them new machinery:

- **Broadcast is the gaol's native register.** A `say` with no target reaches
  every one of them at once (`HEARING_RADIUS_M` covers the room several times
  over), so one player line gives all eight news and the stage picks who answers.
  Talking to a cell is not talking to eight people in sequence; it is talking to
  a room, and the model's own choice of who speaks is the scene working.
- **`require_news: true` is doing you a favour in here.** An inmate with nothing
  new to react to takes no turn and costs nothing — so the room is quiet until
  you make it not quiet, which is exactly how a cell should feel.
- **Six on stage out of nine is not a shortfall.** The three the stage leaves out
  are the ones with nothing to say this minute. If it ever reads as dead, the
  lever is `max_actors` in `config.ron`, not a special case in the sim.

This is also the ceiling that sets M4b′'s confinement cap: the room is already
at the stage's limit with its authored eight, so arrivals should be counted in
ones and twos, not admitted freely. A cell of twenty would not be twenty times
the scene — it would be the same six talking with fourteen mutes behind them.

What must **not** happen is the gaol getting its own scheduler, its own lane or
its own cadence. One turn at a time is the sim's shape everywhere; the Stone
House is a room, not an exception.

### Where it is — and the lore resolved

The lore put the Stone House by the River Gate (`areas.json` `river_gate`,
x ≈ −353, z ≈ −95) while the whole gaol cast, Ede Clove included, spawns at
x ≈ 226–244 — the opposite end of Bell-and-Sluice. One had to move, and the
building is the cheap one to move, because **the watch is already posted at the
Bellstand**: `rounds.json` has `workplaces["bailiff_and_gaoler"] = ["Bellstand
watch-bell tower"]`, put there by M2. The guards and the keeper already spend
their working day in that yard. Building the gaol under the River Gate would
mean either a gaol with no staff or undoing M2's postings; building it at the
Bellstand costs nothing and the cast is standing in it already.

**The Stone House goes to the Bellstand**, in the side court behind the square
and under the watch-bell tower (`bellstand_tower`, x 33.8–55.8, z −202 to −176).
Resolved in `lore/core_lore/secular_government.md` 2026-07-26: the name is older
than the building, the first Stone House by the River Gate was condemned in the
Hammering — the Line-keeper's power to strip an unsafe upper storey and close a
passage was already in that same chapter — and custody moved to the watch's own
yard. The old shell still stands and the old still call it the Stone House.

The gameplay reasons this is the better site:

- **The watch is already posted here.** M2 anchored the routeless rest of the
  bench on the Bellstand watch-bell tower via `workplaces["bailiff_and_gaoler"]`.
  Guards and gaol in one yard is both correct and free.
- **The Bellstand is a major place** (`pl_u2ka`) the player already knows and
  visits; the River Gate is a far western corner nobody has a reason to reach.
- **Escorts stay short** from most of the city — the M4 argument again.
- **The bell rings directly overhead.** A prisoner told they go at Lamplight
  then *hears the Scold ring Lamplight over their own head*. The peals already
  exist (`soundscape.rs`, drive's `bell` action); this makes one of them a
  clock the player is serving time against.

Build it beside the belfry with the existing idiom — `build_bellstand_belfry`
and `build_bellfoot_passage` (`src/city/mod.rs:2922`, `:3117`) — then the usual
rebake: collider → `export_collision_footprints` → `scripts/bake_navigation.py`,
plus `scripts/bake_places.py` for the new `pl_` id, and **re-pick the nav node
pins afterwards** (they move on every bake).

### What commitment does

- **You are booked as a description, not a name.** Nobody in the city knows the
  player, so the keeper's book reads *"an outland stranger in a grey hood"* —
  the "unknown people" rule paying for itself again.
- **Confiscation is narrow: only `WardNotice.taken`.** M3.5 already models the
  specific stolen thing, and returning it settles the word by itself
  (`settle_on_return`). Do **not** seize the player's inventory generally: it is
  a rage mechanic and it would fight the offer machinery.
- **The fee is posted publicly** — the lore is explicit that inventing one is
  extortion, so it is a fixed number the keeper will state on request. **3
  sparks** (a pilgrim badge was 1).
- **You are given nothing.** No bedding, no rations, no candle; families bring
  those, and `blanket` is already an item kind documented as *"dried before
  curfew, carried into the gaol with food — the poverty item"*
  (`features/implemented/add_items_described_in_the_lore.md`). **The inmates
  have blankets and bread and the player does not.** That single asymmetry is
  the engine of every conversation in the room, and it needs no system at all.

### Five doors out

1. **Pay the posted fee** — the existing offer machinery; the keeper gets the
   M3.5 restitution percept and priority turn, then `settle_notice` + `release`.
2. **Surety** — someone comes and vouches for you. **Build for this one.** It
   requires a person who knows you and will cross the city for you: if you have
   been decent, somebody shows up; if you have robbed the ward, nobody does. It
   is speech plus `go_to` plus `release` — no new mechanism whatsoever — and it
   silently audits the entire playthrough.
3. **Serve it** (below).
4. **Talk the keeper round** — `release` is on every turn.
5. **Break out** — the lock is broken, after all. Compounds into M4d's
   unanswerable escape notice: the one word in the city that no restitution can
   answer and only an officer can end. Note that the gates are *not* shut to you
   — for the reason M4's ladder gives, gate refusal is not built — so what you
   are buying is a permanent warrant, not a locked city.

A standing HUD line must always name what would free you right now. Never a
mystery box.

### Time is diegetic, with an invisible ceiling

The sentence is stated in the game's own clock — **"you go at Lamplight"** —
because everything else in the sim is. Back it with a hard real-time ceiling of
**6 minutes**, since at `seconds_per_day: 3600` an office bell can be 8.5 real
minutes out. When the bell is near the ceiling never fires and the diegetic
answer is simply true; when it is far, the player is released early and the
keeper has a reason ("the keeper wants the room").

Six here against M4e's **4** at a station, deliberately: the station cap is the
price of standing at a gate arch with one person to talk to, and this one is
sized for a room worth staying in. If they ever want to be one number, it is
because the Stone House failed at being interesting, and that is the thing to
fix instead.

### `confined` — one state, both sides of the door

The eight inmates will not stay put by themselves: the movement ladder reads
needs first, so an inmate whose `thirst` drops sets off for the nearest cistern
and walks through the gaol wall. So `confined` is a character state checked
**before** the needs rungs.

It needs guarding in **two** places, because there are two ways to start
walking:

1. **The ladder** — `round.rs::decide` (`:6590`), returning "stay" above every
   rung it already has. That one guard covers curfew routing, parched, famished,
   stall-seeking, the social pull, the wander and the round legs, because all of
   them are decided there.
2. **The verb** — `go_to` in `actions.rs`, both forms: `{"place_id": …}` and
   `{"person": …}`. The ladder guard does nothing about an LLM that simply
   decides to leave, and a confined actor who says "I am going to the well" and
   is not stopped is worse than one who never says it. Refuse it plainly so the
   model reads the refusal and stays in character.

It is the same state the player's commitment uses, and the same one an NPC
committed under M4b′ arrives in — the authored eight and tonight's arrest are
not two mechanisms, they are one flag with different histories. That is the sign
the design is right. Confined NPCs still speak, offer, accept, remember and
think normally; they simply have nowhere to go, which is the whole point.

### Sub-milestones

- **M5a — the building.** Geometry beside the belfry, collider, footprint
  export, nav rebake, a `pl_` id, an `areas.json` entry so the soundscape can
  bind a bed to it.
- **M5b — the cast moves in.** `confined` on the eight, both guards (the ladder
  one and the `go_to` one), their postings, and the guards (Tobin Marle, Ewart
  Rasp) and keeper (Ede Clove) stationed rather than wandering — M2 already
  points `workplaces["bailiff_and_gaoler"]` at the Bellstand, so this is mostly
  narrowing a posting that exists.
- **M5c — commitment.** Booking by description, `taken` confiscation, the
  posted fee, the diegetic sentence and the ceiling, the HUD line.
- **M5d — the doors.** Surety, visitors at the grate, the keeper's `release`,
  break-out wiring into M4d — all of them for the cast as well as the player, so
  an arrested townsman's kin can come and stand surety for them whether or not
  anyone is watching.

### Tests

- All eight `prisoner`-circumstance characters spawn inside the Stone House and
  are `confined`.
- An NPC committed by M4b′ lands in the same room in the same state, is released
  by the same paths, and returns to their round afterwards with their posting
  intact.
- A confined NPC with `thirst` under the well rung does **not** path to a
  cistern, does not take a round leg, and is not curfew-routed.
- A confined NPC's own `go_to` is refused — both `{"place_id": …}` and
  `{"person": …}` — while `say`, `offer_item`, `accept_item` and `remember` all
  still work.
- Committing the player confiscates exactly the notice's `taken` item and
  nothing else.
- The posted fee is a constant the keeper reports identically to every asker.
- Release fires at the next office bell, and at the 6-minute ceiling when the
  bell is further off than that.
- Surety: an unrelated NPC's `release` on a committed player is refused; the
  keeper's is honoured.
- Walking out past the keeper is an escape attempt and raises M4d's
  unanswerable notice.

## Suggested order

- **M0** — the accept/decline nudge (the bug; one call + test). **DONE
  2026-07-24**: `Engine::player_offer_reply` resolves the offerer from
  `World.offers` before the apply and hands them the ordinary priority slot;
  scheduler + engine tests pin that the handoff outranks an empty stage.
- **M1** — memory-guidance line against recording unpaid sales. **DONE
  2026-07-24**: turn.j2 now says an accepted offer is not yet a paid one —
  record the open debt until the price is really in `you_hold`. Fixtures
  regenerated.
- **M2** — stations and beats for the existing law cast. **DONE 2026-07-24**,
  rounds only as scoped: three sergeant beats (Havise Ashe west
  Wickmarket/Gradine, Jos Fitch east Gradine/Coswald's, Segwin Mott south
  Maren's Green/Tallage — dawn to curfew, curfew-exempt), five gate keepers
  standing Dayspring→Snuffing (Renn Skell at the Harne Gate per his own
  sheet; wall guards Renn Brant, Colm Vell, Colm Bram, Hamel Fenn posted to
  the Stone/Wool/River Gates and the Reed Postern — the doc's "named gate
  keepers" were aspirational, these are the chosen ones), Odo Trask at the
  toll-house counter, and the routeless rest of the watch anchored on the
  Bellstand watch-bell tower via `workplaces["bailiff_and_gaoler"]`.
- **M3** — ward notices. **DONE 2026-07-24**: `crates/cathedral-sim/src/notices.rs`.
  A law-cast actor (occupation in `LAW_OCCUPATIONS`) who judges a heard
  report credible uses the law-only `raise_notice` verb — prose
  about/deed/where the ward repeats, plus private accused/wronged ids for
  settlement. Carriers (law always; citizens diluted through a deterministic
  roll against `curiosity_of`) get an arrival percept and a standing
  `word_in_the_ward` sheet section; a law carrier entering hearing range of
  the accused gets a face-to-face percept once (`notices::confront`, engine
  poll), which is what makes the confrontation an idle turn the news gate
  admits. Restitution through the existing offer/accept — the accused handing
  the taking to the wronged, or paying any law officer — settles the word;
  otherwise it decays after `NOTICE_LIFE_GAME_DAYS` (20 game days). Non-law,
  notice-less prompts are byte-identical (golden fixtures unchanged).
  *(Superseded by M3.5: a transfer no longer settles anything by itself.)*
- **M3.5** — settlement as an act, not a side effect of any transfer (Problem
  4). **DONE 2026-07-26**: `settle_notice {"notice_id": N}` (`actions.rs`) is
  the law cast's and the wronged party's, per-notice, never a blanket clear;
  `notices::settle_on_transfer` is gone, replaced by `settle_on_return` (the
  named `taken` handed back to the wronged, and the player-as-wronged, the two
  settlements no verb can reach) and `restitution_candidates`, which settles
  nothing and instead earns the acceptor the "this may be what the word wants"
  percept plus a priority turn (`Engine::nudge_restitution_acceptor`, the M0
  accept-nudge argument from the other side of the exchange). `raise_notice`
  gained the optional `taken`; the sheet numbers its notices; the wronged always
  carry their own word. Bribery now expresses itself by omission: the sergeant
  takes the purse and simply does not call the verb.
- **M4** — taking hold (Problem 5, designed 2026-07-26, revised against the code
  2026-07-26, not started). The floor under refusal: summons → warrant →
  `seize` into custody → an escort to the *nearest* station → a real grab with a
  ~5 s struggle-out if you break the 8 m leash. Custody is a state and the grab
  only enforces it, so complying is simply walking there beside an LLM sergeant
  who can `release` you the whole way; the reflex is host-side code at 3 m and
  every judgement above it is a verb. Sub-milestones M4a–M4e; needs no new
  geometry. **The court is deliberately out** — the warrant comes from an
  ignored summons, not a bench — and the Stone House stays for grave matters
  only, whenever it gets built. The review pass settled five things the first
  draft got wrong: an officer must **close on foot** before they can take you
  (conversation reaches 20 m, so nothing about talking puts you in reach — the
  approach is the tell); `notices::confront` must **re-arm on every rung
  change** or the ladder stalls after one encounter; the tether clamps the
  **desired** position **before** the sweep, never the resolved one; the
  witnessed-breach door becomes *a notice this officer raised within the last
  game hour*, which is checkable from fields that already exist; and the grab
  reflex is **host-side**, telling the sim through commands, because a 10 Hz
  spatial feed cannot decide a 3 m radius. Gate refusal is cut — only two gates
  have leaves and they answer to the clock, not the law. **The law takes the
  cast too, not only the player** (M4b′): `raise_ward_notice_for` already brands
  NPCs who foul the street in front of a sergeant and nothing has ever come of
  it, and an NPC prisoner is far cheaper than a player one — the sim already
  owns their feet, so custody is an escort plus a `you_are_held` sheet section
  plus a `struggle` verb, with no host code at all. It needs a city-wide
  confinement cap so a bad-tempered sergeant cannot gaol the bread supply.
- **M5** — the Stone House (designed 2026-07-26, revised against the code
  2026-07-26, not started). The gaol already
  has a cast: eight `prisoner`-circumstance characters whose sheets say they are
  held on Stone House rations, currently spawned walking free because there is
  nowhere to hold them. Jail is not a lockout here — it is the one room in the
  city where a captive cast has nothing to do but talk to you — so it is built
  for staying, with a diegetic sentence, a posted 3-spark fee, and **surety** as
  the exit that quietly audits how you have treated the city. The eight are only
  the standing population — M4b′'s arrests give the cell traffic, so it is a room
  worth checking on rather than a diorama. Moved to the
  Bellstand under the watch-bell (lore resolved in `secular_government.md`: the
  river-gate house was condemned in the Hammering and the name outlived it), so
  the bell that counts your release rings over your head. `confined` is one
  state on both sides of the door. Sub-milestones M5a–M5d.
