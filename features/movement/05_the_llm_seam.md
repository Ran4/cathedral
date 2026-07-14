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
go_to {"place": "wickmarket"}       # set off for a named place
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
system: your action "go_to {'place': 'outer_wharves'}" failed: too far — you are starving
and the wharves are half a mile beyond the River Gate
```

— which is the same self-correction channel every other failed action already uses
(`scheduler.rs:556-574`). The model can argue about it next turn. Three lines of code; a cast instead
of a set of puppets.

**Arrival is a percept.** When they get there, an inbox line: *"You have arrived at the Wickmarket."*
Which is news, which gives them a turn if they are on stage, which means **the NPC narrates their own
arrival** without anybody scripting it. Elegant, and free.

**The model never sees coordinates.** `place` takes an **area id** — the vocabulary the sheet already
speaks, since `location_description` has always rendered area labels. The model picks a *place*, like
a person would. It does not pick a point, it cannot pick an unreachable one, and the failure modes
collapse to three: `unknown_place`, `no_route`, `too_far`.

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
"places_you_know": ["wickmarket", "coswalds_yard", "the_gradine", "ford_well",
                    "the_lanthorn", "cinder_row", "the_needle"],
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

**`places_you_know`** — you cannot `go_to` an id you have never been given. This is the smallest list
that makes the verb usable: the places in this character's own ward, plus the five squares, plus
anywhere their Round takes them. Not all seventy. It is also, quietly, a *characterisation* device —
a Reed Ward boatman who does not know the id for the glaziers' guildhall is a Reed Ward boatman.

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

- **It does not pick coordinates.** It picks places.
- **It does not plan a day.** It may edit one leg of one, once a night.
- **It does not steer.** Avoidance, corridors, the Needle's pinch — all code.
- **It does not decide when to eat, drink, or sleep.** Needs decide that, and needs preempt it.
- **It does not know it is being scheduled.** `significance` never enters the prompt today
  (`lore.rs:23-25`) and must not start now.
- **It is never load-bearing.** Turn cognition off entirely — `fake_backend: true`, or just pull the
  API key — and the city still gets up, walks to the well, works, and goes to bed at the Snuffing.

That last one is the acceptance test for this whole document. **If the city stops moving when the LLM
stops answering, the layering is wrong.**
