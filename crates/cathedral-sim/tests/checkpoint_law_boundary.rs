//! Independent law-owner boundaries. Candidates remain unadopted.
mod prompt_support;

use cathedral_sim::{
    ItemId, Office, PlaceId, Vec3, World, WorldTime,
    checkpoint::{Admitted, CheckpointBudget, Cohort},
    custody::{Confinement, Station},
    lore::{LoreProfile, PlanningWard},
    notices::{
        self, Rung,
        checkpoint::{LawCheckpointContext, WorldLawCandidate, WorldLawDtoV1},
    },
    timeline::LogicalTime,
    world::WorldBackboneDtoV1,
};
use prompt_support::actor;

fn world() -> World {
    let mut world = prompt_support::seed_world();
    world.current_time = Some(WorldTime::from_game_days(2.0));
    world.items.clear();
    for character in world.characters.values_mut() {
        character.sheet.holds.clear();
        character.state.holds.clear();
        character.state.position_m = Vec3::ZERO;
    }
    world
        .characters
        .get_mut(&actor("cb947"))
        .unwrap()
        .sheet
        .lore = Some(LoreProfile {
        significance: cathedral_sim::Significance::Minor,
        planning_ward: PlanningWard::Fabric,
        age: 30,
        gender: "m".into(),
        occupation_id: Some("bailiff_and_gaoler".into()),
        occupation_display: None,
        title: None,
        rank: None,
        faction_role: None,
        illegal_activity: None,
        district: "Fabric".into(),
        father: None,
        mother: None,
        children: Vec::new(),
        circumstances: Vec::new(),
        conditions: Vec::new(),
        home: None,
        home_point_m: None,
        core_character_description: String::new(),
        extended_character_description: String::new(),
        curiosity: None,
        generated: false,
        generated_routine: None,
    });
    world
}

fn now() -> LogicalTime {
    LogicalTime::new(10.0).unwrap()
}

fn raise(world: &mut World, accused: &str, date: Option<f64>) -> u64 {
    world
        .notices
        .raise(
            "a person at the quay".into(),
            "a missing purse".into(),
            Some("the old quay".into()),
            None,
            date,
            actor("departed_raiser"),
            Some(actor(accused)),
            Some(actor("departed_wronged")),
            Some(ItemId::from_raw("departed_item")),
        )
        .unwrap()
}

fn station() -> Station {
    Station {
        place_id: PlaceId::from_raw("former_station"),
        name: "The former station".into(),
        point: Vec3::new(40.0, 0.0, 10.0),
        stone_house: true,
    }
}

