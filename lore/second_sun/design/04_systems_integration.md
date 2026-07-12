# Systems Integration: The Second Sun on the Existing Machine

> Use: design spec for developers — how the Second Sun rides the shipped architecture end to end (lore injection, rumor drift, pass state, items, TTS, config, fake_backend, drive scripts, failure isolation, cost); not for NPC context injection.
> Canon: 00_canon.md

## 0. Status and thesis

**This is a proposal. Nothing below is implemented.** The claim it exists to make an engineer believe is narrower than "one new subsystem," and honest for being narrower: tiers T0 through T2 of `00_vision.md` add **no new player verbs and no new UI**. The player's whole kit stays walking, speaking, offering, looking. The build inventory, counted rather than rounded down: the render feature owned by `01_the_light_rules.md`; the deterministic matcher owned by `02_voice_and_the_passphrase.md`; and, on the sidecar and bridge, the additions the sections below propose and tag — a lore renderer with trigger predicates (design/03 §1's heard / witnessed / received / remembers; `witnessed` has no existing event source), two injection classes, budgets, drop order, and literacy gating (§1); rumor-row linkage with TTL freeze, hop budgets, and dedupe (§2); the `world.second_sun` block, its transactional rotation, and three Bevy projections — lintel texture, Needle decal, knell audio (§3); item fields (§4); the `manner` wire field and its DSP treatment (§5); a config block (§6); the C-series fake cast (§7); three drive actions, dev anchors, and the scheduled world events they fire — funerals, quarter-feasts, weather, the Concurrence (§8). Each carries its own **[proposed]** tag below; an engineer scoping the work should count the tags, not this paragraph. What remains true and load-bearing: every one of those additions is *data and events flowing through machinery that already ships* — the authoritative Python sidecar and its world state, the version-1 JSON-lines bridge, `WorldMirror` reconciliation with resync-on-gap, the say / offer_item / accept / remember / goal verbs, the 20 m hearing and 4 m offer radii, the three-capability handshake (LLM cognition, player STT, NPC voices — each degrading independently), `fake_backend`, and `CATHEDRAL_DRIVE`. No new processes, no new protocol version, no new call types. This document is the wiring diagram. Decisions canon is silent on are marked **[spec decision]**; proposed additions to shipped surfaces are marked **[proposed]**.

One constraint governs every section: **prompts carry only what an actor may say aloud.** Vision pillar 2 forbids an NPC who *can* spill an unauthored truth, so `00_canon.md` itself — which holds the suppressed facts, the ambiguity list, and the design frame — is never injected anywhere, into anyone, under any trigger. Only per-actor extracts flow, and the extraction is the security model.

## 1. The lore-injection map

The corpus is the feature's database; the sidecar's prompt renderer is its query engine. Two injection classes **[spec decision]**:

- **Standing** — rendered into the character sheet every turn (like `backstory` today). Cheap to reason about, paid on every call.
- **Transient** — rendered once, into `since_your_last_turn` or a scene block, when a trigger fires. Paid only when used.

Four trigger kinds: **seed** (world creation, like `knows` today), **location** (actor at or near a named site), **event** (a sim event: meeting sitting, funeral, feast), **in-hand** (a readable item held or offered within 4 m).

