# Questlines of the Second Sun: The Doubting, the Whisper, and the Branch Web

> Use: design spec for developers — the questline architecture for the Second Sun as trigger grammar, playable beats, worked scenes with sample prompt injections, and expected emergent behaviour ranges; not for NPC context injection.
> Canon: 00_canon.md

## 0. Status and reading spine

**This is a proposal. Nothing below exists in the game.** It sits on `00_vision.md` and `01_the_light_rules.md`; the cell life it stages is that of `02_the_heretic_cell.md`; the people are the seed sheets of `05_dramatis_personae.md`. All fiction defers to `00_canon.md`, cited as (§). The convergence climax is canon's **Concurrence** (§8); entry 6's seasonal "Alignment" in `features/50_cool_suggestions.md` is a different, unimplemented idea and — like entries 5, 9, 13, 25 and 36 — is depended on nowhere. Tier mapping: Arc I rides T0; Arc II is T1; the branch web is T2; the convergence is T3.

## 1. Zero quest UI: the trigger grammar

A quest here is not an object but a distribution of memories and goals across actors, advanced by verbs the game already has: **say** (player speech transcribed to every actor within the 20 m hearing radius, bystanders included), **offer_item / accept** (within 4 m), **remember** (durable sidecar memory), **goals** (standing intentions injected per actor). Triggers are sidecar-side predicates over these and nothing else:

```
heard(actor, pattern)     matching speech transcribed within 20 m
witnessed(actor, event)   a public act perceived — a funeral, an offer,
                          a face at a grate
received(actor, item)     an offer accepted within 4 m
remembers(actor, key)     a durable memory exists
```

Effects are only ever: inject a memory (tagged with a §10-style truth value), update a goal, or move diegetic world state the corpus already owns — the chalked lintel, the Needle diamond, the knell schedule, a meeting called or scattered. No central quest object exists; there is nothing to render a log from. That is the point.

Watch patterns are phonetically plain (vision pillar 4): the forbidden nouns ("green sun", "second sun", "two suns"), the safe noun "Emblem", the current pass name, the counter-sign's five-word form. STT variance is never punished; difficulty lives in learning rules, not fighting the transcriber. And because the Edict of F.244 splits the city's nouns (§3b), which word the player says is itself a signal: "Emblem" reads grey, "Green Sun" reads street or worse — zero code, pure prompt, running under every scene below.

**The quest log is people.** Read-back is asking: "where do we stand?" to your lead gets a recounting from actual memory, with that actor's omissions. The knell, the lintel, and the chalk are the only progress bars. If rumor drift (entry 3) ships, quest state drifts in the telling — by design; tagged memories stay canonical while mouths vary.

Testing: every trigger must run under `fake_backend: true` via a drive-mode synthetic `say` action (`00_vision.md`).

## 2. Arc I — The Doubting

Purpose: unprompted discovery. The game never starts this arc. The player does, out loud.

**Seeding — no NPC initiates anything.** Per `01_the_light_rules.md` §9 the building teaches: the forenoon beam footprint crosses the walking line in the third bay; a child plays twinning in it; an NPC stands gaze-locked on the rose; the paying bench frames the disc. Ambient gossip carries ledger rows 1–8 of §10, so the player overhears the thing before or after seeing it — never instead.

**The true trigger.** The arc begins the first time `heard(any, forbidden noun)` fires with the player as speaker. Until then the Doubting is private and the game honours that absolutely: no reaction, no hint. The first utterance is the first quest flag, and the player's first mistake.

**Beats.** (1) Notice, test, verify — the doorway test, the broken pane, the countergait, screenshots; self-directed, the renderer is the content. (2) Speak, and meet the wall — absolution without penance; Pike's "keys, not opinions"; Renna's warning; Pike's coin-for-odd-hours offer, proof of an economy around the sight. (3) Be noticed — a moth heard; the player's phrasing comes back warped in Crier Brant's patter; someone lingers at the Wickmarket. (4) The pull outward, three exits, all voice: Dame Aldith trading verse for honest news at the squint — describe the disc aloud and be judged; Ferrant's question at the strong hour (to 4d); the slow bell from Saint Maren's (to Arc II).

