//! The Night Office (movement M6, `features/implemented/movement/05_the_llm_seam.md` §4):
//! the second cognition lane.
//!
//! Once a game day, at their own bedtime, a character may rewrite their own
//! agenda — settle what they learned, change what they are set on, and move one
//! leg of tomorrow. Thirty-one Majors reflect individually; the hundred and
//! twenty Minors are batched eight ways, one prompt per ward; the ~350 ambients
//! get a deterministic code roll and cost nothing. **Thirty-nine provider calls
//! a game day**, trickled through the hours when the player is most likely to be
//! somewhere quiet.
//!
//! ## Why it is a lane and not a queue
//!
//! Cognition has one request in flight. A nightly reflection over five hundred
//! people, run through [`NpcScheduler`](crate::scheduler::NpcScheduler) at its
//! one-second minimum, is eight minutes of exclusive scheduler time during which
//! the player cannot be answered — the worst-feeling bug in the game, entirely
//! self-inflicted. So this is a *second slot*, all the way down to
//! [`Cognition::request_night`], with three rules:
//!
//! 1. **One in flight.** Never more.
//! 2. **Yields absolutely.** Never submits while the floor is busy, while the
//!    player is composing, while anyone is on stage with the player, or while a
//!    protected player reaction is pending or out. The player's lane is
//!    untouchable, and the Night Office must not so much as queue behind it.
//! 3. **Drops silently.** If the night ends before everyone has reflected, the
//!    rest keep yesterday's Round. **A missed Night Office is not an error.**
//!    Nothing waits for it, nothing retries, nobody notices.
//!
//! Bedtime staggers itself: the Round already says when each character sleeps
//! ([`Round::bedtime`]), and they do not all sleep at once, so the lane fills
//! naturally across the night with no scheduler of its own.
//!
//! ## What it may change
//!
//! `remember`, `forget`, `set_goal` and `set_round` — memory, intent, and one
//! leg of tomorrow. Nothing that happens in the world: a reflection cannot
//! speak, walk, give or make a sound, and the verbs it is offered
//! (`assets/prompts/night.j2`) say so. A reply that reaches for anything else is
//! a diagnostic and nothing more; it never lands in an inbox, because a private
//! thought at midnight must not become the morning's news.

use crate::receipts::{
    self, AffectedRef, BatchAdmission, NIGHT_PRODUCER, OperationId, Outcome, Ticket,
};

use std::collections::{BTreeMap, VecDeque};

use serde_json::Value;

use crate::{
    actions::set_round_leg,
    attention::{StageConfig, on_stage},
    character::Control,
    clock::{Office, WorldClock},
    ids::{ActorId, PlaceId, RequestId},
    lore::{PlanningWard, Significance},
    prompt::{PromptEnv, parse_reply, render_night_prompt, render_ward_prompt, ward_minors},
    round::Round,
    scheduler::SchedulerEvent,
    traits::{Cognition, Completion},
    world::World,
};

/// The office a Major with no bed in their day reflects at — the curfew, which
/// is when the streets clear whether or not their round says so.
const DEFAULT_BEDTIME: Office = Office::Snuffing;

/// The office the ward batches are owed at: the curfew bell, when the gates shut
/// and everyone who is going to be home is home.
const WARD_OFFICE: Office = Office::Snuffing;

/// How long the lane waits after the backend refuses it before trying again. The
/// only refusal that can happen is a night request still out from a previous
/// attempt, so this is politeness, not backoff.
const RETRY_SECONDS: f64 = 5.0;

/// How often a night that is standing down says so. It can be blocked for a
/// whole game night, so one line a minute is a diagnosis where one a frame is a
/// flood.
const YIELD_REPORT_SECONDS: f64 = 60.0;

/// The gap between reflections, as a fraction of a **game** day — so the night
/// is trickled rather than fired off in one burst (05_the_llm_seam.md §4:
/// *"trickled through the hours when the player is most likely to be somewhere
/// quiet"*). At the shipped clock this is about nine real seconds, which spreads
/// thirty-eight calls across roughly a tenth of the day.
///
/// It is a fraction of the game day and not a constant because the debug
/// time-scale exists: at 60× a whole night is a real minute, and a fixed
/// nine-second pace would drop almost every reflection unspent.
const PACE_FRACTION_OF_DAY: f64 = 1.0 / 400.0;

/// The longest ward mood the lane will store. Every Minor of the ward carries it
/// on every prompt for a game day, so an unbounded one would be a token leak
/// with a hundred and twenty multipliers on it.
pub const WARD_MOOD_MAX_CHARS: usize = 600;

/// The most `set_round` edits one ward batch may make. The ward speaks for its
/// people; it does not get to rewrite them.
pub const WARD_EDITS_MAX: usize = 3;

/// Who a reflection is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Subject {
    /// One Major, reflecting on their own day at their own bedtime.
    Person(ActorId),
    /// One ward's Minors, batched into a single prompt after the curfew.
    Ward(PlanningWard),
}

