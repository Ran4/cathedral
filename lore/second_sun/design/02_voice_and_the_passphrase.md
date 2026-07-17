# The Voice and the Passphrase: Design of the Whisper at the Grate

> Use: design spec for developers — the microphone as the key to the door: the full grate exchange, whisper handling, bystander risk, rotation, failure ladders, accessibility, and deterministic test paths; nothing in this document is implemented.
> Canon: 00_canon.md

## 0. Status and scope

**This is a proposal. Nothing below exists in the game.** It turns canon §5 (the pass) and Pillar 4 of `00_vision.md` ("the microphone is a place in the world") into a buildable specification. Decisions canon is silent on are marked **[spec decision]** and bind later documents unless canon is amended.

One law governs everything: **the code holds the key; the actor holds the manner** **[spec decision]**. Canon §3a makes it objective fact that the Gaunt Passage grate opens only to the current pass and counter-sign, so admission is decided by a deterministic matcher in the authoritative sidecar, never by an LLM. Grigor Ashe's model performs — pauses, salt patter, deflection — but cannot be charmed open and cannot refuse a correct pass; it receives the verdict as a `system:` inbox event and acts it out.

## 1. Ground truth: the pipeline this rides on

- **Speech in.** Completed-utterance STT (cloud `gpt-4o-transcribe`, streaming with batch fallback, or local Canary-Qwen), ended by `stt_trailing_silence_ms` of quiet (default 500 ms, clamped 300–1500); the transcript enters the sidecar as ordinary player speech.
- **Hearing.** One authoritative recipient calculation: everyone within 20 m hears, bystanders included, full 3D distance, no occlusion; LLM recipients get prose inbox entries. There is no whisper radius and this spec adds none (§4).
- **Turn-taking.** NPC replies are real LLM calls on the round-robin scheduler; beats of silence at the grate are honest — nobody hurries a doorkeeper — and `pause_microphone_during_npc_voice: true` keeps the player off the challenge.
- **Latency budget [proposed scheduling rule].** "Honest silence" has a ceiling: at forty scheduled actors (design/04 §10) an unaided round-robin could park Ashe's reply for a minute, and every live beat this spec stages dies at a sixty-second doorkeeper. Rule: any actor in an **active exchange with the player** — challenged at the grate, or directly addressed within 20 m — takes top scheduler priority until the exchange lapses (no player line for ~30 s). Acceptance: challenge-to-reply at the grate under **5 seconds** on either real backend, measured as `grate_reply_latency` alongside `repeat_prompt_rate` in design/05 §3. One stated exception: during the Concurrence crowd spike, priority holds only for the named cast (extras already run thinned schedules, design/04 §10) — a slow doorkeeper on the feast of the year is fiction; on an ordinary night it is a defect.
- **Speech out.** The configured TTS backend (local streaming Pocket TTS, cloud, or off); NPC speech must always also surface as text (§9).
- **Authority.** Pass state — current name, current and next sign, cracked-pane flag, Ashe's mark list — lives in sidecar world state, injected into member prompts. Bevy only projects.
- **Offline.** `fake_backend: true` replaces cognition, STT, and TTS deterministically; every flow here must run there (§10).

## 2. The two layers, restated as data

Canon §5: the **name** is the given name of the most recently buried soul the cell's coin interred — public on Saint Maren's charnel door lintel, tolled on Maren Smallvoice; the *rule* is the secret. The **counter-sign** is quarterly: a saint's possessive plus a terse image, five plain words or fewer. Fixed liturgy:

1. Wicket: *"Who walks at the other noon?"*
2. Candidate: *"Old Sef walks."* (the fresh name)
3. Wicket: *"By whose lantern?"*
4. Candidate: *"Belwyn's, unlit."* (the current sign)

Both secrets are short and phonetically plain by canon design — the fiction already did the STT engineering — and neither leaked layer opens anything alone.

## 3. The flow at the grate, step by step

**Grate-range** is 2 m — canon's "arm's reach" **[spec decision]**. The protocol is site-independent (the Wicket travels with the diamond); the Gaunt grate is the reference implementation.

