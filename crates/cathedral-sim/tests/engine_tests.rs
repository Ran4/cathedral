//! The engine pump (`test_protocol_server.py` ENGINE rows 5, 9, 10, 13-15, 20
//! and `test_sounds.py::ServerSoundTests` 45-49).
//!
//! Python drove these through a whole stdio server with a monkeypatched clock;
//! here the clock is a field and the backends are probes, so every assertion is
//! about the engine itself. The rows this file does *not* carry:
//!
//! * 58-68 (floor arbitration) live in `floor_tests.rs` — the floor is pure.
//! * 69-70 (a transcribed say prioritizes the nearest LLM hearer) belong to
//!   `_resolve_transcription`, which is the speech router's, and land with P6.
//!   The *same* nudge through a sound is here (48).
//! * 71 (the scripted conversation) is `e2e_fake.rs`.

mod prompt_support;

use std::{cell::RefCell, rc::Rc, sync::Arc};

use cathedral_sim::{
    ActorId, Capabilities, Cognition, CognitionBusy, Completion, Engine, EngineCommand,
    EngineConfig, EngineInitError, EngineMessage, FakeCognition, HEARING_RADIUS_M, ItemId, Movement,
    NavData, NullSight, NullTranscription, Office, Patrol, PublicSnapshot, SpeechError,
    SpeechEventId, SpeechRouter, SttBackendKind, Subsystem, Tts, TtsBackendKind, TtsOutcome,
    TtsRequest, TtsSubmitError, Vec3, WALK_SPEED_MPS, WALK_Y, WorldClock, WorldSeed, apply_action,
    ids::RequestId, speech_reading_seconds,
};
use prompt_support::{areas, catalog, prompt_env};
use serde_json::json;

// ---------------------------------------------------------------- test doubles

/// [`FakeCognition`] behind a handle, because the engine owns its `Box<dyn
/// Cognition>` and the test still has to drain the staged completions and hand
/// them back as `EngineCommand::LlmCompletion` — exactly what the host does.
#[derive(Clone, Default)]
struct SharedCognition(Rc<RefCell<FakeCognition>>);

impl SharedCognition {
    fn drain(&self) -> Vec<Completion> {
        self.0.borrow_mut().drain_completions()
    }
}

impl Cognition for SharedCognition {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request(prompt)
    }
}

#[derive(Default)]
struct TtsState {
    available: bool,
    refuse: Option<TtsSubmitError>,
    submitted: Vec<TtsRequest>,
    warmed: Vec<TtsBackendKind>,
}

/// A voice backend that never makes a sound, but remembers everything it was
/// asked to.
#[derive(Clone, Default)]
struct TtsProbe(Rc<RefCell<TtsState>>);

impl TtsProbe {
    fn available() -> Self {
        let probe = Self::default();
        probe.0.borrow_mut().available = true;
        probe
    }

    fn refusing(error: TtsSubmitError) -> Self {
        let probe = Self::available();
        probe.0.borrow_mut().refuse = Some(error);
        probe
    }

    fn submitted(&self) -> Vec<TtsRequest> {
        self.0.borrow().submitted.clone()
    }

    fn warmed(&self) -> Vec<TtsBackendKind> {
        self.0.borrow().warmed.clone()
    }
}

impl Tts for TtsProbe {
    fn available(&self, _kind: TtsBackendKind) -> bool {
        self.0.borrow().available
    }

    fn submit(&mut self, request: TtsRequest) -> Result<(), TtsSubmitError> {
        let mut state = self.0.borrow_mut();
        if let Some(error) = state.refuse {
            return Err(error);
        }
        state.submitted.push(request);
        Ok(())
    }

    fn warm(&mut self, kind: TtsBackendKind) {
        self.0.borrow_mut().warmed.push(kind);
    }
}

// -------------------------------------------------------------------- harness

/// The player's spawn in the Python tests' `hello()` — inside fart range of all
/// three NPCs.
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.91, 111.0);

struct Harness {
    engine: Engine,
    cognition: SharedCognition,
    now: f64,
}

struct Builder {
    llm: bool,
    fake_mode: bool,
    sounds_enabled: bool,
    tts_selected: TtsBackendKind,
    tts: TtsProbe,
    turn_delay_seconds: f64,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            // The sound/command tests run with `llm_available=False` so the turn
            // stream never starts and nothing but the test moves the world.
            llm: false,
            fake_mode: false,
            sounds_enabled: true,
            tts_selected: TtsBackendKind::Off,
            tts: TtsProbe::default(),
            turn_delay_seconds: 0.0,
        }
    }
}

impl Builder {
    fn llm(mut self) -> Self {
        self.llm = true;
        self
    }

    fn fake(mut self) -> Self {
        self.fake_mode = true;
        self
    }

    fn voices(mut self, selected: TtsBackendKind, tts: TtsProbe) -> Self {
        self.tts_selected = selected;
        self.tts = tts;
        self
    }

    fn silent_world(mut self) -> Self {
        self.sounds_enabled = false;
        self
    }

    fn build(self) -> Harness {
        let cognition = SharedCognition::default();
        let capabilities = Capabilities::new(
            self.llm,
            false,
            false,
            self.tts_selected == TtsBackendKind::Cloud,
            self.tts_selected == TtsBackendKind::Local,
            self.tts_selected,
        );
        let engine = Engine::new(
            EngineConfig {
                fake_mode: self.fake_mode,
                sounds_enabled: self.sounds_enabled,
                turn_delay_seconds: self.turn_delay_seconds,
                tts_selected: self.tts_selected,
                ..EngineConfig::default()
            },
            &seed(),
            areas(),
            catalog(),
            prompt_env(),
            Box::new(cognition.clone()),
            Box::new(NullTranscription),
            Box::new(self.tts.clone()),
            Box::new(NullSight),
            capabilities,
            (PLAYER_SPAWN, 0.0),
            0,
            0.0,
        )
        .expect("the seeded world has a player");
        Harness {
            engine,
            cognition,
            now: 0.0,
        }
    }
}

impl Harness {
    /// One pump, with whatever the fake provider finished since the last one —
    /// the host's job, done here.
    fn poll(&mut self) -> Vec<EngineMessage> {
        self.send_all(Vec::new())
    }

    fn send(&mut self, command: EngineCommand) -> Vec<EngineMessage> {
        self.send_all(vec![command])
    }

    fn send_all(&mut self, commands: Vec<EngineCommand>) -> Vec<EngineMessage> {
        let mut queued: Vec<EngineCommand> = self
            .cognition
            .drain()
            .into_iter()
            .map(EngineCommand::LlmCompletion)
            .collect();
        queued.extend(commands);
        self.engine.poll(self.now, queued)
    }

    /// Swallow the first poll's `Ready` so the tests can talk about what follows.
    fn ready(&mut self) -> Vec<EngineMessage> {
        let messages = self.poll();
        assert!(matches!(messages[0], EngineMessage::Ready { .. }));
        messages
    }

    fn move_player(&mut self, spatial_seq: i64, facing_yaw: f64) -> Vec<EngineMessage> {
        self.send(EngineCommand::SpatialUpdate {
            spatial_seq,
            updates: vec![cathedral_sim::SpatialActorUpdate::new(
                player(),
                PLAYER_SPAWN,
                Some(facing_yaw),
            )],
        })
    }

    /// Apply an NPC action straight to the world, as the Python tests do.
    fn npc(&mut self, actor: &str, verb: &str, args: serde_json::Value) {
        apply_action(
            self.engine.world_mut(),
            &ActorId::from_raw(actor),
            verb,
            &args,
        )
        .unwrap_or_else(|error| panic!("{actor} {verb}: {error}"));
    }
}

fn seed() -> WorldSeed {
    WorldSeed::from_json_str(&prompt_support::demo_seed()).expect("the demo seed loads")
}

fn player() -> ActorId {
    ActorId::from_raw("player")
}

fn is_town_bell(message: &EngineMessage) -> bool {
    matches!(message, EngineMessage::Sound { sound_id, .. } if sound_id == "town_bell")
}

