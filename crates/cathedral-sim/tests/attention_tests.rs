//! The proximity gate on idle cognition, driven through the whole engine
//! (`features/gate_idle_cognition_on_proximity.md`).
//!
//! The unit tests in `attention.rs` and `scheduler.rs` pin the stage query and
//! the selection lanes in isolation. These pin the thing the feature is actually
//! *for*, and they do it by counting the one artifact that costs money: the
//! `PromptExchange` the engine emits for every LLM call it makes, successful or
//! not.
//!
//! The demo cast — Sven, Conny and Ilse — stands around `z ≈ 113`. The player
//! spawns among them; a field is anything a few hundred metres away.

mod prompt_support;

use std::{cell::RefCell, rc::Rc};

use cathedral_sim::{
    ActorId, Capabilities, Cognition, CognitionBusy, Engine, EngineCommand, EngineConfig,
    EngineMessage, FakeCognition, IdleCognitionMode, NullSight, NullTranscription, NullTts,
    SpatialActorUpdate, StageConfig, TtsBackendKind, Vec3, WorldSeed, ids::RequestId,
};
use prompt_support::{areas, catalog, prompt_env};

/// Shoulder to shoulder with all three NPCs.
const IN_THE_CROWD: Vec3 = Vec3::new(0.0, 0.91, 111.0);
/// Nobody within 380 m.
const IN_A_FIELD: Vec3 = Vec3::new(0.0, 0.91, 500.0);

/// [`FakeCognition`] behind a handle: the engine owns its `Box<dyn Cognition>`,
/// and the test still has to hand the staged completions back the way the host
/// does.
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
    spatial_seq: i64,
}

impl Harness {
    /// A gated engine with a live (fake) provider, and the player standing where
    /// the caller says.
    fn staged(player_spawn: Vec3) -> Self {
        let cognition = SharedCognition::default();
        let engine = Engine::new(
            EngineConfig {
                fake_mode: true,
                turn_delay_seconds: 0.0,
                tts_selected: TtsBackendKind::Off,
                idle_mode: IdleCognitionMode::Stage,
                stage: StageConfig::default(),
                ..EngineConfig::default()
            },
            &WorldSeed::from_json_str(&prompt_support::demo_seed()).expect("the demo seed loads"),
            areas(),
            catalog(),
            prompt_env(),
            Box::new(cognition.clone()),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
            (player_spawn, 0.0),
            0,
            0.0,
        )
        .expect("the seeded world has a player");
        Harness {
            engine,
            cognition,
            now: 0.0,
            spatial_seq: 0,
        }
    }

    fn send_all(&mut self, commands: Vec<EngineCommand>) -> Vec<EngineMessage> {
        let mut queued: Vec<EngineCommand> = self
            .cognition
            .0
            .borrow_mut()
            .drain_completions()
            .into_iter()
            .map(EngineCommand::LlmCompletion)
            .collect();
        queued.extend(commands);
        self.engine.poll(self.now, queued)
    }

    /// Pump for `seconds` of sim time and return every LLM call it cost.
    ///
    /// The step is a tenth of a second: fine enough that the floor's reading
    /// pauses resolve, coarse enough that a multi-minute run is instant.
    fn run(&mut self, seconds: f64) -> Vec<ActorId> {
        let mut prompts = Vec::new();
        let steps = (seconds / 0.1).round() as u32;
        for _ in 0..steps {
            for message in self.send_all(Vec::new()) {
                if let EngineMessage::PromptExchange { actor_id, .. } = message {
                    prompts.push(actor_id);
                }
            }
            self.now += 0.1;
        }
        prompts
    }

    fn move_player(&mut self, position_m: Vec3) {
        self.spatial_seq += 1;
        let update = SpatialActorUpdate::new(ActorId::from_raw("player"), position_m, None);
        self.send_all(vec![EngineCommand::SpatialUpdate {
            spatial_seq: self.spatial_seq,
            updates: vec![update],
        }]);
    }
}

fn distinct(actors: &[ActorId]) -> Vec<&str> {
    let mut names: Vec<&str> = actors.iter().map(ActorId::as_str).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// The headline number. Today this run costs ~1,100 provider calls an hour,
/// sustained, whether or not there is anybody to talk to.
#[test]
fn alone_in_a_field_costs_nothing() {
    let mut harness = Harness::staged(IN_A_FIELD);
    assert_eq!(harness.run(300.0), Vec::<ActorId>::new());
}

/// …and the same five minutes in the crowd keeps the whole cast thinking. The
/// point of the gate is *where* the money goes, not that less of it is spent.
#[test]
fn standing_in_a_crowd_keeps_everyone_nearby_thinking() {
    let mut harness = Harness::staged(IN_THE_CROWD);
    let prompts = harness.run(300.0);
    assert!(prompts.len() > 10, "the crowd went quiet: {prompts:?}");
    // Every call, without exception, was spent on somebody the player is
    // standing next to.
    assert_eq!(distinct(&prompts), ["cb947", "k0fb1", "sv3n1"]);
}

/// The gate follows the player, poll by poll: the city stops behind him and
/// starts again in front of him.
#[test]
fn walking_away_stops_the_thinking_and_walking_back_resumes_it() {
    let mut harness = Harness::staged(IN_THE_CROWD);
    assert!(!harness.run(60.0).is_empty());

    harness.move_player(IN_A_FIELD);
    assert_eq!(
        harness.run(120.0),
        Vec::<ActorId>::new(),
        "the cast kept thinking about a player who had left"
    );

    harness.move_player(IN_THE_CROWD);
    assert!(!harness.run(60.0).is_empty(), "the cast never woke back up");
}

/// An empty stage must not deafen the city. The gate is on the lane that fires
/// because *time passed* — never on the lanes that fire because something
/// happened, which is the only way an ambient NPC ever thinks at all.
#[test]
fn a_sound_still_reaches_an_npc_the_player_cannot_see() {
    let mut harness = Harness::staged(IN_A_FIELD);
    assert!(harness.run(10.0).is_empty());

    // A bell rings among the NPCs, 380 m from the player. He does not hear it;
    // they do, and the nearest witness is nudged into a turn.
    harness.send_all(vec![EngineCommand::DebugSound {
        sound_id: "town_bell".into(),
        position_m: Vec3::new(0.0, 4.0, 113.0),
    }]);

    let prompts = harness.run(10.0);
    assert_eq!(
        prompts.len(),
        1,
        "exactly one nudge per sound, gate or no gate: {prompts:?}"
    );
}

/// §4: no idle turn may start while the player is mid-utterance. The protected
/// reaction lane is immediate at *selection*, but it cannot preempt a provider
/// call that is already out — so the call must not be made.
#[test]
fn nothing_starts_thinking_while_the_player_holds_the_microphone() {
    let mut harness = Harness::staged(IN_THE_CROWD);
    assert!(!harness.run(30.0).is_empty());

    // The microphone opens. Whatever was in flight finishes; nothing new starts.
    harness.send_all(vec![EngineCommand::PlayerAudioBegin {
        wav_basename: "utterance.wav".into(),
        sample_rate: 24_000,
    }]);
    assert!(harness.engine.speech_router().player_composing());
    let during = harness.run(60.0);
    assert!(
        during.len() <= 1,
        "the cast kept thinking over the player: {during:?}"
    );

    // He gives up on the sentence; the crowd resumes.
    harness.send_all(vec![EngineCommand::PlayerAudioAbort {
        wav_basename: "utterance.wav".into(),
    }]);
    assert!(!harness.engine.speech_router().player_composing());
    assert!(!harness.run(30.0).is_empty());
}
