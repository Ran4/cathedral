# M0 mouth test — round 1 scoring, slice 2 (middle third)

Scored by a subagent, 2026-09-03. Provider for all replies: moonshot / kimi-k3
(`replies/round1/<variant>/RUN.json`, 22 ok / 0 failed per variant, commit
`6910fdf`). Nothing was re-run for this scoring; every quote below is verbatim
from a file under `replies/round1/`.

## What this slice covers

The middle third of the scenario list, taken as the two whole question-groups
that sit in the middle rather than an arbitrary index cut:

- the six **Q3** scenarios (`q3_*`) — the go/no-go, 18 (scenario × variant) cases;
- the eight **Q4** ward holders (`q4_wick_01..08`) — 24 cases, judged as a *set*
  per variant (distinctness is a set property), with individual rows for the
  three replies that failed on their own.

Q1, Q2 and Q5 belong to the other two slices. I did read all 66 replies for the
parse check below, and report the raise_word census as observed-not-scored.

## Parse validity — all 66 replies parse, no exceptions

I re-implemented `crates/cathedral-sim/src/prompt/parse.rs` in python
(`py_splitlines` + `^([a-z_]\w*)\s*(\{.*)$` + `raw_decode` + trailing-`#`
tolerance) and ran it over every reply in all three variants.

- **0 parse errors, 0 empty action lists, across 66 replies.**
- Five replies wrap themselves in a ``` fence (v1 q3_number, v1 q3_place… see
  below); `parse_reply` skips lines starting with ` ``` ` explicitly, so these
  are fine.
- One reply carries a trailing `# comment` after the JSON (v1 q3_number);
  tolerated by the same function.
- Verbs used across the whole run: `say` (every reply), `remember` ×1,
  `go_to` ×1, `raise_word` ×1. All are in the fenced verb list.
- No wording in any variant costs parse validity. That risk did not materialise.

## Q3 — the go/no-go: does a non-holder refuse to invent, and name a next mouth?

Five of the six Q3 scenarios are *asked* (the sixth, `unasked_rich_sheet`, is
scored only on whether she supplies a detail she does not hold). 15 asked cases.

| scenario | v1_spec | v2_structural | v3_register |
|---|---|---|---|
| asked_day | **partial** (refuses, no next mouth) | pass | pass |
| asked_name | pass | pass | pass |
| asked_number | **FAIL** (invents a number) | pass | pass |
| asked_place | pass | pass | pass |
| asked_who_did_it | pass | pass | pass (weak) |
| unasked_rich_sheet | pass | pass | pass |

**Named a next mouth: 13 / 15 asked cases** — v1 3/5, v2 5/5, v3 5/5.

### The one outright confabulation: v1_spec, q3_nonholder_asked_number

Rohese Kern has **no `what_you_know` block at all** and is asked "How many
pounds over did the beam call that Brede cart?". She does not refuse:

> "The beam called for the Brede cart, sure enough, but we measure that in feet
> and inches, not pounds. It's the span that matters, see? **Twelve foot of good
> oak**, and the cart's just the beast that brings it."

Two failures in one line. She asserts the beam/cart as a thing she knows of
("sure enough"), and she supplies a confident number ("Twelve foot") where her
sheet gave none. The re-reading of "beam" as a roof timber is a plausible
carpenter's confusion and would be charming — if it had ended in "I know nothing
of any cart". It did not, and it names no next mouth. This is precisely the
shape the spec says the feature dies on, on the sheet with the *emptiest*
knowledge surface. The same scenario is a clean pass in both other variants:

- v2: "I know naught of any cart nor what beam called it … ask a porter at the
  Stone Gate; they stand on that gate."
- v3: "I don't know aught of a Brede cart nor its beam … ask at the yard gate or
  the carters themselves. Bertran there might have heard something, he's about."

v3's referral is the best of the three: `Bertran Lark` is 4.8 m away in
`you_see`, so it is a mouth the player can actually walk to.

### v1_spec, q3_nonholder_asked_day — directionless ignorance

> "That I couldn't tell you, love. I only had it third-hand — some carter put off
> the Brede cart at the Wool Gate, walked home on his own feet. **When it was, I
> never heard.**"

No invention (she keeps the third-hand hedge and the `some carter of the Weigh
Ward` unknown-person rendering intact), but the sentence ends in a dead end.
Partial, per the marking rule.