| Document | Audience | Trigger | Budget (tokens) |
|---|---|---|---|
| `07_what_everyone_knows.md` (the digest; the file *is* the compressed form, authored to ~1,100 words and injectable verbatim) | every LLM actor | seed | 300, standing |
| `06_rumor_pool.md` (2–6 rows, picked by district, faction, temperament) | every LLM actor | seed, plus ambient tick at `ambient_rumor_rate` | 150, standing |
| `05_dramatis_personae.md` (own block; one-line know-of digests for acquaintances **[proposed — the per-NPC one-liners are not written; the file carries full blocks only]**) | each named NPC | seed | 250 + ~20 per acquaintance, standing |
| `01_cosmology_and_doctrine.md` (catechism extract / folk-practice extract) | clergy and the devout / market and street actors | seed | 200, standing |
| `02_the_heretic_cell.md` (member extract; role extracts for Wicket, Namekeeper, Tracer) | the sworn nineteen only | seed; procedures refreshed on meeting nights (event) | 300 member / 450 role, standing |
| `03_the_church_and_the_office.md` (office extract; the Custody Doctrine paragraph alone for wick-priests) | Custody staff, Dorn / wick-priests | seed | 300 / 80, standing |
| `04_chronicle_of_the_city.md` (living-memory rows, F.419 onward / deep chronicle) | every adult / Ferrant, Rasp, Dorn, Aldith, Renna | seed | 150 / 400, standing |
| `09_rose_window_iconography.md` | Jonet Sparr, Pike, Ferrant, pilgrim-facing vergers | seed | 250, standing |
| `10_gazetteer_of_the_second_sun.md` (entry for the current site only) | whoever stands there | location | 120, transient |
| `11_glossary_and_naming.md` | no one — tooling reference for minting and the fake cast | never | 0 |
| `documents/edict_of_the_undivided_light.md` | Crier Brant (he cries it); anyone, as a posted readable prop at the Bellstand | seed / in-hand | 200, transient |
| `documents/heretic_catechism.md` | meeting attendees (read aloud); anyone, in-hand | event / in-hand | 250, transient |
| `documents/the_sparr_deposition.md`, `documents/trial_records.md`, `documents/letters_on_the_doubled_shadow.md` | holder or offeree only | in-hand | 200–400, transient |
| `documents/sermon_for_the_feast.md` | the preaching wick-priest | event (feast) | 300, transient |
| `00_canon.md`, everything in `design/` | no one, ever | never | 0 |

**Budget rule [spec decision].** Standing lore per prompt is capped at `standing_lore_budget_tokens` (default 1,200); transient at 500. Over budget, drop in fixed order: gazetteer entry, rumor rows beyond the first two, chronicle depth, iconography.

