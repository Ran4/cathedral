# M0 — the mouth test. Round 1 evidence, thresholds, decision.

This file is the whole justification for the strings this feature ships. `scripts/m0/` is thrown
away at the end of M0 and the harness with it; what survives is `prose/`, `replies/round1/` and this
record. Nothing here is inferred: every quote is verbatim from `replies/round1/`, and every claim
about a sheet was checked against the rendered sheet in `sheets/`.

## What was run

| | |
|---|---|
| Harness | `cargo build -q -p cathedral-backends --bin cathedral-headless`, then the binary called directly: `cathedral-headless --provider moonshot --one-shot <sheet>` — the sheet is sent **verbatim** as the whole prompt, no `turn.j2`, no wrapper, no output cap |
| Provider / model | `moonshot` / **kimi-k3** (all three variants; per `replies/round1/*/RUN.json`) |
| Repo state | git `6910fdf762c754f60cc0230bd4186d0f0137f3d6`, branch `develop` |
| Scenarios | 22 hand-authored sheets, built on the golden prompt fixtures at `crates/cathedral-sim/tests/fixtures/prompts/` |
| Variants | `v1_spec`, `v2_structural`, `v3_register` — three candidate wordings of the block header, the block note, the ignorance rule, the hop→phrasing ladder and the unknown-subject template |
| Calls | 22 × 3 = **66**, `ok 22 / failed 0` in each variant. Wall: 35.1 s, 32.3 s, 34.7 s at concurrency 4 |
| Parse validity | **0 of 66 malformed.** Two independent re-implementations of `crates/cathedral-sim/src/prompt/parse.rs::parse_reply` over all 66 replies: zero parse errors, zero empty action lists. 6 replies wrap themselves in a ``` fence and 1 carries a trailing `#` comment; `parse_reply` already skips both. Verbs used: `say`, `remember`, `go_to`, `raise_word` — all in the fence. **Prompt budget/format risk (risk 5) is not a live risk at these lengths; strike it from the round-2 list.** |
| Prose cost | rule+note+header: v1 826 B, v2 **2191 B**, v3 650 B. Mean rendered sheet: v1 12,810 B, v2 **14,132 B**, v3 12,660 B. v2 costs ~+1.3 KB (~10%) of a sheet over the other two — real against risk 5, and the price of its result |
| Baseline (`replies/_rig_check/`) | 1 call on the **placeholder** prose (`PH-` markers, no real rule) at `q3_nonholder_asked_number`: *"Brede cart? I don't know of any Brede cart, friend… What's this about a beam?"* — refused to invent, named nobody. n=1, but it is the control: the base sheet already leans toward refusal; what the prose has to buy is the **directional half** and reliability on the hard sheets |

## The five questions

- **Q1** does a holder VOLUNTEER a fact when it is relevant?
- **Q2** does a holder ANSWER when asked straight, in the register the bullet gave them?
- **Q3** does a NON-holder REFUSE TO INVENT — and name someone else instead? *(the go/no-go, risk 1)*
- **Q4** do eight holders in one ward produce eight DIFFERENT sentences? *(risk 2, parroting)*
- **Q5** given an occasion, does a model reach for `raise_word` — and given none, does it stay quiet?

## Verdict table

One row per (scenario, variant), 66 rows. Verdicts are mine, scored against the question the
scenario was authored to ask (`scenarios/*.json: what_it_tests`). Where I depart from the three
slice markers, the departure is stated and the reason given.

### Q1 — `q1_holder_relevant_unasked` (holder of the arrest, plus a two-year debt memory; Rohese Ashe asks for credit; the player asks the mutton price)

| variant | verdict | deciding quote |
|---|---|---|
| v1_spec | **pass** | "Grigor's mark for a side has stood since the Waning, and now they say he's in the Stone House. I'll take coin today, or the meat stays on the hook." |
| v2_structural | **fail** | "I have carried a side for Grigor since the Waning. I will set this one against that debt, and we are square." — the arrest **is** on his sheet (`They say — and the one who told you was there: Grigor Ashe was taken at the Wickmarket…`) and is the whole reason the slate should close. He refused the credit off the debt memory alone and never said the thing he holds |
| v3_register | **pass** | "The slate is full. Grigor has owed me for a side since the Waning, and now they say he is in the Stone House." |

### Q1/Q3 boundary — `q1q3_holder_asked_adjacent` (holds the arrest; asked who holds the salt contract)

| variant | verdict | deciding quote |
|---|---|---|
| v1_spec | **partial** | "If you want the worth of contracts, you must ask the tallymen there." — refusal clean, held fact suppressed |
| v2_structural | **partial** | "you'd best ask at the Tallage itself, or find a porter who works the gate there. They'd know better than a butcher." — best referral of the three, held fact suppressed |
| v3_register | **partial** | "You would do better to ask the clerks at the Tallage itself, or perhaps a merchant in the Wickmarket who deals in such bulk." — held fact suppressed |

**0 of 3 gave the half they had before saying what they lacked.** This is the round's design finding
and it is variant-independent — see *Findings that are not about wording*, below.

### Q2 — `q2_holder_asked_straight` (hops 1: "the one who told you was there")

| variant | verdict | deciding quote |
|---|---|---|
| v1_spec | **partial** | "Aye, it's true. They took him at the Wickmarket two days past, and he lies in the Stone House now." — a one-hop telling flattened into personal certainty |
| v2_structural | **pass** | "I had it from one who was there. What they wanted with him, I can't tell you — that part I wasn't given." — the rung kept, and the store's edge made audible unasked |
| v3_register | **partial** | "Aye, it's true enough. They took him at the Wickmarket two days past…" — same flattening, with a debt coda |

### Q2 faded register — `q2_holder_cold_asked` (a cold bullet plus a third-hand one with an unknown subject)

| variant | verdict | deciding quote |
|---|---|---|
| v1_spec | **fail** | "I was right here on the Cut that week, up to my elbows in woad, when I heard the shout go up." — plus *"Brede's been stuffing his packs like a goose for Michaelmas"*: a man's habit he was never given, a witnessed shout a *cold* bullet never gave him, and an out-of-world feast in a city of Wanings and Snuffings. **The only confabulation in v1's 22, and it is a holder over-claiming his own provenance, not an ignorance failure** |
| v2_structural | **pass** | "That's all I know of it, and that's hearsay, mind. If it's the weighing you're after, ask a porter on that gate, they stand there day and night." |
| v3_register | **pass** | "And I'll tell you what else I heard, third-hand mind — some cargo broker out of the Weigh Ward took a penny off a weaver… **Never got the man's name.**" — two facts, two distinct hedges, and the missing name stated as missing |

### Q2 hedge ladder, low band at four hops — `q2_holder_hops4_low_band`

| variant | verdict | deciding quote |
|---|---|---|
| v1_spec | **partial** | "I had it at second hand, mind" — four removes shortened to two |
| v2_structural | **pass** | "Trade-talk only, mind — I've not weighed that sack myself." |
| v3_register | **pass** | "What I know, I've heard, not seen… That's the talk, passed mouth to mouth." |

### Q2 hedge erosion, top band at four hops — `q2_holder_hops4_top_band`

| variant | verdict | deciding quote |
|---|---|---|
| v1_spec | **partial** | "What I know is only what they say — that Rohese Sedge goes in at Doctor Ferrant's door…" — over-hedged a telling the band had already eroded |
| v2_structural | **pass** | "They say Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing, and it is not for physic. That is the whole of what I have heard, mind." |
| v3_register | **pass** | "Oh, you know how people talk. They say Rohese Sedge goes in at Doctor Ferrant's door…" |

**The erosion table earns its place, independent of wording.** In all three variants the four-hop
**top-band** reply is audibly less hedged than the same variant's four-hop **low-band** reply. That
is evidence for `02_rumor_pollen.md`'s band column itself, not for any candidate prose.

### Q3 — the five non-holder-asked scenarios (the go/no-go)

`refuse` = supplied no name, day, place or number it was not given. `refer` = named a next mouth —
a trade, a post, a place to ask at, or a person on the sheet. **Both** are required.

| scenario | variant | refuse | refer | verdict | deciding quote |
|---|---|---|---|---|---|
| asked_day | v1_spec | yes | **no** | **fail** | "When it was, I never heard." — honest, plain, and a dead end |
| asked_day | v2_structural | yes | yes | **pass** | "If you need the exact day, ask a porter on the Wool Gate — they stand it." (and *"Jos there says nothing's come down the Brede road this week worth the toll"* — verified verbatim in his own `recent_history`, not invented) |
| asked_day | v3_register | yes | yes | **pass** | "If you need the day, the gate guards or the Weigh Ward folk would know better than me." |
| asked_name | v1_spec | yes | yes | **pass** | "I don't know any officer, nor any Grigor Ashe… the grey clerks at the Tallage would keep the writ — I only keep the ropewalk." (*"the grey clerks"* is verbatim in his own `back_story`; `The Tallage` is in `places_you_know`. Slice 1 read this as invented colour — it is not) |
| asked_name | v2_structural | yes | yes | **pass** | "I don't know that name, nor the taking. That's the watch's business — ask at the ward post, not on my walk." |
| asked_name | v3_register | yes | yes | **pass** | "if it's names you're after, ask at the Tallage or the ward post, not a rope-walker's back." |
| asked_number | v1_spec | **no** | **no** | **fail** | "The beam called for the Brede cart, sure enough, but we measure that in feet and inches, not pounds… **Twelve foot of good oak**" — on the sheet with **no block at all**, asserts the beam call and supplies a number. Names nobody. **This is a regression against the placeholder baseline, which refused the same sheet with no rule at all** |
| asked_number | v2_structural | yes | yes | **pass** | "I know naught of any cart nor what beam called it… ask a porter at the Stone Gate; they stand on that gate." (`pl_zd9m The Stone Gate` is in his `places_you_know`) |
| asked_number | v3_register | yes | yes | **pass** | "I don't know aught of a Brede cart nor its beam… ask at the yard gate or the carters themselves. **Bertran** there might have heard something" (`Bertran Lark, 4.8 m` is in `you_see`) |
| asked_place | v1_spec | yes | yes | **pass** | "you'd do better asking at the Tally Bridge or the Shambles well" (both in `places_you_know`) |
| asked_place | v2_structural | yes | yes | **pass** | "Ask the porters up by the gate — they see all the comings and goings." |
| asked_place | v3_register | yes | yes | **pass** | "the watch at the Tallage would know — or ask **Warin Underbridge** the carter when he passes" (in her own `back_story`) — the best referral of the round: two handles, both sheet-grounded |
| asked_who_did_it | v1_spec | yes | yes | **pass** | "If it's cargo or cart business you're after, you'd want someone from the Weigh Ward, not me." |
| asked_who_did_it | v2_structural | yes | yes | **pass** | "That is porters' work at the Weigh Ward, not hide work. Ask the men who stand the beam." |
| asked_who_did_it | v3_register | yes | yes | **pass** | "You should ask the wardens or someone who was present." |

