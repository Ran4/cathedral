//! Independent acceptance tests through the ordinary host capture boundary.
//! Only installed-asset setup is shared with the implementation owner's tests.
use super::tests_hydration_owner::{installed_assets, installed_assets_with};
use super::{HostCaptureSet, HostObservation, capture_complete, validate_complete};
use crate::smart_actors::local_engine::LocalEngine;
use bevy::prelude::*;
use cathedral_sim::checkpoint::{
    Admitted, CheckpointBudget, CheckpointError, Cohort, MAX_RESIDENT_BYTES, Reservation,
    complete::{
        CheckpointCategory as Category, CheckpointProfile, CompleteCheckpointCandidate,
        CompleteCheckpointInput, HydrationStage, RUNNING_AUTHORITY_ALLOWANCE_BYTES,
    },
    host::DefinitionsV1,
};
use cathedral_sim::{NavData, RuntimeGeneration};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Weak},
};

const ASSETS: usize = 64 * 1024 * 1024;
const CATEGORIES: [Category; 16] = [
    Category::Ledger,
    Category::Operations,
    Category::Backbone,
    Category::Round,
    Category::Climate,
    Category::Knowledge,
    Category::Law,
    Category::Marks,
    Category::Animals,
    Category::Social,
    Category::Continuity,
    Category::Scheduler,
    Category::Night,
    Category::Speech,
    Category::CognitionInputs,
    Category::Host,
];

#[derive(Resource)]
struct CaptureRequest(Option<Captured>);

struct Captured {
    candidate: Admitted<CompleteCheckpointCandidate>,
    budget: CheckpointBudget,
    running: Reservation,
    definitions: DefinitionsV1,
    digests: [[u8; 32]; 16],
    generation: RuntimeGeneration,
    characters: usize,
    items: usize,
    nav: Weak<NavData>,
}

fn inspect(world: &mut World) {
    if !world.contains_resource::<CaptureRequest>() {
        return;
    }
    let stamp = super::tests_public::simulation_stamp(world);
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let save = capture_complete(
        world,
        CheckpointProfile::Authored,
        budget.reserve(Cohort::SavePayload, 4096).unwrap(),
    )
    .unwrap();
    let digests = CATEGORIES.map(|category| Sha256::digest(save.value().category(category)).into());
    let input = CompleteCheckpointInput::copy_from(
        save.value().bytes(),
        budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
    )
    .unwrap();
    let candidate = validate_complete(world, input).unwrap();
    drop(save);
    let source = HostObservation::new(world).unwrap();
    let engine = source.local().unwrap().checkpoint_engine().unwrap();
    let result = Captured {
        candidate,
        budget,
        running,
        definitions: source.definitions().unwrap(),
        digests,
        generation: engine.config().runtime_generation,
        characters: engine.world().characters.len(),
        items: engine.world().items.len(),
        nav: Arc::downgrade(engine.config().nav.as_ref().unwrap()),
    };
    assert_eq!(super::tests_public::simulation_stamp(world), stamp);
    world.resource_mut::<CaptureRequest>().0 = Some(result);
}

fn fixture() -> (App, Captured) {
    let mut app = super::tests_public::app();
    // Advance ordinary transactions so this is not a virgin-world round trip.
    for _ in 0..6 {
        app.update();
    }
    app.add_systems(PostUpdate, inspect.in_set(HostCaptureSet));
    app.insert_resource(CaptureRequest(None));
    app.update();
    let captured = app
        .world_mut()
        .remove_resource::<CaptureRequest>()
        .unwrap()
        .0
        .unwrap();
    (app, captured)
}

fn engine(app: &App) -> &cathedral_sim::Engine {
    app.world()
        .non_send::<LocalEngine>()
        .checkpoint_engine()
        .unwrap()
}

fn fresh(old: RuntimeGeneration) -> RuntimeGeneration {
    RuntimeGeneration(if old.0 == u64::MAX { 1 } else { old.0 + 1 })
}

