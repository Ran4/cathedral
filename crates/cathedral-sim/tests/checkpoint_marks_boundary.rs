//! Independent marks-owner boundaries; candidates are never adopted as a city.
mod prompt_support;

use cathedral_sim::{
    Vec3, World, WorldTime,
    checkpoint::{Admitted, CheckpointBudget, Cohort},
    marks::{
        self, Mark, MarkAnchor, MarkCatalog, MarkKind,
        checkpoint::{MarksCheckpointContext, WorldMarksCandidate, WorldMarksDtoV1},
    },
    notices,
    timeline::LogicalTime,
    world::WorldBackboneDtoV1,
};
use prompt_support::actor;
use std::sync::Arc;

fn world() -> World {
    let mut world = prompt_support::seed_world();
    world.current_time = Some(WorldTime::from_game_days(2.0));
    world.places.add_home(&actor("cb947"), "Conny", Vec3::ZERO);
    world
}

fn now() -> LogicalTime {
    LogicalTime::new(10.0).unwrap()
}
fn anchor() -> MarkAnchor {
    MarkAnchor::Household(actor("cb947"))
}

fn draw(world: &mut World) -> cathedral_sim::ids::MarkId {
    marks::draw_or_refresh(
        world,
        MarkKind::ChalkCross,
        anchor(),
        Some(actor("departed_hand")),
        2.0,
    )
    .unwrap()
    .id
}

fn save(world: &World, budget: &CheckpointBudget) -> Admitted<Vec<u8>> {
    world
        .export_marks_checkpoint(now(), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}

fn load(
    bytes: &[u8],
    context: MarksCheckpointContext<'_>,
    budget: &CheckpointBudget,
) -> Admitted<WorldMarksCandidate> {
    WorldMarksDtoV1::decode(
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
fn orphan_marks_wait_for_the_saved_sweep_gate_before_disappearing() {
    let mut world = world();
    let live = draw(&mut world);
    let mut orphan = world.marks.get(live).unwrap().clone();
    orphan.anchor = MarkAnchor::Household(actor("departed_home"));
    orphan.about = Some(actor("departed_home"));
    let (house, _) = world.marks.insert(orphan);
    let (place, _) = world.marks.insert(Mark {
        kind: MarkKind::WellTally,
        anchor: MarkAnchor::Place("The former well".into()),
        about: None,
        author: Some(actor("departed_hand")),
        drawn_game_days: 2.0,
        last_decayed_game_days: 2.0,
        strength: 0.75,
        strokes: 4,
    });
    world.marks.rewind_sweep_clock(2.0);
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        MarksCheckpointContext::from_world(&world, now()),
        &budget,
    );
    assert_eq!(candidate.value().marks(), &world.marks);
    // Continue a test-local owner copy; there is no public partial install API.
    world.marks = candidate.value().marks().clone();
    assert_eq!(world.public_snapshot(&actor("player")).marks.len(), 1);
    assert!(!marks::sweep(&mut world, 2.0 + 0.5 / 1440.0));
    assert!(world.marks.get(house).is_some() && world.marks.get(place).is_some());
    assert!(marks::sweep(&mut world, 2.0 + 2.0 / 1440.0));
    assert!(world.marks.get(house).is_none() && world.marks.get(place).is_none());
    assert!(world.marks.get(live).is_some());
}

#[test]
fn an_empty_scrubbed_store_keeps_the_spent_day_and_next_identity() {
    let mut world = world();
    world
        .notices
        .raise(
            "a neighbour".into(),
            "a missing purse".into(),
            None,
            None,
            Some(0.0),
            actor("departed_officer"),
            Some(actor("cb947")),
            None,
            None,
        )
        .unwrap();
    notices::chalk_the_debtors(&mut world, 2.1);
    let first = world.marks.find(MarkKind::ChalkCross, &anchor()).unwrap().0;
    marks::scrub(&mut world, first).unwrap();
    assert!(world.marks.is_empty());
    world.current_time = Some(WorldTime::from_game_days(2.1));
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        MarksCheckpointContext::from_world(&world, now()),
        &budget,
    );
    world.marks = candidate.value().marks().clone();
    assert!(notices::chalk_the_debtors(&mut world, 2.9).is_empty());
    assert!(
        world.marks.is_empty(),
        "loading must not cost the player's clean-wall window"
    );
    notices::chalk_the_debtors(&mut world, 3.0);
    let second = world.marks.find(MarkKind::ChalkCross, &anchor()).unwrap().0;
    assert_eq!(second.0, first.0 + 1);
}

#[test]
fn refresh_after_calendar_rewind_keeps_age_author_and_identity() {
    let mut world = world();
    let id = draw(&mut world);
    world.marks.get_mut(id).unwrap().strength = 0.37;
    let refreshed = marks::draw_or_refresh(
        &mut world,
        MarkKind::ChalkCross,
        anchor(),
        Some(actor("new_hand")),
        1.5,
    )
    .unwrap();
    assert_eq!(refreshed.id, id);
    assert!(!refreshed.fresh);
    world.current_time = Some(WorldTime::from_game_days(1.5));
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        MarksCheckpointContext::from_world(&world, now()),
        &budget,
    );
    let mark = candidate.value().marks().get(id).unwrap();
    assert_eq!(mark.drawn_game_days, 2.0);
    assert_eq!(mark.last_decayed_game_days, 1.5);
    assert_eq!(mark.author, Some(actor("departed_hand")));
    assert_eq!(mark.strength, 1.0);
    assert_eq!(candidate.value().marks(), &world.marks);
}

