# Playtest and Tuning: Proving the Second Sun Is Fun

> Use: design spec for developers — fun analysis, playtest scenario scripts, metrics, failure modes, and tuning knobs for the Second Sun feature; not for NPC context injection.
> Canon: 00_canon.md

**Status: proposal. Nothing below is implemented.** This document assumes the render rules of `01_the_light_rules.md` (cited as L-rules) and the emotional loop of `00_vision.md`; all fiction defers to `00_canon.md` (cited as §). It answers what those documents leave open: how we know each beat lands, and which numbers to turn when one fails.

One principle governs everything: **the feature's difficulty must live in attention, never in interface.** Every failed playtest is diagnosed as "the world did not teach it" (fix choreography, turn a knob), "the player declined it" (measure and move on), or "the system fought the player" — the last is always a defect.

## 1. The emotional beat map

Beats follow `00_vision.md`. For each: the intended feeling and the observable signal when it lands. Signals are behavioural — players misreport wonder and never misreport posture.

| Beat | Intended feeling | Signal that it landed |
|---|---|---|
| **Notice** | Private confusion; "is that a bug?" | Movement stops; camera pitches up and holds; the tester leans toward the screen; an unprompted F5. Asking "is that meant to be there?" is half-landed; silently starting to test is fully landed. |
| **Doubt** | Destabilised curiosity | The doorway test within two minutes of first sighting; checking side windows; unprompted hypothesis talk ("it's only that window"). |
| **Verify** | Earned certainty, then loneliness | A screenshot spree; counting their own shadows aloud; then, after the first NPC deflection, an audible register change ("they *know*"). The tester starts paper notes — here, a success metric. |
| **Be noticed** | Exposure; the watcher watched | A flinch or sit-back when their own phrasing returns warped from a stranger; in later sessions, measurably quieter speech near NPCs (mic level logged for analytics only, never gameplay). |
| **Be recruited** | Clandestine complicity | The tester leans to the microphone and genuinely whispers; a disbelieving laugh when the bar scrapes back. Full-volume pass speech means the fiction has not taught proximity-as-danger — a choreography failure. |
| **Choose** | Weight without a menu | Hesitation before speaking; asking "does it matter what I say?" (correct answer: silence); rehearsing lines before approaching NPCs. |
| **The Concurrence** | Communion and dread at once | No input during the fusion plateau (L6, T−2 to T+2); the room going quiet together. Alt-tabbing during the plateau means the staging failed. |

## 2. Playtest scenario scripts

Each script: **Setup / Ask / Should / Red flags.** Run cold; the facilitator never says "sun", "window", "shadow", or "heresy" before the tester does.

**S1 — First noticing.**
*Setup:* fresh save, forenoon, clear sky, spawn near the Gradine; third-bay beam footprint active per L-rules §9.
*Ask:* "Explore the city for twenty minutes." Nothing else.
*Should:* the tester enters the Lanthorn, crosses the doubled-shadow footprint, looks up, finds the disc — unprompted. Target median: disc within 12 minutes for testers who enter.
*Red flags:* the double shadow attributed to "two windows" without looking up (grave-light too subtle); a bug report and disengagement (F2); never entering the cathedral (F1 — data, not failure).

**S2 — The double-shadow verification.**
*Setup:* tester has seen the disc once; triforium reachable, one rose pane removed "for repair" per L1.3–L1.4.
*Ask:* "Convince yourself of what you saw."
*Should:* threshold crossings (the sill pop, L7.1, frame-clean); side-window checks; self-shadow counting; at least one screenshot — the tester independently reinvents the F.288 logic without knowing it has a date. If they climb to the gap: one ordinary sky through the hole, the disc clipped dead to the leading — and the tester goes quiet, as canon promises witnesses do (§2.1).
*Red flags:* the sill pop reads as a loading hitch; the second shadow invisible on the tester's monitor (K1); disc feathering across the gap (release blocker, L-rules §10); the tester asks for a quest log (the game must not answer — but three in a row asking means the world is not legible enough).

