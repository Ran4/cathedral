//! The turn scheduler (`tests/test_prompt_scheduler.py::SchedulerTests` 11-23),
//! plus the Rust-only contracts the port introduces (request-id matching, the
//! reply size limit, the exact status wire strings).
//!
//! Python's scheduler tests spin `wait_until` loops against a daemon thread and
//! synchronize with `threading.Event`s. Here the backend is a staged fake: a
//! request is answered synchronously but *delivered* only when the test says so,
//! which reproduces the interleavings those handshakes were testing (an event
//! arriving mid-request; a `prioritize` landing while a request is outstanding)
//! without a single thread or sleep.

mod prompt_support;

use cathedral_sim::{
    Cognition, CognitionBusy, CognitionError, Completion, Control, INBOX_MAX_ENTRIES, IdleGate,
    NpcScheduler, PromptEnv, RequestId, SchedulerEvent, StatusEvent, Subsystem, Vec3, World,
    apply_action, llm_turn_order,
};
use prompt_support::{actor, md_section, prompt_env, seed_world};
use serde_json::json;

// --------------------------------------------------------------------- doubles

type ReplyFn = Box<dyn FnMut(&str, usize) -> Result<String, CognitionError>>;

/// A scripted provider. `request` answers immediately but *stages* the
/// completion; the harness decides which poll it arrives on.
struct ScriptedCognition {
    reply: ReplyFn,
    prompts: Vec<String>,
    staged: Vec<Completion>,
    next_request_id: u64,
}

impl ScriptedCognition {
    fn new(reply: ReplyFn) -> Self {
        Self {
            reply,
            prompts: Vec::new(),
            staged: Vec::new(),
            next_request_id: 1,
        }
    }

    fn take_staged(&mut self) -> Vec<Completion> {
        std::mem::take(&mut self.staged)
    }
}

impl Cognition for ScriptedCognition {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        let result = (self.reply)(&prompt, self.prompts.len());
        self.prompts.push(prompt);
        self.staged.push(Completion {
            request_id,
            result,
            duration_seconds: 0.25,
        });
        Ok(request_id)
    }
}

/// A backend that can never take work: the defensive "cognition worker is busy"
/// branch.
struct BusyCognition;

impl Cognition for BusyCognition {
    fn request(&mut self, _prompt: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
}

// --------------------------------------------------------------------- harness

struct Harness {
    world: World,
    transcript: Vec<String>,
    env: PromptEnv,
    cognition: ScriptedCognition,
    scheduler: NpcScheduler,
    now: f64,
    floor_busy: bool,
    events: Vec<SchedulerEvent>,
}

impl Harness {
    fn new(reply: ReplyFn, minimum_delay_seconds: f64) -> Self {
        Self::with_backoff(reply, minimum_delay_seconds, 60.0, 0.0)
    }

    fn with_backoff(
        reply: ReplyFn,
        minimum_delay_seconds: f64,
        maximum_backoff_seconds: f64,
        now: f64,
    ) -> Self {
        let world = seed_world();
        let scheduler = NpcScheduler::new(
            llm_turn_order(&world),
            minimum_delay_seconds,
            maximum_backoff_seconds,
            now,
        );
        Self {
            world,
            transcript: Vec::new(),
            env: prompt_env(),
            cognition: ScriptedCognition::new(reply),
            scheduler,
            now,
            floor_busy: false,
            events: Vec::new(),
        }
    }

    fn start(&mut self) {
        self.scheduler.start(self.now);
    }

    /// One poll, delivering everything the backend has staged.
    fn poll(&mut self) {
        let mut completions = self.cognition.take_staged();
        self.poll_with(&mut completions);
    }

    /// One poll that delivers nothing: the request stays outstanding, so the
    /// world can change (or a priority land) while the provider "thinks".
    fn poll_pending(&mut self) {
        self.poll_with(&mut Vec::new());
    }

    fn poll_with(&mut self, completions: &mut Vec<Completion>) {
        let events = self.scheduler.poll(
            self.now,
            &mut self.world,
            &mut self.transcript,
            completions,
            self.floor_busy,
            // Ungated: these tests are about the turn stream itself, not about
            // who is standing near the player. The gate's own behavior is
            // covered in `attention.rs` and in the scheduler's unit tests.
            IdleGate::All,
            &mut self.cognition,
            &self.env,
        );
        self.events.extend(events);
    }

    fn poll_times(&mut self, times: usize) {
        for _ in 0..times {
            self.poll();
        }
    }

    /// The name on each prompt's character sheet — Python's `calls` list.
    fn prompted_names(&self) -> Vec<String> {
        self.cognition
            .prompts
            .iter()
            .map(|prompt| {
                cathedral_sim::fake::sheet_name(prompt)
                    .expect("every prompt carries a sheet")
                    .to_string()
            })
            .collect()
    }