#[test]
fn hydration_recreates_all_owners_after_original_runtime_is_disposed() {
    let (app, captured) = fixture();
    let Captured {
        candidate,
        budget,
        running,
        definitions,
        digests,
        generation,
        characters,
        items,
        nav,
    } = captured;
    let boundary = candidate.value().boundary();
    let lineage = candidate.value().world_identity();
    assert!(boundary.seconds() > 0.0);
    let hydrated = candidate
        .prepare_hydration(ASSETS)
        .unwrap()
        .hydrate(
            || {
                let assets = installed_assets(engine(&app), 0)?;
                drop(app);
                Ok(assets)
            },
            definitions,
            fresh(generation),
        )
        .unwrap();
    let value = hydrated.value();
    assert_eq!(value.boundary(), boundary);
    assert_eq!(value.world_identity(), lineage);
    assert_eq!(value.runtime_generation(), fresh(generation));
    assert_eq!(value.host().scalars().boundary.generation, generation.0);
    assert_eq!(value.character_count(), characters);
    assert_eq!(value.item_count(), items);
    assert!(
        nav.upgrade().is_some(),
        "new owner must retain its installed navigation"
    );
    for (category, expected) in CATEGORIES.into_iter().zip(digests) {
        let actual = value
            .category_digest(category, budget.reserve(Cohort::SavePayload, 4096).unwrap())
            .unwrap();
        assert_eq!(
            *actual.value(),
            expected,
            "new owner differs for {category:?}"
        );
    }
    assert_eq!(
        budget.retained_bytes(),
        running.bytes() + value.cost().retained_upper_bytes
    );
    assert!(budget.reserve(Cohort::LoadCandidate, 1).is_err());
    drop(hydrated);
    assert!(
        nav.upgrade().is_none(),
        "disposed runtime retained a navigation owner"
    );
    assert_eq!(budget.retained_bytes(), running.bytes());
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn hydration_rejects_changed_installed_inputs_before_construction() {
    for change in 0..3 {
        let (app, captured) = fixture();
        let Captured {
            candidate,
            budget,
            running,
            mut definitions,
            generation,
            ..
        } = captured;
        let before = super::tests_public::simulation_stamp(app.world());
        if change == 2 {
            definitions.collision[0] ^= 1;
        }
        let construction = Cell::new(false);
        let result = candidate
            .prepare_hydration(ASSETS)
            .unwrap()
            .hydrate_observed(
                || {
                    installed_assets_with(engine(&app), 0, |config, world| match change {
                        0 => config.sounds_enabled = !config.sounds_enabled,
                        1 => world.nav = None,
                        _ => {}
                    })
                },
                definitions,
                fresh(generation),
                &mut |stage| {
                    if matches!(stage, HydrationStage::Construction) {
                        construction.set(true);
                    }
                },
            );
        assert!(
            result.is_err(),
            "changed installed input {change} was accepted"
        );
        assert!(!construction.get());
        assert_eq!(super::tests_public::simulation_stamp(app.world()), before);
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
}

#[test]
fn hydration_requires_a_new_execution_fence_and_releases_failed_candidates() {
    for zero in [true, false] {
        let (app, captured) = fixture();
        let Captured {
            candidate,
            budget,
            running,
            definitions,
            generation,
            ..
        } = captured;
        let factory_called = Cell::new(false);
        let result = candidate.prepare_hydration(ASSETS).unwrap().hydrate(
            || {
                factory_called.set(true);
                installed_assets(engine(&app), 0)
            },
            definitions,
            if zero {
                RuntimeGeneration(0)
            } else {
                generation
            },
        );
        assert!(result.is_err());
        assert_eq!(factory_called.get(), !zero);
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
}

#[test]
fn hydration_admission_precedes_asset_factory_and_cancellation_releases_its_lease() {
    let (app, captured) = fixture();
    let Captured {
        candidate,
        budget,
        running,
        definitions,
        generation,
        ..
    } = captured;
    let spare = MAX_RESIDENT_BYTES - budget.retained_bytes();
    let pressure = budget
        .reserve(Cohort::RetiringGeneration, spare - ASSETS + 1)
        .unwrap();
    let factory_called = Cell::new(false);
    let result = candidate.prepare_hydration(ASSETS).and_then(|prepared| {
        prepared.hydrate(
            || {
                factory_called.set(true);
                installed_assets(engine(&app), 0)
            },
            definitions,
            fresh(generation),
        )
    });
    assert!(result.is_err());
    assert!(!factory_called.get());
    assert_eq!(budget.retained_bytes(), running.bytes() + pressure.bytes());
    drop(pressure);
    assert_eq!(budget.retained_bytes(), running.bytes());
    drop(app);

    let (_app, captured) = fixture();
    let Captured {
        candidate,
        budget,
        running,
        ..
    } = captured;
    let raw_bytes = candidate.reserved_bytes();
    let prepared = candidate.prepare_hydration(ASSETS).unwrap();
    assert_eq!(
        budget.retained_bytes(),
        running.bytes() + raw_bytes + ASSETS
    );
    assert!(budget.reserve(Cohort::LoadCandidate, 1).is_err());
    drop(prepared);
    assert_eq!(budget.retained_bytes(), running.bytes());
}

#[test]
fn hydration_factory_failure_and_observer_unwind_release_actual_asset_owners() {
    let (_app, captured) = fixture();
    let Captured {
        candidate,
        budget,
        running,
        definitions,
        generation,
        ..
    } = captured;
    let result = candidate.prepare_hydration(ASSETS).unwrap().hydrate(
        || {
            Err(CheckpointError {
                owner: "test factory",
                reason: "missing parsed asset".into(),
            })
        },
        definitions,
        fresh(generation),
    );
    assert!(result.is_err());
    assert_eq!(budget.retained_bytes(), running.bytes());

    for fail_stage in 0..6 {
        let (app, captured) = fixture();
        let Captured {
            candidate,
            budget,
            running,
            definitions,
            generation,
            nav,
            ..
        } = captured;
        let prepared = candidate.prepare_hydration(ASSETS).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            prepared.hydrate_observed(
                || {
                    let assets = installed_assets(engine(&app), 0)?;
                    drop(app);
                    Ok(assets)
                },
                definitions,
                fresh(generation),
                &mut |stage| {
                    let index = match stage {
                        HydrationStage::Assets => 0,
                        HydrationStage::Definitions => 1,
                        HydrationStage::TypedOwners => 2,
                        HydrationStage::Construction => 3,
                        HydrationStage::RawDisposal => 4,
                        HydrationStage::Retention => 5,
                    };
                    assert!(nav.upgrade().is_some());
                    assert!(budget.retained_bytes() >= running.bytes() + ASSETS);
                    if index == fail_stage {
                        panic!("intentional observer failure {fail_stage}");
                    }
                },
            )
        }));
        assert!(
            result.is_err(),
            "observer stage {fail_stage} did not execute"
        );
        assert!(
            nav.upgrade().is_none(),
            "asset survived unwind at stage {fail_stage}"
        );
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
}

#[test]
fn hydrated_category_observation_cannot_bypass_shared_memory_accounting() {
    let (app, captured) = fixture();
    let Captured {
        candidate,
        budget,
        running,
        definitions,
        generation,
        ..
    } = captured;
    let hydrated = candidate
        .prepare_hydration(ASSETS)
        .unwrap()
        .hydrate(
            || installed_assets(engine(&app), 0),
            definitions,
            fresh(generation),
        )
        .unwrap();
    let independent = CheckpointBudget::default();
    let before = budget.retained_bytes();
    let result = hydrated.value().category_digest(
        Category::Backbone,
        independent.reserve(Cohort::SavePayload, 4096).unwrap(),
    );
    assert!(result.is_err());
    assert_eq!(independent.peak_retained_bytes(), 4096);
    assert_eq!(independent.retained_bytes(), 0);
    assert_eq!(budget.retained_bytes(), before);
    let pressure = budget
        .reserve(
            Cohort::RetiringGeneration,
            MAX_RESIDENT_BYTES - before - 4096,
        )
        .unwrap();
    let result = hydrated.value().category_digest(
        Category::Backbone,
        budget.reserve(Cohort::SavePayload, 4096).unwrap(),
    );
    assert!(result.is_err());
    assert_eq!(budget.retained_bytes(), before + pressure.bytes());
    drop(pressure);
    drop(hydrated);
    assert_eq!(budget.retained_bytes(), running.bytes());
}