impl Subject {
    /// How the diagnostics name it.
    fn label(&self, world: &World) -> String {
        match self {
            Self::Person(actor_id) => world
                .characters
                .get(actor_id)
                .map(|actor| actor.name().to_string())
                .unwrap_or_else(|| actor_id.to_string()),
            Self::Ward(ward) => format!("{} ward", ward.as_str()),
        }
    }

    /// The completion budget, so a reflection costs what a turn by the same
    /// person would. A ward speaks for Minors and is priced as one.
    fn output_token_budget(&self, world: &World) -> Option<u32> {
        match self {
            Self::Person(actor_id) => world
                .characters
                .get(actor_id)
                .map(|actor| actor.significance().output_token_budget()),
            Self::Ward(_) => Some(Significance::Minor.output_token_budget()),
        }
    }
}

/// A reflection that is owed, and the night it is owed for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Due {
    semantic: Option<OperationId>,
    presence_epoch: Option<u64>,
    subject: Subject,
    /// The game day the bedtime fell on. A reflection still queued when the day
    /// rolls over is dropped where it stands — the night ended without it, and
    /// that is not an error.
    day: i64,
}

/// The one outstanding night request.
#[derive(Debug, Clone, PartialEq)]
struct Flight {
    semantic: OperationId,
    owed_day: i64,
    presence_epoch: Option<u64>,
    subject: Subject,
    request_id: RequestId,
    /// Kept so a *failed* exchange can still be archived, exactly as the
    /// scheduler keeps its own.
    prompt: String,
}

/// Everything the lane needs the caller to have already decided, computed once
/// per poll by the engine (D20) — and, because the lane is idle almost always,
/// computed *lazily*: [`NightOffice::poll`] asks for it only on the polls where
/// it would otherwise submit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NightGate {
    /// A line is being presented, or the player is holding the microphone.
    pub floor_busy: bool,
    /// The player is mid-utterance.
    pub player_composing: bool,
    /// Anyone at all is on stage with the player.
    pub stage_occupied: bool,
    /// A protected reaction to player speech is queued or already out.
    pub player_reaction: bool,
}

impl NightGate {
    /// Whether the lane must stand down. Rule 2, in one place.
    pub fn yields(&self) -> bool {
        self.reason().is_some()
    }

    /// Why it is standing down, for the diagnostic — a night that quietly does
    /// nothing is indistinguishable from a night that ran and changed nothing,
    /// and the two want very different fixes.
    fn reason(&self) -> Option<&'static str> {
        if self.player_reaction {
            Some("the player is owed a reply")
        } else if self.player_composing {
            Some("the player is speaking")
        } else if self.floor_busy {
            Some("a line is being presented")
        } else if self.stage_occupied {
            Some("somebody is on stage with the player")
        } else {
            None
        }
    }
}

/// Which of the three tiers the Night Office serves. All three ride the master
/// switch, so `enabled: false` is exactly today's behaviour and every fixture,
/// host and test keeps its bytes until it asks for the lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NightOfficeConfig {
    pub enabled: bool,
    /// Individual reflection for the Majors — 31 calls a game day.
    pub majors: bool,
    /// Ward-batched reflection for the Minors — 8 calls a game day.
    pub wards: bool,
    /// The ambient cast's code-rolled evening — no calls at all.
    pub ambients: bool,
}

impl Default for NightOfficeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            majors: true,
            wards: true,
            ambients: true,
        }
    }
}

/// The second cognition lane.
#[derive(Debug)]
pub struct NightOffice {
    config: NightOfficeConfig,
    queue: VecDeque<Due>,
    in_flight: Option<Flight>,
    /// Exact completion retained when the complete receipt batch cannot fit.
    held_result: Option<Completion>,
    /// The game day each subject last reflected on, so a bedtime crossed twice
    /// (a paused game, a debug time-scale) still buys one reflection.
    last_reflected: BTreeMap<Subject, i64>,
    /// Each Major's bedtime office, resolved from the round once at seed. The
    /// Round is authored content and does not change under us, so neither does
    /// this.
    bedtimes: BTreeMap<ActorId, Office>,
    /// Calendar days at the last office-crossing check, so a whole office passing inside
    /// one frame at 60× still owes its reflections exactly once.
    last_office_days: f64,
    /// Once-per-day semantic ambient decision, independent of provider duties.
    last_ambient_reroll_day: Option<i64>,
    /// The `now` at or after which a refused submit may be retried.
    next_attempt_at: f64,
    /// The `now` at or after which a stood-down night says so again. Throttled
    /// hard: the lane can be blocked for a whole game night, and one line a
    /// minute is a diagnosis where sixty a second is a flood.
    next_yield_report: f64,
    seeded: bool,
    /// Run totals, for the closing diagnostic.
    reflected: u64,
    dropped: u64,
}

