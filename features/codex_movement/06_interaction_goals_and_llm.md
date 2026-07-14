# 6. Interaction, goals, and optional LLM reflection

Movement changes cognition even if cognition never controls walking. Prompts need current activity context; moving targets need attention behavior; offers need stable proximity; daily experience can eventually influence personal motives. This chapter makes those integrations explicit while preserving the cost and responsiveness guarantees of the current system.

## 6.1 Principles

1. Ordinary movement, work, sleep, navigation, and schedules use no LLM calls.
2. Player-facing cognition remains the highest-priority provider work.
3. An LLM may request only typed high-level intent; code validates and executes it.
4. An LLM never returns coordinates, route points, steering, animation, or arbitrary schedule code.
5. Reflection is optional, budgeted, dirty/event-driven, and safe to skip forever.
6. Ambient population does not receive hundreds of individual daily calls.
7. Routine events are summarized by code; prompts contain only material experience.
8. Provider failure changes richness, not the city's ability to function.

## 6.2 Talking to a moving person

The existing targeted-speech path should acquire a movement attention lease as soon as the simulation accepts the utterance, not when the provider responds.

Recommended sequence:

1. Validate that the actor can currently hear/be addressed.
2. Record the utterance using the actor's authoritative position at delivery time.
3. Acquire `AttentionLease::PlayerConversation` for a short renewable physical duration.
4. Suspend or mark the current activity according to its interruption policy.
5. If walking, decelerate over a fraction of a second and turn toward the player.
6. Submit/queue cognition with activity and interruption context.
7. Renew the lease while the actor has protected reaction priority, is speaking, is awaiting a direct player response, or has a pending offer.
8. After a quiet timeout, release the lease and resume/re-evaluate.

The actor should not chase the player merely because the player spoke from hearing range. If the player moves away while a reply is pending, the actor can face the last/current known player position and speak at the actual resulting distance. Existing speech delivery rules decide who hears it.

If the actor is inside a virtual interior and the player is outside, ordinary targeted speech should fail the same way as any disconnected spatial stage unless a portal/intercom-like rule explicitly allows it.

## 6.3 Conversation versus schedules

Schedules should be socially interruptible. A character who is already late may mention it, but should not abruptly leave during the protected exchange.

Prompt context can include:

```text
Current activity: travelling to your assigned market stall.
Schedule context: expected at 07:00; you are currently 12 minutes late.
Interruption: you stopped because the player addressed you.
Resume policy: you may return to the trip after this exchange.
```

This is descriptive context, not an instruction to fabricate an excuse. The behavior engine owns the lateness calculation. The LLM may say it is late or choose a valid high-level action; it cannot rewrite the clock.

When the lease ends:

- if the original intent remains valid and its reservation survived, resume from the current position;
- if it expired or the actor moved materially, re-evaluate;
- if the conversation produced a higher-priority validated directive, apply normal preemption;
- if no provider reply arrives, the fake/no-response path releases the lease after a bounded timeout.

## 6.4 Offers, handoffs, and movement

Offers currently depend on proximity. Movement creates several edge cases:

- proposer and recipient may walk out of range before acceptance;
- one party may begin a portal transition;
- a routine may release the destination that kept them together;
- an item action may be generated from a prompt containing an older position.

Policy:

- a targeted pending offer creates a short interaction lease for both spatially present parties when in range;
- normal routine movement is suspended while the offer is immediately actionable;
- the offer retains its existing semantic TTL and still fails naturally if distance becomes invalid;
- portal transitions are not allowed to begin while a protected handoff is executing;
- action application always validates current authoritative distance, never prompt-time or render-time distance;
- after completion/expiry, both parties independently resume/re-evaluate.

Do not pin actors together for the full lifetime of a long unattended offer. The interaction lease is shorter and renewable only through active engagement.

## 6.5 Prompt representation of place and motion

Prompts should use human-readable semantic context derived from nav/area data:

- current named area or nearest known place;
- current activity and purpose;
- destination name and expected arrival/lateness;
- whether the actor is stopped for the player;
- nearby known actors as determined at prompt time;
- relevant day phase and clock time;
- material changes since the prior turn.

