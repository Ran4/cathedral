# M0b scoring — provider **moonshot / kimi-k3**

Scored from the reply files only, by M0's own rules (`NOTES.md` "The three thresholds" + the Q4
distinctness rule), unchanged. This is a re-derived and extended record: an earlier pass of the
same session scored the 34-reply matrix; every number and every handle check in it was recomputed
from the files before being kept, and the record now also covers the 21 replies that landed after
it (`v6_both_bands_hops3/` 2, `v8_bare/` 19), which the brief did not name and which are scored
separately, below the line.

**55 moonshot replies read in full:** `v4_rungs` 8, `v5_exemplar` 8, `v6_both` 15,
`v7_letitlie` 1, `v6_both_bands` 2 (the specified matrix + the band sanity fire = 34), plus
`v6_both_bands_hops3` 2 and `v8_bare` 19. Every quote is verbatim from
`m0_evidence/replies/m0b_moonshot/`; every referral handle and every specific was grepped in the
sheet that carried it — `m0_evidence/sheets/<variant>_turnj2/` for the matrix; for the hops3 and
v8 fires the sheets exist **only** in the firing agent's scratchpad (`…/scratchpad/m0b2/`), see
"Evidence gaps".

**Jaccard calibration.** M0's own code (content-word bag, M0's stopword list, spoken `say` text
only), re-run on `replies/round1/v2_structural`: mean **0.24**, max **0.50**, **0** pairs ≥ 0.60 —
M0's published moonshot figures exactly. On `replies/cross_openai` it gives 0.40 / 0.72 / **4**
against M0's 0.41 / 0.72 / **5**: the fifth pair M0 listed (07~08 "0.60") computes to **0.562**
here, so M0's script differs from this one by one stop-word or a rounding step at the boundary.
Comparable within rounding; the moonshot side, which is what this sheet compares against, is exact.

---

## Headline

| | moonshot / kimi-k3 |
|---|---|
| Threshold 1 (`v6_both`, the five `q3_nonholder_asked_*`) | **5 / 5 PASS** — refuses and names a next mouth every time, 0 invented specifics |
| Threshold 2 (eight `q4_wick` holders) | **`v6_both` 6 / 8 PASS** (range 5–7) · `v4_rungs` **4 / 8 FAIL** · `v5_exemplar` **4 / 8 FAIL** · (`v8_bare`, beyond the matrix: **4 / 8 FAIL**) |
| Threshold 3 (`raise_word`, no occasion) | **0 uses** — PASS; 0 uses in all 55 replies |
| Q1 (holder volunteers the relevant fact) | **`v6_both` FAIL · `v7_letitlie` FAIL** (and it invents a number) · `v8_bare` pass — R3 does not win |
| Invented name / day / place / number | **1 hard** (`v7_letitlie`) + soft referral-place paraphrases in the band fires (see Inventions) |
| Exemplar leak | **1 of 34** in the matrix — a lift of R2's **new** clause, not the old exemplar; 0 of the 21 later replies |

**One line: ship `v6_both`; do not ship `v7_letitlie` (R3); do not ship `v8_bare`; and do not
record R1 or R2 as proven by this provider.** Moonshot passed threshold 2 before M0b (7/8) and
passes it after (6/8) — the repairs buy nothing measurable here and the lexical metric moves the
wrong way. This was the provider that was already passing; openai's sheet decides R1.

---

## Threshold 1 — non-holder asked: refuse AND name a next mouth (`v6_both`, 5 sheets)

`refuse` = supplied no name, day, place or number it was not given. `refer` = named a next mouth
(a trade, a post, a place to ask at, or a person on the sheet). Both required.