    fn inbox(&self, actor_id: &str) -> Vec<String> {
        self.world.characters[&actor(actor_id)].inbox().to_vec()
    }

    fn statuses(&self) -> Vec<&StatusEvent> {
        self.events
            .iter()
            .filter_map(|event| match event {
                SchedulerEvent::Status(status) => Some(status),
                _ => None,
            })
            .collect()
    }

    fn diagnostics(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter_map(|event| match event {
                SchedulerEvent::Diagnostic(line) => Some(line.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// The scripted no-op turn every test that only cares about *ordering* replies.
fn noop(_prompt: &str, _call: usize) -> Result<String, CognitionError> {
    Ok(r#"set_goal {"goal": null}"#.to_string())
}

/// Put the player next to Sven, so a player `say` is heard.
fn player_beside_sven(world: &mut World) {
    world
        .characters
        .get_mut(&actor("player"))
        .unwrap()
        .state
        .position_m = Vec3::new(-1.8, 0.91, 113.0);
}

fn player_says(world: &mut World, target: &str, text: &str) {
    apply_action(
        world,
        &actor("player"),
        "say",
        &json!({"target": target, "text": text}),
    )
    .expect("the player's say applies");
}

// ----------------------------------------------------------------------- tests

/// 11. `test_wait_does_not_grow_transcript`
#[test]
fn wait_does_not_grow_transcript() {
    let mut harness = Harness::new(Box::new(|_, _| Ok("wait {}".to_string())), 100.0);
    harness.start();
    harness.poll(); // submits Sven
    harness.poll(); // applies his `wait`

    assert!(harness.scheduler.in_flight_actor_id().is_none());
    assert_eq!(harness.transcript, Vec::<String>::new());
}

/// 12. `test_event_arriving_during_completion_remains_for_next_turn`
#[test]
fn event_arriving_during_completion_remains_for_next_turn() {
    let mut harness = Harness::new(Box::new(noop), 100.0);
    player_beside_sven(&mut harness.world);
    harness
        .world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .inbox = vec!["old event".to_string()];

    harness.start();
    harness.poll_pending(); // Sven's prompt is out; the provider is thinking

    // The prompt that went out carries only what had already arrived.
    assert_eq!(
        md_section(&harness.cognition.prompts[0], "since_your_last_turn").unwrap(),
        ["old event"]
    );

    // A percept lands mid-request …
    player_says(&mut harness.world, "sv3n1", "new event");
    harness.poll(); // … and now the turn applies

    let inbox = harness.inbox("sv3n1");
    assert_eq!(inbox.len(), 1, "{inbox:?}");
    assert!(inbox[0].contains("new event"), "{inbox:?}");
}

/// 13. `test_provider_failure_requeues_percepts_without_duplication`
///
/// A failed turn puts its percepts back as unread-and-pending, so the retried
/// prompt presents them as new — never in both fields at once — and they
/// graduate into `recent_history` exactly once.
#[test]
fn provider_failure_requeues_percepts_without_duplication() {
    let mut harness = Harness::with_backoff(
        Box::new(|_, call| {
            if call == 0 {
                Err(CognitionError::new("RuntimeError"))
            } else {
                noop("", call)
            }
        }),
        0.0,
        60.0,
        1_000.0,
    );
    player_beside_sven(&mut harness.world);
    player_says(&mut harness.world, "sv3n1", "psst");

    let heard = r#"A stranger (id player) said to you: "psst""#.to_string();
    let sven = actor("sv3n1");
    assert_eq!(
        harness.world.characters[&sven].inbox(),
        std::slice::from_ref(&heard)
    );
    assert_eq!(
        harness.world.characters[&sven].pending_history(),
        std::slice::from_ref(&heard)
    );

    harness.start();
    harness.poll(); // submit Sven
    harness.poll(); // the provider failed

    assert_eq!(harness.world.characters[&sven].inbox()[0], heard);
    assert_eq!(
        harness.world.characters[&sven].pending_history(),
        std::slice::from_ref(&heard)
    );
    assert!(harness.world.characters[&sven].recent_history().is_empty());

    // Past the failure backoff; the round robin resumes at Conny.
    harness.now += 200.0;
    harness.poll_times(4); // Conny, Ilse, Sven again, apply Sven

    let sven_prompts: Vec<&String> = harness
        .cognition
        .prompts
        .iter()
        .filter(|prompt| cathedral_sim::fake::sheet_name(prompt) == Some("Sven"))
        .collect();
    assert_eq!(sven_prompts.len(), 2, "Sven must have been retried");

    let since = md_section(sven_prompts[1], "since_your_last_turn").unwrap();
    assert_eq!(since[0], heard);
    let recent = md_section(sven_prompts[1], "recent_history").unwrap();
    assert!(
        !recent.contains(&heard),
        "the retried prompt must not show the percept twice: {recent:?}"
    );

    // The successful retry graduates it exactly once.
    assert_eq!(
        harness.world.characters[&sven].recent_history(),
        std::slice::from_ref(&heard)
    );
    assert!(harness.world.characters[&sven].pending_history().is_empty());
}

/// A run of provider failures must not grow the inbox without bound. Each
/// failure restores the drained percepts and appends a `system:` line; before
/// the re-cap that pushed the buffer one entry past `INBOX_MAX_ENTRIES` every
/// time, so a persistently-failing provider leaked memory one line per retry
/// (`features/movement/code_review.md` finding 2). No player is nearby, so the
/// only thing touching the inbox is the failure path itself.
#[test]
fn repeated_provider_failures_keep_the_inbox_bounded() {
    let mut harness = Harness::with_backoff(
        Box::new(|_, _| Err(CognitionError::new("boom"))),
        0.0,
        0.0, // no backoff, so the failing actor is retried without advancing `now`
        0.0,
    );
    // Seed Sven's inbox right at the bound, so any off-by-one overflows it.
    let sven = actor("sv3n1");
    harness
        .world
        .characters
        .get_mut(&sven)
        .unwrap()
        .state
        .inbox = (0..INBOX_MAX_ENTRIES).map(|i| format!("percept {i}")).collect();

    harness.start();
    // Enough polls to submit and fail Sven several times over.
    for _ in 0..24 {
        harness.poll();
        let inbox_len = harness.world.characters[&sven].inbox().len();
        assert!(
            inbox_len <= INBOX_MAX_ENTRIES,
            "the inbox must stay bounded across failures, saw {inbox_len}"
        );
    }

    // Still full, and the newest line is the system failure notice — the oldest
    // seeded percepts fell off the front rather than the buffer growing.
    let inbox = harness.world.characters[&sven].inbox();
    assert_eq!(inbox.len(), INBOX_MAX_ENTRIES);
    assert!(
        !inbox.last().unwrap().starts_with("percept"),
        "the most recent entry should be the failure notice, not a seeded percept: {:?}",
        inbox.last()
    );
}

/// 14. `test_player_is_skipped_and_round_robin_is_global`
#[test]
fn player_is_skipped_and_round_robin_is_global() {
    let mut harness = Harness::new(Box::new(noop), 0.0);
    harness.start();
    harness.poll_times(5);

    let names = harness.prompted_names();
    assert_eq!(names[..4], ["Sven", "Conny", "Ilse", "Sven"]);
    assert!(!names.contains(&"Player".to_string()), "{names:?}");
}

/// 15. `test_priority_runs_after_current_then_round_robin_resumes`
#[test]
fn priority_runs_after_current_then_round_robin_resumes() {
    let mut harness = Harness::new(Box::new(noop), 0.0);
    harness.start();
    harness.poll_pending(); // Sven's turn is outstanding

    // The priority lands while the request is in flight: it does not preempt it.
    assert!(
        harness
            .scheduler
            .prioritize(&harness.world, &actor("k0fb1"), false, harness.now)
    );
    // The player can never be scheduled, so the handoff is a silent no-op.
    assert!(
        !harness
            .scheduler
            .prioritize(&harness.world, &actor("player"), false, harness.now)
    );

    harness.poll_times(3);
    // Round robin alone would have given Conny the second turn.
    assert_eq!(harness.prompted_names()[..3], ["Sven", "Ilse", "Conny"]);
}

/// Two handoffs landing between turns must BOTH run, oldest first. The lane
/// used to be a single last-write-wins slot: the second event silently erased
/// the first, and while the on-stage idle rotation eventually recovered the
/// dropped actor, an off-stage exchange (two NPCs talking in a far ward) has
/// no rotation to fall back on — the conversation just died.
#[test]
fn queued_handoffs_all_fire_oldest_first() {
    let mut harness = Harness::new(Box::new(noop), 0.0);
    harness.start();
    harness.poll_pending(); // Sven's turn is outstanding

    assert!(
        harness
            .scheduler
            .prioritize(&harness.world, &actor("k0fb1"), false, harness.now)
    );
    assert!(
        harness
            .scheduler
            .prioritize(&harness.world, &actor("cb947"), false, harness.now)
    );
    // Re-queueing an already-queued actor is accepted but must not buy a
    // second turn: the one turn answers everything that has reached them.
    assert!(
        harness
            .scheduler
            .prioritize(&harness.world, &actor("k0fb1"), false, harness.now)
    );

    harness.poll_times(4);
    // Ilse then Conny drain from the queue in arrival order; the round robin
    // then resumes where it left off (Conny again), which also proves the
    // re-queued Ilse did not buy a second priority turn.
    assert_eq!(
        harness.prompted_names()[..4],
        ["Sven", "Ilse", "Conny", "Conny"]
    );
}

/// 16. `test_prioritize_immediate_pulls_the_next_turn_forward`
#[test]
fn prioritize_immediate_pulls_the_next_turn_forward() {
    let mut harness = Harness::with_backoff(Box::new(noop), 100.0, 60.0, 1_000.0);
    harness.start();
    harness.poll(); // submit Sven
    harness.poll(); // apply Sven → next turn is 100 s away

    assert_eq!(harness.prompted_names(), ["Sven"]);
    // The frozen clock sits inside the inter-turn delay.
    harness.poll_times(2);
    assert_eq!(harness.prompted_names(), ["Sven"]);

    assert!(
        harness
            .scheduler
            .prioritize(&harness.world, &actor("k0fb1"), true, harness.now)
    );
    harness.poll();
    assert_eq!(harness.prompted_names(), ["Sven", "Ilse"]);
}

/// 17. `test_prioritize_without_immediate_keeps_the_turn_delay`
#[test]
fn prioritize_without_immediate_keeps_the_turn_delay() {
    let mut harness = Harness::with_backoff(Box::new(noop), 100.0, 60.0, 1_000.0);
    harness.start();
    harness.poll();
    harness.poll();
    assert_eq!(harness.prompted_names(), ["Sven"]);

    assert!(
        harness
            .scheduler
            .prioritize(&harness.world, &actor("k0fb1"), false, harness.now)
    );
    harness.poll_times(5);
    assert_eq!(
        harness.prompted_names(),
        ["Sven"],
        "a plain prioritize changes selection, never timing"
    );

    harness.now += 200.0;
    harness.poll();
    assert_eq!(harness.prompted_names(), ["Sven", "Ilse"]);
}

/// 18. `test_targeted_say_hands_the_next_turn_to_the_addressee`
#[test]
fn targeted_say_hands_the_next_turn_to_the_addressee() {
    let mut harness = Harness::new(
        Box::new(|_, call| {
            if call == 0 {
                Ok(r#"say {"target": "k0fb1", "text": "Ilse, a word?"}"#.to_string())
            } else {
                noop("", call)
            }
        }),
        0.0,
    );
    harness.start();
    harness.poll_times(4);

    assert_eq!(harness.prompted_names()[..3], ["Sven", "Ilse", "Conny"]);
}

/// A transcription and an already-running background turn can complete in the
/// same engine poll. The background reply's targeted `say` must not overwrite
/// the listener the player just woke, and its ordinary delay must not postpone
/// that listener either (session 76's Gile/Dunstan/Mote starvation).
#[test]
fn player_reaction_survives_a_same_poll_background_handoff() {
    let mut harness = Harness::new(
        Box::new(|_, call| {
            if call == 0 {
                Ok(r#"say {"target": "k0fb1", "text": "Ilse, a word?"}"#.to_string())
            } else {
                noop("", call)
            }
        }),
        100.0,
    );
    player_beside_sven(&mut harness.world);
    harness.start();
    harness.poll_pending(); // Sven is already thinking.

    player_says(&mut harness.world, "cb947", "Where is the rail?");
    assert!(harness.scheduler.prioritize_player_reaction(
        &harness.world,
        &actor("cb947"),
        harness.now
    ));

    // Sven completes and hands the ordinary slot to Ilse. Conny's protected
    // player reaction still submits in this same poll, despite the 100 s delay.
    harness.poll();
    assert_eq!(harness.prompted_names(), ["Sven", "Conny"]);
    assert_eq!(
        harness.scheduler.in_flight_actor_id(),
        Some(&actor("cb947"))
    );
    assert!(harness.scheduler.in_flight_is_player_reaction());
    assert_eq!(
        harness.scheduler.priority_actor_id(),
        Some(&actor("k0fb1")),
        "the background handoff remains queued behind the player reaction"
    );
    assert_eq!(
        md_section(&harness.cognition.prompts[1], "since_your_last_turn").unwrap(),
        [
            "A stranger (id player) said to you: \"Where is the rail?\"",
            "Sven said to a stranger (id k0fb1): \"Ilse, a word?\""
        ]
    );

    // Once Conny succeeds, the ordinary delay and handoff resume unchanged.
    harness.poll();
    harness.now += 100.0;
    harness.poll();
    assert_eq!(harness.prompted_names(), ["Sven", "Conny", "Ilse"]);
}

/// 19. `test_broadcast_say_leaves_round_robin_order_unchanged`
#[test]
fn broadcast_say_leaves_round_robin_order_unchanged() {
    let mut harness = Harness::new(
        Box::new(|_, call| {
            if call == 0 {
                Ok(r#"say {"text": "gather round, everyone"}"#.to_string())
            } else {
                noop("", call)
            }
        }),
        0.0,
    );
    harness.start();
    harness.poll_times(4);

    assert_eq!(harness.prompted_names()[..3], ["Sven", "Conny", "Ilse"]);
}

/// 20. `test_say_to_the_player_leaves_round_robin_order_unchanged`
#[test]
fn say_to_the_player_leaves_round_robin_order_unchanged() {
    let mut harness = Harness::new(
        Box::new(|_, call| {
            if call == 0 {
                Ok(r#"say {"target": "player", "text": "hello traveller"}"#.to_string())
            } else {
                noop("", call)
            }
        }),
        0.0,
    );
    player_beside_sven(&mut harness.world);
    harness.start();
    harness.poll_times(4);

    // The say must actually have applied, or this test proves nothing.
    assert!(
        harness
            .transcript
            .iter()
            .any(|line| line.contains("hello traveller")),
        "{:?}",
        harness.transcript
    );
    assert_eq!(harness.prompted_names()[..3], ["Sven", "Conny", "Ilse"]);
}

/// 21. `test_malformed_actions_become_system_events_without_crash`
#[test]
fn malformed_actions_become_system_events_without_crash() {
    let mut harness = Harness::new(
        Box::new(|_, _| {
            Ok([
                r#"say {"text": 9}"#,
                r#"offer_item {"item_id": ["bad"]}"#,
                "unknown_verb {}",
                "not even an action",
            ]
            .join("\n"))
        }),
        100.0,
    );
    harness.start();
    harness.poll();
    harness.poll();

    let inbox = harness.inbox("sv3n1");
    assert!(inbox.len() >= 4, "{inbox:?}");
    assert!(
        inbox.iter().all(|line| line.starts_with("system:")),
        "{inbox:?}"
    );
    // The parse error is reported before the failed actions, and the failing
    // action is quoted back verbatim enough to self-correct from.
    assert_eq!(
        inbox[0],
        "system: your last output was invalid: not understood: not even an action"
    );
    // Python interpolated the args `dict`, so the quoted-back action carries
    // Python literal syntax — `{'text': 9}`, not JSON's `{"text":9}`. The line
    // is re-rendered as `since_your_last_turn` next turn, so it is prompt bytes.
    assert!(
        inbox[1].starts_with(r#"system: your action "say {'text': 9}" failed: "#),
        "{}",
        inbox[1]
    );
    assert!(
        inbox[2].starts_with(r#"system: your action "offer_item {'item_id': ['bad']}" failed: "#),
        "{}",
        inbox[2]
    );
}

/// The `system:` line renders the args exactly like `str(dict)`: single quotes,
/// `': '` / `', '` separators, and Python's `None`/`True`/`False`/float
/// spellings — never JSON. The line is re-rendered as `since_your_last_turn` on
/// the next turn, so these are prompt bytes.
#[test]
fn a_failed_actions_args_are_rendered_as_a_python_dict() {
    let mut harness = Harness::new(
        Box::new(|_, _| {
            Ok([
                r#"offer_item {"item_id": "nope", "target": null}"#,
                r#"say {"text": "it's 1.5", "flag": true, "n": 3}"#,
                "eat {}",
            ]
            .join("\n"))
        }),
        100.0,
    );
    harness.start();
    harness.poll();
    harness.poll();

    let inbox = harness.inbox("sv3n1");
    assert!(
        inbox[0].starts_with(
            r#"system: your action "offer_item {'item_id': 'nope', 'target': None}" failed: "#
        ),
        "{}",
        inbox[0]
    );
    // Note the key order: `serde_json::Map` is a BTreeMap, so it sorts where
    // Python's dict kept the model's document order (`text` was written first).
    // A known, documented residual — see `render_args`.
    assert!(
        inbox[1].starts_with(
            r#"system: your action "say {'flag': True, 'n': 3, 'text': "it's 1.5"}" failed: "#
        ),
        "{}",
        inbox[1]
    );
    // The empty dict is the one case where JSON and Python agree — which is why
    // the golden fixtures (`say {}` / `eat {}`) never caught the divergence.
    assert!(
        inbox[2].starts_with(r#"system: your action "eat {}" failed: "#),
        "{}",
        inbox[2]
    );
}

/// 22. `test_busy_floor_holds_a_completed_result_without_new_submissions`
#[test]
fn busy_floor_holds_a_completed_result_without_new_submissions() {
    let mut harness = Harness::new(
        Box::new(|_, _| Ok(r#"say {"text": "held until the floor frees"}"#.to_string())),
        0.0,
    );
    harness.floor_busy = true;
    harness.start();
    harness.poll(); // submit Sven
    harness.poll(); // his reply arrives — and is held
    assert!(harness.scheduler.has_held_result());

    // Held: nothing applied, the turn stays in flight, and even with a zero
    // inter-turn delay no new turn may start behind the held one.
    harness.poll_times(5);
    assert_eq!(harness.prompted_names(), ["Sven"]);
    assert_eq!(harness.transcript, Vec::<String>::new());
    assert_eq!(
        harness.scheduler.in_flight_actor_id(),
        Some(&actor("sv3n1"))
    );
    // A held result is not archived — it has not been harvested yet.
    assert!(
        !harness
            .events
            .iter()
            .any(|event| matches!(event, SchedulerEvent::PromptExchange { .. }))
    );

    harness.floor_busy = false;
    harness.poll();

    assert!(!harness.scheduler.has_held_result());
    assert_eq!(harness.transcript.len(), 1);
    assert!(harness.transcript[0].contains("held until the floor frees"));
    // The same poll that applied the held turn started the next one.
    assert_eq!(harness.prompted_names(), ["Sven", "Conny"]);
}

/// 23. `test_provider_failure_uses_backoff_and_preserves_service`
#[test]
fn provider_failure_uses_backoff_and_preserves_service() {
    let mut harness = Harness::with_backoff(
        Box::new(|_, _| Err(CognitionError::new("TimeoutError"))),
        1.0,
        8.0,
        0.0,
    );
    let question = "player question that must survive a failed call".to_string();
    harness
        .world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .inbox
        .push(question.clone());

    harness.start();
    harness.poll(); // submit
    harness.poll(); // fail

    let inbox = harness.inbox("sv3n1");
    assert_eq!(inbox[0], question, "the restored percept comes first");
    assert!(
        inbox.last().unwrap().contains("provider failed"),
        "{inbox:?}"
    );
    assert!(harness.scheduler.in_flight_actor_id().is_none());

    // delay 1, cap 8 → the first failure retries after exactly one second.
    harness.poll();
    assert!(harness.scheduler.in_flight_actor_id().is_none());
    harness.now = 1.0;
    harness.poll();
    assert!(harness.scheduler.in_flight_actor_id().is_some());
}

// ------------------------------------------------- Rust-only port contracts

/// Python logged a failed turn in two places, with two *different* strings: the
/// stderr line got `type(error).__name__` (`scheduler.py:242-246`), the prompt
/// archive got `repr(error)` (`scheduler.py:205-213`, prompt.md §5.2).
///
/// Collapsing both into the kind is what makes a 401, a rate limit and an
/// outage indistinguishable in `logs/latest_session/prompts/`, so the split is
/// pinned here.
#[test]
fn a_failed_turn_prints_the_kind_and_archives_the_detail() {
    let mut harness = Harness::new(
        Box::new(|_, _| {
            Err(CognitionError::detailed(
                "LlmHttpError",
                "LlmHttpError: provider returned 401: {\"error\": \"invalid api key\"}",
            ))
        }),
        0.0,
    );
    harness.start();
    harness.poll(); // submit
    harness.poll(); // fail

    // The one-line diagnostic stays short — the kind, exactly as Python printed.
    assert!(
        harness
            .diagnostics()
            .contains(&"[smart actors] LLM request for Sven failed: LlmHttpError"),
        "{:?}",
        harness.diagnostics()
    );

    // The archive keeps the story: the status code and the provider's message.
    let archived = harness
        .events
        .iter()
        .find_map(|event| match event {
            SchedulerEvent::PromptExchange { error, .. } => error.as_deref(),
            _ => None,
        })
        .expect("a failed exchange is still archived");
    assert_eq!(
        archived,
        "LlmHttpError: provider returned 401: {\"error\": \"invalid api key\"}"
    );
}

/// The `system:` lines the scheduler fabricates are model-visible text: pin the
/// wording so a refactor cannot quietly reword what the LLM is told.
#[test]
fn the_system_inbox_lines_keep_their_wording() {
    let mut harness = Harness::with_backoff(
        Box::new(|_, _| Err(CognitionError::new("TimeoutError"))),
        1.0,
        8.0,
        0.0,
    );
    harness.start();
    harness.poll();
    harness.poll();
    assert_eq!(
        harness.inbox("sv3n1"),
        ["system: the cognition provider failed; your turn will be retried later"]
    );

    // The submit-refusal branch restores the percepts it drained and says so.
    let mut world = seed_world();
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .inbox = vec!["a percept".to_string()];
    let mut scheduler = NpcScheduler::new(llm_turn_order(&world), 0.0, 60.0, 0.0);
    scheduler.start(0.0);
    let events = scheduler.poll(
        0.0,
        &mut world,
        &mut Vec::new(),
        &mut Vec::new(),
        false,
        IdleGate::All,
        &mut BusyCognition,
        &prompt_env(),
    );

    assert_eq!(
        world.characters[&actor("sv3n1")].inbox(),
        ["a percept", "system: the cognition worker is busy"]
    );
    assert!(scheduler.in_flight_actor_id().is_none());
    let degraded = events
        .iter()
        .find_map(|event| match event {
            SchedulerEvent::Status(status) if status.state == "degraded" => Some(status),
            _ => None,
        })
        .expect("a refused submit degrades");
    assert_eq!(
        degraded.message.as_deref(),
        Some("cognition worker is busy")
    );
}

/// The status payloads are wire format for the HUD (D30), `{backoff:g}` included.
#[test]
fn status_events_carry_the_python_wire_strings() {
    let mut harness = Harness::with_backoff(
        Box::new(|_, call| {
            if call == 0 {
                noop("", call)
            } else {
                Err(CognitionError::new("TimeoutError"))
            }
        }),
        1.0,
        8.0,
        0.0,
    );
    harness.start();
    harness.poll(); // submit Sven → thinking
    harness.now = 1.0;
    harness.poll(); // apply Sven → idle (the delay defers Conny to the next poll)
    harness.now = 2.0;
    harness.poll(); // submit Conny → thinking
    harness.poll(); // Conny's provider failed → degraded

    let statuses = harness.statuses();
    assert!(
        statuses
            .iter()
            .all(|status| status.subsystem == Subsystem::Llm)
    );
    assert!(statuses.iter().all(|status| status.backend.is_none()));

    assert_eq!(statuses[0].state, "thinking");
    assert_eq!(statuses[0].actor_id.as_ref(), Some(&actor("sv3n1")));
    assert_eq!(statuses[1].state, "idle");
    assert_eq!(statuses[1].actor_id.as_ref(), Some(&actor("sv3n1")));

    let degraded = statuses.last().unwrap();
    assert_eq!(degraded.state, "degraded");
    assert_eq!(degraded.actor_id.as_ref(), Some(&actor("cb947")));
    // `%g`, so "1 seconds" — not "1.0 seconds".
    assert_eq!(
        degraded.message.as_deref(),
        Some("provider request failed; retrying in 1 seconds")
    );
    assert!(
        harness
            .diagnostics()
            .contains(&"[smart actors] LLM request for Conny failed: TimeoutError")
    );
}

/// Every harvested exchange is archived — successes, failures, and the prompt
/// that produced them (D24).
#[test]
fn every_harvested_exchange_is_archived() {
    let mut harness = Harness::new(
        Box::new(|_, call| {
            if call == 0 {
                Ok("wait {}".to_string())
            } else {
                Err(CognitionError::new("TimeoutError"))
            }
        }),
        0.0,
    );
    harness.start();
    harness.poll_times(3);

    let exchanges: Vec<(&str, Option<&str>, Option<&str>)> = harness
        .events
        .iter()
        .filter_map(|event| match event {
            SchedulerEvent::PromptExchange {
                actor_name,
                answer,
                error,
                prompt,
                duration_seconds,
                ..
            } => {
                assert!(
                    prompt.contains("**since_your_last_turn**"),
                    "the archive keeps the prompt"
                );
                assert_eq!(*duration_seconds, 0.25, "the backend measured it");
                Some((actor_name.as_str(), answer.as_deref(), error.as_deref()))
            }
            _ => None,
        })
        .collect();

    assert_eq!(
        exchanges,
        [
            ("Sven", Some("wait {}"), None),
            ("Conny", None, Some("TimeoutError")),
        ]
    );
}

/// D17: the size limit is enforced on receipt, so every backend — fakes
/// included — is covered, and an oversized reply behaves exactly like a
/// provider failure.
#[test]
fn an_oversized_reply_is_a_provider_failure() {
    let mut harness = Harness::new(
        Box::new(|_, _| Ok("é".repeat(cathedral_sim::MAX_LLM_REPLY_CHARS + 1))),
        0.0,
    );
    harness.start();
    harness.poll();
    harness.poll();

    assert_eq!(
        harness.inbox("sv3n1"),
        ["system: the cognition provider failed; your turn will be retried later"]
    );
    let archived = harness
        .events
        .iter()
        .find_map(|event| match event {
            SchedulerEvent::PromptExchange { error, answer, .. } => Some((error, answer)),
            _ => None,
        })
        .expect("the oversized exchange is still archived");
    // Python raised `ValueError` inside the worker thread, and the log only ever
    // showed the exception *kind* (`type(error).__name__`) — which is also what
    // `CognitionError` carries for every real backend. So the diagnostic reads
    // exactly as it did in Python.
    assert_eq!(archived.0.as_deref(), Some("ValueError"));
    assert!(archived.1.is_none(), "an oversized reply is not an answer");
    assert!(
        harness
            .diagnostics()
            .contains(&"[smart actors] LLM request for Sven failed: ValueError"),
        "{:?}",
        harness.diagnostics()
    );

    // A reply of exactly the limit is fine — and these are Unicode scalars, not
    // bytes: the 2-byte é above is over the limit by one *character*.
    let mut harness = Harness::new(
        Box::new(|_, _| {
            let mut reply = "wait {}\n".to_string();
            reply.push_str(&"#".repeat(cathedral_sim::MAX_LLM_REPLY_CHARS - reply.chars().count()));
            Ok(reply)
        }),
        0.0,
    );
    harness.start();
    harness.poll();
    harness.poll();
    assert!(harness.inbox("sv3n1").is_empty(), "the reply was accepted");
}

/// D10: completions are matched by request id. Anything else is stale — it is
/// dropped on the spot rather than poisoning a later turn's result slot.
#[test]
fn a_completion_for_an_unknown_request_is_discarded_as_stale() {
    let mut harness = Harness::new(Box::new(noop), 100.0);
    harness.start();
    harness.poll_pending(); // Sven is in flight, request id 1

    let mut spurious = vec![Completion {
        request_id: RequestId(999),
        result: Ok(r#"say {"text": "I was never asked"}"#.to_string()),
        duration_seconds: 0.0,
    }];
    harness.poll_with(&mut spurious);

    assert_eq!(
        harness.scheduler.in_flight_actor_id(),
        Some(&actor("sv3n1")),
        "a stale completion must not end the real turn"
    );
    assert!(harness.transcript.is_empty());
    let degraded = harness.statuses();
    let degraded = degraded.last().unwrap();
    assert_eq!(degraded.state, "degraded");
    assert_eq!(degraded.actor_id, None, "there is no actor to blame");
    assert_eq!(
        degraded.message.as_deref(),
        Some("discarded a stale LLM result")
    );

    // The real completion still lands.
    harness.poll();
    assert!(harness.scheduler.in_flight_actor_id().is_none());
}

/// scheduler.md risk 3: a result whose actor stopped being an LLM mid-flight is
/// discarded — and its drained percepts die with it. Deliberate: there is no
/// longer anyone to re-read them.
#[test]
fn a_result_for_a_no_longer_llm_actor_is_discarded_with_its_percepts() {
    let mut harness = Harness::new(Box::new(noop), 0.0);
    player_beside_sven(&mut harness.world);
    player_says(&mut harness.world, "sv3n1", "psst");
    harness.start();
    harness.poll_pending(); // Sven's prompt drained the percept

    // The world changes under the request.
    harness
        .world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .sheet
        .control = Control::Player;

    harness.poll();

    assert!(harness.transcript.is_empty());
    assert!(harness.world.characters[&actor("sv3n1")].inbox().is_empty());
    assert!(
        harness
            .events
            .iter()
            .any(|event| matches!(event, SchedulerEvent::PromptExchange { .. })),
        "a stale result is still an exchange that happened"
    );
    // The discard ends the turn, so the same poll already moved on to Conny —
    // find the degraded row rather than assuming it is last.
    assert!(
        harness
            .statuses()
            .iter()
            .any(|status| status.state == "degraded"
                && status.actor_id.is_none()
                && status.message.as_deref() == Some("discarded a stale LLM result"))
    );
}

/// scheduler.md risk 4: `close` stops submissions, but a turn that already cost
/// a provider call is still applied. Ported deliberately.
#[test]
fn polling_after_close_still_applies_the_finished_turn() {
    let mut harness = Harness::new(
        Box::new(|_, _| Ok(r#"say {"text": "the last word"}"#.to_string())),
        0.0,
    );
    harness.start();
    harness.poll_pending();
    harness.scheduler.close();

    harness.poll();
    assert_eq!(harness.transcript.len(), 1);
    assert!(harness.transcript[0].contains("the last word"));
    // …but nothing new starts.
    harness.poll_times(3);
    assert_eq!(harness.prompted_names(), ["Sven"]);
    assert!(!harness.scheduler.running());
}

/// scheduler.md risks 7-8: the rotation is frozen at construction, and a
/// selection spent on an actor who can no longer act is simply burnt — the next
/// poll moves on rather than retrying.
#[test]
fn an_actor_who_stops_being_an_llm_burns_its_slot_without_stalling_the_stream() {
    let mut harness = Harness::new(Box::new(noop), 0.0);
    harness
        .world
        .characters
        .get_mut(&actor("cb947"))
        .unwrap()
        .sheet
        .control = Control::Player;

    harness.start();
    harness.poll(); // Sven
    harness.poll(); // apply Sven, select Conny → skipped silently, no submit
    assert_eq!(harness.prompted_names(), ["Sven"]);
    harness.poll(); // the very next poll selects Ilse
    assert_eq!(harness.prompted_names(), ["Sven", "Ilse"]);
}