Avoid dumping route polygons, exact coordinates, velocity vectors, package scores, or debug reasons into normal prompts. A verbose headless/debug mode can archive them in meta, not in the actor's perceived world.

The “unknown people” rule remains authoritative. A moving actor does not learn a name merely by passing within range. Reflection may refer only to stable actor IDs already known through allowed events, and output validation rejects invented references.

## 6.6 High-level intent actions

Routine behavior does not require a new prompt verb. A later milestone can allow player-facing cognition to create an intent from a fixed vocabulary:

```json
{
  "action": "adopt_intent",
  "kind": "visit_place",
  "target": "place:wickmarket",
  "urgency": "normal",
  "expires": "today_at_18:00",
  "reason": "agreed_with_player"
}
```

Possible initial kinds:

- `visit_place`;
- `return_home`;
- `wait_at_place`;
- `meet_actor_at_place` only after rendezvous exists;
- `resume_routine`;
- `cancel_optional_intent`.

Validation must confirm:

- action kind is allowed for this turn source;
- target ID appeared in the prompt's allowed-target table or is otherwise known;
- actor has a compatible navigation/presence profile;
- place has an appropriate accessible affordance;
- expiry and priority are bounded;
- it does not override quest/safety constraints;
- it does not imply an unavailable capability;
- it does not create or reveal knowledge the actor lacks.

If validation succeeds, the action creates a normal `IntentSource::Cognition`/`Directive` and the behavior engine decides when/how it can execute. If it fails, return a clear action result to the actor's next prompt and leave the world unchanged.

Do not initially add `follow_actor`. Following a moving target requires distance bands, waiting behavior, portal rules, loss/reacquisition, and anti-harassment semantics. Deliver static place intentions first.

## 6.7 Prose goals versus executable motives

The current character `goal` is useful narrative state. Keep it as such. Add a separate structured field:

```rust
pub struct GoalState {
    pub narrative_goal: Option<String>,
    pub motive: Option<PersonalMotiveState>,
    pub directives: Vec<Directive>,
}
```

Examples:

- narrative goal: “Learn whether the new canon can be trusted.”
- structured motive: `Observe(PlaceId::CathedralForecourt)` twice this week.
- directive: `MeetActorAtPlace` tomorrow, if that feature is supported.

The LLM may revise the narrative goal in prose. It may only choose a structured motive from a provided schema/target list. Code owns feasibility and turns the motive into agenda opportunities.

No runtime NLP attempts to turn “become the greatest baker in Ombreval” into a path to a random oven.

## 6.8 Experience journal

Daily reflection should not receive every step, wait, or routine activity. Maintain a compact journal of material events:

```rust
pub struct ExperienceJournal {
    pub since: GameInstant,
    pub entries: Vec<ExperienceEntry>,
    pub routine_summary: RoutineSummary,
    pub dirty_score: u16,
}

pub enum ExperienceEntryKind {
    PlayerUtterance,
    ActorUtterance,
    Agreement,
    OfferOutcome,
    GoalOutcome,
    ImportantWitnessedEvent,
    RelationshipEvent,
    SignificantFailure,
    PlaceDiscovery,
}
```

The code-generated `RoutineSummary` can say:

```text
worked assigned shift; arrived 18 game minutes late;
completed 3 deliveries; spoke with the player; missed evening service
```

It must not claim mental reactions. “Felt humiliated” is an LLM/authorial interpretation; “player rejected offer” is a fact.

Journal controls:

- coalesce repeated nearby events;
- cap entries and preserve highest materiality;
- retain stable IDs and concise text/facts;
- never log every movement waypoint;
- expire low-value routine summaries after reflection or a retention horizon;
- archive prompt inputs through the existing backend host only when a request is actually made.

## 6.9 Reflection eligibility

“Every person reflects every day” is too expensive and is usually narratively empty. Eligibility should require all of:

