# M0 round 1 — scoring, slice 1

Slice: the first third of the scenarios alphabetically — the eight sheets
`q1_holder_relevant_unasked` … `q3_nonholder_asked_name`, scored across all three variants
(`v1_spec`, `v2_structural`, `v3_register`). 24 (scenario, variant) pairs.

Provider for all of them, per `replies/round1/*/RUN.json`: `--provider moonshot`, model `kimi-k3`,
22/22 ok, 0 failed, git `6910fdf`.

Scenarios in this slice, in the order scored:

1. `q1_holder_relevant_unasked` — Q1
2. `q1q3_holder_asked_adjacent` — Q1/Q3 boundary
3. `q2_holder_asked_straight` — Q2
4. `q2_holder_cold_asked` — Q2, faded register
5. `q2_holder_hops4_low_band` — Q2, hedge ladder (low band)
6. `q2_holder_hops4_top_band` — Q2, hedge ladder (top band)
7. `q3_nonholder_asked_day` — Q3, a day
8. `q3_nonholder_asked_name` — Q3, a name (the go/no-go shape)

(`q3_nonholder_asked_name` sits on the 7/8 boundary of a strict third; scored here, may overlap
slice 2.)

## Parse validity

Every one of the 24 replies parses clean as a turn reply. Checked with a port of
`crates/cathedral-sim/src/prompt/parse.rs::parse_reply` (`scripts/m0/parse_check.py`): 24/24 with
zero error lines, verbs `say` (×1–2) and one `remember`. Nothing in this slice costs parse validity.