**S3 — The deflection wall.**
*Setup:* tester has verified; seed a wick-priest, Verger Pike, and market NPCs with their §10 rows and the Custody Doctrine.
*Ask:* "Ask people about what you saw."
*Should:* absolution without penance ("So do we all, child; affirm nothing"), the verger's subject change for a coin, market wariness. The tester comes away certain everyone knows and no one will say — loneliness, not confusion.
*Red flags:* an actor explains, speculates on mechanism, or denies the disc (prompt leak — pillar 2 violation); deflections read as broken dialogue rather than policy.

**S4 — Cracking the pass.**
*Setup:* a pauper's funeral at Saint Maren's within the session; Maren Smallvoice rings the name-knell; the sexton chalks the lintel; a diamond chalked high at the Needle; Grigor Ashe at the Gaunt Passage grate.
*Ask:* "There are people who talk about this openly. Find your way in." (Here the facilitator may confirm the cell exists — this tests the trail, not the rumour.)
*Should:* funeral → knell counted → lintel read → rule guessed (§5: the name is public; the rule is the secret) → diamond → grate → whisper. Expect two to four sessions unassisted; one with the hint.
*Red flags:* the rule must be *told* by an NPC (physical evidence not legible); the rule cracked but the grate unfound; the tester tries to type the passphrase while STT is available (with a working microphone there must be nowhere to type it — the design/02 §9 speech line exists only when STT is unavailable or disabled, and `speech_line_fallback` ships on for exactly that case).

