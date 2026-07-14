# Gate idle NPC cognition on novelty (and on character)

Status: **§1 implemented** (2026-07-14) — the novelty gate, which is the whole
cost win. §2 (curiosity) and the boredom timer are **not** written; see *What is
left* below. Successor to `features/implemented/gate_idle_cognition_on_proximity.md`.

`attention.rs::Novelty` is the gate; the engine composes it with the stage once
per poll (D20) and stamps it from `NpcScheduler::take_submitted`;
`config.ron: smart_actors.idle_cognition.require_news` is the switch.

Measured in the running game (fake backend, muted microphone), standing still at
the player's spawn for 90 s:

| | prompts | what they were |
| --- | --- | --- |
| `require_news: false` (before) | 90 | the same three people, re-asked every second, answering `wait` |
| `require_news: true` (after) | **3** | one each, on arrival, then silence |

**−96.7%**, against a ≥90% target. Every one of the three surviving prompts
carries `since_your_last_turn: ["nothing"]` — the arrival round the acceptance
criteria ask for, and nothing else. The same A/B headless:
`--stage -t 8` spends all eight ticks re-asking Conny/Ilse/Sven in rotation;
`--news -t 8` spends three and then stops, with *"nobody has anything to react
to"*.

An earlier measurement of the same runs read 19 prompts, not 3. The extra 16
were real: this machine's microphone was open, ambient room noise was being
recorded as an utterance every 15 s, and `fake_backend` transcribes any
utterance as *"What's your name?"* — so a phantom player really was talking to
the cast, and they really did have news. The gate was right and the measurement
was wrong. Worth knowing before trusting a fake-mode prompt count on a machine
with a live mic.

## Two deviations from the design below

1. **The memory lapses (`NOVELTY_MEMORY_SECONDS`, 60 s).** The design implies a
   permanent per-actor hash. A permanent one never re-greets: walk away, come
   back an hour later, and the street you last thought about is the street you
   see now, so nothing has changed and nobody looks up — the city would freeze
   behind you for good. Forgetting the moment an actor leaves the stage is the
   obvious fix and is *worse*: `max_actors` (6) means the identity of the
   nearest six churns as you move through a crowd, so every actor bumped off the
   cap and back would buy a fresh turn, and the gate would leak exactly the bill
   it removes. So the memory is refreshed for every on-stage actor each poll and
   lapses on **absence, not silence** — a quiet neighbour is never forgotten
   (`a_quiet_neighbour_is_never_forgotten`), a player who actually left is
   (`walking_away_and_coming_back_is_news_again`).

2. **The inbox is not hashed.** The design lists it as one of three signals. It
   needs no memory of its own: a turn is what *drains* it, so a non-empty inbox
   is by construction something the actor has not been shown. This is what makes
   the stamp safe at submit-time — a line that lands during the two seconds a
   prompt is in flight survives to be shown on the next turn, where hashing the
   inbox at completion would have swallowed it.

## What is left

- **The boredom timer** (work order §2). Not written. Nothing in the city
  initiates *ex nihilo* now: a fishmonger cannot decide to cry his wares into a
  quiet street, because nothing has changed for him. The known cost the design
  names, unchanged and still unbought.
