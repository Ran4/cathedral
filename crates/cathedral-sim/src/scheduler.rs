//! The global NPC turn stream (`scheduler.py`): one request in flight, ever.
//!
//! Round-robin over the LLM cast, with an ordinary priority lane for NPC
//! turn-taking, a protected FIFO lane for reactions to player speech, provider
//! backoff, and floor gating.
//!
//! Three lanes select the next actor, and only one of them is a clock. The
//! player-reaction lane fires because the player spoke; the priority lane fires
//! because an addressed `say` or an audible sound reached someone. The round
//! robin fired because time passed — so it, alone, is gated on an [`IdleGate`]
//! the engine computes from the player's neighborhood
//! (`features/gate_idle_cognition_on_proximity.md`). An ambient NPC across the
//! city remains reachable by speech and by sound exactly as before; what stops
//! is thinking at nobody.
//!
//! Python ran the provider call on a daemon thread and everything else on the
//! poll thread. Here the split is a trait boundary instead: [`Cognition::request`]
//! is a non-blocking submit, the completion comes back as a plain
//! [`Completion`] value in the next [`NpcScheduler::poll`]. Rendering, parsing,
//! action application and world mutation all still happen inside `poll` — which
//! is the point: a reply is revalidated against the *then-current* world, not
//! the one it was prompted from.
//!
//! The scheduler is clock-free. Every `time.monotonic()` in Python is a `now`
//! parameter here.

use std::collections::VecDeque;

use serde_json::{Map, Value};

use crate::{
    MAX_LLM_REPLY_CHARS,
    actions::apply_action,
    attention::IdleGate,
    character::Control,
    ids::{ActorId, RequestId},
    lore::Significance,
    prompt::{PromptEnv, parse_reply, render_prompt_and_drain},
    pyfmt::py_repr_map,
    status::{STATE_DEGRADED, STATE_IDLE, STATE_THINKING, StatusEvent},
    traits::{Cognition, CognitionError, Completion},
    world::World,
};

/// An oversized reply is a provider failure, not a turn (D17). Python raised
/// this inside the worker thread (`scheduler.py:86-87`); we enforce it on
/// receipt so every backend — fakes included — is covered.
///
/// The *value* is the exception kind, because that is all Python's log ever
/// showed: `raise ValueError(…)` inside the worker surfaced as
/// `type(error).__name__` in `[smart actors] LLM request for … failed: …`
/// (`scheduler.py:242-246`). [`CognitionError`] carries a short kind name by
/// contract — real backends report `TimeoutError` and friends — so the one error
/// the sim raises itself has to speak the same language.
pub const REPLY_TOO_LARGE: &str = "ValueError";

/// The `system:` lines the scheduler fabricates into an actor's inbox.
///
/// They are model-visible English, but they are *world facts about this turn*
/// rather than prompt prose, so they stay `format!` calls in Rust (D3). They are
/// appended to `inbox` only, never to `pending_history`: shown once as
/// `since_your_last_turn`, then gone — they never graduate into
/// `recent_history` (scheduler.md §2).
const SYSTEM_PROVIDER_FAILED: &str =
    "system: the cognition provider failed; your turn will be retried later";
const SYSTEM_PROMPT_FAILED: &str = "system: your prompt could not be prepared";
const SYSTEM_WORKER_BUSY: &str = "system: the cognition worker is busy";

/// Everything the scheduler tells the outside world (scheduler.md §7.5).
///
/// One `Vec` replaces Python's returned statuses, its `[smart actors] …` stderr
/// prints, and its `PromptLog.record` callback — so the sim stays IO-free and
/// the tests stay assertable.
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerEvent {
    /// Forwarded to the HUD as today's `status` message.
    Status(StatusEvent),
    /// A former stderr line; the host logs it via `tracing`.
    Diagnostic(String),
    /// One archived LLM exchange, successes and failures alike (D24: the file
    /// writing lives in cathedral-backends).
    PromptExchange {
        actor_id: ActorId,
        actor_name: String,
        prompt: String,
        answer: Option<String>,
        duration_seconds: f64,
        error: Option<String>,
    },
}

/// Which lane selected a turn. Carried on the flight because a failure owes
/// each lane a different courtesy: a protected reaction goes back to the head
/// of the player's lane, an ordinary handoff to the head of its own — off
/// stage that lane is the only thing that ever selects its occupant, so a
/// consumed slot is a conversation ended by a network blip — and an idle turn
/// goes back to nobody, because the rotation that produced it comes around
/// again on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnLane {
    /// Player speech selected this turn. Its completed reply may ignore a
    /// background-only conversation-floor hold, but never speech the player
    /// can hear or the player's own microphone hold.
    PlayerReaction,
    /// An addressed `say` or a sound nudge selected this turn.
    Handoff,
    /// The gated round robin selected this turn.
    Idle,
}

/// The one outstanding request.
#[derive(Debug, Clone, PartialEq)]
struct InFlight {
    actor_id: ActorId,
    /// Presence generation at submit. A reply from before departure may never
    /// act on a later visit by the same stable actor id.
    presence_epoch: u64,
    request_id: RequestId,
    lane: TurnLane,
    /// The inbox as it was *before* the prompt drained it — restored on failure.
    drained_events: Vec<String>,
    /// The percepts the prompt showed as `since_your_last_turn`: they graduate
    /// into `recent_history` on success and go back to pending on failure.
    presented: Vec<String>,
    /// Kept sim-side so a *failed* exchange can still be archived.
    prompt: String,
}

pub struct NpcScheduler {
    /// The LLM cast in world-insertion order, frozen at construction
    /// (`scheduler.py:137-139`). An NPC added to the world later is never
    /// scheduled; one whose control changes is skipped at selection time but
    /// keeps its slot (scheduler.md risk 7).
    order: Vec<ActorId>,
    minimum_delay_seconds: f64,
    maximum_backoff_seconds: f64,
    round_robin_index: usize,
    /// Ordinary turn-taking handoffs — addressed `say`s and sound nudges —
    /// oldest first and de-duplicated, like `player_reactions`.
    ///
    /// This used to be a single last-write-wins slot, which silently dropped a
    /// handoff whenever two real events landed between turns. On stage the idle
    /// rotation eventually recovered the dropped actor; an off-stage exchange
    /// (two NPCs talking in a far ward) has no rotation to fall back on, so a
    /// dropped handoff there killed the conversation outright.
    priority_handoffs: VecDeque<ActorId>,
    /// Protected reactions to player speech, oldest first and de-duplicated.
    ///
    /// This is deliberately separate from `priority_handoffs`: a background
    /// NPC reply can finish in the same poll as STT and hand its addressee the
    /// ordinary lane. That must not erase the listener the player just woke.
    /// Separate lanes, one queue slot: no actor is ever in both at once, or the
    /// two pops would spend two provider calls on the one turn's worth of news.
    player_reactions: VecDeque<ActorId>,
    in_flight: Option<InFlight>,
    /// A finished turn the floor would not let us apply yet.
    held_result: Option<Completion>,
    next_turn_at: f64,
    /// Consecutive provider failures; only a successful turn resets it.
    provider_failures: u32,
    running: bool,
    /// Whose prompt went out during this poll, for [`Self::take_submitted`].
    submitted: Option<ActorId>,
}