impl NightOffice {
    pub fn new(config: NightOfficeConfig, now: f64, clock: &WorldClock) -> Self {
        Self {
            config,
            queue: VecDeque::new(),
            in_flight: None,
            held_result: None,
            last_reflected: BTreeMap::new(),
            bedtimes: BTreeMap::new(),
            last_office_days: clock.game_days(now),
            last_ambient_reroll_day: None,
            next_attempt_at: now,
            next_yield_report: now,
            seeded: false,
            reflected: 0,
            dropped: 0,
        }
    }

    /// Resolve every Major's bedtime off the seeded round. Returns one
    /// diagnostic line, or none when the lane is off.
    pub fn seed(&mut self, world: &World, round: &Round) -> Option<String> {
        if !self.config.enabled {
            return None;
        }
        self.seeded = true;
        for actor_id in &world.roster {
            let actor = &world.characters[actor_id];
            if actor.control() != Control::Llm || actor.significance() != Significance::Major {
                continue;
            }
            self.bedtimes.insert(
                actor_id.clone(),
                round.bedtime(actor_id).unwrap_or(DEFAULT_BEDTIME),
            );
        }
        let wards = if self.config.wards {
            PlanningWard::ALL
                .iter()
                .filter(|ward| ward_minors(world, **ward).next().is_some())
                .count()
        } else {
            0
        };
        let majors = if self.config.majors {
            self.bedtimes.len()
        } else {
            0
        };
        Some(format!(
            "[smart actors] night office: {majors} majors reflect at their own bedtimes, \
             {wards} wards batch at {} ({} calls a game day)",
            WARD_OFFICE.label(),
            majors + wards,
        ))
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled && self.seeded
    }

    /// How many reflections are owed but not yet spent — the headless tracer's
    /// one number, and how a test sees the lane fill across a night.
    pub fn owed(&self) -> usize {
        self.queue.len()
    }

    pub fn in_flight_subject(&self) -> Option<String> {
        self.in_flight.as_ref().map(|flight| match &flight.subject {
            Subject::Person(actor_id) => actor_id.to_string(),
            Subject::Ward(ward) => ward.as_str().to_string(),
        })
    }

    /// Reflections completed and reflections dropped unspent, for the run's
    /// closing line.
    pub fn totals(&self) -> (u64, u64) {
        (self.reflected, self.dropped)
    }

    /// Whether the lane would spend the slot right now, if the gate let it.
    ///
    /// The state as it stands: this is the question [`Self::poll`] asks *after*
    /// the harvest. The engine's gate wants [`Self::could_submit`] instead —
    /// asked here, a flight whose reply has already landed still counts as out.
    pub fn wants_slot(&self, now: f64) -> bool {
        self.enabled()
            && self.in_flight.is_none()
            && !self.queue.is_empty()
            && now >= self.next_attempt_at
    }

    /// Whether *this poll* could end in a submit — [`Self::wants_slot`], but
    /// counting a flight whose reply is already sitting in `completions` as
    /// landed, because [`Self::poll`] harvests it before it asks the slot
    /// question again.
    ///
    /// The engine asks *before* computing [`NightGate`], because the stage
    /// question is a `characters_within` scan and the lane is idle on almost
    /// every poll of almost every run. Paying for that scan every frame, for a
    /// night that is not happening, is exactly the kind of cost this whole
    /// feature exists not to pay. But it must be this form and not
    /// [`Self::wants_slot`]: on the poll where a reflection lands the flight is
    /// still out while the gate is built, that form says no, and the submit
    /// that follows the harvest would then read a stage stamped empty without
    /// anyone having looked — Rule 2, bypassed on exactly the polls that
    /// complete a reflection. Call [`Self::ring`] first, so a bedtime that rang
    /// this very poll is already in the queue when this is asked.
    pub fn could_submit(&self, now: f64, completions: &[Completion]) -> bool {
        let slot_free = match &self.in_flight {
            None => true,
            Some(flight) => {
                self.held_result.is_some()
                    || completions
                        .iter()
                        .any(|completion| completion.request_id == flight.request_id)
            }
        };
        self.enabled() && slot_free && !self.queue.is_empty() && now >= self.next_attempt_at
    }

    /// Enqueue the reflections the bells crossed since the last poll, and run
    /// the ambient code roll. A *span* is tested rather than an instant, so a
    /// whole office passing inside one frame at 60× still rings exactly once.
    ///
    /// Separate from [`Self::poll`] only so the engine can ring, then ask
    /// [`Self::could_submit`], then answer the stage question at most once a
    /// night instead of sixty times a second.
    pub fn ring(
        &mut self,
        now: f64,
        world: &mut World,
        round: &mut Round,
        clock: &WorldClock,
        events: &mut Vec<SchedulerEvent>,
    ) {
        if !self.enabled() {
            return;
        }
        self.ring_bedtimes(now, world, round, clock, events);
    }