fn clock_office(messages: &[EngineMessage]) -> Option<Office> {
    messages.iter().find_map(|message| match message {
        EngineMessage::Clock { office, .. } => Some(*office),
        _ => None,
    })
}

/// The offices ring as a *sound for the player alone* — never a percept, never a
/// nudge — and every poll republishes the clock. This is the whole budget
/// argument of `features/movement/01_the_clock.md` §7 in one test: a bell that
/// costs nothing because no character ever hears it.
#[test]
fn the_offices_ring_for_the_player_only_and_publish_the_clock() {
    // One game day in 60 s, opening at Dayspring (07:00); High Wick (noon) rings
    // 12.5 s in. No cognition, so nothing but the clock moves the world.
    let clock = WorldClock::new(60.0, Office::Dayspring, 0, 0.05);
    let mut engine = Engine::new(
        EngineConfig {
            clock,
            ..EngineConfig::default()
        },
        &seed(),
        areas(),
        catalog(),
        prompt_env(),
        Box::new(SharedCognition::default()),
        Box::new(NullTranscription),
        Box::new(TtsProbe::default()),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (PLAYER_SPAWN, 0.0),
        0,
        0.0,
    )
    .expect("the seeded world has a player");

    // The opening office is entered, not rung.
    let opening = engine.poll(0.0, Vec::new());
    assert_eq!(clock_office(&opening), Some(Office::Dayspring));
    assert!(!opening.iter().any(is_town_bell));

    // Past noon: High Wick rings once, and its recipient is the player alone.
    let noon = engine.poll(13.0, Vec::new());
    assert_eq!(clock_office(&noon), Some(Office::HighWick));
    let bells: Vec<&EngineMessage> = noon.iter().filter(|message| is_town_bell(message)).collect();
    assert_eq!(bells.len(), 1, "one stroke of High Wick's four is due by 13 s");
    match bells[0] {
        EngineMessage::Sound {
            recipient_ids,
            witness_ids,
            actor_id,
            text_for_player,
            ..
        } => {
            assert_eq!(
                recipient_ids,
                &vec![player()],
                "no NPC is a recipient, so no inbox is touched"
            );
            assert!(witness_ids.is_empty());
            assert!(actor_id.is_none(), "a world sound is never attributed");
            assert!(text_for_player.is_none(), "the HUD readout carries the hour, not a toast");
        }
        other => panic!("expected a bell, got {other:?}"),
    }

    // The T key speeds time up; the same poll's clock already carries 10×.
    let cycled = engine.poll(13.0, vec![EngineCommand::CycleTimeScale]);
    let scale = cycled.iter().find_map(|message| match message {
        EngineMessage::Clock { scale, .. } => Some(*scale),
        _ => None,
    });
    assert_eq!(scale, Some(10.0));
}

/// A clock-only engine (no cognition moves the world), opened on `clock` at the
/// construction `now == 0.0`, so a later poll's span begins at 0.
fn clock_only_engine(clock: WorldClock) -> Engine {
    Engine::new(
        EngineConfig {
            clock,
            ..EngineConfig::default()
        },
        &seed(),
        areas(),
        catalog(),
        prompt_env(),
        Box::new(SharedCognition::default()),
        Box::new(NullTranscription),
        Box::new(TtsProbe::default()),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (PLAYER_SPAWN, 0.0),
        0,
        0.0,
    )
    .expect("the seeded world has a player")
}

/// When one poll span crosses two offices whose ordinals overlap in time — High
/// Wick's fourth stroke (noon + 9 s) falls after the Waning's first (three game
/// hours later, ~7.5 s away at 60 s/day) — every owed stroke must still ring in
/// due-time order, none stranded behind a later-office stroke. This is the
/// regression for the per-office FIFO that only drained its front
/// (`features/movement/code_review.md` finding 1).
#[test]
fn overlapping_office_ordinals_ring_in_due_order() {
    // 60 s/day opening at Dayspring (07:00). High Wick (noon) rings at 12.5 s
    // with four strokes 3 s apart (12.5/15.5/18.5/21.5); the Waning (15:00) at
    // 20.0 s with five (20.0/23.0/26.0/29.0/32.0). Their strokes interleave —
    // the Waning's first (20.0) lands *between* High Wick's third (18.5) and
    // fourth (21.5) — so a per-office FIFO strands 20.0 behind 21.5.
    let clock = WorldClock::new(60.0, Office::Dayspring, 0, 0.05);
    let mut engine = clock_only_engine(clock);

    let count_bells = |messages: &[EngineMessage]| messages.iter().filter(|m| is_town_bell(m)).count();

    // Enter Dayspring; nothing rings yet.
    assert_eq!(count_bells(&engine.poll(0.0, Vec::new())), 0);

    // One span (0, 21] crosses *both* High Wick and the Waning. Four strokes are
    // due by 21 s (12.5, 15.5, 18.5, 20.0). The old front-only FIFO stranded the
    // Waning's 20.0 behind High Wick's not-yet-due 21.5 and rang only three.
    assert_eq!(
        count_bells(&engine.poll(21.0, Vec::new())),
        4,
        "the Waning's first stroke (20.0 s) is due and must not wait behind High Wick's 21.5 s stroke"
    );

    // Drain the rest of the game day in one big span (21, 70]: High Wick's and
    // the Waning's five remaining strokes plus Lamplight, the Snuffing, and day
    // 1's Watch / Kindling / Dayspring. Every owed stroke of the day must ring,
    // and the whole day's total is the sum of the seven ordinals.
    let rest = count_bells(&engine.poll(70.0, Vec::new()));
    assert_eq!(
        rest + 4,
        (1 + 2 + 3 + 4 + 5 + 6 + 7),
        "one game day rings each office's full ordinal exactly once — nothing lost, nothing doubled"
    );
}

/// The yaw that points the player straight at `actor` (yaw 0 faces -Z).
fn yaw_towards(harness: &Harness, actor: &str) -> f64 {
    let world = harness.engine.world();
    let from = world.characters[&player()].position_m();
    let to = world.characters[&ActorId::from_raw(actor)].position_m();
    (-(to.x - from.x)).atan2(-(to.z - from.z))
}

// ------------------------------------------------------------------- movement

/// The committed street graph — a valid `Some(nav)` so `tick_movement` runs. The
/// mover's path is injected below and long enough never to need re-routing, so
/// the map's geometry does not enter the assertions.
const NAV_JSON: &str = include_str!("../../../assets/world/navigation.json");
const NAV_BIN: &[u8] = include_bytes!("../../../assets/world/navigation.bin");

fn real_nav() -> std::sync::Arc<NavData> {
    std::sync::Arc::new(NavData::from_parts(NAV_JSON, NAV_BIN).expect("the committed nav loads"))
}

