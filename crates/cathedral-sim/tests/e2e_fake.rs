//! The flagship acceptance test (`test_end_to_end.py`, server-core.md §10.4).
//!
//! The same scripted conversation the Python suite drives through the whole
//! sidecar — player asks a name, Ilse answers, player asks for the coin, Ilse
//! offers it, the player accepts and re-offers it to Conny — driven here against
//! [`Engine`] + [`FakeCognition`] with a fake clock.
//!
//! It is deliberately end-to-end: the fake cognition parses the *rendered*
//! minijinja prompt (D25), so this test fails if the template stops emitting a
//! machine-readable sheet, if the percept wording drifts, if the round robin
//! reorders, if the conversation floor deadlocks, or if any action's validation
//! changes. Nothing here is mocked but the provider and the clock.

mod prompt_support;

use std::{cell::RefCell, rc::Rc};

use cathedral_sim::{
    ActorId, Capabilities, Cognition, CognitionBusy, DANCE_MAX_SECONDS, Engine, EngineCommand,
    EngineConfig, EngineMessage, FakeCognition, GestureKind, ItemId, NullSight, NullTranscription,
    NullTts, PublicSnapshot, RequestId, TtsBackendKind, Vec3, WorldSeed, apply_action,
};
use prompt_support::{areas, catalog, demo_seed, prompt_env};

/// `hello()`'s spawn in the Python tests: on the forecourt, within 20 m of the
/// whole cast and within 4 m of Ilse, so a broadcast say reaches everyone and
/// the coin can change hands.
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.91, 111.0);
/// Sim seconds per pump. Ilse's answer holds the floor for its reading estimate
/// (2 + 32/15 ≈ 4.1 s), which this steps through in a few polls.
const CLOCK_STEP_SECONDS: f64 = 0.5;
/// Enough for every wait below with room to spare; a real deadlock trips it.
const MAX_POLLS: usize = 400;

const ILSE: &str = "k0fb1";
const CONNY: &str = "cb947";
const SVEN: &str = "sv3n1";
const PLAYER: &str = "player";
const COIN: &str = "c0prs";

/// The engine owns its `Box<dyn Cognition>`; the test still has to drain the
/// staged completions and feed them back as commands — exactly the host's job.
#[derive(Clone, Default)]
struct SharedCognition(Rc<RefCell<FakeCognition>>);

impl Cognition for SharedCognition {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request(prompt)
    }
}

struct Harness {
    engine: Engine,
    cognition: SharedCognition,
    now: f64,
    /// Every message the engine has ever emitted, in order.
    messages: Vec<EngineMessage>,
}

impl Harness {
    /// Fake mode, text-only speech, zero turn delay — `SmartActorServer(fake_mode
    /// =True, speech_backend=TextOnlySpeech(), turn_delay_seconds=0)`.
    fn new() -> Self {
        let cognition = SharedCognition::default();
        let seed = WorldSeed::from_json_str(&demo_seed()).expect("the demo seed");
        let engine = Engine::new(
            EngineConfig {
                fake_mode: true,
                turn_delay_seconds: 0.0,
                tts_selected: TtsBackendKind::Off,
                ..EngineConfig::default()
            },
            &seed,
            areas(),
            catalog(),
            prompt_env(),
            Box::new(cognition.clone()),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
            (PLAYER_SPAWN, 0.0),
            0,
            0.0,
        )
        .expect("the seeded world has a player");
        Self {
            engine,
            cognition,
            now: 0.0,
            messages: Vec::new(),
        }
    }

    fn send(&mut self, command: EngineCommand) {
        self.pump(vec![command]);
    }

    /// One pump, carrying whatever the fake provider finished since the last one.
    fn pump(&mut self, commands: Vec<EngineCommand>) {
        let completions: Vec<EngineCommand> = self
            .cognition
            .0
            .borrow_mut()
            .drain_completions()
            .into_iter()
            .map(EngineCommand::LlmCompletion)
            .collect();
        self.poll(completions.into_iter().chain(commands).collect());
    }