| scenario | refuse | refer | verdict | deciding quote | handle on the sheet? |
|---|---|---|---|---|---|
| asked_day | yes | yes | **pass** | *"I couldn't tell you the day, friend. I only heard it second-hand myself, and no day was given with it. If it's the cart's business you're after, ask a carter at the Wool Gate — they keep their own tally of who comes and goes."* | yes — her one bullet renders *"a carter of the Weigh Ward (you don't know their name) was put off the Brede cart at the **Wool Gate**"*; both handles are the bullet's own. (*"second-hand"* on a hops-2 bullet: one remove short, flag F1) |
| asked_name | yes | yes | **pass** | *"I don't know that name, nor any officer. If it's the watch you want, ask at the Tallage — they keep their own."* | yes — `The Tallage` in `places_you_know` |
| asked_number | yes | yes | **pass** | *"Pounds over? I don't know what you're asking, friend. I'm a carpenter, not a carter—if it's about a cart or a load, you'd want the teamsters or the porters at the Stone Gate. If it's about a beam, I've got my hands full with this ridge piece here."* | yes — `The Stone Gate` in `places_you_know`, *Carpenter* in `you`, the ridge piece is her `current_goal`. Teamsters/porters are trades, not persons. **No `what_you_know` block on this sheet** |
| asked_place | yes | yes | **pass** | *"Grigor Ashe? Never heard the name, love. I wash linen, I don't take folk anywhere. If it's the watch took him, you'd want the ward post by the Tally Bridge — ask there."* | `The Tally Bridge` yes; *Laundress* yes; **"ward post" appears nowhere on the sheet** — an institution given a location (flag F4). Passed, because M0 scored *"ask at the ward post"* clean. **Also blockless** |
| asked_who_did_it | yes | yes | **pass** | *"I don't know who cut the cord. I was at my bench with the hides. If you want answers about bales and beams, ask the cargo brokers at the Weigh Ward—they handle that sort of thing all day."* | yes — his bullet renders *"a **cargo broker of the Weigh Ward** (you don't know their name)"*; bench and hides are `you_are`. The tail is a lift of R2's new clause (Leak, below) |

**5 / 5 — PASS.** Same as M0's moonshot and openai results. Two of the five carry no block at all
and both still refused and referred. Nothing in R1+R2 threatens the go/no-go on this provider.

---

## Threshold 2 — eight holders of one fact

All eight sheets in a variant are byte-identical apart from `you`, `back_story` and the one
rendered knowledge line (verified by the firing agent and spot-checked 01/07 here);
`stored_memories — none` on every one, so a first-hand claim outside 03 (own line) and 08
(hops 0) is a fabrication, not a recall. Question asked, all eight: *"Is it true they took
Grigor Ashe at the Wickmarket?"*

**Rule, unchanged from M0:** a reply is its own sentence if it carries at least one clause that is
*the mouth's own* — an opinion, a question back, a referral, a contradiction of the premise, a
warning, a first-hand claim, a trade self-description, or an oath off its own sheet. A reply that
is only the rendered fact plus its hedge is not, and **all such replies in a set count as one**.
Provenance restated in the mouth's own pronouns (*"I had it from one who…"*, *"more than that I
was not given"*) is the hedge, not the mouth.

### `v4_rungs` (R1 alone) — **4 / 8, FAIL**

| holder | rung | own? | deciding quote |
|---|---|---|---|
| 01 Ansel | hops1 | **own** | *"good riddance or bad luck to him, depending on what he did to earn it"* — an opinion. Defective: *"Saw it with my own eyes, or near enough"* on a hops-1 rung with no memories |
| 02 Sibbe | hops1 | shared | *"Aye, it's true. They took Grigor Ashe right here, two days past. He's in the Stone House now."* — fact, no hedge, nothing else |
| 03 Osanne | hops0_own | **own** | *"I saw it from my own doorway. They took him, and he said not one word."* |
| 04 Gile | hops2 (new) | shared | *"though the one who told me wasn't there themselves, they had it from the one who was"* — the new rung with the pronoun swapped |
| 05 Rohese | hops2 (new) | shared | *"I wasn't here that day, but the talk is the watch took him"* — provenance inverted, and *"the watch"* is an agent the said line does not give (flag F2; "watch" is nowhere on her sheet) |
| 06 Havise | hops3 (new) | shared | *"It came to me down a line of mouths."* — the rung verbatim but for the pronoun |
| 07 Petronel | hops4 (new) | shared | *"More than that I was not given."* — the store's edge; provenance |
| 08 Ede | hops0 | **own** | *"Saw it myself two days past"* — grounded in *"You saw this yourself"* |

own = 01, 03, 08 + cluster {02, 04, 05, 06, 07} → **4**. Jaccard mean **0.25**, max **0.50**, **0**
pairs ≥ 0.60. Mean reply **203 B**.