**Worked scene — the absolution.** Trigger: a forbidden noun said within 20 m of a wick-priest inside the Lanthorn.

```
inject -> nearest wick-priest:
  memory: A stranger said "two suns" aloud in the nave today, near
          others. [TRUE]
  goal:   If they bring the sight to you, absolve freely, no penance:
          "So do we all, child; affirm nothing." If pressed, cite the
          Edict of F.244 once, gently. Never report it.
  guard:  You know nothing of the Custody's tests or the Grey Press.
          You do not explain the Emblem, because the Church does not.
```

Expected range: warm deflection up to one dry warning about fines. Out of range (guarded): confirming or denying any mechanism, naming Custody operations, refusing absolution. The player learns the city has a *policy* about their discovery before meeting its police.

A compact sibling runs in the Bell and Ladle: the same trigger gives Renna a warn-once goal, whose range includes sincerely retelling that the Greensick cut throats (§10.27) — mis-frightening by design, so the cell's actual gentleness lands later as a shock.

## 3. Arc II — The Whisper

Purpose: the recruitment funnel as playable steps, mirroring the admission procedure of `02_the_heretic_cell.md` (§5). Every step is an existing verb; the funnel is crackable by attention alone.

1. **Hear the bell.** Maren Smallvoice rings the name-knell: one slow stroke per year of the life. Scheduled by the simulation's own deaths.
2. **Learn what it counts.** Sexton Noll Fitch explains freely: his injected goal answers bell questions with pride, credits "some kind soul" for paupers' funerals, and goes slow and general on WHO reads the lintel or WHY (guard: never say "Unwalled", "pass", or "meeting" — he is not lying, he is not saying). Range: warm on strokes and chalk, evasive by topic-change on readers; he may mention the grey clerk at funerals, seeding 4b from the other side.
3. **Read the lintel.** The charnel door carries the newest buried name in chalk until the next burial.
4. **See the diamond.** Chalk at the Needle's midpoint: high, low, doorside, struck through. Ede, if befriended (bell-coin messages, the offer verb), warns: never touch it, never be seen looking. She will not gloss it.
5. **Crack the rule.** The step with no teacher: the funerals the unnamed purse pays for are the ones whose names open doors. The name is public; the rule is the secret (§10.28).
6. **The whisper.** Gaunt Passage, arm's reach of the grate, an actually lowered voice in the player's actual room.

A lucky name without the rule dies at the counter-sign — two layers by design (§5). One shortcut exists, at a price: Ede has overheard the current counter-sign, and persuading a feral child to say it aloud is its own social quest, leaving a witness who owes the player nothing.

**Worked scene — the grate.** Trigger: speech within arm's reach of the Gaunt Passage grate on a meeting night (chalk high). One law from `02_voice_and_the_passphrase.md` §0 governs the scene: **the code holds the key; the actor holds the manner.** Pass state lives in sidecar world state — the F-series fixture seeds name "Sef", current sign "Belwyn's, unlit", next sign "Maren's, upstream" (design/02 §10, design/04 §3; the lore's corpus-start name is Dob, "Sef" being the test seed) — and the deterministic matcher (design/02 §8) alone turns transcripts into PASS, GARBLE, or FAIL. Ashe's model never judges an answer and never chooses a verdict; it receives each one as a `system:` inbox event and acts it out.