    /// One pump that carries a player command but no completion — the frame in
    /// which the provider simply has not answered yet.
    ///
    /// It is the only way to observe a percept *before* the turn stream consumes
    /// it: with a turn already in flight the scheduler can neither apply nor
    /// submit, so the inbox the say lands in is still there to assert on. Python
    /// got this for free — `handle_envelope` ran on the protocol thread and the
    /// scheduler polled on another.
    fn poll_command(&mut self, command: EngineCommand) {
        assert!(
            self.engine.scheduler().in_flight_actor_id().is_some(),
            "a turn must be in flight, or this poll would drain the inbox it is about to fill"
        );
        self.poll(vec![command]);
    }

    fn poll(&mut self, commands: Vec<EngineCommand>) {
        let messages = self.engine.poll(self.now, commands);
        self.messages.extend(messages);
    }

    /// Pump — advancing the clock, so the inter-turn delay and the conversation
    /// floor both resolve — until `condition` holds.
    fn pump_until(&mut self, what: &str, condition: impl Fn(&Harness) -> bool) {
        for _ in 0..MAX_POLLS {
            if condition(self) {
                return;
            }
            self.pump(Vec::new());
            self.now += CLOCK_STEP_SECONDS;
        }
        panic!("gave up waiting for {what} after {MAX_POLLS} polls");
    }

    fn player_say(&mut self, request_id: &str, text: &str, spatial_seq: i64) {
        self.poll_command(EngineCommand::DebugPlayerSay {
            request_id: request_id.to_string(),
            text: text.to_string(),
            // Broadcast: open speech, no gaze target.
            target_id: None,
            position_m: PLAYER_SPAWN,
            spatial_seq,
        });
    }

    fn holds(&self, actor: &str) -> Vec<ItemId> {
        self.engine.world().characters[&actor_id(actor)]
            .holds()
            .to_vec()
    }

    fn inbox_len(&self, actor: &str) -> usize {
        self.engine.world().characters[&actor_id(actor)]
            .inbox()
            .len()
    }

    /// Did Ilse's introduction reach the player as a speech event?
    fn ilse_introduced_herself(&self) -> bool {
        self.messages.iter().any(|message| {
            matches!(
                message,
                EngineMessage::Speech { speaker_id, text, .. }
                    if speaker_id.as_str() == ILSE && text.contains("My name is Ilse")
            )
        })
    }

    fn latest_snapshot(&self) -> &PublicSnapshot {
        self.messages
            .iter()
            .rev()
            .find_map(|message| match message {
                EngineMessage::Snapshot(snapshot) => Some(snapshot),
                EngineMessage::Ready { snapshot, .. } => Some(snapshot),
                _ => None,
            })
            .expect("the engine has published at least the ready snapshot")
    }

    fn command_results(&self) -> Vec<(&str, bool, &str)> {
        self.messages
            .iter()
            .filter_map(|message| match message {
                EngineMessage::CommandResult {
                    request_id,
                    success,
                    message,
                    ..
                } => Some((request_id.as_str(), *success, message.as_str())),
                _ => None,
            })
            .collect()
    }
}