1. **Approach.** With a meeting sitting (sidecar schedule), three seconds within grate-range — or any word spoken there — wakes Ashe's turn: *someone is at the grate*.
2. **Vigilance.** Ashe's LLM decides whether to open the exchange or stay a salt merchant, prompted not to challenge while a stranger lingers in his perception — a trait, not a hard gate: jumpy, mercenary, sometimes hurried, canon needs him imperfect, for a whisper at a grate *can* be overheard. If no meeting sits there is no challenge at all: the cellar is a genuine salt business and Ashe answers with weights and prices **[spec decision — the cover is real commerce; probing on the wrong night teaches nothing]**.
3. **Challenge.** Ashe speaks line 1 — ordinary `say` speech: 20 m, bystanders and all. The cell's protection is geography and hour, not acoustics.
4. **Response window.** The player answers into the live microphone; the transcript goes to the deterministic matcher (§8): **PASS**, **GARBLE** (unintelligible or malformed), or **FAIL** (well-formed but wrong — always subtyped *wrong* or *stale*, which decides the rung in §7).
5. **Mishear etiquette.** On GARBLE, Ashe re-asks in character — *"The grate is thick, friend. Again."* — up to twice per layer per visit **[spec decision]**. If the microphone produced nothing (a whisper below the utterance detector), a ~8 s silence-timeout triggers the same re-ask **[spec decision]**: a doorkeeper who heard nothing says so. Garbles never advance any ladder.
6. **Second layer.** On a PASS name, Ashe speaks line 3 and the window repeats.
7. **Admission.** Both layers PASS: Ashe's inbox gets *the pass is correct — admit*, and the bar scrapes back (a world-state door change). Whoever spoke the words gets in — the grate cannot tell a cracked rule from a sworn neighbour; what happens inside is social, handled by the actors inside.
8. **Refusal.** On FAIL at either layer, the canonical beat: the grate silent a breath too long, then, mundane, *"We're closed, friend — the salt's weighed at Lowmarket."* The meeting drains out the far end and becomes people sheltering from weather. Ashe records the face: a durable memory plus his private list, sidecar state (canon: it may one day be merchandise).
9. **Deferral without offence.** If the re-ask budget is spent on garbles, Ashe defers — *"The wind is wrong tonight. Come again."* — no mark **[spec decision]**. A bad microphone night costs a walk, never a standing.

## 4. What counts as whispering — honest and approximate

Canon demands a *whisper*; the hearing model has no amplitude channel, and `00_vision.md` forbids amplitude-gated stealth (a trap for cheap microphones). Two designs; the second is normative.

**Honest tier (optional, flavour only).** Where the capture layer can cheaply expose mean utterance amplitude, the sidecar may take it as metadata. It never gates anything: Ashe's prompt learns *softly said* or *said aloud*, so his performance can approve — *"Quiet feet, neighbour"* — or wince at a shouted pass. Amplitude must never resize the 20 m radius (that is entry 12 of `features/50_cool_suggestions.md`, unimplemented) and must never alter a verdict **[spec decision]**.

**Fiction-preserving baseline (required).** Whispering is approximated by *proximity plus stakes*: grate-range is arm's reach, the exchange is short, and the player knows every word carries 20 m. Players lower their real voices unprompted — exactly the behaviour Pillar 4 wants. The one true hazard of a real whisper is technical: a voice below the utterance detector produces no transcript, and the silence-timeout re-ask (§3.5) exists so the answer to "you whispered too well" is the doorkeeper asking again, never a failure state.

## 5. The third ear: bystander risk

The core dramatic mechanic, and it costs zero new code: every line of the exchange — Ashe's challenge included — is ordinary speech, so anyone within 20 m receives it into memory.

- **The player as leaker.** Speak the pass with a grey clerk in the passage mouth and the Custody holds both layers — though the name goes stale at the next funeral, the cell's actuarial defence. An overheard pass is a wasting asset.
- **The player as thief.** Stand in Gaunt Passage while a genuine neighbour is admitted and harvest the exchange whole. Ashe's vigilance (§3.2) makes this cost patience and position — a Lowmarket crowd, a rain-shelter pretext, the dogleg's blind corner — not make it impossible. The eavesdropped route in is legitimate design: the grate opens to words.
- **The informant economy.** What a bystander does with a heard pass is actor judgment, not script: a neighbour keeps silence, a frightened citizen tells a wick-priest, a moth sells it through Lise Copp's pawnshop as dry-money work, Crier Jos Brant repeats crowd-talk without knowing what he is — ledger rows 24, 25, 27, 28 (canon §10), riding in the relevant prompts.
- **Ede of the Needle** holds the current counter-sign and not the rule (canon §7.13): the designed leak, the child-shaped path by which a stuck player buys half the key for a penny and some honesty.
- **The cell's counterplay** is simulated, not narrated: meetings sit where the diamond says; the Wicket delays under observation; repeated strange faces reach the Tracer, whose answer is §7's cracked pane.