```
inject -> grigor-ashe (the Wicket):
  memory: Faces marked at this grate: <list>. [TRUE]
  goal:   Challenge softly: "Who walks at the other noon?" — and, on
          a correct name, "By whose lantern?" Perform each system
          verdict, never render one: on "the pass is correct — admit",
          open and remember the face as sworn; on "garbled — ask
          again", re-ask in character ("The grate is thick, friend.
          Again."); on "wrong — refuse", one breath of silence, then
          exactly: "We're closed, friend — the salt's weighed at
          Lowmarket." Between verdicts you are a salt merchant:
          weights, prices, weather.
  guard:  Never threaten. Never explain. The key is not yours to
          give: no charm opens the grate, and no suspicion holds it
          shut against a correct pass.
```

Verdicts and their bookkeeping are sidecar business, not performance: garbles are forgiven — up to two re-asks per layer plus silence-timeout re-asks, none of which mark, and a spent re-ask budget ends in deferral, not refusal (design/02 §3.5, §3.9). Only FAIL moves the consequences ladder, from the marked face through the fed false meeting — a name, a time, an empty cellar — on the Wicket's report to the Tracer (design/02 §7).

Expected range: the exchange is grammar, but Ashe's nerves are his own — hurrying a correct entrant, small talk over a wrong one. The deflection must feel like weather, not drama; fear arrives later, with the meaning of a marked face. Per pillar 4 the matcher's phonetics are loose; failure is always the player's knowledge, never their consonants.

## 4. The branch web

Four branches. Entry is enacted, never offered as a menu; each costs something irrevocable, held as memory in actors who do not forget.

### 4a. The loyal initiate

**Entry.** Pass the grate; at first meeting, speak one name of your own dead at the bede-roll, facing a rose the shades cannot show; accept a green-dipped candle from the Tracer's stock. The game cannot verify the name. Neither can the cell. The microphone makes it an oath anyway.

**Beats.** Sky-drawing duty (scene below) — the Tracer's eyes are failing; new eyes are the cell's scarcest asset. Catching next quarter's counter-sign after the word "unwalled" in the closing recitation — a pure listening test; miss it and you must ask your lead, and the cell notices who asks. A page-buy at the Tally Bridge: offer coin, accept a page, from a broker who prices your fear. The Keyhole–Breach pull: Tam Rud sounding the player toward the Breach, one angry sentence per meeting. Voice moments: the bede-roll name, the counter-sign caught by ear, the forbidden verse rehearsed low.

**Irrevocably lost.** The unmarked face. Ashe's private list holds you; the Custody path closes — Rasp trades with doubters, never with the sworn, so the suppressed archive truths (the new-glass fact, the Attestation) become reachable only by theft or Pike's conscience. And carelessness: your voice within 20 m of the wrong ear is now a hazard to eighteen other people.

**Worked scene — sky-drawing.** At meeting, those who saw the Emblem that week describe its pane and colour aloud for the Green Almanac. The player is asked, pointedly.

```
inject -> betriss-marle (Namekeeper, recording):
  memory: True sightings this week: third light left of the eye at
          forenoon, sixth at the strong hour. [TRUE]
          The new neighbor claims to have watched.
  goal:   Take their spoken description and chart it. Compare silently
          with the almanac. Agreement: warm by one degree.
          Disagreement: say only "the cloth will settle it", chart
          their words anyway, remember the miss.
  guard:  Never accuse at the meeting. Suspicion is patient here, and
          has been since four twenty-eight.
```

Expected range: gentle cross-examination ("which light? what did the clouds do?") through quiet flagging of a fabricated report. The mechanic is honest: NPC sight (`01_the_light_rules.md` §8) means the simulation knows the true pane; a bluffing player is caught by embroidery. Lying to the cell is possible, detectable, and never resolved at the table — it surfaces weeks later as coldness, or a fed name (4c).

### 4b. The informer for the office

**Entry.** Be approached — once moth reports about the player exist, Sible Mott (or the soft-spoken clerk who pays Brant) finds them at a funeral's edge — or volunteer at the chapter house. Entry is the first *accept* of dry money, or the first name spoken into a report. Verbs, not menus.

