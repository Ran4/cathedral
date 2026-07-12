# The Second Sun — Feature Vision

> Use: design spec for developers — player fantasy, pillars, emotional loop, scope tiers, and non-goals for the Second Sun as a feature; not for NPC context injection.
> Canon: 00_canon.md

**This is a proposal. Nothing described here is implemented; the Second Sun (entry 1) does not yet exist in the game.**

Vision document for entry 1 in `features/50_cool_suggestions.md`. All fiction defers to the canon bible; section references (§) are into `00_canon.md`. Related unimplemented ideas (entries 5, 6, 9, 13, 25, 36 there) are nodded to where natural and depended on nowhere.

## The player fantasy

You noticed something the game never pointed at. Standing in the nave of the Lanthorn you glanced up through the Great Rose and there were two suns, one of them the wrong colour, and when you stepped outside to check, the sky was ordinary. Nobody told you to look. No marker flared. You tested it yourself — screenshots, doorways, the doubled shadow at your own feet — and then you said something about it out loud, near people, with your actual voice, and the city heard you. Now a chandler watches you at market, a funeral bell has started to sound like a key turning, and a grate in a covered passage will open if you can work out what to whisper into your real microphone. You are not the chosen one. You are the newest person to notice, in a city that has been carefully not-noticing for two hundred years, and both of the institutions built on that silence would like a word.

## Design pillars

**1. The sun never performs.**
Demands: the phenomenon is a render-layer fact and nothing else (§2) — visually perfect, verifiable by eye and F5 screenshot, identical on every visit for every player forever. Every observable in §2 — the wrong green-white, the unfiltered colour through stained panes, the countergait, the no-parallax, the double shadows, the beams that do not mix — is enforced by the renderer, not by narrative claim.
Forbids: any reaction to the player. No pulsing when approached, no whispering audio bed, no gameplay buff from standing in the beam, no particle flourish at quest beats, no boss form. It carries no warmth (wax does not soften in it) and it carries no mechanics. The moment the sun does something, the dread dies. Its entire power is that it is merely, stubbornly there.

**2. Nobody is ever told the truth.**
Demands: the designed ambiguity of §3f is a binding constraint on every system we build — dialogue, quests, documents, endings, achievements, debug output. The Custody's archive proves only what the sun is *not*. The heretics are wrong on purpose. The player can learn everything both factions know and still not know what it is.
Forbids: an NPC who knows; a findable document that explains; a quest reward of revelation; a sidecar prompt that leaks a mechanism to an LLM actor (actors must not be *able* to spill a truth we never authored); and the softer failure — implying a truth through art direction, achievement names, or file names a datajunkie will decompile. The phenomenon has no explanation *anywhere*, including in our own code comments.

**3. Knowledge lives in voices and stone, not menus.**
Demands: every fact the player needs is diegetic and already representable — chalk on a lintel, a bell counting years, a rhyme children skip to, a name spoken at a grate, an NPC's durable memory. The rumour ledger (§10) with its truth values is the feature's real database, injected as sidecar context.
Forbids: a quest log, a journal, a codex that "unlocks", objective markers, a faction reputation bar, any HUD element at all. If the player wants a record, that is what F5 screenshots and their own notes are for — the game already treats screenshots as the player's survey journal, and this feature leans into it. If a beat cannot be communicated through the world and the actors, the beat is redesigned, not the UI.

