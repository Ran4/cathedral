//! Public component boundaries; no Engine or World adoption is installed here.
use cathedral_sim::{
    NavData, World,
    checkpoint::{CheckpointBudget, Cohort},
    round::{
        Round,
        checkpoint::{RoundCheckpointContext, RoundDtoV1},
    },
    world::WorldBackboneDtoV1,
};
use serde_json::json;

fn navigation_json() -> String {
    json!({
        "schema_version": 1,
        "grid": {
            "x0": 0.0, "z0": 0.0, "cell_m": 1.0, "w": 4, "h": 4,
            "agent_radius_m": 0.1, "bitset_file": "fixture.bin",
            "bitset_bits": 16, "bitset_sha256": "unused-authoring-label"
        },
        "nodes": [[0.5, 0.5], [1.5, 0.5]],
        "edges": [[0, 1, 1.0]],
        "places": [], "sites": [], "doors": [],
        "reference": {"forecourt": 0}
    })
    .to_string()
}

#[test]
fn navigation_binding_uses_both_exact_inputs_and_ignores_cache_warmth() {
    let source = navigation_json();
    let nav = NavData::from_parts(&source, &[255, 255]).unwrap();
    let original = nav.checkpoint_fingerprint();
    assert_eq!(nav.cached_distance_m(0, 1), Some(1.0));
    assert_eq!(nav.checkpoint_fingerprint(), original);
    assert_eq!(nav.clone().checkpoint_fingerprint(), original);
    assert_eq!(
        NavData::from_parts(&source, &[255, 255])
            .unwrap()
            .checkpoint_fingerprint(),
        original,
    );

    // The parsed graph can be identical while the installed source differs.
    let changed_json = NavData::from_parts(&(source.clone() + "\n"), &[255, 255]).unwrap();
    assert_eq!(changed_json.cached_distance_m(0, 1), Some(1.0));
    assert_ne!(changed_json.checkpoint_fingerprint(), original);
    let changed_bits = NavData::from_parts(&source, &[255, 127]).unwrap();
    assert_eq!(changed_bits.cached_distance_m(0, 1), Some(1.0));
    assert_ne!(changed_bits.checkpoint_fingerprint(), original);
}

#[test]
fn wrong_navigation_context_refuses_without_leaking_candidate_admission() {
    let source = navigation_json();
    let nav = NavData::from_parts(&source, &[255, 255]).unwrap();
    let changed = NavData::from_parts(&source, &[255, 127]).unwrap();
    let world = World::new();
    let round = Round::new();
    let context = RoundCheckpointContext::from_world(&world, Some(&nav));
    let budget = CheckpointBudget::default();
    let encoded = round
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let retained_save = budget.retained_bytes();
    let rejected = RoundDtoV1::decode(
        encoded.value(),
        budget
            .reserve(Cohort::LoadCandidate, encoded.value().len() + 4096)
            .unwrap(),
        RoundCheckpointContext::from_world(&world, Some(&changed)),
    )
    .unwrap_err();
    assert_eq!(rejected.owner, "round");
    assert!(rejected.reason.contains("context mismatch"));
    assert_eq!(budget.retained_bytes(), retained_save);

    let restored = RoundDtoV1::decode(
        encoded.value(),
        budget
            .reserve(Cohort::LoadCandidate, encoded.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(restored.value().counts(context).people, 0);
    drop((restored, encoded));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn round_references_can_use_an_unadopted_backbone() {
    let world = World::new();
    let backbone_budget = CheckpointBudget::default();
    let backbone_bytes = world
        .export_backbone_checkpoint(backbone_budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let backbone = WorldBackboneDtoV1::decode(
        backbone_bytes.value(),
        backbone_budget
            .reserve(Cohort::LoadCandidate, backbone_bytes.value().len() + 4096)
            .unwrap(),
        &world.item_catalog,
        &world.command_ledger,
    )
    .unwrap()
    .into_candidate(&world.item_catalog, &world.command_ledger)
    .unwrap();
    drop(backbone_bytes);

    // Separate standalone component budgets do not prove complete-envelope
    // coexistence. This checks that reference validation needs no seeded World.
    let context = RoundCheckpointContext::from_backbone(
        backbone.value(),
        &world.item_catalog,
        None,
        &world.shelters,
    );
    let round_budget = CheckpointBudget::default();
    let round = Round::new();
    let bytes = round
        .export_checkpoint(
            context,
            round_budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
    let decoded = RoundDtoV1::decode(
        bytes.value(),
        round_budget
            .reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    let counts = decoded.value().counts(context);
    assert_eq!(
        (counts.characters, counts.people, counts.residents),
        (0, 0, 0)
    );
    drop((decoded, bytes, backbone));
    assert_eq!(round_budget.retained_bytes(), 0);
    assert_eq!(backbone_budget.retained_bytes(), 0);
}

#[test]
fn noncanonical_input_keeps_its_raw_charge_plus_definition_allowance() {
    let world = World::new();
    let context = RoundCheckpointContext::from_world(&world, None);
    let budget = CheckpointBudget::default();
    let dto = Round::new()
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap();
    let cost = dto.value().cost().unwrap();
    let encoded = dto.encode().unwrap();
    let mut padded = encoded.value().clone();
    let padding_bytes = 1024 * 1024;
    padded.resize(padded.len() + padding_bytes, b' ');
    drop(encoded);

    let decoded = RoundDtoV1::decode(
        &padded,
        budget
            .reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    // Whitespace changes only encoded length in preflight. The candidate keeps
    // that raw-input bound as well as the separate trusted-definition allowance.
    assert!(budget.retained_bytes() >= cost.peak_bytes + 3 * padding_bytes);
    let candidate = decoded.into_candidate(context).unwrap();
    assert!(budget.retained_bytes() >= cost.peak_bytes + 3 * padding_bytes);
    drop(candidate);
    assert_eq!(budget.retained_bytes(), 0);
}