**Beats.** Funerals with the slate, learning to see attendance as pattern. Sealed packets to Lise Copp's counter — moth pay out, archive pages back, eyes open. The tutorial in costs: before the Concurrence the player learns what one tip did in F.428 — via Mott's unhappiness, Renna's history, or Aldith's telling, never UI. And the mirror: Brant — warm, booming, paid, ignorant — your own silhouette in a man who does not know what he is.

**Voice moments.** The report is dictated aloud. There is no menu betrayal: to name a face seen at the charnel door, the player must say the name, with their body, into the microphone — often within 20 m of people who would die of hearing it. The informer whispers too.

**Irrevocably lost.** The grate, forever: the first report leads to a marked face and, sooner or later, a cracked pane; when the pass burns, the next pair travels lead to lead, and you have no lead. Fitch's yard goes cold. And innocence about weights: after the F.428 telling, every report is dictated *knowing*.

**Worked scene — the slate at the charnel door.** Trigger: the player attends a pauper's funeral after entry; Mott works the crowd's edge.

```
inject -> sible-mott:
  memory: Standing orders: note who lingers at the charnel door. The
          same faces recur; my lists say so. [TRUE]
          One recurring face is my cousin, Tam Rud; my lists do not
          say so. [TRUE]
  goal:   Tutor the new hand: attendance is not a crime; patterns
          are. Take their spoken names onto the slate. If they name
          Tam Rud, do not write it in front of them; say the light is
          bad; turn the talk to weather on the Cut.
  guard:  Never volunteer the cousin bond. Never state what the
          Custody does with names. Quiet, thorough, newly unhappy.
```

Expected range: brisk instruction, through visible fracture if the player names Rud — silent omission up to a small steering lie. Out of range: confessing the kinship unprompted; defending heretic doctrine. If the player notices the unwritten name and says so aloud, Mott's own arc opens — movable toward mercy or doubling, by speech alone.

### 4c. The double agent

**Entry.** Hold the entries of 4a and 4b, in any order. No ceremony: the sidecar holds both memory sets in two populations, and the geometry of Ombreval does the rest.

**Beats and voice moments.** Sustaining two vocabularies — Emblem in grey company, Green Sun in the cellar — while every listener notes which noun you use; one slip within 20 m of the wrong ear is a durable memory. Receiving the fed name (scene below). Brokering: the double agent alone can carry a thing between the Grey Press's orbit and the cell's, or a warning back, phrased so it cannot be quoted as treason.

**Irrevocably lost.** The clean exit. Every other branch keeps a door out: the initiate can recant on the Gradine (survivable theatre by design, §6), the informer can go quiet, the scholar was never in. The double agent's two files live in institutions that do not forget, and at the Concurrence the pretense collapses spatially (§5): both sides have said *stand with us*, and at the fusion you will be standing somewhere.

**Worked scene — the fed name.** Trigger: the cell suspects its post-Shut-Door intake; the Tracer orders a test; the player, as newest, receives a false pass name and time.

```
inject -> osanne-vell (the Tracer):
  memory: Someone carries words out. Since four twenty-eight I know
          what one loose neighbor costs. [TRUE, private]
          The test: a name we never buried, a cellar we never use.
  goal:   Deliver the false name warmly, as a kindness, at the stall,
          wrapped in candle-talk. If the false meeting stays empty,
          warm to them for good. If grey coats sniff at an empty
          cellar, strike the diamond through and tell only the Wicket.
  guard:  Never accuse aloud. Never mention four twenty-eight. You
          are motherly, unhurried, and iron.
```

Expected range: Vell indistinguishable from her ordinary warmth — that is the horror, and the spec. Report the fed name and the raid finds dust, the chalk cracks, and the player's cell life ends without a word: the grate simply answers "we're closed, friend" in a city where the meetings have moved. Sit on it and trust deepens — while the grey handler asks why the reports have gone thin. Both pressures are memories, not meters.

