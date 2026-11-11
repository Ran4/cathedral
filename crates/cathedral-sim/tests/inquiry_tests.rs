//! Asking about a matter under inquiry, through the whole engine
//! (`M4_accounts_inquiries_findings.md`).
//!
//! A question put in words reaches a holder as a turn: the offline mouth
//! answers from its sheet and `recount`s. The ask control records with no turn
//! at all. Both have to leave the same account, so these drive both against
//! [`Engine`] on the demo cast, with an inline case pack and the provider
//! scripted from the rendered prompt.
//!
//! What the player's record of the matter then says, from the first account
//! to the finding, is the `Case` message, and it is followed here too.

mod prompt_support;

use std::{cell::RefCell, fs, rc::Rc};

use cathedral_sim::fake::sheet_name;
use cathedral_sim::knowledge::{self, Account, FactView, Telling};
use cathedral_sim::{
    ActorId, Capabilities, CaseAccount, CaseFinding, CaseInquiry, Cognition, CognitionBusy,
    Completion, Engine, EngineCommand, EngineConfig, EngineMessage, FactId, NullSight,
    NullTranscription, NullTts, RequestId, SpatialActorUpdate, TtsBackendKind, Vec3, World,
    WorldSeed, fake_reply, inquiry, prompt::render_prompt,
};
use prompt_support::{actor, areas, catalog, demo_seed, fixtures_dir, prompt_env, seed_world};
use serde_json::json;

const SVEN: &str = "sv3n1";
const CONNY: &str = "cb947";
const ILSE: &str = "k0fb1";
const PLAYER: &str = "player";
const INQUIRY: &str = "test_ledger";

/// Within earshot of the whole cast.
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.91, 111.0);
/// 19 m from Sven and more than 20 m from Conny and Ilse, who stand beside
/// him: they hear what Sven says from here and nothing the player does.
const ONLY_SVEN_HEARS: Vec3 = Vec3::new(-8.0, 0.91, 132.0);
const OUT_OF_EARSHOT: Vec3 = Vec3::new(0.0, 0.91, 60.0);

const CLOCK_STEP_SECONDS: f64 = 0.5;
const MAX_POLLS: usize = 400;

/// The verb's row on a sheet, as `turn.j2` lists it.
const RECOUNT_ROW: &str = "recount {}  # After telling the stranger";
const PROMISE: &str = "I will fetch the reeve.";
const SILENT_RECOUNT: &str = "tell them what you know in the same turn - an account is what you \
                              said, and you have said nothing";

/// Sven saw the ledger left open today; Conny had it from him and has the day
/// wrong. Ilse reviews it and holds nothing.
fn ledger_pack() -> String {
    json!({
        "schema_version": 1,
        "facts": [{
            "id": "test.ledger",
            "topic": "talk",
            "said": "the ledger was left open on the counter overnight, {day}",
            "seeded": [SVEN],
            "day": 0,
            "decays": false
        }],
        "retold": [{"fact": "test.ledger", "by": CONNY, "from": SVEN, "day_offset": -1}],
        "inquiries": [{
            "id": INQUIRY,
            "title": "The open ledger",
            "reviewer": ILSE,
            "review_office": "waning",
            "facts": ["test.ledger"],
            "sources_needed": 1,
            "finding": "Its keeper left it open and nobody else touched it."
        }]
    })
    .to_string()
}

fn accounts(world: &World) -> &[Account] {
    world
        .knowledge
        .player_learned
        .get(&FactId::from_raw("test.ledger"))
        .map_or(&[], |receipt| receipt.accounts.as_slice())
}

fn account_of(world: &World, speaker: &str) -> Option<Account> {
    accounts(world)
        .iter()
        .find(|account| account.speaker.as_str() == speaker)
        .cloned()
}

/// Every prompt the engine submits, for the harness to answer on a later poll.
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

struct Harness {
    engine: Engine,
    probe: Probe,
    /// How many of the probe's requests have been answered.
    answered: usize,
    /// What the provider says to a prompt: the offline mouth, unless a test
    /// scripts something else.
    reply: fn(&str) -> String,
    now: f64,
    spatial_seq: i64,
    messages: Vec<EngineMessage>,
}

