# NPCs stop walking when they talk (and trade) with each other

Ilse and Sven walk their round errands while conversing — and while handing a
fish and a coin back and forth at full stride, which looks absurd. The design
principle already exists and is even written down: `interrupt_for_conversation`
(`round.rs`) says *"nobody keeps walking away from a conversation."* It just only
fires for the **player**. This feature extends the same courtesy to NPC↔NPC
exchanges.

## What's already there (don't rebuild it)

The player-side machinery has exactly the right shape; this is a generalization,
not a new system.

- **The interrupt.** `round::interrupt_for_conversation(round, world, id)` —
  drops a walker (`Approaching`/`Travelling`/`Returning`) to `Phase::Idle` where
  they stand; well queuers keep their place; non-enrolled actors are left alone.
  The ladder re-decides once the hold lapses, so the errand resumes on its own.
- **The trigger.** `Engine::flush_speech` (`engine.rs:1565-1573`): fires on a
  *targeted* line — player→NPC or NPC→player. The NPC→NPC case falls through
  both branches today. Broadcast lines never trigger it, and shouldn't.
- **The warmth/lapse pattern.** `last_player_exchange: Option<(ActorId, f64)>`
  plus `conversation_partner(now)`: warm while the last targeted line is younger
  than `STAGE_PARTNER_MEMORY_SECONDS` (30 s, `attention.rs:72`), lapses on
  silence with no explicit "conversation over" event.
- **The hold.** `round::tick(..., in_conversation)` threads the single warm
  partner through `service_sources` (a finished draw doesn't send them home,
  `round.rs:951`) and `run_ladder` (skipped entirely, `round.rs:1043`).
- **The convergence guarantee** (why two chatty LLMs is a bounded problem):
  conversations are percept-driven, and a `wait` emits no domain event — no
  percept, no eligibility, cost falls to zero
  (`features/implemented/gate_idle_cognition_on_novelty.md`, "Why the loop
  terminates"; pinned by the `actions.rs` test "waiting is not a world event").
  The LLM itself ends conversations by having nothing new to say. This feature
  only needs a backstop for the tail, not a chat-termination system.

Note: there is **no interest meter** anywhere, and the *boredom timer* in the
novelty doc is the opposite mechanism (re-eligibility after N quiet minutes, off
by default). Don't conflate them; neither is what bounds the hold here — the
ladder is (below).

## The three pieces to build

**1. Generalize the warmth slot to a pair-keyed set.** Replace the single
`last_player_exchange` slot (conceptually — the player entry can stay as-is or
become an entry in the set) with warm exchanges keyed by unordered actor pair:
`(min(a,b), max(a,b)) → last_line_at`. Written on every *targeted* line between
two characters, expired by the same 30 s silence rule. Like the novelty hash,
this is derived bookkeeping, not world state — it belongs next to the stage in
`attention.rs` or on the `Engine`, **not** in `World`.

**2. Interrupt and hold both parties.** In `flush_speech`, on any NPC→NPC
targeted line, call `interrupt_for_conversation` for **both** speaker and
target (the speaker may themselves be mid-errand). Change `round::tick`'s
`in_conversation: Option<&ActorId>` into the set of all currently-warm actors;
`run_ladder` and `service_sources` skip anyone in it. Also fire the interrupt
from `offer_item` / `accept_offered_item` (`actions.rs:81-82`) for giver and
receiver — a physical handoff is at least as conversation-shaped as a line, and
it's the fish-and-coin case that prompted this. (`require_interaction_range`
already forces proximity; this adds *standing*.)

**3. The rung override — urgency beats chat.** The hold must not outrank the
ladder's high rungs, or a long exchange leaves both of them in the street at
the Snuffing — *exactly the person the watch stops*
(`features/movement/04_the_round.md` §6). Instead of skipping a warm actor
outright, `run_ladder` evaluates them and honors the hold **only against rungs
below curfew**: parched (rung 2) and curfew (rung 5) break the hold; the round
(rung 9), social pull (11) and wander (12) respect it. One comparison, no new
meter.

When a high rung is about to break the hold, don't just march the body off
mid-sentence: inject the pressure as a `system:` percept on the standard
self-correction channel ("night is falling — you need to be home") so the LLM
gets **one turn to excuse itself**, then release the hold regardless of what it
says. The prompt already carries `current_goal`; this is a nudge through an
existing seam (`features/movement/05_the_llm_seam.md`), not new plumbing.

## Scope / caution

- The player path must not regress: `conversation_partner` also feeds the
  stage's reserved seat and the hot-channel snapshot (`engine.rs:782`) — keep
  the player pair's behaviour byte-identical, whatever the new representation.
- The warm set is small (bounded by concurrent conversations, in practice a
  handful) — a `BTreeMap` is fine, no spatial anything.
- Watch for hold cycles at scale: A talks to B, B to C… a chain could freeze a
  crowd. The 30 s-per-pair lapse plus the rung override bound it; if a plaza
  ever gels solid anyway, cap holds per actor at one warm pair (the newest).
- Headless first: `cargo run -p cathedral-backends --bin cathedral-headless --
  --fake -t 10` should show a pair exchanging an item with both `Phase::Idle`
  during the exchange and errands resuming after the lapse.
- Tests to add mirror the existing ones: NPC→NPC line interrupts both walkers;
  offer/accept interrupts; hold lapses on 30 s silence and the errand resumes;
  curfew breaks a warm hold (after the one excuse-yourself turn); broadcast
  lines interrupt nobody.