    /// Harvest, apply, submit. [`Self::ring`] runs first, in the same poll.
    #[allow(clippy::too_many_arguments)]
    pub fn poll(
        &mut self,
        now: f64,
        world: &mut World,
        clock: &WorldClock,
        completions: &mut Vec<Completion>,
        gate: NightGate,
        cognition: &mut dyn Cognition,
        env: &PromptEnv,
    ) -> Vec<SchedulerEvent> {
        let mut events: Vec<SchedulerEvent> = Vec::new();
        if !self.enabled() {
            return events;
        }

        // Harvest ours and ours only, before the scheduler drains the rest —
        // it discards anything it is not waiting on, so a night completion left
        // in the vec would be logged as a stale result and lost.
        if let Some(completion) = self.held_result.take() {
            self.apply(now, world, completion, &mut events);
        } else if let Some(flight) = &self.in_flight
            && let Some(index) = completions
                .iter()
                .position(|completion| completion.request_id == flight.request_id)
        {
            let completion = completions.remove(index);
            self.apply(now, world, completion, &mut events);
        }

        if self.wants_slot(now) {
            match gate.reason() {
                None => self.submit(now, world, clock, cognition, env, &mut events),
                Some(reason) if now >= self.next_yield_report => {
                    self.next_yield_report = now + YIELD_REPORT_SECONDS;
                    events.push(SchedulerEvent::Diagnostic(format!(
                        "[night] standing down ({reason}); {} reflections owed",
                        self.queue.len()
                    )));
                }
                Some(_) => {}
            }
        }
        events
    }

    fn ring_bedtimes(
        &mut self,
        now: f64,
        world: &mut World,
        round: &mut Round,
        clock: &WorldClock,
        events: &mut Vec<SchedulerEvent>,
    ) {
        let crossings =
            crate::clock::offices_crossed_days(self.last_office_days, clock.game_days(now));
        self.last_office_days = clock.game_days(now);
        for (instant, office) in crossings {
            let day = instant.floor() as i64;
            if self.config.majors {
                // Roster order, so a night fills the queue in the same order
                // every run — the lane has no clock of its own and must not
                // acquire one through a map's iteration.
                for actor_id in &world.roster {
                    if self.bedtimes.get(actor_id) != Some(&office) {
                        continue;
                    }
                    if !world.is_present(actor_id) {
                        continue;
                    }
                    self.enqueue(Subject::Person(actor_id.clone()), day);
                }
            }
            if self.config.wards && office == WARD_OFFICE {
                for ward in PlanningWard::ALL {
                    if ward_minors(world, ward).next().is_some() {
                        self.enqueue(Subject::Ward(ward), day);
                    }
                }
            }
            if self.config.ambients
                && office == WARD_OFFICE
                && self.last_ambient_reroll_day != Some(day)
            {
                self.last_ambient_reroll_day = Some(day);
                let moved = round.reroll_ambient_evenings(world, day);
                if moved > 0 {
                    events.push(SchedulerEvent::Diagnostic(format!(
                        "[night] {moved} ambient evenings moved to a tavern hearth for day {}",
                        day + 1
                    )));
                }
            }
        }
    }

    /// Queue one reflection, unless the subject has already had tonight's.
    fn enqueue(&mut self, subject: Subject, day: i64) {
        if self.last_reflected.get(&subject) == Some(&day) {
            return;
        }
        if self.queue.iter().any(|due| due.subject == subject) {
            return;
        }
        // Stamped as spent at *queue* time, not at completion: a reflection that
        // never gets its turn has still had its night, and a queue that
        // re-offered it every crossing would spend the whole next day catching
        // up on a night that is over.
        self.last_reflected.insert(subject.clone(), day);
        self.queue.push_back(Due {
            subject,
            day,
            semantic: None,
            presence_epoch: None,
        });
    }