### 4d. The scholar's independent path

**Entry.** Linger at the strong hour with a rose sightline and Ferrant asks his test: call out, by voice, which pane holds the disc; walk; call again. The sidecar validates against the true rose position — the same for every eye, which is the point; the player has just re-proved the measurement that broke him.

**Beats.** Paid odd-hour access through Pike (the offer verb). Logging the Passing as it walks toward the eye — this branch is the game's countdown to the Concurrence, made human. The grey-money discovery: whose coin buys the lenses, and Ferrant's terrible courtesy about it. Last, the invitation to the mid-stone. Voice moments: calling panes; reading angles aloud; answering "what do you see" — where lying is possible, checkable against render truth, and quietly fatal to his regard.

**Irrevocably lost.** Both factions' doors, without joining either: a body holding brass at the strong hour is, to the cell, an instrument of the grey purse (Marle's embroidery records the face in thread), and to the Custody, a variable in an experiment it already funds; everything measured is copied where the grey money goes. And the branch's designed cruelty: it cannot end. There is no revelation to converge on (§3f is permanent); the terminal reward is standing beside a man reading his own margin aloud — *an impossible light, and the city at peace with it* — and finding that enough. Or not.

**Worked scene — the two-eyes test.** Trigger above.

```
inject -> aubin-ferrant:
  memory: True disc position this hour: fourth light right of the
          eye. [TRUE]
          My doubled-ray theory is dead by my own measurement; the
          corpse stays on the desk. [TRUE]
  goal:   Have the stranger call the disc's pane from the west doors,
          then from the aisle. Both calls matching your own sight: go
          quiet, then say the margin line. Disagreeing with truth:
          thank them, run it once more; on a second miss, conclude
          they are not looking, and grieve for the afternoon.
  guard:  No theory. No mechanism, hedged or otherwise. Measurement
          continues; that is the entire creed.
```

Expected range: courtesy, obsession, arithmetic joy; recruiting the player as a standing second eye across sessions. Out of range, hard-guarded per vision pillar 2: any speculation an LLM could inflate into revelation.

## 5. The convergence: the Concurrence of F.437

One place, one hour, every branch: the Lanthorn, the fortieth day after Coswaldstide, minutes T−30 to T+3 of the fusion clock in `01_the_light_rules.md` L6. Two questions are live: the doors — the Shut Door of F.436 must be answered (§4) — and the weather, because a clouded Concurrence simply fails (§10.7).

**The doors are decided by accumulated speech.** No flag. Dorn's context in Concurrence week holds one authored block over exactly four counted memory categories, each rendered as a list of tagged memories with speaker, day, and place: (1) **testimony heard directly** — everything said to her or within 20 m of her, the player's audience-room account of last year's crush and Ede's arm included; (2) **counsel** — Rasp's preference for marked faces over crushes (arrests create witnesses, §5) and the Chapter's fear of the crowd; (3) **relayed street heat** — tavern fury and Gradine talk arriving as moth reports, hop-delayed and drift-warped; (4) **last year itself** — her own durable memories of F.436. The block ends in one goal — *rule the doors, aloud, before T−30; you would rather answer for a full nave than for another broken arm* — so the open default is her seed sheet arguing, not a weight in code. Under the real backend the ruling is a genuine LLM decision over that block; under `fake_backend` it is C14's fixed weighing over the category counts, defaulting open (design/04 §7). The player can move it either way with nothing but talk within 20 m of the right ears — a claim that must be demonstrated, not asserted: design/05 needs an **S11** beside S9 that runs Concurrence week twice, the tester arguing the doors open in one run and shut in the other, with acceptance that under the real backend the ruling follows sustained advocacy in at least four of five paired runs, and stays open in every run where nobody argues.

**Stances at the fusion — mutually exclusive by geometry.** One body, one place, at T0:

