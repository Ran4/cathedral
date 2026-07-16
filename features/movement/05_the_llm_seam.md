# L4 — The Mind: where the LLM touches the body

Two touchpoints. That is all, and the restraint is the point.

---

## 1. The invariant that makes it safe

> **The body never blocks on the mind.**

Lifted from seagame, where it is structural rather than stated. `updateIdle`
(`~/seagame/src/crew/update.ts:1751-1772`) tries the command queue first; if the queue is empty, the
ladder runs. So an order that is slow, that fails, or that never comes **costs nothing at all** —
nobody was waiting for it. The crew member just keeps living.

Everything below is built on that. `go_to` is a *suggestion layered on an already-autonomous agent*,
not the agent's brain. If cognition is down (no API key — which the capability probe already treats as
a first-class state), the city still gets up in the morning, walks to the well, works, and goes to
bed. It just does it in silence.

That is also the honest answer to the brief's *"ideally an LLM would control all thinking, but that's
going to be incredibly expensive."* It would not merely be expensive. It would be **fragile**: 500
bodies that stop moving the moment a provider hiccups.

---

## 2. Touchpoint one: `go_to`

### The prompt currently forbids it, in so many words

`assets/prompts/turn.j2`:

> Use ONLY the verbs listed below, spelled exactly as shown (lowercase English). There are no other
> verbs: **if what you want to do has no verb here (like walking somewhere), express it in speech with
> `say` instead of inventing a verb.**

Somebody wrote *"like walking somewhere"* as the canonical example of a thing the model cannot do.
That paragraph is the first thing this feature deletes.

### The verb

```
go_to {"place_id": "pl_x2vw"}       # set off for a place you know (an entry in places_you_know)
go_to {"person": "sv3n1"}           # set off toward someone, and keep after them while they move
stop {}                             # abandon it; go back to your own business
```

### The semantics, which are the whole design

**`go_to` sets an intent. It does not move anyone.** It returns immediately —
`Ok("Osanne Vell sets off for the Wickmarket")` — and the body carries it out over the next seconds or
minutes, through the ladder's rung 0.

**It expires.** Default ten game minutes. An LLM that says "go to the market" does not own that NPC
for the rest of the day; when the intent lapses, the Round resumes. This is the single most important
line in this document, because without it one confused reply strands a character permanently.

**It can be refused.** A starving man told to walk half a mile does not go. The action fails with a
real `ActionError`, and the model gets the existing `system:` line —

```
system: your action "go_to {'place_id': 'pl_9q2v'}" failed: too far — you are starving
and the wharves are half a mile beyond the River Gate
```

— which is the same self-correction channel every other failed action already uses
(`scheduler.rs:556-574`). The model can argue about it next turn. Three lines of code; a cast instead
of a set of puppets.

**Refusal reasons must stay few, physical, and obvious.** Two of the three error codes are not really
refusal at all — `unknown_place` and `no_route` are validation against hallucinated ids and unreachable
places, needed under any design. Only `too_far` is the body saying no, and it earns its place for one
reason: needs preempt intents anyway, so the alternative to refusing upfront is accepting the intent,
returning `Ok("sets off for the wharves")`, and then silently abandoning it — after which the LLM
*believes* it is headed to the wharves and may later narrate an arrival that never happened. The
refusal keeps the mind's picture of the world truthful, on a channel that already exists.

The temptation to resist is adding more "realistic" refusal conditions — tired, curfew, weather, fear
of a ward. Each one is code making a *willpower* judgment on behalf of a character. A starving man who
drags himself half a mile to see his dying brother is exactly the drama the LLM might reach for, and
there is no push-through verb — arguing next turn does not get him there. That trade is acceptable for
one extreme, mechanical threshold (a real starvation state plus a real distance); it is not acceptable
as a personality system in disguise. Ship `too_far` gated on something simple and defensible, and do
not add a second refusal condition until one proves necessary in play.

