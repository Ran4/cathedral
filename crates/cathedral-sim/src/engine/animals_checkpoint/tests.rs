use super::*;
use crate::{
    NullSight, NullTranscription, NullTts,
    checkpoint::{CheckpointBudget, Cohort},
};
use serde_json::json;

struct Unavailable;
impl Cognition for Unavailable {
    fn request(
        &mut self,
        _: String,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        Err(crate::CognitionBusy)
    }
}
fn engine() -> Engine {
    Engine::new(
        EngineConfig {
            fake_mode: true,
            nav: Some(owner::tests::nav()),
            clock: WorldClock::new(60.0, Office::Dayspring, 2, 0.05),
            ..Default::default()
        },
        &crate::WorldSeed::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/demo_seed.json"
        )))
        .unwrap(),
        AreaMap::from_json_str(include_str!("../../../../../assets/world/areas.json")).unwrap(),
        SoundCatalog::from_toml_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sounds/catalog.toml"
        )))
        .unwrap(),
        PromptEnv::new(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/turn.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/night.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/strings.toml"
            )),
        )
        .unwrap(),
        Box::new(Unavailable),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::default(),
        (Vec3::new(0.0, 0.91, 111.0), 0.0),
        0,
        0.0,
    )
    .unwrap()
}

fn logical(n: f64) -> LogicalTime {
    LogicalTime::new(n).unwrap()
}
fn bytes(e: &Engine, now: f64) -> Admitted<Vec<u8>> {
    let b = CheckpointBudget::default();
    e.export_animals_checkpoint(logical(now), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn candidate(e: &Engine, now: f64) -> Admitted<EngineAnimalsCandidate> {
    let b = CheckpointBudget::default();
    let saved = bytes(e, now);
    let c = AnimalsCheckpointContext::from_world(&e.world, logical(now))
        .with_engine_nav(e.config.nav.as_deref());
    EngineAnimalsDtoV1::decode(
        saved.value(),
        b.reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap()
}
fn install(d: &EngineAnimalsCandidate, e: &mut Engine) {
    // Independently constructed dependencies survive. Scramble every covered
    // mutable field and both immutable resolver slots before private install.
    let world_nav = e.world.nav.clone();
    let config_nav = e.config.nav.clone();
    e.world.dogs = vec![owner::tests::dog("wrong")];
    e.world.nav = None;
    e.config.nav = None;
    e.config.player_id = ActorId::from_raw("wrong");
    e.movement_now = 999.25;
    e.dogs_published = !d.data.dogs_published;
    e.world.nav = world_nav;
    e.config.nav = config_nav;
    e.world.dogs = d.data.world.dogs.clone();
    e.config.player_id = d.data.player_id.clone();
    e.movement_now = d.data.movement_now.seconds();
    e.dogs_published = d.data.dogs_published;
    assert_eq!(
        bytes(e, d.data.boundary.seconds()).value(),
        &serde_json::to_vec(&d.data).unwrap(),
        "immediate canonical equality before ordinary poll"
    );
}
fn prefix(e: &mut Engine) {
    e.world.dogs = owner::tests::active().dogs;
    e.tick_movement(0.075, &mut Vec::new());
}
fn dog_messages(out: &[EngineMessage]) -> Vec<Vec<crate::dogs::DogView>> {
    out.iter()
        .filter_map(|m| match m {
            EngineMessage::Dogs { dogs } => Some(dogs.clone()),
            _ => None,
        })
        .collect()
}
#[test]
fn checkpoint_animals_engine_shared_slice_and_publication_continue() {
    let mut control = engine();
    let mut resumed = engine();
    prefix(&mut control);
    prefix(&mut resumed);
    assert_eq!(control.movement_now, 0.05);
    let saved = candidate(&control, 0.075);
    install(saved.value(), &mut resumed);
    let mut pubs = 0;
    for frame in 1..=800 {
        let now = 0.075 + f64::from(frame) * 0.025;
        let a = control.poll(now, vec![]);
        let b = resumed.poll(now, vec![]);
        assert_eq!(dog_messages(&a), dog_messages(&b));
        pubs += dog_messages(&a).len();
        assert_eq!(control.world.dogs, resumed.world.dogs);
        assert_eq!(
            control.movement_now.to_bits(),
            resumed.movement_now.to_bits()
        );
        assert_eq!(control.world.world_revision, resumed.world.world_revision);
        assert_eq!(bytes(&control, now).value(), bytes(&resumed, now).value());
    }
    assert!(pubs > 1);
}
#[test]
fn checkpoint_animals_initial_rest_stop_and_empty_publication_edges() {
    struct NoDogCognition;
    impl Cognition for NoDogCognition {
        fn request(
            &mut self,
            _: String,
        ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
            panic!("dog movement/publication may not request cognition")
        }
    }
    for published in [false, true] {
        for empty in [false, true] {
            let mut control = engine();
            let mut resumed = engine();
            for e in [&mut control, &mut resumed] {
                e.cognition = Box::new(NoDogCognition);
                e.world.dogs = if empty {
                    vec![]
                } else {
                    vec![owner::tests::dog("rest")]
                };
                e.dogs_published = published;
            }
            let d = candidate(&control, 0.0);
            install(d.value(), &mut resumed);
            let rev = control.world.world_revision;
            let mut a = vec![];
            let mut b = vec![];
            control.tick_movement(0.0, &mut a);
            resumed.tick_movement(0.0, &mut b);
            assert_eq!(dog_messages(&a), dog_messages(&b));
            assert_eq!(dog_messages(&a).len(), usize::from(!empty && !published));
            assert_eq!(control.world.world_revision, rev);
            assert_eq!(resumed.world.world_revision, rev);
            let mut quiet = vec![];
            resumed.tick_movement(0.01, &mut quiet);
            assert!(dog_messages(&quiet).is_empty());
        }
    }
    let mut control = engine();
    let mut resumed = engine();
    for e in [&mut control, &mut resumed] {
        let mut d = owner::tests::dog("arrival");
        d.speed = 0.08;
        e.world.dogs = vec![d];
        e.dogs_published = true;
    }
    let d = candidate(&control, 0.0);
    install(d.value(), &mut resumed);
    for (now, expected) in [(0.05, 1), (0.10, 0)] {
        let mut a = vec![];
        let mut b = vec![];
        control.tick_movement(now, &mut a);
        resumed.tick_movement(now, &mut b);
        assert_eq!(dog_messages(&a), dog_messages(&b));
        assert_eq!(dog_messages(&a).len(), expected);
    }
}
#[test]
fn checkpoint_animals_exact_two_nav_bindings_and_backward_or_inactive_anchor() {
    let mut e = engine();
    e.world.nav = None;
    e.movement_now = 8.123456789012345;
    let d = candidate(&e, 3.0);
    assert_eq!(
        d.value().movement_now().seconds().to_bits(),
        e.movement_now.to_bits()
    );
    let saved = bytes(&e, 3.0);
    let b = CheckpointBudget::default();
    let wrong = AnimalsCheckpointContext::from_world(&e.world, logical(3.0));
    assert!(
        EngineAnimalsDtoV1::decode(
            saved.value(),
            b.reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
                .unwrap(),
            wrong
        )
        .is_err()
    );
    assert_eq!(b.retained_bytes(), 0);
    e.config.nav = None;
    e.movement_now = 0.0;
    let d = candidate(&e, 100.0);
    assert_eq!(d.value().movement_now().seconds(), 0.0);
    // Retained living pack and false publication are valid with no nav; it is
    // the ordinary no-nav early-out that decides future behavior.
    assert!(!d.value().dogs().is_empty());
    assert!(!d.value().dogs_published());
    let mut out = vec![];
    e.tick_movement(100.0, &mut out);
    assert!(out.is_empty());
    assert_eq!(e.movement_now, 0.0);
}
#[test]
fn checkpoint_animals_engine_refusals_raw_admission_and_reference_validation() {
    let e = engine();
    let original: serde_json::Value = serde_json::from_slice(bytes(&e, 0.0).value()).unwrap();
    for key in original.as_object().unwrap().keys() {
        let mut bad = original.clone();
        bad.as_object_mut().unwrap().remove(key);
        refuse(&e, serde_json::to_vec(&bad).unwrap());
    }
    for (key, value) in [
        ("version", json!(7)),
        ("unknown", json!(0)),
        ("player_id", json!("absent")),
        ("config_nav", json!(null)),
        ("movement_now", json!(-1.0)),
        ("dogs_published", json!(null)),
    ] {
        let mut bad = original.clone();
        bad[key] = value;
        refuse(&e, serde_json::to_vec(&bad).unwrap());
    }
    let saved = String::from_utf8(bytes(&e, 0.0).value().clone()).unwrap();
    refuse(
        &e,
        saved
            .replacen(
                "\"dogs_published\":",
                "\"dogs_published\":false,\"dogs_published\":",
                1,
            )
            .into_bytes(),
    );
    let padded = format!(
        "{}{}",
        " ".repeat(8192),
        saved.replace("Bracken", "\\u0042racken")
    );
    let b = CheckpointBudget::default();
    let c = AnimalsCheckpointContext::from_world(&e.world, logical(0.0));
    let decoded = EngineAnimalsDtoV1::decode(
        padded.as_bytes(),
        b.reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap();
    let raw_charge = b.retained_bytes();
    let canonical = decoded.value().cost().unwrap().peak_bytes;
    assert!(raw_charge > canonical);
    let d = decoded.into_candidate(c).unwrap();
    assert_eq!(b.retained_bytes(), raw_charge);
    drop(d);
    assert_eq!(b.retained_bytes(), 0);
}
fn refuse(e: &Engine, bad: Vec<u8>) {
    let b = CheckpointBudget::default();
    assert!(
        EngineAnimalsDtoV1::decode(
            &bad,
            b.reserve(Cohort::LoadCandidate, bad.len() + 4096).unwrap(),
            AnimalsCheckpointContext::from_world(&e.world, logical(0.0))
        )
        .is_err()
    );
    assert_eq!(b.retained_bytes(), 0);
}
#[test]
fn checkpoint_animals_engine_golden() {
    let mut e = engine();
    prefix(&mut e);
    assert_eq!(
        bytes(&e, 0.075).value().as_slice(),
        include_bytes!("../../../tests/fixtures/checkpoint_v1/engine_animals.json")
    );
}
#[test]
#[ignore = "create only the new M2a8 component fixture"]
fn create_animals_engine_fixture() {
    let mut e = engine();
    prefix(&mut e);
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkpoint_v1/engine_animals.json");
    assert!(!p.exists() || p.metadata().unwrap().len() == 0);
    std::fs::write(p, bytes(&e, 0.075).value()).unwrap();
}
#[test]
#[ignore = "closed owner layout diagnostic"]
fn checkpoint_animals_engine_layout() {
    println!(
        "EngineDto={} EngineCandidate={} LogicalTime={}",
        std::mem::size_of::<EngineAnimalsDtoV1>(),
        std::mem::size_of::<EngineAnimalsCandidate>(),
        std::mem::size_of::<LogicalTime>()
    );
}
#[test]
fn checkpoint_animals_human_and_dog_share_the_restored_fractional_slice() {
    let mut control = engine();
    let mut resumed = engine();
    let walking = ActorId::from_raw("walking");
    for e in [&mut control, &mut resumed] {
        let mut human = crate::notices::checkpoint::tests::person("walking", 0.0, None);
        human.state.position_m = Vec3::new(0.0, crate::WALK_Y, 0.0);
        human.state.facing_yaw = -std::f64::consts::FRAC_PI_2;
        human.state.movement = Some(crate::character::Movement {
            exact_local: false,
            path: vec![Vec3::new(20.0, crate::WALK_Y, 0.0)],
            speed: 0.4,
            gait_phase: 2.25,
            patrol: None,
            choke_wait: 0.0,
        });
        e.world.add_character(human);
        e.world.dogs = owner::tests::active().dogs;
        e.tick_movement(0.075, &mut Vec::new());
    }
    let d = candidate(&control, 0.075);
    install(d.value(), &mut resumed);
    let start = control.world.characters[&walking].position_m();
    for at in [0.09, 0.101, 0.124, 0.151, 0.201] {
        let mut a = vec![];
        let mut b = vec![];
        control.tick_movement(at, &mut a);
        resumed.tick_movement(at, &mut b);
        assert_eq!(dog_messages(&a), dog_messages(&b));
        assert_eq!(control.world.dogs, resumed.world.dogs);
        let human = &control.world.characters[&walking];
        let restored = &resumed.world.characters[&walking];
        assert_eq!(human.position_m(), restored.position_m());
        assert_eq!(human.state.movement, restored.state.movement);
        assert_eq!(
            control.movement_now.to_bits(),
            resumed.movement_now.to_bits()
        );
        if at == 0.09 {
            assert_eq!(
                human.position_m(),
                start,
                "fractional slice must not advance either owner"
            );
        }
    }
    assert!(
        control.world.characters[&walking]
            .position_m()
            .distance(start)
            > 0.0
    );
}