### `v5_exemplar` (R2 alone, old 5-rung ladder) — **4 / 8, FAIL**

| holder | rung | own? | deciding quote |
|---|---|---|---|
| 01 Ansel | hops1 | **own** | *"you'd do better asking at the Stone House itself — I've bread to mind and no part in it"* — referral + trade self-description (Baker, sheet). Provenance right: *"I had it from one who was standing there"* |
| 02 Sibbe | hops1 | shared | *"Aye, it's true. Taken right here at the Wickmarket, two days past. They say he's in the Stone House now."* |
| 03 Osanne | hops0_own | **own** | *"I was standing in my own doorway when they took him — he did not say one word, and the sergeant had his arm up behind his back."* |
| 04 Gile | hops2 (old) | shared | *"Mouth to mouth it goes … Whose word it was to begin with I couldn't tell you."* — the retired rung, halved and read back |
| 05 Rohese | hops2 (old) | shared | *"I heard it myself, though it was third-hand."* |
| 06 Havise | hops2 (old) | shared | *"Third-hand it comes to me, mind — I couldn't say whose word it was to begin with. But it's what the ward is saying."* |
| 07 Petronel | hops2 (old) | shared | *"Aye, that's what they're saying. … They say he lies in the Stone House now."* |
| 08 Ede | hops0 | **own** | *"Aye. Saw it myself, two days past."* |

own = 01, 03, 08 + cluster {02, 04, 05, 06, 07} → **4**. Jaccard **0.26 / 0.50 / 0**. Mean **221 B**.
Four mouths reciting one rung — M0's finding 2, reproduced at the shipping position.

### `v6_both` (R1 + R2, the shipping candidate) — **6 / 8, PASS (range 5–7)**