    /// Take the next reflection still owed *tonight* and send it.
    fn submit(
        &mut self,
        now: f64,
        world: &mut World,
        clock: &WorldClock,
        cognition: &mut dyn Cognition,
        env: &PromptEnv,
        events: &mut Vec<SchedulerEvent>,
    ) {
        let today = clock.at(now).day;
        // Rule 3, and the only place it needs stating: everything the night
        // outran is dropped here, silently, with no percept and no retry.
        while self.queue.front().is_some_and(|due| due.day != today) {
            if let Some(due) = self.queue.pop_front()
                && let Some(work) = due.semantic
            {
                receipts::release_finished_root(world, work);
            }
            self.dropped += 1;
        }
        let Some(mut due) = self.queue.pop_front() else {
            return;
        };

        let prompt = match &due.subject {
            Subject::Person(actor_id) => {
                // Presence, control and enrolment can all have changed since
                // the bell: a Major who left the city, or one the round never
                // enrolled, has no day for a reflection to alter.
                if !world.is_present(actor_id)
                    || (due.semantic.is_some()
                        && world
                            .characters
                            .get(actor_id)
                            .is_none_or(|a| Some(a.state.presence_epoch) != due.presence_epoch))
                {
                    self.dropped += 1;
                    if let Some(id) = due.semantic {
                        receipts::release_finished_root(world, id);
                    }
                    return;
                }
                render_night_prompt(world, actor_id, env)
            }
            Subject::Ward(ward) => render_ward_prompt(world, *ward, env),
        };
        let prompt = match prompt {
            Ok(prompt) => prompt,
            Err(error) => {
                self.dropped += 1;
                events.push(SchedulerEvent::Diagnostic(format!(
                    "[night] prompt for {} failed: {error}",
                    due.subject.label(world)
                )));
                if let Some(id) = due.semantic {
                    receipts::release_finished_root(world, id);
                }
                return;
            }
        };

        let semantic = match due.semantic {
            Some(id) => id,
            None => match world.command_ledger.reserve_operation(NIGHT_PRODUCER) {
                Ok(id) => {
                    due.semantic = Some(id);
                    due.presence_epoch = match &due.subject {
                        Subject::Person(actor) => {
                            world.characters.get(actor).map(|a| a.state.presence_epoch)
                        }
                        Subject::Ward(_) => None,
                    };
                    id
                }
                Err(reason) => {
                    self.queue.push_front(due);
                    self.next_attempt_at = now + RETRY_SECONDS;
                    events.push(SchedulerEvent::Diagnostic(format!(
                        "[action] night admission deferred: {}",
                        reason.code
                    )));
                    return;
                }
            },
        };
        let presence_epoch = due.presence_epoch;
        match cognition.request_night(prompt.clone(), due.subject.output_token_budget(world)) {
            Ok(request_id) => {
                // Trickle rather than burst: the next reflection waits out a
                // slice of the game day, so a night is a night and not
                // thirty-eight requests at the same second of it.
                self.next_attempt_at = now + pace_seconds(clock);
                self.in_flight = Some(Flight {
                    semantic,
                    owed_day: due.day,
                    presence_epoch,
                    subject: due.subject,
                    request_id,
                    prompt,
                });
            }
            Err(_busy) => {
                // The backend has no second slot, or its own night request is
                // still out. Either way the reflection waits rather than
                // reaching for the player's lane — put it back at the front and
                // try again in a moment. If the night ends first, it drops.
                self.queue.push_front(due);
                self.next_attempt_at = now + RETRY_SECONDS;
            }
        }
    }

    /// Archive the exchange and carry out whatever the reflection decided.
    fn reply_payload(flight: &Flight, reply: &str) -> serde_json::Value {
        serde_json::json!({"service":"night_reflection", "subject":match &flight.subject { Subject::Person(id) => id.as_str(), Subject::Ward(ward) => ward.as_str() }, "owed_day":flight.owed_day, "presence_epoch":flight.presence_epoch, "reply_version":receipts::PROVIDER_REPLY_VERSION, "reply_digest":receipts::provider_reply_digest(reply)})
    }