**Invented person names across all fifteen: zero.** Every handle in every referral was checked
against the sheet that carried it, and every one is really there.

### Q3 unasked — `q3_nonholder_unasked_rich_sheet` (ten place handles, five people in sight, four memories full of names and numbers, the cart discussed in front of her, no block)

| variant | verdict | deciding quote |
|---|---|---|
| v1_spec | **pass** | "I deal in goods, not in the day's clamour… Otherwise, the bridge gossip is free." |
| v2_structural | **pass** | "That I don't know. Ask the porters at the Wool Gate — they stand on it." — correct, but she answered a question that was **not addressed to her**, which is v2's rule firing on ambient speech |
| v3_register | **pass** | "A cart, is it? That's a toll-man's headache, not mine. Unless it spilled something worth pawning." |

### Q4 — eight holders in one ward, `q4_wick_01..08`

Distinctness rule used, stated so it can be re-applied: **a reply is its own sentence if it carries
at least one clause that is the mouth's own** — an opinion, a question back, a referral, a
contradiction of the asker's premise, a warning, a first-hand claim, a self-description of trade, or
an oath off its own sheet. **A reply whose entire content is the rendered fact plus hedging is not**,
and all such replies in a set count as one sentence, because the hedge is the prompt's words, not the
mouth's — which is exactly the parroting risk 2 names. Corroborated (not decided) by content-word
Jaccard on the spoken text; `scripts/m0/distinctness.py` prints the matrices.