/// Movement rides the HOT channel: polling across time advances a mover's
/// `position_m` in fixed 20 Hz slices and republishes it as
/// `EngineMessage::Movement`, and — the whole point of the hot/cold split —
/// **never** bumps `world_revision` or emits a `Snapshot`.
#[test]
fn movement_advances_on_the_hot_channel_without_touching_the_revision() {
    // No cognition, so nothing but movement and the clock can move the world.
    let mut engine = Engine::new(
        EngineConfig {
            nav: Some(real_nav()),
            ..EngineConfig::default()
        },
        &seed(),
        areas(),
        catalog(),
        prompt_env(),
        Box::new(SharedCognition::default()),
        Box::new(NullTranscription),
        Box::new(TtsProbe::default()),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (PLAYER_SPAWN, 0.0),
        0,
        0.0,
    )
    .expect("the seeded world has a player");

    // The demo seed has no pacing actor, so seed one by hand: a demo NPC set on a
    // straight 100 m leg due +x that it cannot exhaust within the test window.
    let mover = ActorId::from_raw("cb947");
    {
        let character = engine
            .world_mut()
            .characters
            .get_mut(&mover)
            .expect("cb947 is in the demo seed");
        character.state.position_m = Vec3::new(0.0, WALK_Y, 0.0);
        character.state.movement = Some(Movement {
            path: vec![Vec3::new(100.0, WALK_Y, 0.0)],
            speed: WALK_SPEED_MPS,
            gait_phase: 0.0,
            patrol: Some(Patrol {
                a: "here".into(),
                b: "there".into(),
                heading_to_b: true,
            }),
        });
    }

    // First poll drains `Ready`; the mover has not moved yet (span (0,0] is empty).
    let ready = engine.poll(0.0, Vec::new());
    assert!(matches!(ready[0], EngineMessage::Ready { .. }));
    let revision_before = engine.world().world_revision;

    // One second later: whole 0.05 s slices of 0.09 m each, ~1.8 m due +x. The
    // exact count is 19 or 20 depending on where 0.05's f64 rounding lands on the
    // boundary — the leftover carries in `movement_now`, so the average holds at
    // 1.8 m/s and any single sample is within one slice of the ideal.
    let messages = engine.poll(1.0, Vec::new());

    let moved = messages
        .iter()
        .find_map(|message| match message {
            EngineMessage::Movement { moved } => Some(moved),
            _ => None,
        })
        .expect("a Movement message was published");
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].actor_id, mover);
    let advance = moved[0].position_m.x;
    assert!(
        (advance - 1.8).abs() <= WALK_SPEED_MPS * 0.05 + 1e-9,
        "advanced ~1.8 m/s (got {advance})"
    );
    // Every slice is an exact 0.09 m, so the total is a whole multiple of it.
    let slices = advance / (WALK_SPEED_MPS * 0.05);
    assert!((slices - slices.round()).abs() < 1e-6, "a whole number of slices");
    assert_eq!(moved[0].speed, WALK_SPEED_MPS as f32);

    // The world itself agrees, and it did so without a snapshot or a revision bump.
    assert_eq!(engine.world().characters[&mover].position_m().x, advance);
    assert_eq!(
        engine.world().world_revision,
        revision_before,
        "a movement-only poll must not touch the public revision"
    );
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, EngineMessage::Snapshot(_))),
        "movement must not re-trigger the snapshot chain"
    );
}

// ---------------------------------------------------- the conversation hold

/// An engine with the committed nav graph and no cognition: the daily round is
/// live (the whole demo cast enrols — lore-less, they read as Majors, so nobody
/// is drafted to keep a well) and nothing but the round and the test moves the
/// world.
fn round_engine() -> Engine {
    Engine::new(
        EngineConfig {
            nav: Some(real_nav()),
            ..EngineConfig::default()
        },
        &seed(),
        areas(),
        catalog(),
        prompt_env(),
        Box::new(SharedCognition::default()),
        Box::new(NullTranscription),
        Box::new(TtsProbe::default()),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (PLAYER_SPAWN, 0.0),
        0,
        0.0,
    )
    .expect("the seeded world has a player")
}

/// M5: a `go_to` ending — here a lapse — hands the walker the scheduler's
/// priority slot through the engine's nudge forwarding, exactly as an
/// addressed `say` would. Off stage there is no idle rotation, so without this
/// the ending's percept would sit unread and the errand chain die silently
/// (`features/movement/05_the_llm_seam.md` §3).
#[test]
fn an_intent_ending_hands_the_walker_the_priority_slot() {
    let mut engine = round_engine();
    engine.poll(0.0, Vec::new()); // drain Ready; the round seeded at construction
    let walker = ActorId::from_raw("cb947");

    let target = engine
        .world()
        .places
        .named("The Gradine")
        .expect("the registry is live")
        .id
        .clone();
    cathedral_sim::apply_action(
        engine.world_mut(),
        &walker,
        "go_to",
        &serde_json::json!({"place_id": target.as_str()}),
    )
    .expect("the coarse handles are everyone's");
    // Shrink the budget so the errand lapses within two polls.
    engine
        .world_mut()
        .characters
        .get_mut(&walker)
        .unwrap()
        .state
        .intent
        .as_mut()
        .unwrap()
        .budget_seconds = 0.5;

    engine.poll(1.0, Vec::new()); // stamps the deadline
    engine.poll(2.0, Vec::new()); // lapses it
    assert!(
        engine.world().characters[&walker].state.intent.is_none(),
        "the intent lapsed"
    );
    assert!(
        engine.world().characters[&walker]
            .inbox()
            .iter()
            .any(|line| line.contains("lapsed")),
        "the lapse is a percept: {:?}",
        engine.world().characters[&walker].inbox()
    );
    assert_eq!(
        engine.scheduler().priority_actor_id(),
        Some(&walker),
        "the nudge reached the scheduler's priority lane"
    );
}

/// NPC↔NPC lines get the player's courtesy
/// (`features/npcs_stop_walking_when_talking_to_each_other.md`): a *targeted*
/// line stops both speaker and target where they stand and holds their rounds
/// while the exchange is warm; a broadcast line interrupts nobody; and the
/// 30 s lapse hands the errand back with no "conversation over" event.
#[test]
fn an_npc_to_npc_line_stops_the_walker_and_a_broadcast_does_not() {
    let mut engine = round_engine();
    engine.poll(0.0, Vec::new()); // drain Ready
    let sven = ActorId::from_raw("sv3n1");
    let ilse = ActorId::from_raw("k0fb1");

    // The wander rung eventually sets Sven milling about his spawn.
    let mut now = 0.0;
    while !engine.world().characters[&sven].is_walking() {
        now += 1.0;
        assert!(now < 600.0, "the wander rung never moved Sven");
        engine.poll(now, Vec::new());
    }

    // A broadcast line from two strides away: Sven keeps walking.
    apply_action(
        engine.world_mut(),
        &ilse,
        "say",
        &json!({"text": "Fresh fish, straight off the boat!"}),
    )
    .expect("Ilse speaks");
    engine.poll(now, Vec::new());
    assert!(
        engine.world().characters[&sven].is_walking(),
        "a broadcast line interrupts nobody"
    );

    // A targeted line: he stops on the spot, before the next movement slice.
    apply_action(
        engine.world_mut(),
        &ilse,
        "say",
        &json!({"text": "Sven — a word.", "target": "sv3n1"}),
    )
    .expect("Ilse addresses Sven");
    engine.poll(now, Vec::new());
    assert!(
        !engine.world().characters[&sven].is_walking(),
        "a targeted line stops the walker mid-stride"
    );

    // While the exchange is warm, the round holds them both where they stand.
    let spoke_at = now;
    while now < spoke_at + 29.0 {
        now += 1.0;
        engine.poll(now, Vec::new());
        assert!(!engine.world().characters[&sven].is_walking(), "held while warm");
        assert!(!engine.world().characters[&ilse].is_walking(), "the speaker too");
    }

    // The lapse: silence hands the errand back on its own.
    let deadline = now + 600.0;
    while !engine.world().characters[&sven].is_walking()
        && !engine.world().characters[&ilse].is_walking()
    {
        now += 1.0;
        assert!(now < deadline, "no errand resumed after the exchange lapsed");
        engine.poll(now, Vec::new());
    }
}

/// A physical handoff is as conversation-shaped as a line — the fish-and-coin
/// case the feature exists for: `offer_item` and `accept_offered_item` hold
/// giver and receiver standing until the exchange goes cold.
#[test]
fn an_item_handoff_holds_giver_and_receiver_standing() {
    let mut engine = round_engine();
    engine.poll(0.0, Vec::new()); // drain Ready
    let sven = ActorId::from_raw("sv3n1"); // holds the fish
    let ilse = ActorId::from_raw("k0fb1"); // 3.6 m away — inside offer range

    // Sven holds out the fish before anyone's first ladder cadence fires.
    apply_action(
        engine.world_mut(),
        &sven,
        "offer_item",
        &json!({"item_id": "fzbn9", "target": "k0fb1"}),
    )
    .expect("Sven offers the fish");
    let mut now = 0.5;
    engine.poll(now, Vec::new());

    // The pair is warm: through the whole window neither sets off on an errand.
    while now < 29.5 {
        now += 1.0;
        engine.poll(now, Vec::new());
        assert!(!engine.world().characters[&sven].is_walking(), "the giver stands");
        assert!(!engine.world().characters[&ilse].is_walking(), "the receiver stands");
    }

    // The accept re-warms the exchange; only its lapse frees them.
    apply_action(
        engine.world_mut(),
        &ilse,
        "accept_offered_item",
        &json!({"item_id": "fzbn9"}),
    )
    .expect("Ilse takes the fish");
    let accepted_at = now;
    engine.poll(now, Vec::new());
    let deadline = now + 600.0;
    while !engine.world().characters[&sven].is_walking()
        && !engine.world().characters[&ilse].is_walking()
    {
        now += 1.0;
        assert!(now < deadline, "no errand resumed after the handoff went cold");
        engine.poll(now, Vec::new());
    }
    assert!(
        now >= accepted_at + 30.0,
        "the handoff held them for the full warm window (freed at {now})"
    );
}