    fn apply(
        &mut self,
        now: f64,
        world: &mut World,
        completion: Completion,
        events: &mut Vec<SchedulerEvent>,
    ) {
        let flight = self
            .in_flight
            .as_ref()
            .expect("completion belongs to a flight");
        let semantic = flight.semantic;
        if world.command_ledger.get(semantic.command(0)).is_some()
            && !world.command_ledger.operation_pending(semantic)
        {
            // An execution retry cannot spend an already committed semantic
            // reply again, even if the new execution failed or returned junk.
            let admission = match &completion.result {
                Ok(reply) if receipts::provider_reply_within_limit(reply) => world
                    .command_ledger
                    .begin_batch(&[(semantic.command(0), Self::reply_payload(flight, reply))]),
                _ => BatchAdmission::Refused(Outcome::rejected(
                    "semantic_reply_committed",
                    "this semantic reply already has a committed result",
                )),
            };
            self.in_flight = None;
            receipts::release_finished_root(world, semantic);
            match admission {
                BatchAdmission::Replay(receipt) => {
                    events.push(SchedulerEvent::ActionReceipt(receipt))
                }
                BatchAdmission::Refused(reason) => {
                    events.push(SchedulerEvent::CommandAdmissionRefused {
                        id: semantic.command(0),
                        reason,
                        retryable: false,
                    })
                }
                _ => unreachable!("existing committed root cannot require new admission"),
            }
            return;
        }

        let mut completion = completion;
        if completion
            .result
            .as_ref()
            .is_ok_and(|reply| !receipts::provider_reply_within_limit(reply))
        {
            completion.result = Err(crate::traits::CognitionError::new("ReplyTooLarge"));
        }
        let mut reserved = None;
        let flight = self.in_flight.as_ref().expect("matching night flight");
        let current = match &flight.subject {
            Subject::Person(id) => {
                world.is_present(id)
                    && world.characters.get(id).is_some_and(|actor| {
                        actor.control() == Control::Llm
                            && Some(actor.state.presence_epoch) == flight.presence_epoch
                    })
            }
            Subject::Ward(_) => true,
        };
        if current && let Ok(reply) = &completion.result {
            let semantic = flight.semantic;
            let (actions, _) = parse_reply(reply);
            let mut rows = vec![(semantic.command(0), Self::reply_payload(flight, reply))];
            for (index, (verb, args)) in actions
                .iter()
                .take(usize::from(receipts::MAX_STEPS))
                .enumerate()
            {
                rows.push((semantic.command(index as u16 + 1), serde_json::json!({
                    "service": "night_action", "subject": match &flight.subject { Subject::Person(id) => id.as_str(), Subject::Ward(ward) => ward.as_str() }, "verb": verb, "args": args,
                })));
            }
            let admission = if actions.len() > usize::from(receipts::MAX_STEPS) {
                BatchAdmission::Refused(Outcome::rejected(
                    "reply_action_limit",
                    "reflection exceeds its bounded action count",
                ))
            } else {
                world.command_ledger.begin_batch(&rows)
            };
            match admission {
                BatchAdmission::New(tickets) => reserved = Some(tickets),
                BatchAdmission::Replay(receipt) => {
                    self.in_flight = None;
                    receipts::release_finished_root(world, semantic);
                    events.push(SchedulerEvent::ActionReceipt(receipt));
                    return;
                }
                BatchAdmission::Deferred(reason) => {
                    events.push(SchedulerEvent::CommandAdmissionRefused {
                        id: semantic.command(0),
                        reason,
                        retryable: true,
                    });
                    self.held_result = Some(completion);
                    return;
                }
                BatchAdmission::Refused(reason) => {
                    self.in_flight = None;
                    self.dropped += 1;
                    receipts::release_finished_root(world, semantic);
                    events.push(SchedulerEvent::CommandAdmissionRefused {
                        id: semantic.command(0),
                        reason,
                        retryable: false,
                    });
                    return;
                }
            }
        }
        let flight = self
            .in_flight
            .take()
            .expect("a night result is only harvested while a night request is out");
        let label = flight.subject.label(world);
        let (actor_id, actor_name) = match &flight.subject {
            Subject::Person(actor_id) => (actor_id.clone(), label.clone()),
            // A ward is not an actor, so the archive files it under a handle
            // that cannot collide with one: ids are five characters, wards
            // are words.
            Subject::Ward(ward) => (ActorId::from_raw(format!("ward:{}", ward.as_str())), label),
        };

        // The archive is unconditional and first, exactly as the scheduler's is:
        // a failed reflection is precisely what you want in the log.
        events.push(SchedulerEvent::PromptExchange {
            actor_id,
            actor_name: actor_name.clone(),
            prompt: flight.prompt,
            answer: completion.result.as_ref().ok().cloned(),
            duration_seconds: completion.duration_seconds,
            error: completion
                .result
                .as_ref()
                .err()
                .map(|error| error.detail().to_string()),
        });

        let reply = match &completion.result {
            Ok(reply) => reply.clone(),
            Err(error) => {
                // No backoff, no retry, no `system:` line: a night that failed
                // is a night that did not happen, and the morning must not be
                // told about it.
                self.dropped += 1;
                receipts::release_finished_root(world, flight.semantic);
                events.push(SchedulerEvent::Diagnostic(format!(
                    "[night] {actor_name}'s reflection failed: {error}"
                )));
                return;
            }
        };

        if !current {
            self.dropped += 1;
            receipts::release_finished_root(world, flight.semantic);
            return;
        }
        let mut tickets = reserved
            .expect("successful reflection batch reserved")
            .into_iter();
        let root = tickets.next().expect("reflection root");
        self.reflected += 1;
        let done = match &flight.subject {
            Subject::Person(actor_id) => {
                self.apply_person(now, world, actor_id, &reply, tickets.collect(), events)
            }
            Subject::Ward(ward) => {
                self.apply_ward(now, world, *ward, &reply, tickets.collect(), events)
            }
        };
        events.push(SchedulerEvent::Diagnostic(format!(
            "[night] {actor_name} reflected: {}",
            if done.is_empty() {
                "nothing to settle".to_string()
            } else {
                done.join(", ")
            }
        )));
        let receipt = world.command_ledger.finish(
            root,
            now,
            Outcome::completed("reflection and action results committed"),
            vec![],
        );
        events.push(SchedulerEvent::ActionReceipt(receipt));
        receipts::release_finished_root(world, flight.semantic);
    }

