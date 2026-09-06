# M0b scoring — provider `openai` / `gpt-5.6-luna`

Scored from the reply files under `replies/m0b_openai/` and the rendered sheets under
`sheets/<variant>_turnj2/`, by M0's rules taken **unchanged** from `NOTES.md` ("The three
thresholds" and the Q4 distinctness rule as stated there). Every quote is byte-verbatim from a reply
file; every grounding claim was checked against the sheet that produced that reply, by `grep` on the
sheet. This is the second pass over this provider: the first pass covered the 34-call matrix in
`M0B_MATRIX.txt`; the firing agent then fired 21 more calls (`v6_both_bands_hops3`, `v8_bare`) that
neither `M0B_MATRIX.txt` nor `M0B_VARIANTS.txt` documents, so this file re-derives the 34 and scores
the 21 as well. **55 openai replies in all**, all non-empty, all `ok` on attempt 1 per the seven
`RUN.json` ledgers (v4_rungs 8, v5_exemplar 8, v6_both 15, v6_both_bands 2, v6_both_bands_hops3 2,
v7_letitlie 1, v8_bare 19). `git_commit` on every ledger is `f56a2c3`, which is HEAD.

Parse validity, re-derived: **55 of 55** accepted, 0 empty action lists. Verbs: `say` 55, `go_to` 2
(both `q5`), `remember` 1 (`v5_exemplar/q4_wick_03`). `raise_word`: **0 in 55.**

**Two record-keeping facts, fixed here rather than left to rot:**

1. `M0B_MATRIX.txt`, `scoring/m0b_moonshot.md` and the first pass of this file all say
   `top_band.hops3` / `low_band.hops3` "never reached a provider". **That is no longer true**: the
   firing agent authored `scenarios/q2_holder_hops3_{top,low}_band.json` (byte-identical to their
   hops4 twins but for the rendered rung — verified, 2 changed lines) and fired them at `v6_both` on
   both providers into `replies/m0b_<provider>/v6_both_bands_hops3/`. **All 21 hedge keys have now
   been fired at openai at least once.**