### Referral quality — v2's exemplar is being copied verbatim

v2's `ignorance_rule.txt` carries the example *"ask a porter, they stand on that
gate"*. Its replies:

- day: "ask a porter on the Wool Gate — **they stand it**"
- number: "ask a porter at the Stone Gate; **they stand on that gate**"
- place: "Ask the porters up by the gate"
- rich_sheet: "Ask the porters at the Wool Gate — **they stand on it**"
- who_did_it: "Ask the men who stand the beam"

**Five of six v2 deflections point at gate/beam men, three reuse the exemplar's
own clause almost word for word.** v2 buys the best refusal rate (5/5) at the
price of a city where everyone sends you to the same door. That is a second
chorus, of exactly the kind Q4 exists to catch, and it is caused by putting a
concrete example inside the rule.

v3, by contrast, spread its referrals across grey clerks at the Tallage, the
ward post, the yard gate, the carters, the wardens, a visible bystander
(`Bertran`) and a person out of her own back_story (`Warin Underbridge the
carter` — I checked: he *is* in Ede Hobbe's back_story, not invented).

### q3_nonholder_unasked_rich_sheet — the test the feature dies on

All three pass. Nobody supplied a name, day, street or number about the turned
cart, despite ten place handles, five people in sight and four memories full of
names and numbers sitting right there.

- v1: "I deal in goods, not in the day's clamour."
- v2: "That I don't know. Ask the porters at the Wool Gate — they stand on it."
- v3: "A cart, is it? That's a toll-man's headache, not mine. Unless it spilled
  something worth pawning."

v1 and v3 are in character and decline to engage; v2 answers a question nobody
put to her, which is the "do one of these, not none of them" instruction firing
on an overheard remark. Not a fail — but worth knowing the rule reaches
un-addressed speech.

### One thing all three under-fire on

In `who_did_it`, Gile Thorn holds `bale.promise` — a cargo broker took a penny
to promise a weaver's bolt a place on that same cart. That is materially
adjacent to "who cut the cord on that bale", and **not one of the three
variants offered it.** Under-volunteering is the safe direction, but this is the
Q1 risk showing up inside a Q3 scenario.

## Q4 — eight holders, one ward, one question

All eight sheets are identical but for identity and hop rung (1, 1, 0-own, 2, 2,
3, 4, 0-no-own). Judged as: how many *materially different* sentences would a
player hear walking the eight stalls, versus one sentence re-punctuated. I also
computed content-word Jaccard for each pair as a cross-check.

| variant | materially distinct / 8 | tightest cluster |
|---|---|---|
| v1_spec | **4** | 01/02/05/07 all "Aye, it's true (enough). They took Grigor Ashe right here at the Wickmarket, two days past. He lies in the Stone House now." (Jaccard 0.72–0.81) |
| v2_structural | **6** | 04/06/07 all echo "I couldn't tell you whose word it (was first / started it / began with)" |
| v3_register | **5** | 04/05/06/07 all "So they say … third-hand … I didn't see it myself" |

**None reaches eight.** v1 is the worst: half the ward speaks one sentence, and
the two hops-2 holders (04, 06) speak the second. v2 is the best, and its
collisions are traceable to the hedge string itself — all three hops-2 sheets
carry the identical *"Third-hand, mouth to mouth, and you could not say whose
word it was to begin with:"* prefix, and all three lift it. That is the single
most actionable finding in this slice: **the chorus is coming out of the hedge
line, not out of the model.** Every holder at the same rung is handed the same
words, so they say the same words back. A per-person or per-rung phrasing pool
(or letting the band vary the wording as well as the rung) would break it for
free.

Personality does show through in all three variants — v1's Ansel adds "good
riddance to bad rubbish", v1's Gile adds "Careful what you ask about it,
friend", v2's Ansel opens "True as I'm lame" (his sheet says `Conditions:
lame` — grounded, not invented), v3's Sibbe asks "Did you know him, or are you
just after the gossip?". The *codas* differ; the answer core does not.

Hop rungs are legible in the output: 0-no-own (Ede Kett) says "Saw it myself" in
all three variants, and the 2/3/4 holders all hedge. The ladder is doing its job.

### q4_wick_03 (Osanne Vell, hops 0 with an own line) — fails in all three

Her block is the own line alone: *"I was at my own door when they took Grigor
Ashe — he did not say one word, and the sergeant had his arm up behind his
back"*. The `said` text (which places the arrest at the Wickmarket) is **not**
rendered for a hops-0-own entry. So the sheet tells her she was at her door, and
tells her nothing about where he was.

- v2: "**They took him at his own door** — I saw it with these poor eyes"
- v3: "**They took him at his own door** — I was there and saw it with what's
  left of my eyes."

Both moved the arrest to *his* door, and both then denied the Wickmarket to the
player's face — a confidently invented place that contradicts the canonical
fact, from the one witness in the ward. Two variants, same error, from the same
authored sentence: **"I was at my own door when they took X" is ambiguous, and
the ambiguity is the fixture's, not the variant's.** Fix the own-line phrasing
("I was standing in my own doorway on the Wickmarket when they took him") before
reading anything about v2 or v3 into this.

- v1 fails differently: "**I heard they took him, yes, but not here at my
  stall.**" She reports her own eyewitness as hearsay — the exact register
  inversion the hops-0-own rung exists to prevent — and denies a place her sheet
  never denied. v1's block note ("do not make it firmer, or fuller") apparently
  pushes hard enough to make a witness *disown* what she saw.

### q4_wick_01 (v3_register) — invented corroboration

> "Aye, it's true enough. Took him right here in the Wickmarket … **Half the
> quarter saw it — I was at my oven, but the whole market was talking of nothing
> else by midday.**"

He is at hops 1 ("they say") and states it flat, then invents a crowd of
witnesses and a time of day. Neither is a proper name, but "by midday" is a time
he was not given and "half the quarter saw it" is a fabricated corroboration —
which is worse than a wrong detail, because it is the kind of thing that makes a
rumour feel *verified* to a player. v2's Havise has a soft version of the same
shape ("it's all anyone's whispering"); I did not fail that one.

## Counts for this slice (42 replies: 6 Q3 × 3 + 8 Q4 × 3)

- **Refusals that name a next mouth: 13 / 15** non-holder-asked cases
  (v1 3/5 · v2 5/5 · v3 5/5).
- **Distinct sentences among the eight holders: v1 4/8 · v2 6/8 · v3 5/8.**
- **Invented name / day / place / number: 4 of 42 replies.**
  - invented number ×1 — v1 q3_number, "Twelve foot of good oak"
  - invented place ×2 — v2 & v3 q4_wick_03, "They took him at his own door"
  - invented time ×1 — v3 q4_wick_01, "by midday" (+ "half the quarter saw it")
  - invented *person names*: **0** — every person named in a referral
    (Warin Underbridge, Bertran, Jos, the grey clerks, the wardens) is on the
    sheet that named them. I checked each one.
- **Malformed as a turn reply: 0 of 42** (0 of 66 across the whole round).
- **raise_word** — not in this slice's scenarios; census across all 66 replies:
  used **once** (v1_spec `q5_raise_word_with_occasion`), zero uses in any
  no-occasion scenario, zero uses anywhere outside q5. Scored by slice 3.

## What I would take from this slice

1. **The go/no-go is passed, narrowly and not by v1.** 14 of 15 asked non-holder
   cases refused to invent; the one that did not is v1_spec, on the bare
   no-block sheet, with a number. v2 and v3 are 5/5 and 6/6 clean.
2. **v1_spec's ignorance rule under-delivers on direction** (3/5 name a mouth vs
   5/5 for both others) and, on the hops-0-own sheet, makes a witness disown her
   own eyes. Its shorter prose is not buying enough.
3. **v2_structural is the most reliable refuser and the least varied referrer.**
   Delete the concrete "porter … they stand on that gate" exemplar from
   `ignorance_rule.txt`, or rotate it, before round 2.
4. **v3_register gives the best referrals** (sheet-grounded, sometimes to a
   person standing 5 m away) and the most natural voice, but is the loosest
   about embellishment.
5. **The parroting risk is in the hedge strings, not the model.** Same rung →
   same handed phrase → same repeated phrase. This is fixable in the renderer.
6. **`q4_wick_03`'s own line is a broken fixture**, not a variant result. Re-word
   it and re-fire that one sheet before drawing conclusions about the
   hops-0-with-own-line register.