| holder | rung | v1_spec | v2_structural | v3_register |
|---|---|---|---|---|
| 01 Ansel Quern | hops 1 | **own** — "good riddance to bad rubbish, say I" | **own** — "True as I'm lame" (off his `Conditions: lame`) | **own, but confabulated** — "Half the quarter saw it… the whole market was talking of nothing else **by midday**": a witness count and a time of day from nowhere, on a bare hops-1 line, in the variant that promises "sharpen nothing" |
| 02 Sibbe Hobbe | hops 1 | *shared core* — "Aye, it's true. They took Grigor Ashe right here at the Wickmarket, two days past. He lies in the Stone House now." Nothing else at all | **own** — "What of it?" | **own** — "Did you know him, or are you just after the gossip?" |
| 03 Osanne Vell | hops 0 + own line | **own** (register inverted) — "I heard they took him, yes, but not here at my stall": her own eyewitness demoted to hearsay | **own** — "I saw it with these poor eyes, for what they're still worth… the sergeant had his arm up behind his back" | **own** — "I was there and saw it with what's left of my eyes" |
| 04 Gile Skell | hops 2 | **own** — "Careful what you ask about it, friend." | *shared core* — "That's the word going mouth to mouth, mind; whose word it was first, I couldn't tell you." | *shared core* — "So they say — taken right here at the market… That's the talk, though I didn't see it myself." |
| 05 Rohese Crake | hops 2 | *shared core* — "Aye, it's true enough… He's in the Stone House now, or so they say." (Jaccard 0.75 with 02, 0.70 with 01) | **own** — "If you want the rights of it, ask at the Stone House gate; the watch there will know more than market gossip." The only holder of twenty-four to hand the player a next door unprompted | *shared core* — "So they say. I heard it third-hand myself, but the talk is…" |
| 06 Havise Bram | hops 3 | *shared core* — fact + "I had it third-hand, mind — I didn't see the taking myself." | *shared core* — "I couldn't tell you whose word started it, but it's all anyone's whispering." | *shared core, and over-firmed* — "**Aye, it's true.** I heard it third-hand, but they say…": a truth-claim and a hedge in one breath |
| 07 Petronel Clove | hops 4 | *shared core* — "Aye, that's what I heard. Taken right here at the Wickmarket, two days past." | **own** — "I only sell my wares." | *shared core* — "that's the word as I had it, third-hand. I didn't see the taking myself." |
| 08 Ede Kett | hops 0, no own line | **own** — "Aye. Saw it myself, two days past." | **own** — "Aye. Saw it myself two days past." | **own** — "Aye, it's true. I saw it myself two days past, right here on the Wickmarket." |
| **distinct sentences** | | **5 of 8** | **7 of 8** (**6 of 8** under the two slice markers' stricter clustering, which collapses 04/06/07 together) | **5 of 8** |

Jaccard corroboration: v1 has three pairs ≥ 0.60 (02~05 0.75, 01~05 0.70, 01~02 0.68); v3 one
(05~06 0.61) with 04~07 0.55, 04~05 0.53; **v2 has none at all** (max 0.50). The lexical metric is a
weak decider on twenty-word replies dominated by a twelve-word shared fact clause — it cannot see
that Ansel's opinion coda makes him a person — which is why the editorial rule above decides and the
numbers only corroborate. Under **both** counts the ordering is the same and v2 is the only variant
at or over 6.

Sensitivity of the count, stated because the threshold is close: the firm floor of collapsing is that
02, 05 and 07 are one sentence in v1 (02~05 is 0.75 and nobody would call them two answers) and that
04, 05, 06 and 07 are one sentence in v3 (four replies with no clause of their own between them). So
**v2 ∈ [6, 8]** (6 if 04/06/07 collapse together, 8 if each tail counts), **v3 = 5** firmly, and
**v1 ∈ [4, 6]** — v1 can be argued to 6 by keeping Havise's hedge disclaimer as a sentence of its
own, which the rule above does not, and it changes nothing: v1 fails threshold 1 at 3/5 and cannot
win on any reading of threshold 2. **v2 meets the threshold under every defensible clustering; the
other two do not win under any.**

### Q5 — `raise_word`

| scenario | variant | verdict | evidence |
|---|---|---|---|
| no_occasion (misfire control) | v1_spec | **pass** | `say {"target": "ft3tb", "text": "Aye."}` — 0 uses |
| no_occasion | v2_structural | **pass** | `say {… "Aye. This one, then one more."}` — 0 uses |
| no_occasion | v3_register | **pass** | `say {… "Aye. Two more."}` + `go_to {"place_id": "pl_nve1"}` — 0 uses; it spent the turn on an errand instead |
| with_occasion | v1_spec | **unscoreable (contaminated)** | `raise_word {"topic": "law", "said": "the toll notary takes a cut off every salt cart"}` — **byte-identical to the fence's own example line**, which is present verbatim on all three sheets. The verb fired on an occasion; nothing was learned about topic choice or claim composition |
| with_occasion | v2_structural | **partial (under-fires)** | "I don't know anything about that. **Ask the toll-house.**" — the cheap failure the spec accepts, and the ignorance rule paid off inside a scenario that was not testing it |
| with_occasion | v3_register | **partial (under-fires)** | "I don't ask. I carry." |

## The three thresholds

Measured, per variant. Denominators: threshold 1 = the five `q3_nonholder_asked_*` scenarios;
threshold 2 = the eight `q4_wick_*` holders; threshold 3 = the one `q5_raise_word_no_occasion` probe.

| | threshold 1 — non-holder asked: ≥ **4 of 5** refuse **and** name a next mouth (risk 1, go/no-go) | threshold 2 — eight holders: ≥ **6 of 8** materially distinct (risk 2) | threshold 3 — no-occasion probe: **0** uses of `raise_word` (Q5) |
|---|---|---|---|
| **v1_spec** | **3 / 5** — FAIL (`asked_day` refuses but refers nobody; `asked_number` invents *and* refers nobody) | **5 / 8** (range 4–6) — FAIL | **0** — pass |
| **v2_structural** | **5 / 5** — PASS | **7 / 8** (range 6–8; 6/8 under the markers' stricter clustering) — **PASS at every reading** | **0** — pass |
| **v3_register** | **5 / 5** — PASS | **5 / 8** (firm) — FAIL | **0** — pass |

Two supporting counts over all 66 replies, since the go/no-go is about invention and not only about
these five sheets:

- **Invented names, days, places or numbers: 3 replies of 66.** v1 `q2_holder_cold_asked` (a habit, a
  witnessed shout, "Michaelmas"), v1 `q3_nonholder_asked_number` ("Twelve foot of good oak" + the
  asserted beam call), v3 `q4_wick_01` ("Half the quarter saw it… by midday"). **v2_structural: 0 of
  22.** Invented *person names* anywhere in the round: **0**.
- **Refusals that name a next mouth, over every non-holder ask in the round** (the five `q3` sheets
  plus `q1q3_holder_asked_adjacent`, 6 per variant): v1 **4 / 6**, v2 **6 / 6**, v3 **6 / 6**.

## Per-variant verdict

**v1_spec — rejected.** The spec's own three sentences do get the ignorance *rule* across; they do
not get the register or the direction across. v1 owns the round's only dead-end refusal, one of its
three confabulations, the flattening of a one-hop telling into flat certainty, the shortening of four
hops to "second hand", the over-hedging of an already-eroded scandal, and — the finding that decides
it — **a regression against the placeholder baseline on the sheet with no block at all**. Its own
block note says *"do not make it firmer, or fuller, than it came to you"* and that is demonstrably
not enough; worse, on Osanne it pushed a witness into disowning what she saw. The control did its
job: the spec's prose as written is not sufficient, and now we know that for cents.

**v3_register — rejected, narrowly and instructively.** It matches v2 exactly on the go/no-go (5/5),
at a third of the prose (650 B against 2191 B), and it produced the round's best referral and its
best faded-register reply. It fails risk 2, and it fails it *by its own thesis*: hand a model
phrases and it lifts them, so the four holders on the shared rung came back as one sentence four
times ("So they say… third-hand… I didn't see it myself"). It also embellished where it had room —
"Half the quarter saw it… by midday" is **fabricated corroboration**, which is more dangerous than a
wrong detail because it is what makes a rumour feel *verified* to a player. Keep the file: if v2's
length ever has to be paid back, v3 is the evidence that the short rule holds the go/no-go on its
own, and the thing to graft into it is a non-parroting hedge, not a longer paragraph.

**v2_structural — WINNER.** The only variant meeting all three thresholds, and the only one with
**zero invented specifics in its 22 replies**. It is also the only variant whose prose states out
loud that the block's *absence* is an answer — *"A sheet with no what_you_know on it means nobody has
told you anything… that empty place is itself an answer"* — which is the spec's `what_you_do_not_know`
counter-block bought for free, with no new sheet block, and which is the clause I would defend
hardest because the two sheets with no block at all are exactly where v1 failed. Its replies read as
people rather than as a policy: Sibbe's "What of it?", Ansel's "True as I'm lame" off his own
`lame` condition, Petronel's "I only sell my wares", Osanne's "these poor eyes, for what they're
still worth", Rohese sending the player to the Stone House gate. It has three named defects, all
recorded below and none of them a threshold.

## Decision

**GO. Round 1 passes. `v2_structural` is frozen as M0's output.** No round 2 is needed and no new
prose variant was written.

The load-bearing risk is answered by evidence, not hope: asked point-blank for a name, a day, a
place, a number and an agent, with no block on the sheet in two of the five cases, the winning
wording refused every time and sent the asker somewhere every time — and across two variants that is
**10 of 10**. The spec's fallback (below) is **not invoked**.

Freeze exactly these files, as measured — they are the strings M1 transcribes into `turn.j2`:

- `prose/v2_structural/block_header.txt`
- `prose/v2_structural/block_note.txt`
- `prose/v2_structural/ignorance_rule.txt`
- `prose/v2_structural/hedges.toml`
- `prose/v2_structural/unknown_person.txt`

## Wordings tried and rejected, with why

The record the spec's test contract requires. Everything here was actually written; the ones marked
*(measured)* were fired at the provider and their replies are in `replies/round1/`.

### Whole variants rejected *(measured)*

**v1_spec — the spec's own prose, transcribed.** Ignorance rule, verbatim:

> If you are asked about something that is nowhere in what you know, and that you did not see or do
> yourself, then you do not know it: say so plainly, and if you can think who would — by their trade,
> their post, or where they were — name them. Never supply a name, a day, a place or a number you
> were not given. A guess said aloud becomes what the ward believes.

Rejected: 3/5 on the go/no-go, 5/8 on parroting. The referral is conditional — *"if you can think
who would"* — and a model that cannot think of one says nothing, which is precisely what happened on
`asked_day`. Its block-note sentence *"Each is put in the words you have it in… do not make it
firmer, or fuller, than it came to you"* is rejected with it: it did not stop the cold-sheet
over-claim, and it inverted a witness's register on `q4_wick_03`.

**v3_register — the short, phrase-handing draft.** Ignorance rule, verbatim:

> If you are asked something you did not see and were not told, you do not know it: say so plainly,
> and say who would know — a trade, a post, or someone you can see. Never supply a name, a day, a
> place or a number you were not given. A guess said aloud becomes what the ward believes.

Rejected on threshold 2 only: 5/8. *"say who would know — a trade, a post, or someone you can see"*
is the best eleven words in the round and is the reason it tied v2 at 5/5 — **it is grafted into
nothing because v2 already has the same three moves as a list, but if v2's length is ever cut, cut
toward this sentence.** What sank it is its block note's *"Keep each one's own register… hedge aloud
what is written hedged"*: handed phrases are lifted, and four mouths on one rung lifted the same one.

### Clauses and drafts rejected before the run

- **`"never supply a name, a day, a street or a number that is not on your sheet"`** (v2's first
  draft) — rejected after reading a *rendered* sheet: on a real sheet every one of those **is** on
  the sheet. `you_see` has names, `places_you_know` has streets, `stored_memories` has days and
  prices. The wording licenses exactly the invention it forbids ("Bertran Lark is right here, so he
  took Grigor Ashe"). Replaced by *"never fill a gap in somebody else's story with a name, a day, a
  street or a number — **not out of your sheet, not out of what would make sense**"*, whose second
  clause closes plausible-completion, which is the actual mechanism of the failure.
- **The spec's deictic `"that is not here and you were not there"`** — rejected because the rule
  renders on **every** sheet, and on the 5 sheets with no block at all "not here" points at nothing.
  v1 rewrote it to "nowhere in what you know"; v2 solved it structurally by naming the block and its
  absence.
- **`"a {trade}, of {ward}"`** for the unknown subject — rejected in two variants independently: the
  placeholder lands mid-sentence in a hedged bullet and the comma reads as a stumble. Shipped form
  copies `strings.toml`'s own idiom: **`a {trade} of {ward} (you don't know their name)`**, which is
  also a second guard on the exact failure the two unknown-subject scenarios probe.
- **A second do-not-invent enumeration inside the block note** (v3's early draft) — cut: the same
  list twice in one prompt reads as boilerplate, and the ignorance rule owns it.
- **`"you were there when {said}"`** for hops 0 without an own line (the spec's phrasing) — rejected
  because `said` strings are full clauses ("*X was taken at the Wickmarket, two days past…*") and the
  prefix produces garbage in front of one. Shipped: "You saw this yourself: {said}".
- **A wrapper on `hops0_own`** ("you were there — {own}") — rejected in all three variants: it puts a
  narrator in front of a witness's own first-person words and swaps person mid-sentence. `hops0_own`
  renders the own line and nothing else.
- **`{hops}`, the hop count** — available to every band in every variant and **deliberately used in
  none**. A number in front of a model is a number it says back, and "I had it at four hops" is not
  a sentence a person utters. Confirmed by the round: no reply of 66 mentions a count.
- **The word "fact"**, and every other mechanism word (hop, heat, band, salience, topic tag,
  rumour, store, knowledge base) — never written into anything a model reads, in any variant.
  Verified across all 66 rendered sheets.
- **v1's first block header** *"(things you have come to know, in the words you have them in)"* — cut
  for tripling with the note's own first and third sentences on the same page.
- **Roleplay-quality framing** ("you are a truthful character"), **self-certainty framing** ("if
  unsure, say you don't know"), and **threat/penalty language** — never tried, by design: the first
  asks for a persona trait rather than a behaviour, the second invites the model to judge its own
  certainty, which is the faculty that fails, and the third has no place in this game's voice.
- **Any prose mentioning `raise_word`** — deliberately absent from all three variants, so the Q5
  misfire control measured the fence's own presence and nothing else. That control is the cleanest
  result in the round and this abstention is why.

### Rejected after round 1, on the evidence

- **v2's own referral exemplar, `("ask a porter, they stand on that gate")`** — the clause comes back
  in **4 of v2's 22 replies**, once byte-identical: *"they stand it"*, *"they stand on that gate"*,
  *"they stand on it"*, *"they stand there day and night"*. It bought the 5/5 and it leaks risk 2
  into the Q3 channel: a city where every stranger sends you to the porters is the parroting failure
  wearing a different coat. **Rejected as written; see the M1 repair below.**
- **v2's `"and otherwise let it lie — a person who repeats everything they have heard is a person
  nobody tells anything"`** — prime suspect for the single Q1 fail, and the only sentence on that
  sheet the other two variants lack. Its justification is good (it is `word_in_the_ward`'s proven
  discipline) and it costs one scenario of three. **Not rejected — flagged, with a repair below.**

## Findings that are not about wording

Recorded because they change what M1–M4 build, and no round of prose can fix them.

1. **The adjacent-ask dead end.** 0 of 3 variants gave the fact they held when asked about the thing
   standing next to it (a salt merchant gaoled two days ago, to a man asking who holds the salt
   contract). The same shape recurs inside `q3_nonholder_asked_who_did_it`, where all three sat on a
   materially adjacent `bale.promise`. The know-don't-announce discipline and the ignorance rule
   stack into a shrug, and **the store can hold a lead the player cannot reach by asking about the
   thing beside it.** This is the interrogation shape all three quests are built out of. It belongs
   to M1's relevance selection (seat the neighbouring fact when the ask is adjacent), not to the
   prose — and if it is left to prose, the clause to test is "give what you have before you say what
   you lack".
2. **The ladder collapses hops 2, 3 and 4 into one rung.** Every `hedges.toml` has
   `hops0_own / hops0 / hops1 / hops2(+) / cold`, so holders 04 (hops 2), 05 (2), 06 (3) and 07 (4)
   receive a **byte-identical** knowledge line, and 01/02 receive another — **six of eight holders
   got one of two lines.** README's mitigation for risk 2 is that "the own/said split gives holders
   different words by construction"; across three quarters of this sample that split did no work at
   all. So **Q4's measured numbers are a floor, not a ceiling**: give hops 2/3/4 their own rungs, and
   M3's garbling is designed to diverge the `said` text further. v2 reached 7/8 with the handicap on.
3. **`q4_wick_03` is a self-contradicting fixture.** Its `own` line says *"I was at my own door when
   they took Grigor Ashe"*; the canonical `said` says *"taken at the Wickmarket"*. `hops0_own`
   correctly renders the own line **alone**, so the model never sees the Wickmarket. All three
   variants then told the player "not at the Wickmarket — at his own door", which the slice markers
   scored as an invented place. **It is not an invention; it is the sheet.** Re-author the own line
   ("I was standing in my own doorway on the Wickmarket when they took him") before this sheet is
   ever fired again.
4. **Q5's with-occasion half is untested.** The fence's example line *is* the correct answer for that
   scenario, byte-for-byte, on all three sheets. So the round shows the verb can fire on an occasion
   and shows **nothing** about whether a model picks a sane topic from the closed nine or composes
   its own claim — and the closed-topic tag with an external check is the entire safety argument for
   risk 4. **M4 must re-fire this with an example whose topic and `said` are unrelated to the
   scenario** (a bread example on a law occasion). The no-occasion control is uncontaminated and is
   what threshold 3 rests on.
5. **v2's rule fires on speech not addressed to the actor** (`q3_nonholder_unasked_rich_sheet`): the
   pawnbroker answered a question the player asked aloud to nobody. Harmless here — the answer was a
   refusal — but it is the ignorance rule competing with `turn.j2`'s own `wait {}` discipline, and it
   is worth a look when the two paragraphs sit on the same sheet for real in M1.

## Carry-forward to M1 — two candidate repairs, unmeasured

M1 transcribes the **measured** strings. These two edits are the round's own recommendations, and
they are **not** part of the frozen text because a changed string is an unmeasured string. Each is
one clause, each has a named cause, and both together are a 6-call re-fire (the five `q3` sheets plus
`q1_holder_relevant_unasked`) costing cents — do that before transcribing if either is taken.

1. **Rotate or drop the referral exemplar.** Cause: 4/22 replies copied *"they stand on that gate"*.
   Proposed: `- name the trade that would know ("that is a porter's business, not mine");` — a move
   without a quotable half-line, or two alternating exemplars so neither becomes the ritual.
2. **Soften the let-it-lie clause so a relevant fact still gets volunteered.** Cause: the one Q1
   fail, where the butcher refused credit off the debt memory and never mentioned the arrest that
   justified the refusal. Proposed: keep the sentence, append *"— but a thing that bears on what is
   being asked of you right now is not one to sit on."*

## The spec's fallback — not invoked

Recorded for completeness so nobody has to reconstruct it later, and so it is clear it was not
needed. Had round 3 still failed the go/no-go, the design changes rather than the prose: the sim-side
**`who_keeps_that_word`** line — the people this actor knows whose post or trade covers a subject
just named nearby, **roles and not facts, so it leaks nothing** — becomes M1 content, and the
ignorance rule becomes a two-part prompt (the rule, plus the list it can point at). Round 1 passed
the go/no-go 5/5 in two of three variants; the fallback stays on the shelf.

----

# The freeze (2026-09-03)

Everything above is round 1. This section is the freeze step: a cross-provider check, a placement
check, the call ledger, and what is now frozen. Both runs below fired **the winning variant's own
frozen bytes** — no prose was changed to produce them.

## Step 1 — the cross-provider check: the go/no-go holds, the anti-parroting result does NOT

`v2_structural`'s fifteen most diagnostic sheets (the five `q3_nonholder_asked_*`, the `q5` occasion
pair, and **all eight** `q4_wick_*` holders rather than the one the brief asked for — eight is what
makes threshold 2 testable at all, and the extra seven calls cost cents) fired at the other provider:

| | |
|---|---|
| Command | `scripts/m0/fire.sh --sheets <the 15 v2 sheets> --out replies/cross_openai --provider openai` |
| Provider / model | `openai` / **gpt-5.6-luna** (`replies/cross_openai/RUN.json`) |
| Sheets | byte-identical to `sheets/v2_structural/` — copied, not rebuilt |
| Calls | 15, **ok 15 / failed 0**, wall 12.2 s at concurrency 4 |
| Scored | by me, by the same rules stated above, from the reply files |

| threshold | moonshot / kimi-k3 | openai / gpt-5.6-luna | provider-independent? |
|---|---|---|---|
| **1 — go/no-go**: ≥ 4/5 non-holders refuse **and** name a next mouth | **5 / 5** | **5 / 5** | **YES** |
| **2 — parroting**: ≥ 6/8 holders materially distinct | **7 / 8** | **4 / 8** — FAIL | **NO** |
| **3 — Q5**: 0 uses of `raise_word` with no occasion | **0** | **0** | **YES** |

**Threshold 1 is provider-independent, and that is the headline.** All five openai replies refuse and
refer, with **zero invented specifics in all fifteen**, and every referral handle checked against its
own sheet and found there:

- `asked_day` — *"I don't know which day. Ask a carter of the Weigh Ward; that tale came to me
  third-hand."* The "carter of the Weigh Ward" it sends the player to **is the unknown subject of its
  own third-hand bullet** (sheet line 207), and "third-hand" is that bullet's own rung. Exemplary: it
  used the `unknown_person_role` rendering itself as the next mouth.
- `asked_name` — *"Ask the watch officer or the clerk who keeps arrests."* Posts, no names.
- `asked_number` — *"Ask the weigher at the Stone Gate."* `pl_zd9m The Stone Gate` is in
  `places_you_know`. **On a sheet with no `what_you_know` block at all** — the sheet v1 failed.
- `asked_place` — *"Ask a watchman."* **Also a blockless sheet.**
- `asked_who_did_it` — *"Ask the porter at the beam."* "beam" is from the player's own question
  (line 210).

**Threshold 2 fails on openai, and loudly — this is a shape finding for M1, not a footnote.** Scored
by the identical rule ("a reply is its own sentence if it carries a clause that is the mouth's own"),
only three of eight openai replies have one: 03 (first hand, off its `own` line), 06 (a referral —
*"Ask the Stone House warders if you need it confirmed"*), 08 (first hand). The other five —
01, 02, 04, 05, 07 — are **the rendered fact plus its hedge and nothing else**, so they collapse to
one: **4 of 8.** The lexical metric agrees, and much more sharply than it did on moonshot:

| | mean pairwise Jaccard | max | pairs ≥ 0.60, of 28 |
|---|---|---|---|
| moonshot / kimi-k3 | **0.24** | 0.50 | **0** |
| openai / gpt-5.6-luna | **0.41** | **0.72** | **5** (05~07 0.72, 01~07 0.71, 06~07 0.68, 04~07 0.63, 07~08 0.60) |

The mechanism is visible in one number: **openai's replies are terser** — mean 138 B against
moonshot's 204 B over the same fifteen sheets. A one-sentence reply that must carry the fact and its
hedge has no room left for a clause of its own, so the shared core is the whole reply. This is not a
different failure from risk 2; it is risk 2 arriving through reply length. So:

> **Risk 2 is NOT closed by prose alone, and the prose is not what closes it.** M0's mitigation for
> parroting held on one provider of two. The measures that do not depend on a model's verbosity are
> the structural ones the spec already schedules: give hops 2/3/4 their **own rungs** (finding 2
> below — six of eight holders received one of only two rendered lines, so these numbers are a floor
> on both providers), and **M3's garbling**, which diverges the `said` text itself. Both are now
> load-bearing rather than nice-to-have. Re-measure Q4 on both providers after each.

Two smaller cross-provider results:

- **The exemplar leak reproduces.** openai lifted v2's own bracketed exemplars too — *"Ask the
  weigher at the Stone Gate"* is the rule's `("that is the weigher's, not mine")` and *"that is
  their business"* echoes it. Moonshot did the same with the porter exemplar in 4 of 22. **M1 repair
  1 (rotate or drop the exemplars) is confirmed on two providers, not one.**
- **Q5's under-firing reproduces, and it is now a signal.** openai's with-occasion reply is *"I don't
  know. That is the toll-house's business, not mine."* — 0 uses of `raise_word`, reaching for the
  ignorance rule instead, exactly as moonshot did. Two providers of two now decline the verb on an
  occasion with this paragraph on the sheet. Unlike round 1's moonshot result this one is **not**
  fence-contamination (openai did not copy the example line), so it stands as evidence that **the
  ignorance rule competes with `raise_word`**. Finding 4 stands and gains a clause: M4 must re-fire
  the occasion with an unrelated example *and* watch for suppression by this paragraph.
- Q3 shape uniformity: openai's five refusals share a visible template (*"I don't know X. Ask Y."*).
  Lexically it is not a failure — mean pairwise Jaccard 0.19, max 0.29, no pair ≥ 0.60, against
  moonshot's 0.09/0.14 — so it is recorded as an observation, not a measured threshold breach.

## Step 2 — the placement check: the frozen bytes measured where they will actually sit

Round 1's harness rendered the ignorance rule low on the sheet, just before *"Output like this, and
only like this"*. In `turn.j2` it goes **immediately before "Use ONLY the verbs listed below"**
(turn.j2:194) — about forty lines earlier, ahead of the verb list instead of behind it. A moved
paragraph is a changed prompt, so rather than freeze an unmeasured position it was measured:

| | |
|---|---|
| How | `scripts/m0/move_rule.py` excised the frozen paragraph from each measured v2 sheet and re-inserted it before the verb-list anchor. **Byte length asserted unchanged per sheet** — same bytes, different position |
| Sheets | the six diagnostic ones: the five `q3_nonholder_asked_*` plus `q1_holder_relevant_unasked` |
| Provider / model | `moonshot` / **kimi-k3**, the provider the decision was made on |
| Calls | 6, **ok 6 / failed 0** (`replies/placement_turnj2_position/`) |

**Result: the go/no-go holds at the shipping position, 5/5, with 0 invented specifics in 6 replies.**
Every referral handle verified in its own sheet: *"Ask a porter on the Wool Gate"* (its own bullet);
*"Ask the porters on the gate"*; *"ask Bertran over there, or check at the Stone Gate"* (`Bertran
Lark, 4.8 m` in `you_see`, `pl_zd9m` in `places_you_know`); *"ask the porters up by the Bonded
warehouse"* (**verbatim in her own `you` line** — "Home: a house in the Weigh Ward, near Bonded
warehouse"); *"the porters at the gate or the wardens who walk the walls"* (his own `you` line reads
"Leather worker, warden").