impl NpcScheduler {
    /// `order` is the LLM cast in turn order — [`llm_turn_order`] builds it from
    /// a world's roster.
    ///
    /// `maximum_backoff_seconds` is normalized to `max(1, cap, delay)` here
    /// (`scheduler.py:128-130`), so the effective cap is never below one second
    /// and never below the inter-turn delay. A negative `minimum_delay_seconds`
    /// (Python raised `ValueError`) is clamped to zero — the caller reads it
    /// from config, and a bad number must not take the cast offline.
    pub fn new(
        order: Vec<ActorId>,
        minimum_delay_seconds: f64,
        maximum_backoff_seconds: f64,
        now: f64,
    ) -> Self {
        // `f64::max` returns the other operand for NaN, so this sanitizes too.
        let minimum_delay_seconds = minimum_delay_seconds.max(0.0);
        let maximum_backoff_seconds = maximum_backoff_seconds.max(1.0).max(minimum_delay_seconds);
        Self {
            order,
            minimum_delay_seconds,
            maximum_backoff_seconds,
            round_robin_index: 0,
            priority_handoffs: VecDeque::new(),
            player_reactions: VecDeque::new(),
            in_flight: None,
            held_result: None,
            // The first turn is eligible immediately.
            next_turn_at: now,
            provider_failures: 0,
            running: false,
            submitted: None,
        }
    }

    /// Idempotent: also clears any residual delay so the first turn can start on
    /// the next poll.
    pub fn start(&mut self, now: f64) {
        self.running = true;
        self.next_turn_at = self.next_turn_at.min(now);
    }

    /// Stop submitting new turns.
    ///
    /// A `poll` after `close` still APPLIES an in-flight result — `running`
    /// gates only submission (`scheduler.py:162-164`, scheduler.md risk 4).
    /// Ported deliberately: applying a turn that already cost a provider call is
    /// harmless and loses nothing.
    pub fn close(&mut self) {
        self.running = false;
    }

