# The Sound of the City: Bells, Cries, and the Audible Simulation

> Use: design spec for developers — every canonical sound of Ombreval consolidated as asset, meaning, trigger, range, and the sidecar heard-event it emits; the bell layer specified as game mechanics; not for NPC context injection.
> Canon: 00_canon.md

## 0. Status and scope

**This is a proposal. Nothing below is implemented.** It sits on `02_voice_and_the_passphrase.md` (the name-knell as pass-rotation clock), `04_systems_integration.md` (the rotation transaction and event stream; cited as design/04), and `03_questlines.md` §1 (the `heard` predicate). All fiction defers to `00_canon.md` (§) and the bell canon of `11_glossary_and_naming.md` and `07_what_everyone_knows.md`; **[spec decision]** and **[proposed]** tags as in the siblings.

Three laws govern everything here:

1. **The sidecar owns the cause; Bevy owns the playback.** Every sound below is scheduled by authoritative sidecar state and rides the snapshot/event stream, as the knell already does in design/04 §3. The Rust side is a speaker, never a clock.
2. **Percept, never meaning.** A heard-event injects only what an ear gets — *"you hear the name-knell from Saint Maren's: seventeen strokes."* What it *means* lives in standing lore (`07_what_everyone_knows.md` already teaches every actor the bell code). The sidecar never explains a bell to anyone.
3. **The Second Sun is silent.** Canon §2 is visual-only; vision pillar 1 forbids a whispering audio bed. §9 makes this testable.

## 1. The bronze layer: why bells are mechanics

Speech dies at 20 m; a bell crosses the map — Ombreval's one broadcast channel, and canon has already spent it as mechanism: the name-knell of Maren Smallvoice **is** the pass-rotation clock (§5, §8 — "the sound of a key turning"), its stroke count **is** the dead's age, and the player is expected to count it by ear (design/05 S4). The offices tell the hour, the Scold the law, the Ruin the calamity. None of it is ambience: a player who learns the code navigates the day, the dead, and the conspiracy without one line of UI — vision pillar 3, delivered in bronze.

**Patterns are data, not recordings [spec decision].** The store holds one stroke sample per bell (§6); every peal, toll, knell, and alarm is assembled at runtime as an ordered stroke sequence with intervals. Consequences, each load-bearing: the knell's count is a number in the funeral transaction, never a baked file; the offices are countable because count is data; and the Ruin — the ring rung backward — is *literally the same sequence reversed*, so fire and feast can never drift apart in tuning. Assembled peals get 20–40 ms of seeded jitter per stroke, so bronze sounds pulled by hands, not by a sequencer **[spec decision]**.

## 2. The consolidated table

Ranges are 3D radii, no occlusion, matching the hearing model (design/02 §1); the 20 m speech radius is the baseline. The heard-event column is the load-bearing one: the percept class injected into NPC context (§5).