- **Curiosity** (work order §3, "who speaks first is a fact about the
  character"). Not written. All six neighbours still greet you on arrival — one
  turn each now rather than a rotation, which makes it affordable but no less
  silly. This is the remaining *silliness* half of the feature; the *cost* half
  is done.

## Goal

Stop paying for silence.

An NPC standing beside the player with nothing new to react to should cost
nothing. Today they are asked, every rotation, whether anything has changed;
they answer `wait {}`; and we pay ~2.2k input tokens for that answer. Then we
ask again.

Second, smaller goal: it should not be *every* NPC who greets you when you walk
past. Who opens their mouth first is a fact about the character, not about the
scheduler.

## Background: what proximity gating did and did not fix

The proximity gate removed the LLM turns spent on a city the player cannot see.
Alone in a field the cast now costs nothing, which was the headline number and it
holds.

But it did **not** reduce the turn *rate*. The scheduler still allows exactly one
request in flight, with `npc_turn_delay_seconds` (1.0 s) plus ~2.2 s of provider
latency, so the cast takes a turn roughly every 3.2 s — about 19 a minute —
exactly as it always did. What changed is *where those turns land*. Before, they
were spread across a 240-slot rotation over a 1.2 km city and nearly every one
happened out of earshot. Now all of them go to the ≤ `max_actors` (6) people
within `radius_m` (32 m) of the player.

The firehose did not shrink. It got aimed at the player's head.

Two consequences, both observed in play:

1. **Cost.** Standing still in a market still costs ~1,100 calls and ~2.5M input
   tokens an hour, and most of those calls come back `wait {}`. A `wait` turn is
   the worst trade in the system: full prompt in, five tokens out, no world
   change, and — because nothing changed — the same question gets asked again
   next rotation. It is a loop that pays to be told nothing happened.
2. **Silliness.** Every idle turn is now, by construction, taken by somebody who
   can see the player. A stranger appearing in `you_see` is a reasonable thing
   for a character to remark on *once*. It is not reasonable for six people to do
   it every time you walk down the street.

`turn.j2` is not the problem. It already says *"Do not manufacture conversation
merely because it is your turn. Use `wait {}` alone whenever there is nothing
useful and socially appropriate for you to do."* The cast is obeying. The waste
is that we asked at all.

The slogan the proximity feature shipped under — *"an NPC thinks because you are
near them"* — quietly conflated two things. Being near someone justifies keeping
them **simulated**. It does not justify them **addressing you**, and it certainly
does not justify re-asking them every three seconds whether they have changed
their mind about saying nothing.

## Design

Three changes, in descending order of value. The first is the one that matters;
the third is independent of the other two and is written up separately (see
*Related* below).

### 1. The novelty gate — an idle turn requires news

Today an on-stage actor is idle-eligible. Make them idle-eligible only if
**something has changed for them since their last turn**. This is a cheap, pure,
non-LLM test; no provider call is needed to know that nothing has happened.

The signals that constitute news, roughly:

- their `inbox` is non-empty — somebody spoke near them, a sound reached them, a
  system line landed;
- the **set of actor ids they can see** changed — somebody arrived or left,
  notably the player;
- the offers involving them changed (`you_offer` / `offered_to_you`).

Everything else — the player shuffling a metre, the clock advancing, their own
`wait` — is not news.

**Hash the set of ids, not positions.** If novelty keys on coordinates, the
player breathing on the spot re-fires the whole stage and the feature buys
nothing.

**Why the loop terminates.** An NPC's `wait` emits no domain event and does not
bump the world revision — there is an existing test that pins exactly this
(`actions.rs`, "waiting is not a world event"). So a `wait` creates no percept
for anybody. That is the property the whole design rests on, and it is already
true.

Trace the case that is annoying today. The player walks up: his id enters the
visible set of the six people around him, so each is eligible for **one** turn.
Suppose one of them greets him. That utterance lands in the other NPCs' inboxes,
so they become eligible, and one may answer. That answer lands back in the
first's inbox. A conversation runs — entirely on real percepts, at the existing
turn rate, with the existing `wait` discipline killing the lines that would only
repeat. Then it dies: everybody waits, nobody speaks, no inbox fills, no visible
set changes, no hash moves. **And the cost falls to zero and stays there** until
the player speaks, walks somewhere new, or a bell rings.

Five minutes standing in a market goes from ~95 turns to something like 6–10.
The thing that was true in theory — *keep the LLM going, they will just be quiet
most of the time* — becomes true in the bill, because quiet stops being rented.

Where the eligibility state lives is an implementation question, but it should
**not** go into `World`: it is derived bookkeeping, not world state. A
`BTreeMap<ActorId, u64>` of last-turn context hashes next to the stage in
`attention.rs`, recomputed once per poll where `on_stage` already runs (D20), is
the obvious home.

**Known cost of this.** An NPC can no longer initiate *ex nihilo*. A fishmonger
cannot spontaneously decide to cry his wares into a quiet street, because nothing
has changed for him. Given that the city outside the stage is already frozen
solid, this takes away nothing the game currently has. If spontaneity is wanted
back, a slow **boredom timer** — eligible again after N minutes regardless of
news — buys it at a price that is explicitly configured rather than accidental.
Note it also interacts with goals: an NPC whose `current_goal` implies action
("buy fish") cannot act on it without a percept, and the boredom timer is the
escape hatch for that too. Start with the timer off and see whether the world
feels dead without it.

### 2. Curiosity — who speaks first is a fact about the character

Personality belongs on the question *"do we spend a turn on them at all?"*, not
on *"what do they say once we have?"*. Hanging it there makes it a cost lever and
a character lever with one mechanism.

It composes exactly with the lanes already in place:

| Lane | Curiosity applies? |
| --- | --- |
| player reaction (the player spoke) | **no** — ungated, as now |
| priority (addressed `say`, sound nudge) | **no** — ungated, as now |
| idle (on stage, has news) | **yes** |

So curiosity governs **unprompted initiative only**. Speak to the haughtiest
magistrate in the city and he answers you immediately, exactly as he does today.
He simply never opens his mouth first. That is not a scheduling hack; that is the
character.

Mechanically it should weight the idle rotation (`stage_turn_order`) and/or set a
per-NPC idle cooldown: a curious child is eligible often, an uppity canon rarely
or never. Random kids strike up conversations; rank does not.

Where the number comes from — the open question, and worth deciding deliberately:

- **Authored.** An optional `curiosity` in `lore/characters/**/*.json`. Matches
  the codebase's "data, not code" rule, where that directory is already the
  source of truth for significance and status. Honest, and 500 files need not be
  touched at once if there is a default.
- **Derived.** From metadata the `LoreProfile` already carries — `age` (children),
  `rank` / `title` / `statuses` (the uppity), `occupation_id` (a hawker or a
  beggar is professionally curious; a guard or a canon is not). Costs no
  re-authoring, but it is a heuristic and will feel arbitrary at the edges.

Recommended: derived default, authored override. Get the texture for free, and
buy precision where a character deserves it.

Note this is *orthogonal* to `Significance`. Significance answers "how much is
this person worth spending on" and already sets the completion caps
(1,200 / 700 / 350). Curiosity answers "does this person start conversations".
An ambient child should be highly curious and cheap; a major canon should be
expensive and aloof. Do not overload one field with both.

## Work order

1. The novelty gate: eligibility from a per-actor context hash, computed beside
   `on_stage`. This is the whole cost win and it stands alone — ship it first and
   measure before doing anything else.
2. The boredom timer, off by default, as the escape hatch for (1)'s known cost.
3. Curiosity: derived default from `LoreProfile`, optional authored override,
   feeding the idle rotation's weights and/or a per-NPC cooldown.
4. Config under `smart_actors.idle_cognition`, beside `mode` / `radius_m` /
   `max_actors`, and revertible the same way.

## Acceptance

- **Standing still in a crowd, saying nothing.** After the arrival round settles,
  **zero** prompts over several minutes. This is the headline number, and it is
  the one that is broken today.
- **Walking up to someone.** They take *one* turn on arrival — not a rotation of
  re-asked, re-waited turns.
- **An NPC-to-NPC conversation** still runs to its natural end, and then stops
  costing money. Verify it terminates rather than ringing.
- **Speaking to anyone** — near, far, ambient, aloof — has unchanged latency. The
  reaction and priority lanes are untouched, and the existing tests that pin them
  must keep passing.
- **An aloof NPC never opens, but always answers.**
- Input tokens per minute standing in a market drop by ≥ 90%.

## Risks

- **What counts as "changed" is a judgement call.** Too sensitive and the gate
  saves nothing; too coarse and an NPC misses something they should have reacted
  to. The visible-*set* rule is the load-bearing one — get it wrong and this
  feature is theatre.
- **A frozen crowd may read as a frozen crowd.** The gate makes silence free, but
  it also makes it total. The boredom timer exists for this; it may need to be on.
- **Derived curiosity risks caricature** — young = chatty, titled = rude. Fine as
  a default, bad as the last word. The authored override is what keeps it from
  being a stereotype generator.
- This still does nothing for the city outside the stage. The non-LLM behavior
  layer remains the dependency the proximity feature promoted it to.

## Related

- `features/implemented/gate_idle_cognition_on_proximity.md` — the predecessor;
  the stage, the lanes and the config block this builds on.