**4. The microphone is a place in the world.**
Demands: the player's real voice is the only door into the conspiracy, and it is a *located* voice — heard by every actor within the 20 m radius, bystanders included. The pass exchange at the Gaunt Passage grate (§5) is deliberately built for this: short, phonetically plain, kind to speech-to-text; a given name plus a five-word counter-sign in a strict mintable form. Whispering at arm's reach of the grate must feel physically furtive because it *is* — the player lowers their actual voice in their actual room.
Forbids: a dialogue-wheel fallback that trivialises the exchange; passphrase entry via text box as a primary path; voice-print gimmicks; punishing STT variance with instant burn (the Wicket's deflection line and the marked-face escalation in §5 are the *designed* failure path — mundane, unsettling, recoverable once). Any accessibility fallback must preserve the costs: proximity, bystanders, one chance to get it wrong.

## Why this is the identity feature

The game's three unique assets are an always-open microphone, LLM actors with durable memory, and a renderer whose subject is impossible light. Every other candidate feature uses one or two. This one requires all three, load-bearing, simultaneously:

- **Impossible light** provides a mystery no scripted game can offer honestly: the evidence is *actually in the renderer*. The player does not take the game's word that shadows fall two ways — they stand in the nave and count their own. The broken-pane rule, the countergait, the Passing: all real, all screenshottable, none narrated.
- **LLM actors** provide a society that can *respond to noticing*. The Custody Doctrine ("to see is no sin; to affirm is", §3b) is a prompt, not a cutscene — a wick-priest actor genuinely absolves the sight and genuinely stiffens at the affirmation. Two hundred years of institutional not-saying can be performed by actors who each hold their own slice of §10 and their own reasons.
- **The open mic** makes the player's noticing *audible*. In any other game, discovering a secret is private. Here, saying "second sun" within 20 m of the wrong stall is an event that enters memories and travels. The whisper at the grate is the inverse: the one time the game asks you to speak *quietly*, and means it.

No other game can do this because no other game has a hot microphone wired into a candlelit heresy. That sentence is the pitch.

## The core emotional loop

**Notice** — *private confusion.* The player sees two suns through the rose and half-assumes a render bug. This is intended and precious: the first witness experience is indistinguishable from doubting your own graphics card. Nothing acknowledges the sighting. Do not tutorialise this beat; protect it.

**Doubt** — *destabilised curiosity.* The player self-tests: step outside (gone, §2.5), other windows (nothing), back inside (there). They are re-deriving the Custody's two centuries of method without knowing it. The design goal is that the player invents the F.288 experiment on their own before ever hearing it existed.

**Verify** — *earned certainty, and loneliness.* Screenshots. The double shadow. Watching the Kiss lay its X down the nave. And then the social wall: every NPC deflects. The priest absolves without penance. The verger changes the subject for a coin. The player now *knows*, provably, and discovers that knowing is not the same as being allowed to say. This loneliness is the emotional fuel for everything after.

**Be noticed** — *exposure.* Somewhere in the verify phase the player has spoken. A moth heard, or a neighbour did; either way, someone lingers near them at the Wickmarket, or their own phrasing comes back warped in a crier's patter. The feeling flips from "I am watching the city" to "the city was watching me watch". This beat costs no content beyond memory injection and the existing hearing radius — it is emergent, which is why it lands.

**Be recruited** — *initiation and complicity.* The trail is authored but unmarked: a pauper's funeral, a bell striking once per year of a life, a name chalked on the charnel door, a diamond chalked at the Needle, a grate in Gaunt Passage. The player cracks the rule (§5: the name is public; the rule is the secret) and whispers into a real microphone in a dark cellar — the specific thrill of doing something clandestine with your body, not your cursor.

**Choose** — *weight without a menu.* Keyhole or Breach, Custody or cell, or the long walk away — and over it all the F.437 Concurrence and the unanswered doors question (§4, the Shut Door). The choice is never presented; it is enacted, by where the player stands, what they carry, and what they say aloud, to whom, within earshot of whom.

The loop then re-enters at **Be noticed** from the other side: the player is now somebody *other* players of the social simulation — the Custody, the cell, the moths — notice, doubt, verify, and recruit against.

## Layering onto existing systems — no new UI

- **Renderer.** One additional sun disc composited only where the sightline passes through rose-glass to sky, from interior positions; one additional cold directional light valid only within the Lanthorn interior volume, giving the grave-shadow and the contradictory godrays. Countergait is a mirrored solar ephemeris. The no-parallax (§2.6) falls out of rendering the disc in glass-space rather than sky-space. All deterministic, all screenshot-stable.
- **STT and the 20 m radius.** The pass exchange is ordinary speech to the sidecar — the counter-sign's mintable form ("saint's possessive plus terse image, five plain words or fewer") exists precisely to survive transcription. No amplitude detection required at any tier; arm's-reach proximity to the grate plus content suffices. Overhearing risk needs zero new code: the grate exchange is speech like any other.
- **Actor memory and goals.** Cell membership, suspicion, sightings, the current pass and counter-sign are durable memories in the authoritative sidecar. The §10 ledger rows are prompt fodder with truth values, exactly what `lore/AGENTS.md` says lore is for.
- **Items within 4 m.** Pilgrim badges, green-dipped candles, bell-coins, forged and genuine Sparr pages — the contraband economy uses the existing offer/accept verb unchanged. A Sparr page changing hands at the Tally Bridge is the item system plus context, nothing more.
- **Audio and calendar.** The name-knell of Maren Smallvoice and the chalked lintel are the pass-rotation "UI": one audio cue and one texture swap, both diegetic. Quarter-feasts already live in the calendar hooks (§8).
- **Testing.** `fake_backend: true` must cover the full grate exchange deterministically, which requires a drive-mode path for injecting a synthetic transcript (a small `say <text>` action in `CATHEDRAL_DRIVE` is the likely shape). The render facts of §2 are screenshot-assertable via the existing drive `shot` action.

## Scope tiers

**T0 — the illusion (minimum shippable).** The §2 render package: disc through rose glass only, green-white, countergait, double shadows indoors, gone outside, dawn-showing through to the daily Passing. Plus roughly a dozen ledger rows (§10, statements 1–8) injected as ambient rumour context so actors can gossip about it. No cell, no quest. *Effort:* one renderer feature, one light rig, prompt-pack text. *Proves:* the sight alone generates player conversation and screenshots; actors discussing a thing the renderer actually does is already unlike any other game.

**T1 — the cell and the pass.** Saint Maren's charnel door with sim-driven chalk; the name-knell; the Needle's chalk diamond states; the Gaunt Passage grate; Grigor Ashe as Wicket with the full §5 exchange, deflection line, and marked-face escalation; one meeting scene (bede-roll, sky-drawing, the recitation carrying next quarter's counter-sign). *Effort:* a handful of placed props and states, one interior, three to five seeded actors, sidecar pass logic. *Proves:* the microphone-as-key loop end to end — that a rule-based, uncracked-by-UI puzzle (attend the funeral, read the lintel, learn what the knell means) is solvable by attention alone, and that STT holds up under whispering players.

**T2 — the church answers.** The Custody as an active force: Rasp's office, Pike's odd-hour keys, Brant as unknowing moth, the grey clerk at funerals; moth reports generated from actually-overheard player speech; a Gradine recantation as public theatre; the page trade through Lise Copp; two or three questlines (recover an alleged Sparr page; find the moth; assist Ferrant's measurements and watch his theory die again). *Effort:* the big tier — a dozen seeded actors from §7, quest content, economy tuning. *Proves:* two institutions reacting to the same open microphone create stakes without any karma system; the rumour network can carry §10's truth values under adversarial pressure.

**T3 — the alignment event.** The Concurrence of F.437 (§2.8, §8): the Passing centred in the eye, every doubled shadow in the nave fusing for a handful of minutes; crowd assembly on the Gradine; the doors question as a live, influenceable situation; the Unwalled's forbidden verse sung in the nave; the clouded-Concurrence failure honoured — if the weather system says overcast, the feast simply fails, and the city takes it hard. *Effort:* one scheduled citywide event, crowd staging, weather coupling, choral audio. *Proves:* the game can stage its loudest moment with no cutscene and no UI, and hold Pillar 2 — the mystery survives its own spotlight — under maximum player scrutiny.

Each tier ships whole. T0 without T1 is still a wonder; T1 without T2 is still a conspiracy; nothing in a lower tier assumes a higher one exists.

## Non-goals

- **Explaining the sun.** Not in T3, not in DLC, not in a director's commentary. §3f is permanent.
- **Combat or violent resolution.** The Unwalled do not kill (§10.27); the Custody prefers records to bodies. Betrayal, deflection, and architecture-as-punishment are the whole threat model.
- **Faction meters, reputation bars, join-screens.** Standing is held in actor memories or nowhere.
- **Extending the phenomenon.** No beams landing three streets away (that is ledger row 30, an unconfirmed street tale, and stays one), no night appearances, no second moon. Nothing else doubles (§2.10). Entry 5's wandering-light survey is a different feature and must not bleed in.
- **Voice biometrics or amplitude-gated stealth.** Tempting, out of scope, and a trap for players with cheap microphones.
- **Making the pass hard for the transcriber instead of the player.** Difficulty lives in learning the rule, never in fighting STT.

## What makes it fun

Not "atmosphere". These moments:

- **The doorway test.** Leaning your camera across the west door's threshold to watch a sun blink in and out of existence. Every player invents this within a minute of noticing, and the game rewards it with perfect consistency. Fun = a hypothesis confirmed by your own hands.
- **The broken pane.** Climbing to the triforium to look through the one gap where a pane is missing and seeing a single ordinary sky through the hole (§2.1). Canon says everyone who tests it goes quiet afterward. Playtesters do too. Fun = a twist delivered by geometry.
- **Counting the knell.** The moment the slow bell stops meaning "someone died" and starts meaning "the password changed" — and you catch yourself counting strokes to guess an age. Fun = re-perceiving an ambient sound as a mechanism, forever.
- **The whisper.** Leaning toward your own microphone at midnight, housemates asleep, actually whispering "Old Sef walks" at a grate, and hearing the bar scrape back. Fun = your body doing the verb.
- **Hearing yourself come back wrong.** A joke made in the Wickmarket returns days later from a stranger on the Tally Bridge, warped into something with teeth, and you can triangulate it back to whoever heard you. Fun = detective work against your own past voice.
- **Feeding the moth.** Suspecting an informer, handing them a false name and an empty cellar, and watching who arrives. The cell's own method (§5), turned by the player. Fun = a sting operation built from nothing but speech, memory, and patience.
- **Standing in the crossing at the strong hour** beside actors who are praying, haggling, and taking notes in grey, while two suns share one window above all of you — knowing which of your neighbours knows, and who is pretending not to. Fun = dramatic irony you earned, in a crowd that is actually thinking.

The through-line: every one of these is the player acting on the world with attention, voice, and position — the three inputs this game owns — and the world visibly, durably answering.