    /// Remove departed actors from every pending lane without cancelling the
    /// provider request. An outstanding completion is still archived, but its
    /// prompt-drained buffers are deliberately discarded and the epoch guard
    /// prevents it from mutating this or a later visit.
    pub fn actors_departed(&mut self, actors: &[ActorId]) {
        self.priority_handoffs
            .retain(|actor| !actors.contains(actor));
        self.player_reactions
            .retain(|actor| !actors.contains(actor));
        if self
            .submitted
            .as_ref()
            .is_some_and(|actor| actors.contains(actor))
        {
            self.submitted = None;
        }
        if let Some(flight) = self
            .in_flight
            .as_mut()
            .filter(|flight| actors.contains(&flight.actor_id))
        {
            flight.drained_events.clear();
            flight.presented.clear();
        }
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn in_flight_actor_id(&self) -> Option<&ActorId> {
        self.in_flight.as_ref().map(|flight| &flight.actor_id)
    }

    /// The actor whose prompt went out during the last [`Self::poll`], taken once.
    ///
    /// A turn is the only thing that clears an actor's news, and this is the one
    /// instant it happens — the render has just drained their inbox and the
    /// prompt has left with the world as it stands. The engine stamps
    /// [`Novelty`](crate::attention::Novelty) with it.
    ///
    /// Set only on a *successful* submit: a prompt that failed to render, or one
    /// the worker refused, showed the actor nothing and must not mark their news
    /// as seen. Both of those paths put a system line back in the inbox, so such
    /// an actor stays eligible and is retried.
    ///
    /// Reported rather than stamped here because the scheduler must not be the
    /// thing that decides how often derived per-poll state is recomputed (D20).
    pub fn take_submitted(&mut self) -> Option<ActorId> {
        self.submitted.take()
    }

    /// Whether the outstanding prompt was protected player-speech work.
    ///
    /// The engine uses this to ignore only conversation-floor holds belonging
    /// to speech the player cannot hear. Ordinary turns remain globally paced.
    pub fn in_flight_is_player_reaction(&self) -> bool {
        self.in_flight
            .as_ref()
            .is_some_and(|flight| flight.lane == TurnLane::PlayerReaction)
    }

    /// Who has been handed the next selection slot, if anyone. Protected player
    /// reactions are reported before the ordinary sound/post-`say` lane.
    pub fn priority_actor_id(&self) -> Option<&ActorId> {
        self.player_reactions
            .front()
            .or(self.priority_handoffs.front())
    }

    /// Whether a finished turn is parked waiting for the floor.
    pub fn has_held_result(&self) -> bool {
        self.held_result.is_some()
    }

    /// Whether protected player-speech work is queued or already out.
    ///
    /// The Night Office's yield condition (movement M6): the player's lane is
    /// untouchable, and a reflection must not so much as be *submitted* beside
    /// it — not because it would take the slot (it has its own), but because a
    /// player who is mid-conversation is the one state in which the whole city
    /// should be spending its attention on him.
    pub fn player_reaction_pending(&self) -> bool {
        !self.player_reactions.is_empty() || self.in_flight_is_player_reaction()
    }

    pub fn turn_order(&self) -> &[ActorId] {
        &self.order
    }

    /// Queue `actor` for the next free selection slots, oldest handoff first.
    /// Returns whether the handoff was accepted.
    ///
    /// A non-LLM or unknown target is rejected — which is exactly what makes an
    /// NPC's `say` to the *player* a silent no-op instead of a broken handoff.
    /// The check is against the world, not against `order`: Python looked
    /// `world.characters` up (`scheduler.py:166-176`) and so do we.
    ///
    /// An actor already queued is not queued twice: their one turn answers
    /// everything that has reached them, because the render drains the whole
    /// inbox. De-duplication is also what bounds the queue (by the cast size)
    /// without a cap. It spans **both** lanes — somebody the player has already
    /// woken needs nothing from this one, and a second slot here would buy a
    /// paid provider call whose `since_your_last_turn` the earlier turn already
    /// emptied. The lanes are split so a background handoff cannot *erase* the
    /// player's listener, not so the same actor thinks twice.
    ///
    /// `immediate` collapses the remaining inter-turn/backoff delay. It never
    /// preempts the in-flight request and never bypasses floor gating; without
    /// it, only the *selection order* changes and the timing stands.
    pub fn prioritize(
        &mut self,
        world: &World,
        actor_id: &ActorId,
        immediate: bool,
        now: f64,
    ) -> bool {
        let Some(actor) = world.characters.get(actor_id) else {
            return false;
        };
        if actor.control() != Control::Llm || !world.is_present(actor_id) {
            return false;
        }
        if !self.priority_handoffs.contains(actor_id) && !self.player_reactions.contains(actor_id) {
            self.priority_handoffs.push_back(actor_id.clone());
        }
        if immediate {
            self.next_turn_at = self.next_turn_at.min(now);
        }
        true
    }

    /// Queue the nearest listener to fresh player speech as protected work.
    ///
    /// Unlike ordinary NPC handoffs, player reactions are FIFO, de-duplicated,
    /// immediate, and cannot be overwritten before submission. If the same
    /// actor is already thinking, one queued follow-up remains necessary: that
    /// in-flight prompt cannot contain the words that arrived after it was
    /// rendered.
    ///
    /// The de-duplication is cross-lane and this lane wins: an ordinary handoff
    /// still waiting for them is *absorbed* rather than left standing, because
    /// the one turn we are queueing here drains the percept that handoff was
    /// queued for too.
    pub fn prioritize_player_reaction(
        &mut self,
        world: &World,
        actor_id: &ActorId,
        now: f64,
    ) -> bool {
        let Some(actor) = world.characters.get(actor_id) else {
            return false;
        };
        if actor.control() != Control::Llm || !world.is_present(actor_id) {
            return false;
        }
        self.priority_handoffs.retain(|queued| queued != actor_id);
        if !self.player_reactions.contains(actor_id) {
            self.player_reactions.push_back(actor_id.clone());
        }
        self.next_turn_at = self.next_turn_at.min(now);
        true
    }

    /// One tick of the turn stream: harvest, apply, submit.
    ///
    /// A single call can both apply a finished turn and submit the next one —
    /// with `minimum_delay_seconds == 0` a fresh turn is in flight after every
    /// poll.
    ///
    /// `floor_busy` and `idle` are both computed by the caller once per frame
    /// (D20): passing values instead of Python's callback keeps the floor's
    /// expiry side effects — and now the stage query — out of the scheduler's
    /// call count. The scheduler must not be able to change how often either
    /// runs.
    // The parameter list is pinned by ARCHITECTURE §2.5 (and §6, which adds
    // `transcript` and `env` to scheduler.md's signature). Bundling the borrows
    // into a context struct would only move the arguments, and it would move
    // them away from the shape the spec names.
    #[allow(clippy::too_many_arguments)]
    pub fn poll(
        &mut self,
        now: f64,
        world: &mut World,
        transcript: &mut Vec<String>,
        completions: &mut Vec<Completion>,
        floor_busy: bool,
        idle: IdleGate<'_>,
        cognition: &mut dyn Cognition,
        env: &PromptEnv,
    ) -> Vec<SchedulerEvent> {
        let mut events: Vec<SchedulerEvent> = Vec::new();

        // 1. Harvest. A held result outranks the queue — and, given the single
        //    in-flight invariant, excludes it.
        let mut result: Option<Completion> = if self.in_flight.is_some() {
            self.held_result.take()
        } else {
            None
        };
        for completion in completions.drain(..) {
            let matches = self
                .in_flight
                .as_ref()
                .is_some_and(|flight| flight.request_id == completion.request_id);
            if matches && result.is_none() {
                result = Some(completion);
                continue;
            }
            // A completion for a request we are not waiting on. Python left it
            // in the worker queue, where it would poison the *next* turn's
            // result slot; with RequestId matching (D10) we can identify it here
            // and drop it on the spot. Only reachable if a backend answers twice
            // or answers after a close (scheduler.md risk 10).
            events.push(SchedulerEvent::Status(StatusEvent::llm(
                STATE_DEGRADED,
                None,
                Some("discarded a stale LLM result".to_string()),
            )));
        }

        // 2. Gate. The floor defers APPLICATION, never submission: while it is
        //    busy the finished turn is parked and in-flight stays set, so no new
        //    turn can start behind it — but a poll with nothing in flight still
        //    submits normally, and the next speaker keeps thinking while the
        //    previous line is presented.
        if let Some(completion) = result {
            if floor_busy {
                self.held_result = Some(completion);
            } else {
                self.apply_result(now, world, transcript, completion, &mut events);
            }
        }

        // 3. Submit. The in-flight check sees the value *after* step 2 cleared
        //    it, which is how one poll can apply and submit. Whether there is
        //    anyone to submit *for* is now `select_next_actor`'s answer alone —
        //    a non-empty `order` no longer means a turn is owed.
        if self.running && self.in_flight.is_none() && now >= self.next_turn_at {
            self.submit_next_turn(now, world, idle, cognition, env, &mut events);
        }

        events
    }

    fn apply_result(
        &mut self,
        now: f64,
        world: &mut World,
        transcript: &mut Vec<String>,
        completion: Completion,
        events: &mut Vec<SchedulerEvent>,
    ) {
        // In-flight is cleared before any validation, exactly as in Python: even
        // a discarded result ends the turn.
        let mut flight = self
            .in_flight
            .take()
            .expect("a result is only harvested while a request is in flight");
        // The prompt's only remaining job is the archive below.
        let prompt = std::mem::take(&mut flight.prompt);

        // The size limit is a provider failure, not a turn (D17). Enforced
        // before the archive so an oversized reply is logged as the error it is.
        let result = match completion.result {
            Ok(reply) if reply.chars().count() > MAX_LLM_REPLY_CHARS => {
                Err(CognitionError::new(REPLY_TOO_LARGE))
            }
            other => other,
        };

        let known_actor = world.characters.get(&flight.actor_id);
        let actor_name = known_actor
            .map(|actor| actor.name().to_string())
            .unwrap_or_else(|| flight.actor_id.to_string());
        let is_current_llm = known_actor.is_some_and(|actor| {
            actor.control() == Control::Llm
                && actor.state.presence == crate::Presence::InCity
                && actor.state.presence_epoch == flight.presence_epoch
        });

        // The archive is unconditional and first — a stale result is an exchange
        // that happened, and a failed one is exactly what you want in the log
        // (`scheduler.py:205-213`). A *held* result is not archived: it has not
        // been harvested yet.
        events.push(SchedulerEvent::PromptExchange {
            actor_id: flight.actor_id.clone(),
            actor_name: actor_name.clone(),
            prompt,
            answer: result.as_ref().ok().cloned(),
            duration_seconds: completion.duration_seconds,
            // The archive gets the *detail* (`repr(error)`), not the kind: a
            // bare "LlmHttpError" cannot tell a bad key from a rate limit
            // (`scheduler.py:205-213`, prompt.md §5.2). The diagnostic below
            // keeps printing the kind, exactly as Python did.
            error: result
                .as_ref()
                .err()
                .map(|error| error.detail().to_string()),
        });

        // The request id already matched, so Python's actor-echo check is
        // subsumed — but the world can still have changed under the request, so
        // the actor-exists / still-LLM revalidation stays (scheduler.md §4.2.d).
        if !is_current_llm {
            // The drained percepts die with the result. Defensible: the actor is
            // gone (or is no longer an LLM), so there is nobody left to re-read
            // them (scheduler.md risk 3).
            events.push(SchedulerEvent::Status(StatusEvent::llm(
                STATE_DEGRADED,
                None,
                Some("discarded a stale LLM result".to_string()),
            )));
            return;
        }

        match result {
            Err(error) => self.apply_failure(now, world, &flight, &actor_name, &error, events),
            Ok(reply) => self.apply_reply(now, world, transcript, &flight, &reply, events),
        }
    }

    /// The provider never produced a turn.
    fn apply_failure(
        &mut self,
        now: f64,
        world: &mut World,
        flight: &InFlight,
        actor_name: &str,
        error: &CognitionError,
        events: &mut Vec<SchedulerEvent>,
    ) {
        let backoff = self.backoff_after_failure();
        self.next_turn_at = now + backoff;
        self.requeue_unspent_turn(&flight.actor_id, flight.lane);

        let actor = world
            .characters
            .get_mut(&flight.actor_id)
            .expect("the actor exists");
        // Let the actor perceive the events the failed prompt took away, as
        // *new* ones: prepend, so anything that arrived mid-request stays behind
        // them in chronological order. Presented percepts go back to pending the
        // same way, so the retry shows them in `since_your_last_turn` instead of
        // duplicating them into `recent_history`.
        prepend(&mut actor.state.inbox, flight.drained_events.clone());
        prepend(&mut actor.state.pending_history, flight.presented.clone());
        // Appended last, so it lands after the mid-flight arrivals too.
        actor.state.inbox.push(SYSTEM_PROVIDER_FAILED.to_string());
        // Restoring the whole drained inbox and appending a system line grows it
        // by one past the bound; across a run of failures that is unbounded, so
        // re-cap it here (code_review.md finding 2).
        actor.rebound_percepts();

        events.push(SchedulerEvent::Diagnostic(format!(
            "[smart actors] LLM request for {actor_name} failed: {error}"
        )));
        events.push(SchedulerEvent::Status(StatusEvent::llm(
            STATE_DEGRADED,
            Some(flight.actor_id.clone()),
            Some(format!(
                "provider request failed; retrying in {} seconds",
                format_g(backoff)
            )),
        )));
    }

    /// `delay, 2·delay, 4·delay, …` capped — with a floor that keeps a zero
    /// development delay from spinning (`scheduler.py:231-237`).
    fn backoff_after_failure(&mut self) -> f64 {
        self.provider_failures += 1;
        // `powi` saturates to +inf long before it overflows, and `min` then
        // takes the cap: a very long outage cannot wrap around to a short retry.
        let exponential = self.minimum_delay_seconds * 2f64.powi(self.provider_failures as i32 - 1);
        let backoff = self.maximum_backoff_seconds.min(exponential);
        backoff.max(self.maximum_backoff_seconds.min(1.0))
    }

    /// The provider produced a turn: graduate its percepts, then apply it.
    fn apply_reply(
        &mut self,
        now: f64,
        world: &mut World,
        transcript: &mut Vec<String>,
        flight: &InFlight,
        reply: &str,
        events: &mut Vec<SchedulerEvent>,
    ) {
        self.provider_failures = 0;
        // A player reaction may have arrived while this request was in flight.
        // Preserve its immediate wake-up instead of replacing it with the
        // ordinary inter-turn delay as the completed background turn applies.
        self.next_turn_at = if self.player_reactions.is_empty() {
            now + self.minimum_delay_seconds
        } else {
            now
        };

        let actor = world
            .characters
            .get_mut(&flight.actor_id)
            .expect("the actor exists");
        // Identical to the name the caller resolved: this path is only reached
        // once the actor has been revalidated as a live LLM character.
        let actor_name = actor.name().to_string();
        // The turn happened, so the percepts it presented become recollection —
        // before the reply's own lines are remembered after them.
        actor.absorb_presented_history(&flight.presented);

        let (actions, errors) = parse_reply(reply);
        for error in &errors {
            let actor = world
                .characters
                .get_mut(&flight.actor_id)
                .expect("the actor exists");
            actor
                .state
                .inbox
                .push(format!("system: your last output was invalid: {error}"));
            events.push(SchedulerEvent::Diagnostic(format!(
                "[smart actors] {actor_name}: {error}"
            )));
        }

        // One reply is one turn, and `seize` may not be a wordless one
        // (`law_and_order.md` M4). Cleared here so the marker means "in *this*
        // reply", never "at some point earlier today".
        world.spoke_this_turn = None;

        for (verb, args) in actions {
            let value = Value::Object(args.clone());
            let line = match apply_action(world, &flight.actor_id, &verb, &value) {
                Ok(line) => line,
                Err(error) => {
                    // The final boundary: arbitrary model output never takes the
                    // process down, and the actor gets told so it can correct
                    // itself next turn. Later actions in the same reply still run.
                    let actor = world
                        .characters
                        .get_mut(&flight.actor_id)
                        .expect("the actor exists");
                    actor.state.inbox.push(format!(
                        "system: your action \"{verb} {}\" failed: {error}",
                        render_args(&args)
                    ));
                    events.push(SchedulerEvent::Diagnostic(format!(
                        "[smart actors] {actor_name}: {verb} failed: {error}"
                    )));
                    continue;
                }
            };

            // Turn-taking: an addressed `say` hands the next selection slot to
            // the addressee, so a two-way exchange does not braid through the
            // global round robin — and `tell_way` (M5) is addressed the same
            // way: the way just given is news its holder should get to act on,
            // on stage or off. Deliberately not immediate — the inter-turn
            // delay and the floor still govern *when*, only selection changes.
            // `prioritize` rejects the player itself, so this is best-effort;
            // several targeted says in one reply all queue, oldest first.
            let addressed = match verb.as_str() {
                "say" => args.get("target"),
                "tell_way" => args.get("person"),
                _ => None,
            };
            if let Some(Value::String(target)) = addressed
                && let Ok(target_id) = ActorId::new(target.clone())
            {
                self.prioritize(world, &target_id, false, now);
            }

            // Waiting is a real, validated choice — but it is not a world event
            // and must not make the transcript grow forever.
            if verb != "wait" {
                transcript.push(line);
            }
        }

        // The `system:` lines pushed above (invalid output, failed actions) can
        // carry the inbox past its bound when mid-flight percepts already filled
        // it; keep the invariant every path shares.
        world
            .characters
            .get_mut(&flight.actor_id)
            .expect("the actor exists")
            .rebound_percepts();

        events.push(SchedulerEvent::Status(StatusEvent::llm(
            STATE_IDLE,
            Some(flight.actor_id.clone()),
            None,
        )));
    }

    fn submit_next_turn(
        &mut self,
        now: f64,
        world: &mut World,
        idle: IdleGate<'_>,
        cognition: &mut dyn Cognition,
        env: &PromptEnv,
        events: &mut Vec<SchedulerEvent>,
    ) {
        // Selection happens — and the rotation advances / the queued handoff is
        // consumed — BEFORE the validity check: a skipped actor still burns its
        // turn, and so do a failed render and a refused submit. Intentional
        // (scheduler.md risk 8).
        let Some((actor_id, lane)) = self.select_next_actor(idle) else {
            // Nobody may think right now: the stage is empty, or the player is
            // mid-utterance. `next_turn_at` is deliberately left where it is —
            // in the past — so the first poll after someone walks into the
            // stage submits at once. An NPC should already be mid-thought when
            // you arrive, not boot when you look at them.
            return;
        };
        let Some(actor) = world.characters.get(&actor_id) else {
            // Silently skipped, with no delay change, so the very next poll
            // selects the following actor. Only reachable if the world mutated
            // after construction — `order` is frozen.
            return;
        };
        if actor.control() != Control::Llm || !world.is_present(&actor_id) {
            return;
        }
        let presence_epoch = actor.state.presence_epoch;
        let actor_name = actor.name().to_string();
        let output_token_budget = actor
            .lore()
            .map(|_| actor.significance().output_token_budget());
        // Taken before the render, because the render is what empties the inbox.
        let drained_events = actor.inbox().to_vec();

        let (prompt, presented) = match render_prompt_and_drain(world, &actor_id, env) {
            Ok(rendered) => rendered,
            Err(error) => {
                // The renderer already restored the inbox and pending history
                // itself (`prompt.py:257-260`), so we must NOT restore again.
                let actor = world
                    .characters
                    .get_mut(&actor_id)
                    .expect("the actor exists");
                actor.state.inbox.push(SYSTEM_PROMPT_FAILED.to_string());
                // The renderer restored the (possibly full) inbox; the appended
                // line then pushes one past the bound — re-cap it so repeated
                // prompt failures cannot grow it without limit.
                actor.rebound_percepts();
                self.requeue_unspent_turn(&actor_id, lane);
                self.next_turn_at = now + self.minimum_delay_seconds.max(1.0);
                events.push(SchedulerEvent::Diagnostic(format!(
                    "[smart actors] prompt for {actor_name} failed: {error}"
                )));
                events.push(SchedulerEvent::Status(StatusEvent::llm(
                    STATE_DEGRADED,
                    Some(actor_id.clone()),
                    Some("prompt rendering failed".to_string()),
                )));
                return;
            }
        };

        match cognition.request_with_budget(prompt.clone(), output_token_budget) {
            Ok(request_id) => {
                // Every lane lands here, and every lane clears news: an actor who
                // has just answered the player has been shown the same world an
                // idle turn would have shown them.
                self.submitted = Some(actor_id.clone());
                self.in_flight = Some(InFlight {
                    actor_id: actor_id.clone(),
                    presence_epoch,
                    request_id,
                    lane,
                    drained_events,
                    presented,
                    prompt,
                });
                events.push(SchedulerEvent::Status(StatusEvent::llm(
                    STATE_THINKING,
                    Some(actor_id),
                    None,
                )));
            }
            Err(busy) => {
                let actor = world
                    .characters
                    .get_mut(&actor_id)
                    .expect("the actor exists");
                prepend(&mut actor.state.inbox, drained_events);
                prepend(&mut actor.state.pending_history, presented);
                actor.state.inbox.push(SYSTEM_WORKER_BUSY.to_string());
                // Same restore-and-append as the provider-failure path: re-cap so
                // a run of busy rejections cannot grow the buffers past the bound.
                actor.rebound_percepts();
                self.requeue_unspent_turn(&actor_id, lane);
                self.next_turn_at = now + self.minimum_delay_seconds.max(1.0);
                events.push(SchedulerEvent::Diagnostic(format!(
                    "[smart actors] could not queue {actor_name}'s turn: {busy}"
                )));
                events.push(SchedulerEvent::Status(StatusEvent::llm(
                    STATE_DEGRADED,
                    Some(actor_id),
                    Some("cognition worker is busy".to_string()),
                )));
            }
        }
    }

    /// Protected player reactions win first, then the ordinary priority
    /// handoffs (oldest first), then the gated round robin. Neither of the
    /// first two advances the round robin, so the rotation resumes exactly
    /// where it left off (`scheduler.py:360-367`).
    ///
    /// `None` means no turn is owed at all — the lane that would have supplied
    /// one is empty or closed. It is the poll that buys nothing, and not
    /// spending it is the whole feature.
    fn select_next_actor(&mut self, idle: IdleGate<'_>) -> Option<(ActorId, TurnLane)> {
        // Ungated, always: the player spoke, so someone answers.
        if let Some(actor_id) = self.player_reactions.pop_front() {
            return Some((actor_id, TurnLane::PlayerReaction));
        }
        // The player is still composing. The lane he needs is the one above,
        // and the ordinary slot's occupant — an NPC handoff, a sound nudge — is
        // exactly the two seconds of irrelevant thinking his words would
        // otherwise queue behind. It is a sticky slot; it keeps until he stops.
        if idle.is_suppressed() {
            return None;
        }
        // Ungated by proximity: an addressed `say` or an audible sound reached
        // them. This is also the only way an ambient NPC ever thinks, and it
        // must stay that way.
        if let Some(actor_id) = self.priority_handoffs.pop_front() {
            return Some((actor_id, TurnLane::Handoff));
        }
        self.next_idle_actor(idle)
            .map(|actor_id| (actor_id, TurnLane::Idle))
    }

    /// Scan the rotation forward for the first actor the gate admits, bounded by
    /// one lap.
    ///
    /// Scanning rather than rebuilding `order` is what preserves both the
    /// rotation's fairness and its significance weighting, and it is what keeps
    /// `order` frozen at construction (the invariant the struct's comment pins).
    fn next_idle_actor(&mut self, idle: IdleGate<'_>) -> Option<ActorId> {
        // No modulo on an empty rotation: the loop simply never runs.
        for offset in 0..self.order.len() {
            let index = (self.round_robin_index + offset) % self.order.len();
            if !idle.allows(&self.order[index]) {
                continue;
            }
            self.round_robin_index = (index + 1) % self.order.len();
            return Some(self.order[index].clone());
        }
        None
    }

    /// Put a turn that never reached the model back at the head of the lane
    /// that selected it, so the retry outranks whatever queued behind it while
    /// the prompt was out.
    ///
    /// An idle turn goes back to nobody: the rotation that produced it comes
    /// around again on its own, and a failure must not buy it a seat in a
    /// lane it was never in.
    fn requeue_unspent_turn(&mut self, actor_id: &ActorId, lane: TurnLane) {
        match lane {
            TurnLane::PlayerReaction => self.requeue_player_reaction_front(actor_id),
            TurnLane::Handoff => self.requeue_handoff_front(actor_id),
            TurnLane::Idle => {}
        }
    }

    /// Put a player reaction that never reached the model back at the head of
    /// its lane.
    ///
    /// Cross-lane like the two `prioritize`s, and for a reason only the
    /// requeues have: the actor was *off* both queues while their prompt was
    /// out, so an ordinary handoff could queue them in the meantime. The retry
    /// renders after it and drains it, so leaving that slot standing would owe
    /// them a second, contentless turn.
    fn requeue_player_reaction_front(&mut self, actor_id: &ActorId) {
        self.priority_handoffs.retain(|queued| queued != actor_id);
        if let Some(index) = self
            .player_reactions
            .iter()
            .position(|queued| queued == actor_id)
        {
            self.player_reactions.remove(index);
        }
        self.player_reactions.push_front(actor_id.clone());
    }

    /// Put an ordinary handoff that never reached the model back at the head
    /// of its lane.
    ///
    /// Off stage this lane is the only way its occupant is ever selected — the
    /// idle rotation never admits them — so a slot consumed by a provider
    /// blip, a failed render or a busy worker used to leave the restored
    /// percept in an inbox nothing would ever read again: one transient
    /// failure ended an off-stage exchange outright. The cross-lane rule runs
    /// the other way from the requeue above, exactly as in
    /// `prioritize_player_reaction`: a player reaction queued while this
    /// prompt was out *absorbs* the retry, because the one turn it owes drains
    /// the restored inbox too.
    fn requeue_handoff_front(&mut self, actor_id: &ActorId) {
        if self.player_reactions.contains(actor_id) {
            return;
        }
        self.priority_handoffs.retain(|queued| queued != actor_id);
        self.priority_handoffs.push_front(actor_id.clone());
    }
}

/// The LLM cast in turn order: the world's insertion order (D12), which is what
/// makes the rotation Sven → Conny → Ilse rather than the map's sorted ids.
pub fn llm_turn_order(world: &World) -> Vec<ActorId> {
    world
        .roster
        .iter()
        // Presence is deliberately checked when a slot is submitted, not when
        // this frozen order is built. Road actors begin beyond the walls and
        // must acquire idle turns after a later visit without reconstructing
        // the scheduler.
        .filter(|actor_id| world.characters[*actor_id].control() == Control::Llm)
        .cloned()
        .collect()
}

/// Significance-aware autonomous turn stream, for an **ungated** rotation.
///
/// Major lore actors receive four slots for each minor slot. Ambient actors
/// receive no idle slot at all, but `prioritize` and the protected player
/// reaction lane can still schedule them immediately. Non-lore fixtures retain
/// one slot so compact tests and custom seeds keep their historic behavior.
///
/// These weights answer *"who, out of 500 people, deserves scarce global
/// compute?"*, and Ambient ×0 is the right answer to that question: the ambient
/// cast is the people you will never meet. See [`stage_turn_order`] for the
/// question a gated rotation asks instead.
pub fn background_turn_order(world: &World) -> Vec<ActorId> {
    weighted_turn_order(world, |significance| match significance {
        None => 1,
        Some(Significance::Major) => 4,
        Some(Significance::Minor) => 1,
        Some(Significance::Ambient) => 0,
    })
}

/// The same stream, weighted for a rotation the stage has already filtered.
///
/// With the gate in, the rotation answers a different question — *"who, out of
/// the six people standing in front of the player, thinks next?"* — and there
/// Ambient ×0 is backwards. The market crowd around you **is** ambient; under
/// the ungated weights they would be exactly the statues the gate is supposed to
/// prevent.
///
/// So the weights flatten to Major 3 / Minor 2 / Ambient 1, and the
/// per-significance completion caps ([`Significance::output_token_budget`]) keep
/// the difference in the bill instead: an ambient fishmonger beside you should
/// live, just in fewer tokens.
pub fn stage_turn_order(world: &World) -> Vec<ActorId> {
    weighted_turn_order(world, |significance| match significance {
        None => 1,
        Some(Significance::Major) => 3,
        Some(Significance::Minor) => 2,
        Some(Significance::Ambient) => 1,
    })
}

/// Interleave the cast by weight: one pass per layer, so a ×4 actor is spread
/// across the rotation rather than stacked at their roster position.
fn weighted_turn_order(
    world: &World,
    copies: impl Fn(Option<Significance>) -> usize,
) -> Vec<ActorId> {
    let weighted: Vec<_> = world
        .roster
        .iter()
        .filter_map(|actor_id| {
            let actor = &world.characters[actor_id];
            if actor.control() != Control::Llm {
                return None;
            }
            let copies = copies(actor.lore().map(|profile| profile.significance));
            (copies > 0).then_some((actor_id, copies))
        })
        .collect();
    let layers = weighted
        .iter()
        .map(|(_, copies)| *copies)
        .max()
        .unwrap_or(0);
    let mut order = Vec::new();
    for layer in 0..layers {
        order.extend(
            weighted
                .iter()
                .filter(|(_, copies)| *copies > layer)
                .map(|(actor_id, _)| (*actor_id).clone()),
        );
    }
    order
}

fn prepend(target: &mut Vec<String>, mut front: Vec<String>) {
    front.append(target);
    *target = front;
}

/// The failed action's args, as the actor gets to see them.
///
/// Python interpolated the `dict` itself (`scheduler.py:277-279`), so the line
/// carries Python literal syntax — `{'item_id': ['bad'], 'target': None}` — not
/// JSON. It lands in the actor's inbox and is re-rendered verbatim as
/// `since_your_last_turn` on the next turn, so it is prompt bytes: it has to be
/// `str(dict)`.
///
/// One residual divergence, which the syntax fix cannot reach: `serde_json::Map`
/// is a `BTreeMap` (the type is pinned by ARCHITECTURE §2.4, and serde_json's
/// `preserve_order` feature would unify across the whole workspace), so the keys
/// come out **sorted** where Python's dict kept the model's document order —
/// `{'extra': 1, 'text': 'hi'}` for a reply that wrote `text` first. Same
/// characters, different order; only a multi-key *failed* action shows it.
fn render_args(args: &Map<String, Value>) -> String {
    py_repr_map(args)
}

/// Python's `f"{value:g}"` (D30): 6 significant digits, trailing zeros trimmed.
///
/// The degraded status text is wire format, so a one-second backoff has to read
/// "retrying in 1 seconds" — not "retrying in 1.0 seconds", and not the full
/// float noise Rust's default `{}` would print for, say, 0.1 + 0.2.
fn format_g(value: f64) -> String {
    const PRECISION: i32 = 6;
    if !value.is_finite() {
        return format!("{value}");
    }
    // `{:.5e}` is 6 significant digits, and the exponent it reports is the
    // post-rounding one — exactly what CPython's `%g` branches on.
    let scientific = format!("{:.*e}", (PRECISION - 1) as usize, value);
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("`{:e}` always emits an exponent");
    let exponent: i32 = exponent.parse().expect("the exponent is an integer");
    if !(-4..PRECISION).contains(&exponent) {
        let sign = if exponent < 0 { '-' } else { '+' };
        return format!("{}e{sign}{:02}", trim_zeros(mantissa), exponent.abs());
    }
    let decimals = (PRECISION - 1 - exponent).max(0) as usize;
    trim_zeros(&format!("{value:.decimals$}")).to_string()
}

fn trim_zeros(text: &str) -> &str {
    if !text.contains('.') {
        return text;
    }
    text.trim_end_matches('0').trim_end_matches('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::{
        Character, CharacterSheet, LoreProfile, PlanningWard, Significance, Vec3,
        traits::CognitionBusy,
    };

    fn lore(significance: Significance) -> LoreProfile {
        LoreProfile {
            significance,
            planning_ward: PlanningWard::Fabric,
            age: 30,
            gender: "f".into(),
            occupation_id: Some("smith".into()),
            occupation_display: Some("Smith".into()),
            title: Some("Smith".into()),
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district: "The Gradine".into(),
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances: Vec::new(),
            conditions: Vec::new(),
            home: None,
            home_point_m: None,
            core_character_description: "You work carefully.".into(),
            extended_character_description: String::new(),
            curiosity: None,
            generated: false,
        }
    }

    fn lore_character(id: &str, significance: Significance) -> Character {
        Character::from_sheet(CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: ActorId::from_raw(id),
            name: id.into(),
            control: Control::Llm,
            back_story: "You work carefully.".into(),
            location_description: "The Gradine".into(),
            appearance: Default::default(),
            voice_key: Some("ilse".into()),
            position_m: Vec3::ZERO,
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: crate::GOAL_NONE.into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: Some(lore(significance)),
            presence: crate::Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::EconomicClass::Resident,
        })
    }