## 6. Rotation as live content

Both secrets rotate on world events the player can attend, which is what makes the pass *content* rather than a lock.

**The name rotates on death.** When the sim buries a pauper on the cell's coin: Noll Fitch chalks the given name on the charnel door lintel (a texture swap — the lintel is the diegetic UI), Maren Smallvoice rings the name-knell (one stroke per year of the life), and the sidecar sets `pass_name` in the same transaction. Old Sef yields to Pin the day Pin goes into the ground. Members need no messenger — the rule plus a public lintel is the distribution — and the player who has cracked the rule reads the same lintel. Funeral cadence is a content dial: one burial per one to two in-game weeks keeps the name fresh without farce **[spec decision]**.

**The sign rotates on quarter-feasts.** At the first meeting after each quarter-feast (canon §8), the closing recitation carries next quarter's sign exactly once — *attend the words after "unwalled."* Mechanically: a real broadcast `say` inside the meeting; only those present receive it; LLM members must `remember` it; a player in the room learns it by literally listening and catching the phrase.

**Ritual lines [proposed — new machinery].** The law of §0 cuts both ways: an LLM never holds the key, so an LLM is never trusted to *speak* it verbatim either. Where exact words are load-bearing, the sidecar authors the line itself — a **scripted `say`**, issued on the speaking actor's ordinary scheduled turn *in place of* a model call, and written into the actor's own transcript as its own speech so the model owns the words retrospectively. The Namekeeper's closing recitation is the paradigm case: a fixed template with `next_counter_sign` substituted exactly once after "unwalled," so the sign-promotion transaction (design/04 §3) keys off deterministic sidecar text, never off model output. The same device stages the meeting's ordered rites: the sidecar hands each rite's officiant a scripted opening line on their turn, and free LLM turns carry the conversation between rites — the ceremony is a skeleton of scripted beats fleshed by ordinary cognition. Scripted NPC speech is machinery the current action model does not have and must be added to design/04's inventory as a new mechanism. Absentees are served lead-to-lead: each lead gets a sidecar goal — *bring the sign to your light* — which becomes genuine NPC-to-NPC street speech the player can shadow and overhear. Distribution is itself eavesdroppable content; the informant risk the cell runs is the one the player runs.

**Sign minting.** New signs follow canon's strict form (saint's possessive plus terse image, five plain words or fewer) from a curated pool **[spec decision: authored, not freely generated — every entry pre-tested against both STT backends; transcription-hostile saints like Vhairé are matched by image tokens alone (§8)]**.

**Name minting [spec decision — the sign rule, applied to the layer that needs it more].** The name is the layer the stale detector must discriminate *phonetically*, so it cannot come from an uncontrolled generator: canon's one-syllable small names (Pin, Ib, Cobb, Sef) collapse under a vowel-folded key (Ib/Ebb/"if"; Sef/"safe") into false PASSes and false stale-FAILs. The sidecar owns pauper naming: cell-paid burials draw their given names from the **same STT-pre-tested authored pool as the signs**, seeded from the corpus's own past-name lists, and a candidate is admitted to the pool only if its phonetic key keeps a minimum pairwise distance from the fresh name, everything in `pass_history`, and every uninterred registry name. Canon's small-name flavour lives in the pool's authorship, not in free generation. This moves design/05 F4's "re-mint any name that fails twice across testers" from playtest reaction to generation-time constraint — a name that fails pre-testing never enters the pool — which it must be, since a name is canonically fixed the moment it is chalked and tolled and can never be re-minted after the fact; the playtest rule remains only as a backstop for retiring a pool entry before its next use.

**The cracked pane.** On evidence of compromise — recurring stale-name attempts (§7), a mark returning with the current sign, a raid scare — the Tracer burns both secrets: the Needle's diamond is struck through, the grate answers only salt, and the next pair travels lead to lead. To the player: the city quietly changing its locks around you.

## 7. The consequences ladder

Rungs, from soft to hard. Only FAIL verdicts move the ladder; GARBLE never does.

