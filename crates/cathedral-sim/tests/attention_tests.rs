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
    ActorId, Capabilities, Cognition, CognitionBusy, CuriosityConfig, Engine, EngineCommand,
    EngineConfig, EngineMessage, FakeCognition, IdleCognitionMode, NOVELTY_MEMORY_SECONDS,
    NullSight, NullTranscription, NullTts, SpatialActorUpdate, StageConfig, TtsBackendKind, Vec3,
    WorldSeed, ids::RequestId,
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
    /// A proximity-gated engine: on stage is enough to think, every rotation.
    fn staged(player_spawn: Vec3) -> Self {
        Self::build(player_spawn, false, CuriosityConfig::default())
    }

    /// The game's own configuration: on stage **and** with something to react to
    /// (`features/gate_idle_cognition_on_novelty.md`).
    fn with_news(player_spawn: Vec3) -> Self {
        Self::build(player_spawn, true, CuriosityConfig::default())
    }

    /// …and the third gate: whether the person is one who speaks *first*.
    ///
    /// The demo trio carries no lore sheet, so each is maximally curious by
    /// definition (`CURIOSITY_WITHOUT_LORE`) and `scale` becomes the whole city's
    /// chance outright — which is exactly the dial these tests want. `0.0` is a
    /// street of magistrates; `1.0` is the world before this feature.
    fn with_curiosity(player_spawn: Vec3, scale: f64) -> Self {
        Self::build(
            player_spawn,
            true,
            CuriosityConfig {
                enabled: true,
                scale,
            },
        )
    }

    /// A gated engine with a live (fake) provider, and the player standing where
    /// the caller says.
    fn build(
        player_spawn: Vec3,
        idle_requires_news: bool,
        idle_curiosity: CuriosityConfig,
    ) -> Self {
        let cognition = SharedCognition::default();
        let engine = Engine::new(
            EngineConfig {
                fake_mode: true,
                turn_delay_seconds: 0.0,
                tts_selected: TtsBackendKind::Off,
                idle_mode: IdleCognitionMode::Stage,
                stage: StageConfig::default(),
                idle_requires_news,
                idle_curiosity,
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

    /// The player says something out loud, from where he is standing.
    fn player_say(&mut self, request_id: &str, text: &str, position_m: Vec3) {
        self.spatial_seq += 1;
        self.send_all(vec![EngineCommand::DebugPlayerSay {
            request_id: request_id.to_string(),
            text: text.to_string(),
            target_id: None,
            position_m,
            spatial_seq: self.spatial_seq,
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

// ------------------------------------------------------- the novelty gate
//
// `features/gate_idle_cognition_on_novelty.md`. Proximity aimed the firehose at
// the player's head; it did not shrink it. These pin the part that does — and
// they count the same artifact, the `PromptExchange` that costs money.

/// The headline number, and the one that was broken.
///
/// The same five minutes in the same crowd that costs `standing_in_a_crowd…`
/// above a turn every ~3 seconds — every one of them asking somebody with
/// nothing to say whether they have changed their mind about saying nothing.
#[test]
fn standing_in_a_crowd_stops_costing_money_once_the_news_runs_out() {
    let mut harness = Harness::with_news(IN_THE_CROWD);

    // The player arrives: three strangers appear in three sets of eyes, and each
    // is worth exactly one thought.
    let arrival = harness.run(30.0);
    assert_eq!(distinct(&arrival), ["cb947", "k0fb1", "sv3n1"]);

    // Then nothing happens, and nothing keeps happening. Under the proximity
    // gate alone this window costs ~90 calls.
    let settled = harness.run(300.0);
    assert_eq!(
        settled,
        Vec::<ActorId>::new(),
        "the crowd went back to paying to be told nothing happened: {settled:?}"
    );
}

/// …and the arrival itself is *one* turn each, not a rotation of re-asked,
/// re-waited ones. Walking down a street must not cost a prompt per person per
/// three seconds for as long as you stand in it.
#[test]
fn walking_up_to_someone_costs_one_turn_each() {
    let mut harness = Harness::with_news(IN_A_FIELD);
    assert!(harness.run(10.0).is_empty());

    harness.move_player(IN_THE_CROWD);
    let prompts = harness.run(120.0);
    assert_eq!(
        prompts.len(),
        3,
        "the three of them should think once between them, not on a clock: {prompts:?}"
    );
}

/// The gate is on *initiative*, never on *response*. An NPC who has been silent
/// for five minutes because nothing has happened is not asleep — speak to him
/// and he answers on the next poll, exactly as he always did.
///
/// This is the property that makes the whole feature safe: the reaction lane
/// never consults novelty, so the latency the player actually feels is unchanged.
#[test]
fn speaking_to_a_settled_crowd_is_answered_at_once() {
    let mut harness = Harness::with_news(IN_THE_CROWD);
    harness.run(30.0);
    assert!(harness.run(120.0).is_empty(), "the crowd never settled");

    // A word into a silent street. The inbox is news, so the cast wakes.
    harness.player_say("hoy", "What's your name?", IN_THE_CROWD);
    let answered = harness.run(20.0);
    assert!(
        !answered.is_empty(),
        "nobody answered the player: the reaction lane must never be gated on news"
    );
}

/// An NPC-to-NPC exchange runs on real percepts, at the existing turn rate, and
/// then *dies* — rather than ringing forever because each `wait` re-fires the
/// next question.
///
/// Ilse answers the player out loud; Sven and Conny hear her, which is real news
/// for both, so each is owed a turn. Neither has anything to add, and a silent
/// turn creates no percept for anybody — so the room goes quiet and stays quiet.
#[test]
fn a_conversation_runs_to_its_end_and_then_stops_costing_money() {
    let mut harness = Harness::with_news(IN_THE_CROWD);
    harness.run(30.0);
    assert!(harness.run(120.0).is_empty());

    harness.player_say("ask-name", "What's your name?", IN_THE_CROWD);
    let conversation = harness.run(60.0);
    assert!(
        conversation.contains(&ActorId::from_raw("k0fb1")),
        "Ilse never answered: {conversation:?}"
    );

    // The words reached the others, so they got their turn. The point is that it
    // ended.
    let after = harness.run(300.0);
    assert_eq!(
        after,
        Vec::<ActorId>::new(),
        "the conversation rang on with nothing left to say: {after:?}"
    );
}

/// The city does not freeze permanently behind you. Walk away for real, come
/// back, and they look up — even though the street they last thought about is
/// exactly the street they see now.
///
/// This is what [`NOVELTY_MEMORY_SECONDS`] buys, and it is why the memory lapses
/// on *absence* rather than on silence: the quiet neighbour above is never
/// forgotten, but the player who left is.
#[test]
fn walking_away_and_coming_back_is_news_again() {
    let mut harness = Harness::with_news(IN_THE_CROWD);
    harness.run(30.0);
    assert!(harness.run(60.0).is_empty(), "the crowd never settled");

    harness.move_player(IN_A_FIELD);
    assert_eq!(
        harness.run(NOVELTY_MEMORY_SECONDS * 2.0),
        Vec::<ActorId>::new(),
        "the cast kept thinking about a player who had left"
    );

    harness.move_player(IN_THE_CROWD);
    assert!(
        !harness.run(30.0).is_empty(),
        "he walked back into the street and nobody noticed him"
    );
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

/// …and novelty does not change that either. A bell is news for everyone who
/// hears it, but the *nudge* is a priority turn, and priority is ungated: the
/// far-off NPC who has never met the player still thinks when the world happens
/// to him.
#[test]
fn a_sound_still_reaches_an_npc_who_has_no_news_and_no_player() {
    let mut harness = Harness::with_news(IN_A_FIELD);
    assert!(harness.run(10.0).is_empty());

    harness.send_all(vec![EngineCommand::DebugSound {
        sound_id: "town_bell".into(),
        position_m: Vec3::new(0.0, 4.0, 113.0),
    }]);

    let prompts = harness.run(10.0);
    assert_eq!(
        prompts.len(),
        1,
        "the bell must reach exactly one witness, news gate or no news gate: {prompts:?}"
    );
}

// ---------------------------------------------------------------- curiosity
//
// `features/gate_idle_cognition_on_novelty.md` §2. Novelty made silence free but
// left every one of the ~500 people you walk past thinking about you the moment
// you appeared — one turn each rather than a rotation, which is affordable and
// still silly. These pin the third gate, and above all the two things it must
// never touch: the latency of an answer, and the reach of a sound.

/// The verdict is a fact about the news, not about the clock — and this is the
/// engine-level form of the trap.
///
/// The player stands in a crowd for five minutes. Every one of those ~18,000
/// polls, all three of them have news (they have never thought, and a stranger
/// is standing in front of them) and every one of those polls asks the same
/// question. A roll re-drawn per poll would pass within a frame or two and a
/// street of magistrates would greet the player like touts. Not one of them says
/// a word.
#[test]
fn an_aloof_crowd_never_speaks_first_however_long_you_stand_there() {
    let mut harness = Harness::with_curiosity(IN_THE_CROWD, 0.0);
    let prompts = harness.run(300.0);
    assert_eq!(
        prompts,
        Vec::<ActorId>::new(),
        "somebody's mind was changed for them by the passage of time: {prompts:?}"
    );
}

/// *"An aloof NPC never opens, but always answers."*
///
/// The same three people who would not look up above answer the moment they are
/// spoken to. Curiosity is on the changed-context branch of the novelty gate and
/// on nothing else: a non-empty inbox is somebody else's initiative arriving, and
/// the reaction and priority lanes never consulted the gate to begin with. This
/// is the property that makes reticence safe to ship — the latency the player
/// actually feels is unchanged, for everyone, at every station.
#[test]
fn an_aloof_crowd_answers_the_player_at_once() {
    let mut harness = Harness::with_curiosity(IN_THE_CROWD, 0.0);
    assert!(harness.run(30.0).is_empty(), "somebody greeted the player");

    harness.player_say("hoy", "What's your name?", IN_THE_CROWD);
    assert!(
        !harness.run(20.0).is_empty(),
        "nobody answered: curiosity leaked out of the idle lane and into the reaction lane"
    );
}

/// …and the world still happens to them. A bell is a percept, not a
/// conversational opening, and the nudge it buys is a priority turn — ungated by
/// proximity, by news, and now by character too.
#[test]
fn a_sound_still_nudges_an_aloof_npc() {
    let mut harness = Harness::with_curiosity(IN_A_FIELD, 0.0);
    assert!(harness.run(10.0).is_empty());

    harness.send_all(vec![EngineCommand::DebugSound {
        sound_id: "town_bell".into(),
        position_m: Vec3::new(0.0, 4.0, 113.0),
    }]);

    let prompts = harness.run(10.0);
    assert_eq!(
        prompts.len(),
        1,
        "the bell must reach exactly one witness, aloof or not: {prompts:?}"
    );
}

/// A fully curious city is the city before this feature, to the prompt: the same
/// arrival round, the same one turn each, and then the same silence. That is what
/// makes `config.ron: curiosity: false` (and `curiosity_scale: 1.0`) an honest
/// A/B rather than a different game.
#[test]
fn a_curious_city_is_the_city_before_curiosity() {
    let mut harness = Harness::with_curiosity(IN_A_FIELD, 1.0);
    assert!(harness.run(10.0).is_empty());

    harness.move_player(IN_THE_CROWD);
    let prompts = harness.run(120.0);
    assert_eq!(
        prompts.len(),
        3,
        "the arrival round changed shape: {prompts:?}"
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

/// The pair-keyed warm-exchange bookkeeping the NPC↔NPC conversation hold rides
/// on (`features/npcs_stop_walking_when_talking_to_each_other.md`): a targeted
/// line warms both members whoever spoke, the same 30 s silence rule lapses the
/// pair, and talking to yourself warms nothing.
#[test]
fn warm_exchanges_hold_both_parties_and_lapse_on_silence() {
    use cathedral_sim::{STAGE_PARTNER_MEMORY_SECONDS, WarmExchanges};
    use std::collections::BTreeSet;

    let mut exchanges = WarmExchanges::default();
    let a = ActorId::from_raw("aaaaa");
    let b = ActorId::from_raw("bbbbb");

    // Order-blind: who spoke and who listened is the same conversation.
    exchanges.note(&b, &a, 10.0);
    assert_eq!(
        exchanges.warm_actors(10.0 + STAGE_PARTNER_MEMORY_SECONDS - 1.0),
        BTreeSet::from([a.clone(), b.clone()])
    );

    // A later line refreshes the pair; silence after it lapses the hold.
    exchanges.note(&a, &b, 20.0);
    assert!(!exchanges.warm_actors(20.0 + STAGE_PARTNER_MEMORY_SECONDS - 1.0).is_empty());
    assert!(exchanges.warm_actors(20.0 + STAGE_PARTNER_MEMORY_SECONDS).is_empty());

    // Talking to yourself is not a conversation.
    exchanges.note(&a, &a, 100.0);
    assert!(exchanges.warm_actors(100.0).is_empty());
}