    #[derive(Default)]
    struct BudgetCognition {
        requests: Vec<(String, Option<u32>)>,
    }

    impl Cognition for BudgetCognition {
        fn request(&mut self, _prompt: String) -> Result<RequestId, CognitionBusy> {
            panic!("the scheduler should use the budget-aware boundary")
        }

        fn request_with_budget(
            &mut self,
            prompt: String,
            max_output_tokens: Option<u32>,
        ) -> Result<RequestId, CognitionBusy> {
            self.requests.push((prompt, max_output_tokens));
            Ok(RequestId(0))
        }
    }

    #[test]
    fn backoff_doubles_from_the_delay_and_stops_at_the_cap() {
        // delay 1, cap 8 → 1, 2, 4, 8, 8, …
        let mut scheduler = NpcScheduler::new(Vec::new(), 1.0, 8.0, 0.0);
        let seen: Vec<f64> = (0..5).map(|_| scheduler.backoff_after_failure()).collect();
        assert_eq!(seen, [1.0, 2.0, 4.0, 8.0, 8.0]);
    }

    #[test]
    fn a_zero_delay_still_backs_off_a_whole_second() {
        // The anti-spin floor: min(1.0, cap) — otherwise delay 0 retries forever.
        let mut scheduler = NpcScheduler::new(Vec::new(), 0.0, 60.0, 0.0);
        assert_eq!(scheduler.backoff_after_failure(), 1.0);
        assert_eq!(scheduler.backoff_after_failure(), 1.0);
    }