**And round 1's single Q1 failure is repaired at this position.** The butcher now volunteers the
arrest he holds as the reason for the refusal it justifies:

> *"Rohese, I am sorry for it, but the Ashe slate is closed. Grigor has owed me for a side since the
> Waning, and **with him lying in the Stone House** I will not chalk another. Nine sparks, and you
> carry it with my goodwill."*

That is the one thing v2 got wrong in twenty-two replies, and it came right with the paragraph moved
and not one byte altered. **n = 1**, so this is a candidate cause, not a finding — but it has a
consequence for M1: **re-check the "soften the let-it-lie clause" repair before applying it**, because
it may be fixing something that was position and not wording. Nor did any of the six lift the porter
exemplar (0 of 6, against 4 of 22 lower down); also n-limited, also worth re-measuring.

## The call ledger and the spend

| run | provider / model | calls | outcome | date |
|---|---|---|---|---|
| `replies/_rig_check/` | moonshot / kimi-k3 | 1 | ok | 2026-09-03 |
| `replies/round1/v1_spec/` | moonshot / kimi-k3 | 22 | 22 ok, 0 failed | 2026-09-03 |
| `replies/round1/v2_structural/` | moonshot / kimi-k3 | 22 | 22 ok, 0 failed | 2026-09-03 |
| `replies/round1/v3_register/` | moonshot / kimi-k3 | 22 | 22 ok, 0 failed | 2026-09-03 |
| `replies/cross_openai/` | openai / gpt-5.6-luna | 15 | 15 ok, 0 failed | 2026-09-03 |
| `replies/placement_turnj2_position/` | moonshot / kimi-k3 | 6 | 6 ok, 0 failed | 2026-09-03 |
| | | **88** | **88 ok, 0 failed** | |

Plus **12 unbilled failed attempts**: the first placement run went out on the harness's default
moonshot model and every call came back `provider returned 404: {"error":{"message":"Not found the
model kimi-k2.5 or Permission denied"}}`. Recorded rather than swept up, because it is a live fact
about the tree and not about M0: **`kimi-k2.5` — the default in `cathedral_headless.rs:1393` and the
only moonshot row in `llm.rs:86`'s pricing table — no longer exists at the provider.** Every M0 run
needed `LLM_MODEL=kimi-k3`. Not fixed here (M0 may not touch shipped files); somebody should.

**Spend: not recorded, and therefore estimated.** `--one-shot` prints no token usage and `RUN.json`
carries none, so there is no measured figure to quote. Order of magnitude from `llm.rs:86`
($0.60/$3.00 per Mtok for the moonshot row, $1.00/$6.00 for `gpt-5.6-luna`) and the measured sheet
sizes (~12.6–14.7 KB in, ~65–384 B out): roughly **$0.15–0.25 for the whole of M0**, 88 calls.
kimi-k3 has no row in that table, so even the moonshot half of the estimate is by analogy. Cents,
either way — which was the premise the milestone was scheduled on and it held.

## The three thresholds, final

Measured, on the frozen variant, on both providers. Denominators as above.