    /// One Major's reflection. Everything runs through the ordinary action
    /// layer, so a `set_round` at midnight is validated exactly as one at noon
    /// would be — but only the four verbs the night template offers are let
    /// through, and nothing it says reaches an inbox.
    fn apply_person(
        &mut self,
        now: f64,
        world: &mut World,
        actor_id: &ActorId,
        reply: &str,
        tickets: Vec<Ticket>,
        events: &mut Vec<SchedulerEvent>,
    ) -> Vec<String> {
        let mut done: Vec<String> = Vec::new();
        let (actions, errors) = parse_reply(reply);
        for error in errors {
            events.push(SchedulerEvent::Diagnostic(format!(
                "[night] {actor_id}: {error}"
            )));
        }
        for ((verb, args), ticket) in actions.into_iter().zip(tickets) {
            if !is_night_verb(&verb) {
                let reason = Outcome::rejected(
                    "not_night_verb",
                    format!("{verb} is not a night verb; ignored"),
                );
                events.push(SchedulerEvent::Diagnostic(format!(
                    "[night] {actor_id}: {}",
                    reason.message
                )));
                let receipt = world.command_ledger.finish(ticket, now, reason, vec![]);
                events.push(SchedulerEvent::ActionReceipt(receipt));
                continue;
            }
            let (result, receipt) = receipts::commit_actor_action(
                world,
                now,
                ticket,
                actor_id,
                &verb,
                &Value::Object(args),
            );
            events.push(SchedulerEvent::ActionReceipt(receipt));
            match result {
                Ok(_) if verb != "wait" => done.push(verb),
                Ok(_) => {}
                Err(error) => events.push(SchedulerEvent::Diagnostic(format!(
                    "[night] {actor_id}: {verb} failed: {error}"
                ))),
            }
        }
        done
    }

    /// One ward's reflection: a mood every Minor of the ward will carry, and up
    /// to [`WARD_EDITS_MAX`] rounds moved.
    ///
    /// These are not [`crate::actions::apply_action`] verbs, because there is no acting actor —
    /// a ward has no hands. `ward_mood` is the ward's alone; `set_round` here
    /// names somebody else, and the ward's decision *teaches* them the way as
    /// part of making it, exactly as `tell_way` would have. That is the one
    /// deliberate departure from the self-only whitelist: the ward did not
    /// consult the person, so requiring them to have already known the place
    /// would make the verb almost always fail.
    fn apply_ward(
        &mut self,
        now: f64,
        world: &mut World,
        ward: PlanningWard,
        reply: &str,
        tickets: Vec<Ticket>,
        events: &mut Vec<SchedulerEvent>,
    ) -> Vec<String> {
        let mut done = Vec::new();
        let (actions, errors) = parse_reply(reply);
        for error in errors {
            events.push(SchedulerEvent::Diagnostic(format!(
                "[night] {} ward: {error}",
                ward.as_str()
            )));
        }
        let mut edits = 0;
        for ((verb, args), ticket) in actions.into_iter().zip(tickets) {
            let result = if verb == "set_round"
                && world.travel_actions.len() + world.round_actions.len()
                    >= receipts::PROTECTED_CAPACITY
            {
                Err("active action receipt capacity is full".to_owned())
            } else {
                self.apply_ward_action(world, ward, &verb, &args, &mut edits)
            };
            let outcome = match result {
                Ok(Some(line)) => {
                    done.push(line.clone());
                    if verb == "set_round" {
                        let actor =
                            ActorId::new(args["person"].as_str().expect("validated person"))
                                .expect("validated identity");
                        receipts::bind_round_edit(world, &actor, ticket.id, now, true);
                        Outcome::new(
                            receipts::ReceiptState::Accepted,
                            "round_edit_accepted",
                            &line,
                        )
                    } else {
                        Outcome::completed(line)
                    }
                }
                Ok(None) => Outcome::completed("no world action requested"),
                Err(error) => {
                    events.push(SchedulerEvent::Diagnostic(format!(
                        "[night] {} ward: {error}",
                        ward.as_str()
                    )));
                    Outcome::rejected("invalid_ward_action", error)
                }
            };
            let receipt = world.command_ledger.finish(
                ticket,
                now,
                outcome,
                AffectedRef::new("ward", ward.as_str())
                    .into_iter()
                    .collect(),
            );
            events.push(SchedulerEvent::ActionReceipt(receipt));
        }
        done
    }