impl Harness {
    /// The demo cast with the ledger pack installed, the Ready poll done.
    fn new() -> Self {
        let probe = Probe::default();
        let engine = Engine::new(
            EngineConfig {
                fake_mode: true,
                turn_delay_seconds: 0.0,
                case_packs: vec![ledger_pack()],
                ..EngineConfig::default()
            },
            &WorldSeed::from_json_str(&demo_seed()).expect("the demo seed"),
            areas(),
            catalog(),
            prompt_env(),
            Box::new(probe.clone()),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
            (PLAYER_SPAWN, 0.0),
            0,
            0.0,
        )
        .expect("the ledger pack names only the demo cast");
        let mut harness = Self {
            engine,
            probe,
            answered: 0,
            reply: fake_reply,
            now: 0.0,
            spatial_seq: 0,
            messages: Vec::new(),
        };
        harness.pump(Vec::new());
        harness
    }

    /// One poll: every prompt submitted since the last one is answered first,
    /// as a provider would have by now, then `commands` are applied.
    fn pump(&mut self, commands: Vec<EngineCommand>) {
        let completions: Vec<EngineCommand> = self.probe.0.borrow()[self.answered..]
            .iter()
            .map(|(request_id, prompt)| {
                EngineCommand::LlmCompletion(Completion {
                    request_id: *request_id,
                    result: Ok((self.reply)(prompt)),
                    duration_seconds: 0.0,
                })
            })
            .collect();
        self.answered += completions.len();
        let commands = completions.into_iter().chain(commands).collect();
        self.messages.extend(self.engine.poll(self.now, commands));
    }

    /// Poll, advancing the clock, until `done`. On return `now` is still the
    /// time of the poll that made it so.
    fn pump_until(&mut self, what: &str, done: impl Fn(&Harness) -> bool) {
        for _ in 0..MAX_POLLS {
            if done(self) {
                return;
            }
            self.now += CLOCK_STEP_SECONDS;
            self.pump(Vec::new());
        }
        panic!("gave up waiting for {what} after {MAX_POLLS} polls");
    }

    /// The typed-chat box, with the player standing at `from`.
    fn say(&mut self, text: &str, from: Vec3) {
        self.spatial_seq += 1;
        self.pump(vec![EngineCommand::PlayerSay {
            request_id: format!("say-{}", self.spatial_seq),
            text: text.to_string(),
            position_m: from,
            spatial_seq: self.spatial_seq,
        }]);
    }

    /// The ask control.
    fn ask(&mut self, request_id: &str, target: &str, inquiry_id: &str) {
        self.pump(vec![EngineCommand::PlayerAsk {
            request_id: request_id.to_string(),
            target_id: actor(target),
            inquiry_id: inquiry_id.to_string(),
        }]);
    }

    /// The lodge control: the ledger accounts of `speakers`, to the reviewer.
    fn lodge(&mut self, request_id: &str, speakers: &[&str]) {
        self.pump(vec![EngineCommand::PlayerLodge {
            request_id: request_id.to_string(),
            inquiry_id: INQUIRY.to_string(),
            accounts: speakers
                .iter()
                .map(|speaker| (FactId::from_raw("test.ledger"), actor(speaker)))
                .collect(),
        }]);
    }

    /// The player's record of the ledger matter each time it was sent, oldest
    /// first. The pack has the one inquiry.
    fn cases(&self) -> Vec<&CaseInquiry> {
        self.messages
            .iter()
            .filter_map(|message| match message {
                EngineMessage::Case { inquiries } => Some(&inquiries[0]),
                _ => None,
            })
            .collect()
    }