    #[test]
    fn the_cap_is_normalized_to_at_least_one_second_and_the_delay() {
        // cap 0 → 1; cap below the delay → the delay.
        let scheduler = NpcScheduler::new(Vec::new(), 0.0, 0.0, 0.0);
        assert_eq!(scheduler.maximum_backoff_seconds, 1.0);
        let scheduler = NpcScheduler::new(Vec::new(), 30.0, 5.0, 0.0);
        assert_eq!(scheduler.maximum_backoff_seconds, 30.0);
        // A negative delay is clamped rather than fatal.
        let scheduler = NpcScheduler::new(Vec::new(), -5.0, 60.0, 0.0);
        assert_eq!(scheduler.minimum_delay_seconds, 0.0);
    }

    #[test]
    fn g_formatting_matches_python() {
        assert_eq!(format_g(1.0), "1");
        assert_eq!(format_g(2.0), "2");
        assert_eq!(format_g(60.0), "60");
        assert_eq!(format_g(2.5), "2.5");
        assert_eq!(format_g(0.5), "0.5");
        assert_eq!(format_g(1.0 / 3.0), "0.333333");
        assert_eq!(format_g(1_234_567.0), "1.23457e+06");
    }

    #[test]
    fn autonomous_order_weights_major_minor_and_omits_ambient() {
        let mut world = World::new();
        world.add_character(lore_character("major", Significance::Major));
        world.add_character(lore_character("minor", Significance::Minor));
        world.add_character(lore_character("ambnt", Significance::Ambient));

        assert_eq!(
            background_turn_order(&world)
                .iter()
                .map(ActorId::as_str)
                .collect::<Vec<_>>(),
            ["major", "minor", "major", "major", "major"]
        );
        assert_eq!(
            llm_turn_order(&world)
                .iter()
                .map(ActorId::as_str)
                .collect::<Vec<_>>(),
            ["major", "minor", "ambnt"]
        );
    }