// ------------------------------------------------------------------- matchers

fn sounds(messages: &[EngineMessage]) -> Vec<&EngineMessage> {
    messages
        .iter()
        .filter(|message| matches!(message, EngineMessage::Sound { .. }))
        .collect()
}

fn speeches(messages: &[EngineMessage]) -> Vec<&EngineMessage> {
    messages
        .iter()
        .filter(|message| matches!(message, EngineMessage::Speech { .. }))
        .collect()
}

fn command_results(messages: &[EngineMessage]) -> Vec<&EngineMessage> {
    messages
        .iter()
        .filter(|message| matches!(message, EngineMessage::CommandResult { .. }))
        .collect()
}

fn snapshot_of(messages: &[EngineMessage]) -> Option<&PublicSnapshot> {
    messages.iter().find_map(|message| match message {
        EngineMessage::Snapshot(snapshot) => Some(snapshot),
        EngineMessage::Ready { snapshot, .. } => Some(snapshot),
        _ => None,
    })
}

fn statuses(messages: &[EngineMessage]) -> Vec<&cathedral_sim::StatusEvent> {
    messages
        .iter()
        .filter_map(|message| match message {
            EngineMessage::Status(status) => Some(status),
            _ => None,
        })
        .collect()
}

/// The one `command_result` a single-command poll produces.
fn result(messages: &[EngineMessage]) -> (bool, Option<String>, String) {
    let results = command_results(messages);
    assert_eq!(results.len(), 1, "{messages:#?}");
    match results[0] {
        EngineMessage::CommandResult {
            success,
            error_code,
            message,
            ..
        } => (*success, error_code.clone(), message.clone()),
        _ => unreachable!(),
    }
}

// ==================================================================== the tests

/// 5. `test_ready_contains_capabilities_and_full_snapshot`
#[test]
fn ready_comes_first_and_carries_the_capabilities_and_the_whole_snapshot() {
    let mut harness = Builder::default().build();
    let messages = harness.poll();

    let EngineMessage::Ready {
        capabilities,
        snapshot,
    } = &messages[0]
    else {
        panic!("ready is the first message ever emitted: {messages:#?}");
    };
    // Nothing configured: the all-false block, voices off.
    assert_eq!(*capabilities, Capabilities::default());
    assert_eq!(snapshot.player_id, player());
    assert_eq!(snapshot.actors.len(), 4);
    assert_eq!(snapshot.items.len(), 2);
    assert!(snapshot.offers.is_empty());

    // The privacy invariant, asserted the way Python did — on the encoded bytes.
    let encoded = serde_json::to_string(&snapshot).unwrap();
    for secret in [
        "back_story",
        "memories",
        "goal",
        "voice_key",
        "inbox",
        "knows",
    ] {
        assert!(
            !encoded.contains(secret),
            "{secret} leaked into the snapshot"
        );
    }

    // Without cognition the cast is *offline*, not merely quiet — said once.
    let statuses = statuses(&messages);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].subsystem, Subsystem::Llm);
    assert_eq!(statuses[0].state, "unavailable");
    assert_eq!(
        statuses[0].message.as_deref(),
        Some("text cognition is not configured")
    );

    // Ready is emitted exactly once.
    let again = harness.poll();
    assert!(
        !again
            .iter()
            .any(|m| matches!(m, EngineMessage::Ready { .. }))
    );
    assert!(
        !again
            .iter()
            .any(|m| matches!(m, EngineMessage::Snapshot(_)))
    );
}

/// 49. `test_ready_snapshot_ships_actor_facing`
#[test]
fn the_ready_snapshot_ships_every_actors_facing() {
    let mut harness = Builder::default().build();
    let messages = harness.poll();
    let snapshot = snapshot_of(&messages).expect("ready carries a snapshot");

    for actor in &snapshot.actors {
        assert!(actor.facing_yaw.is_finite(), "{}", actor.id);
    }
    // The spawn *is* the first spatial update, and it carries the facing.
    let spawned = snapshot
        .actors
        .iter()
        .find(|actor| actor.id == player())
        .unwrap();
    assert_eq!(spawned.position_m, PLAYER_SPAWN);
    assert_eq!(spawned.facing_yaw, 0.0);
    assert_eq!(spawned.name_for_player, "You");
}

