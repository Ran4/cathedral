//! Deterministic routing and percept lifecycle through the complete Engine.
//! Replies are scripted; live social judgment is evaluated separately.
mod prompt_support;

use cathedral_sim::{traits::Completion, *};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

const PLAYER: &str = "player";
const PARTNER: &str = "sv3n1";
const NEARER: &str = "cb947";
const STRANGER: &str = "k0fb1";
const AT: Vec3 = Vec3::new(0.0, 0.91, 111.0);
fn id(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

#[derive(Clone, Default)]
struct Probe(Rc<RefCell<Vec<(RequestId, String)>>>);
impl Cognition for Probe {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        let mut requests = self.0.borrow_mut();
        let request = RequestId(requests.len() as u64 + 1);
        requests.push((request, prompt));
        Ok(request)
    }
}

#[derive(Clone, Default)]
struct Stt(Rc<RefCell<Vec<TranscriptionJobId>>>);
impl Transcription for Stt {
    fn realtime_begin(&mut self, _: &str) -> bool {
        false
    }
    fn realtime_append(&mut self, _: &str, _: &[i16]) -> bool {
        false
    }
    fn realtime_commit(&mut self, _: &str) -> bool {
        false
    }
    fn realtime_clear(&mut self, _: &str) {}
    fn available(&self, _: SttBackendKind) -> bool {
        true
    }
    fn submit_batch(
        &mut self,
        job: TranscriptionJobId,
        _: PathBuf,
        _: SttBackendKind,
    ) -> Result<(), SttSubmitError> {
        self.0.borrow_mut().push(job);
        Ok(())
    }
}

struct Harness {
    engine: Engine,
    probe: Probe,
    stt: Stt,
    now: f64,
    seq: i64,
}
impl Harness {
    fn new() -> Self {
        let probe = Probe::default();
        let stt = Stt::default();
        let engine = Engine::new(
            EngineConfig {
                fake_mode: true,
                turn_delay_seconds: 0.0,
                idle_mode: IdleCognitionMode::Stage,
                stage: StageConfig {
                    radius_m: 0.0,
                    max_actors: 0,
                },
                idle_requires_news: true,
                idle_curiosity: CuriosityConfig {
                    enabled: true,
                    scale: 0.0,
                },
                tts_selected: TtsBackendKind::Off,
                ..EngineConfig::default()
            },
            &WorldSeed::from_json_str(&prompt_support::demo_seed()).unwrap(),
            prompt_support::areas(),
            prompt_support::catalog(),
            prompt_support::prompt_env(),
            Box::new(probe.clone()),
            Box::new(stt.clone()),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(true, true, true, false, false, TtsBackendKind::Off),
            (AT, 0.0),
            0,
            0.0,
        )
        .unwrap();
        Self {
            engine,
            probe,
            stt,
            now: 0.0,
            seq: 0,
        }
    }
    fn poll(&mut self, commands: Vec<EngineCommand>) -> Vec<EngineMessage> {
        self.engine.poll(self.now, commands)
    }
    fn say(&mut self, text: &str) -> Vec<EngineMessage> {
        self.seq += 1;
        self.poll(vec![EngineCommand::PlayerSay {
            request_id: format!("typed-{}", self.seq),
            text: text.into(),
            position_m: AT,
            spatial_seq: self.seq,
        }])
    }
    fn complete(&mut self, result: Result<&str, CognitionError>) -> Vec<EngineMessage> {
        assert!(self.engine.scheduler().in_flight_actor_id().is_some());
        let request_id = self.probe.0.borrow().last().unwrap().0;
        self.now += 0.1;
        self.poll(vec![EngineCommand::LlmCompletion(Completion {
            request_id,
            result: result.map(str::to_owned),
            duration_seconds: 0.1,
        })])
    }
    fn settle(&mut self) {
        for _ in 0..10 {
            self.now += 0.1;
            self.poll(vec![]);
            if self.engine.scheduler().in_flight_actor_id().is_none() {
                return;
            }
            self.complete(Ok("wait {}"));
        }
        panic!("unexpected endless idle work");
    }
    fn engage(&mut self) {
        self.say("Sven, how will you trade?");
        self.assert_thinking(PARTNER);
        self.complete(Ok(
            "say {\"target\":\"player\",\"text\":\"I will work the forge.\"}",
        ));
        self.now += 4.0;
        self.settle();
        assert_eq!(
            self.engine.conversation_partner(self.now),
            Some(&id(PARTNER))
        );
    }
    fn focus(&mut self, actor: Option<&str>, duration: f64) {
        self.poll(vec![EngineCommand::PlayerAttention {
            actor_id: actor.map(id),
        }]);
        self.now += duration;
        self.poll(vec![EngineCommand::PlayerAttention {
            actor_id: actor.map(id),
        }]);
    }
    fn assert_thinking(&self, actor: &str) {
        assert_eq!(
            self.engine.scheduler().in_flight_actor_id(),
            Some(&id(actor))
        );
        assert!(self.engine.scheduler().in_flight_is_player_reaction());
    }
    fn interject(&mut self, actor: &str) {
        apply_action(
            self.engine.world_mut(),
            &id(actor),
            "say",
            &serde_json::json!({"target": PLAYER, "text": "I can help with that."}),
        )
        .unwrap();
        self.poll(vec![]);
        self.now += 4.0;
        self.settle();
    }
    fn start_voice(&mut self, key: &str) {
        self.poll(vec![EngineCommand::PlayerUtteranceStarted {
            wav_basename: key.into(),
        }]);
    }
    fn recording(&mut self, key: &str) {
        self.seq += 1;
        self.poll(vec![EngineCommand::PlayerRecording {
            request_id: key.into(),
            wav_basename: key.into(),
            stt_backend: SttBackendKind::Local,
            position_m: AT,
            spatial_seq: self.seq,
        }]);
    }
    fn transcribe(&mut self, index: usize, text: &str) -> Vec<EngineMessage> {
        let job = self.stt.0.borrow()[index];
        self.poll(vec![EngineCommand::Transcription(
            TranscriptionOutcome::Done {
                job,
                result: Ok(text.into()),
            },
        )])
    }
}