    #[test]
    fn the_stage_order_flattens_the_weights_and_lets_ambient_in() {
        let mut world = World::new();
        world.add_character(lore_character("major", Significance::Major));
        world.add_character(lore_character("minor", Significance::Minor));
        world.add_character(lore_character("ambnt", Significance::Ambient));

        // Major 3 / Minor 2 / Ambient 1: the crowd in front of you all lives,
        // and the interleave still favours the people worth talking to.
        assert_eq!(
            stage_turn_order(&world)
                .iter()
                .map(ActorId::as_str)
                .collect::<Vec<_>>(),
            ["major", "minor", "ambnt", "major", "minor", "major"]
        );
    }

    #[test]
    fn frozen_orders_keep_road_actors_who_enter_after_construction() {
        let mut road_actor = lore_character("road", Significance::Minor);
        road_actor.state.presence = crate::Presence::BeyondTheWalls;
        let mut world = World::new();
        world.add_character(road_actor);

        let order = stage_turn_order(&world);
        assert_eq!(
            order.iter().map(ActorId::as_str).collect::<Vec<_>>(),
            ["road", "road"]
        );
        let mut scheduler = NpcScheduler::new(order, 0.0, 60.0, 0.0);

        // While absent, submission performs the live presence check and burns
        // the slot. Once the same stable actor enters, the already-frozen
        // rotation can select it normally.
        assert!(!world.is_present(&ActorId::from_raw("road")));
        world
            .transition_presence(
                &[ActorId::from_raw("road")],
                crate::Presence::InCity,
                &[(ActorId::from_raw("road"), Vec3::ZERO)]
                    .into_iter()
                    .collect(),
            )
            .unwrap();
        assert_eq!(
            scheduler.select_next_actor(IdleGate::All),
            Some((ActorId::from_raw("road"), TurnLane::Idle))
        );
    }