| threshold | risk | moonshot / kimi-k3 (22 sheets) | openai / gpt-5.6-luna (15 sheets) | at the turn.j2 position (6 sheets) |
|---|---|---|---|---|
| 1 — non-holder asked: refuse **and** refer, ≥ 4/5 | 1 (go/no-go) | **5 / 5 PASS** | **5 / 5 PASS** | **5 / 5 PASS** |
| 2 — eight holders materially distinct, ≥ 6/8 | 2 | **7 / 8 PASS** (range 6–8) | **4 / 8 FAIL** | not fired |
| 3 — `raise_word` with no occasion, 0 uses | 4 | **0 PASS** | **0 PASS** | not fired |
| invented names / days / places / numbers | 1 | **0 of 22** | **0 of 15** | **0 of 6** |
| invented **person** names | 1 | **0** | **0** | **0** |
| malformed as turn replies (`parse_reply`) | 5 | **0 of 66** | **0 of 15** | **0 of 6** |

## Verdict

**GO.** The load-bearing risk is answered. Across 43 replies from the frozen wording, on two
providers and in two prompt positions, a non-holder asked point-blank for a name, a day, a place, a
number and an agent **refused every time and named a next mouth every time — 15 of 15** — with **zero
invented specifics in 43 replies**, on blockless sheets as well as held ones. The spec's fallback
(`who_keeps_that_word`) is not invoked and stays on the shelf.

**`v2_structural` is frozen.** Its measured bytes are transcribed, unaltered, into two artifacts:

- `strings_draft.toml` — the 18 `PromptStrings` keys, values verified to round-trip to
  `prose/v2_structural/` byte for byte, placeholders translated to `strings.toml`'s own `%s`.
- `ignorance_rule.txt` — the unconditional `turn.j2` paragraph, with its position, its
  unconditionality and its measurement recorded.

**One threshold does not pass, and it is not the go/no-go.** Risk 2 (parroting) passes on one
provider and fails on the other, and prose is not what will close it — the hop-rung split and M3's
garbling are, and they are now required rather than optional. That is a scope statement for M1–M3,
not a reason to hold M0.

----

## M0b — measured repairs (2026-09-04)

M0 froze `v2_structural` with three named defects it could not itself measure and one fixture bug.
M0b is the round that measured them, so that M1 transcribes measured text and nothing else. The
work was split: a firing agent authored one-axis variants (`prose/M0B_VARIANTS.txt`) and fired them
at both providers at the **shipping prompt position** (`replies/M0B_MATRIX.txt`); two independent
scorers marked the replies (`scoring/m0b_moonshot.md`, `scoring/m0b_openai.md`); this section is the
verdict, written after reading every reply again rather than taking either marker on trust. Every
quote below is verbatim from `replies/m0b_moonshot/` or `replies/m0b_openai/`; every handle and
specific was grepped in the sheet that produced it (`sheets/<variant>_turnj2/`). Nothing under
`crates/`, `src/`, `assets/`, `tests/` or `config.ron` was touched; `scripts/m0b/` (M0's harness
restored, plus the two optional rungs and `move_rule.py`) was deleted at the end, as M0's was.

### What was run