#[test]
fn a_world_without_a_player_is_not_a_world_the_engine_will_run() {
    let seed = WorldSeed::from_json_str(r#"{"items": [], "characters": []}"#).unwrap();
    let outcome = Engine::new(
        EngineConfig::default(),
        &seed,
        areas(),
        catalog(),
        prompt_env(),
        Box::new(SharedCognition::default()),
        Box::new(NullTranscription),
        Box::new(TtsProbe::default()),
        Box::new(NullSight),
        Capabilities::default(),
        (PLAYER_SPAWN, 0.0),
        0,
        0.0,
    );
    let Err(error) = outcome else {
        panic!("a world with nobody in it has no player either");
    };
    assert_eq!(error, EngineInitError::MissingPlayer(player()));
}

/// 9. `test_spatial_updates_are_atomic_monotonic_and_snapshotted`
#[test]
fn spatial_updates_are_monotonic_and_snapshotted() {
    let mut harness = Builder::default().build();
    harness.ready();

    let moved = Vec3::new(4.0, 0.91, 100.0);
    let messages = harness.send(EngineCommand::SpatialUpdate {
        spatial_seq: 1,
        updates: vec![cathedral_sim::SpatialActorUpdate::new(
            player(),
            moved,
            None,
        )],
    });
    let snapshot = snapshot_of(&messages).expect("a position change bumps the revision");
    let after = snapshot.actors.iter().find(|a| a.id == player()).unwrap();
    assert_eq!(after.position_m, moved);
    let revision = snapshot.world_revision;

    // A stale (smaller) sequence is refused and moves nobody.
    let messages = harness.send(EngineCommand::SpatialUpdate {
        spatial_seq: 0,
        updates: vec![cathedral_sim::SpatialActorUpdate::new(
            player(),
            Vec3::new(99.0, 0.91, 99.0),
            None,
        )],
    });
    assert!(snapshot_of(&messages).is_none(), "nothing changed");
    assert!(
        messages.iter().any(
            |m| matches!(m, EngineMessage::Diagnostic(line) if line.contains("spatial_update"))
        )
    );
    assert_eq!(
        harness.engine.world().characters[&player()].position_m(),
        moved
    );
    assert_eq!(harness.engine.snapshot().world_revision, revision);
}

/// 10. `test_unknown_actor_and_nonfinite_position_are_rejected`
#[test]
fn only_the_player_may_be_moved() {
    let mut harness = Builder::default().build();
    harness.ready();
    let sven = ActorId::from_raw("sv3n1");
    let before = harness.engine.world().characters[&sven].position_m();
    let sequence = harness.engine.world().spatial_sequence;

    for actor_id in [sven.clone(), ActorId::from_raw("nobody")] {
        let messages = harness.send(EngineCommand::SpatialUpdate {
            spatial_seq: 5,
            updates: vec![cathedral_sim::SpatialActorUpdate::new(
                actor_id,
                Vec3::new(1.0, 0.91, 1.0),
                None,
            )],
        });
        assert!(
            messages.iter().any(
                |m| matches!(m, EngineMessage::Diagnostic(line) if line.contains("only move the player"))
            ),
            "{messages:#?}"
        );
    }
    // A known NPC's position is world state, not something a client may rewrite.
    assert_eq!(
        harness.engine.world().characters[&sven].position_m(),
        before
    );
    assert_eq!(harness.engine.world().spatial_sequence, sequence);
}

/// 13. `test_player_accept_transfers_once_and_deduplicates_request` — the
///     transfer half. The dedup half is a deliberate behavior change (D14): the
///     second accept now errors instead of replaying a cached success.
#[test]
fn accepting_an_offer_transfers_the_item_exactly_once() {
    let mut harness = Builder::default().build();
    harness.ready();
    let coin = ItemId::from_raw("c0prs");
    let ilse = ActorId::from_raw("k0fb1");
    harness.npc(
        "k0fb1",
        "offer_item",
        json!({"item_id": "c0prs", "target": "player"}),
    );

    let messages = harness.send(EngineCommand::PlayerAccept {
        request_id: "accept-1".into(),
        item_id: coin.clone(),
        position_m: PLAYER_SPAWN,
        spatial_seq: 1,
    });
    let (success, code, _) = result(&messages);
    assert!(success, "{code:?}");

    let world = harness.engine.world();
    assert_eq!(
        world.characters[&player()].holds(),
        std::slice::from_ref(&coin)
    );
    assert!(world.characters[&ilse].holds().is_empty());
    assert!(world.offers.is_empty());
    world.assert_invariants();

    // D14: no idempotent replay cache survives the protocol. A genuinely
    // repeated accept is a bug in the caller and now says so.
    let messages = harness.send(EngineCommand::PlayerAccept {
        request_id: "accept-2".into(),
        item_id: coin.clone(),
        position_m: PLAYER_SPAWN,
        spatial_seq: 2,
    });
    let (success, code, _) = result(&messages);
    assert!(!success);
    assert_eq!(code.as_deref(), Some("no_offer"));
    assert_eq!(harness.engine.world().characters[&player()].holds(), [coin]);
}

/// 14. `test_failed_player_action_returns_code_without_mutation`
#[test]
fn a_failed_player_action_returns_its_code_and_moves_nothing() {
    let mut harness = Builder::default().build();
    harness.ready();
    let fish = ItemId::from_raw("fzbn9");
    let sven = ActorId::from_raw("sv3n1");

    // The player does not hold Sven's fish.
    let messages = harness.send(EngineCommand::PlayerOffer {
        request_id: "offer-1".into(),
        item_id: fish.clone(),
        target_id: ActorId::from_raw("cb947"),
        position_m: PLAYER_SPAWN,
        spatial_seq: 1,
    });
    let (success, code, message) = result(&messages);
    assert!(!success);
    assert_eq!(code.as_deref(), Some("not_owner"));
    assert!(!message.is_empty());

    assert_eq!(harness.engine.world().characters[&sven].holds(), [fish]);
    assert!(harness.engine.world().offers.is_empty());
}

/// §6.2 step 4: the position update that came with a *failing* action may well
/// have succeeded, and its revision bump has to reach the game — so the events
/// and the snapshot go out BEFORE the failure result.
#[test]
fn a_failed_action_still_ships_the_position_it_moved_the_player_to() {
    let mut harness = Builder::default().build();
    harness.ready();

    let walked = Vec3::new(1.0, 0.91, 110.0);
    let messages = harness.send(EngineCommand::PlayerAccept {
        request_id: "accept-1".into(),
        // Nobody has offered the player anything.
        item_id: ItemId::from_raw("fzbn9"),
        position_m: walked,
        spatial_seq: 1,
    });

    let snapshot_at = messages
        .iter()
        .position(|m| matches!(m, EngineMessage::Snapshot(_)))
        .expect("the move bumped the revision");
    let result_at = messages
        .iter()
        .position(|m| matches!(m, EngineMessage::CommandResult { .. }))
        .expect("the accept failed");
    assert!(snapshot_at < result_at, "{messages:#?}");

    let (success, code, _) = result(&messages);
    assert!(!success);
    assert_eq!(code.as_deref(), Some("no_offer"));
    // The walk stuck even though the accept did not.
    assert_eq!(
        harness.engine.world().characters[&player()].position_m(),
        walked
    );
}

/// 15. `test_debug_say_is_fake_only_and_uses_real_range_validation`
#[test]
fn debug_say_is_fake_only_and_gets_the_real_sim_validation() {
    let mut harness = Builder::default().build();
    harness.ready();
    let messages = harness.send(EngineCommand::DebugPlayerSay {
        request_id: "say-1".into(),
        text: "Hello?".into(),
        target_id: None,
        position_m: PLAYER_SPAWN,
        spatial_seq: 1,
    });
    let (success, code, message) = result(&messages);
    assert!(!success);
    assert_eq!(code.as_deref(), Some("forbidden"));
    assert_eq!(message, "debug_player_say is available only in fake mode");
    assert!(speeches(&messages).is_empty());

    // In fake mode it is a real `say`, with the real recipient calculation.
    let mut harness = Builder::default().fake().build();
    harness.ready();
    let messages = harness.send(EngineCommand::DebugPlayerSay {
        request_id: "say-1".into(),
        text: "What's your name?".into(),
        target_id: None,
        position_m: PLAYER_SPAWN,
        spatial_seq: 1,
    });
    let (success, code, _) = result(&messages);
    assert!(success, "{code:?}");

    let said = speeches(&messages);
    assert_eq!(said.len(), 1);
    let EngineMessage::Speech {
        speaker_id,
        target_id,
        recipient_ids,
        speaker_name_for_player,
        ..
    } = said[0]
    else {
        unreachable!()
    };
    assert_eq!(*speaker_id, player());
    assert_eq!(*target_id, None, "a broadcast has no target");
    assert!(recipient_ids.contains(&ActorId::from_raw("k0fb1")));
    assert_eq!(speaker_name_for_player, "You");

    // Full sim validation: the 500-character cap applies to the player too.
    let messages = harness.send(EngineCommand::DebugPlayerSay {
        request_id: "say-2".into(),
        text: "x".repeat(501),
        target_id: None,
        position_m: PLAYER_SPAWN,
        spatial_seq: 2,
    });
    let (success, code, _) = result(&messages);
    assert!(!success);
    assert_eq!(code.as_deref(), Some("text_too_long"));
}

/// The Enter chat box: a typed `say` is a real `say` in ANY mode — the fake
/// gate is `DebugPlayerSay`'s alone — and, like a transcribed utterance, it
/// hands the nearest LLM hearer the protected player-reaction slot.
#[test]
fn typed_player_say_works_outside_fake_mode_and_nudges_the_nearest_hearer() {
    let mut harness = Builder::default().build();
    harness.ready();
    assert_eq!(harness.engine.scheduler().priority_actor_id(), None);

    let messages = harness.send(EngineCommand::PlayerSay {
        request_id: "say-1".into(),
        text: "Good morning!".into(),
        position_m: PLAYER_SPAWN,
        spatial_seq: 1,
    });
    let (success, code, _) = result(&messages);
    assert!(success, "{code:?}");

    let said = speeches(&messages);
    assert_eq!(said.len(), 1);
    let EngineMessage::Speech {
        speaker_id,
        target_id,
        recipient_ids,
        ..
    } = said[0]
    else {
        unreachable!()
    };
    assert_eq!(*speaker_id, player());
    assert_eq!(*target_id, None, "typed chat is a broadcast");
    assert!(recipient_ids.contains(&ActorId::from_raw("k0fb1")));
    // The nearest LLM hearer got the protected reaction slot (it may already
    // have been submitted within the same poll).
    assert!(
        harness.engine.scheduler().priority_actor_id().is_some()
            || harness.engine.scheduler().in_flight_is_player_reaction()
    );

    // Full sim validation still applies: the 500-character `say` cap.
    let messages = harness.send(EngineCommand::PlayerSay {
        request_id: "say-2".into(),
        text: "x".repeat(501),
        position_m: PLAYER_SPAWN,
        spatial_seq: 2,
    });
    let (success, code, _) = result(&messages);
    assert!(!success);
    assert_eq!(code.as_deref(), Some("text_too_long"));
}

/// A targeted debug say hands the addressee the next *selection* slot — not an
/// immediate turn: the inter-turn delay and the floor still govern the timing
/// (`server.py:1050-1051`).
#[test]
fn a_targeted_player_say_gives_the_addressee_the_next_slot() {
    let mut harness = Builder::default().fake().build();
    harness.ready();
    assert_eq!(harness.engine.scheduler().priority_actor_id(), None);

    harness.send(EngineCommand::DebugPlayerSay {
        request_id: "say-1".into(),
        text: "Ilse?".into(),
        target_id: Some(ActorId::from_raw("k0fb1")),
        position_m: PLAYER_SPAWN,
        spatial_seq: 1,
    });
    assert_eq!(
        harness.engine.scheduler().priority_actor_id(),
        Some(&ActorId::from_raw("k0fb1"))
    );
}

/// 62. `test_player_speech_never_holds_the_floor` — from the engine's side.
#[test]
fn the_players_own_speech_never_holds_the_npc_floor() {
    let mut harness = Builder::default().fake().build();
    harness.ready();
    harness.send(EngineCommand::DebugPlayerSay {
        request_id: "say-1".into(),
        text: "A line long enough to have a reading time.".into(),
        target_id: None,
        position_m: PLAYER_SPAWN,
        spatial_seq: 1,
    });
    assert!(!harness.engine.floor_busy(harness.now));
}

// ------------------------------------------------------------------- voices

/// 20. `test_off_mode_never_queues_synthesis`
#[test]
fn voices_off_never_queues_synthesis_but_the_text_still_ships() {
    let tts = TtsProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Off, tts.clone())
        .build();
    harness.ready();

    harness.npc("k0fb1", "say", json!({"text": "Voices are off."}));
    let messages = harness.poll();

    assert_eq!(speeches(&messages).len(), 1, "the text still ships");
    assert!(tts.submitted().is_empty(), "no backend received work");
    assert!(
        !messages
            .iter()
            .any(|m| matches!(m, EngineMessage::TtsFailed { .. })),
        "voices being off is not a failure"
    );
    // Nothing to await, so the line paces on its reading estimate instead.
    assert!(harness.engine.floor_busy(harness.now));
    harness.now = speech_reading_seconds("Voices are off.") + 0.01;
    assert!(!harness.engine.floor_busy(harness.now));
}