    #[test]
    fn a_predeparture_completion_stays_stale_after_the_actor_reenters() {
        let road = ActorId::from_raw("road");
        let mut world = World::new();
        world.add_character(lore_character("road", Significance::Minor));
        {
            let actor = world.characters.get_mut(&road).unwrap();
            actor.state.memories.push("Old road memory".into());
            actor.state.recent_history.push("Old road history".into());
            actor.notify_percept("Unread city news");
        }

        let mut scheduler = NpcScheduler::new(vec![road.clone()], 0.0, 60.0, 0.0);
        scheduler.start(0.0);
        let env = PromptEnv::new(
            include_str!("../../../assets/prompts/turn.j2"),
            include_str!("../../../assets/prompts/night.j2"),
            include_str!("../../../assets/prompts/strings.toml"),
        )
        .unwrap();
        let mut cognition = BudgetCognition::default();
        scheduler.poll(
            0.0,
            &mut world,
            &mut Vec::new(),
            &mut Vec::new(),
            false,
            IdleGate::All,
            &mut cognition,
            &env,
        );
        assert_eq!(scheduler.in_flight_actor_id(), Some(&road));
        assert!(world.characters[&road].inbox().is_empty());

        world
            .transition_presence(
                std::slice::from_ref(&road),
                crate::Presence::BeyondTheWalls,
                &Default::default(),
            )
            .unwrap();
        scheduler.actors_departed(std::slice::from_ref(&road));
        world
            .transition_presence(
                std::slice::from_ref(&road),
                crate::Presence::InCity,
                &[(road.clone(), Vec3::ZERO)].into_iter().collect(),
            )
            .unwrap();
        assert_eq!(world.presence_epoch(&road), Some(2));

        // Closing prevents a replacement turn from being submitted after the
        // stale result is harvested; it does not suppress applying/archiving
        // the request that was already in flight.
        scheduler.close();
        let mut completions = vec![Completion {
            request_id: RequestId(0),
            result: Ok("remember {\"memory\": \"Poison from the prior visit\"}".into()),
            duration_seconds: 0.25,
        }];
        let mut transcript = Vec::new();
        let events = scheduler.poll(
            1.0,
            &mut world,
            &mut transcript,
            &mut completions,
            false,
            IdleGate::All,
            &mut cognition,
            &env,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            SchedulerEvent::PromptExchange {
                actor_id,
                answer: Some(answer),
                ..
            } if actor_id == &road && answer.contains("Poison")
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            SchedulerEvent::Status(status)
                if status.message.as_deref() == Some("discarded a stale LLM result")
        )));
        assert_eq!(world.characters[&road].memories(), ["Old road memory"]);
        assert_eq!(world.characters[&road].recent_history(), ["Old road history"]);
        assert!(world.characters[&road].inbox().is_empty());
        assert!(world.characters[&road].state.pending_history.is_empty());
        assert!(transcript.is_empty());
    }

    #[test]
    fn the_idle_lane_skips_actors_off_stage_and_keeps_the_rotation_fair() {
        let mut world = World::new();
        for id in ["one", "two", "tre"] {
            world.add_character(lore_character(id, Significance::Minor));
        }
        let order = stage_turn_order(&world);
        // Minor ×2, so each actor appears twice: the scan must not mistake the
        // second copy for a fresh actor.
        assert_eq!(order.len(), 6);
        let mut scheduler = NpcScheduler::new(order, 0.0, 60.0, 0.0);

        let stage: BTreeSet<ActorId> = [ActorId::from_raw("tre")].into_iter().collect();
        let gate = IdleGate::Stage(&stage);
        // Only "tre" is near the player, so the rotation lands on "tre" every
        // time — skipping past "one" and "two" rather than stalling on them.
        for _ in 0..3 {
            assert_eq!(
                scheduler.select_next_actor(gate),
                Some((ActorId::from_raw("tre"), TurnLane::Idle))
            );
        }
        // An empty stage buys nothing, so it costs nothing.
        let empty = BTreeSet::new();
        assert_eq!(scheduler.select_next_actor(IdleGate::Stage(&empty)), None);
        // Ungated, the rotation resumes where the gated scan left it.
        assert_eq!(
            scheduler.select_next_actor(IdleGate::All),
            Some((ActorId::from_raw("one"), TurnLane::Idle))
        );
    }

    /// Law-and-order M0: the offerer's wake-up after a silent player accept.
    /// The engine hands them the ordinary priority slot, and that lane is
    /// deliberately ungated by proximity — an offerer who watched the player
    /// sprint out of the stage must still get the turn that reads the
    /// acceptance percept, or the theft never registers.
    #[test]
    fn a_priority_handoff_outranks_an_empty_stage() {
        let mut world = World::new();
        world.add_character(lore_character("offrr", Significance::Ambient));
        let mut scheduler = NpcScheduler::new(stage_turn_order(&world), 0.0, 60.0, 0.0);

        // The player accepted in silence and left: no speech, no reaction lane,
        // and the stage they left behind is empty.
        assert!(scheduler.prioritize(&world, &ActorId::from_raw("offrr"), false, 0.0));
        let empty = BTreeSet::new();
        assert_eq!(
            scheduler.select_next_actor(IdleGate::Stage(&empty)),
            Some((ActorId::from_raw("offrr"), TurnLane::Handoff))
        );
        // Once answered, the empty stage buys nothing again.
        assert_eq!(scheduler.select_next_actor(IdleGate::Stage(&empty)), None);
    }

    #[test]
    fn a_composing_player_suppresses_every_lane_but_his_own() {
        let mut world = World::new();
        world.add_character(lore_character("major", Significance::Major));
        world.add_character(lore_character("ambnt", Significance::Ambient));
        let mut scheduler = NpcScheduler::new(stage_turn_order(&world), 0.0, 60.0, 0.0);

        // A sound nudge has handed the ordinary slot to the ambient NPC…
        assert!(scheduler.prioritize(&world, &ActorId::from_raw("ambnt"), true, 0.0));
        // …but the player is mid-utterance, so nothing starts: neither that
        // handoff nor the round robin may spend the slot his words are about to
        // need.
        assert_eq!(scheduler.select_next_actor(IdleGate::Suppressed), None);

        // The protected lane is the exception, and it still outranks everything.
        assert!(scheduler.prioritize_player_reaction(&world, &ActorId::from_raw("major"), 0.0));
        assert_eq!(
            scheduler.select_next_actor(IdleGate::Suppressed),
            Some((ActorId::from_raw("major"), TurnLane::PlayerReaction))
        );
        // The nudge was only deferred, never dropped.
        assert_eq!(
            scheduler.select_next_actor(IdleGate::All),
            Some((ActorId::from_raw("ambnt"), TurnLane::Handoff))
        );
    }

    /// The two lanes are a priority split, not a second seat. An actor nudged
    /// in both used to pop off each in turn: the first turn drained her whole
    /// inbox answering the player, and the second was a paid provider call with
    /// an empty `since_your_last_turn` that could also make her speak again
    /// unprompted. The rotation is empty here, so every `Some` below is a lane.
    #[test]
    fn an_actor_queued_in_both_lanes_takes_a_single_turn() {
        let mut world = World::new();
        world.add_character(lore_character("major", Significance::Major));
        let mut scheduler = NpcScheduler::new(Vec::new(), 0.0, 60.0, 0.0);
        let major = ActorId::from_raw("major");

        // The world nudges her first (a pocket percept in plain sight), and the
        // player speaks to her a second later.
        assert!(scheduler.prioritize(&world, &major, true, 0.0));
        assert!(scheduler.prioritize_player_reaction(&world, &major, 1.0));
        assert_eq!(
            scheduler.select_next_actor(IdleGate::All),
            Some((major.clone(), TurnLane::PlayerReaction)),
            "the player's lane still wins the promotion"
        );
        assert_eq!(
            scheduler.select_next_actor(IdleGate::All),
            None,
            "the nudge was absorbed by that turn, not left owing a second one"
        );

        // …and the other way round: the player wakes her, then a sound reaches
        // her before she is selected.
        assert!(scheduler.prioritize_player_reaction(&world, &major, 2.0));
        assert!(
            scheduler.prioritize(&world, &major, true, 2.0),
            "the handoff is accepted — she will answer it — it just buys no slot"
        );
        assert_eq!(
            scheduler.select_next_actor(IdleGate::All),
            Some((major.clone(), TurnLane::PlayerReaction))
        );
        assert_eq!(scheduler.select_next_actor(IdleGate::All), None);

        // The one window in which she is on neither queue is while her prompt
        // is out; a handoff landing there must not survive the retry either.
        assert!(scheduler.prioritize(&world, &major, false, 3.0));
        scheduler.requeue_player_reaction_front(&major);
        assert_eq!(
            scheduler.select_next_actor(IdleGate::All),
            Some((major.clone(), TurnLane::PlayerReaction))
        );
        assert_eq!(scheduler.select_next_actor(IdleGate::All), None);

        // The mirror window: her *ordinary* prompt is out when the player
        // speaks to her. The protected slot absorbs the failed handoff's
        // retry rather than letting it stand beside it.
        assert!(scheduler.prioritize_player_reaction(&world, &major, 4.0));
        scheduler.requeue_handoff_front(&major);
        assert_eq!(
            scheduler.select_next_actor(IdleGate::All),
            Some((major.clone(), TurnLane::PlayerReaction))
        );
        assert_eq!(scheduler.select_next_actor(IdleGate::All), None);
    }

    #[test]
    fn an_ambient_actor_can_react_without_an_idle_order_and_gets_the_small_budget() {
        let mut world = World::new();
        let ambient = ActorId::from_raw("ambnt");
        world.add_character(lore_character("ambnt", Significance::Ambient));
        let mut scheduler = NpcScheduler::new(Vec::new(), 0.0, 60.0, 0.0);
        scheduler.start(0.0);
        assert!(scheduler.prioritize_player_reaction(&world, &ambient, 0.0));

        let env = PromptEnv::new(
            include_str!("../../../assets/prompts/turn.j2"),
            include_str!("../../../assets/prompts/night.j2"),
            include_str!("../../../assets/prompts/strings.toml"),
        )
        .unwrap();
        let mut cognition = BudgetCognition::default();
        // The gate is the tightest one there is — an empty stage — and the
        // reaction lane sails straight through it.
        let empty = BTreeSet::new();
        let events = scheduler.poll(
            0.0,
            &mut world,
            &mut Vec::new(),
            &mut Vec::new(),
            false,
            IdleGate::Stage(&empty),
            &mut cognition,
            &env,
        );
        assert!(events.iter().any(|event| matches!(
            event,
            SchedulerEvent::Status(status) if status.state == STATE_THINKING
        )));
        assert_eq!(cognition.requests.len(), 1);
        assert_eq!(
            cognition.requests[0].1,
            Some(Significance::Ambient.output_token_budget())
        );
    }

    /// An off-stage NPC is selected exclusively through the handoff lane — the
    /// gated rotation never admits them — so a provider blip on that one turn
    /// used to consume the slot for good: the restored percept sat in an inbox
    /// nothing would ever read, and the far-ward exchange died silently.
    #[test]
    fn an_off_stage_handoff_survives_a_provider_failure() {
        let mut world = World::new();
        let ambient = ActorId::from_raw("ambnt");
        world.add_character(lore_character("ambnt", Significance::Ambient));
        world
            .characters
            .get_mut(&ambient)
            .unwrap()
            .notify_percept("Bertram says to you: the cart is stuck");
        let mut scheduler = NpcScheduler::new(stage_turn_order(&world), 0.0, 60.0, 0.0);
        scheduler.start(0.0);

        // An addressed `say` reached them across the city: the ordinary lane,
        // not the protected one.
        assert!(scheduler.prioritize(&world, &ambient, true, 0.0));

        let env = PromptEnv::new(
            include_str!("../../../assets/prompts/turn.j2"),
            include_str!("../../../assets/prompts/night.j2"),
            include_str!("../../../assets/prompts/strings.toml"),
        )
        .unwrap();
        let mut cognition = BudgetCognition::default();
        let empty = BTreeSet::new();
        scheduler.poll(
            0.0,
            &mut world,
            &mut Vec::new(),
            &mut Vec::new(),
            false,
            IdleGate::Stage(&empty),
            &mut cognition,
            &env,
        );
        assert_eq!(scheduler.in_flight_actor_id(), Some(&ambient));

        // The provider fails — a network blip, not a turn.
        let mut completions = vec![Completion {
            request_id: RequestId(0),
            result: Err(CognitionError::new("TimeoutError")),
            duration_seconds: 0.1,
        }];
        scheduler.poll(
            1.0,
            &mut world,
            &mut Vec::new(),
            &mut completions,
            false,
            IdleGate::Stage(&empty),
            &mut cognition,
            &env,
        );
        assert_eq!(scheduler.in_flight_actor_id(), None);
        assert_eq!(scheduler.priority_actor_id(), Some(&ambient));

        // Past the backoff, the same empty stage: the retry goes out, and its
        // prompt still carries the percept the failed one restored.
        scheduler.poll(
            2.0,
            &mut world,
            &mut Vec::new(),
            &mut Vec::new(),
            false,
            IdleGate::Stage(&empty),
            &mut cognition,
            &env,
        );
        assert_eq!(scheduler.in_flight_actor_id(), Some(&ambient));
        assert_eq!(cognition.requests.len(), 2);
        assert!(cognition.requests[1].0.contains("the cart is stuck"));
        assert!(cognition.requests[1].0.contains(SYSTEM_PROVIDER_FAILED));
    }
}