fn actor_id(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

fn item_id(id: &str) -> ItemId {
    ItemId::from_raw(id)
}

/// Whose sheet a rendered prompt is.
///
/// The name has to be read off the sheet's `**you**` line, not grepped for:
/// every prompt also lists the *other* people's names under `you_see`, so a
/// substring match for `Sven` hits Conny's prompt too. Parsing it is also the
/// assertion that matters — the fake cognition reads the same line.
fn sheet_name(prompt: &str) -> String {
    cathedral_sim::fake::sheet_name(prompt)
        .expect("every prompt carries a machine-readable sheet")
        .to_string()
}

/// server-core.md §10.4, steps 1-7 (Python test 71/24).
#[test]
fn the_scripted_conversation_offers_accepts_and_re_offers_the_coin() {
    let mut harness = Harness::new();
    harness.pump(Vec::new());
    assert!(
        matches!(harness.messages[0], EngineMessage::Ready { .. }),
        "the handshake comes first"
    );

    // 1. Broadcast player speech lands in all three NPC inboxes.
    harness.player_say("ask-name", "What's your name?", 1);
    assert_eq!(harness.inbox_len(SVEN), 1, "Sven heard the question");
    assert_eq!(harness.inbox_len(CONNY), 1, "Conny heard the question");
    assert_eq!(harness.inbox_len(ILSE), 1, "Ilse heard the question");

    // 2. The round robin reaches Ilse (Sven and Conny take no-op turns first) and
    //    the fake answers from the rendered sheet.
    harness.pump_until("Ilse's introduction", Harness::ilse_introduced_herself);

    // 3. Ask for the coin. Her *previous* answer still holds the conversation
    //    floor for its reading estimate, so the offering turn only applies once
    //    that pacing window has passed — which the fake clock steps through.
    harness.player_say("ask-coin", "Please offer me your coin", 2);
    harness.pump_until("Ilse's offer", |harness| {
        harness.engine.world().offers.contains_key(&item_id(COIN))
    });

    // 4. The offer is recorded; the coin has NOT moved.
    let offer = harness.engine.world().offers[&item_id(COIN)].clone();
    assert_eq!(offer.giver_id, actor_id(ILSE));
    assert_eq!(offer.target_id, Some(actor_id(PLAYER)));
    assert!(
        harness.holds(ILSE).contains(&item_id(COIN)),
        "an offered item stays in the giver's hands until it is accepted"
    );
    assert_eq!(
        harness
            .latest_snapshot()
            .offers
            .last()
            .expect("the snapshot carries the offer")
            .item_id,
        item_id(COIN),
        "the game sees the pending offer"
    );

    // 5. The player accepts: the coin moves exactly once and the offer clears.
    harness.send(EngineCommand::PlayerAccept {
        request_id: "accept-coin".to_string(),
        item_id: item_id(COIN),
        position_m: PLAYER_SPAWN,
        spatial_seq: 3,
    });
    assert_eq!(
        harness
            .holds(PLAYER)
            .iter()
            .filter(|held| **held == item_id(COIN))
            .count(),
        1,
        "the coin arrives once, not twice"
    );
    assert!(!harness.holds(ILSE).contains(&item_id(COIN)));
    assert!(!harness.engine.world().offers.contains_key(&item_id(COIN)));

    // 6. The player re-offers it to Conny: he keeps it until Conny accepts.
    harness.send(EngineCommand::PlayerOffer {
        request_id: "offer-conny".to_string(),
        item_id: item_id(COIN),
        target_id: actor_id(CONNY),
        quantity: None,
        position_m: PLAYER_SPAWN,
        spatial_seq: 4,
    });
    assert!(harness.holds(PLAYER).contains(&item_id(COIN)));
    assert_eq!(
        harness.engine.world().offers[&item_id(COIN)].target_id,
        Some(actor_id(CONNY))
    );

    // 7. Nothing the player asked for failed, all four commands answered.
    let results = harness.command_results();
    assert_eq!(
        results.iter().map(|(id, ..)| *id).collect::<Vec<_>>(),
        ["ask-name", "ask-coin", "accept-coin", "offer-conny"],
    );
    assert!(
        results.iter().all(|(_, success, _)| *success),
        "every command_result succeeded: {results:?}"
    );
}

/// The turn stream itself, pinned by the same script: the player is never
/// prompted, and the rotation is the seed's insertion order — not the id sort
/// (`cb947 < k0fb1 < sv3n1`) a `HashMap`/`BTreeMap` would have produced (D12).
#[test]
fn the_round_robin_follows_the_seed_order_and_skips_the_player() {
    let mut harness = Harness::new();
    harness.pump_until("three completed turns", |harness| {
        harness.cognition.0.borrow().prompts().len() >= 3
    });

    let prompts = harness.cognition.0.borrow().prompts().to_vec();
    let names: Vec<String> = prompts.iter().map(|prompt| sheet_name(prompt)).collect();
    assert_eq!(names[..3], ["Sven", "Conny", "Ilse"]);
    assert!(
        !names.iter().any(|name| name == "Player"),
        "the player is never given a turn"
    );
}

/// npc_bodies M4 acceptance, end-to-end: the player asks for a wave, the round
/// reaches a nearby NPC whose scripted reply greets and waves at the player, and
/// the whole gesture path fires — the transient `EngineMessage::Gesture` the
/// host plays the pose from, aimed at the player, witnessed by a bystander NPC
/// whose mind therefore gets a percept it could answer (§3).
#[test]
fn asking_for_a_wave_produces_a_gesture_event_witnessed_by_a_bystander() {
    let mut harness = Harness::new();
    harness.pump(Vec::new());

    harness.player_say("ask-wave", "Please wave at me!", 1);
    harness.pump_until("a wave gesture", |harness| {
        harness.messages.iter().any(|message| {
            matches!(
                message,
                EngineMessage::Gesture { kind: GestureKind::Wave, target_id, .. }
                    if target_id.as_ref().map(ActorId::as_str) == Some(PLAYER)
            )
        })
    });

    let (waver, recipients) = harness
        .messages
        .iter()
        .rev()
        .find_map(|message| match message {
            EngineMessage::Gesture {
                actor_id,
                kind: GestureKind::Wave,
                recipient_ids,
                ..
            } => Some((actor_id.clone(), recipient_ids.clone())),
            _ => None,
        })
        .expect("a wave was emitted");
    // Presented like speech: the player, as the target, is in the recipient set…
    assert!(recipients.iter().any(|id| id.as_str() == PLAYER));
    // …and so is at least one bystander NPC — the mind that can react to it.
    assert!(
        recipients
            .iter()
            .any(|id| id.as_str() != PLAYER && *id != waver),
        "a bystander NPC witnessed the wave: {recipients:?}"
    );
}

/// A looping `dance` rides the snapshot so a player who arrives mid-loop still
/// sees it (§7), and the engine ends it after the 60 s cap even if the dancer
/// never acts again. Driven directly on the world — the fake never dances
/// autonomously — with the fake's staged completions deliberately never fed
/// back, so no turn ever applies the dancer's next action to clear it early.
#[test]
fn a_dance_rides_the_snapshot_and_the_engine_expires_it_after_the_cap() {
    let mut harness = Harness::new();
    harness.pump(Vec::new());

    apply_action(
        harness.engine.world_mut(),
        &actor_id(ILSE),
        "gesture",
        &serde_json::json!({"kind": "dance"}),
    )
    .expect("Ilse can dance");

    let ilse_gesture = |harness: &Harness| {
        harness
            .latest_snapshot()
            .actors
            .iter()
            .find(|actor| actor.id.as_str() == ILSE)
            .expect("Ilse is in the snapshot")
            .active_gesture
    };

    harness.poll(Vec::new());
    assert!(
        harness.messages.iter().any(|message| matches!(
            message,
            EngineMessage::Gesture { actor_id, kind: GestureKind::Dance, .. }
                if actor_id.as_str() == ILSE
        )),
        "the dance emitted a transient trigger"
    );
    assert_eq!(ilse_gesture(&harness), Some(GestureKind::Dance));

    // Past the cap, the next poll clears the loop and republishes the snapshot.
    harness.now += DANCE_MAX_SECONDS + 1.0;
    harness.poll(Vec::new());
    assert_eq!(ilse_gesture(&harness), None);
}