#[test]
fn a_voiced_line_is_awaited_until_the_game_reports_it_presented() {
    let tts = TtsProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Local, tts.clone())
        .build();
    harness.ready();

    let text = "You may have my copper coin.";
    harness.npc("k0fb1", "say", json!({"text": text}));
    let messages = harness.poll();

    let submitted = tts.submitted();
    assert_eq!(submitted.len(), 1);
    assert_eq!(submitted[0].text, text);
    assert_eq!(submitted[0].kind, TtsBackendKind::Local);
    assert!(!submitted[0].voice_key.is_empty());

    let synthesizing = statuses(&messages)
        .into_iter()
        .find(|status| status.state == "synthesizing")
        .expect("a submitted line reports it");
    assert_eq!(synthesizing.subsystem, Subsystem::Tts);
    assert_eq!(synthesizing.actor_id, Some(ActorId::from_raw("k0fb1")));
    assert_eq!(synthesizing.backend.as_deref(), Some("local"));

    // Awaited, not merely paced: the floor waits for the presentation, well past
    // the reading estimate.
    let event_id = speech_event_id(&messages);
    harness.now = 60.0_f64.min(speech_reading_seconds(text) + 1.0);
    assert!(harness.engine.floor_busy(harness.now));

    harness.send(EngineCommand::SpeechPresented {
        event_id: event_id.clone(),
    });
    // Only the 0.4 s beat is left.
    harness.now += 0.5;
    assert!(!harness.engine.floor_busy(harness.now));

    // Idempotent (D26): a duplicate ack must not re-arm the beat.
    harness.send(EngineCommand::SpeechPresented { event_id });
    assert!(!harness.engine.floor_busy(harness.now));
}

#[test]
fn a_refused_submission_degrades_to_text_and_never_awaits_audio() {
    for (error, reason) in [
        (TtsSubmitError::QueueFull, "speech queue is full"),
        (
            TtsSubmitError::PathInUse,
            "speech output path is already in use",
        ),
    ] {
        let tts = TtsProbe::refusing(error);
        let mut harness = Builder::default()
            .voices(TtsBackendKind::Local, tts)
            .build();
        harness.ready();

        harness.npc("k0fb1", "say", json!({"text": "Refused."}));
        let messages = harness.poll();

        let degraded = statuses(&messages)
            .into_iter()
            .find(|status| status.state == "degraded")
            .expect("a refusal is degraded, not silent");
        assert_eq!(degraded.subsystem, Subsystem::Tts);
        assert_eq!(degraded.message.as_deref(), Some(reason));
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, EngineMessage::TtsFailed { reason: r, .. } if r == reason))
        );

        // R10: a refused line must not stall the cast for the failsafe window —
        // it falls back to the reading estimate.
        assert!(harness.engine.floor_busy(harness.now));
        harness.now = speech_reading_seconds("Refused.") + 0.01;
        assert!(!harness.engine.floor_busy(harness.now));
    }
}

#[test]
fn an_unavailable_voice_backend_says_so_and_the_line_stays_text() {
    // `available == false` while `tts_selected == local`: the configured backend
    // went away under us.
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Local, TtsProbe::default())
        .build();
    harness.ready();

    harness.npc("k0fb1", "say", json!({"text": "No voice."}));
    let messages = harness.poll();

    let unavailable = statuses(&messages)
        .into_iter()
        .find(|status| status.state == "unavailable" && status.subsystem == Subsystem::Tts)
        .expect("the missing backend is reported");
    assert_eq!(
        unavailable.message.as_deref(),
        Some("local NPC voice backend is unavailable")
    );
    assert_eq!(unavailable.backend.as_deref(), Some("local"));
    assert_eq!(speeches(&messages).len(), 1, "the text still ships");
}

#[test]
fn a_line_the_player_cannot_hear_is_never_synthesized() {
    let tts = TtsProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Local, tts.clone())
        .build();
    harness.ready();

    // Out of earshot: no audio for an inaudible line (`server.py:1970`).
    harness.send(EngineCommand::SpatialUpdate {
        spatial_seq: 1,
        updates: vec![cathedral_sim::SpatialActorUpdate::new(
            player(),
            Vec3::new(0.0, 0.91, 111.0 + HEARING_RADIUS_M * 5.0),
            None,
        )],
    });

    harness.npc("k0fb1", "say", json!({"text": "Nobody in earshot."}));
    let messages = harness.poll();
    assert_eq!(speeches(&messages).len(), 1, "the event still exists");
    assert!(tts.submitted().is_empty());
}

#[test]
fn synthesis_outcomes_become_messages_and_a_failure_releases_the_floor() {
    let tts = TtsProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Local, tts)
        .build();
    harness.ready();

    harness.npc("k0fb1", "say", json!({"text": "Streamed."}));
    let messages = harness.poll();
    let event_id = speech_event_id(&messages);

    let samples: Arc<[i16]> = Arc::from(vec![0i16, 1, 2].into_boxed_slice());
    let messages = harness.send(EngineCommand::Tts(TtsOutcome::Chunk {
        event_id: event_id.clone(),
        seq: 0,
        sample_rate: 24_000,
        samples: samples.clone(),
    }));
    assert!(messages.iter().any(|m| matches!(
        m,
        EngineMessage::TtsChunk {
            chunk_seq: 0,
            sample_rate: 24_000,
            ..
        }
    )));

    let messages = harness.send(EngineCommand::Tts(TtsOutcome::StreamEnd {
        event_id: event_id.clone(),
        chunk_count: 2,
        first_chunk_ms: 173,
    }));
    assert!(messages.iter().any(|m| matches!(
        m,
        EngineMessage::TtsStreamEnd {
            chunk_count: 2,
            first_chunk_ms: 173,
            ..
        }
    )));

    // A whole-WAV success.
    let wav: Arc<[u8]> = Arc::from(vec![0u8; 8].into_boxed_slice());
    let messages = harness.send(EngineCommand::Tts(TtsOutcome::Done {
        event_id: event_id.clone(),
        result: Ok(wav),
    }));
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, EngineMessage::TtsReady { .. }))
    );

    // The line is still awaited (nothing was presented), and a synthesis failure
    // is what frees it — otherwise a dead worker stalls the cast for 45 s.
    assert!(harness.engine.floor_busy(harness.now));
    let messages = harness.send(EngineCommand::Tts(TtsOutcome::Done {
        event_id: event_id.clone(),
        result: Err(SpeechError::new("the request timed out")),
    }));
    assert!(messages.iter().any(
        |m| matches!(m, EngineMessage::TtsFailed { reason, .. } if reason == "the request timed out")
    ));
    harness.now += 0.5; // past the post-utterance beat
    assert!(!harness.engine.floor_busy(harness.now));
}