| Sound | Source | Diegetic meaning | Trigger | Range | Heard-event percept (class) |
|---|---|---|---|---|---|
| The seven offices | **Perrin Evenblow** (Lanthorn) | The Candor's hours, Watch through Snuffing | Sim clock, seven times daily | citywide | none — updates the scene-header clock only (§5) |
| Full ring | all six of the ring, the Orphan included | The Concurrence and the quarter-feasts | Calendar event | citywide | "the full ring from the Lanthorn — the Orphan among them" |
| Great toll | **Great Ambrelle, called Gravemouth** | Bellday's great mass; the great dead | Calendar; death of rank | citywide | "Gravemouth tolling, slow, unnumbered" |
| The Ruin | the ring, backward, greatest first | Fire or flood. Drop everything | Calamity event (dev-triggered only, §7) | citywide | "the ring rung backward — the Ruin" (at onset) |
| Alarm | **Coswald Ironthroat** | A local fire or breach; help to one quarter | Calamity event, local | citywide | "Ironthroat ringing fast from the Lanthorn" (at onset) |
| Fog and flood | **Vhairé Farcall** | A report from the off-map Serle wharves: river fog or rising water | Weather state | citywide, strongest in the south wards | "Farcall for the outer wharves" / "Farcall — the river beyond the wall is rising" |
| The court | **Gaudry Truetongue** | The Praelucent's court; Gradine recantations | Court event | 400 m | "Truetongue: the court is sitting" |
| The name-knell | **Maren Smallvoice** (Saint Maren's) | A funeral; one slow stroke per year of the life | Burial transaction (design/04 §3) | 300 m | "the name-knell from Saint Maren's: N strokes" (at completion) |
| Curfew | **The Scold** (Bellstand) | The Snuffing curfew, in law | Sim clock, after the seventh office | 500 m | "the Scold ringing curfew" |
| Summons | **The Scold** | Gather the square; a proclamation follows | Proclamation event | 500 m | "the Scold gathering the Bellstand" |
| The wick-call | lamplighters' whistle, three notes | The shape on the ladder is lawful | Lighting round, each corner, dusk | 40 m | "the lamplighters' wick-call nearby" |
| Crier's opening | Jos Brant's handbell and "Oyez!" | A crying begins; come close to hear it | Crier schedule; after a Scold summons | 80 m | "the crier's bell and his Oyez from the Bellstand" |
| Crier's words | Jos Brant, speaking | The edict or news itself | Ordinary `say` on his turn | **20 m** | ordinary speech transcript, as today |
| The boatmen's song | *The Water Knows One*, the Hungry Ox | Boatmen at their pots; louder when grey coats pass | Tavern occupied, evening | 30 m spill | "low singing from the Hungry Ox: the water knows one, boys" |
| The forbidden verse | ***Lucem non murabis***, choral | The Unwalled at the Concurrence Passing | T3 staging (design/03 §5) | the nave; 100 m of the Gradine | "the crowd in the third bay sings: Lucem non murabis" |
| The rehearsed verse | the same, low | Meeting rite (canon §5) | Scripted `say` in the meeting | 20 m | ordinary speech transcript |
| The Second Sun | — | — | — | — | **none, ever** (§9) |

## 3. Pattern specifications

**The offices [spec decision].** Each office is rung as its ordinal: the Watch one stroke, the Kindling two, Dayspring three, High Wick four, the Waning five, Lamplight six, the Snuffing seven, at 3 s intervals. A player anywhere in the city learns the hour by counting — the ear-skill the knell demands, taught seven times a day for free.

**The full ring [spec decision].** Youngest voice to greatest — Truetongue, Farcall, Evenblow, the Orphan, Ironthroat, Gravemouth — rounds of six, five minutes at feast dawn. The Orphan rings **only** inside the full ring, never alone: folk distrust of the nameless bell (no one weds under it; Belwyn's ward is made when it rings) stays consistent because the only days it speaks are days everyone is already braced.

**The Ruin.** The full-ring sequence exactly reversed: Gravemouth first, recognisable on the first stroke, so its percept fires at onset (§5). Ironthroat alone is a burning house; the Ruin is a burning quarter **[spec decision]** — the difference between help wanted and run.

**The name-knell.** Strokes equal the years of the life, interval `knell_stroke_interval_s` (default 3.0 — slow enough to count). Scheduled inside the same sidecar transaction that updates `burial_registry`, `lintel_name`, and `pass_name` (design/04 §3): the bell and the lock cannot disagree, which is the trust model of design/02 §6. The percept fires at completion, count included — an NPC's ears are text, and text arrives counted; the *player's* ears are their own, which is design/05 S4's puzzle and must never be shortcut by any HUD.

**Gravemouth's toll.** For the great dead: slow, unnumbered, one stroke to a long breath, until the body is in the choir. It does not count years **[spec decision]** — counting is Smallvoice's office, and keeping them distinct protects the knell's meaning as the pass clock.

**The Scold.** Its curfew ring is the *legal* Snuffing, following Evenblow's seventh office — the office is prayer, the Scold is law, and the minutes between them are the city's dusk grace **[spec decision]**. Its summons pattern precedes every crying and Gradine proclamation.

**Farcall.** Long intervals report fog at the off-map Serle wharves; a faster pattern reports rising water beyond the south wall. Boat crews use it outside the playable city, while citizens hear only a material warning about delayed freight and prices. It never changes Saint Maren's crypt or any in-city geometry, and its playback must not imply an accessible canal.

## 4. The human layer

**The wick-call.** Three whistled notes at each corner of the lighting round (`08_folk_culture.md`): the lawful-shape signal. One wav, one percept class, deduped hard (§5) — dusk gets a texture, and an actor seeing a ladder-shape at a window can reason "no wick-call heard" like a citizen.

**The crier.** Brant's cry is two sounds with two ranges, and the split is the design: the handbell-and-Oyez opener is game audio at 80 m — a summons — while the words are his ordinary `say` at 20 m. "Gather in" is literal: the hearing model forces the crowd to close distance for content, which is the fiction exactly, and no speech-radius exception is created (design/04 §5 forbids resizing).

**The boatmen's song.** A low-mixed wav loop from the Hungry Ox on occupied evenings; boatman actors may also sing single verses as scripted `say` when grey coats pass (the canonical behaviour), giving hearers a real transcript. The wav is texture; the `say` is content.

**The forbidden verse.** At meetings: rehearsed low, a scripted `say` (design/02 §6's ritual-line machinery), 20 m, overhearable — that risk is canon. At the Concurrence: the crowd-scale singing is a choral wav (extras cannot all TTS), *paired* with a percept carrying the words to everyone in range. **Rule [spec decision]: any wav that carries words ships with a sidecar percept containing those words** — wav for ears, text for actors and deaf play. Named cast still sing via scripted `say` so their part enters neighbours' memories attributably (the initiate "expected in the singing", design/03 §5).

## 5. The heard-event

One new sidecar event type **[proposed]**, authoritative and bridge-visible:

```
{ type: "sound", id, class: "bell" | "signal" | "cry" | "song",
  asset_pattern: [ {bell: "smallvoice", strokes: 17, interval_s: 3.0}, ... ],
  site: "saint_marens", pos: [x, y, z], audible_m: 300,
  percept: "you hear the name-knell from Saint Maren's: seventeen strokes",
  percept_at: "completion" | "onset" }
```

Bevy consumes `asset_pattern` and `pos` for positional playback (and the rendered bell swing, design/02 §9); the sidecar computes recipients from `audible_m` and injects `percept` transiently. Rules:

- **No new calls.** A percept rides the recipient's next scheduled turn in `since_your_last_turn` — a citywide ring is forty lines of prompt text, zero extra turns (design/04 §10's cost model, honoured).
- **Onset vs completion.** Alarms (the Ruin, Ironthroat) inject at onset — actors must react mid-ring. Counted patterns inject at completion; a count does not exist until it ends.
- **The offices are a clock, not events.** Seven percepts per actor per day would be token waste: Evenblow instead updates the scene-header time-of-day every actor already receives ("the last office rung was the Waning"). Only *deviations* from the daily round are events.
- **Dedupe by class.** At most the latest percept per class per day; ambient classes (wick-call, song, crier opener) inject once per actor per evening **[spec decision]**. No inbox silts up with corner-whistles.
- **Quests hear bells.** The `heard(actor, pattern)` predicate of design/03 §1 matches sound percepts as well as speech, so `heard(any, "the Ruin")` is a valid trigger with no second mechanism.
- **Player audio hygiene.** Bell playback ducks 6 dB while the player is mid-utterance at the grate **[spec decision]** — presentation only, protecting cheap open mics from speaker bleed; never touching verdicts or radii.

Config, additive under `smart_actors.second_sun` (design/04 §6) **[proposed]**: `sounds: ( enabled: true, bell_time_scale: 1.0, knell_stroke_interval_s: 3.0, ambient_dedupe_per_day: 1 )`.

## 6. The wav asset list

`lore/AGENTS.md` invites wav lore; proposed home: `lore/claude/second_sun/sounds/` **[proposed]**. One stroke per bell; patterns assembled (§1).

- `bell_<name>_stroke.wav`, eight of them — `gravemouth` (bourdon, longest decay), `ironthroat` (hard, brazen), `farcall` (carrying, plaintive), `evenblow` (even, mild), `truetongue` (bright, young — cast F.365), `orphan` (slightly off-true against the ring **[spec decision]**: folk unease, earned by ear), `smallvoice` (small, clear — the knell's unit), `scold` (cracked, secular, unlovely).
- `wick_call.wav` — three whistled notes.
- `crier_handbell_oyez.wav` — handbell shake plus Brant's canonical "Oyez, oyez, neighbors, gather in!"
- `song_water_knows_one.wav` — one verse and chorus, low male voices, tavern-muffled loop.
- `verse_lucem_non_murabis.wav` — choral, unison swelling to parts; T3 only.

No asset exists for the Ruin, any peal, or any knell: sequences, not files. Nothing in this list — and nothing ever added to `sounds/` — may be triggered by, synchronised to, or mixed against the phenomenon (§9).

## 7. Determinism under fake_backend

Everything above must run offline, byte-for-byte (design/04 §7). Seeds, extending the F-series world of design/02 §10:

- **The seeded next burial is Pin, dead at seventeen [spec decision]** — `event funeral_next` (design/04 §8) deterministically chalks *Pin*, sets `pass_name: "Pin"`, and schedules a knell of exactly **17 strokes**: fast enough for CI, honest as a count.
- `bell_time_scale` compresses intervals under drive (0.1 makes the knell ~5 s); the *count* never changes, only the clock **[spec decision]**.
- New drive events **[proposed]**, `fake_backend` only: `event ruin` and `event proclamation`. The Ruin has no simulation source yet — no fire or flood system exists — so it ships as pattern-plus-dev-trigger, honestly labelled.
- Every stroke and percept prints an evidence line under drive (`[bell] smallvoice 3/17`, `[percept] <actor>: you hear...`) so scripts assert counts and recipients from stdout **[proposed]**.

Offline tests (B-series, one each in the Python suite): B1 the funeral knell counts 17 and the percept says seventeen; B2 recipients honour `audible_m` (an actor 350 m from Saint Maren's gets nothing); B3 the Ruin is the full ring reversed, injected at onset; B4 offices update the scene clock, no percepts; B5 the crier's opener reaches 60 m while his words stop at 20 m; B6 dedupe holds under three wick-calls; B7 the Concurrence verse wav pairs with its worded percept; B8 zero sound events originate from any render-layer trigger.

## 8. Drive-mode verification

Continuing design/02 §11 (P1–P4) and design/04 §8 (P5–P9); assumes `fake_backend: true` and the seeds above. One new dev anchor: `nave_crossing` **[proposed]**.

```sh
# P10 — the knell counts true, and the city hears it counted
CATHEDRAL_DRIVE='wait-online; goto charnel_door; shot lintel_before; \
  event funeral_next; sleep 8; shot lintel_after; quit' cargo run
# assert: 17 `[bell] smallvoice` lines; bystander percept says "seventeen
# strokes"; lintel texture now reads Pin (rotation already proven in P6)

# P11 — summons at 80 m, words at 20 m
CATHEDRAL_DRIVE='wait-online; goto tally_bridge; event proclamation; \
  sleep 4; shot heard_bell_not_words; goto bellstand; sleep 6; \
  shot heard_words; quit' cargo run
# assert: at the bridge, only the Scold and opener percepts; Brant's
# edict text reaches only inboxes within 20 m

# P12 — the silent sun (the negative test)
CATHEDRAL_DRIVE='wait-online; goto nave_crossing; event weather clear; \
  sleep 10; shot passing_silent; quit' cargo run
# assert: zero `type: "sound"` events across the Passing; the only
# permitted noise sources are people and bronze
```

The Ruin's backward order and onset percept are covered offline by B3 (`event ruin` plus the stroke log). Drive scripts demonstrate; the B-series proves.

## 9. Forbidden: the sound of the Second Sun

There is none, and there must never be one. Canon §2 is purely visual; vision pillar 1 forbids reaction and flourish. Banned: any hum, drone, or tone tied to the disc, the grave-shadow, the beams, the Passing, or the fusion; any sting on first sighting or on entering the beam; any mix change or filter keyed to rose-sightline; any "reversed bell" easter egg gesturing at the countergait. The Concurrence's audio is entirely human and bronze — the full ring, the crowd, the verse — and at the fusion the specified sound is whatever the people present make, which in a well-staged run (design/05 §1) is silence. P12 and B8 keep this enforceable: a sound event whose cause traces to the render layer is a release blocker, the audio equivalent of design/05 S3's prompt leak.

## 10. Accessibility

Every mechanical sound has a non-audio twin: the knell's count echoes in the rendered swing (design/02 §9) and lands permanently as chalk on the lintel; the offices show in light and lamp state; percept text serves any future subtitle surface, since the sidecar already authors every sound as a sentence. Saint Perrin Halfbell, who tuned bells by palm and jawbone, is the fiction's own warrant that a bell seen and felt is a bell heard.

The summary an engineer can carry out: one event type, one stroke sample per bell, patterns as data, percepts as sentences, meanings left where canon put them — in what everyone knows. The city gets a voice that carries past the Needle, the conspiracy keeps its clock, and the thing in the window keeps its terrible quiet.