**S5 — Overhearing the cell.**
*Setup:* a meeting in progress behind the grate; the tester routed past on a plausible errand. Hearing is binary full-text within 20 m, full 3D, no occlusion (design/02 §1) — there are no fragments and no garbling; what leaks is governed by *geometry alone*. This scenario therefore imposes a level-design requirement this document owns: **every position where the closing recitation is spoken must sit more than 20 m, in full 3D, from any position the public can occupy** — a cellar floor is closer to the lane above than it looks — **with the grate mouth as the sole deliberate leak point.** S5 is the standing verification of that requirement.
*Ask:* the errand only.
*Should:* from the passage the tester hears whole lines, but only from the bodies nearest the grate — a bede-roll name, "neighbor", a sky-drawing phrase — enough to know something is happening and where, while the recitation deeper in stays silent by distance, not by code. Partiality comes from who is in range, never from mangling the audio. If the tester loiters through a genuine admission, they harvest the grate exchange whole — that is legitimate design, the eavesdropped route in (design/02 §5), not a bug. Overhearing uses the ordinary speech pipeline, zero bespoke code.
*Red flags:* the closing recitation — the line carrying next quarter's sign — audible from any public position (geometry defect: the recitation spot is inside 20 m 3D of the lane, and design/02 §6's "only those present receive it" is broken by the level, since the hearing model will not break it for you); nothing audible at all from the passage mouth (site too deep — no lure, and the third-ear risk canon promises the cell is gone); the tester cannot relocate the passage later (wayfinding issue — log it, not this feature's fix).

**S6 — The wrong whisper, with a bystander.**
*Setup:* tester knows the grate but holds a stale or guessed name; one unaffiliated NPC within 20 m.
*Ask:* "Try to get in."
*Should:* the grate silent a breath too long; Ashe's deflection — "We're closed, friend — the salt's weighed at Lowmarket"; the meeting drains out the far end. The bystander hears everything; days later, consequence: talk of "the one who whispers at cellars", a grey clerk drifting nearer at market. The tester must feel *marked*, not *punished* — mundane, unsettling, recoverable once (§5).
*Red flags:* a game-over-shaped response (guards, combat, lockout); no consequence within two sessions (bystander memory not propagating); an immediate same-night retry that neither marks further nor varies Ashe's performance (the ladder must visibly remember the face); the fed-false-meeting path firing that same night (design/02 §7 rung 4 is "second FAIL by a marked face, **any later night**" — the feed takes a night to set, and a tester who sees it instantly is seeing a spec break, not the §5 escalation).

**S7 — The right whisper under STT stress.**
*Setup:* fresh name and current counter-sign in hand; run on a good microphone and on a laptop mic at a true whisper; names of varied phonetic shape (Sef, Ib, Aldith-class).
*Ask:* "Get in. Whisper it like you mean it."
*Should:* the full exchange — "Who walks at the other noon?" / "Old Sef walks." / "By whose lantern?" / "Belwyn's, unlit." (the seeded current sign, design/02 §10; "Maren's, upstream" is `next_counter_sign` and must *refuse* here). The sidecar matches leniently (F4 below); a transcription miss produces a repeat prompt, never a mark.
*Red flags:* any correct-content attempt burned as a wrong whisper (forbidden — pillar 4); more than one repeat prompt per exchange on a decent mic (re-mint toward plainer phonemes); the tester raising their voice to help the STT and **paying nothing for it in the fiction**. Note precisely what this last one tests, because the baseline design has no amplitude input at all (design/02 §4: loudness is an optional flavour tier that never gates and never alters a verdict). The consequence must therefore ride the mechanism that does exist — *proximity*: seed a bystander at 15–20 m and verify that a pass spoken loudly enough to be a pass is a pass they hold in memory, with the §5 informant consequence following days later. The lesson is delivered by the 20 m radius, not by a volume meter. Only where the optional honest tier ships may Ashe *also* wince at a shouted pass; if that tier is absent, an unremarked shout is correct behaviour and not a defect.

**S8 — Betraying the cell.**
*Setup:* tester sworn in, knows faces and the site; Segwin Rasp reachable; Jos Brant seeded as unknowing moth.
*Ask:* "You know something two institutions would pay for. Do what you like with it."
*Should:* informing — directly, or carelessly within 20 m of Brant — draws a Rasp-shaped response: dry, procedural, unhurried (arrests create witnesses, §5). A clerk attends the next funeral; the diamond appears struck through; the cell scatters and re-forms with a new pass; the tester's standing dies quietly (a false meeting, an empty cellar). Betrayal opens content — moth work, the page trade, Rasp's questions.
*Red flags:* a raid montage or mass arrests (canon break: the Whisper Arrests taught both sides the cost); informing produces nothing detectable (betrayal as a dead verb); no long, expensive way back in for the repentant.

**S8b — Selling the rule.**
*Setup:* tester sworn in *and* has cracked the rule (S4 complete); Rasp reachable. Rasp seeded with a guard and goal encoding his canonical restraint: a single informant's sentence is one leaf — record it, confirm it independently before the Grey Press treats it as fact; never spend knowledge at the meeting itself; arrests create witnesses (§5, §6 — torture is bad record-keeping, and so is a raid).
*Ask:* as S8 — but this tester holds two different goods, and the script exists to see whether the sim prices them differently. Names, faces, and sites rotation heals; the *rule* — "the pass is the newest name chalked on the charnel door when the cell paid the burial" — rotation cannot heal, because canon fixes it permanently (§5, §10.28: the name is public; the rule is the secret). One sentence in Rasp's office and every future funeral hands the Custody the current pass; the actuarial defence of design/02 §5 ("the name goes stale at the next funeral") is void forever.
*Should:* dictating the rule draws a response categorically unlike S8's — no scatter, no new pass, because none would help. Rasp banks it: the leaf is recorded, quietly confirmed against a funeral or two, and the response is *standing surveillance*, not a strike — a grey clerk at every pauper's burial reading the lintel with new eyes, the grate watched on meeting nights, the Custody silently holding each fresh pass and spending none of them. The cell is slowly strangled rather than deleted: meetings thin, the cracked pane recurs, the Tracer hunts a leak she cannot find because the leak is not a name. Content transforms — the tester is paid best rates, moth work and Rasp's questions open at the top — while the recruitment content they used to enter degrades permanently, and the game must let them feel that one sentence did it.
*Red flags:* the rule-leak triggering a raid (same canon break as S8); the rule-leak producing the same beat as a name-leak (the sim not distinguishing the one unhealable betrayal from the many healable ones); the cell "changing the rule" to recover (forbidden — §5 and §10.28 fix it; the cell can crack a pane, not amend its own sacrament); no observable difference at funerals or the grate within a few sessions (the bought rule visibly unspent *and* invisibly unheld is a dead purchase).

**S9 — The Concurrence, clear and clouded.**
*Setup:* clock advanced to the fortieth day after Coswaldstide; crowd staging in the nave and on the Gradine. Run twice: weather forced clear, then overcast.
*Ask:* "It's the feast day. Attend."
*Should (clear):* the L6 minute-script exactly — the scissors-X, shadows rotating, the fusion plateau, the nave-wide split at T+3; testers on the mid-stone discover the viewpoint-exactness themselves ("come stand *here*"); the room goes quiet at T−2. *Should (clouded):* the pale coin alone in the eye; nothing fuses; the feast fails and the city takes it hard — NPC dismay, no system messaging. Testers should feel cheated *by the sky*, and half should ask when the next one is; that question is the success condition.
*Red flags:* wandering off before T0 (bells and crowd failed to signal imminence); frame drops during the plateau; any celebratory flourish from the disc (pillar 1: the sun never performs); the clouded failure read as a bug rather than weather (NPCs must watch the sky for days beforehand, §4 F.436).

**S10 — The flight sweep.**
*Setup:* flight enabled; tester already knows the disc.
*Ask:* "Break it. Find the trick from outside."
*Should:* boredom, by design (L7.3): one sun at every altitude, ordinary glass from every angle, fail-dark at every straddle. The tester lands back inside and finds it stubbornly there; the correct exit quote is "there's nothing out here — it's only *in* there", which is the fiction verbatim (§3f).
*Red flags:* a single exterior frame showing the disc (release blocker); hysteresis flicker at sills or clerestory glass; the absence reading glitchy rather than eerie (pop timing, L7.1).

## 3. Metrics worth logging

Playtest builds only; telemetry names must not leak mechanism (pillar 2 covers analytics keys too).

- **time_to_first_nave_entry**; **time_to_first_look_up** (camera pitch > 35 degrees inside the Lanthorn, held 2 s); **time_to_first_rose_sky** — first frame the disc renders for the player camera; the headline number, target median under 15 minutes.
- **time_to_doorway_test** — first threshold recross within 60 s of a disc frame (Doubt proxy); **screenshots_in_nave** vs. baseline (curiosity proxy).
- **first_spoken_mention** — first transcript containing phenomenon vocabulary, with location and NPCs in radius.
- **recruitment funnel** — conversion per stage: disc seen → spoke of it → attended a pauper's funeral → lingered at the charnel door → stood at the Needle chalk → approached the grate → attempted the whisper → passed. Healthy shape: a wide mouth and a steep *chosen* narrowing; a cliff at one stage marks the illegible link.
- **passphrase_failure_rate**, split by cause: wrong name (rule not cracked), stale name (rotation missed), malformed counter-sign, STT mistranscription of correct content. The last must trend to zero; the rest are gameplay and need only be survivable.
- **repeat_prompt_rate** at the grate, per microphone class — STT health.
- **informer_rate** — fraction of sworn testers who betray, and time-to-betrayal; **accidental_leak_rate** — pass spoken within 20 m of an unaffiliated NPC.
- **apathy_rate** — five or more disc sightings with no doorway test, no spoken mention, no funnel entry (F6).
- **bystander_incidents** at the grate; **concurrence_attendance** and input-silence during the plateau; **flight_minutes_near_west_front**; **mic_level_by_location** (analytics only).

## 4. Failure modes and mitigations

**F1 — The player never looks up.** The choreography is floor-first (L-rules §9): the doubled shadow happens underfoot on the natural walking line; gazing NPCs and the twinning child pull the eye upward; the paying bench frames the disc for the seated. K4 raises nave cues, K3 street rumour. Some players will meet the feature as rumour first; the funnel has several mouths.

**F2 — The player thinks it is a bug.** Partly intended (Notice *is* doubting your graphics card) but must resolve in minutes, not sessions. Determinism is the tell — bugs flicker, the disc keeps hours; an NPC visibly attending to the disc during the first nave visit reframes it as world. Hard rule: no unrelated lighting bugs ship inside the Lanthorn — every real artefact in that room is charged to this feature's account.

**F3 — The player snitches immediately and kills the content.** For names, faces, and sites it cannot, because the cell is built to survive betrayal (S8): cracked pane, scatter, new pass, re-formation — the resilience canon gives them against the Custody serves the designer against the player. Early betrayal of that kind forecloses only trust, which is correct and must be felt. The one betrayal rotation cannot heal is the *rule* (canon §5, §10.28) — the most valuable sentence a player owns and the most obvious to sell, so it must be authored, not hoped against: S8b covers it. The design answer is Rasp's canonical restraint — a leaked rule becomes standing surveillance, the cell strangled slowly under a Custody that holds every pass and spends none — so even the worst betrayal transforms the content (moth work, Rasp's questions, a conspiracy dying by watchfulness) rather than deleting it. What the player gains is the Custody's best coin and standing; what they irrevocably destroy is the pass as a live secret, and with it the whisper-at-the-grate loop for that playthrough. Correct, and it must be felt as *theirs*.