- reflection feature enabled and backend healthy;
- actor tier/cadence eligible;
- journal dirty score above threshold or a specifically important event;
- actor at a reflection-friendly boundary (normally sleeping/resting, never required exactly at midnight);
- foreground cognition budget is clear;
- global daily/hourly token/request/cost budgets have room;
- actor has not reflected too recently.

Suggested starting policy:

| Cast tier | Default reflection policy |
|---|---|
| major | eventful only; eligible daily, but global priority cap applies |
| minor | eventful only; minimum several-game-day cooldown |
| ambient | no individual reflection |

At a 48-minute game day, 30 individual major calls every day would already be 37.5 background calls per real hour before minor actors or normal dialogue. That is not acceptable as an implicit default. Start with a hard global cap such as 3–5 reflection requests per game day and a token/USD ceiling, then tune using real prompt archives and value review.

Reflection should be optional and initially off or conservative in normal config until measured.

## 6.10 Scheduling reflection without harming dialogue

The current cognition scheduler is intentionally constrained. A long background reflection occupying its single in-flight slot could make a player wait for an NPC reply. “Low priority in the queue” is insufficient once the HTTP request has begun.

Non-negotiable rule:

> Background reflection may never occupy or delay the foreground conversation lane.

Implementation options, in preference order:

1. a separate backend `ReflectionCognition` lane with concurrency 1, independent cancellation/lifecycle, and its own strict budget;
2. provider requests that can be cancelled immediately when foreground work arrives, with proven cancellation behavior;
3. no reflection until one of the above exists.

The separate lane can still share credentials and global accounting. It must not share a one-request worker that serializes foreground behind background. The sim interacts through another non-blocking trait returning plain values; filesystem/network/threading remains in `cathedral-backends`.

Spread eligible reflections across actors' sleep periods with seeded jitter. Do not enqueue every reflection at midnight. If a budget is exhausted, leave the journal for a later night or compact it deterministically.

## 6.11 Reflection request and response

A reflection prompt should contain:

- stable actor identity and concise relevant profile;
- current narrative goal and structured motive;
- recent retained memories;
- material journal entries and factual routine summary;
- allowed memory operations;
- allowed motive kinds and target IDs;
- strict response schema and length limit;
- instruction not to invent events, identities, or knowledge;
- current day/phase only if relevant.

Example response shape:

```json
{
  "summary": "The market dispute matters more than the missed service.",
  "memory_additions": [
    {
      "fact": "The player warned me about trouble near the east gate.",
      "importance": 0.7,
      "references": ["actor:player", "place:east_gate"]
    }
  ],
  "memory_supersedes": [],
  "narrative_goal": "Learn whether the east-gate warning is credible.",
  "motive_suggestion": {
    "kind": "visit_place",
    "target": "place:east_gate",
    "window": "daytime_optional"
  }
}
```

The response is not applied directly. Parse, validate, normalize, and apply atomically against a base semantic revision:

- text lengths and counts are capped;
- references must be in the allowed known set;
- memories cannot delete protected facts;
- a superseded memory ID must exist and be allowed;
- target and motive kind must be whitelisted;
- schedule window is selected from a fixed enum;
- stale output cannot overwrite a newer player-generated goal without a merge rule;
- invalid output produces no partial mutation;
- retry count is zero or one and consumes budget.

Store request kind, usage, estimated cost, actor, base revision, validation result, and applied changes in prompt archive meta.

## 6.12 Ambient population strategy

Do not send one enormous prompt containing all 350 ambient biographies. It would be expensive, hard to validate, likely to homogenize people, and capable of inventing hundreds of unreviewed facts.

Ambient behavior should remain deterministic from role, ward, phase, seed, and current city conditions. If a little daily variation is desired, use at most one compact aggregate call per game day (and only under budget) to propose bounded **city pressure seeds**, for example:

```json
{
  "pressures": [
    {"ward": "river", "kind": "seek_indoor_social", "weight": 0.15},
    {"ward": "market", "kind": "leave_early", "weight": 0.10}
  ]
}
```

Inputs are aggregate facts and a short list of actual city events, not all actors. Outputs come from a fixed enum, are clamped, expire within a day, and never alter individual memory or lore. Code applies them as small preference modifiers. If the call fails or is disabled, seeded code variation supplies the day profile.