#[test]
fn invalid_decay_configuration_retains_bits_and_the_ordinary_fallback() {
    for scale in [
        0.0,
        -1.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::from_bits(0x7ff8_0000_0000_0042),
    ] {
        let mut world = world();
        let id = draw(&mut world);
        world.marks.decay_scale = scale;
        let budget = CheckpointBudget::default();
        let saved = save(&world, &budget);
        let candidate = load(
            saved.value(),
            MarksCheckpointContext::from_world(&world, now()),
            &budget,
        );
        assert_eq!(
            candidate.value().marks().decay_scale.to_bits(),
            scale.to_bits()
        );
        world.marks = candidate.value().marks().clone();
        let half_life = world
            .mark_catalog
            .spec(MarkKind::ChalkCross)
            .unwrap()
            .half_life_days_dry;
        assert!(marks::sweep(&mut world, 2.0 + half_life));
        assert!((world.marks.get(id).unwrap().strength - 0.5).abs() < 1e-12);
        drop((candidate, saved));
        assert_eq!(budget.retained_bytes(), 0);
    }
}

#[test]
fn changed_catalog_refuses_then_the_original_binds_to_an_unadopted_backbone() {
    let mut world = world();
    let id = draw(&mut world);
    world.marks_enabled = false;
    world.mark_kinds.cross = false;
    world.mark_kinds.ward_sign = false;
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let save_charge = budget.retained_bytes();
    let original_catalog = world.mark_catalog.clone();
    let mut source: serde_json::Value =
        serde_json::from_str(&prompt_support::asset("world/marks.json")).unwrap();
    source["kinds"]["chalk_cross"]["label"] = "a different public meaning".into();
    world.mark_catalog = Arc::new(MarkCatalog::from_json(&source.to_string()).unwrap());
    assert!(
        WorldMarksDtoV1::decode(
            saved.value(),
            budget
                .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
                .unwrap(),
            MarksCheckpointContext::from_world(&world, now())
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), save_charge);
    world.mark_catalog = original_catalog;
    // Separate component budgets prove borrowing, not complete-envelope coexistence.
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
    let context = MarksCheckpointContext::from_backbone(
        backbone.value(),
        now(),
        &world.mark_catalog,
        &world.shelters,
    );
    let candidate = load(saved.value(), context, &budget);
    assert_eq!(candidate.value().marks(), &world.marks);
    assert!(candidate.value().marks().get(id).is_some());
    assert!(!candidate.value().marks_enabled());
    let switches = candidate.value().mark_kinds();
    assert!(!switches.cross && switches.tally && !switches.ward_sign);
    world.marks = candidate.value().marks().clone();
    world.marks_enabled = candidate.value().marks_enabled();
    world.mark_kinds = switches;
    assert!(marks::marks_within(&world, Vec3::ZERO, 10.0).is_empty());
    world.marks_enabled = true;
    assert!(marks::binding_mark_about(&world, MarkKind::ChalkCross, &actor("cb947")).is_none());
    world.mark_kinds.cross = true;
    assert_eq!(
        marks::binding_mark_about(&world, MarkKind::ChalkCross, &actor("cb947")),
        Some(id)
    );
    drop((candidate, backbone, saved));
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(backbone_budget.retained_bytes(), 0);
}

#[test]
fn unknown_state_refuses_and_padded_input_stays_charged_through_validation() {
    let mut world = world();
    draw(&mut world);
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let save_charge = budget.retained_bytes();
    let context = MarksCheckpointContext::from_world(&world, now());
    let mut wire: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    wire["unrecognized_owner_state"] = true.into();
    let bytes = serde_json::to_vec(&wire).unwrap();
    assert!(
        WorldMarksDtoV1::decode(
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
    let decoded = WorldMarksDtoV1::decode(
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
    assert_eq!(candidate.value().marks(), &world.marks);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}