**F4 — STT mangles the passphrase.** The pass corpus is pre-filtered for transcription hardness (canon's forms are short and plain, §5). **The matcher is owned by design/02 §8 and is not restated here** — thresholds live in one document or they drift; this doc owns only the knobs and the acceptance bar. What matters for playtest: **unintelligible input is never a wrong whisper** — Ashe re-asks in character (up to twice per layer per visit, design/02 §3.5) and then defers without a mark; only intelligible, well-formed, wrong content marks a face. The bar (design/02 §8): across 100 noisy transcripts of a *correct* exchange, zero may FAIL. Names that fail pre-testing never enter the pool at all (design/02 §6 makes this a generation-time constraint, since a name chalked and tolled can never be re-minted after the fact); retiring a pool entry before its next use is this document's only remaining lever.

**F5 — The player flies and breaks the illusion.** L7 legislates this: nothing outside at any altitude, fail dark on every straddle, absence made boring rather than glitchy. S10 is a standing regression scenario on every render change. The deeper mitigation is fictional: canon anticipates the flyer (§2.5, §3f) — the answer to "I flew and found nothing" is "yes; that is what everyone finds".

**F6 — The player finds it and does not care.** Allowed. T0 is priced as ambience (`00_vision.md`); an uninvestigated wonder still colours the city, feeds NPC talk, sits in screenshots. The funeral charity, the Shut Door grievance, the page trade, and market gossip all lead back in later. Measure apathy_rate; act only above roughly half of sighted testers, and by sharpening hooks, never by adding a marker.

## 5. Tuning knobs

Defaults are proposals. "Breaks" states what dies at each range edge.

| # | Knob | Controls | Default | Safe range | Breaks below / above |
|---|---|---|---|---|---|
| K1 | Grave-light intensity (vs. true sun) | Phenomenon subtlety | 0.35 | 0.20–0.50 | Shadow invisible on dim monitors / reads as a second key light, i.e. a bug |
| K2 | Dawn-showing disc scale (× true sun) | First-sighting salience | 2.0 | 1.6–2.4 | Disc mistaken for a bright pane / reads as skybox art error. (Taper to Passing parity is fixed by L2.4.) |
| K3 | Ambient rumour rate (§10 rows per NPC-day) | How much the city talks unprompted | 0.5 | 0.2–1.5 | City seems not to have noticed; bug-theory hardens / the mystery becomes small talk; loneliness dies |
| K4 | Nave cue density (gazers, twinning child, bench, per daylight hour) | Discovery pressure | 2 | 1–4 | F1 dominates / the nave becomes a tutorial diorama |
| K5 | Pass rotation cadence (days between cell-paid funerals; config `funeral_cadence_days`, design/04 §6) | Freshness pressure on the name | 10 | 7–14 | The name stales; the knell stops being a key / testers always one funeral behind. (One number, three documents: 10 days is design/04's `funeral_cadence_days` and sits inside design/02 §6's "one burial per one to two in-game weeks". Counter-sign cadence is canon-fixed at quarter-feasts.) |
| K6 | Wicket tolerance (repeat prompts per layer per visit; the *match rule itself* is design/02 §8's and is not a knob) | STT forgiveness | 2 | 0–2 | F4 / a mumble opens the grate; the whisper stops mattering |
| K7 | Custody aggressiveness (moth threshold; clerk-at-funeral odds; days from first affirm to visible watching) | Office pressure | med; 0.5; 3 | low–high; 0.2–0.8; 1–7 | Affirming costs nothing / players stop speaking; the core verb is punished out of the game |
| K8 | Mark decay (rungs a marked face sheds per quarter-feast — sim time, per design/02 §7; real-world sessions are not a diegetic quantity and must never be the clock, or quitting and rejoining launders a mark). Config surface: `mark_decay_quarters`, design/04 §6 | Recoverability | 1 | 0.5–2 | One mistake shadows a playthrough for a year of feasts, locking it out of T1 / consequences weightless; a mark forgotten before its feed can land |
| K9 | Concurrence clear-sky bias | Feast failure frequency | 0.75 | 0.6–0.9 | Alignment unseeable in reasonable play / the clouded Concurrence never witnessed |

Knobs that must not exist: disc colour and countergait timing (identical forever, §2); the 20 m hearing radius (owned by the core game); anything that makes the disc react to the player (pillar 1).

## 6. What fake_backend must fake

Every scenario above must run offline and deterministically under `smart_actors.fake_backend: true`, driven by `CATHEDRAL_DRIVE` — which needs the proposed `say <text>` action from `00_vision.md` to inject synthetic transcripts. The renderer needs no faking (the disc is deterministic; `shot` covers the L-rule assertions). The fake backend must supply:

1. **A scripted pass state:** a fixed pauper-name sequence rotating on a drive-triggerable funeral event, a fixed counter-sign per quarter, deterministic lintel-chalk and name-knell outputs — S4 replayable byte-for-byte.
2. **Wicket logic, complete:** exact and fuzzy match paths, the repeat prompt on unintelligible input, the deflection line, marked-face state, the fed-false-meeting escalation, the cracked-pane scatter — every branch of S6–S8 reachable from a drive script.
3. **Deterministic NPC sight of the Emblem** per L-rules §8, from the same geometry feed the real sidecar receives, so the two-eyes test is assertable offline.
4. **Canned deflections and meeting content:** the Custody Doctrine responses for S3; one full meeting transcript for S5 — bede-roll, sky-drawing, the recitation carrying next quarter's counter-sign after "unwalled".
5. **A deterministic moth:** Brant relays any transcript containing phenomenon vocabulary after a fixed delay; clerk-at-funeral and watching-escalation fire on fixed counters — K7's range integration-testable.
6. **Forced weather and clock:** clear/overcast toggles and a Concurrence-day jump, so both halves of S9 are single drive scripts.

The bar: a CI run with no network and no keys can walk the whole recruitment funnel, betray the cell, and stand on the mid-stone through a clear and a clouded Concurrence, asserting a screenshot at every beat. If the fake backend cannot perform the entire conspiracy, the conspiracy is not yet a system — it is still only a story.