fn diagnostic(messages: &[EngineMessage]) -> &str {
    messages
        .iter()
        .find_map(|m| match m {
            EngineMessage::Diagnostic(line) if line.starts_with("[conversation]") => {
                Some(line.as_str())
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn nearer_bystander_and_unsolicited_interjection_do_not_acquire_a_reciprocal_exchange() {
    let mut h = Harness::new();
    h.engage();
    h.interject(NEARER);
    assert_eq!(h.engine.conversation_partner(h.now), Some(&id(PARTNER)));
    let messages = h.say("How are you going to trade?");
    h.assert_thinking(PARTNER);
    assert!(diagnostic(&messages).contains("evidence=ongoing exchange"));
    let hearers = messages
        .iter()
        .find_map(|m| match m {
            EngineMessage::Speech {
                speaker_id,
                target_id,
                recipient_ids,
                ..
            } if speaker_id == &id(PLAYER) => {
                assert!(
                    target_id.is_none(),
                    "inference never changes broadcast hearing"
                );
                Some(recipient_ids)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(hearers.len(), 3);
    let bystander = &h.engine.world().characters[&id(NEARER)];
    assert!(
        bystander
            .inbox()
            .last()
            .unwrap()
            .contains("you are overhearing")
    );
    assert!(bystander.inbox().last().unwrap().contains("Sven"));
}

#[test]
fn fleeting_focus_and_looking_away_preserve_continuity_but_sustained_focus_switches() {
    let mut h = Harness::new();
    h.engage();
    h.focus(Some(NEARER), 0.2);
    h.focus(None, 0.1);
    h.say("And then?");
    h.assert_thinking(PARTNER);
    h.settle();
    h.focus(Some(NEARER), 0.7);
    h.say("What about your stall?");
    h.assert_thinking(NEARER);
    h.settle();
    h.focus(Some(PARTNER), 0.7);
    h.say("Back to your plan.");
    h.assert_thinking(PARTNER);
}

#[test]
fn named_acceptance_of_an_interjection_establishes_its_followup_partner() {
    let mut h = Harness::new();
    h.engage();
    h.interject(NEARER);
    h.say("Conny, tell me your idea.");
    h.assert_thinking(NEARER);
    h.complete(Ok(
        "say {\"target\":\"player\",\"text\":\"I have a stall by the water.\"}",
    ));
    h.now += 4.0;
    h.settle();
    h.say("How do you get there?");
    h.assert_thinking(NEARER);
}

#[test]
fn mentioned_and_quoted_names_never_redirect_but_a_vocative_overrides_focus() {
    let mut h = Harness::new();
    h.engage();
    for text in [
        "What did Conny tell you?",
        "Conny said you could trade here.",
        "Conny, the fisherman, said you could trade here.",
        "Conny, who sells fish, already told me.",
        "I heard 'Conny, come here' yesterday.",
        "\"Conny, can you help?\" was what I said.",
    ] {
        h.say(text);
        h.assert_thinking(PARTNER);
        h.settle();
    }
    h.focus(Some(PARTNER), 0.7);
    h.say("Conny can you help me?");
    h.assert_thinking(NEARER);
}

#[test]
fn group_reactions_survive_the_nearest_actors_wait_and_keep_the_previous_partner() {
    let mut h = Harness::new();
    h.engage();
    let messages = h.say("Does anyone know where to buy a lamp?");
    assert!(diagnostic(&messages).contains("evidence=open group language"));
    h.assert_thinking(NEARER);
    h.complete(Ok("wait {}"));
    assert!(!h.engine.scheduler().in_flight_is_player_reaction());
    assert_ne!(h.engine.scheduler().in_flight_actor_id(), Some(&id(NEARER)));
    h.settle();
    assert_eq!(h.engine.conversation_partner(h.now), Some(&id(PARTNER)));
    h.say("How will you pay for it?");
    h.assert_thinking(PARTNER);
}

#[test]
fn expired_absent_removed_or_distant_partners_cannot_hold_a_reaction() {
    for unavailable in ["expired", "absent", "removed", "distant"] {
        let mut h = Harness::new();
        h.engage();
        match unavailable {
            "expired" => h.now += 31.0,
            "absent" => {
                h.engine
                    .world_mut()
                    .characters
                    .get_mut(&id(PARTNER))
                    .unwrap()
                    .state
                    .presence = Presence::BeyondTheWalls
            }
            "removed" => {
                h.engine.world_mut().characters.remove(&id(PARTNER));
            }
            _ => {
                h.engine
                    .world_mut()
                    .characters
                    .get_mut(&id(PARTNER))
                    .unwrap()
                    .state
                    .position_m = AT + Vec3::X * 21.0
            }
        }
        h.say("Can you help?");
        h.assert_thinking(NEARER);
    }
    let mut h = Harness::new();
    for actor in [PARTNER, NEARER, STRANGER] {
        h.engine
            .world_mut()
            .characters
            .get_mut(&id(actor))
            .unwrap()
            .state
            .presence = Presence::BeyondTheWalls;
    }
    let messages = h.say("Hello?");
    assert!(diagnostic(&messages).contains("no plausible listener"));
    assert!(h.engine.scheduler().in_flight_actor_id().is_none());
}

#[test]
fn voice_captures_attention_at_onset_and_uses_the_typed_selection_policy() {
    let mut h = Harness::new();
    h.engage();
    h.focus(Some(NEARER), 0.7);
    h.start_voice("before-turn.wav");
    assert!(
        h.engine.speech_router().player_composing(),
        "local recording onset gates idle work too"
    );
    h.focus(Some(PARTNER), 0.7);
    h.recording("before-turn.wav");
    h.now += 8.0;
    h.transcribe(0, "How do you trade?");
    h.assert_thinking(NEARER);
    assert!(!h.engine.speech_router().player_composing());
    let voice_prompt = h.probe.0.borrow().last().unwrap().1.clone();
    assert!(voice_prompt.contains("probable addressee: you; evidence: sustained attention"));
}

#[test]
fn delayed_capture_revalidates_hearing_and_does_not_overwrite_a_newer_player_choice() {
    let mut h = Harness::new();
    h.engage();
    h.start_voice("old.wav");
    h.recording("old.wav");
    h.now += 1.0;
    h.say("Conny, what do you sell?");
    h.assert_thinking(NEARER);
    h.transcribe(0, "What about your forge?");
    h.complete(Ok("wait {}"));
    h.assert_thinking(PARTNER);
    assert_eq!(h.engine.conversation_partner(h.now), Some(&id(NEARER)));
    h.settle();
    h.focus(Some(PARTNER), 0.7);
    h.start_voice("departed.wav");
    h.recording("departed.wav");
    h.engine
        .world_mut()
        .characters
        .get_mut(&id(PARTNER))
        .unwrap()
        .state
        .presence = Presence::BeyondTheWalls;
    h.transcribe(1, "Can you hear me?");
    h.assert_thinking(NEARER);
}

#[test]
fn context_stays_with_its_utterance_across_switches_and_failure_retry() {
    let mut h = Harness::new();
    h.engage();
    h.say("What happens next?");
    h.assert_thinking(PARTNER);
    let first = h.probe.0.borrow().last().unwrap().1.clone();
    h.complete(Err(CognitionError::new("offline")));
    h.now += 2.0;
    h.poll(vec![]);
    let retried = h.probe.0.borrow().last().unwrap().1.clone();
    assert!(first.contains("probable addressee: you; evidence: ongoing exchange"));
    assert!(retried.contains("probable addressee: you; evidence: ongoing exchange"));
    h.settle();
    h.say("Conny, now tell me your plan.");
    h.settle();
    let old = h.engine.world().characters[&id(STRANGER)]
        .inbox()
        .iter()
        .find(|line| line.contains("What happens next?"))
        .unwrap();
    assert!(old.contains("a stranger (id sv3n1)"));
    assert!(!old.contains("Conny"));
    assert!(
        !h.engine.world().characters[&id(STRANGER)]
            .knows()
            .contains(&id(PARTNER))
    );
}

#[test]
fn newcomers_receive_only_current_speech_and_no_hidden_names_or_old_dialogue() {
    let mut h = Harness::new();
    h.engine
        .world_mut()
        .characters
        .get_mut(&id(STRANGER))
        .unwrap()
        .state
        .position_m = AT + Vec3::X * 100.0;
    h.engage();
    h.engine
        .world_mut()
        .characters
        .get_mut(&id(STRANGER))
        .unwrap()
        .state
        .position_m = AT + Vec3::X * 2.0;
    h.say("How will you do that?");
    let inbox = h.engine.world().characters[&id(STRANGER)]
        .inbox()
        .join("\n");
    assert!(inbox.contains("apparent attention"));
    assert!(!inbox.contains("Sven"));
    assert!(!inbox.contains("forge"));
    assert!(!inbox.contains("ongoing exchange"));
}

#[test]
fn aborted_and_ownerless_local_capture_expire_without_stalling_idle_cognition() {
    let mut h = Harness::new();
    h.start_voice("cancel.wav");
    h.poll(vec![EngineCommand::PlayerAudioAbort {
        wav_basename: "cancel.wav".into(),
    }]);
    assert!(!h.engine.speech_router().player_composing());
    h.start_voice("lost.wav");
    h.now += conversation::CAPTURE_LIFETIME_SECONDS + 0.1;
    h.poll(vec![]);
    assert!(!h.engine.speech_router().player_composing());
}

#[test]
fn npc_reply_during_capture_does_not_cancel_a_players_deliberate_switch() {
    let mut h = Harness::new();
    h.engage();
    h.focus(Some(NEARER), 0.7);
    h.start_voice("switch.wav");
    h.recording("switch.wav");
    h.now += 1.0;
    apply_action(
        h.engine.world_mut(),
        &id(PARTNER),
        "say",
        &serde_json::json!({"target": PLAYER, "text": "And I have an anvil."}),
    )
    .unwrap();
    h.poll(vec![]);
    h.transcribe(0, "Tell me about your stall.");
    h.assert_thinking(NEARER);
    assert_eq!(h.engine.conversation_partner(h.now), Some(&id(NEARER)));
}

#[test]
fn later_group_speech_prevents_old_audio_from_changing_the_partner_even_in_the_same_poll_time() {
    let mut h = Harness::new();
    h.engage();
    h.focus(Some(NEARER), 0.7);
    h.start_voice("old-switch.wav");
    h.recording("old-switch.wav");
    h.say("Does anyone have a lamp?");
    h.transcribe(0, "What do you sell?");
    assert_eq!(h.engine.conversation_partner(h.now), Some(&id(PARTNER)));
}

#[test]
fn a_fresh_followup_preempts_the_rest_of_a_group_invitation() {
    let mut h = Harness::new();
    h.engage();
    h.say("Does anyone know the way?");
    h.assert_thinking(NEARER);
    h.say("Ilse, can you show me?");
    h.complete(Ok("wait {}"));
    h.assert_thinking(STRANGER);
}

#[test]
fn an_unseen_target_is_not_identified_to_an_overhearer() {
    let mut h = Harness::new();
    h.engine
        .world_mut()
        .characters
        .get_mut(&id(PARTNER))
        .unwrap()
        .state
        .position_m = AT - Vec3::X * 15.0;
    h.engine
        .world_mut()
        .characters
        .get_mut(&id(NEARER))
        .unwrap()
        .state
        .position_m = AT + Vec3::X * 15.0;
    h.say("Sven, how will you trade?");
    let line = h.engine.world().characters[&id(NEARER)]
        .inbox()
        .last()
        .unwrap();
    // The spoken vocative is public, but the suffix must not tie that name to
    // an unseen body or reveal its id. Strip the actual words before checking.
    let suffix = line.split(" [Conversation:").nth(1).unwrap();
    assert!(suffix.contains("someone else nearby"));
    assert!(!suffix.contains("Sven"));
    assert!(!suffix.contains(PARTNER));
}
