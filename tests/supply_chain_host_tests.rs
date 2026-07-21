//! Host-side M5 acceptance: committed clock loading and Bevy cart projection.

use std::path::PathBuf;

use bevy::prelude::*;
use cathedral_sim::{
    ActorId as SimActorId, CartLoadKind, NavData, Office, PartyId, PartyPhase, Round, WorldClock,
    WorldConfig, build_world,
};

// These are the production host modules. The game is currently a binary crate,
// so the integration target supplies only the small parent modules their
// `super`/`crate` imports require.
mod actors {
    use bevy::prelude::*;

    #[derive(Component, Debug)]
    pub struct ActorView;
}

#[path = "../src/smart_actors/model.rs"]
#[allow(dead_code)]
mod model;
#[path = "../src/smart_actors/road_carts.rs"]
mod road_carts;

mod smart_actors {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Deserialize, Serialize)]
    #[serde(default)]
    pub struct ClockSettings {
        pub seconds_per_day: f64,
        pub start_office: String,
        pub start_day: i64,
        pub night_brightness: f64,
        pub ring_the_offices: bool,
    }

    impl Default for ClockSettings {
        fn default() -> Self {
            Self {
                seconds_per_day: 3_600.0,
                start_office: "dayspring".into(),
                start_day: 0,
                night_brightness: 0.05,
                ring_the_offices: true,
            }
        }
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    #[serde(default)]
    pub struct SmartActorsConfig {
        pub tts_backend: String,
        pub stt_backend: String,
        pub pause_microphone_during_npc_voice: bool,
        pub stt_streaming: bool,
        pub stt_trailing_silence_ms: u32,
        pub clock: ClockSettings,
    }

    impl Default for SmartActorsConfig {
        fn default() -> Self {
            Self {
                tts_backend: "local".into(),
                stt_backend: "cloud".into(),
                pause_microphone_during_npc_voice: true,
                stt_streaming: true,
                stt_trailing_silence_ms: 400,
                clock: ClockSettings::default(),
            }
        }
    }
}

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn committed_default_brede_snapshot() -> model::WorldSnapshot {
    let root = repo_root();
    let loaded = config::load_config_from_paths(
        root.join("tests/fixtures/m5-no-local-config.ron"),
        root.join("default_config.ron"),
    );
    let settings = &loaded.smart_actors.clock;
    assert_eq!(settings.start_day, 2);
    assert_eq!(settings.start_office, "dayspring");

    let office = Office::from_config_name(&settings.start_office)
        .expect("the committed start office is valid");
    let clock = WorldClock::new(
        settings.seconds_per_day,
        office,
        settings.start_day,
        settings.night_brightness,
    );
    let seed =
        cathedral_backends::world_data::load_world_seed(&root.join("assets"), &root.join("lore"))
            .expect("the shipped seed and lore load");
    let mut world = build_world(&seed, WorldConfig::default());
    let nav = NavData::from_parts(
        include_str!("../assets/world/navigation.json"),
        include_bytes!("../assets/world/navigation.bin"),
    )
    .expect("the committed navigation loads");
    let mut round = Round::new();
    let diagnostics = round.seed(&mut world, &nav, 0.0, &clock);
    assert!(
        diagnostics.iter().all(|line| !line.contains("road party")),
        "road party diagnostics: {diagnostics:?}"
    );

    let brede = PartyId::from_raw("brede_wool_gate");
    assert_eq!(round.party_state(&brede).unwrap().phase, PartyPhase::InCity);
    assert_eq!(round.party_state(&brede).unwrap().trip_number, 1);
    let carts = round.road_carts(&world);
    assert_eq!(carts.len(), 1);
    assert_eq!(carts[0].party_id, brede);
    assert_eq!(carts[0].leader_id, SimActorId::from_raw("rbrde"));
    assert_eq!(
        carts[0].load,
        vec![CartLoadKind::GrainSacks, CartLoadKind::WoolBales]
    );

    let mut snapshot = world.public_snapshot(&SimActorId::from_raw("player"));
    snapshot.road_carts = carts;
    model::WorldSnapshot::from(&snapshot)
}

fn cargo_counts(world: &mut World) -> (usize, usize, usize) {
    let mut query = world.query::<&road_carts::RoadCartCargo>();
    let mut grain = 0;
    let mut wool = 0;
    let mut cloth = 0;
    for cargo in query.iter(world) {
        match cargo.0 {
            CartLoadKind::GrainSacks => grain += 1,
            CartLoadKind::WoolBales => wool += 1,
            CartLoadKind::ClothBolts => cloth += 1,
        }
    }
    (grain, wool, cloth)
}

fn cart_state(world: &mut World) -> (usize, Vec3, usize) {
    let mut query = world.query::<(&road_carts::RoadCartView, &Transform, &Children)>();
    let carts: Vec<_> = query.iter(world).collect();
    assert!(carts.len() <= 1);
    carts
        .first()
        .map_or((0, Vec3::ZERO, 0), |(view, transform, children)| {
            assert_eq!(view.party_id, "brede_wool_gate");
            (1, transform.translation, children.len())
        })
}

#[test]
fn committed_default_enters_brede_and_cart_projection_tracks_load_pose_and_departure() {
    let mut snapshot = committed_default_brede_snapshot();
    let leader_id = model::ActorId("rbrde".into());

    let mut mirror = model::WorldMirror::default();
    mirror
        .replace_snapshot(snapshot.clone())
        .expect("the sim snapshot validates in the host mirror");

    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .insert_resource(mirror)
        .add_systems(Startup, road_carts::setup_road_cart_assets)
        .add_systems(Update, road_carts::reconcile_road_carts);
    let leader = app
        .world_mut()
        .spawn((
            leader_id,
            actors::ActorView,
            Transform::from_xyz(10.0, 5.0, 20.0),
        ))
        .id();

    app.update();
    assert_eq!(
        cart_state(app.world_mut()),
        (1, Vec3::new(10.0, 0.0, 22.7), 10)
    );
    assert_eq!(cargo_counts(app.world_mut()), (2, 1, 0));

    snapshot.world_revision += 1;
    snapshot.road_carts[0].load = vec![
        CartLoadKind::GrainSacks,
        CartLoadKind::WoolBales,
        CartLoadKind::ClothBolts,
    ];
    app.world_mut()
        .resource_mut::<model::WorldMirror>()
        .replace_snapshot(snapshot.clone())
        .unwrap();
    app.world_mut()
        .entity_mut(leader)
        .insert(Transform::from_xyz(30.0, 4.0, -8.0));
    app.update();
    assert_eq!(
        cart_state(app.world_mut()),
        (1, Vec3::new(30.0, 0.0, -5.3), 11)
    );
    assert_eq!(cargo_counts(app.world_mut()), (2, 1, 1));

    snapshot.world_revision += 1;
    snapshot.road_carts.clear();
    app.world_mut()
        .resource_mut::<model::WorldMirror>()
        .replace_snapshot(snapshot)
        .unwrap();
    app.update();
    assert_eq!(cart_state(app.world_mut()).0, 0);
    assert_eq!(cargo_counts(app.world_mut()), (0, 0, 0));
}
