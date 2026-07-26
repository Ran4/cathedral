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
- **M4** (someday) — the stone: arrest, the gaol cast, bench days. Not before
  M3 has proven the loop is fun.