/// 16/17. `test_tts_backend_selection_is_strict…` / `…unavailable_tts_does_not_change_server_state`
///
/// The `invalid_tts_backend` half dies with the wire: the backend arrives typed,
/// so an "automatic" string never reaches the engine.
#[test]
fn selecting_a_voice_backend_is_acknowledged_and_refuses_what_it_cannot_do() {
    let mut harness = Builder::default().build();
    harness.ready();
    assert_eq!(
        harness.engine.capabilities().tts_selected,
        TtsBackendKind::Off
    );

    // No backend is available: the selection fails and the state does not move.
    let messages = harness.send(EngineCommand::SetTtsBackend {
        request_id: "tts-1".into(),
        backend: TtsBackendKind::Local,
    });
    let (success, code, message) = result(&messages);
    assert!(!success);
    assert_eq!(code.as_deref(), Some("tts_unavailable"));
    assert_eq!(message, "local NPC voice backend is unavailable");
    assert_eq!(
        harness.engine.capabilities().tts_selected,
        TtsBackendKind::Off
    );

    // With a live backend it succeeds, acks, warms, and reports the selection.
    let tts = TtsProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Off, tts.clone())
        .build();
    harness.ready();
    let messages = harness.send(EngineCommand::SetTtsBackend {
        request_id: "tts-2".into(),
        backend: TtsBackendKind::Local,
    });
    let (success, _, message) = result(&messages);
    assert!(success);
    assert_eq!(message, "NPC voice backend set to local");
    assert_eq!(
        harness.engine.capabilities().tts_selected,
        TtsBackendKind::Local
    );
    assert_eq!(tts.warmed(), [TtsBackendKind::Local]);

    let selected = statuses(&messages)
        .into_iter()
        .find(|status| status.state == "selected")
        .expect("the HUD is told");
    assert_eq!(selected.message.as_deref(), Some("local"));
    assert_eq!(selected.backend.as_deref(), Some("local"));

    // Turning them off never needs a backend.
    let messages = harness.send(EngineCommand::SetTtsBackend {
        request_id: "tts-3".into(),
        backend: TtsBackendKind::Off,
    });
    assert!(result(&messages).0);
    assert_eq!(
        harness.engine.capabilities().tts_selected,
        TtsBackendKind::Off
    );
}

/// `server.py:594-599`: a run that *starts* on the local voice warms it at
/// construction, not inside the cast's first line. Without this the model load
/// lands in the first `submit`, and a slow enough cold start outlives the
/// floor's failsafe deadline and drops the line's audio.
#[test]
fn a_run_that_starts_on_the_local_voice_warms_it_before_the_first_line() {
    let tts = TtsProbe::available();
    let _harness = Builder::default()
        .voices(TtsBackendKind::Local, tts.clone())
        .build();
    assert_eq!(
        tts.warmed(),
        [TtsBackendKind::Local],
        "the model loads at startup, not on the first utterance"
    );

    // Cloud needs no warmup, and neither does silence.
    for selected in [TtsBackendKind::Cloud, TtsBackendKind::Off] {
        let tts = TtsProbe::available();
        let _harness = Builder::default().voices(selected, tts.clone()).build();
        assert!(tts.warmed().is_empty(), "{selected:?}");
    }
}

/// `server.py:500-512` + `860-861`: when the host's availability fallback
/// silences a configured voice, the reason is sent as a `tts`/`unavailable`
/// status right after `ready` — otherwise the player gets a mute cast with no
/// explanation. It is said once, and never for a run that asked for silence.
#[test]
fn a_silenced_voice_explains_itself_once_after_ready() {
    let mut harness = Harness {
        engine: Engine::new(
            EngineConfig {
                tts_selected: TtsBackendKind::Off,
                tts_startup_message: Some(
                    "Configured local NPC voice backend is unavailable; voices are off".to_string(),
                ),
                ..EngineConfig::default()
            },
            &seed(),
            areas(),
            catalog(),
            prompt_env(),
            Box::new(SharedCognition::default()),
            Box::new(NullTranscription),
            Box::new(TtsProbe::default()),
            Box::new(NullSight),
            Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
            (PLAYER_SPAWN, 0.0),
            0,
            0.0,
        )
        .expect("the seeded world has a player"),
        cognition: SharedCognition::default(),
        now: 0.0,
    };

    let messages = harness.ready();
    let told = statuses(&messages)
        .into_iter()
        .find(|status| status.subsystem == Subsystem::Tts)
        .expect("the player is told why the cast is mute");
    assert_eq!(told.state, "unavailable");
    assert_eq!(
        told.message.as_deref(),
        Some("Configured local NPC voice backend is unavailable; voices are off")
    );

    // Said once: the next pump repeats nothing.
    let messages = harness.poll();
    assert!(
        statuses(&messages)
            .into_iter()
            .all(|status| status.subsystem != Subsystem::Tts)
    );

    // A run that simply has no voice configured has nothing to apologize for.
    let mut harness = Builder::default().build();
    let messages = harness.ready();
    assert!(
        statuses(&messages)
            .into_iter()
            .all(|status| status.subsystem != Subsystem::Tts)
    );
}

// ------------------------------------------------------------------- sounds

/// 46. `test_player_facing_gates_the_player_hud_percept` — S7: the player is
///     subject to the identical cone test, and fails dark.
#[test]
fn the_players_facing_gates_his_own_percept_and_the_actor_is_withheld() {
    let mut harness = Builder::default().build();
    harness.ready();
    let towards_sven = yaw_towards(&harness, "sv3n1");

    harness.move_player(1, towards_sven);
    harness.npc("sv3n1", "make_sound", json!({"sound": "fart"}));
    let messages = harness.poll();

    let seen = sounds(&messages);
    assert_eq!(seen.len(), 1);
    let EngineMessage::Sound {
        sound_id,
        sound_class,
        actor_id,
        audible_distance,
        witness_ids,
        text_for_player,
        ..
    } = seen[0]
    else {
        unreachable!()
    };
    assert_eq!(sound_id, "fart");
    assert_eq!(sound_class, "body");
    assert_eq!(*audible_distance, 20.0);
    assert_eq!(text_for_player.as_deref(), Some("Sven farted."));
    assert_eq!(*actor_id, Some(ActorId::from_raw("sv3n1")));
    assert!(witness_ids.contains(&player()));

    // Turn around: the same sound, heard but not seen.
    harness.move_player(2, towards_sven + std::f64::consts::PI);
    harness.npc("sv3n1", "make_sound", json!({"sound": "fart"}));
    let messages = harness.poll();

    let heard = sounds(&messages);
    assert_eq!(heard.len(), 1);
    let EngineMessage::Sound {
        actor_id,
        witness_ids,
        recipient_ids,
        text_for_player,
        ..
    } = heard[0]
    else {
        unreachable!()
    };
    assert_eq!(text_for_player.as_deref(), Some("[You heard a big fart!]"));
    // Fail dark: an unattributed sound must not leak its actor's id.
    assert_eq!(*actor_id, None);
    assert!(!witness_ids.contains(&player()));
    assert!(recipient_ids.contains(&player()));
}

