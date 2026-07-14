# Gate idle NPC cognition on proximity

Status: **implemented** (2026-07-14). `crates/cathedral-sim/src/attention.rs` is
the stage; `scheduler.rs` gates the round-robin arm on it;
`config.ron: smart_actors.idle_cognition` is the switch.

Measured against the shipped cast, standing at the player's spawn for 25 s:

| | prompts | distinct actors |
| --- | --- | --- |
| `mode: "all"` (before) | 26 | 26 — scattered across the city, none of them near the player |
| `mode: "stage"` (after) | 25 | 6 — the people he is standing next to |

Two deviations from the design below, both noted in place:

- §4's "suppress idle submission" also suppresses the *ordinary priority slot*
  (an NPC-to-NPC handoff, a sound nudge), not just the round robin. Those are
  equally "some irrelevant NPC's thinking" from the player's side, and the
  acceptance criterion asks for latency that is *never* worse. The slot is
  sticky, so nothing is dropped — only deferred. The protected player-reaction
  lane still fires immediately.
- `IdleGate` carries that suppression as a third variant rather than a separate
  bool argument, so the scheduler learns "who may take a non-player-reaction turn
  right now?" from one value: `All`, `Stage(..)`, or `Suppressed`.

The conversation partner's reserved seat lapses after
`STAGE_PARTNER_MEMORY_SECONDS` (30 s). Without a lapse a partner never expires,
and "alone in a field" would keep costing one prompt per rotation for the rest of
the run — which is the headline number this feature exists to zero.

## Goal

No LLM turn should be spent on an actor the player cannot see, hear, or talk to.

Today the sim thinks continuously about the whole lore cast regardless of where
the player is. The desired rule is the one a player would state: an NPC thinks
because you are near them, because you spoke to them, or because something
happened to them — never merely because their name came up in a rotation.

Autonomous movement, errands and gossip for the unobserved city are explicitly
*out of scope* here; they become a non-LLM behavior layer in a later pass. This
feature only removes the LLM turns that buy nothing.

## What the code does now

The scheduler is a single global lane — `in_flight: Option<InFlight>`
(`crates/cathedral-sim/src/scheduler.rs:117`) — and submits a new turn whenever
that slot is empty and the inter-turn delay has elapsed
(`scheduler.rs:337-343`). So cost is *not* proportional to cast size: there is
at most one provider call at any instant. The problem is that the call never
stops.

`select_next_actor` (`scheduler.rs:671`) picks from three lanes in order:

| Lane | Source | Trigger |
| --- | --- | --- |
| `player_reactions` | `prioritize_player_reaction` — the nearest listener to fresh player speech (`speech_router.rs:1013-1023`) | The player spoke |
| `priority_actor_id` | `prioritize` — an addressed `say` (`scheduler.rs:556`) or a sound nudge (`engine.rs:1172`) | Something happened |
| round robin | `order`, frozen at construction from `background_turn_order` (`engine.rs:458`) | Nothing. It is a clock |

The first two lanes are already exactly the behavior we want: they fire because
of a real event. The third is the whole problem.

`background_turn_order` (`scheduler.rs:716-746`) weights the rotation by lore
significance — Major ×4, Minor ×1, **Ambient ×0**. Against the shipped cast (30
major, 120 minor, 350 ambient sheets under `lore/characters/`) that is a
240-slot rotation, and the 350 ambient NPCs already receive no idle turn at all.
The gating instinct exists in the codebase; it just stops at "ambient".

### Measured cost

From session 99 (`logs/session_99_2026-07-14_16_38_11/prompts/`), against
`gpt-5.6-luna`:

- prompt size ~8.5-9.6k chars (roughly 2.2k tokens);
- provider latency 2.1-2.3 s;
- one turn every ~3.2 s wall clock, including the 1.0 s
  `npc_turn_delay_seconds`.

That is about **1,100 calls and ~2.5M input tokens per hour**, sustained, at
idle — whether the player is in a crowded market or standing alone in a field.
The same rotation gives each major actor a slot roughly every 3 minutes and each
minor actor one roughly every 13 minutes, so the spend does not even buy
believable background life. It is simultaneously too expensive and too slow.

## Design

Keep the single in-flight invariant, the floor gating, the priority slot and the
protected player-reaction lane. Change only *which actors the round-robin arm is
allowed to select*.

### 1. The stage

A new pure module, `crates/cathedral-sim/src/attention.rs`:

```rust
pub struct StageConfig {
    pub radius_m: f64,
    pub max_actors: usize,
}

/// The actors eligible for an idle turn: the player's neighborhood, nearest
/// first, plus whoever the player is currently in an exchange with.
pub fn on_stage(world: &World, player_id: &ActorId, cfg: &StageConfig) -> BTreeSet<ActorId>
```

Membership:

- every LLM actor within `radius_m` of the player. `World::characters_within`
  (`world.rs:121`) already returns them ordered by distance then id, so the
  result is deterministic and the cap below is free;
- plus the player's current conversation partner, even if they have drifted out
  of the radius mid-exchange (the floor holder, or the most recent addressee in
  the transcript);
- truncated to `max_actors`, nearest first.

`radius_m` should be **larger** than `HEARING_RADIUS_M` (20 m, `lib.rs:82`) —
30 m is a reasonable first guess. The point is that an NPC has already been
thinking by the time you are in earshot, instead of animating the instant you
arrive. That tell — statues that come alive when looked at — is the main
qualitative risk of this feature, and a generous radius is the cheap mitigation.