**Digests and extracts are an open work item [proposed — owner unassigned].** The principle is settled: every digest and extract is *authored once, per document, as part of the corpus* — never summarised at runtime, because a runtime summariser is a prompt-leak surface (§0's extraction-is-the-security-model rule). The state today is honest about itself: `07_what_everyone_knows.md` already **is** the digest of its row and needs no further work; every other row marked "extract", "digest", or "own block" above is a *proposal*, and the token budgets beside them are targets, not measurements — the full-size files exceed them. Writing those extracts (a `digests/` subfolder, one file per map row, each within its stated budget) is the first task of implementing this map, and no row may be injected in full-size form as a substitute.

**Literacy rule [spec decision].** In-hand injection renders `lore_text` only for literate actors. An illiterate actor (Lise Copp, canonically) receives a physical description instead — seal, hand, wear, the seller's manner. Canon's best joke about her becomes an injection rule, free.

## 2. Rumor drift on the verbs that exist

There is no rumor system. A rumor is a memory an actor happens to speak, and drift is what an LLM does to a paraphrase for free.

**Data shape [spec decision].** A rumor row lives twice: canonically in the sidecar — `{row_id, canon_text, truth_value, ttl_hops}`, truth values straight from canon §10 via `06_rumor_pool.md` — and per-actor as an ordinary durable memory holding that actor's *surface text*, tagged with the `row_id`. The tag is sidecar metadata; it is stripped from all speech and never rendered as something an actor can mention.

**Propagation, step by step.**

1. Seeding at world creation per the map above: each actor's rumor memories start as lightly varied phrasings of their rows.
2. A retelling is an ordinary `say` on the actor's ordinary turn — usually because conversation drifted there, occasionally because the scheduler handed the actor a one-shot goal ("share something you heard lately") at `ambient_rumor_rate`. No new verb, no broadcast channel.
3. Every hearer within 20 m gets the prose inbox entry the pipeline already produces. If a hearer's model chooses `remember`, the sidecar links the new memory to the speaker's `row_id` when the retelling is attributable (same turn window, token overlap) — best-effort, used only for caps and analytics; the fiction needs no linkage at all.
4. **Truth never drifts.** `truth_value` rides the sidecar row, not the mouth. "He gave the beggar bread" can become "he is a nobleman in disguise" at the surface while the underlying row stays what it is. This is exactly what `lore/AGENTS.md` asks rumor lists to be.
5. **Drift is bounded.** After `rumor_ttl_hops` linked retellings (default 3), the row freezes: further tellings are spoken from memory verbatim and new hearers store the frozen surface. Drift stays recognisable, canon-safe, and cheap.
6. **Dedupe.** An actor already holding a `row_id` refreshes, never duplicates — killing the echo storm where two neighbours retell each other the same row forever.

The player is a first-class node with zero extra code: player speech is transcribed into every 20 m hearer's inbox today. A player's phrase can enter the pool as an untagged row the moment one actor remembers it — which is the entire "Be noticed" beat of `00_vision.md`, emergent.

## 3. The pass in world state

The grate exchange itself is specified in `02_voice_and_the_passphrase.md`; here is where its state lives. One block in authoritative sidecar world state **[proposed]**:

```
world.second_sun = {
  pass_name:         "Sef",
  pass_history:      ["Cobb", "Ib", ...],       # the stale detector's registry
  counter_sign:      "Belwyn's, unlit",
  next_counter_sign: "Gaudry's, sealed brass",  # freshly minted at quarter-feast, spoken
                                                # once; no sign returns within the
                                                # Namekeeper's living memory (lore 02 sec. 5)
  burial_registry:   [...],                     # all recent burials, cell-paid flagged
  lintel_name:       "Sef",                     # what the chalk says right now
  diamond:           "high",                    # high | low | doorside | struck | none
  marks:             { char_id: rung },         # the Wicket's ladder, per face
  meeting:           { site: "gaunt", sitting: false },
}
```

**Rotation is transactional.** A cell-paid funeral event updates `burial_registry`, `lintel_name`, and `pass_name`, and schedules the name-knell (strokes = years of the life) in one sidecar transaction — the lintel, the bell, and the lock can never disagree. The quarter-feast meeting promotes `next_counter_sign` to `counter_sign` in the same tick as the closing-recitation broadcast `say` that carries it.

**Bevy only projects.** `lintel_name` and `diamond` ride the snapshot; the Rust side maps them to a chalk texture swap and a decal at the Needle's midpoint; the knell is an audio event stream. Because the whole block rides authoritative snapshots, a sequence gap and resync *cannot* desynchronise the chalk from the lock — the existing reconciliation model is the correctness proof, not a risk.

**Visibility.** Sworn members' prompts render the pass fields; everyone else gets nothing — Ede included. Her overheard counter-sign (canon §7.13) is **not** a render of the live field: it is one ordinary durable memory of one specific sign — "Belwyn's, unlit", heard once from her hiding place — and it goes stale on rotation exactly like a lapsed neighbour's **[spec decision]**. Wiring her to `counter_sign` would silently turn the designed leak into an evergreen bypass: befriend the child once and the quarterly rotation never costs anything again. Her hiding place at the Needle's midpoint is where the chalk goes, not where signs are spoken, so she does not automatically re-overhear; the child-shaped shortcut expires with the quarter, and design/02 §7's "or Ede" recovery route is only as fresh as her memory. The deterministic matcher (design/02 §8) reads this block directly; no LLM ever holds the key, only the manner.

## 4. Items of the conspiracy: offer_item, unchanged

The contraband economy compiles entirely to the existing offer / accept / decline / retract verbs at 4 m. Proposed item set: green-dipped candles, pilgrim badges, bell-coins, coin (dues, page-money, dry money), Sparr pages, *Colm's Last Letter*, the Green Almanac cloth, sealed archive packets.

Item fields **[proposed]**: `{id, name, description, lore_text?, provenance?}`.

- **`lore_text`** is the document's actual words (the `documents/` files are literally item text). Injection is transient, in-hand, literacy-gated (§1).
- **`provenance`** (`genuine | copp-forgery | unknown`) is sidecar-only and **never rendered into any prompt**. No actor can detect a forgery by reading metadata; authenticity arguments at meetings are genuine LLM judgement over the text plus each actor's extracts — sometimes right, per canon §5, which is the fun. The simulation knows *Colm's Last Letter* is Corin's work (ledger row 21); nobody's context does.
- **Provenance memory.** On accept, the sidecar writes a memory to both parties — who, what, where, when. Entry 32 of `features/50_cool_suggestions.md` (gift provenance) falls out of this for free; nothing here depends on it.
- **The trades are two offers.** A page-buy at Lise's counter is the player offering coin and Lise offering a page, both within 4 m; moth pay is a packet offered by a clerk. The re-offer-replaces and jilted-target semantics that already exist cover haggling withdrawals with no new code.

## 5. Whisper on the wire

Speech events gain one optional field, `manner: "spoken" | "whispered"` **[proposed]**, set by the *sidecar from scene context* — grate exchange lines, meeting interiors, the bede-roll — never from microphone amplitude, and never by the model directly (it is scene state, not performance).

Treatment per TTS backend:

- **Local Pocket TTS (streaming):** a cheap DSP chain on the output — roughly −10 dB gain and a mild high-shelf lift, which reads as a stage whisper. Nothing fancier; breathiness modelling is out of scope.
- **Cloud OpenAI:** pass a whisper instruction where the voice endpoint supports one; otherwise apply the same DSP chain.
- **Off:** no audio to treat.

Hard rules **[spec decision]**: `manner` never alters the 20 m recipient calculation — canon insists a whisper at a grate *can* be overheard, and design/02 §4 forbids amplitude-gated anything. It is presentation only. And since all NPC speech must surface as text regardless of backend (design/02 §9), whispered lines carry a visible marker — "(whispered)" — so the information survives TTS-off and deaf play identically. The player's own whispering is design/02 §4's problem and is solved there (proximity plus stakes; silence-timeout re-asks).

## 6. Proposed config.ron knobs

One nested block under `smart_actors`, additive, default-on-safe **[proposed]**:

```ron
smart_actors: (
    // ... existing fields unchanged ...
    second_sun: (
        enabled: true,                     // master gate: no lore injection, no pass
                                           // state, no C-series lines when false
        lore_pack: "lore/claude/second_sun",
        standing_lore_budget_tokens: 1200, // section 1 cap; drop order fixed
        transient_lore_budget_tokens: 500,
        ambient_rumor_rate: 0.5,           // nudge goals per NPC-day (K3 in design/05)
        rumor_hop_budget_per_day: 8,       // global cap on linked retellings
        rumor_ttl_hops: 3,                 // drift freeze depth
        funeral_cadence_days: 10,          // cell-paid burial cadence (K5)
        mark_decay_quarters: 1,            // Wicket ladder decay (K8, in sim time)
        moth_report_delay_days: 2,         // heard-to-relayed lag for moth actors
        whisper_tts: true,                 // section 5 treatment; presentation only
        speech_line_fallback: true,        // typed say when STT is absent (design/02 sec. 9)
    ),
),
```

Each knob gates exactly one mechanism named above. Where a knob restates a design/05 tuning knob (K3, K5, K8), design/05 owns the safe range and the "breaks at" analysis; this block is merely the config surface. Renderer tuning (K1 grave-light intensity, K2 disc scale) does **not** live under `smart_actors` — the disc is not the sidecar's business, by design pillar 1.

## 7. The deterministic scenario set

`fake_backend: true` must carry the entire conspiracy offline (the bar set in design/05 §6). The grate's F-series tests (F1–F10) are owned by design/02 §10. This document enumerates the remaining canned exchanges the fake cast needs — the **C-series** — each a fixed, scripted response keyed to a deterministic trigger:

- **C1 — the absolution.** Wick-priest, on any forbidden-noun transcript within 20 m: "So do we all, child; affirm nothing." One Edict citation if pressed. Never reports.
- **C2 — Renna's warning.** Warn-once at the Bell and Ladle, sincerely including the false throat-cutting rumor (ledger row 27, F).
- **C3 — Pike's keys.** Subject change on phenomenon talk; a fixed coin-for-odd-hours offer through the offer verb.
- **C4 — the moth relay.** Brant re-emits any phenomenon-vocabulary transcript into his patter after `moth_report_delay_days`, warped by a fixed substitution table ("second sun" → "the two lights, neighbours") — deterministic drift standing in for LLM drift.
- **C5 — the sexton.** Warm fixed answers on strokes and chalk; topic-change deflection on who reads the lintel or why.
- **C6 — Ede.** Bell-coin accepted → the canonical rhyme; the persuade path → the counter-sign spoken exactly once.
- **C7 — the squint.** Aldith trades verse for news on a fixed judgement rule (transcript length plus one keyword) standing in for LLM honesty-judging.
- **C8 — the counter.** Lise's two-offer page trade at fixed prices; forged and genuine pages indistinguishable in every output.
- **C9 — the meeting.** One full fixed transcript: bede-roll, sky-drawing, dues, and the closing recitation carrying next quarter's sign after "unwalled", as a single broadcast `say`.
- **C10 — the cloth.** Marle's sky-drawing comparison against the deterministic NPC-sight feed (design/05 §6.3); fixed warm/flag branches.
- **C11 — the slate.** Mott's funeral tutoring lines, with the Rud omission as a fixed branch.
- **C12 — the fed name.** Vell's test delivery lines and the empty-cellar consequence, matching design/02's F5 hook.
- **C13 — the two-eyes test.** Ferrant validates called panes against the deterministic ephemeris; the margin line as fixed reward.
- **C14 — the doors.** Dorn's Concurrence ruling as a fixed weighing over counted memory categories, defaulting open.

Plus the fake capability layer itself: fake STT (input arrives only via the drive `say` action), fake TTS (silent, text only), and scripted events — funeral, quarter-feast, weather, Concurrence day — fired on drive triggers (§8). Every C-series exchange gets one offline integration test in the existing Python suite (`uv run --offline --no-project python -m unittest discover -s tests`).

## 8. Drive-mode smoke scripts

`CATHEDRAL_DRIVE` proposals, continuing design/02 §11 (which owns P1–P4, the grate, and proposes `say <text>` and `goto <anchor>`). One further action **[proposed]**: `event <name>` — fire a scripted sidecar event, honoured under `fake_backend` only (`funeral_next`, `quarter_feast`, `day_pass`, `weather clear|overcast`, `concurrence_day`). New dev anchors: `bellstand`, `tally_bridge`, `maren_yard` **[proposed]**. All scripts assume `fake_backend: true` and the seeded F-series world.

```sh
# P5 — rumor hop: speak near the moth, hear it come back warped (C4)
CATHEDRAL_DRIVE='wait-online; goto bellstand; \
  say the second sun stood in the third light today; sleep 2; \
  event day_pass; event day_pass; goto tally_bridge; sleep 6; \
  shot rumor_returned; quit' cargo run
# assert: sidecar transcript contains the C4-warped line, tagged to the row

# P6 — rotation: lintel changes on a funeral; yesterday's name goes stale
CATHEDRAL_DRIVE='wait-online; goto charnel_door; shot lintel_before; \
  event funeral_next; sleep 4; shot lintel_after; \
  goto gaunt_grate; sleep 4; say Old Sef walks; sleep 4; \
  shot grate_stale_name; quit' cargo run
# assert: lintel texture differs between shots; rung-3 mark event in transcript

# P7 — the page trade: coin across the counter, page back (C8)
CATHEDRAL_DRIVE='wait-online; goto tally_bridge; sleep 2; \
  click Offer Coin; sleep 3; click Accept Sparr Page; sleep 2; \
  shot page_in_hand; quit' cargo run
# NOTE — the click targets above are [proposed], not observed. What ships
# today (prompt_playgound/AGENTS.md) is the *sidecar* side of the economy:
# offer_item / accept_offered_item / decline_offer / retract_offer at 4 m,
# with pending offers rendered on each actor's sheet as you_offer /
# offered_to_you. Those are NPC actions. How the *player* offers and accepts
# is unspecified — no player-facing offer surface is established anywhere in
# this corpus or the game docs. Before P7 can run, one must be chosen, and
# the choice is constrained by vision pillar 3 (no new HUD): the cheapest
# honest options are a proximity keypress on the existing interaction prompt,
# or routing offer/accept through speech like every other verb ("a bell for
# the page"). If a clickable surface is chosen instead, pillar 3 must be
# scoped explicitly to *new* UI and the pre-existing surface named. Until
# then this script is a sketch of an input path that does not exist.

# P8 — whisper manner: text marker on the grate exchange
CATHEDRAL_DRIVE='wait-online; goto gaunt_grate; sleep 4; \
  say Old Sef walks; sleep 3; say Belwyns unlit; sleep 3; \
  shot whispered_marker; quit' cargo run
# assert: NPC lines render with the (whispered) marker; the DSP chain
# itself is verified by ear once per release, not in CI

# P9 — failure isolation: STT deliberately unconfigured, feature still playable
# (run with stt keys absent; drive `say` exercises the same sidecar path the
#  speech-line fallback uses)
CATHEDRAL_DRIVE='wait-online; goto gaunt_grate; sleep 4; \
  say Old Sef walks; sleep 3; say Belwyns unlit; sleep 3; \
  shot grate_open_no_stt; quit' cargo run
# assert: handshake reports stt unavailable; grate opens anyway
```

Screenshots prove world state; memory and inbox contents are asserted in the paired Python tests. As design/02 puts it: drive scripts demonstrate, the offline suite proves.

## 9. Failure isolation

The handshake reports three independent capabilities; the feature must honour that independence exactly.

| Capability lost | Degrades to | Must keep working |
|---|---|---|
| **STT** (mic dead, no key, no model, user choice) | the speech line (design/02 §9): a typed utterance becomes a located `say` — same 20 m, same bystanders, same matcher, same ladder | the whole conspiracy; the Esc menu can restore or switch backends at runtime |
| **TTS** (off, or cycled off with X) | all NPC speech as text; whisper manner becomes the text marker | rotation cues — the knell and bells are game audio, not TTS, so the pass "UI" is untouched |
| **LLM cognition** (no key, provider outage, backoff) | performances fall back to C-series fixed lines where defined, silence elsewhere; rumor drift pauses | the deterministic spine: matcher verdicts, rotation transactions, chalk, knell, diamond, offers. Rows are durable memories — drift resumes on recovery with nothing lost |
| **Sidecar crash / restart** | game relaunch path via uv; player commands blocked until the replacement snapshot lands (existing behaviour) | pass state, marks, and chalk ride the snapshot, so projection and authority cannot diverge |
| **All of it** (`fake_backend: true`) | the deterministic cast, end to end | every scenario in §7 and design/02 §10, offline, byte-for-byte |

The principle beneath the table: everything canon declares objective fact (§3a — the grate opens only to the current pass; the lintel is true; the knell counts true) is code and world state, never model output. Therefore no capability loss, outage, or degradation can make the game contradict canon. The LLMs supply manner, judgement, and drift — the parts that are allowed to fail into silence.

## 10. The cost of a conspiracy — honest numbers

- **No new call types.** Every LLM call the feature ever makes is an ordinary scheduled turn on the existing round-robin. The feature adds tokens to existing calls, and a bounded number of extra turns. That is the entire cost model.
- **Per-turn overhead.** Standing lore ≤ 1,200 tokens plus transient ≤ 500 on a baseline sheet of roughly 2–3k: about 40–60% more prompt tokens for lore-bearing actors. Completions are unchanged. (Provider prompt caching would eat much of this; `llm_client.PRICING` does not model cache discounts, so budget without them.)
- **A rumor hop is nearly free.** A hop that rides an existing conversation costs *zero extra calls* — it is what the speaker's turn and the listener's turn would have contained anyway. Extra calls come only from nudge goals: `ambient_rumor_rate` 0.5/NPC-day across, say, forty scheduled actors is ~20 nudged turns per in-game day.
- **Caps, in order of bite.** `rumor_hop_budget_per_day` (8) bounds linked propagation regardless of crowd size; `rumor_ttl_hops` (3) freezes surface text so an old row stops being regenerated at all; `row_id` dedupe kills echo storms, the one genuinely unbounded failure mode. Worst case, a full in-game week of maximum gossip is ≤ 56 linked retellings — noise against the ordinary turn stream.
- **The spike is the Concurrence.** Crowd staging concentrates scheduled actors in one nave. The existing scheduler priority and provider backoff are the control surface; additionally, crowd extras should run thinned schedules on C-series lines, with full cognition reserved for the named cast of `05_dramatis_personae.md` **[spec decision]**.
- **Measure before promising.** The prototype already prints per-run USD from the pricing table. The acceptance criterion is a measurement, not a guess: one knob-default in-game day under each real backend, costed, before any dollar figure is written down anywhere — including here.

The summary an engineer can carry to a meeting: the Second Sun is one renderer feature, one small matcher, and §0's counted list of sidecar additions — none of them a new player verb, a new UI, or a new call type. The city already knows how to hear, remember, hand things over, and talk. We are giving it something worth not talking about.