| Rung | Trigger | What happens |
|---|---|---|
| 0 | Garbles, silence, spent re-asks | Re-ask, then deferral. No mark, ever. |
| 1 | First FAIL-wrong: a registry name the cell never buried, or an invented sign in valid form — including an invented sign spoken *beside a correct fresh name* | The breath of silence; the Lowmarket line; meeting drains; face marked (Ashe's memory + list). |
| 2 | FAIL-stale at the sign layer: fresh name, previous quarter's sign | Soft refusal, no mark on first occurrence **[spec decision]** — the signature of a lapsed neighbour, whom the cell's own protocol covers ("your lead brings it to you"). Second occurrence marks. |
| 3 | FAIL-stale at the name layer: a previous pass-name (`pass_history`) | Refusal, mark, and a report to the Namekeeper: someone is working from old intelligence, so the *rule* may have leaked. Recurring stale-name attempts from different faces trigger the cracked pane (§6) **[spec decision]**. |
| 4 | Second FAIL by a marked face, any later night | The feed: a false name, a false time, an empty cellar. Whether the darker canon rumour comes true — the unsigned note to the Custody — is decided by the Tracer's actor on the Wicket's report: Osanne Vell's model weighs it, keeping the cruellest move a character's choice and, under `fake_backend`, deterministic **[spec decision]**. |
| — | Right words, wrong night or site | Nothing answers. The words hang in the air with the full 20 m risk and no reward — the cheapest way players teach themselves caution. |

**No rung is permanent.** Marks decay one rung per quarter-feast **[spec decision]**, funerals reset the name, rotation reopens the lock: a player at rung 4 who waits a quarter, reads the lintel, and recovers the new sign (meeting, lead-shadowing, or Ede) is back at the grate. Nothing here can hard-lock the conspiracy shut; it can only cost attention, the game's currency.

## 8. The matcher: STT failure modes and forgiveness

Difficulty lives in learning the rule, never in fighting the transcriber (`00_vision.md`, non-goals). Known failure modes across both backends: homophone drift (Sef to "Seth" or "chef"); possessive elision ("Belwyn's, unlit" to "belwyns unlit" or "Belwyn is unlit"); truncation when a whisper dips under the trailing-silence detector; streaming partials superseded by batch results; vocabulary gaps on invented names.

**Matcher design [spec decision].** Deterministic, sidecar-side, on the normalised transcript (case-folded, punctuation stripped):

- **Name layer.** Exhaustive verdict table, tested in this order against each transcript token's phonetic key (vowel-folded, Soundex-strength, no more): a key match on the **fresh name** → **PASS** — "Seth walks" opens for Sef, correctly; a match on any **previous pass-name** (`pass_history`) → **FAIL-stale** (rung 3); a match on any **other burial-registry name the cell's coin never paid for** → **FAIL-wrong** (rung 1); **anything else** → **GARBLE**. Every FAIL carries its subtype, and the ladder (§7) consumes the subtype, never a bare FAIL. The name-minting constraint (§6) guarantees these three key sets never collide.
- **Sign layer.** PASS if the saint token matches phonetically and at least half the image tokens match, or if all image tokens match without the saint (rescuing transcription-hostile saints). FAIL only if the answer has the canonical shape — saint's possessive plus image — but resolves to a wrong sign: a **previous quarter's sign** → **FAIL-stale** (rung 2); an **invented sign** in valid form → **FAIL-wrong** (rung 1). Otherwise GARBLE.
- **The shape rule:** FAIL requires a confident, well-formed, wrong answer; everything else is GARBLE and forgiven. The ladder punishes guessers, never microphones.

Acceptance: a player who knows both secrets and can be transcribed *at all* is never marked; across 100 noisy transcripts of a correct exchange (both backends), zero may FAIL — GARBLE is tolerable, FAIL is a defect **[spec decision]**.

## 9. Accessibility and the no-mic path

STT is already an independent handshake capability and can be absent (no key, no model, or by choice). The fallback keeps every cost and loses only the larynx:

- **The speech line [spec decision].** With STT unavailable or disabled, a keypress opens a free-text line; the entered text becomes an ordinary player `say` at the player's position — same 20 m radius, same bystanders, same matcher, same ladder, same one chance to be wrong. Free text, never a dialogue wheel: the puzzle is knowing the words, and a menu that lists them is a spoiler engine. The degraded path, never the primary one (Pillar 4).
- **Deaf and hard-of-hearing play.** All NPC speech renders as text regardless of TTS backend — challenge, recitation, and the phrase after "unwalled" included, so the sign is learnable by reading. The name-knell's stroke count gets a visual echo in the bell's rendered swing; the lintel already carries the name in chalk. Saint Perrin Halfbell, who tuned bells by palm and jawbone, is the fiction's own permission for seen and felt bells.
- **Speech differences.** The phonetic-key matcher plus GARBLE-forgiveness is the accessibility mechanism for accented, dysarthric, or synthesised speech; §8's bar is measured against varied speakers, not one developer's voice.

## 10. The deterministic path: fake_backend

Under `fake_backend: true` every flow above runs offline, identically every time: the matcher is already deterministic; Ashe's performance comes from the scripted fake cast (fixed lines per verdict); the speech line (§9) and drive-mode injection (§11) carry input; pass state is seeded (`pass_name: "Sef"`, sign "Belwyn's, unlit", next sign fixed, `pass_history: ["Cobb", "Ib"]` per design/04, plus one never-cell-paid registry burial, `Ansel` — Ansel is the FAIL-wrong name for rung-1 tests, Cobb the FAIL-stale name for rung-3 tests); Vell's rung-4 decision follows a fixed rule. Test matrix, one integration test each in the engine suite (`cargo test -p cathedral-sim`; the Python sidecar this originally named was ported into `crates/cathedral-sim`):

F1 correct exchange opens the grate; F2 garbled name, re-ask, correct, opens; F3 empty utterance, timeout re-prompt, no ladder movement; F4 wrong name (Ansel), deflection line, mark recorded; F5 marked face returns wrong, receives the false feed; F6 stale name (Cobb), Namekeeper report; second stale face cracks the pane; F7 a bystander 15 m away holds both exchange lines in its inbox; F8 scripted funeral advances the name and yesterday's name now trips rung 3; F9 the quarter-feast meeting broadcasts the next sign once and absentees receive it lead-to-lead; F10 marks decay across a quarter-feast.

## 11. Drive-mode verification (proposals)

`CATHEDRAL_DRIVE` (see `.claude/rules/CATHEDRAL_DRIVE.md`) has no speech action today. This spec proposes two: **`say <text>`** — inject a synthetic player utterance through the sidecar's normal hearing path, bypassing audio (the shape `00_vision.md` anticipated); and **`goto <anchor>`** — teleport to a named dev anchor (`gaunt_grate`, `charnel_door`, `needle_mid`) **[proposed drive actions — none exist]**. Illustrations, assuming `fake_backend: true` and the F-series seed:

```sh
# P1 — happy path: challenge, name, sign, door opens
CATHEDRAL_DRIVE='wait-online; goto gaunt_grate; sleep 4; \
  say Old Sef walks; sleep 3; say Belwyns unlit; sleep 3; \
  shot grate_open; quit' cargo run

# P2 — forgiveness: garble, re-ask, recover (no mark)
CATHEDRAL_DRIVE='wait-online; goto gaunt_grate; sleep 4; \
  say old chef wax; sleep 3; say Old Sef walks; sleep 3; \
  say Belwyns unlit; sleep 3; shot grate_open_after_garble; quit' cargo run

# P3 — rung 1: wrong name (Ansel: buried in the registry, never on the
# cell's coin — Cobb would be a *previous pass-name* and trip rung 3
# instead), deflection, mark; verify via sidecar log
CATHEDRAL_DRIVE='wait-online; goto gaunt_grate; sleep 4; \
  say Ansel walks; sleep 4; shot grate_refused; quit' cargo run
# then: grep for the mark event in the sidecar transcript on stdout

# P4 — the third ear: seeded bystander in the passage hears both layers
CATHEDRAL_DRIVE='wait-online; goto gaunt_grate; sleep 4; \
  say Old Sef walks; sleep 3; say Belwyns unlit; sleep 3; \
  shot exchange_done; quit' cargo run
# assertion lives in the paired Python test (F7): the bystander inbox
```

Screenshots prove door state; marks and inbox contents are asserted in the paired Python tests. Drive scripts demonstrate; the offline suite proves.

## 12. Failure policy, summarised

Garbles never move the ladder; a correct pass opens for any speaker (canon §3a); the LLM cannot override the matcher; every rung has a route back (lintel, knell, meeting, lead, Ede); everything said within 20 m is heard, and usable.

The grate asks eight words and gives one door, one risk, and one rule worth cracking. Keep the matcher generous, the doorkeeper mundane, and the twenty metres honest.