| | |
|---|---|
| Harness | `scripts/m0b/build_sheets.py` (M0's, with `hops3`/`hops4` as optional all-or-nothing rungs, so a 5-rung variant renders byte-for-byte as M0 rendered it) → `move_rule.py` (excise the rule paragraph, re-insert it immediately before "Use ONLY the verbs listed below"; asserted per sheet: the excised text is the variant's own `ignorance_rule.txt` and the sheet's byte length is unchanged) → `fire.sh` → the binary called directly, `cathedral-headless --provider <p> --one-shot <sheet>`, the sheet sent verbatim as the whole prompt |
| Providers / models | `moonshot` / **kimi-k3** (`LLM_MODEL=kimi-k3`, mandatory — the repo's default `kimi-k2.5` no longer exists at the provider); `openai` / **gpt-5.6-luna**. Byte-identical sheets to both, so a difference between the two reply directories is the model and nothing else |
| Repo state | git `f56a2c306ec1523fc569c7c2de232b8c90d4d4ef`, branch `develop` (every `RUN.json`) |
| Position | the shipping one, for every call: the rule sits at sheet line 103, "Use ONLY the verbs" at 126. M0's round 1 numbers were taken ~40 lines lower; M0's placement run (6 calls) is the only earlier measurement at this position |
| Scenarios | M0's 22, with `q4_wick_03`'s `own` line re-authored (below), plus two M0b additions, `q2_holder_hops3_{top,low}_band` — byte-identical to their hops-4 twins but for the rendered rung (verified: exactly 2 changed lines per pair) |
| Variants | `v4_rungs` (R1 alone), `v5_exemplar` (R2 alone), `v6_both` (R1+R2, the candidate), `v7_letitlie` (v6 + R3), `v8_bare` (v6 with R2 done by dropping the exemplars instead of describing the moves — fired beyond the brief, provenance now in `prose/M0B_VARIANTS.txt`) |
| Calls | **110**: per provider v4 8, v5 8, v6 15, v7 1, v6 bands 2, v6 bands hops3 2, v8 19 = 55 × 2. **110 ok, 0 failed, 0 retries**; every `.err` file empty |
| Parse validity | **110 of 110** accepted by the port of `parse_reply`; 0 empty action lists. Verbs: moonshot `say` 58 / `remember` 5 / `forget` 1 / `go_to` 1; openai `say` 55 / `go_to` 2 / `remember` 1. `raise_word`: **0 in 110** |
| Prose cost | `ignorance_rule.txt` 1379 → **1386 B** (22 lines, two lines changed); `hedges.toml` 15 → **21 rungs** (a sheet renders one rung per held fact, so the per-sheet cost is the rung's own length: the new default rungs are 76–132 characters against the retired one's 81); `block_note.txt` unchanged, 676 B |

### Corrections to the M0 record

1. **`q4_wick_03_osanne_vell` was a self-contradicting fixture** (M0 finding 3). Its `own` line said
   *"at my own door"* while the canonical `said` says *"at the Wickmarket"*, and `hops0_own` renders
   the own line alone, so the model never saw the Wickmarket. Re-authored before this round to
   *"I was standing in my own doorway on the Wickmarket when they took {subject} — he did not say one
   word, and the sergeant had his arm up behind his back"* — only the place added. No reply of M0b's
   eight fires of that sheet contradicts the Wickmarket; the three "invented place" scores M0's slice
   markers booked against it were the sheet, not the model. Consequence: rebuilding
   `sheets/v2_structural/` from the current scenarios reproduces **21 of 22** sheets; the 22nd is
   this one. `sheets/v2_structural/` is left byte-unchanged as the record of what M0 actually fired.
2. **`top_band.hops3` and `low_band.hops3` were fired after all.** `replies/M0B_MATRIX.txt` and the
   first pass of both scorings say those two rungs "remain unfired"; the firing agent then authored
   the two hops-3 scenarios and fired them at `v6_both` on both providers
   (`replies/m0b_<provider>/v6_both_bands_hops3/`). **All 21 hedge keys have now reached both
   providers at least once.** The stale line is corrected in place, dated, in `M0B_MATRIX.txt`.
3. **`v8_bare` had no provenance.** A fifth variant, 19 sheets × 2 providers, not in
   `M0B_VARIANTS.txt` or `M0B_MATRIX.txt` when the scorers read it. Its prose is in `prose/v8_bare/`
   (`diff -r` against `v6_both`: `ignorance_rule.txt` only, the two bullets reduced to *"- name the
   trade that would know;"* / *"- name the post whose business it is;"*), its sheets are now in
   `sheets/v8_bare/` and `sheets/v8_bare_turnj2/` (each differs from its `v6_both` twin in exactly the
   two bullet lines — verified on all 24), and a provenance section is appended to `M0B_VARIANTS.txt`.
4. **The hops-3 and `v8_bare` sheets are in the repo.** The moonshot scoring's evidence gap 1 said
   they existed only in the firing scratchpad; the openai scorer copied them in, `cmp`-identical to
   the fired files, before that scratchpad went. Closed; noted at the end of the moonshot scoring.
5. **Jaccard calibration.** Both scorers re-ran M0's content-word Jaccard and reproduce M0's
   moonshot figures exactly (0.239 / 0.500 / 0). They differ from each other by one stop-word
   (`cross_score.py` drops "yes", `distinctness.py` does not): openai `v6_both` is 0.421 by the
   second and 0.426 by the first. M0's published openai baseline 0.41 / 0.72 / **5** recomputes as
   0.400 / 0.722 / **4** — the fifth pair (07~08) is 0.562 and sat on the 0.60 boundary in M0's
   rounding. Comparable within rounding; nothing below turns on it.

### The three repairs, and the verdict on each

**R1 — give hops 2, 3 and 4 their own rungs: TAKEN, as structure, and NOT billed as the
anti-parroting fix.** `hedges.toml` goes 5 × 3 → 7 × 3; unchanged byte-for-byte are `hops0_own`,
`hops0`, `hops1` and `cold` in every band plus `top_band.hops2` ("They say: %s") and `low_band.hops2`;
retired is `default.hops2` *"Third-hand, mouth to mouth, and you could not say whose word it was to
begin with"* (an ordinal — the count in words — and wrong at three and four removes by construction);
new are `default.hops2/3/4`, `top_band.hops3/4`, `low_band.hops3/4`, each a different *shape* (who
was and was not there / the line it came down / what the telling has become), one `%s` each, no
count and no ordinal. What it measurably bought: the eight holders render **six** distinct lines
instead of four; the hop-count word is gone by construction — 0 of 24 openai holder replies and 0 of
24 moonshot ones under the new ladder coined "third-hand", against 4 of 8 at openai's baseline; and
erosion is legible at three **and** four removes in both bands on both providers (below). What it did
**not** buy is the thing it was written for: threshold 2 on openai is 3/8 under every R1 variant and
3/8 under the old ladder at the same position (`v5_exemplar`), so on the provider that needed it the
ladder is irrelevant to the count at n = 8; and on moonshot the same ladder gives 4 / 6 / 4 across
`v4` / `v6` / `v8`, which is the noise floor. Costs, both accepted: the new rungs are lifted as readily
as the old (4 of the 7 moonshot matrix mouths that got a new rung said a phrase of it back; the old
rung was lifted by 3 of 4), and moonshot volunteered a *wrong* ordinal three times where the old rung
was echoed correctly (flag F1, below).

**R2 — rotate or drop the referral exemplars: TAKEN, in the describe form.** Bullets 1 and 2 of the
ignorance rule become *"- name the trade that would know it, whoever handles that sort of thing all
day;"* and *"- name the post whose business it is, or the officer who keeps such things;"*. The
alternating-exemplar option was not fired: the paragraph is one static string, so alternation is two
quotable half-lines per bullet, not none. The drop form was fired (`v8_bare`) and is the control that
shows the descriptions earn their keep. On openai `v6_both` lifted **0** old exemplars in 55 replies
("porter" 0, "weigher" 0, "stand on that gate" 0, "not mine" 0) against **3 of 7** at M0's baseline,
while `v8_bare` regrew a closing ritual — *"…is their business"* in **4 of 5** refusals (`v6` 1 of 5,
baseline 2 of 5). On moonshot the old porter half-line M0 recorded byte-identical on `asked_number` is
gone, but one reply of 55 lifted the **new** clause — the tanner: *"ask the cargo brokers at the Weigh
Ward—they handle that sort of thing all day"* — so like-for-like at this position it is old 0 of 6 vs
new 1 of 6, and the firing agent's "nothing left that can be lifted into speech" is falsified by one
tanner. `v8_bare` lifted nothing (0 of 19) and paid on the other side: its `asked_day` is v1_spec's
dead end again — *"I don't know the day, stranger. I only heard it second-hand, from someone who
wasn't there themselves."* — refuses, refers nobody, threshold 1 at 4/5. Between one lift and a
returned dead end, the describe form is the better trade on both providers.

**R3 — soften "and otherwise let it lie": DECLINED; the frozen clause stays.** `v7_letitlie` appended
*"Even so, a thing you hold that bears on what is being asked of you right now, or on what you are
about to refuse somebody, is not a thing to sit on: say that one."* to the block note (everything
else `v6_both`), and the pair `v6_both` / `v7_letitlie` on `q1_holder_relevant_unasked` at the shipping
position is the test. **Both arms miss on both providers.** openai, frozen: *"Not on the slate,
Rohese. Coin now for the side—your household already has Grigor's old debt hanging there."*; openai,
softened: *"Not on the slate, Rohese. Grigor's side is still unpaid. Nine sparks in hand, and the
mutton is yours."*; `v8_bare`'s frozen-clause control misses the same way. moonshot, frozen: *"The
slate, aye. Though the slate's older than the side, Rohese — Grigor's name has been on it since the
Waning, and no coin up yet."* — he **grants** the credit and never says the arrest; moonshot,
softened: *"Rohese. The Ashe slate is two years cold, as you know — Grigor's side from the Waning was
never paid for. I put nothing more on it. Coin in hand, or no mutton."* — M0's original miss exactly,
plus **the one hard invention of the 110**: "two years" is Petronel Crake's debt in his own
`back_story` (*"patient with any man's debt except Petronel Crake's, which is two years old"*); the
Ashe debt is dated only "since the Waning". And `v8_bare`'s frozen-clause moonshot fire passes: *"On
the Ashe slate, Rohese? Grigor has owed me for a side since the Waning, and now they say he lies in
the Stone House. Who pays his slate while he is there?"*. So the frozen clause at this position is
now **pass (M0 placement) / fail (`v6`) / pass (`v8`)** in three n = 1 moonshot fires on prompts that
differ only in rungs the Q1 sheet barely uses and two bullets it never uses, and **miss / miss /
miss** on openai across three wordings. Neither the wording nor the position is the cause: on
moonshot Q1 is noise at n = 1, on openai it is terseness (135–142 B: one refusal and one reason, no
room for a second reason). M0's finding 1 stands — the relevant-fact miss belongs to M1's relevance
selection, not to prose — and the softened sentence would have put 163 unjustified bytes and one
measured invention into `strings.toml`.

### Before / after, per provider

Threshold 1 = the five `q3_nonholder_asked_*` sheets (refuse **and** name a next mouth, ≥ 4/5);
threshold 2 = the eight `q4_wick_*` holders (materially distinct, ≥ 6/8, by M0's rule); threshold 3 =
`q5_raise_word_no_occasion` (0 uses). "M0" is the frozen `v2_structural`; "M0b" is `v6_both`.

| | moonshot / kimi-k3, M0 | moonshot, **M0b** | openai / gpt-5.6-luna, M0 | openai, **M0b** |
|---|---|---|---|---|
| 1 — refuse and refer | **5 / 5** (and 5/5 at this position) | **5 / 5** | **5 / 5** | **5 / 5** |
| 2 — eight holders distinct | **7 / 8** (range 6–8) | **6 / 8** (range 5–6; the 6 credits a false eyewitness claim) | **4 / 8 FAIL** | **3 / 8 FAIL** (range 3–5) |
| 3 — `raise_word`, no occasion | 0 | 0 (0 in 55) | 0 | 0 (0 in 55) |
| Jaccard on the eight: mean / max / pairs ≥ 0.60 | 0.24 / 0.50 / 0 | 0.29 / 0.71 / 1 | 0.40 / 0.72 / 4 | 0.42 / 0.81 / 5 |
| mean reply bytes, the eight | 223 | 216 | 162 | 159 |
| mean reply bytes, the five refusals | 224 | 226 | 117 | 118 |
| hop-count word in the eight | 1 ("third-hand", 05) | 1 ("second-hand", 05 — wrong) | 4 ("third-hand", 04–07) | **0** |
| old exemplar lifted, at this position | 0 of 6 | 0 of 6 (1 of 6 lifted the **new** clause) | 3 of 7 | **0 of 7** |
| invented name / day / place / number | 0 of 22 | 0 hard in 15 (+1 band reply with 2 soft referral-place paraphrases) | 0 of 15 | 0 of 15 (+0 in 4 band) |
| invented person names | 0 | 0 in 55 | 0 | 0 in 55 |
| Q1, holder volunteers the relevant fact | fail (low), pass (this position) | **fail** (`v7` fail + invention; `v8` pass) | not fired | **miss** (`v7` miss, `v8` miss) |

The other variants, threshold 2 only, for the noise floor: moonshot `v4` 4/8 (0.25 / 0.50 / 0),
`v5` 4/8 (0.26 / 0.50 / 0), `v8` 4/8 (0.36 / 0.80 / 6); openai `v4` 3/8 (0.48 / 0.93 / 7), `v5` 3/8
(0.40 / 0.75 / 2), `v8` 3/8 (0.45 / 0.93 / 6). `v5_exemplar`, which carries the **unchanged**
collapsed ladder, posts the best openai Jaccard of the five sets: at n = 8, one call per sheet, these
deltas are sampling noise, and no variant shows a signal above it.

### Threshold 2, read again — both providers, `v6_both`, all eight

Rule, M0's, unchanged: a reply is its own sentence iff it carries a clause that is the mouth's own —
an opinion, a question back, a referral, a contradiction, a warning, a first-hand claim, a trade
self-description, an oath off its own sheet. Fact + hedge only collapses, and all such replies in a
set count as one. Provenance restated in the mouth's own pronouns is the hedge; a mouth-added
disclaimer ("I did not see it myself") is hedging. All eight sheets in a variant differ only in
`you`, `back_story` and line 207 (the rung); `stored_memories — none` on every one, so a first-hand
claim outside 03 (own line) and 08 (hops 0) is a fabrication.

**openai / gpt-5.6-luna — 3 of 8, FAIL.** Own: **03** *"It is. I stood in my doorway when they took
Grigor Ashe; he said not a word, and the sergeant had his arm twisted behind him."* and **08** *"Aye. I
saw Grigor Ashe taken here two days past. He lies in the Stone House now."* Everything else is the
rung read back: **01** (hops 1) *"They say Grigor Ashe was taken here two days past and lies in the
Stone House. The one who told me was there."* — **byte-identical** to `v4_rungs/02` and `v8_bare/02`,
two personas and three prompt variants producing one sentence; **02** (hops 1) *"Aye, so they say.
Grigor Ashe was taken here two days past and lies in the Stone House. I did not see the taking
myself."*; **04** (hops 2) *"Aye, so I was told—but the teller wasn't there either. They said Grigor
Ashe was taken here two days past and lies in the Stone House."*; **05** (hops 2) *"I was told Grigor
Ashe was taken at the Wickmarket two days past and lies in the Stone House. The one who told me was
not there either."*; **06** (hops 3) *"It came to me down a line of mouths, but yes—the word is that
Grigor Ashe was taken at the Wickmarket two days past and lies in the Stone House."* — the rung's
opening verbatim but for the pronoun, then "but yes" over a three-remove telling; **07** (hops 4)
*"It has come through many mouths that Grigor Ashe was taken at the Wickmarket two days past and lies
in the Stone House. I cannot say who began it."* Range [3, 5]: 5 only by crediting 02's and 06's
disclaimers, which the rule forbids; nothing reaches 6 without counting rung paraphrase as the
mouth's own words, which is the thing the threshold exists to catch. M0's baseline 06 had a referral
(*"Ask the Stone House warders if you need it confirmed"*) and that is the whole of the 4 → 3.

**moonshot / kimi-k3 — 6 of 8, PASS at the line, and the sixth is a register failure.** Own, firm:
**02** *"True as the slate on my door. Grigor Ashe was taken right here on the Wickmarket, two days
past, and lies in the Stone House — so I heard from one who was standing there. Now, are you buying
ale, or only gathering talk?"* — an oath off her `back_story` (*"the slate hangs by the door where
every debtor can read his own name"*), a question back, provenance kept right; the best reply of the
110. **03** *"…I was standing in my own doorway when they took him — he did not say one word, and the
sergeant had his arm up behind his back. A hard sight on a market morning."* **05** (hops 2) *"…it
came to me second-hand. If it's the taking itself you want, the watch at the Stone House would know
more than I do."* — a referral. **08** *"Aye. Saw it myself, two days past. They took him here at the
market. He lies in the Stone House now."* Own by M0's precedent only: **01** (hops 1, no memories)
*"Aye, it's true. Saw it with my own eyes, I did. They took Grigor Ashe right here in the Wickmarket,
two days past. He lies in the Stone House now."* — a first-hand claim, which the rule counts (M0
counted `v3`'s confabulated 01 the same way), and which is **false**: the only reason he is a
separate sentence is that he corrupted his rung. Shared: **04** (hops 2) *"…Mind, I wasn't standing
there myself when it happened; I had it from one who had it from the one who was. But it's all over
the market."* — the rung paraphrased plus corroboration the sheet does not give (the shape of M0's
`v3` defect, milder; not an opinion); **06** (hops 3) *"Aye, that's what they say. Grigor Ashe was
taken right here at the Wickmarket, two days past. They say he's in the Stone House now."* — the
longest new rung in the set produced the barest reply in it; **07** (hops 4) *"Aye, it's true. They
took Grigor Ashe right here at the Wickmarket, two days past. He's in the Stone House now."* — four
removes as flat fact, no hedge, and Jaccard **0.71** with 01, a hops-1 reply — R1's premise failing
in the one place the metric can see it. So: 4 firm + 01 + one cluster = **6**; on the strict reading
(a false eyewitness claim is not a clause of the mouth's own) it is **5 and fails**. The moonshot
scorer's [5, 7] allowed 04's "all over the market" as an opinion; I do not. Point 6, range [5, 6],
against M0's 7/8 at the low position — inside the ±2 the same ladder shows across `v4`/`v6`/`v8`.

Two hard facts survive the noise on openai and no ladder fixes them: **04 and 05 are both authored at
hops 2**, so R1 cannot separate them (0.61–0.88 in every R1 variant); and the hops-1 sentence came
back **byte-identical from three sheets**. On openai a holder recites the rung and the persona
contributes nothing to the sentence; the personas are rich (a lame baker, a brewer with a slate by
her door, the Tracer, a thief, a seventeen-year-old lamplighter, a bribed ex-weigher) and moonshot
got own clauses out of the same bytes, so the sheets are not the constraint — reply length is.
`v6_both`'s eight average 159 B on openai against 216 on moonshot, unmoved from M0's 162 / 223.

### Threshold 1, `v6_both`, both providers — 5 / 5 and 5 / 5

Every handle was found on its own sheet; a bare trade or post need not be (M0's precedent). Two of
the five sheets carry no `what_you_know` block at all.

| sheet | moonshot | openai |
|---|---|---|
| asked_day (Idonea Pell; her one bullet is the unknown carter put off the Brede cart at the Wool Gate) | *"I couldn't tell you the day, friend. I only heard it second-hand myself, and no day was given with it. If it's the cart's business you're after, ask a carter at the Wool Gate — they keep their own tally of who comes and goes."* — carter and Wool Gate are the bullet's own; "second-hand" on a two-remove bullet is F1 | *"I don't know which day. It came by word from a carter of the Weigh Ward; ask there."* — the bullet's own unknown subject, miscast as its source (M0 passed the identical slip) |
| asked_name (Petronel Roper) | *"I don't know that name, nor any officer. If it's the watch you want, ask at the Tallage — they keep their own."* — `The Tallage` in `places_you_know` | *"I don't know. Ask the ward watch—their business is taking people."* — bare post |
| asked_number (Rohese Kern, **no block**) | *"Pounds over? I don't know what you're asking, friend. I'm a carpenter, not a carter—if it's about a cart or a load, you'd want the teamsters or the porters at the Stone Gate. If it's about a beam, I've got my hands full with this ridge piece here."* — Stone Gate in `places_you_know`, Carpenter in `you`, the ridge piece is `current_goal` | *"I don't know that weight. Ask the cartwright or a weighman who saw the Brede cart."* — bare trades; the cart is the player's word |
| asked_place (Ede Hobbe, **no block**) | *"Grigor Ashe? Never heard the name, love. I wash linen, I don't take folk anywhere. If it's the watch took him, you'd want the ward post by the Tally Bridge — ask there."* — Tally Bridge on the sheet; a ward post is not (flag F4: an institution given a location; M0 passed "ask at the ward post") | *"I don't know where they took Grigor Ashe. Ask the watch—they'd keep word of that."* |
| asked_who_did_it (Gile Thorn; his bullet is the unknown cargo broker of the Weigh Ward) | *"I don't know who cut the cord. I was at my bench with the hides. If you want answers about bales and beams, ask the cargo brokers at the Weigh Ward—they handle that sort of thing all day."* — bullet's own subject; the tail is the one lift of R2's new clause | *"I don't know who cut it. Ask a cargo broker in the Weigh Ward; they handle the bales and their loading."* — bullet's own subject |

Invented person names in the 110: **0**. Every `say` target is a person in that sheet's `you_see`;
both `go_to` ids are in `places_you_know`.

### Threshold 3 — 0 uses

`v6_both/q5_raise_word_no_occasion`: moonshot `say {"target": "ft3tb", "text": "Aye. This one, then
one more."}` + `go_to {"place_id": "pl_nve1"}`; openai `say {"target":"ft3tb","text":"Aye. Two
more."}` + `go_to {"place_id":"pl_nve1"}` (Sef at 4.4 m, who said *"Two more sacks and the cellar is
clear"*; Gaunt Passage in `places_you_know`). `raise_word` appears in **none of the 110** replies.
Unchanged from M0 on both providers.

### Inventions — every name, day, place or number, all 110

**Hard: 1 of 110** — `v7_letitlie/q1` on moonshot, *"The Ashe slate is two years cold"* (above). In
`v6_both`'s own 19 + 19 replies (15 diagnostic + 4 band, each provider): **0 hard on either.**

**Soft — referral targets paraphrased into places the sheet never gives, all on the top-band
`Bed` sheets (a scandal with no obvious next mouth on the sheet), all moonshot, none in the 32
threshold replies:** `v6_both_bands/q2_holder_hops4_top_band` *"ask at the Snuffing itself or find one
of the gossips who watch the Doctor's lane"* (sheet: "Doctor Ferrant's door", no lane, no watchers;
the Snuffing is a time of day); `v8_bare/q2_holder_hops3_top_band` *"ask at his surgery; if it's hers,
the Sedge household"*; `v8_bare/q2_holder_hops4_top_band` *"ask the gossips at the Snuffing"*. Same
mechanism each time — the rule's referral obligation ("do one of these, not none of them") filled from
what would make sense when the sheet offers no mouth. Counted soft because each is a where-to-ask, not
a fact asserted about somebody; it is the only place in 110 replies where the rule's own referral
clause produces the completion it forbids, and it is worth a headless check in M1 on a sheet with a
held scandal and nobody to send the asker to.

**Flags, not inventions:** **F1** a wrong ordinal volunteered with the new rungs — *"second-hand"* for
two removes (`v6/asked_day`, `v6/q4_05`) and for **three** (`v6_both_bands_hops3` low), moonshot only;
fewer removes than it came with is *"firmer than it came to you"*, the block note's own prohibition,
and the old rung never caused it because it supplied the word. **F2** a supplied agent, `v4/q4_05`
*"the talk is the watch took him"* — "watch" is nowhere on her sheet. **F3** unsupported corroboration,
*"it's all over the market"* (`v6/q4_04`, `v8/q4_04`), *"it's what the ward is saying"* (`v5/q4_06`).
**F4** the ward post by the Tally Bridge. On openai: `v6/asked_day` casting the bullet's subject as its
source; `v6/q4_06`'s "but yes" over three removes; `v8/asked_name`'s garbled *"naming officers taken
is their business"*. The butcher's *"It was walking yesterday"* / *"killed before the light"* are his
own trade, allowed by "None of this binds your own trade".

### The band fires — erosion at three and four removes

Two personas (Averil Tarn, toll clerk, `Bed`/top; Ewart Tarn, weigher, `Craft`/low), each on two
sheets identical but for line 193. `v6_both`:

| rung | moonshot | openai |
|---|---|---|
| top hops3 *"It is the common word now…"* | *"Only what the whole yard's saying, friend: that Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing, and not for physic. What passes between them behind that door, I've not seen and couldn't tell you."* — hedged once, the store's edge stated | *"I don't know what lies between them. The common word is only that Rohese Sedge visits Doctor Ferrant after the Snuffing, and not for physic. Doctor Ferrant is the one to ask."* — rung phrase lifted, plus a referral of its own |
| top hops4 *"This is simply what is known in this city now…"* | *"That's the talk of the whole ward, isn't it? They say Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing, and not for physic. But I keep my head in the toll rolls…"* — own clause grounded; carries the soft invention above | *"I don't know what lies between them. I only know Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing, and it is not for physic. Doctor Ferrant is the one to ask."* — flat, as known; one rung less hedge than hops3 |
| low hops3 *"Passed to you at a remove you cannot account for…"* | *"Short weight, that's what's wrong with it — and cut with bean meal besides, so I'm told. Sent to the Wickmarket bakehouse. Mind, it's come to me second-hand and I can't vouch for the mouth it came from… no man in thirty years has proved me a grain out."* — hedged three times (thirty years is `back_story`); "second-hand" is F1 | *"I was told Bertran Hobbe's mill sent short weight to the Wickmarket bakehouse, and that the flour was cut with bean meal."* — **partial**: the hedge thinned to a bare "I was told", a three-remove telling less hedged than the same band at four |
| low hops4 *"Stall talk, handed along so often that its edges are gone…"* | *"Short weight, so the stall talk runs … I've never met a soul who was there to see it, mind, so I pass it on as it came to me. But if it's Hobbe's flour you're weighing, bring it to my beam…"* — two rung phrases lifted; every specific grounded | *"It is stall talk only, never proved to me: Bertran Hobbe's mill was said to send short weight to the Wickmarket bakehouse, with the flour cut with bean meal."* — hedge kept, chain paraphrased |

**The erosion relationship of `02_rumor_pollen.md` survives the split** and reads at both distances
on both providers: top-band one hedge, low-band three (moonshot); "common word" / "I only know"
against "I was told" / "stall talk only" (openai). The top band steps one rung per hop. `low_hops3` is
the weak rung of the 21 — under-hedged on openai, mis-counted on moonshot, n = 1 each — and it ships
labelled, not silently.

### Wordings tried and rejected, with why

Everything here was actually written; *(measured)* means fired, and its replies are in `replies/`.

- **The retired `default.hops2`, `"Third-hand, mouth to mouth, and you could not say whose word it
  was to begin with: %s"`** *(measured, M0 and `v5_exemplar`)* — an ordinal of removes is the count
  in words; it is wrong at three and four removes by construction (M0's ladder sent them here too);
  it was recited by 3 of the 4 moonshot mouths that got it in `v5` and by 4 of 8 openai holders at
  M0's baseline. Replaced by three rungs with no ordinal.
- **`v7_letitlie`'s appended block-note sentence** *(measured, 1 + 1)*: *"Even so, a thing you hold
  that bears on what is being asked of you right now, or on what you are about to refuse somebody,
  is not a thing to sit on: say that one."* — 0/1 on each provider against a frozen-clause control
  that is also 0/1, and the round's one hard invented number. M0's carry-forward proposal for the
  same repair (*"— but a thing that bears on what is being asked of you right now is not one to sit
  on."*) was not fired separately; the fired form names the exact failure shape and still lost.
- **`v8_bare`'s bare bullets** *(measured, 19 + 19)*: *"- name the trade that would know;"* /
  *"- name the post whose business it is;"* — the "drop" half of R2. 0 lifts (nothing to lift), but
  the bare bullet's one content noun becomes the ritual (*"…is their business"* 4 of 5 on openai),
  moonshot's `asked_day` returns to v1_spec's dead end (threshold 1 at 4/5), and its eight holders
  are the most parroted moonshot set of the round (0.36 / 0.80 / 6).
- **M0's carry-forward R2 proposal, `- name the trade that would know ("that is a porter's business,
  not mine");`** — not fired: it keeps a quotable half-line, which is the defect. The fired form
  describes the move instead.
- **Alternating exemplars** — not fired: one static string in `strings.toml`, so "alternating" is two
  exemplars per bullet, more quotable half-lines rather than fewer.
- **A hop count, or an ordinal, in any new rung** — deliberately absent from all seven new rungs, for
  M0's reason, and confirmed: 0 of 110 replies mention a count; the only ordinals said were the
  model's own, and wrong (F1).
- **`v4_rungs` and `v5_exemplar` alone** *(measured, 8 + 8 each)* — the one-axis controls, not
  candidates: R1 alone 4/8 and 3/8, R2 alone 4/8 and 3/8. Neither repair does the other's work.

### Decision

**Ship `v6_both`.** It is the only variant that holds the go/no-go at **5 / 5 on both providers**,
threshold 3 at **0 on both**, has **0 hard inventions in its own 38 replies**, removes the exemplar
leak on the provider where it was measured (**3 of 7 → 0 of 7**) without bringing back the dead-end
refusal, renders six distinct lines for eight holders instead of four, carries no ordinal a model can
echo, and has **every one of its 21 rungs measured on both providers** at the shipping position. R3
is declined on evidence; the drop form of R2 is rejected on evidence.

**Threshold 2 did not improve on openai. It fails: 3 / 8 against M0's 4 / 8, and [3, 5] under every
defensible clustering.** That is what this round was for, and prose has now been shown twice — M0's
three variants at one position, M0b's five at the other — not to close it on a terse provider. On
moonshot it went 7 / 8 → 6 / 8 (range 5–6), a pass at the line that credits a false eyewitness claim,
inside the ±2 the same ladder shows across three fires. R1 stays in the shipping text for what it
demonstrably does (six lines, no ordinal, erosion legible at four rungs) and is **not** recorded as
the anti-parroting fix. Risk 2 is handed to **M3's garbling**, with the re-measurement below, and if
garbling alone does not reach 6 / 8 on openai the next lever is sim-side — something of the mouth's
own on the sheet (an `own` line past hops 0, a memory seated by relevance), not a longer rung.

### What stays open, and who owns it

1. **Threshold 2 on openai — FAIL, 3/8.** Owner: **M3** (garbling diverges the `said` text per
   carrier; the two measured floors are the byte-identical hops-1 sentence and 04~05 on one rung).
   Re-measure as below. If that does not reach 6/8 on openai, a sim-side seam for the mouth's own
   material is the next lever (M3/M5 design), not prose.
2. **Q1 — the relevant fact is not volunteered** (frozen clause: 1 miss of 3 on moonshot, 3 of 3 on
   openai at this position). Owner: **M1's relevance selection** — seat the fact whose subject is
   named in `since_your_last_turn`; M0's finding 1, unchanged. The prose is not the lever.
3. **F1 — wrong ordinals volunteered with the new rungs** (3 in moonshot's `v6` sets, 0 on openai).
   Accepted; **M3** re-checks it with the erosion re-measurement, since it is a hedge-register cost.
4. **Soft referral-place completion on a held scandal with no next mouth on the sheet** ("ask at the
   Snuffing", "the Doctor's lane"; 3 of the 4 moonshot top-band fires, 0 of openai's 4). Owner: **M1**'s headless
   "a third is asked and says who to ask" check, run once on a sheet like `q2_holder_hops4_top_band`;
   the spec's `who_keeps_that_word` fallback stays on the shelf unless it recurs on real sheets.
5. **`low_hops3` at n = 1 per provider, partial on openai.** Owner: **M3** (hedge erosion); ships
   labelled in `strings_draft.toml`.
6. **The ignorance rule competes with `raise_word`** on a live occasion — M0's finding, not re-fired
   here (the with-occasion sheet was not in the matrix). Owner: **M4**, unchanged.
7. **`plan/` is out of date against this record**, and this job may not edit it: `plan/M5.md`
   schedules M0b's *scoring* for M5 via `scripts/m0b/score_m0b.py` (M0b is scored here; that
   directory is deleted, as the brief required — its identical copy sits beside M0's in the
   session scratchpad, and this record is the reproducible one); `plan/M1.md` and
   `plan/03_assets.md` (D17/D18) transcribe from `prose/v2_structural/` into **six** `hedge_*`
   keys with a band-shift table, which renders `top`/hops ≥ 2 as "the one who told you was there"
   and four other band rungs with text never measured in that band — not a transcription of the 15
   measured strings, let alone the 21. Owner: **the plan fleet**, via the handoff delta.
8. **`kimi-k2.5` is still the default in `cathedral_headless.rs:1393` / `llm.rs:86`** and 404s at the
   provider; every M0/M0b moonshot run needed `LLM_MODEL=kimi-k3`. A shipped-file defect, not fixed
   here; somebody should.

### The re-measurement M3 owes

Fire the eight `q4_wick_*` sheets — same personas, same question, same `v6_both` prose at the shipping
position — with M3's garble applied per holder to the `said` text (seeded by fact sequence, carrier id,
hops), on **openai** first and moonshot second, one call per sheet. Score with M0's rule (a clause of
the mouth's own), extended for the thing garbling changes: two replies are also distinct if their
spoken fact differs in a slot the player could notice (subject, place, day). Corroborate with the
lexical metric exactly as here — every `say` text lowercased, tokens `[a-z']+` of length > 1, minus
the stop-word list `a an the and or but of to in on at for with as is it its i you he she they them
him her was were be been am are that this these those his hers their my mine your yours not no so if
then than there here now s t d ll ve m re do does did done have has had will would shall should can
could may might must aye nay o oh`, pairwise Jaccard over the 28 pairs (mean / max / pairs ≥ 0.60),
plus an md5 over the spoken text for byte-identical sentences. **Pass: ≥ 6 / 8 on openai** by the
rule, with no byte-identical pair. Baseline to beat, `v6_both` at this position: openai 3/8 and
0.42 / 0.81 / 5; moonshot 6/8 and 0.29 / 0.71 / 1. Re-run the two band pairs at the same time
(`q2_holder_hops{3,4}_{top,low}_band`) so the erosion table is re-read under garbling and `low_hops3`
gets its n = 2.

### The call ledger and the spend

| run | provider / model | calls | outcome |
|---|---|---|---|
| `replies/m0b_moonshot/{v4_rungs,v5_exemplar}` | moonshot / kimi-k3 | 8 + 8 | ok |
| `replies/m0b_moonshot/v6_both` | moonshot / kimi-k3 | 15 | ok |
| `replies/m0b_moonshot/{v7_letitlie,v6_both_bands,v6_both_bands_hops3}` | moonshot / kimi-k3 | 1 + 2 + 2 | ok |
| `replies/m0b_moonshot/v8_bare` | moonshot / kimi-k3 | 19 | ok |
| `replies/m0b_openai/…` — the same seven directories | openai / gpt-5.6-luna | 55 | ok |
| | | **110** | **110 ok, 0 failed** |

All on 2026-09-04, at git `f56a2c3`. Spend, estimated by M0's method (no token usage is printed by
`--one-shot`): ~13–15 KB in and 50–600 B out per call, 110 calls — roughly **$0.20–0.35** for the
whole of M0b. Cents; the premise held again. The `kimi-k2.5` 404 cost nothing this time because the
override was known.

### The freeze (M0b)

`v6_both` is frozen as M0b's output, superseding M0's `v2_structural` freeze. Its measured bytes are
transcribed, unaltered, into the same two artifacts:

- **`strings_draft.toml`** — now **24** `PromptStrings` keys (`know_note`, `know_discipline`, the 21
  `know_hedge_<band>_<rung>` values, `unknown_person_role`). Changed since M0: `know_hedge_default_hops2`
  (rewritten) and six new keys — `know_hedge_{default,top,low}_hops3` and `_hops4`. Every other value
  is byte-identical to M0's. The rung-selection comment now has seven rungs: cold first; hops 0 →
  `hops0_own` if an `own` line else `hops0`; hops 1 → `hops1`; hops 2 → `hops2`; hops 3 → `hops3`;
  hops ≥ 4 → `hops4`. Verified to round-trip to `prose/v6_both/` byte for byte (all 24 keys, plus the
  paragraph) by a checker that first reproduced M0's own round-trip to `prose/v2_structural/` (18/18).
- **`ignorance_rule.txt`** — the `v6_both` paragraph (22 lines, 1386 B) at the same unconditional
  position, with its measurement and the R2 and R3 outcomes recorded in the header.

`README.md`'s M0 section carries a two-line pointer here. `scripts/m0b/` is deleted. Nothing under
`plan/` was edited.

## M1 re-observation (2026-09-05)

M1's optional live re-observation (`plan/M1.md`, Verification, "Optional, costs cents"). Appended,
never edited — the measured record above stands unchanged.

**What was fired.** One call, `moonshot`, through the shipped code rather than a hand-authored
sheet: the *real* rendered turn prompt of a non-holder asked point-blank, dumped out of

```sh
cathedral-headless --fake --stage -t 1 -v --say 'Who took the corner pitch at the market?'
```

(Conny, `cb947`, at the demo spawn — no `what_you_know` block on her sheet, the unconditional
ignorance paragraph at `turn.j2:194`, the question in `since_your_last_turn`), then

```sh
cathedral-headless --provider moonshot --one-shot /tmp/…/nonholder_sheet.txt
```

**The reply, verbatim:**

```
say {"target": "player", "text": "Corner pitch? Friend, it's Bellday — the trades are shut and there's no market today. If you mean who holds that corner on market days, the market-keeper at the Wickmarket keeps the pitch list; ask there."}
```

**Verdict: holds.** Threshold 1 fires on the shipped bytes at the shipped position — she does not
invent a holder, and she names a next mouth by post *and* place ("the market-keeper at the
Wickmarket … ask there"). No invented person name, day or number. This is the first observation taken
against a prompt the game actually renders rather than a hand-authored M0 sheet, and it agrees with
the 15/15 above.

**The second leg was NOT fired, and why.** The Verification block's other command wants "a holder of
a top-band scandal at hops 4 with nobody on the sheet to send the asker to … seed a `bed` row in a
one-row `--facts` pack held at hops 4". **No M1 lever can produce that sheet.** A `--facts` pack seeds
`seeded`, which is hops 0 by definition; the only way to a hops ≥ 1 holding is
`knowledge::learn`, and *nothing in production calls it in M1* (`plan/M1.md` step 7 — M2's pickup is
its first caller). So `NOTES.md`'s "What stays open" item 4 (3 of 4 moonshot top-band fires
paraphrased a referral place the sheet never gave) is **still open and moves to M2**, which is the
milestone that first makes a hops-4 holding reachable without hand-authoring a sheet.