fn save(world: &World, budget: &CheckpointBudget) -> Admitted<Vec<u8>> {
    world
        .export_law_checkpoint(now(), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}

fn load(
    bytes: &[u8],
    context: LawCheckpointContext<'_>,
    budget: &CheckpointBudget,
) -> Admitted<WorldLawCandidate> {
    WorldLawDtoV1::decode(
        bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap()
}

#[test]
fn committed_history_survives_expiry_and_departure_but_a_missing_active_officer_refuses() {
    let mut world = world();
    let notice = raise(&mut world, "player", Some(-30.0));
    world.custody.seize(
        actor("player"),
        actor("cb947"),
        Some(notice),
        station(),
        1.0,
    );
    world.custody.commit(&actor("player"), 2.0).unwrap();
    assert!(world.custody.forget(&actor("cb947")).is_empty());
    world.characters.remove(&actor("cb947"));
    world.notices.expire(2.0);
    assert!(world.notices.get(notice).is_none());
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let context = LawCheckpointContext::from_world(&world, now());
    let candidate = load(saved.value(), context, &budget);
    let record = candidate.value().custody().get(&actor("player")).unwrap();
    assert_eq!(record.state, Confinement::Committed);
    assert_eq!(record.officer, Some(actor("cb947")));
    assert_eq!(record.notice_id, Some(notice));
    assert_eq!(record.station, station());
    assert_eq!(
        candidate
            .value()
            .history_counts(context)
            .historical_notice_links,
        1
    );
    assert_eq!(
        candidate
            .value()
            .history_counts(context)
            .historical_officers,
        1
    );
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);

    // Active custody needs an officer who can actually continue the escort.
    world.custody.release(&actor("player"));
    world
        .custody
        .seize(actor("player"), actor("cb947"), None, station(), 3.0);
    assert!(
        world
            .export_law_checkpoint(now(), budget.reserve(Cohort::SavePayload, 4096).unwrap())
            .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn restored_service_does_not_repeat_until_each_new_rung() {
    let mut world = world();
    let id = raise(&mut world, "player", Some(2.0));
    let officer = actor("cb947");
    let initial = world.characters[&officer].inbox().len();
    notices::confront(&mut world);
    assert_eq!(world.characters[&officer].inbox().len(), initial + 1);
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        LawCheckpointContext::from_world(&world, now()),
        &budget,
    );
    // Only a test-local component copy is continued; no partial city adoption API exists.
    world.notices = candidate.value().notices().clone();
    notices::confront(&mut world);
    assert_eq!(world.characters[&officer].inbox().len(), initial + 1);
    assert!(
        world
            .notices
            .summon(id, officer.clone(), Office::Watch, Some(3.0))
    );
    notices::confront(&mut world);
    notices::confront(&mut world);
    assert_eq!(world.characters[&officer].inbox().len(), initial + 2);
    assert!(world.notices.issue_warrants(2.99).is_empty());
    assert_eq!(world.notices.issue_warrants(3.0), vec![id]);
    notices::confront(&mut world);
    notices::confront(&mut world);
    assert_eq!(world.characters[&officer].inbox().len(), initial + 3);
    assert_eq!(world.notices.get(id).unwrap().rung(), Rung::Warranted);
}

#[test]
fn undated_obligations_keep_absent_historical_links_and_never_acquire_a_deadline() {
    let mut world = world();
    let id = raise(&mut world, "departed_accused", None);
    assert!(
        world
            .notices
            .summon(id, actor("departed_summoner"), Office::Watch, None)
    );
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        LawCheckpointContext::from_world(&world, now()),
        &budget,
    );
    let mut notices = candidate.value().notices().clone();
    assert_eq!(notices, world.notices);
    assert!(notices.issue_warrants(1_000.0).is_empty());
    notices.expire(1_000.0);
    let notice = notices.get(id).unwrap();
    assert_eq!(notice.rung(), Rung::Summoned);
    assert_eq!(notice.summons.as_ref().unwrap().due_game_days, None);
    assert_eq!(notice.taken, Some(ItemId::from_raw("departed_item")));
}

#[test]
fn duplicate_records_refuse_and_raw_padding_stays_charged_through_candidate_validation() {
    let mut world = world();
    raise(&mut world, "player", Some(2.0));
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let save_charge = budget.retained_bytes();
    let context = LawCheckpointContext::from_world(&world, now());
    let mut wire: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    let live = wire["notices"]["live"].as_array_mut().unwrap();
    live.push(live[0].clone());
    let bytes = serde_json::to_vec(&wire).unwrap();
    assert!(
        WorldLawDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            context
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), save_charge);
    let padding = 1024 * 1024;
    let mut padded = vec![b' '; padding];
    padded.extend_from_slice(saved.value());
    let decoded = WorldLawDtoV1::decode(
        &padded,
        budget
            .reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    let canonical_peak = decoded.value().cost().unwrap().peak_bytes;
    let retained = budget.retained_bytes();
    assert!(retained - save_charge >= canonical_peak + 3 * padding);
    let candidate = decoded.into_candidate(context).unwrap();
    assert_eq!(budget.retained_bytes(), retained);
    assert_eq!(candidate.value().notices(), &world.notices);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn saved_calendar_mismatch_refuses_then_law_borrows_an_unadopted_backbone() {
    let mut world = world();
    raise(&mut world, "player", Some(2.0));
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let save_charge = budget.retained_bytes();
    world.current_time = Some(WorldTime::from_game_days(2.5));
    assert!(
        WorldLawDtoV1::decode(
            saved.value(),
            budget
                .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
                .unwrap(),
            LawCheckpointContext::from_world(&world, now())
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), save_charge);
    world.current_time = Some(WorldTime::from_game_days(2.0));
    // Separate budgets prove borrowing, not complete-envelope coexistence admission.
    let backbone_budget = CheckpointBudget::default();
    let bytes = world
        .export_backbone_checkpoint(backbone_budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let backbone = WorldBackboneDtoV1::decode(
        bytes.value(),
        backbone_budget
            .reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
            .unwrap(),
        &world.item_catalog,
        &world.command_ledger,
    )
    .unwrap()
    .into_candidate(&world.item_catalog, &world.command_ledger)
    .unwrap();
    drop(bytes);
    let context = LawCheckpointContext::from_backbone(backbone.value(), now());
    let candidate = load(saved.value(), context, &budget);
    assert_eq!(candidate.value().notices(), &world.notices);
    drop((candidate, backbone, saved));
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(backbone_budget.retained_bytes(), 0);
}