    /// What `speaker` has said aloud, oldest first.
    fn said(&self, speaker: &str) -> Vec<&str> {
        self.messages
            .iter()
            .filter_map(|message| match message {
                EngineMessage::Speech {
                    speaker_id, text, ..
                } if speaker_id.as_str() == speaker => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn has_diagnostic(&self, line: &str) -> bool {
        self.messages
            .iter()
            .any(|message| matches!(message, EngineMessage::Diagnostic(said) if said == line))
    }

    /// `(success, error code, message)` of one command's result.
    fn command_result(&self, request_id: &str) -> (bool, Option<&str>, &str) {
        self.messages
            .iter()
            .find_map(|message| match message {
                EngineMessage::CommandResult {
                    request_id: id,
                    success,
                    error_code,
                    message,
                } if id == request_id => Some((*success, error_code.as_deref(), message.as_str())),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no result for command '{request_id}'"))
    }

    /// Every prompt rendered so far for the person the sheet names.
    fn prompts_of(&self, name: &str) -> Vec<String> {
        self.probe
            .0
            .borrow()
            .iter()
            .filter(|(_, prompt)| sheet_name(prompt) == Some(name))
            .map(|(_, prompt)| prompt.clone())
            .collect()
    }
}

/// D04's parity. A question typed at a holder makes them say what they hold
/// and put it on record, and the ask control used at the same instant of a
/// twin world records the very same account with nobody speaking: one for a
/// witness, one for somebody who only had it from him.
#[test]
fn a_typed_question_and_the_ask_control_record_the_same_account() {
    let overnight = "the ledger was left open on the counter overnight";
    for (speaker, name, word) in [
        (SVEN, "Sven", format!("{overnight}, today")),
        (CONNY, "Conny", format!("{overnight}, yesterday")),
    ] {
        let mut typed = Harness::new();
        typed.say(&format!("{name}, what of the ledger?"), PLAYER_SPAWN);
        typed.pump_until("the spoken account", |harness| {
            account_of(harness.engine.world(), speaker).is_some()
        });
        let answered_at = typed.now;
        let spoken = account_of(typed.engine.world(), speaker).expect("just waited for it");
        assert_eq!(
            (
                spoken.origin.as_ref().map(ActorId::as_str),
                spoken.word.as_str()
            ),
            (Some(SVEN), word.as_str())
        );
        typed.pump_until("the answer aloud", |harness| {
            !harness.said(speaker).is_empty()
        });
        assert!(typed.said(speaker)[0].contains(overnight));

        let mut twin = Harness::new();
        twin.now = answered_at;
        twin.ask("ask", speaker, INQUIRY);
        assert_eq!(account_of(twin.engine.world(), speaker), Some(spoken));
        assert_eq!(
            twin.command_result("ask"),
            (true, None, format!("{name} tells you: \"{word}\"").as_str())
        );
        assert!(
            twin.said(speaker).is_empty(),
            "the control forces no speech"
        );
    }
}

/// A model that takes the offered verb without saying anything is refused in
/// its own terms, reads the refusal on its next sheet, and no account exists.
#[test]
fn a_recount_with_nothing_said_is_refused_and_records_nothing() {
    let mut harness = Harness::new();
    harness.reply = |prompt| {
        if prompt.contains(RECOUNT_ROW) {
            "recount {}".to_string()
        } else {
            fake_reply(prompt)
        }
    };
    harness.say("Sven, what of the ledger?", PLAYER_SPAWN);

    let diagnostic = format!("[smart actors] Sven: recount failed: {SILENT_RECOUNT}");
    harness.pump_until("the refusal", |harness| harness.has_diagnostic(&diagnostic));
    assert!(accounts(harness.engine.world()).is_empty());

    let told = format!("system: your action \"recount {{}}\" failed: {SILENT_RECOUNT}");
    harness.pump_until("Sven reading the refusal", |harness| {
        harness
            .prompts_of("Sven")
            .iter()
            .any(|prompt| prompt.contains(&told))
    });
}

/// The row is on a sheet only where the verb could work: a holder, asked about
/// the matter, with the player near enough to be told.
#[test]
fn the_recount_row_is_offered_only_to_a_holder_asked_within_earshot() {
    let env = prompt_env();
    let mut world = seed_world();
    inquiry::install(&mut world, &ledger_pack()).expect("the pack installs");
    let asked = ["Player said to you: \"What of the ledger?\"".to_string()];
    let offered = |world: &World, id: &str, since: &[String]| {
        render_prompt(world, &actor(id), Some(since), &env)
            .expect("the sheet renders")
            .contains(RECOUNT_ROW)
    };

    assert!(offered(&world, SVEN, &asked), "a witness who was asked");
    assert!(offered(&world, CONNY, &asked), "and a reteller who was");
    assert!(!offered(&world, ILSE, &asked), "not somebody with nothing");
    assert!(!offered(&world, SVEN, &[]), "not when nobody asked");

    world
        .characters
        .get_mut(&actor(PLAYER))
        .expect("the demo seed has a player")
        .state
        .position_m = OUT_OF_EARSHOT;
    assert!(!offered(&world, SVEN, &asked), "not with nobody to tell");
}

/// A world with no inquiry renders the bytes it always did. `golden_prompts`
/// holds every scenario to its blessed fixture, so the fixtures are the
/// witness that the row reached none of them.
#[test]
fn no_golden_prompt_lists_recount() {
    let mut read = 0;
    for entry in fs::read_dir(fixtures_dir()).expect("the fixtures directory") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_some_and(|extension| extension == "txt") {
            let prompt = fs::read_to_string(&path).expect("a fixture reads");
            assert!(!prompt.contains("recount {"), "{}", path.display());
            read += 1;
        }
    }
    assert!(read > 0, "the fixtures were found");
}

/// Each refusal of the ask control comes back as its own sentence with its
/// code, and nothing reaches the player's notes.
#[test]
fn a_refused_ask_reports_its_reason_and_records_nothing() {
    let mut harness = Harness::new();
    harness.ask("no-matter", SVEN, "test_wheel");
    harness.ask("holds-nothing", ILSE, INQUIRY);
    harness.pump(vec![EngineCommand::SpatialUpdate {
        spatial_seq: 1,
        updates: vec![SpatialActorUpdate::new(actor(PLAYER), OUT_OF_EARSHOT, None)],
    }]);
    harness.ask("too-far", SVEN, INQUIRY);

    assert_eq!(
        harness.command_result("no-matter"),
        (
            false,
            Some("invalid_action"),
            "nobody is looking into any such matter"
        )
    );
    assert_eq!(
        harness.command_result("holds-nothing"),
        (
            false,
            Some("invalid_action"),
            "Ilse has nothing to tell you about that"
        )
    );
    assert_eq!(
        harness.command_result("too-far"),
        (false, Some("out_of_range"), "Sven is too far to ask")
    );
    assert!(harness.engine.world().knowledge.player_learned.is_empty());
}

/// Acceptance 5's line. The question names the ledger, so the sheet Sven
/// answers lists `recount` and an ordinary question would have been answered
/// and recorded; asked to fetch the reeve he only promises. Conny and Ilse
/// hear the promise and not the question, and neither promises in turn.
#[test]
fn asked_to_fetch_the_reeve_the_addressee_promises_once_and_recounts_nothing() {
    let mut harness = Harness::new();
    harness.say(
        "Sven, the ledger was left open. Will you fetch the reeve?",
        ONLY_SVEN_HEARS,
    );
    harness.pump_until("Sven's promise", |harness| !harness.said(SVEN).is_empty());

    let asked = harness
        .prompts_of("Sven")
        .into_iter()
        .find(|prompt| prompt.contains("Will you fetch the reeve?"))
        .expect("Sven was prompted with the question");
    assert!(asked.contains(RECOUNT_ROW));
    assert_eq!(
        fake_reply(&asked),
        format!(r#"say {{"target": "player", "text": "{PROMISE}"}}"#)
    );

    harness.pump_until("both bystanders reading the promise", |harness| {
        ["Conny", "Ilse"].iter().all(|name| {
            harness
                .prompts_of(name)
                .iter()
                .any(|prompt| prompt.contains(PROMISE))
        })
    });
    // A few rounds more, so anything they were going to say has been said.
    for _ in 0..12 {
        harness.now += CLOCK_STEP_SECONDS;
        harness.pump(Vec::new());
    }
    assert_eq!(harness.said(SVEN), [PROMISE]);
    assert!(harness.said(CONNY).is_empty() && harness.said(ILSE).is_empty());
    assert!(accounts(harness.engine.world()).is_empty());
}

/// Seventy tellings that have nothing to do with the ledger, each of them a
/// receipt in the player's notes: more than the notes hold.
fn hear_seventy_other_things(world: &mut World) {
    let ids: Vec<String> = (0..70).map(|n| format!("test.other{n:02}")).collect();
    let facts: Vec<_> = ids
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "topic": "talk",
                "said": "the well rope wants splicing",
                "seeded": [ILSE]
            })
        })
        .collect();
    inquiry::install(
        world,
        &json!({"schema_version": 1, "facts": facts}).to_string(),
    )
    .expect("a plain fact pack installs");
    let days = world.current_time.map(|time| time.game_days());
    for id in ids {
        let key = world
            .knowledge
            .key_of(&FactId::from_raw(id))
            .expect("just installed");
        let telling = Telling {
            hops: 1,
            from: Some(actor(ILSE)),
            heat: 1.0,
            view: FactView::default(),
        };
        knowledge::learn(world, &actor(PLAYER), key, telling, days);
    }
}

/// The player's record of a matter, from nothing asked to the finding. Each
/// account is there with who gave it, when and where, and whose eyes it rests
/// on. The record is sent when it changes and not otherwise, and a full
/// turnover of the player's ordinary notes changes nothing in it. The review
/// is held with nobody doing anything, and what it found arrives on the same
/// record.
#[test]
fn the_case_message_follows_a_matter_from_its_accounts_to_its_finding() {
    let mut harness = Harness::new();
    assert_eq!(
        harness.cases(),
        [&CaseInquiry {
            id: INQUIRY.to_string(),
            title: "The open ledger".to_string(),
            reviewer: "Ilse".to_string(),
            review_at: None,
            finding: None,
            accounts: Vec::new(),
        }]
    );

    // Sven first and Conny a moment later, so the order is the order of
    // telling and not of their ids.
    harness.ask("sven", SVEN, INQUIRY);
    harness.now += CLOCK_STEP_SECONDS;
    harness.ask("conny", CONNY, INQUIRY);
    let here = harness
        .engine
        .world()
        .area_map
        .containing_area(PLAYER_SPAWN)
        .map(|area| area.label.clone());
    assert!(here.is_some(), "the player stands in a named place");
    let overnight = "the ledger was left open on the counter overnight";
    let account =
        |speaker: &str, name: &str, day: &str, register: &str, lodged: bool| CaseAccount {
            fact: FactId::from_raw("test.ledger"),
            speaker: actor(speaker),
            speaker_name: name.to_string(),
            word: format!("{overnight}, {day}"),
            when: "Dayspring, day 0".to_string(),
            place: here.clone(),
            register: register.to_string(),
            lodged,
        };
    let rows = |lodged: bool| {
        vec![
            account(SVEN, "Sven", "today", "saw it themselves", lodged),
            account(CONNY, "Conny", "yesterday", "had it from Sven", lodged),
        ]
    };
    assert_eq!(
        harness.cases().len(),
        3,
        "at the start, then once an account"
    );
    assert_eq!(harness.cases()[2].accounts, rows(false));

    hear_seventy_other_things(harness.engine.world_mut());
    for _ in 0..6 {
        harness.now += CLOCK_STEP_SECONDS;
        harness.pump(Vec::new());
    }
    let notes = &harness.engine.world().knowledge.player_learned;
    assert_eq!(notes.len(), knowledge::PLAYER_RECEIPTS_MAX);
    assert_eq!(harness.cases().len(), 3, "nothing in the record changed");

    harness.lodge("lodge", &[SVEN, CONNY]);
    assert_eq!(
        harness.command_result("lodge"),
        (
            true,
            None,
            "Ilse takes 2 accounts; the review is at the Waning"
        )
    );
    assert_eq!(harness.cases().len(), 4);
    let awaiting = harness.cases()[3];
    assert_eq!(awaiting.review_at.as_deref(), Some("the Waning, day 0"));
    assert_eq!(awaiting.finding, None);
    assert_eq!(awaiting.accounts, rows(true), "both outlived the turnover");

    // Sixty times faster through the afternoon. Nobody does anything.
    harness.pump(vec![
        EngineCommand::CycleTimeScale,
        EngineCommand::CycleTimeScale,
    ]);
    harness.pump_until("the review", |harness| {
        harness
            .cases()
            .last()
            .is_some_and(|case| case.finding.is_some())
    });
    for _ in 0..20 {
        harness.now += CLOCK_STEP_SECONDS;
        harness.pump(Vec::new());
    }
    let found = "Its keeper left it open and nobody else touched it.";
    assert_eq!(harness.cases().len(), 5, "the finding, sent once");
    let reviewed = harness.cases()[4];
    assert_eq!(reviewed.review_at, None);
    assert_eq!(
        reviewed.finding,
        Some(CaseFinding {
            full: true,
            text: found.to_string(),
            held: "the Waning, day 0".to_string(),
        })
    );
    assert_eq!(reviewed.accounts, rows(true));
    assert!(harness.has_diagnostic(&format!(
        "[inquiry] review of '{INQUIRY}' held at the Waning, day 0: {found}"
    )));
}

/// A refused lodging comes back through the command's own result, with its
/// code and its sentence, and the record does not change.
#[test]
fn a_refused_lodging_reports_its_reason_and_the_record_does_not_change() {
    let mut harness = Harness::new();
    harness.ask("sven", SVEN, INQUIRY);
    let sent = harness.cases().len();

    harness.lodge("never-asked", &[CONNY]);
    harness.pump(vec![EngineCommand::SpatialUpdate {
        spatial_seq: 1,
        updates: vec![SpatialActorUpdate::new(actor(PLAYER), OUT_OF_EARSHOT, None)],
    }]);
    harness.lodge("too-far", &[SVEN]);

    assert_eq!(
        harness.command_result("never-asked"),
        (
            false,
            Some("invalid_arguments"),
            "there is no such account in your notes"
        )
    );
    assert_eq!(
        harness.command_result("too-far"),
        (false, Some("out_of_range"), "Ilse is not here to take it")
    );
    assert_eq!(harness.cases().len(), sent);
    assert!(harness.engine.world().inquiries[0].lodged.is_empty());
}
