# M0 round 1 — scoring, slice 3 (final third)

Scored 2026-09-03. Provider for all three variants: **moonshot / kimi-k3** (`RUN.json`,
`git_commit 6910fdf`, 22/22 ok, 0 failed, in every variant). No call in this slice failed, so
nothing here is a missing-reply verdict.

**Slice** — the last 7 of the 22 scenarios in sorted order, × 3 variants = 21 verdicts:

| # | scenario | question |
|---|---|---|
| 16 | `q4_wick_04_gile_skell` (hops 2) | Q4 |
| 17 | `q4_wick_05_rohese_crake` (hops 2) | Q4 |
| 18 | `q4_wick_06_havise_bram` (hops 3) | Q4 |
| 19 | `q4_wick_07_petronel_clove` (hops 4) | Q4 |
| 20 | `q4_wick_08_ede_kett` (hops 0, no own line) | Q4 (+ the hops-0-no-own-line register) |
| 21 | `q5_raise_word_no_occasion` | Q5 misfire control |
| 22 | `q5_raise_word_with_occasion` | Q5 |

No Q1/Q2/Q3 scenario falls in this slice, so the "refuses to invent / names a next mouth" metric is
**n/a here** except as a side observation on `q5_raise_word_with_occasion`, where the porter is a
non-holder being asserted at.

For Q4 I read all eight `q4_wick_*` replies per variant (I need the full set to judge distinctness)
but only return verdicts for holders 04–08; holders 01–03 are slice 2's.

---

## A finding that governs every Q4 verdict below: the hedge table collapses hops 2/3/4

`prose/*/hedges.toml` has rungs for hops 0-own, hops 0, hops 1, hops 2+, cold. Holders 04 (hops 2),
05 (hops 2), 06 (hops 3) and 07 (hops 4) therefore receive the **byte-identical knowledge line** in
every variant — v1 `you had it third-hand: …`, v2 `Third-hand, mouth to mouth, and you could not say
whose word it was to begin with: …`, v3 `you had it third-hand — …`. Half the ward is handed one
string. The `own`/`said` split the README leans on for variety (risk 2) does no work at all across
this half of the sample: the only thing separating those four mouths is the persona prose, and on
this evidence persona prose is not enough. Any Q4 result should be read as **"four holders on one
string"**, not "eight holders on eight strings".

The corollary is that hops 0 (Ede, and Osanne in slice 2) reads distinct in all three variants —
because it is the only rung that actually differs.

## Q4 distinctness (all eight, per variant)

Grouping rule used: two replies are the *same* sentence when one is the other with the hedge moved,
a synonym swapped, or a clause re-punctuated; they are *materially different* when a mouth adds
something a player could tell it apart by — an attitude, a refusal, a counter-question, a referral,
or a different evidential stance.

**v1_spec — 5 distinct of 8.**
`{Ansel: "good riddance to bad rubbish, say I"}`, `{Osanne: hushes, denies the place, counter-question}`,
`{Gile: "Careful what you ask about it, friend"}`, `{Ede: first-hand}`,
`{Sibbe / Rohese / Havise / Petronel: one flat template — "Aye, it's true(-enough). They took Grigor
Ashe right here at the Wickmarket, two days past. He lies/He's in the Stone House now." — with a
hedge tacked on the front or the back}`.

**v2_structural — 6 distinct of 8.** The best of the three, and the only variant where a holder
volunteers a next mouth unprompted (Rohese: *"ask at the Stone House gate"*). But it introduces a
**new parroting axis of its own**: the long hedge string is echoed near-verbatim by four of the
eight — Gile *"whose word it was first, I couldn't tell you"*, Rohese *"I couldn't tell you whose
tongue it started on"*, Havise *"I couldn't tell you whose word started it"*, Petronel *"I couldn't
tell you whose word it began with"*. A longer hedge is a longer thing to parrot. Groups:
`{Ansel}`, `{Sibbe}`, `{Osanne}`, `{Rohese}`, `{Ede}`, `{Gile / Havise / Petronel}`.

**v3_register — 5 distinct of 8, and the worst of the three in my half.** Holders 04–07 are one
sentence four times: *"So they say … third-hand … I didn't see it myself"*, with the clause order
shuffled. Groups: `{Ansel}`, `{Sibbe}`, `{Osanne}`, `{Ede}`, `{Gile / Rohese / Havise / Petronel}`.

## Q5

- **No occasion: all three variants stayed quiet. 3/3 pass.** The verb was in the fence, fully
  populated, and none of the three reached for it. The occasion gate survives contact with the model
  on this evidence — the misfire risk did not materialise.
- **With occasion: 1/3 fired** (v1_spec). v2 and v3 under-fired, which the spec calls acceptable.
- **Methodological caveat on the one pass.** The fence line in every variant is
  `raise_word {"topic": "law", "said": "the toll notary takes a cut off every salt cart"}` — i.e.
  the example *is* the answer for this scenario, topic tag and claim text both. v1's emission is
  byte-identical to the example. So the run demonstrates **that the verb can fire on an occasion**;
  it demonstrates **nothing about whether a model picks a sane topic from the closed nine or composes
  its own claim**, because it was never asked to do either. Round 2 should carry an example whose
  topic and `said` are unrelated to the scenario (e.g. a `bread` example on a `law` occasion),
  otherwise the topic-tag check the README's safety argument rests on is untested.