**Arrival is a percept.** When they get there, an inbox line: *"You have arrived at the Wickmarket."*
Which is news, which gives them a turn if they are on stage, which means **the NPC narrates their own
arrival** without anybody scripting it. Elegant, and free.

**The model never sees coordinates.** `place_id` takes an **opaque handle** from `places_you_know`
(§3) — the same mental model the sheet already uses for people (`sv3n1`): things you can act on are
handles you were given. The model picks a *place*, like a person would. It does not pick a point, it
cannot pick an unreachable one, and the failure modes collapse to three: `unknown_place`, `no_route`,
`too_far`.

### What it touches

Adding a verb is a well-worn path in this codebase — `actions.rs:70-96` is a `match` on a `&str`, and
there is no `Action` enum to extend:

| File | Change |
|---|---|
| `actions.rs` | two `match` arms; two `fn`s; reuse `args_object` / `parse_actor_id` |
| `error.rs` | `ActionErrorCode::{UnknownPlace, NoRoute, TooFar}` + the existing `From` into `CommandErrorCode` |
| `assets/prompts/turn.j2` | delete the "no walking verb" paragraph; add two example lines |
| `prompt/mod.rs` | the sheet gains `places_you_know` (below) |
| `tests/fixtures/prompts/*` | **all 20 golden fixtures regenerate** |

---

## 3. The sheet: three additions, and 20 golden fixtures

`crates/cathedral-sim/tests/golden_prompts.rs` byte-diffs 20 rendered prompts against frozen fixtures.
They are *"the last independent witness that the prompt still says what it said"* (AGENTS.md) — so
changing the sheet is deliberate work, not incidental. Budget for it, and change the sheet **once**,
not four times.

```json
"you_are": {
    "location_description": "The Wickmarket",
    "the_hour": "Lamplight — the lamps are being lit; the market is closing",
    "position_m": { "x": -37.75, "y": 0.91, "z": 379.3 }
},
"places_you_know": [
    { "place_id": "pl_x2vw", "name": "The Wickmarket" },
    { "place_id": "pl_7f3k", "name": "Coswald's Yard" },
    { "place_id": "pl_qq81", "name": "Reed Ward" },
    { "place_id": "pl_m4rd", "name": "Ford Well" }
],
"you_see": {
    "description": "People within 20 metres, nearest first",
    "people": [
        { "id": "sv3n1", "name": "Sven",  "distance_m": 2.7, "moving": false },
        { "id": "k0fb1", "name": "(unknown - you don't know the name of this person)",
          "distance_m": 11.4, "moving": true }
    ]
}
```

**`the_hour`** — the clock, as a *field*. Not a percept. §5 below explains why that one decision is
worth thousands of tokens a day.

**`places_you_know`** — the wayfinding whitelist: you cannot `go_to` a place you have not been handed
a `place_id` for. Each entry is a pair, the same shape `you_see` already uses for people.

*Two kinds of knowledge, deliberately split.* **Knowledge-of** is lore — "Tam Rud lives somewhere in
Reed Ward" — free text in the prompt, injectable as liberally as we like (and we intend to inject a
lot of it), granting no ability. **Wayfinding** is possession of a `place_id` in this list, and only
ids can be walked to. The gap between the two is gameplay, not a limitation: an NPC told *"you'll find
what you seek at Tam Rud's house"* knows *of* the place, walks to Reed Ward — coarse destinations
(the wards, the five squares) are ids **everyone** holds, so getting somewhere always has a legal
first step — and asks around until someone who holds the id shares it. The player can only ever
confer knowledge-of, never an id: player speech is free text, and grounding "Tam Rud's house" from it
would reopen the guessing problem. Telling an NPC where to find something sends them on a journey; it
does not teleport the knowledge. That is a feature.

*The ids are opaque, and the key is `place_id`.* Opaque because the whitelist alone only makes
guessing **fail** — with semantic ids the model will still *try*, deriving `tam_ruds_house` from lore
text and burning turns on `unknown_place` errors. An id it cannot construct is an id it does not
attempt, which redirects the model from guessing the API to acting in the world (walk there, ask
someone). We are steering behavior, not just validating input. The key is `place_id` rather than `id`
so a place handle can never be conflated with a person handle even out of context, and the `pl_`
prefix on the value gives the same distinction at the shape level. Keep the ids short — long opaque
strings invite copy errors.