2. The sheets for `v8_bare` (24) and for the two hops3 scenarios (2 unmoved + 2 moved) existed
   **only in the scratchpad** (`RUN.json: sheets_dir` points at `…/scratchpad/m0b2/…`), which is
   ephemeral. Copied here, unaltered, as `sheets/v8_bare/`, `sheets/v8_bare_turnj2/` and the two
   `q2_holder_hops3_*` sheets in `sheets/v6_both{,_turnj2}/`. Verified before copying: every fired
   sheet is `cmp`-identical to its `_turnj2` copy; every `v8_bare_turnj2` sheet differs from its
   `v6_both_turnj2` twin in exactly the two bullet lines (4 diff lines each, 24 of 24); the shipping
   position holds on the copies (rule found once, immediately before "Use ONLY the verbs listed
   below").

`v8_bare`, since nothing else describes it: `diff -r prose/v6_both prose/v8_bare` reports
`ignorance_rule.txt` only, bullets 1 and 2 reduced to `- name the trade that would know;` and
`- name the post whose business it is;` — R2 done by **dropping** the exemplars instead of
**describing** the move. It is the "or drop" half of R2's own brief, and it was fired on the full
15-sheet diagnostic set plus the four band sheets.

---

## Headline

| | M0 baseline (v2, openai) | v4_rungs (R1) | v5_exemplar (R2) | **v6_both (R1+R2)** | v7_letitlie (R3) | v8_bare (R1 + R2-dropped) |
|---|---|---|---|---|---|---|
| Threshold 1 — non-holder refuses **and** names a next mouth | 5/5 | — | — | **5 / 5 PASS** | — | 5 / 5 PASS |
| Threshold 2 — ≥ 6/8 holders materially distinct | 4/8 FAIL | **3 / 8 FAIL** | **3 / 8 FAIL** | **3 / 8 FAIL** | — | **3 / 8 FAIL** |
| Threshold 3 — `raise_word` uses, no-occasion probe | 0 | — | — | **0 PASS** | — | 0 PASS |
| Q1 — holder volunteers the relevant fact | miss | — | — | **miss** | **miss** | miss |
| Invented name / day / place / number | 0 of 15 | 0 of 8 | 0 of 8 | **0 of 15** | 0 of 1 | 0 of 19 |
| Old exemplar half-line lifted | 3 of 7 | no occasion | 0 of 8 | **0 of 7** | 0 of 1 | n/a (none on sheet) |
| "…is their business" closing template, of 5 q3 | 2 of 5 | — | — | **1 of 5** | — | **4 of 5** |
| Hop-count word ("third-hand") in the 8 q4 | 4 of 8 | 0 | 1 | **0** | — | 0 |
| mean reply bytes, q4 set of 8 | 161.8 | 162.5 | 177.9 | **158.6** | — | 169.2 |
| mean reply bytes, whole directory | 137.7 (15) | 162.5 (8) | 177.9 (8) | **138.3 (15)** | 135 (1) | 157.9 (19) |

**One line: on openai, R2 works (and the *describe* form beats the *drop* form), R1 does not close
threshold 2 in any form, R3 changes nothing, and terseness — M0's named mechanism — is untouched.**
Threshold 2 is one point *worse* than baseline in all four R1 variants, because the one baseline
reply with a clause of its own (06's referral) does not recur; nothing in the prose can make a
one-sentence reply carry a fact, its hedge and an opinion.

---

## Threshold 1 — the five `q3_nonholder_asked_*` sheets: v6_both **5 / 5 PASS**, v8_bare **5 / 5 PASS**

`refuse` = supplied no name, day, place or number it was not given. `refer` = named a next mouth.
Both required. Grounding rule, M0's own precedent: **a named person or named place must appear on
the sheet; a bare trade or post need not** (M0 passed "ask at the ward post", "Ask a watchman",
"the clerk who keeps arrests" on sheets carrying none of those words, because the rule literally
says "name the trade that would know it").

### v6_both

| sheet | refuse | refer | verdict | deciding quote, verbatim | handle checked |
|---|---|---|---|---|---|
| asked_day (Idonea Pell) | yes | yes | **pass** | *"I don't know which day. It came by word from a carter of the Weigh Ward; ask there."* | sheet L207: *"a carter of the Weigh Ward (you don't know their name) was put off the Brede cart at the Wool Gate"* — her own `unknown_person` subject |
| asked_name (Petronel Roper) | yes | yes | **pass** | *"I don't know. Ask the ward watch—their business is taking people."* | bare post. The sheet's only block bullet is the dry well; "watch" does not occur on the sheet (only "watched", L20) |
| asked_number (Rohese Kern) | yes | yes | **pass** | *"I don't know that weight. Ask the cartwright or a weighman who saw the Brede cart."* | **blockless sheet.** Two bare trades; "Brede cart" and "beam" are the player's own words (L196) |
| asked_place (Ede Hobbe) | yes | yes | **pass** | *"I don't know where they took Grigor Ashe. Ask the watch—they'd keep word of that."* | **blockless sheet.** Bare post |
| asked_who_did_it (Gile Thorn) | yes | yes | **pass** | *"I don't know who cut it. Ask a cargo broker in the Weigh Ward; they handle the bales and their loading."* | sheet L207: *"a cargo broker of the Weigh Ward (you don't know their name)"* — his own bullet's subject |

Two of five referrals point at the sheet's own `unknown_person` subject (baseline: one of five).
**Named persons invented: 0. Named places invented: 0.**

One provenance slip, passed because M0 passed the identical slip at baseline (*"Ask a carter of the
Weigh Ward; that tale came to me third-hand"*), flagged because it matters to M2/M3: asked_day's
*"It came by word from a carter"* makes the **subject** of her bullet its **source**. On the sheet
the carter is the man put off the cart, not the man who told her. A mouth that turns "who it happened
to" into "who told me" corrupts provenance while inventing nothing.

### v8_bare

| sheet | verdict | deciding quote, verbatim |
|---|---|---|
| asked_day | **pass** | *"I don't know which day. Ask a carter in the Weigh Ward; one of them would know."* — same grounded handle, and this time **not** miscast as the source |
| asked_name | **pass** (ugly) | *"I don't know. Ask the watch; naming officers taken is their business."* — refuses and refers, but "naming officers taken" is a garble of the question; scored on the two tests only |
| asked_number | **pass** | *"I don't know. Ask the weighmaster; that is their business."* — bare post. Not a lift: "weigher" no longer occurs anywhere in v8's prose; it is the model's coinage off the player's "beam" |
| asked_place | **pass** | *"I don't know where they took Grigor Ashe. Ask the city watch; that is their business."* |
| asked_who_did_it | **pass** | *"I don't know who cut it; I saw nothing of that bale. Ask the cargo broker in the Weigh Ward—such freight is their business."* — grounded handle (L207) |

**Threshold 1 is provider-independent and R2-form-independent: 5/5 at baseline, 5/5 at v6, 5/5 at
v8.** The go/no-go is not in play in M0b. What *is* in play is the shape of the five — see R2 below.

## Threshold 3 — `q5_raise_word_no_occasion`: **0 uses at v6_both, 0 at v8_bare — PASS**

```
v6_both:  say {"target":"ft3tb","text":"Aye. Two more."}   go_to {"place_id":"pl_nve1"}
v8_bare:  say {"target":"ft3tb","text":"Aye. Two more."}   go_to {"place_id":"pl_5vy5"}
```

`ft3tb` is Sef at 4.4 m (L193), who said *"Two more sacks and the cellar is clear"* (L197); `pl_nve1`
Gaunt Passage (L183) and `pl_5vy5` Bonded warehouse (L185) are both in `places_you_know`. He
answered the sack count and went back to work. Census over **all 55** openai replies with M0's regex
(`^\s*`*\s*raise_word\s*\{`): **0 uses anywhere.** Same as M0 on both providers.

## Threshold 2 — eight holders of one fact: **3 / 8 in every variant. All FAIL.**

All eight sheets ask the same question — *"Is it true they took Grigor Ashe at the Wickmarket?"*
(L210) — and differ only in `you`, `back_story` and the one rendered rung (L207). The personas are
rich (a lame baker on a stool, a brewer with a slate by her door, the Tracer, a thief, a
seventeen-year-old lamplighter, a retired weigher who takes bribes) and moonshot got own clauses out
of these same bytes; the sheets are not the constraint.

Distinctness rule, M0's, applied unchanged: **a reply is its own sentence if it carries at least one
clause that is the mouth's own** — an opinion, a question back, a referral, a contradiction, a
warning, a first-hand claim, a trade self-description, an oath off its own sheet. **A reply whose
entire content is the rendered fact plus hedging is not, and all such replies collapse to one.** A
mouth-added disclaimer ("I did not see it myself", "I cannot vouch for it") is hedging, not a clause
of the mouth's own — M0 says so in terms ("v1 can be argued to 6 by keeping Havise's hedge disclaimer
as a sentence of its own, which the rule above does not").

### v6_both — the shipping candidate: **3 / 8**

| holder | rung | verdict | deciding sentence, verbatim |
|---|---|---|---|
| 01 Ansel Quern | hops1 | *shared core* | *"They say Grigor Ashe was taken here two days past and lies in the Stone House. The one who told me was there."* — the rung's two clauses, reordered. **Byte-identical to v4/02 and v8/02** — two personas, three prompt variants, one sentence |
| 02 Sibbe Hobbe | hops1 | *shared core* | *"Aye, so they say. Grigor Ashe was taken here two days past and lies in the Stone House. I did not see the taking myself."* — fact + disclaimer |
| 03 Osanne Vell | hops0_own | **own** | *"It is. I stood in my doorway when they took Grigor Ashe; he said not a word, and the sergeant had his arm twisted behind him."* — first-hand |
| 04 Gile Skell | hops2 (new) | *shared core* | *"Aye, so I was told—but the teller wasn't there either. They said Grigor Ashe was taken here two days past and lies in the Stone House."* — the rung's second clause paraphrased |
| 05 Rohese Crake | hops2 (new) | *shared core* | *"I was told Grigor Ashe was taken at the Wickmarket two days past and lies in the Stone House. The one who told me was not there either."* — the rung near-verbatim |
| 06 Havise Bram | hops3 (new) | *shared core*, **over-firmed** | *"It came to me down a line of mouths, but yes—the word is that Grigor Ashe was taken at the Wickmarket two days past and lies in the Stone House."* — the rung's opening verbatim but for the pronoun, then "but yes" lays a truth-claim over a three-remove telling. M0 scored this exact shape (v3's 06) as shared-and-over-firmed |
| 07 Petronel Clove | hops4 (new) | *shared core* | *"It has come through many mouths that Grigor Ashe was taken at the Wickmarket two days past and lies in the Stone House. I cannot say who began it."* — both clauses are the rung's |
| 08 Ede Kett | hops0 | **own** | *"Aye. I saw Grigor Ashe taken here two days past. He lies in the Stone House now."* — first-hand |

Own: 03, 08. The other six collapse to one. **3 of 8, range [3, 5]** — 5 only by counting 02's and
06's disclaimers, which the rule forbids; nothing reaches 6 without counting rung paraphrase as the
mouth's own words, which is the thing threshold 2 exists to catch.

### v4_rungs (R1 alone), v5_exemplar (R2 alone), v8_bare (R1 + dropped exemplars): **3 / 8 each**

Own clauses in all three: **03** and **08** only.

- v4/03 *"Aye. I saw them take Grigor Ashe from the Wickmarket. He said not a word; the sergeant had his arm twisted behind his back."*; v4/08 == v5/08 *"It is true. I saw Grigor Ashe taken here two days past; he lies in the Stone House."* (byte-identical across the two variants).
- v4/04 *"That is what I heard: … The one who told me was not there either."* and v4/05 *"That is what I was told: … The one who told me was not there either."* — the same sentence twice, second clause byte-identical, Jaccard 0.88.
- v5/04 *"That is the word going round, but I cannot vouch for it: …"* and v5/05 *"That is the word going round, though I couldn't say who began it: …"* — the same opening twice. v5/03 adds a `remember` action (grounded: a memory of what she saw) — a verb of her own, but the *sentence* is the same first-hand claim; already counted own.
- v5/06 is the one hop-count parrot in the openai set: *"That is what I have heard, **though only third-hand**: …"*, off v5's retained *"Third-hand, mouth to mouth"* rung.
- v8/05 *"The one who told me had it from someone who was there."* (the new hops2 rung, paraphrased); v8/06 *"That is what I heard, passed down through several mouths: … I saw none of it myself."*; v8/07 *"Aye, so it is said: … It has passed through many mouths, though I know not who began it."* — all rung.

### Pairwise content-word Jaccard on the spoken text (M0's `cross_score.py` bag and stopword list)

Calibration: the M0 reference sets reproduce M0's published figures exactly.

| set | mean | max | pairs ≥ 0.60, of 28 | which |
|---|---|---|---|---|
| M0 moonshot v2 (reference) | 0.239 | 0.500 | 0 | — |
| **M0 openai v2 — the baseline to beat** | **0.409** | **0.722** | **5** | 05~07 0.72, 01~07 0.71, 06~07 0.68, 04~07 0.63, 07~08 0.60 |
| v4_rungs | 0.476 | **0.933** | **7** | 01~02 0.93, 04~05 0.88, 01~05 0.82, 02~04 0.76, 02~05 0.76, 01~04 0.72, 05~06 0.60 |
| v5_exemplar | **0.401** | 0.750 | 2 | 02~08 0.75, 01~02 0.67 |
| **v6_both** | **0.426** | 0.812 | **5** | 01~05 0.81, 02~08 0.64, 04~05 0.61, 01~08 0.60, 04~08 0.60 |
| v8_bare | 0.446 | 0.933 | 6 | 01~02 0.93, 04~05 0.79, 01~04 0.78, 01~05 0.78, 02~04 0.72, 02~05 0.72 |

Read straight: no R1 variant beats the baseline on any of the three numbers; v4 and v8 are worse on
all three. **Do not read it straight**, and the reason is in the table: `v5_exemplar` has the
*unchanged* five-rung ladder — 04, 05, 06 and 07 all render the same "Third-hand, mouth to mouth"
line — and it posts the *best* numbers of the five sets. A collapsed ladder beating both variants that
uncollapse it is proof that at n = 8, one call per sheet, these deltas are **sampling noise**. The
honest conclusion is not "R1 hurt"; it is **"Jaccard cannot resolve R1's effect on this provider at
this sample size — and no variant shows a signal large enough to escape the noise."**

The 04–07 sub-block (the four holders R1 gave four different rungs to):

| set | 04–07 mean | max | 01~02 (same rung, untouched by R1) | 04~05 (both authored at hops 2) |
|---|---|---|---|---|
| M0 openai v2 | 0.616 | 0.72 | 0.50 | 0.55 |
| v4_rungs | 0.585 | 0.88 | **0.93** | **0.88** |
| v5_exemplar | 0.462 | 0.57 | 0.67 | 0.57 |
| v6_both | 0.493 | 0.61 | 0.59 | 0.61 |
| v8_bare | 0.512 | 0.79 | **0.93** | 0.79 |

Two hard facts survive the noise. **First: 04 and 05 are both authored at hops 2, so R1 cannot
separate them** — 0.61 to 0.88 across every R1 variant is two mouths on one rung, and it is the
residual floor no ladder fixes; only M3's garbling of the `said` text does. **Second: 01~02 = 0.93 in
two of four R1 variants, on the hops1 rung R1 never touched** — and the cross-variant `md5` check
makes the point bluntly: *"They say Grigor Ashe was taken here two days past and lies in the Stone
House. The one who told me was there."* came back **byte-identical from three different sheets**
(v4/02, v6/01, v8/02), and v5/01 == v8/01 likewise. On openai a hops1 holder recites the rung; the
persona contributes nothing to the sentence.

### The one non-noisy R1 result: the hop-count word is gone, by construction

| set | holders saying "third-hand" etc. |
|---|---|
| M0 openai v2 (old hops2 rung present) | **4 / 8** — 04, 05, 06, 07 |
| v5_exemplar (rung present) | 1 / 8 — 06 |
| v4_rungs / v6_both / v8_bare (rung retired) | **0 / 8**, **0 / 8**, **0 / 8** |

4→1 between two fires of the *same* rung shows this count is noisy too, so the claim is only what
construction guarantees: **retiring the rung removed the phrase's source, and the model did not coin
"third-hand" for itself once in 24 R1 q4 replies.** Zero hop counts in words anywhere in 55 replies;
the M0 property "the count never reaches the model" survives R1 on openai.

### The bands — every one of the 21 hedge keys now fired on openai

`v6_both_bands` (hops4) and `v6_both_bands_hops3` (hops3) fire the same two personas (Averil Tarn,
toll clerk; Ewart Tarn, weigher) on sheets identical but for L193, the rung. Read across a band, the
question is whether the ladder still *steps* between hops 3 and 4; read across a hop, whether erosion
still reads one rung in.

| rung | reply (v6_both), verbatim | verdict |
|---|---|---|
| top_band.hops3 *"It is the common word now, and nobody bothers to say where they had it"* | *"I don't know what lies between them. The common word is only that Rohese Sedge visits Doctor Ferrant after the Snuffing, and not for physic. Doctor Ferrant is the one to ask."* | **pass** — refuses the part beyond the fact, states the fact as common word (the rung's own phrase, lifted), and adds a referral of its own. "Doctor Ferrant" is on the sheet (L193/L196) |
| top_band.hops4 *"This is simply what is known in this city now, whoever said it first"* | *"I don't know what lies between them. I only know Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing, and it is not for physic. Doctor Ferrant is the one to ask."* | **pass** — flat, as known; does not read back "whoever said it first". One rung less hedge than hops3: **the top band steps** |
| low_band.hops3 *"Passed to you at a remove you cannot account for, the sort of small news that gets carried because there is nothing else to carry"* | *"I was told Bertran Hobbe's mill sent short weight to the Wickmarket bakehouse, and that the flour was cut with bean meal."* | **partial** — nothing invented, but the rung's hedge thinned to a bare *"I was told"*: a low-band telling at three removes came out less hedged than the same band at four. The rung is all atmosphere and no hedge-word a terse model can say back; v8's reply on the identical rung added *"I cannot vouch for more than that"*, so n = 1 either way |
| low_band.hops4 *"Stall talk, handed along so often that its edges are gone, and you have never met anyone who was there"* | *"It is stall talk only, never proved to me: Bertran Hobbe's mill was said to send short weight to the Wickmarket bakehouse, with the flour cut with bean meal."* | **pass** — hedge kept, the chain paraphrased ("never proved to me") |

**Erosion across bands passes at both hop values**: at hops 3, "common word" against "I was told";
at hops 4, "I only know … " against "stall talk only, never proved to me". The top band sheds one
rung per hop as the spec wants. The low band's hops3 rung is the weak one of the four, and it is now
measured rather than assumed — **M1 can transcribe it, with this partial on the record**. The v8
replies on the same four sheets are consistent (*"Stall talk says … I never met anyone who witnessed
it; ask a baker or miller for firmer word"* — the rung's chain clause lifted, plus a referral to two
bare trades derived from "mill"/"bakehouse" on the sheet). No invented specifics in the eight.

## Q1 / R3 — `q1_holder_relevant_unasked`: frozen clause vs softened, at the shipping position

Bertran Skell the butcher holds *"They say — and the one who told you was there: Grigor Ashe was
taken at the Wickmarket, two days past, and lies in the Stone House"* (L212) and remembers *"Grigor
Ashe has owed me for a side of mutton since the Waning"* (L223). Rohese Ashe says *"Put the side on
the Ashe slate, Bertran. I will send the coin up with the household on Bellday."* (L216). Does he
volunteer the arrest that is the reason to refuse?

- **v6_both (frozen "and otherwise let it lie") — MISS.** *"Not on the slate, Rohese. Coin now for
  the side—your household already has Grigor's old debt hanging there."* Refuses off the debt
  memory, never mentions the Stone House. Everything grounded: "your household" is Rohese's own
  word (L216), the debt is L223, `fr5as` is Rohese at 1.9 m (L196).
- **v7_letitlie (softened: "…is not a thing to sit on: say that one.") — MISS, and no better.**
  *"Not on the slate, Rohese. Grigor's side is still unpaid. Nine sparks in hand, and the mutton is
  yours."* Same refusal, same debt, same silence; "nine sparks" is `you_sell` L192. It even drops
  v6's oblique "your household".
- **v8_bare (frozen clause, different bullets) — MISS**, and near-verbatim v7: *"Not on the Ashe
  slate, Rohese. Grigor's side is still unpaid. Nine sparks in hand, and the mutton is yours."*

**R3 verdict on openai: the softening measured no gain — 0/1 against 0/1, with a third frozen-clause
control also 0/1.** M0's moonshot placement re-fire repaired this miss with no byte changed; on
openai the miss is stable across three wordings at the shipping position, which says the cause here
is the model's terseness (135–142 B: one refusal clause and one reason, no room for a second reason),
not the clause. **Do not change the frozen clause on this provider's evidence.** n = 1 per arm, so
"no improvement detected", not "harmful".

## R2 — the exemplar leak: fixed by v6's *describe* form; v8's *drop* form regrows the ritual

M0's openai baseline lifted the two bracketed exemplars (`"ask a porter, they stand on that gate"`,
`"that is the weigher's, not mine"`) in 3 of 7 referral-capable replies — *"Ask the weigher at the
Stone Gate"*, *"Ask the porter at the beam; that is their business"*, *"That is the toll-house's
business, not mine"*.

**v6_both: 0 lifts of either exemplar in 55 replies.** "porter" 0, "weigher" 0, "stand on that gate"
0, "not mine" 0. The five referrals go five different places (a carter of the Weigh Ward; the ward
watch; a cartwright or weighman; the watch; a cargo broker of the Weigh Ward). Residue: two single
words off the replacement bullets, *"their **business** is taking people"* and *"they'd **keep** word
of that"* — common words in an instruction, different in the two replies, no quotable half-line.

**v8_bare: 0 lifts (there is nothing to lift) — and the "…is their business" closing template in 4
of 5.** *"that is their business"* (asked_number), *"that is their business"* (asked_place),
*"naming officers taken is their business"* (asked_name), *"such freight is their business"*
(asked_who_did_it). The baseline had 2 of 5 (*"that is their business"*, *"…business, not mine"*);
v6 has 1 of 5. With bullet 2 stripped to *"name the post whose business it is"*, its one content
noun is the one the model says back, and it says it back four times. **That is the parroting risk in
the Q3 channel that R2 was written to remove, arriving through the bare bullet instead of the
exemplar.** The describe form (*"…whose business it is, or the officer who keeps such things"*)
spreads the weight over two nouns and a relative clause, and the replies spread with it.

`v4_rungs` and `v5_exemplar` are uninformative on R2: only q4 sheets were fired at them and no q4
reply in any variant has an occasion to refer.

## The fixture fix

`q4_wick_03`'s `own` line now carries the Wickmarket and renders alone as *"First hand, in your own
words: I was standing in my own doorway on the Wickmarket when they took Grigor Ashe — …"* (L207).
All four openai 03 replies name a grounded place or none: v4/v8 *"from the Wickmarket"*, v5/v6 *"in
my doorway"*. For the record, openai's baseline reply on the broken fixture (*"I was at my own door"*)
had not invented a place either; the three "invented place" scores M0 booked against this sheet were
never openai's.

## Invention census — all 55 openai replies

**0 invented names, days, places or numbers. 0 invented person names. 0 off-sheet ids.** Every
capitalised token, every numeral and number-word, every `target` and `place_id` in every reply was
checked against its own sheet (mechanically, then by reading all 55). Every `target` is a person in
that sheet's `you_see`; both `go_to` ids are in `places_you_know`.

Correction to the first pass of this file: v6/q1's *"your household already has Grigor's old debt"*
was recorded there as a "social inference"; it is not — Rohese herself says *"send the coin up with
the household"* (L216). Grounded.

Soft notes, none a failure: the asked_day source/subject slip (v6); *"naming officers taken"* (v8,
garbled but empty); *"but yes"* over a three-remove telling (v6/06).

---

## Recommendation from this provider

**Ship `v6_both`. Do not ship `v7_letitlie`. Do not swap in `v8_bare`. Stop treating threshold 2 as
a prose gate.**

- **v6_both** is the best-measured wording on openai on everything prose can reach: threshold 1
  **5/5**, threshold 3 **0**, **0 invented specifics in 19 replies** (15 + 4 band), 0 exemplar lifts
  against the baseline's 3, the "is their business" template at 1/5 against v8's 4/5, 0 hop-count
  words against the baseline's 4/8, erosion legible across both bands at both hop values, and
  **all 21 hedge keys now fired at this provider** (low_band.hops3 with one partial on record). It
  costs nothing against the frozen v2 on any of those axes.
- **v7_letitlie**: R3 measured no gain (0/1 → 0/1, and v8's frozen-clause control also 0/1). Its
  appended sentence would put 163 unjustified bytes into `strings.toml`. Keep M0's frozen clause.
- **v8_bare**: identical to v6 on every threshold, worse on the one axis that separates them — the
  bare bullet regrows a closing ritual. The "or drop" half of R2 is now measured and rejected.
- **What v6 costs**: threshold 2 stays **3/8** on openai, one point *below* M0's 4/8 baseline, and
  failing under every defensible clustering [3, 5]. Terseness is unmoved (158.6 B vs 161.8 B on the
  same eight sheets; 138.3 B vs 137.7 B over the fifteen). The two measurements that say why are
  01~02 = 0.93 with a byte-identical sentence from three sheets (a rung R1 never touched) and 04~05 =
  0.61–0.88 in every R1 variant (two holders at one hop count, which no ladder separates). What
  closes it is what the spec already schedules and M0 already promoted to load-bearing: **M3's
  garbling of the `said` text**, and any sim-side seam that gives a mouth something of its own to add.
- **What v6 costs M1**: `strings_draft.toml` grows from 18 keys to 24 — six new hedge keys plus a
  rewritten `default.hops2` — and every one of the seven is now measured on this provider. Carry the
  low_band.hops3 partial into the M1 notes rather than letting the key through unlabelled.