The engine computes the stage once per poll, next to `floor_busy`
(`engine.rs:546-561`), for the same reason `floor_busy` is computed there (D20):
the scheduler must not be able to change how often it runs.

### 2. Gate one lane, not three

In `select_next_actor`:

- `player_reactions` — **ungated**. The player spoke; someone answers.
- `priority_actor_id` — **ungated**. An addressed `say` or an audible sound
  reached them. This is also the only way an ambient NPC thinks today, and it
  must stay that way.
- round robin — **gated**. Scan forward from `round_robin_index` for the first
  actor on stage, bounded by `order.len()`. If nobody is on stage, select
  nothing.

`select_next_actor` returns `Option<(ActorId, bool)>`; `poll` step 3
(`scheduler.rs:337-343`) skips submission on `None`. Scanning forward rather
than rebuilding `order` preserves both the rotation's fairness and its
significance weighting, and keeps `order`-is-frozen (`scheduler.rs:100-104`)
true.

The stage reaches the scheduler as an enum, not a bare set:

```rust
pub enum IdleGate<'a> {
    All,                        // every actor in `order` may idle
    Stage(&'a BTreeSet<ActorId>),
}
```

`engine.rs` passes `Stage`. `scheduler_tests.rs`, `floor_tests.rs`, `e2e_fake.rs`
and `cathedral-headless` pass `All`, so they keep exercising the full cast
without having to fake proximity. `cathedral-headless` gets a `--stage` flag for
when the gate itself is what is under test.

### 3. Re-tune the significance weighting — its meaning changes

This is the part that is easy to miss.

Today `background_turn_order` answers *"who, out of 500 people, deserves scarce
global compute?"*, and Major ×4 / Minor ×1 / Ambient ×0 is the right answer:
ambient people are the ones you will never meet.

With the gate in, the rotation answers a different question — *"who, out of the
six people standing in front of the player, thinks next?"* — and there Ambient ×0
is backwards. The market crowd around you **is** ambient. Under the current
weights they would be the statues this feature is supposed to prevent.

Inside the stage, flatten to roughly Major 3 / Minor 2 / Ambient 1, and let the
existing per-significance completion caps (`lore.rs:39`: 1200 / 700 / 350
tokens) keep ambient turns cheap. An ambient fishmonger beside you should live —
just in fewer tokens.

Concretely: `background_turn_order` keeps its current weights for `IdleGate::All`
(the headless runner and the tests depend on them), and a second
`stage_turn_order` supplies the flattened weights used when a stage is active.

### 4. Do not start an idle turn while the player is talking

The protected reaction lane is immediate at *selection*, but it cannot preempt
an in-flight call, so today every player utterance can wait out ~2 s of some
irrelevant NPC's thinking. Cheapest fix, and it composes with this feature:
**suppress idle submission while the player is composing** — microphone hot, STT
in flight, or inside the router's grace window. `SpeechRouter` knows all three;
surface it as one bool and add it to the step-3 submit condition alongside the
gate.

That makes the common case "nothing was in flight when your words landed", so
your partner starts thinking immediately. True cancellation — a
`Cognition::cancel(request_id)`, restoring the actor's drained inbox and pending
history via the existing `apply_failure` path — remains the fallback if
measurement shows suppression is not enough. See the "Foreground cognition lane"
section of `features/quicker_response_improvements.md`, which this supersedes as
the cheap first move.

### 5. Configuration

Under `smart_actors:` in `config.ron`:

```ron
idle_cognition: (
    mode: "stage",     // "stage" | "all"
    radius_m: 32.0,
    max_actors: 6,
),
```

`mode: "all"` reproduces today's behavior exactly, so the change is a one-line
A/B and is revertible without a rebuild.

## Work order

1. `attention.rs` with `on_stage` + unit tests (pure, no engine needed).
2. `IdleGate`, `select_next_actor -> Option`, gated submit in `poll`. Existing
   scheduler tests pass `IdleGate::All` and must not change behavior.
3. Engine computes the stage per poll and passes `IdleGate::Stage`.
4. `stage_turn_order` with the flattened weights.
5. Config plumbing (`config.ron`, `cathedral-backends/src/config.rs`,
   `EngineConfig`), plus the `--stage` flag on `cathedral-headless`.
6. Microphone-hot idle suppression.

## Acceptance

- **Alone in a field**: zero prompts written to `logs/latest_session/prompts/`
  over a multi-minute drive. This is the headline number; today it is ~1,100
  calls/hour.
- **Standing in a crowd**: still at most one in-flight call, so the worst case
  does not get worse than today's steady state. Idle turns are distributed over
  the nearest `max_actors`, ambient included.
- **Conversation**: player-reaction latency unchanged or better, and never worse
  because of a background turn that started while the microphone was live.
- **Walking into a plaza**: NPCs are mid-behavior on arrival, not visibly booting.
  Tune `radius_m` against recorded drives until this reads right.
- Ambient NPCs remain reachable by `say` and by sound nudges regardless of the
  gate — the existing scheduler test at `scheduler.rs:944` pins this and must
  keep passing.

## Known consequence

With the gate in and nothing behind it, the city outside the stage stops moving
entirely: no errands, no autonomous movement, no gossip propagation. That is the
accepted trade — but it promotes the non-LLM behavior layer from a nice-to-have
to a dependency. Until it lands, the world will feel emptier than it does today
in exchange for costing nothing.
