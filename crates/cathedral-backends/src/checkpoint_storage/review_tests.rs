//! Independent M3a storage review, separate from the publication fault matrix.
use super::tests::{Fixture, TempDir, metadata, slot, wait_result};
use super::*;
use cathedral_sim::checkpoint::complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES;
use std::time::{Duration, Instant};

fn save(
    service: &CheckpointStorage,
    fixture: &Fixture,
    slot: SlotId,
    title: &str,
) -> SlotReference {
    let id = service.request_save(slot).unwrap();
    assert_eq!(service.next_capture(), Some(id));
    service
        .attach(id, fixture.capture(), metadata(title))
        .unwrap();
    match wait_result(service, id) {
        Outcome::Saved(reference) => reference,
        other => panic!("save failed: {other:?}"),
    }
}

fn load(service: &CheckpointStorage, slot: SlotId, selection: LoadSelection) -> LoadedCheckpoint {
    let id = service.request_load(slot, selection).unwrap();
    match wait_result(service, id) {
        Outcome::Loaded(loaded) => loaded,
        other => panic!("load failed: {other:?}"),
    }
}

#[test]
fn storage_review_damaged_active_can_recover_and_save_again_without_losing_previous() {
    let fixture = Fixture::new();
    for damage in [
        "missing_reference",
        "truncated_reference",
        "truncated_payload",
    ] {
        let temp = TempDir::new();
        let service = CheckpointStorage::start(temp.path(), fixture.budget.clone()).unwrap();
        let old = save(&service, &fixture, slot(), "Earlier valid save");
        let latest = save(&service, &fixture, slot(), "Latest save");
        drop(service.shutdown().join());
        let active = temp.path().join(disk::slot_file(&slot(), "active"));
        match damage {
            "missing_reference" => std::fs::remove_file(active).unwrap(),
            "truncated_reference" => std::fs::write(active, b"{\"body\":").unwrap(),
            "truncated_payload" => {
                std::fs::write(
                    temp.path()
                        .join(disk::payload_file(&slot(), latest.generation())),
                    b"{",
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let service = CheckpointStorage::start(temp.path(), fixture.budget.clone()).unwrap();
        let failed = service.request_load(slot(), LoadSelection::Active).unwrap();
        assert!(
            matches!(wait_result(&service, failed), Outcome::Failed(_)),
            "{damage}"
        );
        let previous = load(&service, slot(), LoadSelection::Previous);
        assert_eq!(previous.reference, old);
        drop(fixture.validate(previous));

        let recovery = service.request_recovery(slot()).unwrap();
        match wait_result(&service, recovery) {
            Outcome::Recovered(Some(reference)) => assert_eq!(reference, old),
            other => panic!("{damage}: recovery failed: {other:?}"),
        }
        let restored = load(&service, slot(), LoadSelection::Active);
        assert_eq!(restored.reference, old);
        drop(fixture.validate(restored));
        let after = save(&service, &fixture, slot(), "Saved after recovery");
        let restored = load(&service, slot(), LoadSelection::Active);
        assert_eq!(restored.reference, after);
        drop(fixture.validate(restored));
        let previous = load(&service, slot(), LoadSelection::Previous);
        assert_eq!(previous.reference, old);
        drop(fixture.validate(previous));
        drop(service.shutdown().join());
        assert_eq!(
            fixture.budget.retained_bytes(),
            RUNNING_AUTHORITY_ALLOWANCE_BYTES
        );
    }
}

#[test]
fn storage_review_pins_the_directory_and_rejects_a_reference_copied_to_another_slot() {
    let fixture = Fixture::new();
    let temp = TempDir::new();
    let original = temp.path().join("store");
    let moved = temp.path().join("moved-store");
    std::fs::create_dir(&original).unwrap();
    let service = CheckpointStorage::start(&original, fixture.budget.clone()).unwrap();
    let first = save(&service, &fixture, slot(), "First slot");
    // The first terminal proves the worker has opened and pinned the directory.
    std::fs::rename(&original, &moved).unwrap();
    std::fs::create_dir(&original).unwrap();
    let other_slot = SlotId::new("manual-2").unwrap();
    let second = save(&service, &fixture, other_slot.clone(), "Second slot");
    assert!(
        moved
            .join(disk::payload_file(&other_slot, second.generation()))
            .is_file()
    );
    assert_eq!(std::fs::read_dir(&original).unwrap().count(), 0);
    drop(service.shutdown().join());

    // Offline corruption: the copied reference is itself checksummed and valid
    // for its original slot, but cannot authorize loading under another slot.
    std::fs::copy(
        moved.join(disk::slot_file(&slot(), "active")),
        moved.join(disk::slot_file(&other_slot, "active")),
    )
    .unwrap();
    let service = CheckpointStorage::start(&moved, fixture.budget.clone()).unwrap();
    let id = service
        .request_load(other_slot, LoadSelection::Active)
        .unwrap();
    match wait_result(&service, id) {
        Outcome::Failed(error) => assert_eq!(error.phase, Phase::Inspect),
        other => panic!("cross-slot reference accepted: {other:?}"),
    }
    let unaffected = load(&service, slot(), LoadSelection::Active);
    assert_eq!(unaffected.reference, first);
    drop(fixture.validate(unaffected));
    drop(service.shutdown().join());
}

#[test]
fn storage_review_unread_load_pins_admission_until_shutdown_abandonment_is_disposed() {
    let fixture = Fixture::new();
    let temp = TempDir::new();
    let service = CheckpointStorage::start(temp.path(), fixture.budget.clone()).unwrap();
    let saved = save(&service, &fixture, slot(), "Unread load");
    let unread = service.request_load(slot(), LoadSelection::Active).unwrap();
    let later = service.request_load(slot(), LoadSelection::Active).unwrap();
    match wait_result(&service, later) {
        Outcome::Failed(error) => assert_eq!(error.phase, Phase::Admission),
        other => panic!("second load bypassed retained candidate: {other:?}"),
    }
    // Serial processing of the later request proves the first load is terminal.
    // Leave that input inside the service, never handing it to the host.
    assert_eq!(service.retained_operations(), 1);
    let expected = RUNNING_AUTHORITY_ALLOWANCE_BYTES
        + SERVICE_ALLOWANCE_BYTES
        + saved.payload_bytes() as usize
        + 4096;
    assert_eq!(fixture.budget.retained_bytes(), expected);
    let shutdown = service.shutdown();
    assert!(
        !shutdown.is_finished(),
        "unread terminal must retain its disposal worker"
    );
    assert_eq!(fixture.budget.retained_bytes(), expected);
    drop(shutdown);
    let deadline = Instant::now() + Duration::from_secs(5);
    while fixture.budget.retained_bytes() != RUNNING_AUTHORITY_ALLOWANCE_BYTES {
        assert!(
            Instant::now() < deadline,
            "abandoned load {unread:?} leaked its worker or candidate"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn storage_review_deserialization_cannot_bypass_bounded_player_preview_types() {
    let metadata = serde_json::json!({
        "title": "Market visit",
        "captured_unix_seconds": 1_789_441_234i64,
        "known_location": "Market square",
    });
    let accepted: SaveMetadata = serde_json::from_value(metadata.clone()).unwrap();
    assert_eq!(accepted.title(), "Market visit");
    assert_eq!(accepted.known_location(), "Market square");
    for (field, value) in [
        ("title", serde_json::json!("x".repeat(97))),
        ("known_location", serde_json::json!("x".repeat(161))),
        ("title", serde_json::json!("Market\nHidden line")),
        ("captured_unix_seconds", serde_json::json!(i64::MAX)),
        ("npc_private_plan", serde_json::json!("not a preview field")),
    ] {
        let mut invalid = metadata.clone();
        invalid[field] = value;
        assert!(
            serde_json::from_value::<SaveMetadata>(invalid).is_err(),
            "{field}"
        );
    }
    for invalid in [
        "../quick",
        "quick/other",
        ".",
        "",
        "quick\u{0}",
        "quick\\other",
    ] {
        assert!(SlotId::new(invalid).is_err());
        assert!(serde_json::from_value::<SlotId>(serde_json::json!(invalid)).is_err());
    }
}