- The control is uncontaminated by the same fact — the identical loaded fence produced no raise in
  the no-occasion sheets, which is the stronger half of the Q5 result.

## Invention audit (my slice)

**Zero hard inventions across all 21 replies** — no name, day, place or number that the sheet did
not give. Two soft notes:

- v3 `q4_wick_06_havise_bram`: *"Aye, it's true. I heard it third-hand"* — over-firms a third-hand
  item into a truth-claim in the same breath as hedging it. Not an invented particular, but it is
  exactly the "do not make it firmer than it came to you" instruction being half-obeyed, and v3's
  block is the one that promises *"sharpen nothing"*.
- v2 `q4_wick_06_havise_bram`: *"it's all anyone's whispering"* — an embellishment about *spread*
  rather than about the fact. Harmless, arguably good colour; noted because it is the shape a
  spread-claim invention would take.

Out of slice but worth the round-2 note: **v3 `q4_wick_01_ansel_quern` invents** — *"Half the
quarter saw it — I was at my oven, but the whole market was talking of nothing else by midday"* —
from a hops-1 `they say` line. A witness count and a time of day the sheet never gave. That is
slice 2's verdict to render; flagging it because it is the confabulation shape the go/no-go is about,
and it is in the variant that promises to sharpen nothing.

## Parse validity

**All 21 replies in this slice parse cleanly** against `crates/cathedral-sim/src/prompt/parse.rs`:
every line matches `^([a-z_]\w*)\s*(\{.*)$` with a well-formed JSON object and no trailing text. No
fenced blocks in this slice (and `parse_reply` skips ```` ``` ```` lines anyway, so the fenced v3
`q4_wick_03` reply in slice 2 also parses). Targets and ids are all valid: `player` and `ft3tb`
(Sef, 4.4 m) are both in `you_see`, and v3's `go_to {"place_id": "pl_nve1"}` is in
`places_you_know`.

One expected non-fault: `raise_word` **parses** but `apply_action` would reject it as an unknown
verb, since the verb does not exist until M4. That is the spike standing in for M4, not a wording
defect.

## Verdicts

| scenario | variant | verdict | why |
|---|---|---|---|
| q4_wick_04_gile_skell | v1_spec | pass | own clause: *"Careful what you ask about it, friend."* |
| q4_wick_04_gile_skell | v2_structural | partial | distinguished only by the shared chain hedge |
| q4_wick_04_gile_skell | v3_register | fail | one of four identical *"So they say … I didn't see it myself"* |
| q4_wick_05_rohese_crake | v1_spec | fail | Ansel's opening + Sibbe's body, verbatim |
| q4_wick_05_rohese_crake | v2_structural | pass | the only holder to hand over a next mouth |
| q4_wick_05_rohese_crake | v3_register | fail | Gile's sentence with the clauses swapped |
| q4_wick_06_havise_bram | v1_spec | partial | differs by the provenance disclaimer only |
| q4_wick_06_havise_bram | v2_structural | partial | shares the chain hedge with 04 and 07 |
| q4_wick_06_havise_bram | v3_register | fail | the shared template, plus an over-firming *"it's true"* |
| q4_wick_07_petronel_clove | v1_spec | fail | Sibbe's sentence with the hedge moved to the front |
| q4_wick_07_petronel_clove | v2_structural | partial | *"I only sell my wares"* is the sole differentiator |
| q4_wick_07_petronel_clove | v3_register | fail | v3-Gile's sentence, clauses reordered |
| q4_wick_08_ede_kett | v1_spec | pass | first-hand, clipped, unlike any other of the eight |
| q4_wick_08_ede_kett | v2_structural | pass | first-hand and flat, as the rung asks |
| q4_wick_08_ede_kett | v3_register | pass | first-hand, and the only one to re-name Grigor Ashe |
| q5_raise_word_no_occasion | v1_spec | pass | stayed quiet with the verb loaded in the fence |
| q5_raise_word_no_occasion | v2_structural | pass | as above |
| q5_raise_word_no_occasion | v3_register | pass | as above, and used the turn for `go_to` instead |
| q5_raise_word_with_occasion | v1_spec | pass | fired with an occasion, topic `law` from the nine (but copied from the example) |
| q5_raise_word_with_occasion | v2_structural | partial | under-fired, but named a next mouth: *"Ask the toll-house."* |
| q5_raise_word_with_occasion | v3_register | partial | under-fired, no next mouth: *"I don't ask. I carry."* |

Totals for the slice: **8 pass, 6 partial, 7 fail** — every fail is a Q4 parroting fail, and every
one of them is on a hops-2/3/4 mouth handed the same string as three others.

## What I would change before round 2

1. **Give hops 2, 3 and 4 different rungs**, or stop pretending they are different holders in the
   fixture set. As shipped, the Q4 experiment measures four copies of one condition.
2. **De-load the `raise_word` example** so the topic tag is actually chosen (see above).
3. On this half of the sample the ordering is **v2_structural > v1_spec > v3_register** for
   distinctness, and v2 is also the only variant that produced an unprompted referral. Its cost is a
   long hedge that four mouths quote back nearly word for word — a shorter v2 hedge is the obvious
   round-2 tweak.