| holder | rung | own? | deciding quote |
|---|---|---|---|
| 01 Ansel | hops1 | **own (defective)** | *"Saw it with my own eyes, I did."* — a first-hand claim, which the rule counts (M0 counted v3's confabulated 01 the same way) but which is **false**: his rung is *"the one who told you was there"*, `stored_memories — none`. The only reason he is a separate sentence is a register failure |
| 02 Sibbe | hops1 | **own** | *"True as the slate on my door. … so I heard from one who was standing there. Now, are you buying ale, or only gathering talk?"* — an oath off her `back_story` (*"the slate hangs by the door where every debtor can read his own name"*), a question back, provenance kept right. Best reply of the 55 |
| 03 Osanne | hops0_own | **own** | *"I was standing in my own doorway when they took him … A hard sight on a market morning."* (her `current_goal`: *"Get through the morning's trade"*) |
| 04 Gile | hops2 (new) | shared | *"I had it from one who had it from the one who was. But it's all over the market."* — the rung paraphrased plus corroboration the sheet does not give (flag F3) |
| 05 Rohese | hops2 (new) | **own** | *"If it's the taking itself you want, the watch at the Stone House would know more than I do."* — a referral, unprompted (*"second-hand"* for two removes: F1 again) |
| 06 Havise | hops3 (new) | shared | *"Aye, that's what they say. Grigor Ashe was taken right here at the Wickmarket, two days past. They say he's in the Stone House now."* — the longest, most distinctive new rung in the set produced the barest reply in it |
| 07 Petronel | hops4 (new) | shared | *"Aye, it's true. They took Grigor Ashe right here at the Wickmarket, two days past. He's in the Stone House now."* — a **four-remove** telling as flat fact, no hedge. Worst register failure in the 55; Jaccard **0.71** with 01, a hops-1 reply |
| 08 Ede | hops0 | **own** | *"Aye. Saw it myself, two days past."* |

own = 01, 02, 03, 05, 08 + cluster {04, 06, 07} → **6**.

**Sensitivity, because the threshold is exactly met:** **5** if 01's false eyewitness claim is
disqualified as a corruption of the rendered line rather than a clause of his own (a stricter
marker's reading, and it fails the threshold); **7** if 04's *"it's all over the market"* is
allowed as an opinion. So **`v6_both` ∈ [5, 7], point 6**. `v4` and `v5` are ∈ [3, 5], point 4,
and fail on every reading.

### Jaccard, comparable to M0's table

| set | mean | max | pairs ≥ 0.60, of 28 | mean reply B |
|---|---|---|---|---|
| M0 round1 `v2_structural`, moonshot, low position (calibration) | 0.24 | 0.50 | 0 | 223 |
| M0 `cross_openai` `v2_structural` (M0 published 0.41 / 0.72 / 5) | 0.40 | 0.72 | 4 | 162 |
| **m0b `v4_rungs`** | **0.25** | **0.50** | **0** | 203 |
| **m0b `v5_exemplar`** | **0.26** | **0.50** | **0** | 221 |
| **m0b `v6_both`** | **0.29** | **0.71** | **1** (01~07 0.71; next 06~07 0.59) | 216 |
| m0b `v8_bare` (beyond the matrix) | 0.36 | 0.80 | 6 (02~05 0.80, 01~07 0.76, 01~05 0.70, 05~07 0.68, 01~02 0.62, 02~07 0.60) | 216 |

Read honestly: on moonshot the lexical metric got **worse** under the shipping candidate than
under the frozen prose — 0.24 → 0.29, max 0.50 → 0.71, and the first ≥ 0.60 pair moonshot has
ever produced. The pair is 01 (hops 1) with 07 (hops 4): two mouths handed byte-*different* lines
at opposite ends of R1's new ladder, answering in almost the same twelve words (*ashe, days,
grigor, house, past, right, stone, took, true, two, wickmarket*). That is R1's premise failing on
its own terms in the one place the metric can see it.

### What R1 bought on moonshot, isolated

Same position, same eight holder prompts: **`v5` (old ladder) 4/8 → `v6` (new ladder) 6/8** says
R1 is worth +2 and the difference between fail and pass. But **`v4` (new ladder) 4/8 → `v6` (new
ladder) 6/8** differs by the same ±2 with the *same* ladder — R2 changes two bullets no holder
sheet needs — and `v8_bare` (new ladder, bare bullets) is 4/8 again. So **±2 is this metric's
run-to-run noise at n = 8**, and the 6/8 pass sits inside it. Two conclusions:

1. **Moonshot cannot decide R1.** 7/8 before the repair, 6/8 after; measured effect nil to
   slightly negative. openai, which failed at 4/8, is the sheet R1 stands or falls on.
2. **The new rungs are parroted at least as readily as the retired one.** Of the 7 matrix replies
   that received a *new* rung, **4 lifted a distinctive phrase from it**: `v4_04` (*"the one who
   told me wasn't there themselves, they had it from the one who was"*), `v4_06` (*"It came to me
   down a line of mouths"*), `v6_04` (*"I had it from one who had it from the one who was"*), and
   `v6_both_bands` low (*"so the **stall talk** runs … I've **never met a soul who was there**"*
   against *"**Stall talk**, handed along so often … you have **never met anyone who was
   there**"*). The old rung was lifted by 3 of the 4 mouths that got it (v5 04/05/06). R1 changes
   **which** words are parroted, not **whether**; what it does buy is that different rungs get
   parroted *differently*, which is the whole of the variety gain, and the erosion legibility
   (below).

---

## Threshold 3 — `raise_word` with no occasion

`v6_both/q5_raise_word_no_occasion`: `say {"target": "ft3tb", "text": "Aye. This one, then one
more."}` + `go_to {"place_id": "pl_nve1"}`. **0 uses.** (`v8_bare`: *"Aye. Two more."*, 0 uses.)

**0 uses of `raise_word` in all 55 moonshot replies.** Verb tally over the 55: `say` 58,
`remember` 5, `forget` 1, `go_to` 1 — all in the fence. PASS, unchanged from M0.

---

## Q1 — R3, the let-it-lie re-check. **v6 fails, v7 fails; R3 loses.**

Sheet: Bertran Skell the butcher, holding the arrest at hops 1 (*"They say — and the one who
told you was there: Grigor Ashe was taken at the Wickmarket, two days past, and lies in the Stone
House"*) plus `stored_memories: "Grigor Ashe has owed me for a side of mutton since the Waning"`.
Rohese Ashe (`fr5as`, 1.9 m) has just asked for a side on the Ashe slate, coin on Bellday. Pass =
he says the thing he holds, because it is the reason the slate should close.

**`v6_both` (FROZEN clause, shipping position) — FAIL.**

> *"The slate, aye. Though the slate's older than the side, Rohese — Grigor's name has been on it
> since the Waning, and no coin up yet."* + `remember` *"Rohese Ashe took a side of mutton on the
> Ashe slate today, coin promised on Bellday"*

He **grants** the credit and never mentions the arrest. M0 round 1's failure shape, at the
position where M0's placement run had it come out repaired. (*"It was walking yesterday"* is his
own trade — allowed by *"None of this binds your own trade"* — not an invention.)

**`v7_letitlie` (SOFTENED clause) — FAIL, and worse.**

> *"Rohese. The Ashe slate is two years cold, as you know — Grigor's side from the Waning was
> never paid for. I put nothing more on it. Coin in hand, or no mutton."*

Refuses off the debt memory alone, never says the arrest — M0's original miss **exactly** — and
the appended sentence bought an invention: *"two years cold"* is a number the sheet never gives
for this debt. "Two years" on his sheet belongs to a **different** debt, *"patient with any man's
debt except Petronel Crake's, which is two years old"*; the Ashe debt is dated only *"since the
Waning"*. The one reply R3 exists to improve is the one hard invented number in the 55.

**`v8_bare` (FROZEN clause, beyond the matrix) — pass:** *"On the Ashe slate, Rohese? Grigor has
owed me for a side since the Waning, and now they say he lies in the Stone House. Who pays his
slate while he is there?"* — the held fact volunteered as the reason, hedged at its rung.

So the frozen clause at the shipping position now has **three n = 1 fires: pass (M0 placement
run), fail (`v6_both`), pass (`v8_bare`)** — on prompts that differ only in ladder rungs the Q1
sheet barely uses and two referral bullets it never uses. The outcome flips on near-identical
bytes, so **the wording is not the cause; Q1 is noise at n = 1 either way**, and M0's finding 1
(the adjacent/relevant-fact miss belongs to M1's relevance selection, not to prose) stands. The
softened clause failed its one fire and invented a number doing it. **Verdict on R3: do not apply.**

---

## Inventions — every invented name, day, place or number, all 55

**Hard (1):** `v7_letitlie/q1` — *"The Ashe slate is **two years** cold"*, a duration transposed off
an adjacent debt in his own `back_story`. Plausible completion, firing on the actor's *own* memory.

**Soft — referral targets paraphrased into places the sheet never gives (0 in the matrix's 32
threshold replies; all in the band fires):**

- `v6_both_bands/q2_holder_hops4_top_band` — *"ask at the Snuffing itself or find one of the
  gossips who watch **the Doctor's lane**"*. Sheet: *"Doctor Ferrant's door"*, no lane, no
  watchers; and *the Snuffing* is a time of day, not a mouth — a malformed referral.
- `v8_bare/q2_holder_hops3_top_band` — *"ask at **his surgery**; if it's hers, **the Sedge
  household**"*. No surgery on the sheet; "the Sedge household" is derived from her name.
- `v8_bare/q2_holder_hops4_top_band` — *"ask the gossips **at the Snuffing**"*, the time as a place
  again.

All three are the same mechanism: a top-band scandal with no obvious next mouth on the sheet,
and the referral obligation (*"do one of these, not none"*) filled from what would make sense.
Counted soft because each is a where-to-ask, not a fact asserted about somebody; but it is the
only place in 55 replies where the rule's own referral clause produces the completion it forbids.

**Invented person names anywhere in the 55: 0.** Every referral handle was found on its sheet
(`v8_bare`'s too: *"the grey clerks"* in `back_story`, *"the masons at the lodge"* = `The masons'
lodge`, *"the Tallage"*, *"the brokers"* of the Weigh Ward, *"the Stone House"*).

**Four flags, not inventions, all register or accuracy costs of R1:**

- **F1 — the new hops2 rung makes the model count wrong.** The retired rung said *"Third-hand"*
  and got it echoed back correctly. The new rungs carry no ordinal, and where the model
  volunteered one it was a link short every time: *"I only heard it **second-hand**"*
  (`v6/asked_day`, hops 2), *"it came to me **second-hand**"* (`v6/q4_05`, hops 2), *"it's come
  to me **second-hand**"* (`v6_both_bands_hops3` low, **hops 3**). Fewer removes than it came
  with is *"firmer than it came to you"* — the block note's own prohibition. R1 swapped a
  parroted-but-correct ordinal for an unparroted-but-wrong one, three times.
- **F2 — a supplied agent.** `v4/q4_05`: *"the talk is **the watch** took him"*; the said line
  gives only *"was taken"*, and "watch" is nowhere on her sheet. A class, not a name, so not an
  invention by M0's rule — but a gap in somebody else's story filled inside a retelling.
- **F3 — unsupported corroboration.** `v6/q4_04` and `v8/q4_04`: *"But it's all over the
  market."* — the shape of M0's v3 defect (*"Half the quarter saw it"*), milder. `v5/q4_06`:
  *"But it's what the ward is saying."*
- **F4 — an institution given a location.** `v6/asked_place`: *"the ward post by the Tally
  Bridge"* — the bridge is on the sheet, a ward post is not.

---

## Exemplar leak — R2 is not cleared on this provider

M0: v2's bracketed exemplars came back in **4 of 22** moonshot replies at the low position and
**0 of 6** at the shipping position. `grep -iE "stand on that gate|they stand|weigher|not mine|officer
who keeps|all day|handles that sort|such things"` over all 55 moonshot replies hits **once**:

> `v6_both/q3_nonholder_asked_who_did_it`: *"ask the cargo brokers at the Weigh Ward—**they handle
> that sort of thing all day**."*

against R2's new bullet *"name the trade that would know it, **whoever handles that sort of thing
all day**"*. Nothing else in the 55 lifts the old exemplars or the new clauses; the porter line's
byte-identical lift M0 recorded on `asked_number` is gone (*"the porters at the Stone Gate"*, no
"they stand").

1. R2 did remove the specific half-line M0 named.
2. **Like-for-like at the shipping position it is old 0 of 6 vs new 1 of 6**, and the author's
   rationale — *"nobody says 'whoever handles that sort of thing all day' to a stranger in a
   market … there is nothing left in the bullet that can be lifted verbatim into speech"* — is
   **falsified** by that one reply: a tanner said it to a stranger, and it reads worse than the
   porter line did because it is a narrator's phrase in a tradesman's mouth. R2 moved the leak
   from an in-register exemplar to an out-of-register instruction. n is tiny both sides; on
   moonshot R2 is not shown to help and cannot be claimed proven.
3. `v8_bare` (bare bullets, nothing to lift) has **0 lifts in 19** — and pays for it on
   threshold 1, below.

---

## The band fires (not thresholds)

**`v6_both_bands`, hops 4 (matrix sanity fire):**
- top — *"That's the talk of the whole ward, isn't it? They say Rohese Sedge goes in at Doctor
  Ferrant's door after the Snuffing, and not for physic. But I keep my head in the toll rolls, not
  in other folks' doorways…"* Own clause grounded (*"The toll rolls of the Tallage are in your
  hand"*); rung paraphrased; carries the soft invention above.
- low — *"Short weight, so the stall talk runs … I've never met a soul who was there to see it,
  mind, so I pass it on as it came to me. But if it's Hobbe's flour you're weighing, bring it to my
  beam and I'll call it true, grain by grain, in front of the whole yard."* Every specific grounded
  (Hobbe's mill is the said line; the beam and yard are his). Lifts two rung phrases.

**`v6_both_bands_hops3` (beyond the matrix — the two rungs the matrix left unfired).** Sheets
verified single-axis: `diff` against the hops4 twins changes line 193 (the rung) only.
- `top_band.hops3` — *"Only what the whole yard's saying, friend: that Rohese Sedge goes in at
  Doctor Ferrant's door after the Snuffing, and not for physic. What passes between them behind
  that door, I've not seen and couldn't tell you."* Rung (*"the common word now"*) paraphrased,
  hedged once, and the store's edge stated. Clean.
- `low_band.hops3` — *"Short weight, that's what's wrong with it — and cut with bean meal besides,
  so I'm told. Sent to the Wickmarket bakehouse. Mind, it's come to me second-hand and I can't
  vouch for the mouth it came from, but where it came from I can vouch for: not my beam. Nothing
  leaves this yard short, and no man in thirty years has proved me a grain out."* Thirty years is
  his `back_story` verbatim; *"second-hand"* on three removes is F1. Hedged three times.

**Erosion survives at both distances.** Top: one hedge (*"the talk of the whole ward"* / *"what the
whole yard's saying"*); low: three, with the chain named out loud. Same asymmetry M0 measured
across `hops2`, so `02_rumor_pollen.md`'s band column reads correctly at three and four removes
under the new ladder. The step *within* a band from three to four removes is audible in the low
band (*"so I'm told … second-hand"* → *"never met a soul who was there"*) and barely in the top
(*"the whole yard's saying"* → *"the talk of the whole ward"*), which is what a top band that has
already shed its hedges should look like.

**All 21 hedge keys have now reached this provider** — but `top_band.hops3` and `low_band.hops3`
at **n = 1, from sheets that are not in the repo** (Evidence gaps).

---

## `v8_bare` — beyond the matrix: a fifth variant, scored for the record

`prose/v8_bare/` is `v6_both` with the two R2 bullets stripped to *"name the trade that would
know; name the post whose business it is;"* — no exemplar and no description. Not in
`M0B_VARIANTS.txt`; no provenance note anywhere in `m0_evidence/`; sheets only in the firing
scratchpad (verified single-axis against `v6_both`'s: the two bullet lines only). 19 calls.

| threshold | result | deciding quote |
|---|---|---|
| 1 | **4 / 5** — passes the bar, with the dead end back | `asked_day`: *"I don't know the day, stranger. I only heard it second-hand, from someone who wasn't there themselves."* — refuses, **refers nobody**. This is v1_spec's `asked_day` failure, the one M0 said the directional half of the paragraph exists to buy. The other four refer cleanly (grey clerks; masons at the lodge / carters at the Stone Gate; the Tallage; the Weigh Ward brokers) |
| 2 | **4 / 8 FAIL** — Jaccard **0.36 / 0.80 / 6**, the worst matrix moonshot has produced | own: 03 (first hand), 06 (*"the Stone House is where to ask"*), 08. Shared: 01 (*"My source was there and saw it plain"* — provenance, and out of register), 02, 04 (byte-near `v6_04`), 05 (*"I heard it from one who had it from the one who saw it"*), 07. 02~05 at **0.80** |
| 3 | 0 uses | *"Aye. Two more."* |
| Q1 | pass | quoted above |

**Verdict: `v8_bare` is not an improvement.** Dropping the exemplars altogether removes the lift
(0 of 19) and costs the referral on the sheet that most needs it, and its holder set is the
most parroted of the four. It is the control that shows the descriptions in R2's bullets do
work that bare bullets do not; between the leak-once of `v6_both` and the dead-end of `v8_bare`,
`v6_both` is the better trade.

---

## Reply lengths (bytes of the reply file), since terseness is the mechanism

| set | n | mean | min | max |
|---|---|---|---|---|
| M0 round1 `v2_structural`, 8 q4 | 8 | 223 | 126 | 354 |
| `v4_rungs`, 8 q4 | 8 | 203 | 130 | 269 |
| `v5_exemplar`, 8 q4 | 8 | 221 | 134 | 373 |
| `v6_both`, 8 q4 | 8 | 216 | 137 | 327 |
| `v6_both`, all 15 | 15 | 236 | 95 | 594 |
| `v6_both`, 5 q3 | 5 | 226 | 149 | 286 |
| `v7_letitlie` | 1 | 303 | — | — |
| `v6_both_bands` (hops 4) | 2 | 372 | 359 | 386 |
| `v6_both_bands_hops3` | 2 | 315 | 250 | 380 |
| `v8_bare`, all 19 / 8 q4 / 5 q3 | 19 | 231 / 216 / 193 | 50 | 431 |

Moonshot's verbosity is unchanged by the repairs (216 B against M0's 223 B on the same eight
sheets), which is why the failure R1 was written to defeat stays invisible here: this model has
room for a clause of its own whatever the rung says. The firing agent reports openai at 156 B on
the same v6 sheets; the asymmetry persists, so openai's sheet is the one that tells whether R1
works.

---

## Format validity

All 55 replies parse (verbs `say`/`remember`/`forget`/`go_to`, all in the fence). `v6_both/q4_02`
emits `say {"text": …}` with no `target` — legal: `crates/cathedral-sim/src/actions.rs:442`
reads `args_object(args, &["text"], &["target"])`, so `target` is optional and a targetless `say`
is speech aloud, the right answer to a question asked aloud. `v8_bare/q4_04` wraps itself in a
``` fence, which `parse_reply` skips.

## The fixture fix worked

`q4_wick_03`'s repaired `own` line renders *"I was standing in my own doorway on the Wickmarket
when they took Grigor Ashe…"*; no reply in any variant now contradicts the Wickmarket. 03 is an
`own` sentence in every variant and the least similar reply in every matrix.

## Evidence gaps the M0b verdict must name

1. **`v8_bare` and the two hops3 sheets are not in the repo.** `sheets/` holds
   `v4…v7{,_turnj2}` only; the sheets behind `replies/m0b_moonshot/v6_both_bands_hops3/` and
   `v8_bare/` exist only under the firing scratchpad (`RUN.json: sheets_dir`). They verify as
   single-axis today; they will not verify after the scratchpad is gone. Copy them in or strike
   the replies from the record.
2. **`v8_bare` has no provenance line** in `prose/M0B_VARIANTS.txt` or `replies/M0B_MATRIX.txt`.
3. **`top_band.hops3` / `low_band.hops3` are measured at n = 1 each**, clean, on this provider.
4. The Q1 result is three n = 1 fires that disagree; nothing about the let-it-lie clause can be
   concluded from moonshot except that softening it did not help and cost an invention.

---

## Recommendation from this provider's sheet

**Ship `v6_both`.** It clears all three thresholds on moonshot — 5/5, 6/8, 0 — with zero hard
inventions in its own 15 replies.

**Drop R3 (`v7_letitlie`).** Failed the one scenario it exists to fix, in M0's original failure
shape, and invented a number doing it.

**Do not ship `v8_bare`.** Bare bullets bring back the dead-end refusal and the worst holder
parroting moonshot has shown.

**Keep R1 and R2 in `v6_both`, but do not record them as proven.** What this sheet costs the M0b
verdict, plainly:

- Threshold 2 went **7/8 → 6/8**, Jaccard **0.24 / 0.50 / 0 → 0.29 / 0.71 / 1**. The pass is
  real but *at* the line, and ±2 is the noise floor (v4 vs v6 vs v8: same ladder, 4 / 6 / 4).
- The new rungs are lifted by 4 of the 7 mouths that received one; and they cost three
  wrong ordinals (F1) where the retired rung cost none.
- R2's replacement clause was itself lifted once at a position where the old exemplar was lifted
  zero times; `v8_bare` shows the description does earn its referral, so keep it and expect the
  occasional lift.
- Q1 still fails at the shipping position with the frozen clause (1 of 3); the adjacent-fact miss
  is **unfixed by any prose** and belongs to M1's relevance selection, as M0 said.

If openai does not show R1 buying the 4/8 → ≥ 6/8 it was written for, keep R1 for the erosion
legibility it demonstrably has and do **not** bill it as the anti-parroting fix; M3's garbling
then remains the only measure left against risk 2.

---

## Correction (2026-09-04, verdict step)

Evidence gaps 1 and 2 above are closed: the `v8_bare` sheets and the two hops-3 sheets were copied
from the firing scratchpad into `sheets/v8_bare/`, `sheets/v8_bare_turnj2/` and
`sheets/v6_both{,_turnj2}/q2_holder_hops3_*.txt`, unaltered, and they verify single-axis in the repo
(2 changed lines per hops-3/hops-4 pair; 2 changed bullet lines per `v8_bare`/`v6_both` pair on all
24 sheets). `v8_bare` now has a provenance section in `prose/M0B_VARIANTS.txt` and
`replies/M0B_MATRIX.txt`. The "sheets not in the repo" caveats in this file are therefore historical.
`scripts/m0b/` was deleted by the verdict step, as the milestone required. The verdict is in
`NOTES.md`, "M0b — measured repairs (2026-09-04)".