*The list is assembled dynamically, per actor:* home, workplace, the legs of their Round, the places
of their own ward, the five squares and the wards themselves, the homes of people they know — plus
anything later shared with them. A per-actor set in `CharacterState`, grown by sim events, rendered at
prompt time. Not all seventy places. It is also, quietly, a *characterisation* device — a Reed Ward
boatman who does not hold the id for the glaziers' guildhall is a Reed Ward boatman, and *which* ids
someone holds is who they are.

*The transfer mechanic — a verb, deferred past M5.* Directions travel as an ordinary action beside
the speech. A turn is already *"Take one or more actions"* (`turn.j2:116`), so the verb costs no
extra turn and duplicates nothing:

```
say      {"text": "Tam Rud? The corner house past the Ford Well, ask for the blue door."}
tell_way {"person": "ask3r", "place_id": "pl_7f3k"}
```

The rules:

- **The receiving LLM stores nothing (regarding the place_id), ever.** Storage of the place_id is sim state:
  the verb writes the id into the target's `places_you_know` set in `CharacterState`,
  and the sheet *is* the model's memory — at the
  next render the place is simply there. One inbox line ("Betriss told you the way to Tam Rud's
  house" — the name from the world registry, not from anyone's prose) makes the arrival of knowledge
  narratable news.
- **The speaker must hold the id**, and the target must be in earshot (the existing 20 m hearing
  rule). Violations are ordinary `ActionError`s on the standard `system:` self-correction channel.
- **Targeted, not broadcast.** Sharing is deliberate; eavesdroppers learn nothing. Knowledge keeps
  its friction, which is the taste of this whole document.
- **Nothing parses anybody's sentences.** The speech and the verb are independent channels: a
  helpful answer with no verb is just vague directions, and a verb with no speech is a silent point
  of the finger. Both are fine.

The precedent check: names deliberately have *no* introduction verb — "characters introduce
themselves in speech" (`crates/cathedral-sim/AGENTS.md`), and listeners keep names as free-text
memories. But a name's payoff is narrative, and a memory line carries it. A place id's payoff is
mechanical — `go_to` needs it in a structured set — so it needs what names never did. Different
requirement, different mechanism.

*Off the stage, the same conversation runs — on the lane that was never gated.* There is no seek
verb, no registry name-matching, and no parallel code mechanic: the ask-around is `say` + `tell_way`
wherever it happens, because the machinery already exists. The stage gate only gates the round-robin
*idle* lane; an addressed `say` (or an audible sound) puts its recipient in the **priority slot**,
which is never gated, anywhere in the city (`attention.rs`' three lanes), and hearing already
computes its recipients globally. So off stage: the asker arrives in the Reed, asks someone by name;
the addressed say nudges the answerer — a real turn, off stage, with today's scheduler; the answerer
replies with `say` + `tell_way`; the reply nudges the asker, who now holds the id and sets off. The
errand itself is carried in the asker's own memories and goal ("find out where Tam Rud lives" — free
text, consumed only by the LLM that wrote it), so **the sim never needs to know what is being
sought**. That is the piece a `seek {"place_name": ...}` verb was sketched to provide, and with it
gone, so is the verb — along with the registry alias table and the name-matching heuristics it would
have needed. One path, watched or not.

The one missing link: today the arrival percept produces a turn *only on stage* — an asker reaching
the ward alone would stand mute until the intent expired. Arrival from a `go_to` must grant the same
priority nudge an addressed say does. That is the whole change, and it is bounded by construction:
arrivals only happen because a `go_to` was issued, `go_to`s are rare and LLM-initiated, and the nudge
is one turn, not a lease on the scheduler.

(The priority lane used to be a single last-write-wins slot, which would have made off-stage chains
best-effort: any other addressed say landing mid-errand silently erased a link, and off stage there
is no idle rotation to recover the dropped actor. Fixed — handoffs are now a de-duplicated FIFO like
the player-reaction lane, `scheduler.rs::priority_handoffs`.)

The cost is a handful of provider calls per errand, spent where nobody is looking — acceptable
because errands are rare and born only from conversation. §6's acceptance test survives untouched:
with cognition off, errands cannot start, but the city never depended on them; the Round runs
regardless.

M5 ships the static + dynamic assembly only. But the id *format* must be right now, because it
touches the sheet — and the sheet changes once.

**`moving`** — one bool per visible person. Once people walk, `you_see` churns, and the model needs to
tell *"a man is crossing the square"* from *"a man has stopped in front of me."* One bool, and it makes
the difference between an NPC who greets everyone who passes and one who greets people who stop.

---

## 4. Touchpoint two: the Night Office

The brief:

> *Maybe a daily "sleep" where we try to find new goals, summarize what we've learned during the day,
> or whatever. Not relevant for the ambient characters (they might have one huge llm prompt that
> updates them all once per day perhaps?).*

That instinct is right, and the constraint that makes it right is in
`features/quicker_response_improvements.md`:

> The protected reaction queue prevents player speech from being overwritten, but **cognition still
> has one global in-flight request.** A background actor that began thinking just before the
> transcript lands can add the remainder of its provider call to foreground latency.

**One in-flight request.** A nightly reflection over 500 NPCs, run through the ordinary scheduler at
its 1 s minimum delay, is *eight minutes of exclusive scheduler time* during which the player cannot
be answered. It would be the worst-feeling bug in the game and it would be entirely self-inflicted.

So:

| tier | n | Night Office | calls / game day |
|---|---|---|---|
| **Major** | 31 | individual reflection, at their own bedtime, staggered across the game-night | **31** |
| **Minor** | 120 | **batched by ward** — one prompt per ward per night, carrying that ward's day | **8** |
| **Ambient** | 350 | **none.** Round re-rolled in code from occupation + a per-actor seed | **0** |
| | | | **39** |

Thirty-nine calls per game day. At the default clock (1 game day = 1 real hour) that is 39 an hour,
trickled through the hours when the player is most likely to be somewhere quiet.

### The lane

It must not touch the scheduler's single in-flight slot during play. A second cognition lane, with
three rules:

1. **One in flight.** Never more.
2. **Yields absolutely.** Never submits while `floor_busy`, while `player_composing`, or while anyone
   is on stage with the player. The player's protected reaction FIFO is *untouchable*, and the Night
   Office must not so much as queue behind it.
3. **Drops silently.** If the night ends before everyone has reflected, the rest keep yesterday's
   Round. **A missed Night Office is not an error.** Nothing waits for it, nothing retries, nobody
   notices.

Bedtime staggers itself: the Round says when each character sleeps, and they do not all sleep at once.
The lane fills naturally across three game hours.

### What it returns

The same verbs it already knows, plus one:

```
remember {"memory": "The grey coats searched the Reach on the Waning. Corin knew before I did."}
forget   {"memory": "I must ask Betriss about the chalk"}
set_goal {"goal": "Find out who told the Custody about the Reach"}
set_round {"leg": 2, "place": "gaunt_passage"}    # NEW: edit one leg of tomorrow
```

`set_round` edits **one leg**, not the whole day. A character who decides to take their meal at the
Hungry Ox instead of at home changes one line, and tomorrow they walk somewhere different — and the
player, who has no idea why, sees a person who has changed their habits. That is the entire payoff of
this touchpoint, and it is worth the 31 calls.

### The ward prompt, for the Minors

One prompt per ward per night. It carries: what happened in that ward today (the transcript, filtered
by area), who is in it, and what the ward is worried about. It returns a few sentences of *mood* — which
becomes a shared context string on every Minor in that ward — and, optionally, a handful of `set_round`
edits naming specific characters.

This also happens to be the natural home for `features/lore_ward_politics.md`, which proposes exactly
this unit of political identity, and it is why I would build it as *ward*-batched rather than
`significance`-batched.

---

## 5. The three ways this blows the budget

### 5.1 The novelty gate — **a prerequisite, not a follow-up**

`attention.rs::context_hash` decides whether an on-stage actor gets an idle turn by hashing the set of
**ids within 20 m**. Its own doc comment (`attention.rs:390-397`) explains that *positions* were
rejected as a key because *"a neighbour's every step would otherwise be news."*

Movement re-opens that wound through the id set. Once people walk, 20 m membership churns constantly,
`require_news` is satisfied nearly always, the round-robin stops skipping, and the scheduler runs at
its hard ceiling — one turn per second, ~3,600 an hour — for as long as the game is open. Today,
standing alone in a field costs **zero**.

The fix is one sentence of semantics:

> **A man crossing the square does not make you think. A man who stops in front of you does.**

Count an actor in `context_hash` only if they are **settled** — speed below ~0.15 m/s, or continuously
within 20 m for ≥ 3 s. Passers-by never enter the hash. Someone who *stops* near you is genuinely
news.

It is a small change to one pure function, it is *better* than the current rule independent of
movement, and **it must land before the `spatial_update` guard is lifted.** If it does not, the first
build with walking NPCs will quietly multiply the token bill by a large number and it will not be
obvious why.

### 5.2 The bell — solved, in the lore, before we got here

`lore/second_sun/design/06_the_sound_of_the_city.md` §5:

> **The offices are a clock, not events.** Seven percepts per actor per day would be token waste:
> Evenblow instead updates the **scene-header time-of-day** every actor already receives (*"the last
> office rung was the Waning"*). **Only *deviations* from the daily round are events.**

`town_bell` is audible at **600 m** (`catalog.toml:38`) — most of the city. Emitted as a sound percept,
seven offices × ~500 recipients = **3,500 inbox lines per game day**, forever. So:

- the office goes in **`you_are.the_hour`** (§3) — a field, costing nothing, queueing nothing;
- the bell still *sounds*, from the Lanthorn, seven ordinals at 3 s intervals, for the player's ears;
- **no percept, no nudge, no inbox line.**

Deviations remain events, and should: the Ruin (the ring rung backward — fire or flood), the
name-knell, the Scold's summons before a proclamation.

**Verified, and reassuring:** a sound nudges exactly *one* actor into the priority lane, not all of
them — `engine.rs:1314-1327`, *"Exactly one nudge per sound: the turn stream is global and single."* So
even the naive version would not have caused 500 turns. It would have caused 500 inbox lines. Which
brings us to:

### 5.3 The inbox is unbounded — a real bug, which movement would expose

`CharacterState::inbox` is a plain `Vec<String>` (`character.rs:87`). Only `recent_history` is capped
(at `RECENT_HISTORY_MAX_ENTRIES = 32`). The inbox drains when a prompt is rendered — and under the
stage gate, **an ambient NPC in a far ward may never be prompted at all.** Their inbox grows for the
whole session.

It is latent today because so little happens. Movement makes things happen. **Bound it** (drop the
oldest, keep the newest N) while you are in here. It is a five-line fix to a bug that is already
present.

---

## 6. What the LLM still does not get

Stated positively, because the restraint is the design:

- **It does not pick coordinates.** It picks places — opaque `place_id` handles it has been given.
- **It does not plan a day.** It may edit one leg of one, once a night.
- **It does not steer.** Avoidance, corridors, the Needle's pinch — all code.
- **It does not decide when to eat, drink, or sleep.** Needs decide that, and needs preempt it.
- **It does not know it is being scheduled.** `significance` never enters the prompt today
  (`lore.rs:23-25`) and must not start now.
- **It is never load-bearing.** Turn cognition off entirely — `fake_backend: true`, or just pull the
  API key — and the city still gets up, walks to the well, works, and goes to bed at the Snuffing.

That last one is the acceptance test for this whole document. **If the city stops moving when the LLM
stops answering, the layering is wrong.**