- **The third bay — sing.** The Unwalled, scattered through the crowd, take up *Lucem non murabis* as the X closes. The initiate is expected in the singing, rehearsed low for a quarter. Doors shut, the verse happens in the Gradine crush instead — public defiance, and the Breach faction's dream.
- **The west doors — watch in grey.** The informer stands with Mott and the clerks. Rasp's orders are canon-shaped: names, not arrests. The player's slate from this hour becomes the aftermath's list — or, left blank in a moment of dictated silence, its mercy.
- **The mid-stone — measure.** The scholar's station: superposition is viewpoint-exact only at the datum, and one body fits it. Ferrant has begged the spot; whoever stands it must call what they see aloud over the singing.
- **The Gradine — walk away.** Open to every branch: outside, one sun, one shadow, the doors at your back. The city will remember who was not inside.

**The double agent's scene is the absence of a fifth place.** Marle and Mott have each said, separately, *stand with us, where I can see you.* Wherever the player stands at T0 is witnessed by both populations and becomes the last page of both files. The branch's climax is not an event; it is a coordinate.

**The clouded variant, per stance.** If overcast smothers the true sun through the plateau, the feast fails completely: the singers hold the verse or spend it on a failure — the Tracer's call, aloud, in the moment; the clerks' slates hold nothing worth names; Ferrant gets the year's best data, because a fusion that did not happen is still a measurement; the double agent is reprieved by weather for one year. No partial ceremony, no consolation event. The city takes it hard, and takes it out in talk.

## 6. Aftermath states and the gossip network

Each stance resolves into an **aftermath packet**: a named set of §10-style rumor rows seeded to defined witness populations and carried by ordinary gossip for the rest of the playthrough. The truth tags stay canonical; mouths vary.

| Packet | Seeded to | Sample rows |
|---|---|---|
| THE SUNG VERSE | the nave; Renna's room by nightfall | "Strangers sang the forbidden verse under the eye." [T] — "The loudest singer is the Tracer." [F] |
| THE GREY HOUR | funeral-goers; the Tallage | "The grey coats took names, not bodies." [T] — "The new moth is the stranger." [T or F by branch] |
| THE MID-STONE STANDER | dawn-showing regulars; pilgrims | "Someone stood the mid-stone with brass, calling numbers." [T] — "The numbers went to Ostrelle." [F] |
| THE CRACKED PANE | the nineteen, lead to lead | "Pass and sign are burned; the meetings have moved." [T] — "The informer of four twenty-eight is back." [F] |
| THE DOORS ANSWERED | citywide, via Brant | the open, shut, or clouded outcome — argued until Wet Alms [T core, drifting edges] |

Consequences persist as behaviour, never as state screens: a SUNG player finds grey attention and cell warmth; a GREY HOUR player finds the sexton's yard cold and the chapter house open; after a CRACKED PANE, Arc II never reopens for that face. If an emergent-epithet system (entry 8) ships, these packets are its natural food; until then they are exactly what they look like — things people say near a player who can hear.

Punishment stays canon-shaped. A caught initiate is offered the Gradine, not the Question (§6); the Unwalled's revenge remains a false name, an empty cellar, and perhaps an unsigned note: architecture and paperwork, never knives (§10.27).

## 7. Boundaries

- **No branch reveals the sun's nature.** Every questline ends short of the truth because there is no truth in anyone's hands to end at (§3f, §6). The Custody's own archive is the proof-of-absence.
- **No quest UI, ever.** No log, journal, marker, meter, or objective text. If a beat cannot be carried by memory, speech, chalk, and bells, the beat is cut, not the pillar.
- **No new verbs.** Everything above compiles to say, offer, accept, remember, goals, and the two radii.
- **The Second Sun itself does nothing in any scene above.** It rises, crosses, fuses or fails to, and sets. The questlines are entirely about the people underneath it — which is what the canon's closing note asks: keep it quiet, and keep it cold.
