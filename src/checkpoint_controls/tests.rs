use super::*;
use crate::installed_recipe::InstalledRecipe;
use cathedral_sim::checkpoint::MAX_RESIDENT_BYTES;

#[test]
fn control_owner_is_precharged_fixed_and_returns_lease_after_refusal() {
    let installed = InstalledRecipe::new().unwrap();
    let budget = installed.budget().clone();
    let baseline = budget.retained_bytes();
    assert!(size_of::<CheckpointControls>() <= CONTROL_BYTES);
    let (mut controls, allocated) = crate::host_checkpoint::measure_installed_allocations(|| {
        CheckpointControls::admitted(&budget).unwrap()
    });
    assert!(
        allocated <= CONTROL_BYTES,
        "constructor allocated {allocated}"
    );
    assert_eq!(budget.retained_bytes(), baseline + CONTROL_BYTES);
    let (_, allocated) = crate::host_checkpoint::measure_installed_allocations(|| {
        for request_id in 1..=10_000 {
            let receipt = controls
                .request(CheckpointAction::QuickSave, None, None, None, None)
                .unwrap();
            assert_eq!(receipt.request_id, request_id);
            assert_eq!(receipt.refusal, ControlRefusal::NoCommittedStartup);
        }
    });
    assert_eq!(allocated, 0);
    assert_eq!(controls.last.unwrap().request_id, 10_000);
    assert_eq!(budget.retained_bytes(), baseline + CONTROL_BYTES);
    drop(controls);
    assert_eq!(budget.retained_bytes(), baseline);
    let full = budget
        .reserve_running_overhead(MAX_RESIDENT_BYTES - baseline)
        .unwrap();
    assert!(CheckpointControls::admitted(&budget).is_err());
    assert_eq!(budget.retained_bytes(), baseline + full.bytes());
    drop(full);
    drop(installed);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn operation_identity_exhaustion_preserves_last_receipt_without_wrapping() {
    let installed = InstalledRecipe::new().unwrap();
    let budget = installed.budget().clone();
    let mut controls = CheckpointControls::admitted(&budget).unwrap();
    controls.next_request = Some(u64::MAX);
    let final_receipt = controls
        .request(CheckpointAction::QuickLoad, None, None, None, None)
        .unwrap();
    assert_eq!(final_receipt.request_id, u64::MAX);
    assert_eq!(
        controls.request(CheckpointAction::QuickSave, None, None, None, None),
        Err(ControlRefusal::RequestIdsExhausted)
    );
    assert_eq!(controls.last, Some(final_receipt));
}

#[test]
fn real_key_system_reports_refusal_without_waiting_or_consuming_gameplay_keys() {
    let installed = InstalledRecipe::new().unwrap();
    let budget = installed.budget().clone();
    let baseline = budget.retained_bytes();
    let mut app = App::new();
    app.insert_resource(CheckpointControls::admitted(&budget).unwrap())
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<SmartActorHudState>()
        .add_systems(Update, keyboard_controls);
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::F6);
        keys.press(KeyCode::F9);
    }
    app.update();
    let receipt = app.world().resource::<CheckpointControls>().last.unwrap();
    assert_eq!(receipt.request_id, 2);
    assert_eq!(receipt.action, CheckpointAction::QuickLoad);
    assert_eq!(receipt.refusal, ControlRefusal::NoCommittedStartup);
    assert_eq!(
        app.world()
            .resource::<SmartActorHudState>()
            .transient
            .as_ref()
            .unwrap()
            .text,
        "Load unavailable in this build. Your play continues."
    );
    // InputPlugin clears per-frame edges in the real App. Holding remains down
    // without a new edge; neither game movement nor its edge is consumed here.
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::KeyW)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    for _ in 0..20 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<CheckpointControls>().last,
        Some(receipt)
    );
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyW)
    );
    assert_eq!(budget.retained_bytes(), baseline + CONTROL_BYTES);
}