    fn apply_ward_action(
        &self,
        world: &mut World,
        ward: PlanningWard,
        verb: &str,
        args: &serde_json::Map<String, Value>,
        edits: &mut usize,
    ) -> Result<Option<String>, String> {
        match verb {
            "wait" => Ok(None),
            "ward_mood" => {
                let mood = args
                    .get("mood")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|mood| !mood.is_empty())
                    .ok_or_else(|| "ward_mood needs a non-empty \"mood\"".to_string())?;
                world
                    .ward_moods
                    .insert(ward, mood.chars().take(WARD_MOOD_MAX_CHARS).collect());
                Ok(Some("ward_mood".to_owned()))
            }
            "chalk_ward_sign" => {
                let place = args
                    .get("place")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|place| !place.is_empty())
                    .ok_or_else(|| "chalk_ward_sign needs a non-empty \"place\"".to_string())?;
                let authored = world
                    .mark_catalog
                    .ward_sign_place(ward.as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| "chalk_ward_sign has no authored place".to_string())?;
                if !place.eq_ignore_ascii_case(&authored) {
                    return Err(format!(
                        "chalk_ward_sign named {place:?}; this ward's place of resort is {authored:?}"
                    ));
                }
                let day = world.current_time.map_or(0.0, |time| time.game_days());
                crate::marks::draw_or_refresh(
                    world,
                    crate::marks::MarkKind::WardSign,
                    crate::marks::MarkAnchor::Place(authored.clone()),
                    None,
                    day,
                )
                .ok_or_else(|| format!("chalk_ward_sign could not chalk {authored}"))?;
                Ok(Some(format!("chalk_ward_sign {authored}")))
            }
            "set_round" if *edits >= WARD_EDITS_MAX => Err(format!(
                "more than {WARD_EDITS_MAX} set_round edits; ignored"
            )),
            "set_round" => {
                *edits += 1;
                self.ward_set_round(world, ward, args)
                    .map(Some)
                    .map_err(|error| format!("set_round failed: {error}"))
            }
            other => Err(format!("{other} is not a ward verb; ignored")),
        }
    }

    /// `set_round {"person": …, "leg": …, "place_id": …}` from a ward batch.
    fn ward_set_round(
        &self,
        world: &mut World,
        ward: PlanningWard,
        args: &serde_json::Map<String, Value>,
    ) -> Result<String, String> {
        let person = args
            .get("person")
            .and_then(Value::as_str)
            .and_then(|text| ActorId::new(text).ok())
            .ok_or_else(|| "person must be one of your_people's ids".to_string())?;
        let place_id = args
            .get("place_id")
            .and_then(Value::as_str)
            .and_then(|text| PlaceId::new(text).ok())
            .ok_or_else(|| "place_id must be one of their_places' ids".to_string())?;
        let leg = args
            .get("leg")
            .cloned()
            .ok_or_else(|| "set_round needs a leg number".to_string())?;

        // The ward may only move its own people — and only the Minors, who are
        // the ones it reflected for. A Major reflects for themselves.
        if !ward_minors(world, ward).any(|actor| actor.id() == &person) {
            return Err(format!("{person} is not one of this ward's people"));
        }
        if world.places.get(&place_id).is_none() {
            return Err(format!("there is no place with id {place_id}"));
        }
        // Take the edit *first*. `set_round_leg` still refuses an empty round
        // and a leg that is not on the sheet, and it does not consult
        // `places_known` — only the self verb does — so nothing is lost by
        // asking it before teaching, and a refused decision must not leave a
        // way behind it. `tell_way` is otherwise the only road into somebody
        // else's `places_known`, and a route nobody ever told them would
        // outlive the diagnostic by the rest of the game.
        let line = set_round_leg(world, &person, &leg, &place_id).map_err(|error| error.message)?;
        // The receipt adapter asks Round to teach only after its trade and
        // enrollment checks accept the pending edit.
        Ok(line)
    }
}

/// The real seconds between reflections at this clock — a fixed slice of the
/// **game** day, so the debug time-scale speeds the night up with the sun
/// instead of dropping it unspent.
fn pace_seconds(clock: &WorldClock) -> f64 {
    // `WorldClock` guarantees a finite `seconds_per_day >= 1` and a finite
    // positive `scale`, so this cannot go non-finite; the `max` is for the
    // degenerate-but-legal case of a scale so large the quotient underflows.
    let real_seconds_per_game_day = clock.seconds_per_day() / clock.scale();
    (real_seconds_per_game_day * PACE_FRACTION_OF_DAY).max(0.0)
}

/// The four verbs a person's reflection may use, plus `wait`. Everything else —
/// `say`, `go_to`, `offer_item`, a hallucinated verb — is refused here rather
/// than by [`crate::actions::apply_action`], because the refusal is about *when*, not about
/// whether the verb exists.
fn is_night_verb(verb: &str) -> bool {
    matches!(
        verb,
        "remember" | "forget" | "set_goal" | "set_round" | "wait"
    )
}

/// The stage question the gate asks, in the one form both idle modes can
/// answer: is anybody at all standing with the player?
///
/// The engine's own stage set is filtered by novelty and curiosity, and is not
/// computed at all under [`IdleCognitionMode::All`](crate::IdleCognitionMode) —
/// neither of which is the question here. "Nobody is near the player" is, and it
/// has one answer in both modes.
pub fn stage_occupied(
    world: &World,
    player_id: &ActorId,
    partner: Option<&ActorId>,
    stage: &StageConfig,
) -> bool {
    !on_stage(world, player_id, partner, stage).is_empty()
}

#[cfg(test)]
mod tests;