/// 47. `test_player_sound_is_rate_limited_and_confirmed` — S9.
#[test]
fn the_players_own_sound_is_confirmed_and_rate_limited() {
    let mut harness = Builder::default().build();
    harness.ready();

    let messages = harness.send(EngineCommand::PlayerSound {
        sound_id: "fart".into(),
    });
    let own = sounds(&messages);
    assert_eq!(own.len(), 1);
    let EngineMessage::Sound {
        actor_id,
        text_for_player,
        ..
    } = own[0]
    else {
        unreachable!()
    };
    // HUD confirmation even with nobody in range, or F feels broken.
    assert_eq!(text_for_player.as_deref(), Some("You farted."));
    assert_eq!(*actor_id, Some(player()));

    // Inside the 2 s cooldown: dropped silently, not queued — percepts are
    // prompt tokens, and holding F must not become a DoS on the LLM bill.
    harness.now = 1.0;
    let messages = harness.send(EngineCommand::PlayerSound {
        sound_id: "fart".into(),
    });
    assert!(sounds(&messages).is_empty());

    harness.now = 2.5;
    let messages = harness.send(EngineCommand::PlayerSound {
        sound_id: "fart".into(),
    });
    assert_eq!(sounds(&messages).len(), 1);

    // Fire-and-forget: `player_sound` never produces a command result.
    assert!(command_results(&messages).is_empty());
}

/// 48. `test_player_sound_nudges_the_nearest_witness`
#[test]
fn a_sound_nudges_the_nearest_llm_reactor_exactly_once() {
    let mut harness = Builder::default().build();
    harness.ready();
    assert_eq!(harness.engine.scheduler().priority_actor_id(), None);

    let messages = harness.send(EngineCommand::PlayerSound {
        sound_id: "fart".into(),
    });
    let EngineMessage::Sound {
        recipient_ids,
        witness_ids,
        ..
    } = sounds(&messages)[0]
    else {
        unreachable!()
    };
    assert!(
        !recipient_ids.is_empty(),
        "the seeded NPCs are in fart range"
    );

    // A percept in an inbox does nothing until that actor's turn: the nearest
    // witness (falling back to the nearest hearer) is handed the next slot.
    let nudged = harness
        .engine
        .scheduler()
        .priority_actor_id()
        .expect("somebody reacts")
        .clone();
    assert!(recipient_ids.contains(&nudged));
    let expected = witness_ids.first().unwrap_or(&recipient_ids[0]);
    assert_eq!(&nudged, expected, "the nearest one, in distance order");
}

/// 49. `test_debug_sound_rings_the_bell_without_an_actor`
#[test]
fn the_drive_mode_bell_rings_without_an_actor() {
    let mut harness = Builder::default().build();
    harness.ready();

    let messages = harness.send(EngineCommand::DebugSound {
        sound_id: "town_bell".into(),
        position_m: Vec3::new(0.0, 40.0, 140.0),
    });
    let EngineMessage::Sound {
        sound_id,
        actor_id,
        witness_ids,
        text_for_player,
        ..
    } = sounds(&messages)[0]
    else {
        unreachable!()
    };
    assert_eq!(sound_id, "town_bell");
    // World sounds are never attributed, and nothing witnesses them.
    assert_eq!(*actor_id, None);
    assert!(witness_ids.is_empty());
    assert_eq!(
        text_for_player.as_deref(),
        Some("[The town bell is ringing.]")
    );
}

#[test]
fn the_player_cannot_ring_the_bell_and_an_unknown_sound_is_only_diagnosed() {
    let mut harness = Builder::default().build();
    harness.ready();

    // `town_bell` is in the catalog but not `actor_emittable`; an unknown id is
    // indistinguishable from it, so no information leaks either way.
    for sound_id in ["town_bell", "no_such_sound"] {
        let messages = harness.send(EngineCommand::PlayerSound {
            sound_id: sound_id.into(),
        });
        assert!(sounds(&messages).is_empty());
        assert!(command_results(&messages).is_empty());
        assert!(
            messages.iter().any(
                |m| matches!(m, EngineMessage::Diagnostic(line) if line.contains("player-emittable"))
            ),
            "{messages:#?}"
        );
    }
}

#[test]
fn a_silent_world_emits_no_sounds_at_all() {
    let mut harness = Builder::default().silent_world().build();
    harness.ready();

    let messages = harness.send(EngineCommand::PlayerSound {
        sound_id: "fart".into(),
    });
    assert!(sounds(&messages).is_empty());

    let messages = harness.send(EngineCommand::DebugSound {
        sound_id: "town_bell".into(),
        position_m: Vec3::new(0.0, 40.0, 140.0),
    });
    assert!(sounds(&messages).is_empty());
}

// ------------------------------------------------------- scheduler plumbing

#[test]
fn the_turn_stream_only_runs_when_there_is_cognition_to_run_it() {
    // No LLM: the scheduler never starts, and nothing is ever asked.
    let mut harness = Builder::default().fake().build();
    for _ in 0..3 {
        harness.poll();
    }
    assert_eq!(harness.engine.scheduler().in_flight_actor_id(), None);

    // With cognition, the first poll submits Sven's turn (roster order) and the
    // second applies it — one request in flight, ever.
    let mut harness = Builder::default().fake().llm().build();
    let messages = harness.poll();
    assert_eq!(
        harness.engine.scheduler().in_flight_actor_id(),
        Some(&ActorId::from_raw("sv3n1"))
    );
    assert!(
        statuses(&messages)
            .into_iter()
            .any(|status| status.state == "thinking")
    );

    let messages = harness.poll();
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, EngineMessage::PromptExchange { .. })),
        "every exchange is archived, successes and failures alike"
    );
    assert!(
        statuses(&messages)
            .into_iter()
            .any(|status| status.state == "idle")
    );
}

/// A run with no transcription backend at all (the headless default) still
/// answers the microphone rather than stranding the game's pending request —
/// and, crucially, it never leaves a player hold behind that would stall the
/// cast forever. The router's own behavior lives in `speech_router_tests.rs`.
#[test]
fn a_deaf_engine_refuses_recordings_and_frees_the_floor() {
    let mut harness = Builder::default().build();
    harness.ready();

    harness.send_all(vec![
        EngineCommand::PlayerAudioBegin {
            wav_basename: "utterance.wav".into(),
            sample_rate: 24_000,
        },
        EngineCommand::PlayerAudioChunk {
            wav_basename: "utterance.wav".into(),
            seq: 0,
            samples: Arc::from(vec![0i16; 8].into_boxed_slice()),
        },
    ]);
    // The microphone *does* hold the floor while it streams (that is the point),
    // but the hold is rolling and self-expiring.
    assert!(harness.engine.floor_busy(harness.now));
    harness.now = cathedral_sim::FLOOR_PLAYER_CHUNK_HOLD_SECONDS + 0.01;
    assert!(!harness.engine.floor_busy(harness.now));

    let messages = harness.send(EngineCommand::PlayerRecording {
        request_id: "rec-1".into(),
        wav_basename: "utterance.wav".into(),
        stt_backend: SttBackendKind::Cloud,
        position_m: PLAYER_SPAWN,
        spatial_seq: 1,
    });
    let (success, code, _) = result(&messages);
    assert!(!success);
    assert_eq!(code.as_deref(), Some("stt_unavailable"));
    assert!(speeches(&messages).is_empty(), "nothing was ever said");
    assert!(!harness.engine.floor_busy(harness.now));
}

#[test]
fn the_speech_router_carries_the_configured_grace_window() {
    // Clamped by the engine, carried by the router: the parked-recording window.
    let router = SpeechRouter::new(0.05);
    assert_eq!(router.stt_stream_grace_seconds(), 0.05);
    assert_eq!(router.active_stream_count(), 0);
    assert_eq!(router.parked_count(), 0);
}

/// The event id of the one speech message in `messages`.
fn speech_event_id(messages: &[EngineMessage]) -> SpeechEventId {
    match speeches(messages).first().expect("a speech was emitted") {
        EngineMessage::Speech { event_id, .. } => event_id.clone(),
        _ => unreachable!(),
    }
}