An even safer first version is no ambient LLM call at all. Implement and playtest the pressure interface with deterministic generated values before deciding whether a model adds visible value.

## 6.13 Reflection cost ledger

Configuration needs independent caps:

```ron
reflection: (
    enabled: false,
    max_in_flight: 1,
    max_requests_per_game_day: 4,
    max_input_tokens_per_game_day: 12000,
    max_output_tokens_per_game_day: 2000,
    max_cost_usd_per_real_hour: 0.05,
    major_min_dirty_score: 5,
    minor_min_dirty_score: 12,
    minor_cooldown_days: 3,
    ambient_city_pressure: false,
)
```

Numbers above are placeholders for measured tuning, not promises. Enforce both game-day and rolling real-time/provider cost ceilings. When price metadata is unavailable, token/request caps still work. Real environment variables and existing backend configuration rules continue to apply.

Budget priority within eligible work:

1. major actor with new player agreement or relationship-changing event;
2. any named actor with critical goal outcome;
3. other eventful major;
4. eventful minor past cooldown;
5. optional aggregate pressure call.

Stable tie-breaking plus dirty score ensures an actor is not starved forever, but reaching the next day never creates debt that must be spent.

## 6.14 Fake backend and deterministic tests

Fake mode should support a deterministic reflection result or an explicit `reflection unavailable` outcome. Tests need to verify:

- movement/routines are identical with reflection enabled but never scheduled;
- budget gating and eligibility order;
- foreground conversation proceeds while reflection is in flight;
- stale/invalid response rejection;
- known-reference validation;
- structured motive application and later agenda opportunity;
- save/load of queued journal state without replaying applied output.

Do not make full behavior determinism depend on the text generated by a live provider. Integration tests can substitute typed reflection results.

## 6.15 LLM-influenced intent failure

A valid reflection suggestion may later be impossible: a gate closes, a spot is full, or the actor is interrupted. The behavior layer handles it like any personal motive. It may:

- retry in another optional window;
- choose another compatible affordance;
- mark the structured motive blocked/failed;
- add one factual material journal entry if the failure matters;
- never immediately call the LLM just to solve pathfinding.

The next eligible reflection can reconsider the goal. This makes the LLM a slow layer over actual experience, not a synchronous fallback planner.

## 6.16 Privacy and prompt scale

Movement creates many observations; most should not become prompt content. Only pass what the actor plausibly perceived and what is relevant to the reflection. Nearby strangers remain unnamed. Aggregate ambient data should not contain private prompt transcripts or full biographies.

Prompt archives remain impure backend output. The pure sim can create a typed `ReflectionRequest` payload, but cannot write it to disk. Existing private audio/prompt-directory guarantees should extend to reflection metadata.

## 6.17 Rollout recommendation

1. Add activity context and attention leases to ordinary player-triggered cognition.
2. Add the typed intent interface but keep it disabled until destination validation is robust.
3. Add the material journal with no reflection consumer; use it for debug summaries.
4. Add a deterministic fake reflection lane and budget tests.
5. Add independent live background cognition, default disabled.
6. Evaluate prompt archives for actual narrative value before enabling any ambient aggregate call.

Movement must ship before step 4–6 if necessary. Daily LLM reflection is enrichment, never a dependency.

## 6.18 Acceptance criteria

1. A moving NPC stops/faces the player on targeted speech before provider completion and later resumes plausibly.
2. Provider latency or failure cannot leave an actor permanently frozen.
3. Current authoritative positions validate every proximity action.
4. A cognition action can request a known place intent but cannot specify a coordinate or bypass feasibility.
5. Routine walking and schedules produce zero cognition requests.
6. Reflection never delays foreground player dialogue.
7. Reflection request, token, and cost caps are enforced under time acceleration and save/load.
8. Invalid/stale reflection output applies no partial state.
9. Ambient characters behave fully with no individual reflection.
10. Disabling every backend leaves the same movement, clock, routine, and interaction-lease mechanics.
