//! Actual CityPlugin workloads at the existing ordinary HostCaptureSet.
use super::*;
use cathedral_sim::checkpoint::{
    CheckpointBudget, Cohort,
    complete::{self, CheckpointProfile, CompleteCheckpointInput},
};
use std::time::Instant;
#[derive(Resource)]
struct CompleteProbe {
    pending: bool,
    active: bool,
    samples: usize,
    profile: CheckpointProfile,
    output: Option<String>,
    report: Option<serde_json::Value>,
}
fn inspect_complete(world: &mut World) {
    let Some(p) = world.get_resource::<CompleteProbe>() else {
        return;
    };
    if !p.pending {
        return;
    }
    let (samples, profile, output, active) = (p.samples, p.profile, p.output.clone(), p.active);
    let source = HostObservation::new(world).unwrap();
    let before = super::tests_public::simulation_stamp(world);
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let mut phases: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::with_capacity(samples));
    let mut costs = Vec::with_capacity(samples);
    let mut save_stages = Vec::with_capacity(samples);
    let mut load_stages = Vec::with_capacity(samples);
    let mut digest = None;
    for index in 0..samples {
        let start = Instant::now();
        let mut stages = Vec::with_capacity(8);
        let mut previous = start;
        let save = capture_complete_observed(
            world,
            profile,
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
            &mut |stage| {
                let now = Instant::now();
                stages.push((stage, now.duration_since(previous).as_secs_f64() * 1e6));
                previous = now;
            },
        )
        .unwrap();
        save_stages.push(stages);
        phases[0].push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        let input = CompleteCheckpointInput::copy_from(
            save.value().bytes(),
            budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
        )
        .unwrap();
        phases[1].push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        let mut stages = Vec::with_capacity(8);
        let mut previous = start;
        let load = validate_complete_observed(world, input, &mut |stage| {
            let now = Instant::now();
            stages.push((stage, now.duration_since(previous).as_secs_f64() * 1e6));
            previous = now;
        })
        .unwrap();
        load_stages.push(stages);
        phases[2].push(start.elapsed().as_secs_f64() * 1e6);
        assert_eq!(load.value().bytes(), save.value().bytes());
        let mut h = DefinitionHasher::new(b"complete-probe-exact-bytes");
        h.bytes(save.value().bytes());
        let h = h.finish();
        assert!(digest.is_none_or(|prior| prior == h));
        digest = Some(h);
        if index == 0 {
            if let Some(path) = &output {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .unwrap();
                f.write_all(save.value().bytes()).unwrap();
            }
        }
        costs.push(save.value().cost());
        let start = Instant::now();
        drop(load);
        drop(save);
        phases[3].push(start.elapsed().as_secs_f64() * 1e6);
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
    assert_eq!(super::tests_public::simulation_stamp(world), before);
    let characters = source.local().unwrap().world().unwrap().characters.len();
    let placed = source
        .local()
        .unwrap()
        .world()
        .unwrap()
        .characters
        .values()
        .filter(|c| c.lore().is_some_and(|l| l.generated))
        .count();
    let mut readable = std::collections::BTreeMap::<&str, usize>::new();
    source
        .records(&mut |row| {
            *readable.entry(row_kind(&row)).or_default() += 1;
            Ok(())
        })
        .unwrap();
    if active {
        for (kind, count) in [
            ("bubble", 1),
            ("subtitle", 1),
            ("player_receipt", 1),
            ("unread_speech", 4),
            ("unread_intent", 1),
            ("pending_command", 1),
            ("cooldown", 1),
            ("well_draw", 1),
        ] {
            assert_eq!(
                readable.get(kind),
                Some(&count),
                "actual rich workload {kind}"
            );
        }
    }
    let requested = world
        .resource::<smart_actors::SmartActorsConfig>()
        .extra_ambient_npcs;
    assert_eq!(placed, requested as usize);
    let collision = source.resource::<CollisionWorld>().unwrap();
    let vermin = source
        .optional_component::<crate::city::vermin::Vermin>()
        .unwrap();
    let mut report = serde_json::json!({"schema":1,"scenario":"actual-complete-host-boundary-v1","profile":profile,"active":active,"samples":samples,"characters":characters,"placed":placed,"running_reservation_bytes":running.bytes(),"shared_peak_bytes":budget.peak_retained_bytes(),"costs":costs,"phase_microseconds":{"complete_capture":phases[0],"input_copy":phases[1],"complete_load":phases[2],"candidate_disposal":phases[3]},"save_stages":save_stages,"load_stages":load_stages,"no_detached_full_world_copy":true,"extraction_staging":{"borrowed_host_index_bytes":8*1024*1024,"host_sort_scratch_upper_bytes":8*1024*1024,"ledger_sublease_bytes":cathedral_sim::receipts::CommandLedgerDtoV1::WORKING_BYTES,"operations_sublease_bytes":cathedral_sim::operations::OperationKernelDtoV1::WORKING_BYTES},"counts":{"entities":world.entities().len(),"characters":characters,"collision_boxes":collision.boxes.len(),"collision_prisms":collision.convex_prisms.len(),"dynamic_barriers":world.iter_entities().filter(|e|e.contains::<DynamicBarrier>()).count(),"cut_margin":world.contains_resource::<crate::city::CutMarginProfile>(),"vermin_colonies":vermin.map_or(0,|v|v.colonies.len()),"rats":vermin.map_or(0,|v|v.colonies.iter().map(|c|c.rats.len()+c.boil_rats.len()).sum::<usize>()),"rows":readable.values().sum::<usize>()},"placement":{"requested":requested,"placed":placed,"unplaced":0},"readable_counts":readable,"host_image":identity::image_identity(),"ordinary_boundary_unchanged":true});
    let engine = source.local().unwrap().checkpoint_engine().unwrap();
    let world_nav = engine.world().nav.as_deref();
    let engine_nav = engine.config().nav.as_deref();
    let shared_graph = match (&engine.world().nav, &engine.config().nav) {
        (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
        _ => false,
    };
    let mut navs = Vec::<&cathedral_sim::NavData>::new();
    if let Some(nav) = world_nav {
        navs.push(nav);
    }
    if let Some(nav) = engine_nav {
        if !shared_graph {
            navs.push(nav);
        }
    }
    if let Some(v) = vermin {
        navs.push(&v.nav);
    }
    let mut nav_bytes = 0usize;
    for (i, nav) in navs.iter().enumerate() {
        let n = nav.checkpoint_storage_inventory();
        nav_bytes += n.graph_and_indexes_bytes;
        if !navs[..i]
            .iter()
            .any(|prior| nav.checkpoint_shares_distance_cache(prior))
        {
            nav_bytes += n.cache_maximum_bytes;
        }
    }
    let collision_bytes = std::mem::size_of_val(collision)
        + collision.boxes.capacity() * std::mem::size_of::<crate::controller::SolidBox>()
        + collision.convex_prisms.capacity()
            * std::mem::size_of::<crate::controller::SolidConvexPrism>()
        + 64
        + collision
            .convex_prisms
            .iter()
            .map(|p| {
                p.planes.len() * std::mem::size_of::<crate::controller::PrismPlane>()
                    + p.footprint.len() * std::mem::size_of::<Vec2>()
                    + 64
            })
            .sum::<usize>();
    let vermin_bytes = vermin.map_or(0, |v| {
        std::mem::size_of_val(v)
            + v.colonies.capacity() * std::mem::size_of::<crate::city::vermin::Colony>()
            + 32
            + v.colonies
                .iter()
                .map(|c| {
                    (c.rats.capacity() + c.boil_rats.capacity())
                        * std::mem::size_of::<crate::city::vermin::Rat>()
                        + 64
                        + c.rats
                            .iter()
                            .chain(c.boil_rats.iter())
                            .map(|r| {
                                r.legs.capacity() * std::mem::size_of::<crate::city::vermin::Leg>()
                                    + 32
                            })
                            .sum::<usize>()
                })
                .sum::<usize>()
    });
    report["running_inventory"] = serde_json::json!({
        "world_nav":world_nav.map(|n|n.checkpoint_storage_inventory()),
        "engine_nav":engine_nav.map(|n|n.checkpoint_storage_inventory()),
        "world_engine_share_graph_arc":shared_graph,
        "vermin_nav":vermin.map(|v|v.nav.checkpoint_storage_inventory()),
        "vermin_shares_world_cache":vermin.zip(world_nav).is_some_and(|(v,n)|v.nav.checkpoint_shares_distance_cache(n)),
        "nav_distinct_graphs_and_full_caches_upper_bytes":nav_bytes,
        "collision_retained_upper_bytes":collision_bytes,
        "vermin_excluding_nav_retained_upper_bytes":vermin_bytes,
        "cut_margin_retained_upper_bytes":world.get_resource::<crate::city::CutMarginProfile>().map(|p|p.checkpoint_storage_bytes()),
        "host_transport":source.local().unwrap().checkpoint_storage_inventory(),
        "render_ecs_assets_services_and_runtime_stacks_included":false,
    });
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
    let mut p = world.resource_mut::<CompleteProbe>();
    p.pending = false;
    p.report = Some(report);
}
fn run(extra: u32, samples: usize, active: bool, output: Option<String>) -> serde_json::Value {
    let mut app = super::tests::fixture(true, extra);
    app.add_systems(PostUpdate, inspect_complete.in_set(HostCaptureSet));
    if active {
        super::tests::prepare_readable_workload(&mut app);
    }
    app.insert_resource(CompleteProbe {
        pending: true,
        active,
        samples,
        profile: if extra == 0 {
            CheckpointProfile::Authored
        } else {
            CheckpointProfile::Populated
        },
        output,
        report: None,
    });
    app.update();
    app.world_mut()
        .resource_mut::<CompleteProbe>()
        .report
        .take()
        .unwrap()
}
#[test]
fn actual_rich_authored_complete_boundary() {
    let report = run(0, 1, true, None);
    println!("complete authored {report}");
}
#[test]
fn actual_rich_populated_complete_boundary() {
    let report = run(2000, 1, true, None);
    println!("complete populated {report}");
}
#[test]
#[ignore = "serial actual complete authored/populated cost and fixture probe"]
fn m2a16_complete_probe() {
    let extra = match std::env::var("ALIBI_COMPLETE_MODE")
        .as_deref()
        .unwrap_or("authored")
    {
        "authored" => 0,
        "populated" => 2000,
        _ => panic!("invalid mode"),
    };
    let samples = std::env::var("ALIBI_COMPLETE_SAMPLES")
        .unwrap_or("1".into())
        .parse()
        .unwrap();
    assert!((1..=1000).contains(&samples));
    if std::env::var("ALIBI_COMPLETE_FIXTURE").is_ok() {
        assert!(
            std::env::var("ALIBI_COMPLETE_WORLD_ID").is_ok(),
            "fixtures require an explicit initial lineage before Engine construction"
        );
    }
    let active = std::env::var("ALIBI_COMPLETE_INITIAL").is_err();
    let report = run(
        extra,
        samples,
        active,
        std::env::var("ALIBI_COMPLETE_FIXTURE").ok(),
    );
    let path = std::env::var("ALIBI_COMPLETE_REPORT").unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

fn row_kind(row: &RecordRef<'_>) -> &'static str {
    use RecordV1::*;
    match row {
        Draft { .. } => "draft",
        SelectedItem { .. } => "selected_item",
        Custody { .. } => "custody",
        Holder { .. } => "holder",
        Notice { .. } => "notice",
        Journal { .. } => "journal",
        JournalStanding { .. } => "journal_standing",
        Hud { .. } => "hud",
        ActiveOffer { .. } => "active_offer",
        DismissedBroadcast { .. } => "dismissed_broadcast",
        PendingCommand { .. } => "pending_command",
        InventoryContext { .. } => "inventory_context",
        ChalkHold { .. } => "chalk_hold",
        ChalkPen { .. } => "chalk_pen",
        ChalkAnchor { .. } => "chalk_anchor",
        Cooldown { .. } => "cooldown",
        WellDraw { .. } => "well_draw",
        Work { .. } => "work",
        UnreadBell { .. } => "unread_bell",
        UnreadIntent { .. } => "unread_intent",
        UnreadSpeech { .. } => "unread_speech",
        Subtitle { .. } => "subtitle",
        Bubble { .. } => "bubble",
        PlayerReceipt { .. } => "player_receipt",
    }
}
#[derive(Resource)]
struct FixtureCheck {
    path: std::path::PathBuf,
    result: Option<bool>,
}
fn inspect_fixture(world: &mut World) {
    let Some(p) = world.get_resource::<FixtureCheck>() else {
        return;
    };
    if p.result.is_some() {
        return;
    }
    let path = p.path.clone();
    let before = super::tests_public::simulation_stamp(world);
    let result = verify_file(world, &path);
    assert_eq!(super::tests_public::simulation_stamp(world), before);
    world.resource_mut::<FixtureCheck>().result = Some(result);
}
fn verify_file(world: &World, path: &std::path::Path) -> bool {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let mut file = std::fs::File::open(path).unwrap();
    let len = usize::try_from(file.metadata().unwrap().len()).unwrap();
    assert!(len <= cathedral_sim::checkpoint::POPULATED_PAYLOAD_BYTES);
    let reservation = budget.reserve(Cohort::LoadCandidate, len + 4096).unwrap();
    let mut bytes = vec![0; len];
    file.read_exact(&mut bytes).unwrap();
    assert_eq!(file.read(&mut [0]).unwrap(), 0);
    // Trusted fixture header only; the production loader still validates the
    // original entire input independently with its strict admitted parser.
    #[derive(serde::Deserialize)]
    struct Header {
        manifest: Image,
    }
    #[derive(serde::Deserialize)]
    struct Image {
        host_image: [u8; 32],
    }
    let saved = serde_json::from_slice::<Header>(&bytes)
        .unwrap()
        .manifest
        .host_image;
    let raw_hash: [u8; 32] = Sha256::digest(&bytes).into();
    let same_image = saved == identity::image_identity().unwrap().digest;
    let input = CompleteCheckpointInput::from_owned(bytes, reservation).unwrap();
    let loaded = validate_complete(world, input);
    if same_image {
        let loaded = loaded.expect("same final image must validate supported fixture");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(loaded.value().bytes())),
            raw_hash
        );
        println!(
            "complete fixture {}: SAME_IMAGE_EXACT_BYTES_ACCEPTED",
            path.display()
        );
        drop(loaded);
    } else {
        let error = loaded.unwrap_err();
        assert_eq!(error.owner, "complete");
        assert_eq!(error.reason, "exact running host image mismatch");
        println!(
            "complete fixture {}: EXPECTED_INCOMPATIBLE_HOST_IMAGE",
            path.display()
        );
    }
    assert_eq!(budget.retained_bytes(), running.bytes());
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
    same_image
}
fn verify_at_boundary(path: std::path::PathBuf) -> bool {
    let mut app = super::tests::fixture(true, 0);
    app.add_systems(PostUpdate, inspect_fixture.in_set(HostCaptureSet));
    app.insert_resource(FixtureCheck { path, result: None });
    app.update();
    app.world().resource::<FixtureCheck>().result.unwrap()
}
#[test]
fn supported_complete_fixture_image_contract() {
    let directory = std::path::Path::new("src/host_checkpoint/fixtures/complete-v1");
    if !directory.exists() {
        println!(
            "PRE_FIXTURE_STAGE: complete release fixtures are generated only after final executable freeze"
        );
        return;
    }
    for name in ["initial.json", "active.json"] {
        verify_at_boundary(directory.join(name));
    }
}
#[test]
#[ignore = "explicit same-image or incompatible-image archived fixture verification"]
fn m2a16_verify_complete_fixture() {
    let path = std::env::var("ALIBI_COMPLETE_FIXTURE_IN").unwrap();
    let same = verify_at_boundary(path.into());
    assert_eq!(
        same,
        std::env::var("ALIBI_COMPLETE_EXPECT_IMAGE_MISMATCH").is_err()
    );
}
