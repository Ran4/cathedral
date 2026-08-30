Status: SPEC ONLY — unimplemented (2026-08-27)

# Quest: ring a dead woman's name at Marenstide

Working title: **One Bell Is Not a Name**

This is a six-day personal questline inside the larger game. It is **not** the game's premise, main loop, or a
complete GDD. It began life as one — the companion overview beside this file was written as a whole-game design
and has been reframed — and the reframing is the point: the city does not need a premise, it needs a reason to
get up in the morning, bounded, dated, and repeatable in kind.

The quest borrows the player's ordinary verbs and the city's ordinary simulation, gives them one six-day
problem with a price on it, and leaves behind a name in a book that was not there before.

## One-sentence pitch

Twenty-two years after the Hammering, a rope-picker gives you his sister's name and no money; you have six game
days to raise the paupers' ninety-six, find someone still living who will stand and say she was who you say she
was, and get her room on the Cut into a notary's book — so that when Renn Hobbe reads the roll of the drowned at
Marenstide, Noll Fitch rings Maren Smallvoice thirty-four times — one slow stroke a year of the life — and
somebody hears it.

## Player-facing promise

The player is an errand-runner, a beggar of favours and a forger of small paperwork — not a hero, not an
investigator solving a murder, and not the person who fixes the naming law.

To get the bell rung they must:

1. **raise ninety-six sparks** and hand them to the sexton, from purses that each cost something different;
2. **find someone still alive who remembers her**, and get one of them to stand at the reading and say so;
3. **get the room she held entered in the notary's book**, over a sitting tenant whose family name is the one
   thing in the Reed Ward that everybody writes down;
4. **be in earshot at Marenstide** and say her name where people are standing.

There is no combat, no puzzle box and no dialogue tree. Every one of those four is a conversation with somebody
who has their own reason to say no, held at a place they will actually be, before a bell they will actually
leave at.

## Why this belongs in this game

The premise is not an invention laid over the corpus; it is one paragraph of the corpus, played.

`lore/the_dry_boatmen.md:361-376` establishes all of it: Saint Maren keeps the roll of the drowned; at
Marenstide her roll is read at the church; since the diversion almost nobody drowns, so after F.415 the parish
began adding the ward's Hammering dead to the roll **because the Hammering dead were denied their name-knells
and this was a bell that could still be rung for them**; the traditional families object in the ward's flattest
voice — *You cannot drown under slate* — and the wick-priest Renn Hobbe reads them anyway and takes the
argument afterwards in the porch, where it belongs.

`lore/families/family_rud.md:38,55,185,197` supplies the other half. The naming law's worked example of a
*landed* name is the Hawsers: *there are more Hawsers than Alders*, and **a Hawser is nobody's concern, and the
street lets the byname take him**. The Ruds are the same law one stage further on — a name that outlived its
property and is kept alive by nothing but **everyone else's paperwork**: the fulling-master's ell-tally, the
ward book, the debt list. So the quest is the naming law with a price on it. To get a Hawser into a book, you
must argue with a Rud who is already in six of them.

The cast is already carrying it. Noll Fitch's sheet holds the memory *I helped bury the Hammering dead when I
was thirty-five; the common toll was not a name-knell, whatever the Chapter wrote*, and his round is to open the
ground, **ring Maren Smallvoice for the name-knell at one slow stroke a year of the life**, and chalk the newest
buried name on the charnel-door lintel. Renn Hobbe's is *I was twelve in the Hammering and watched my father
climb toward the bells while stone fell; I will not tell a family that one common toll settled their dead.*
Cobb Hawser **is the man who holds the drowned** — every name the Serle has taken in thirty years, in order,
with the weather and the boat — and *nobody has asked him for it in twenty years*. Tilman Rue stands at the back
of paupers' burials watching **who counts the strokes of the name-knell and who reads the chalked name on the
lintel — and who already knew it**. None of that was written for this quest. The quest is what happens when
somebody finally asks.

And it makes the shipped simulation matter at the same time:

- 519 authored people already stand on daily rounds pegged to seven office bells, so a person is *found*, not
  selected;
- speech already reaches everyone within 20 m and may be remembered, so a claim is a public act;
- `knows` / `remember` already make the player a stranger whose word weighs nothing until they are named;
- two-sided offers, the spark and the item catalogue already support payment, alms, debt and bribery;
- notices, custody, the Stone House, posted fees and the grab reflex already provide non-combat consequence;
- chalk marks already change code-side behaviour and can be forged and scrubbed;
- the Night Office already settles the day into thirty Majors' memories and eight ward moods overnight;
- Maren Smallvoice already exists as an assembled stroke pattern that carries 300 m
  (`src/soundscape.rs:946-966`), and the sexton's sheet already has him ringing it and chalking the newest
  buried name on the charnel-door lintel.

Without a quest those systems are a sandbox with excellent weather. This feature adds one bounded reason to
combine them, then gets out of the way.

## Canon boundary

**Canonical and already authored:**

- the roll of the drowned, read at Saint Maren's at Marenstide by the wick-priest (`lore/the_dry_boatmen.md:354-376`);
- the parish's post-F.415 practice of adding the ward's Hammering dead to that roll, and the reason for it —
  they were denied their name-knells;
- the traditional families' objection, *You cannot drown under slate*, and Renn Hobbe reading them anyway;
- Marenstide as a late-summer feast — eel fair and remembrance of the drowned (`lore/core_lore/calendar_and_history.md:31`);
- the Hammering: the localized hailstorm of F.415, ten or fifteen minutes, thousands killed by impact and
  falling roofs, twenty-two years before the present F.437 (`lore/the_great_rains_and_the_hammering.md`);