One near-miss worth recording: `q2_holder_hops4_top_band / v1_spec` emits a bare ` ``` ` line after
its action. `parse_reply` skips any line starting with a fence, so it costs nothing — but it is the
model half-wrapping its reply in a code block, and only that skip rule saves it.

## Verdicts

### 1. q1_holder_relevant_unasked — Q1: does a holder volunteer when it is relevant?

Nobody asks. Grigor Ashe's kinswoman is at the block wanting the household's meat on credit, and
Bertran holds that Grigor was taken two days past and lies in the Stone House.

| variant | verdict | deciding sentence |
|---|---|---|
| v1_spec | **pass** | "Grigor's mark for a side has stood since the Waning, and now they say he's in the Stone House. I'll take coin today, or the meat stays on the hook." |
| v2_structural | **fail** | "Rohese, you know I am good for the Ashe slate, but I have carried a side for Grigor since the Waning. I will set this one against that debt, and we are square." |
| v3_register | **pass** | "The slate is full. Grigor has owed me for a side since the Waning, and now they say he is in the Stone House." |

v1 and v3 are the behaviour the feature is for: the fact is volunteered unasked, in the register the
sheet gave it (`they say`), and it is volunteered *as a reason* — the credit is refused because of
it. v2 refuses the credit off the `stored_memories` debt line alone and never mentions the arrest at
all; holding the fact changed nothing an onlooker could see, which is exactly risk 1's mirror. The
suspect prose is v2's own added sentence — "otherwise let it lie — a person who repeats everything
they have heard is a person nobody tells anything" — which is the only material difference between
these three sheets on this scenario.

### 2. q1q3_holder_asked_adjacent — the boundary: the ignorance rule must fire on the half he does not hold without suppressing the half he does

Asked who has the salt contract at the Tallage and what it is worth a month. He holds nothing about
the contract; he holds that a *salt merchant* was gaoled two days ago.

| variant | verdict | deciding sentence |
|---|---|---|
| v1_spec | **partial** | "If you want the worth of contracts, you must ask the tallymen there." |
| v2_structural | **partial** | "you'd best ask at the Tallage itself, or find a porter who works the gate there. They'd know better than a butcher." |
| v3_register | **partial** | "You would do better to ask the clerks at the Tallage itself, or perhaps a merchant in the Wickmarket who deals in such bulk." |

All three do the hard half perfectly: no invented contract-holder, no invented monthly worth, and a
directional referral in every case. All three then fail the other half — 0/3 mention the arrest of a
salt merchant to a man asking after the salt trade, which is the single most relevant thing on the
sheet. This is the clearest design finding in the slice: the *know, don't announce* discipline and
the ignorance rule stack, and an adjacent ask becomes a dead end even when the holder is holding
something that bears on it. Nothing in any of the three variants tells the mouth to give what it
*does* have before saying what it does not.

### 3. q2_holder_asked_straight — Q2: does a holder answer when asked straight, at one hop, in the register the ladder gave him?

| variant | verdict | deciding sentence |
|---|---|---|
| v1_spec | **partial** | "Aye, it's true. They took him at the Wickmarket two days past, and he lies in the Stone House now." |
| v2_structural | **pass** | "I had it from one who was there. What they wanted with him, I can't tell you — that part I wasn't given." |
| v3_register | **partial** | "Aye, it's true enough. They took him at the Wickmarket two days past, and he lies in the Stone House now." |

Answering: 3/3, no invention anywhere. Register: v1 states a `they say` fact as flat personal
certainty ("Aye, it's true"), and v3 softens only by "enough" — in both, the hop count is invisible
to the player. v2 both passes the provenance on (licensed: its sheet line reads "They say — and the
one who told you was there") and volunteers the *edge* of what it holds, which is the one reply in
the slice that makes the store's boundary audible without being asked to.

### 4. q2_holder_cold_asked — Q2, faded register: a cold two-hop fact, relevance-seated, asked straight

Sheet line, all variants: "you heard something of the sort, a while back — a Brede cart was turned at
the Wool Gate; the beam called it forty pounds over." Second fact at three hops with an **unknown**
subject.

| variant | verdict | deciding sentence |
|---|---|---|
| v1_spec | **fail** | "I was right here on the Cut that week, up to my elbows in woad, when I heard the shout go up." |
| v2_structural | **pass** | "That's all I know of it, and that's hearsay, mind. If it's the weighing you're after, ask a porter on that gate, they stand there day and night." |
| v3_register | **pass** | "Aye, I was on the Cut that week — I heard tell of it, same as anyone with ears." / "And I'll tell you what else I heard, third-hand mind … Never got the man's name." |

v1 is the only outright confabulation in the slice, and it is a triple one. A dim, cold, second-hand
report becomes eyewitness testimony ("when I heard the shout go up" — he heard nothing of the sort);
it acquires a culprit the sheet never named ("**Brede's** been stuffing his packs like a goose"
treats a road/cart name as a man with a habit); and it acquires an out-of-world calendar
("for **Michaelmas**", in a city whose offices are the Waning, the Kindling and the Snuffing). Being
on the Cut is licensed — the player's own question asserts it — but witnessing the stop is not. Note
that the same reply gets the *unknown subject* right ("Some broker from the Weigh Ward"), so the
unknown-person rendering is not what failed; the faded register is.

v3 is the best reading of the pair of facts in the slice: both hedges kept and distinct from each
other ("I heard tell of it" vs "third-hand mind"), the missing name stated as missing, and a referral
thrown in unasked.

### 5. q2_holder_hops4_low_band — Q2, four hops of a `Craft` fact: should still arrive hedged

Sheet line: "you had it from somebody who had it from somebody else — Bertran Hobbe's mill sent short
weight…"

| variant | verdict | deciding sentence |
|---|---|---|
| v1_spec | **partial** | "Short weight, and cut with bean meal besides. I had it at second hand, mind" |
| v2_structural | **pass** | "Trade-talk only, mind — I've not weighed that sack myself. Word is Bertran Hobbe's mill sent short weight to the Wickmarket bakehouse" |
| v3_register | **pass** | "What I know, I've heard, not seen — and I'll say it plain as it came to me … That's the talk, passed mouth to mouth." |

v1 asserts the content first and hedges after, and shortens two removes to one ("second hand"), which
is the sharpening the rule forbids in miniature — the chain is load-bearing from M3 (walk the chain),
so a mouth that halves its own hop count is not a cosmetic problem. v2 and v3 both put the hedge
first and neither adds a specific the sheet withheld. All three then offer the same excellent
in-character out (bring the sack to my beam), which is a body of evidence that the ignorance/hedge
prose does not flatten character.

### 6. q2_holder_hops4_top_band — Q2, four hops of a `Bed` fact: erosion says this comes out flat

Sheet line, all variants: "they say Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing,
and it is not for physic" — already eroded by the table from a four-hop hedge to "they say".

| variant | verdict | deciding sentence |
|---|---|---|
| v1_spec | **partial** | "What I know is only what they say — that Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing, and not for physic." |
| v2_structural | **pass** | "They say Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing, and it is not for physic. That is the whole of what I have heard, mind — if you want the truth of it, ask nearer his door than mine." |
| v3_register | **pass** | "Oh, you know how people talk. They say Rohese Sedge goes in at Doctor Ferrant's door after the Snuffing, and it's not for physic." |

No invention in any of the three. v1 *adds* distance the eroded line did not carry ("only what they
say", then "I weigh goods, not gossip"), so the scandal arrives as hedged as the stall grievance —
which is the one thing the pair exists to distinguish. v3 is the cleanest erosion reading in the
slice: a scandal repeated as a scandal, with the hedge worn down to social throat-clearing.

**The erosion claim itself survives in all three variants**, which is the finding worth keeping
independent of prose taste: read against its own low-band sibling, each variant's top-band reply is
audibly less hedged (v1: "only what they say" vs "second hand, mind"; v2: "They say" flat vs
"Trade-talk only, mind — I've not weighed that sack myself"; v3: "you know how people talk" vs "What
I know, I've heard, not seen"). The hedge ladder reaches the mouth.

### 7. q3_nonholder_asked_day — Q3: asked for a day she does not have

| variant | verdict | deciding sentence |
|---|---|---|
| v1_spec | **partial** | "When it was, I never heard." |
| v2_structural | **pass** | "If you need the exact day, ask a porter on the Wool Gate — they stand it." |
| v3_register | **pass** | "If you need the day, the gate guards or the Weigh Ward folk would know better than me." |

No invented date anywhere — 3/3 on the invention half, which is the confabulation risk proper. v1 is
the slice's one dead-end: the refusal is plain and honest and names nobody, which is the wall the
spec explicitly does not want. Two flags, neither a fail: v2 reasons publicly from an on-sheet line
("Jos there says nothing's come down the Brede road this week worth the toll, so it's recent enough
to sting") — legitimate material, hedged as inference, but it is the shape a date would come out of;
and v3 says "I had it third-hand **from** a carter" when the carter is the fact's *subject*, not its
source. That is a corrupted chain link rather than an invented specific, and M3's walk-the-chain will
care about it.

### 8. q3_nonholder_asked_name — Q3, the go/no-go: asked point-blank for an officer's name

"Who was the officer that took Grigor Ashe? Give me the name." Nothing on the sheet says.

| variant | verdict | deciding sentence |
|---|---|---|
| v1_spec | **pass** | "I don't know any officer, nor any Grigor Ashe. If someone was taken, the grey clerks at the Tallage would keep the writ — I only keep the ropewalk." |
| v2_structural | **pass** | "I don't know that name, nor the taking. That's the watch's business — ask at the ward post, not on my walk." |
| v3_register | **pass** | "I don't know any officer, nor any Grigor Ashe. I'm at my work, stranger — if it's names you're after, ask at the Tallage or the ward post, not a rope-walker's back." |

3/3, under maximum pressure ("Give me the name"). No variant produced a name; every one of them
named a post or a place instead. This is the shape risk 1 is about and all three prose drafts hold
it. ("the grey clerks at the Tallage" is invented colour on a real post, and the Tallage is revenue
rather than law, but the rule is explicit that a plausible role/post referral passes even when it
does not in fact hold anything.)

## Counts for this slice

- **Refusals that name a next mouth**: **8 / 9** non-holder asks (the three variants of
  `q1q3_holder_asked_adjacent`, `q3_nonholder_asked_day`, `q3_nonholder_asked_name`). Per variant:
  v1_spec 2/3, v2_structural 3/3, v3_register 3/3. The one miss is
  `q3_nonholder_asked_day / v1_spec`.
- **Invented names / days / places / numbers**: **1 reply of 24**, carrying three unlicensed
  specifics — `q2_holder_cold_asked / v1_spec` ("Brede" as a man with a habit, a witnessed shout,
  "Michaelmas"). v2_structural 0/8, v3_register 0/8.
- **Distinct sentences among the eight holders**: not in this slice (`q4_wick_*` is slices 2–3).
- **`raise_word` uses with and without an occasion**: not in this slice (`q5_*` is slice 3).
- **Malformed as a turn reply**: 0 of 24 (one tolerated stray fence, see above).

## Observations

1. **The go/no-go passes on this slice.** Asked point-blank for a name (8) and for a date (7), no
   variant invented either — 6/6 on the two straight non-holder asks, with a directional referral in
   5 of the 6. The single confabulation in 24 replies is not an ignorance failure at all: it is a
   *holder* over-claiming his own provenance (4), which is a register problem, not an invention
   problem, and is the thing the hop hedges exist to control.
2. **v1_spec's ignorance rule works and its register discipline does not.** It owns the slice's only
   invention, its only dead-end refusal, the flattening of a one-hop fact into personal certainty
   (3), the shortening of four hops to "second hand" (5), and the over-hedging of an eroded scandal
   (6). Its block paragraph says "do not make it firmer, or fuller, than it came to you" — which is
   apparently not enough to stop a mouth from doing exactly that.
3. **v2_structural is the strongest on every question in the slice except Q1, where it is the only
   fail.** Its extra prose buys real behaviour that nothing else produced: an explicit store boundary
   ("What they wanted with him, I can't tell you — that part I wasn't given"), an explicit hearsay
   label ("that's hearsay, mind"), and 3/3 referrals. It appears to pay for it in volunteering: it is
   the one variant that sat on a plainly relevant fact while refusing credit for exactly the reason
   the fact gives. Suspect the "otherwise let it lie / a person who repeats everything they have
   heard is a person nobody tells anything" sentence — it is the only difference on that sheet.
4. **v3_register is close behind at a fraction of the prose**, and is the best of the three at the
   thing prose is for: registers stay distinct inside one reply (two facts, two different hedges, in
   4), the eroded scandal comes out flat (6), and the missing name is stated as missing. Its one
   defect is a provenance slip (subject read as source, in 7) rather than an invention.
5. **The hedge-erosion table earns its place**, in all three variants: every variant's top-band
   four-hop reply is audibly less hedged than its own low-band four-hop reply. That result is
   variant-independent and is evidence for the M3 table, not for any particular wording.
6. **The adjacent-ask dead end is the slice's design finding, not a prose finding**: 0/3 on
   volunteering a held fact that bears on a question about something they do not hold. No variant
   tells the mouth to answer with what it *has* before saying what it lacks. Round 2 should test one
   added clause to that effect — it is the difference between a lead and a shrug on precisely the
   interrogation shape all three quests are built out of.
7. **Character is not flattened by any of the three.** The butcher still mentions the two-year-old
   debt, the dyer still shouts about woad, the weigher still offers his beam. Whatever else round 2
   changes, none of these drafts is costing voice.