- the paupers' **ninety-six sparks** as the price of a pauper's funeral with knell
  (`lore/second_sun/11_glossary_and_naming.md:120`, `07_what_everyone_knows.md:38`, and Renn Hobbe's own sheet);
- the name-knell's form: **one slow stroke a year of the life**, so Alis at thirty-four is thirty-four
  countable strokes. This is not only lore: `src/soundscape.rs` implements it as
  `BellPattern::NameKnell { years }` with that exact comment, and `CATHEDRAL_DRIVE`'s `bell knell 34` rings and
  logs thirty-four strokes today;
- Noll Fitch the sexton (`ax5nf`), who rings Maren Smallvoice and chalks the newest buried name on the
  charnel-door lintel;
- Odo Trask the notary (`fo6gl`) at the Tallage toll-house, whose fee book lies open on the counter and who
  cannot be bought;
- the naming law, the Hawsers as its landed example and the Ruds as the name kept alive by other people's
  paperwork;
- Cobb Hawser (`g6cbb`), rope-picker, pauper, of the Reed Ward — a living Hawser;
- the Unwalled and the Breach faction, and Tam Rud (`et7rd`) sworn to them;
- the Custody paying its quiet people in dry money across Lise Copp's counter (`fl5cp`), and Tilman Rue
  (`ar5tl`) counting faces at paupers' burials — specifically watching who counts the strokes and who reads the
  lintel;
- **the quiet purse**: some purse has paid the paupers' ninety-six for years, neither the sexton nor the
  wick-priest has ever asked whose it is, and both of their sheets say so. The quest does not invent the money
  that pays for paupers' bells; it makes the player need it.

**Quest additions, not retroactive lore canon:**

- **Alis Hawser** herself: a Hammering dead of F.415, aged thirty-four, sister to Cobb, and her room on the Cut.
  No sheet for her exists and none should be written; she is a name in a letter and in other people's memories,
  which is exactly what the naming law says a Hawser becomes.
- the letter, and Cobb having kept it twenty-two years;
- the parish charging the paupers' ninety-six for a knell rung **out of season for someone the parish did not
  bury at its own cost** — canon prices the funeral-with-knell at ninety-six; that this is also the price of a
  bell twenty-two years late is the quest's own rule, and Renn Hobbe's stated reason for it is that the parish
  has only the one price for a bell;
- the parish requiring **a living witness** before a name is added to the roll. Renn Hobbe is not the obstacle
  — his sheet has him refusing to tell a family that one common toll settled their dead, so he is the quest's
  ally from the first scene. The requirement is the parish's and the ward's, and he is the one who has to hold
  the line in the porch afterwards;
- the notary requiring **two householders of two different wards**, neither of them kin, before a retrospective
  deed is entered. Odo Trask's own sheet supplies the principle in his own words — *an unwitnessed word is only
  weather*;
- the six seeded memories of Alis on existing sheets;
- a dated Marenstide six days after the offer, with an authored reading window;
- the quest's own paid errands, receipts, casebook and outcome packages.

**The quest must not:**

- resolve, touch or imply anything about the impossible light or the second sun. The Hammering's cause is
  canonically unestablished (`lore/the_great_rains_and_the_hammering.md`) and stays unestablished; there is no
  light rule, no Concurrence and no Lanthorn interior in this quest at all;
- settle whether the Hammering dead *should* be on the roll of the drowned. Both sides are authored and both
  stay standing after every ending;
- invent a Hawser dynasty. The whole force of the name is that it anchors nothing and nobody has a use for it;
- make the parish, the notary or the Custody corrupt as a class. Odo Trask cannot be bought, and the routes
  around him are forgery and misdirection, not bribery;
- put water in the Cut, or make the Serle anything other than gone.

## Scope boundary: quest versus base game

### This feature owns

- the offer, activation and resolution of this one quest;
- its six-day clock and the authored Marenstide reading window;
- the letter, the casebook projection and the quest's leads;
- the ninety-six and the four purses that can pay it, with their authored costs;
- the six witness records, their conditions, and their withdrawal reasons;
- the room, the sitting tenant's counter-claim and the retrospective deed;
- the reading itself: the roll, the knell, the lintel name and who is standing there;
- five outcomes, an integrity ledger and the aftermath packages;
- deterministic fake-backend and headless coverage for the whole quest.

### This feature consumes but does not own

- player movement, speech, STT/TTS and typed chat;
- items, stacks, sparks, two-sided offers, hunger and stalls;
- the law ladder, notices, custody, the Stone House and the grab reflex;
- generic chalk marks;
- NPC rounds, navigation, homes, attention/stage gating and the Night Office;
- bells, the soundscape and weather;
- the clock, the offices, the day counter and the week;
- the base game's save/load, quest framework and progression, none of which exist yet.

### Foundation dependencies

Three advertised experiences are foundation work, not current systems. The quest must not promise them before
they exist.

| Dependency | What exists now | What this quest requires |
|---|---|---|
| Persistence | Settings persistence only (`config.ron`); no world or engine checkpoint. | A versioned whole-engine checkpoint. A six-day quest cannot be a single process lifetime, and a quest-only save would restore the ninety-six while inventory, custody, marks, knowledge and NPC positions reset. |
| Earning money | Nothing. Sparks move only through two-sided offers an NPC chooses to make; there is no wage, workplace, employment or payout path anywhere in the sim. | Either a base-game wage seam, or — as specified here — a small set of **authored quest errands that pay a coded amount on a receipt**. The quest owns the errands; a general hire/wage economy is explicitly out of scope and stays a base-game feature. |
| Player curfew | The Snuffing changes NPC rounds and the bench sergeants go to bed at it; there is no player-curfew offence or watch-witnessed detection. | Watch-witnessed proximity detection before any route promises curfew consequences. Until then, night play is atmosphere, not a route. |
| A named-person bell | **Most of it already ships.** `src/soundscape.rs` has `BellPattern::NameKnell { years }`, documented there as *Maren Smallvoice: one slow stroke per year of the life*, with `MAX_KNELL_YEARS` = 120, a 300 m radius taken from `lore/second_sun/design/06` §2, and an evidence line a drive script can assert a stroke count from; `CATHEDRAL_DRIVE`'s `bell knell <years>` rings it today. What is absent is the **sim** side: no action rings it, and no percept carries a name. | One sim-side action that fires the existing pattern with the dead woman's age, chalks a one-slot lintel mark, and emits a percept carrying the name. Genuinely small. |
| Bounded rumor | A design exists (`features/rumors.md`); no runtime propagation. | Preferred for the day-five turn. A temporary authored quest fact queue is acceptable through M3 and must be designed for deletion. |

### Explicit non-goals

- making this the game's opening premise, tutorial or mandatory main story;
- a general quest generator, quest markers, or a global reputation number;
- a job or crafting simulator — the errands are proof of presence, not a wage economy;
- combat, health, skill trees, romance, procedural characters or full building interiors;
- writing Alis a character sheet, a portrait or a voice;
- resolving the Hammering's cause, the Serle's return, or anything in `lore/second_sun/`;
- free-form law, deeds or roll entries authored by the model.

## Quest availability and entry

A midgame quest, available once the player can move, speak, hold sparks and be arrested, and once the game can
persist world state.

Activation predicates:

- the world year is F.437 and Marenstide has not yet passed this year;
- the quest has not already resolved;
- the calendar can place Marenstide **six game days** after the offer, at a reading window in the afternoon
  office;
- Cobb Hawser is alive, at liberty and on his ordinary round;
- Noll Fitch and Renn Hobbe can both be reached at Saint Maren's;
- Odo Trask can be reached at the Tallage toll-house;
- no other authored event owns the same six-day window.

The player needs no ward standing and no trade. Being nobody is the premise: a Hawser is nobody's concern, and
so is a stranger, which is why Cobb asks one.

### Offer scene

Two scenes on the first day, in this order, both at places the cast already stands.

**Cobb Hawser, at Tanners' Slip.** Sixty-three, a pauper, a rope-picker teasing junk cable back to tow, and —
his sheet's own claim — **the man who holds the drowned**: every name the Serle has taken in thirty years, in
order, with the weather and the boat and who was fool enough to send them out. He will tell the lot for a bowl
at the Hungry Ox. Nobody has asked him for any of it in twenty years. He was born boat-family and the name went
with the hull; the street named him Hawser because rope is what he gathers now.

He gives the letter — an ordinary two-sided offer of an item, refusable — and says the thing the quest hangs on:
*she was thirty-four, she died under the slate in F.415, and they never rang her.* He keeps every drowned name
in his head and cannot get his own sister onto the parish's roll, because she died under a roof. He has never
had ninety-six sparks at one time in his life and never will.

**Noll Fitch, at the charnel door.** The sexton — Major, fifty-seven, thirty-five in the Hammering year, and
carrying the memory that *the common toll was not a name-knell, whatever the Chapter wrote*. Everyone gets a
name and a bell, he says, and that is the whole of his politics. He states the terms, and they are the quest:

- the parish has one price for a bell, and it is the paupers' ninety-six — and he will take it from a stranger
  as readily as from the quiet purse that has paid the paupers' knells for years without ever giving a name,
  because it has never asked anything of him either;
- Renn Hobbe will read her gladly and cannot: the ward wants somebody living to stand up and say she was there,
  and a priest who reads a name on nobody's word has to answer for it in the porch every Marenstide after;
- and the notary will not enter a room to a dead woman on one man's word, least of all her brother's.

He will also tell the player, without being asked and in four words, how long the bell takes: **a stroke a
year.** Thirty-four of them, slow, and Rue will count every one.

Accepting seeds `quest_hawser_letter` (a quest document), the casebook, three open leads and the crier's daily
count. Declining does not freeze anything: Marenstide arrives, the roll is read without her, and the quest
remains offerable next year — which is exactly what has happened twenty-two times.

## Completion and duration

- **Expected play time:** 4–6 hours.
- **World time:** six game days, from the offer to the reading at Marenstide.
- **Success condition:** at the reading, the ninety-six are in the sexton's hand, one valid witness is standing,
  and the deed is entered — then the knell rings and the player says the name in earshot.
- **Failure condition:** any of the three missing at the reading. This resolves to an authored outcome, never a
  game-over.
- **Postcondition:** the roll is read either way; the lintel carries a name either way; the city goes on.
- **Continuation:** the player keeps playing on the same save after every outcome.

The quest never waits. Being in custody, asleep or absent at the reading still produces an outcome from the
state actually recorded.

## The player's daily loop

Each day supports one or two serious leads. The loop is the same six steps every day, and the quest never tells
the player which lead is wise.

1. **Read.** The crier's count, the casebook, and what last night changed.
2. **Choose.** One or two leads: which purse, which name, which risk.
3. **Find.** Learn a person's round and reach where they actually are — not a marker.
4. **Ask.** What would move them, and who else has to hear it.
5. **Do.** Carry, work, pay, ask, witness, mark, inform or forge.
6. **Return.** Before their next leg or bedtime, for a receipt, a refusal, or a new lead.
7. **Night.** A roof, or the watch; the Night Office settles what was said into memories and ward moods.

The player should always be able to name one next lead and why it matters. They should frequently be unsure
whether following it is wise.

## Six-day dramatic structure

| Day | Beat | Expected holding | New pressure |
|---|---|---|---|
| 1 — The Letter | The two offer scenes; the three terms; one memory found. | purse 0, witness 0/6, room unheard of | being unknown to the parish; learning the office clock |
| 2 — The Price | The ninety-six becomes a real number; the room turns out to have a man in it; the first fast purse is offered. | purse 4–12, witness 1, room named | money against time |
| 3 — The Wavering | A witness withdraws on the authored objection: *you cannot drown under slate*. The notary's two-householder rule bites. | purse 12–28, witness 1–2, one signature | the best-placed witness is the one who most objects |
| 4 — The Shortfall | Honest errands can no longer close the gap in the days remaining. Lending, dry money, the quiet purse and forgery all open. | purse 30–55, witness 2, room contested | method becomes the choice |
| 5 — The Turn | What the player said comes back through the Night Office and the ward moods. The tenant or the cell reacts. Arrest is live. | purse 55–90, witness 2, deed entered or lost | your own words |
| 6 — Marenstide | The eel fair, the Dry Race, and the reading. The roll, the bell, the name said or not. | 96, a witness standing, the room in the book | no grace period |

The structure is authored. The order in which the six are approached, the purse used and the method are not.

## The three requirements

### 1. The purse — ninety-six sparks

Ninety-six is deliberately more than honest errands can produce in six days. Priced against the shipped
catalogue (`assets/world/items.json`) it is eight coats, or forty-eight loaves, or a little over two bolts of
broadcloth: a real sum for a city where ale, a herring and a chalk pen are one spark each and the gaol's posted
fee is three. That gap is the quest: the shortfall is what the other purses are for, and each of them prices
the name differently.

| Purse | How it is got | What it costs | Ending it points at |
|---|---|---|---|
| **Your own getting** | Authored quest errands, paid on receipt: carried words, buckets from Bitter Well, a fuller's load, a barrow to the Moorings, sacks at the Kindling. 1–4 a task; a hard day is 8–12. | Days. The clock is the price, and every hour earning is an hour not asking. | Well rung |
| **A lender's book** | Averil Skell (`g2rhs`), fish smoker of Maren's Green, lends at a spark on the dozen — her sheet's own sideline, not a moneylending trade. | A debt with a date, and Averil sells debts. | Well rung, then a creditor |
| **The Custody's dry money** | Lise Copp's counter (`fl5cp`), where the grey clerks pay their quiet people. Tilman Rue (`ar5tl`) already counts faces at paupers' burials and would like to know who attends this one. | You are named in a book you cannot read, and the Custody learns the funeral's faces. | Rung by the wrong purse |
| **The quiet purse** | Somebody has paid the paupers' ninety-six for years and neither the sexton nor the priest has ever asked whose it is — both sheets say so, and somebody meets in Fitch's crypt after a pauper's burial while he keeps the hinge oiled and himself elsewhere. Ask, and it will pay for Alis too, on a condition. | The condition is that the name on the lintel becomes a pass — a word said at a door by people who need one. Alis stops being a woman and becomes a password, and Rue is at the back counting who already knew it. | Rung by the wrong purse |

Nothing forbids mixing them, and mixing them is the honest middle: the integrity ledger records the provenance
of every spark handed to the sexton, and the ending reads that, not a morality score.

### 2. The witness — six memories, one standing

Six existing sheets are seeded with a memory of Alis. Every one of them was alive and in that quarter in F.415 —
a content rule, not a coincidence to be broken. The youngest, Wyn Alder, was twelve, and her sheet already
carries what the Hammering did to her; the rest were between thirty and forty-four.

| Person | Id · tier · age | Why they remember | Why they might not stand |
|---|---|---|---|
| Nan, washerwoman | `e9nan` · Major · 66 | Forty-four in the Hammering year, and her sheet's own boast is that she has washed for half the Cut so long she knows a household by its linen and remembers every debt, birth and grudge on that street the way a rent-book does. | A pauper and a widow of the Weigh, and her daughter Sible Mott wears a grey coat at the chapter house — which makes standing up in a Reed argument a family matter as well as a public one. |
| Gude, herb-seller | `cg6ud` · Major · 66 | Sixty-six years on Maren's Green with a stall by the fish-hall steps; she has sold simples to that street her whole life. | She is one of the **Spared** — she looked up through the Great Rose at the strong hour and said plainly that she saw one sun — and has been pitied, envied and doubted ever since. She is tired of being asked to remember things for other people, and she will want to know who is paying. |
| Cobb Hawser | `g6cbb` · Minor · 63 | Her brother, and the man who holds the drowned. | Kin, and the notary discounts kin. He knows it, which is why he asked a stranger, and it is the quest's tutorial in its own rules. |
| Hamel of the Reach | `bn2hm` · Minor · 58 | Tallied freight into the dry warehouses on the Cut's cartway for thirty years and knows who held which door. | **The most exact fit in the cast:** he was born boat-family, the name went with the hull, the street named him for the reach, and the ward's reckoning sent him to the Bench *because there are more Hawsers than Alders and the fallen like to see one of their own on a seat.* Which is also why standing up for a fallen name in an election year is not free. |
| Idonea Tarn | `bn1id` · Minor · 54 | Keeps the Tarn cooperage ledgers on Tanners' Slip, counted in a book that has never once been wrong; she was thirty-two when it happened. | She counts before she speaks, her kinsman Tobin is the reason her votes are never quite about smuggling out loud, and she wants to get through one more reckoning without being made to say an awkward word in public. |
| Wyn Alder | `gw4ld` · Major · 34 | **She was twelve when the Moorings roof came down in the Hammering and killed her father.** She took the fore-pole when her brother froze. She remembers that street's dead exactly. | **The authored objection, and it is personal.** *Water keeps its name and its dead, and the sky can mind its own business.* Her own father died under a roof and she has never asked the parish to read him among the drowned. If Alis is read, why not Alder — and if not Alder, why Alis? On Bellday she stands in the nave with the rest of them and speaks her dead into the green beam, and one of the names she speaks there she would not say aloud in the street. |

Wyn Alder is the quest's best scene and its designed reversal, and none of it was invented for the quest: her
father's death, her line about water keeping its dead, and the name she will not say in the street are all on
her sheet. The day-three withdrawal is not a betrayal and not a broken flag; it is a position, and the reason it
lands is that she is not defending a tradition, she is defending her father from being made an exception to.

The routes out are all authored, and the best of them is not persuasion:

- **ask for Alder too.** Get her to name her own dead to the priest, and the objection inverts: the two names go
  up together and Wyn stands for both. Renn Hobbe, whose father climbed toward the bells while stone fell, is
  the last man in the parish who will refuse that.
- get the name read **apart from** the roll of the drowned, in the porch argument Renn Hobbe already takes every
  Marenstide — satisfies the traditional families, costs the parish's neatness, and is a smaller bell.
- stand a different witness and let Wyn say what she thinks, publicly, at the reading.
- lean on her: Averil Skell holds forty-eight sparks of her brother Ewart's debt and collects in the street.
  Buying that leverage works, and the Night Office will have carried what you did to two wards by morning.

A witness is `Standing` only while: they are alive and at liberty; they were seeded with the memory; they have
not withdrawn on an authored reason; and the deed has not been entered in a way their sheet objects to. Friendly
talk is not a witness. Hostility is not a refusal.

### 3. The room — a deed twenty-two years late

Alis held a room on the Cut in the Reed Ward. Tam Rud (`et7rd`, Major, 29) has it now: a journeyman fuller,
sworn Unwalled, seven years old when she died, and — this is the whole difficulty — a **Rud**, which is to say a
name that appears in the fulling-master's ell-tally, the ward book and a debt list, while a Hawser appears in
nothing at all.

Odo Trask (`fo6gl`, Minor, 47, Notary of the Tallage) will enter a retrospective deed on:

- **two householders of two different wards**, in person at the toll-house, before the Waning, saying the room
  was Hawser's before the slate;
- neither of them kin;
- and no live counter-claim standing unanswered.

Tam is not a villain and must not be written as one. His sheet gives him twenty-nine years, thirty-six sparks a
day in the fulling stocks for a master who counts every ell, an old fuller who taught him his letters and then
died of the damp in Custody keeping after the Whisper Arrests, and a Chapter that barred the nave doors at the
Passing last year and broke a child's arm in the crush. He is angry the way wet wool is heavy — all through, and
slow to dry — and he says far more than he should after two pots at Renna Tapster's, which is the opening the
player has.

Routes to the room, each with an authored cost:

| Route | What it takes | What it costs |
|---|---|---|
| Persuade Tam | He does not want the room's history, he wants the room. An agreement that he keeps it and she gets the record. | Nothing but time — and it is the only route that leaves nobody worse off, so it is also the slowest. |
| Buy the counter-claim off | Tam owes the fulling-master and Averil Skell. Settle it and he withdraws. | Sparks you needed for the ninety-six. |
| Inform on him | He is sworn Breach and he talks after two pots. A word to a grey coat within earshot ends the counter-claim by removing the claimant. | A man in the Stone House by the same road that killed the fuller who taught him his letters — which is on his sheet, and should be said back to the player by somebody. Plus the cell's enmity and Rue's interest in you. |
| Forge the ward's hand | A chalked cross on the door reads as the ward's own, because nothing that reads a mark asks who drew it. | The ward overwrites any notice older than two game days with its own cross (`notices.rs:605`), so a forgery buys about two days, not a win — and being seen drawing it is an offence. |

The notary cannot be bribed. That is on his sheet, and it is why the routes are forgery and misdirection.

## The reading at Marenstide

At the authored afternoon window at Saint Maren's, in this order:

1. the quest state is frozen and evaluated in stable order: sparks handed, witness standing, deed entered;
2. Renn Hobbe reads the roll of the drowned — the canonical annual act, which happens whether or not the player
   did anything;
3. if all three hold, Alis's name is read, Noll Fitch rings Maren Smallvoice as a name-knell — **thirty-four
   slow strokes, one a year of the life, countable and counted** — the sim emits the 300 m percept, and the
   sexton chalks her name on the charnel-door lintel in the one-slot `LintelName` mark;
4. the player may then say her name; a `say` inside the window is checked for the name and for who was within
   20 m to hear it;
5. the traditional families' objection is voiced by whoever of them is present, always, in every outcome;
6. Tilman Rue, if present, does what his sheet says he does: watches who counts the strokes, who reads the
   lintel, and who already knew the name;
7. the outcome package is applied and the quest resolves after the first morning receipt.

The reading may not wait on provider latency. The sim commits the arithmetic and the chalk; available actors
voice a bounded selection around it.

## Deterministic quest state

Authoritative state belongs in `cathedral-sim`, because the deadline, the fake backend and the reading must be
deterministic and headless. Mutable state lives on `World`, not only `Engine`, because NPC actions receive
`&mut World`.

```rust
struct KnellQuest {
    phase: KnellPhase,
    offered_at: Option<WorldTime>,
    reading_at: WorldTime,
    purse_paid: u32,                       // of NINETY_SIX
    purse_provenance: BTreeMap<PurseId, u32>,
    witnesses: BTreeMap<ActorId, WitnessState>,
    deed: DeedState,                       // Unheard | Claimed | Contested | Entered | LostToTenant
    tenant_claim: TenantClaimState,
    tasks: BTreeMap<QuestTaskId, QuestTaskState>,
    receipts: Vec<QuestReceipt>,           // append-only, stable ids
    known_leads: BTreeSet<QuestLeadId>,
    integrity: KnellIntegrityLedger,
    outcome: Option<KnellOutcome>,
}
```

```text
Dormant -> Offered -> Active -> ReadingWindow -> Resolved
                  \-> Declined ------------------^ (read without her)
```

Every collection that enters a snapshot, a prompt or a fixture is stably ordered. No `HashMap` iteration order
may leak into goldens.

### Receipts

Append-only facts with stable ids, the only thing the quest counts:

- a quest errand was completed inside its allowed office and paid `n` sparks from purse `p`;
- an item was handed from A to B within the 4 m offer radius before a named bell;
- named people were within the real 20 m hearing radius when a claim was made;
- a person said they remember Alis Hawser, and to whom;
- a witness pledged to stand, or withdrew for a named reason;
- a householder gave the notary a statement, with ward and kinship recorded;
- a mark was drawn, scrubbed or proved forged;
- sparks were handed to the sexton, with provenance;
- the name was read, the bell rung, the lintel chalked, and who was inside 20 m.

Not all receipts are shown. The casebook projects only what the player learned or caused.

## Quest tasks: errands, not a job simulator

The quest needs a small generic seam for authored paid errands. It does not need an employment system, and it
must not grow into one — a general wage economy is a base-game feature and is named as a dependency above.

Task kinds, all of which the world already physically supports:

- `CarryBetween` — a stack or prop between two anchors before an office (buckets from Bitter Well, a fuller's
  load, a barrow to the Moorings);
- `SpokenErrand` — words given to carry: the sender's `say` inside 4 m is tagged by code as an errand memory at
  both ends, the addressee's turn judges what was actually said into the microphone, and the payment offer is
  judged by the sender's memory. No verb, no log; the bell is the timer;
- `AttendWith` — be present with named people when a domain event occurs (a burial, the reading);
- `StatementTo` — bring a person to a person before a bell and have them say a thing in earshot.

Every task specifies its anchors, required item or empty hands, allowed offices, completion predicate, witness
policy, lapse behaviour, receipts, and one terse in-world line plus an accessible text fallback. No timed input,
no hidden roll, no model-authored completion. The player's skill is route, timing, witnesses and framing.

## The casebook — the letter

One diegetic surface, a projection of known state and never omniscient. It is the letter, with what the player
has learned written on the back of it.

Always visible after acceptance: days to Marenstide and the reading's office; sparks paid of ninety-six;
witnesses standing, of those found; the deed's state; and the one or two leads the player pinned.

Per person, once learned: name, ward, whether they remember her, what they said they wanted, their last known
schedule clue — not a live position — and their withdrawal reason if any.

Fully usable with typed chat and screen-reader-friendly text. Voice is expressive and never a critical
accessibility gate.

## Rumor and Night Office integration

Quest facts eligible to become bounded rumor tokens: a public claim about who held the room; a bribe, a forged
mark or a custody commit; the player contradicting their own carried words; a witness pledging or withdrawing in
public; the cell's money being seen.

Until generic propagation exists, M3 may use an authored quest fact queue that transfers only at fixed public
gatherings and the Night Office, designed for deletion.

The Night Office may settle a witnessed event into a Major's memory, expose one morning receipt, and carry the
hottest fact into a ward mood. It may not withdraw a witness at random or move a spark. Six of the people who
matter here are Major and get individual reflections; Renn Hobbe, Odo Trask, Cobb Hawser, Hamel and Idonea are
currently **Minor**, so mandatory quest-state reevaluation and dawn receipts must run deterministically **with
the Night Office disabled entirely**. Night cognition colours memory and performance only.

**Tier work before ship:** Renn Hobbe (`a9rnh`) and Odo Trask (`fo6gl`) are decisive named actors with
judgement to exercise — the priest reads or refuses, the notary enters or refuses — and both are Minor today.
Promote and deepen them, as the drainage quest requires for Aubin Marle. Do not quietly treat a Minor as a Major
quest subject.

## Law and custody integration

The quest creates opportunities for the existing law, not a parallel punishment system. Potential offences:
forging or scrubbing a civic mark; bribing a witness or a clerk; false statement to a notary; theft to close the
shortfall; informing falsely; trespass after curfew once the watch-witnessed dependency exists.

Arrest is fail-forward. The clock continues; the Stone House already holds eight seeded inmates and is a scene,
not a fail state; the posted fee is 3 sparks (`custody.rs:123`) and the committed line already tells the player
which bell they go at. Witnesses can visit. The reading proceeds in the player's absence — and being in a cell
on the sixth day is one of the authored ways to end up **rung, not heard**.

Note that surety does not exist as a mechanism: today it is a name the spec and the HUD give to talking a keeper
into `release` (`actions.rs:3657-3658`). The quest must not promise a surety record.

## Prompt surface

Only people the quest has actually reached receive the quest section, and only while it is active or recently
resolved. Never all six witnesses, never the ledger, never another person's private objection.

```text
**the_hawser_name**:
- reading: Marenstide, day 6, the Waning, at Saint Maren's
- what the stranger has asked you for: to stand and say the room was Hawser's
- what you remember: a woman two doors along, dead under the slate in F.415
- your objection, if you hold one: you cannot drown under slate
- what you have seen them do: paid Averil Skell's book; drew on a door at night
- your recorded answer: none
```

The model owns the wording, the trust, which authored condition it names, refusal, anger, mercy and gossip. The
sim owns who is a valid witness, what the deed requires, the arithmetic of the ninety-six, task and witness
receipts, kinship, withdrawal validity, the reading's timing and the outcome. The model may say no. It may not
invent a seventh witness, a different price, a second reading, an extension, or a deed.

## Outcomes

Applied at the reading; every one of them is a state the city then lives in.

- **Well rung.** The three held and the name was said in earshot. The lintel carries *Alis Hawser* until the
  next pauper; Renn Hobbe takes the argument in the porch as he does every year; the ward mood carries it, and
  the Majors who were inside 20 m each settle a memory of it overnight. Cobb Hawser's sheet gains the one fact
  he wanted.
- **Rung, room lost.** The bell rang and the deed went to the tenant. She has a name and nowhere it belongs;
  the fulling-master's ell-tally still says Rud, and it always will.
- **Rung by the wrong purse.** The Custody's or the cell's money paid. The name rang and became somebody's
  asset — a pass said at a door, or a line in a book the player cannot read — and Tilman Rue counted the faces
  at it, including the player's.
- **Rung to an empty yard.** Paid and read, but no witness stood and nobody the sim counts was inside 20 m. It
  is chalked and overwritten by the next burial; two days later the lintel is somebody else's.
- **Unrung.** The sixth day passed. The roll was read without her, the letter is still in the player's coat, and
  the quest can be offered again next year — which is the outcome that has occurred twenty-two times already.

### Integrity overlay

Orthogonal to the outcome, and never a score: promises kept and broken; whose sparks paid; bribes offered,
accepted and exposed; forged marks used and discovered; carried words delivered faithfully or altered; a man put
in the Stone House to clear a room. The overlay drives reactions, law and follow-up content. It never changes
the arithmetic after the reading.

## Failure matrix

| Failure | Immediate result | New route |
|---|---|---|
| Miss a person before their next leg | No conversation now. | Learn tomorrow's round, catch them at a public gathering, or send word by a messenger child. |
| A witness withdraws | The casebook names the broken condition. | Answer the objection, read her apart from the roll, or stand somebody else. |
| Short of ninety-six on day five | The sexton will not ring on credit. | A lender, dry money, the cell — or hand what you have and get an authored partial refusal that is not silence. |
| Deed refused for kinship | Cobb's word does not count. | Two non-kin householders of two wards; that is the whole puzzle. |
| Forgery discovered | A notice, and the ward overwrites the mark in two days. | Settle it, run it out, or take the summons. |
| Arrested | Offices pass while custody plays. | Fee, talk the keeper round, escape, or accept an outcome from a cell. |
| Miss the reading | The roll is read from the state recorded. | Live in the city the outcome made. |
| Provider or STT failure | Performance degrades. | Typed chat completes every critical path; casebook and receipts remain sufficient. |

## Data ownership

Content in data, rules in Rust.

```text
assets/world/knell_quest.json   # timing, witnesses, conditions, tasks, purses, strings
```

```text
crates/cathedral-sim/src/knell_quest.rs
src/smart_actors/quest_casebook.rs
src/smart_actors/quest_interaction.rs
```

The host loads and validates the catalog and passes plain values into the IO-free sim. Validate every actor,
place, item, office and condition reference at construction. With no catalog present, prompt and snapshot
behaviour must remain byte-identical to today. If a general quest module exists by then, use it instead of these
provisional paths.

## Engine and projection seams

New player commands: `AcceptQuest { quest_id }`, `PinQuestLead { lead_id }`,
`PlayerQuestTask { request_id, task_id, anchor_id, position_m, spatial_seq }`.

New NPC actions, all argument-validated against quest state:

```text
stand_witness   {"person":"...","condition_id":"..."}
withdraw_witness{"reason_id":"..."}
enter_deed      {"door_id":"...","person":"..."}      # notary only
ring_knell      {"name":"...","years":22}             # sexton only
```

`ring_knell` hands `BellPattern::NameKnell { years }` — which already exists — the dead woman's age through a
new `EngineMessage`, chalks a one-slot `LintelName` mark on the charnel door, and emits a percept carrying the
name. The bell, the mark and the percept are the whole of it; the judgement of pity, price and witness stays the
model's. Because the pattern already logs its own stroke count, the acceptance test for the finale is a grep for
thirty-four strokes.

Project a dedicated `KnellQuestView` into a Bevy `QuestCasebookState` on a quest-only revision. **Do not** put
the casebook inside the actor/item `PublicSnapshot` or bump the public revision per lead: that republishes the
full cast and the configured crowd, and the snapshot's ~160 KiB bound already has little headroom. The Bevy side
renders player-safe typed state and never re-derives validity. Hidden trust, private objections and unknown
receipts never enter the view.

## Milestones

Each milestone is independently playable and testable.

### M0 — Quest state and the six-day clock

- Validated catalog and `KnellQuestState` on the pure sim's `World`, behind an absent/default-off data gate.
- Offer, accept, decline, the six-day deadline, the reading window, idempotent phase transitions.
- The reading evaluates seeded fixtures and produces **Unrung** deterministically.
- Minimal snapshot and diagnostics; no LLM actions, no UI.
- Headless: accept, advance six days, observe the roll read without her.
- Quest-disabled prompts and snapshots remain byte-identical.

### M1 — One witness, one purse, one bell

- Nan only; one authored errand that pays; the ninety-six scaled down to a testable sum at a three-day scale.
- `stand_witness`, `ring_knell`, the `LintelName` mark and the 300 m percept.
- A mini-reading that rings or does not ring, with receipts.
- **This is the go/no-go test for the whole feature.** If asking one person to remember a dead name and stand up
  for it is not compelling at this scale, do not author the other five.

### M2 — The full errand

- All six witnesses with authored conditions and withdrawal reasons, including Wyn Alder's objection.
- The room, the tenant's counter-claim, `enter_deed` and the two-householder rule.
- All four purses; the errand set; the casebook and its accessible text controls.
- Deterministic fake-backend answers for every witness.
- Validate that at least three materially different routes reach **Well rung**.

### M3 — Consequence

- Night Office morning receipts; bounded rumor or the temporary authored queue.
- Law and custody routes: forgery, informing, arrest, and a route that reaches the reading from a cell.
- Withdrawal and refusal presentation; the integrity ledger.
- Correctness must hold with the Night Office disabled and every night reflection dropped.

### M4 — The reading

- The bounded reading presentation: the roll, the objection, the bell, the lintel, the name said.
- Five outcome packages and their aftermath, including Cobb's changed sheet fact.
- Time-jump tests across offices and days; the reading fires exactly once.
- The same world continues afterwards with no ending card.

### M5 — Persistence foundation

- Versioned engine/world checkpoint DTOs; the sim produces and consumes values, the host does atomic file IO.
- Clock, world, rounds, inventory, marks, law, custody, knowledge, rumor and quest state preserved together;
  in-flight cognition and speech deliberately discarded.
- Round-trip and version-rejection tests prove no receipt, witness pledge, deed or reading duplicates on load.
- Checkpoint before each explicit quest mutation and at every office boundary.

### M6 — Content, accessibility and ship

- Promote and deepen Renn Hobbe and Odo Trask; author every route's copy.
- Typed/voice parity and provider-failure fallback on every critical path.
- Audio and visual receipts: the knell, the lintel, the crier's count, the fair.
- Balance the errand economy for 4–6 hours; performance with the authored cast and a configured crowd.
- Headless, Bevy-host and one `CATHEDRAL_HEADLESS=1` drive-mode acceptance run.

## Acceptance criteria

### Deterministic sim

- A fixed seed and command transcript produce byte-stable quest state and reading result.
- Ninety-six paid rings; ninety-five does not. A withdrawn witness never counts. Kin never satisfies the deed.
- The reading fires exactly once at the authored game-time boundary even if the player is absent, in custody, or
  one pump crosses several offices.
- A failed LLM call cannot delete a receipt, a pledge, the deadline or the outcome.
- Stable ordering holds across witnesses, tasks, receipts, prompts and snapshots.
- The whole quest completes with `CATHEDRAL_FAKE_BACKEND=1` and with the Night Office off.

### Content

- Every witness has at least two authored conditions consistent with their sheet, and one authored reason to
  withdraw.
- At least three materially different routes reach **Well rung**, and at least one of them is unlawful.
- No route requires one exact spoken phrase or microphone recognition.
- No single item, task or NPC failure hard-locks the quest.
- The traditional families' objection is voiced in every outcome, and is never presented as wrong.
- No outcome is presented as the morally correct one; paying with the cell's money is a position, not a fail.

### Player comprehension

- After the two offer scenes, a first-time player can state the three things and the deadline.
- At any point before the reading, the casebook exposes at least one actionable learned lead.
- A withdrawn witness names the reason.
- The morning after the reading communicates what happened without prose alone: the lintel, the ward mood, the
  crier.

### Integration

- Typed chat completes every critical path without STT or TTS.
- Custody consumes time but never freezes the quest.
- Offers, items, marks, homes and rounds remain authoritative; quest code duplicates none of them.
- No Bevy-only state determines a witness, a deed or an outcome.
- Checkpoint round trips neither duplicate nor lose a receipt.

## Vertical-slice acceptance scenario

Three game days, one witness, one purse, a reduced sum:

1. Cobb offers the letter at Tanners' Slip; Noll Fitch states the three terms, and the stroke-a-year rule, at the charnel door.
2. The player finds Nan on her round and says "Alis Hawser" inside 20 m; her turn produces a memory.
3. Nan names what would move her: small ale bought in public, and not being made a fool of.
4. One carried-words errand and one bucket errand pay coded sparks on receipt.
5. The player hands the reduced sum to Fitch, with provenance recorded.
6. Nan pledges to stand; the pledge survives a night and the Night Office.
7. At the reading, the bell rings, the lintel is chalked and the player says the name with three people inside
   20 m.
8. The next morning, a ward mood and one Major's memory carry it; the lintel still shows it.
9. The same run, replayed with the sum one spark short, ends **Unrung** — and the difference is legible.

If step 2 to step 6 is not compelling at one witness, do not author the other five.

## Risks and decisions still required

1. **Does the model remember a dead name?** The whole witness track assumes that saying "Alis Hawser" within
   20 m of a seeded sheet reliably surfaces the seeded memory. If it does not, the seeded memory must be
   triggered by a code-side percept on the name rather than by the model's recall. Test this in M1 before
   anything else.
2. **Earning is a dependency, not a feature.** The quest owns four authored errands. If they start growing into
   a wage system, stop and build the base-game seam instead.
3. **Ninety-six may be the wrong number.** It is canon as a price, but the errand economy is invented. Balance
   the errands to the sum, never the sum to the errands — the ninety-six is the one number the lore already
   knows. The one wage the corpus does state is Tam Rud's thirty-six sparks a day as a journeyman fuller, which
   makes ninety-six under three days of skilled work and roughly ten of a stranger's scraps. That gap is the
   quest, and it is also the reason a player who already has a trade in some later version of the game should
   be given a harder version of the errand, not a shorter one.
4. **Wyn Alder's objection must not read as a bug.** A witness who withdraws for a principled reason is the best
   scene in the quest and the easiest one to mistake for a broken flag. It needs a named reason in the casebook
   and a line she will repeat when asked.
5. **Kin discount is a rule the player must learn early.** Cobb asking a stranger is the tutorial for it; if
   playtesters still try to use Cobb as the witness on day five, put it in Fitch's terms more bluntly.
6. **Tier promotion is real work.** Renn Hobbe and Odo Trask are Minor and decisive. Budget it.
7. **Persistence.** Six game days is too long to ship without base save/load. Keep quest state serializable and
   treat disk persistence as a hard ship dependency.
8. **Speech occlusion.** Hearing ignores walls today. Do not build a critical overheard-in-a-room beat until
   perception gains occlusion; use the porch, the green and the charnel door.
9. **One bell, one city.** The knell carries 300 m. Anyone outside that hears nothing, which is correct and must
   not be quietly widened to make the finale feel bigger.

## Open design questions

- Should the parish's price be waivable by Renn Hobbe's own judgement — mercy as a fifth purse — or does that
  dissolve the quest's only hard number?
- May a witness's condition refer to another witness's public pledge, or only to world facts?
- Is reading her **apart from** the roll of the drowned a lesser ending or an equal one? The corpus is neutral;
  the quest should probably be too, and that is a decision, not an omission.
- Does the deed need the room's door to be a real `nav::Door` with a homes entry, or is a place id enough?
- What happens to the lintel name at the next pauper's burial — is being overwritten in two days the point, or
  a disappointment the player will read as a bug?
- Should Cobb be promoted to Major so the person who asked can react to the ending in his own words?

## Source references

- `lore/the_dry_boatmen.md:354-388` — Marenstide, the roll of the drowned, the name-knells denied after F.415,
  *you cannot drown under slate*, Renn Hobbe, the Dry Race.
- `lore/families/family_rud.md` — the naming law, the Hawsers landed, the Ruds kept alive by other people's
  paperwork, the fulling-master's ell-tally.
- `lore/the_great_rains_and_the_hammering.md` — F.415, the Hammering, the twenty-two years since, and the
  cause left unestablished.
- `lore/core_lore/calendar_and_history.md:31` — Marenstide, late summer, remembrance of the drowned.
- `lore/second_sun/11_glossary_and_naming.md:120`, `07_what_everyone_knows.md:38` — the paupers' ninety-six.
- `lore/characters/funerary_worker/ax5nf_noll_fitch.json` — the sexton, Maren Smallvoice, the charnel-door
  lintel.
- `lore/characters/court_officer/fo6gl_odo_trask.json` — the notary and the open fee book.
- `features/implemented/law_and_order.md` — notices, custody, the Stone House, surety's absence.
- `features/implemented/chalking_the_walls.md` — marks, forgery, scrubbing, the two-day ward cross.
- `features/implemented/food_and_items/` — the spark standard, offers, stacks.
- `features/rumors.md` — the preferred bounded propagation for the day-five turn.
- `features/quest_secure_votes_for_a_drainage_funding_plan_before_the_rain/README.md` — the sibling quest, and
  the spec shape this one follows.
- `crates/cathedral-sim/AGENTS.md` — the sim boundary, prompts, actions and scheduling.

## Companion quest overview

The six-page visual overview lives beside this spec:

- `one_bell_is_not_a_name_quest_overview.docx`
- `one_bell_is_not_a_name_quest_overview.pdf`
- `generate_quest_overview.py`, `quest_overview_figures/`

The overview communicates the player experience. This markdown file is authoritative for feature scope and
implementation.
