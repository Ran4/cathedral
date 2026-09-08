//! Public component boundary checks. Whole-engine continuation is a later cut.
use cathedral_sim::{
    Character, CharacterSheet, INBOX_MAX_ENTRIES, ItemId, MAX_ID_CHARS, RECENT_HISTORY_MAX_ENTRIES,
    Vec3,
    character::CharacterDtoV1,
    checkpoint::{CheckpointBudget, Cohort},
};
use serde_json::{Value, json};

fn character(id: &str) -> Character {
    let sheet: CharacterSheet = serde_json::from_value(json!({
        "id": id,
        "name": "The checkpoint witness",
        "control": "llm",
        "back_story": "A witness with an unread account.",
        "location_description": "The market",
        "voice_key": null,
        "position_m": {"x": 1.0, "y": 0.0, "z": 2.0},
        "holds": ["seed_loaf"],
        "memories": ["The seed-time account."]
    }))
    .unwrap();
    Character::from_sheet(sheet)
}

fn export(actor: &Character) -> Value {
    let budget = CheckpointBudget::default();
    let dto = actor
        .export_checkpoint(budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap();
    let expected = serde_json::to_value(dto.value()).unwrap();
    let encoded = dto.encode().unwrap();
    let decoded = CharacterDtoV1::decode(
        encoded.value(),
        budget
            .reserve(Cohort::LoadCandidate, encoded.value().len() + 4096)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(decoded.value()).unwrap(), expected);
    drop((decoded, encoded));
    assert_eq!(budget.retained_bytes(), 0);
    expected
}

fn decode(value: &Value) -> Result<Value, cathedral_sim::checkpoint::CheckpointError> {
    let bytes = serde_json::to_vec(value).unwrap();
    let budget = CheckpointBudget::default();
    let result = CharacterDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
    );
    let result = result.map(|dto| serde_json::to_value(dto.value()).unwrap());
    assert_eq!(budget.retained_bytes(), 0);
    result
}

#[test]
fn full_unread_window_and_repeated_history_occurrences_survive() {
    let mut actor = character("witness");
    for n in 0..RECENT_HISTORY_MAX_ENTRIES {
        actor.remember_percept(format!("account {n}"));
    }
    for n in 0..INBOX_MAX_ENTRIES + 7 {
        actor.notify_percept(format!("account {n}"));
    }
    assert_eq!(actor.pending_history().len(), INBOX_MAX_ENTRIES);
    assert_eq!(actor.recent_history().len(), RECENT_HISTORY_MAX_ENTRIES);
    assert!(actor.pending_history().len() > actor.recent_history().len());
    assert!(
        actor
            .pending_history()
            .iter()
            .any(|line| actor.recent_history().contains(line))
    );

    let saved = export(&actor);
    assert_eq!(saved["value"]["state"]["inbox"], json!(actor.inbox()));
    assert_eq!(
        saved["value"]["state"]["pending_history"],
        json!(actor.pending_history())
    );
    assert_eq!(
        saved["value"]["state"]["recent_history"],
        json!(actor.recent_history())
    );
}

#[test]
fn seed_identity_and_changed_live_state_remain_separate() {
    let mut actor = character("witness");
    actor.state.holds = vec![ItemId::new("new_receipt").unwrap()];
    actor.state.position_m = Vec3::new(-7.0, 0.0, 13.0);
    actor.state.memories = vec!["The corrected account.".into()];
    actor.state.presence_epoch += 1;

    let saved = export(&actor);
    assert_eq!(saved["value"]["sheet"]["holds"], json!(["seed_loaf"]));
    assert_eq!(saved["value"]["state"]["holds"], json!(["new_receipt"]));
    assert_eq!(
        saved["value"]["sheet"]["memories"],
        json!(["The seed-time account."])
    );
    assert_eq!(
        saved["value"]["state"]["memories"],
        json!(["The corrected account."])
    );
    assert_ne!(
        saved["value"]["sheet"]["position_m"],
        saved["value"]["state"]["position_m"]
    );
    assert_ne!(
        saved["value"]["sheet"]["presence_epoch"],
        saved["value"]["state"]["presence_epoch"]
    );
}

#[test]
fn unicode_identity_uses_the_city_scalar_limit() {
    let id = "é".repeat(MAX_ID_CHARS);
    assert!(id.len() > MAX_ID_CHARS);
    let saved = export(&character(&id));
    assert_eq!(saved["value"]["sheet"]["id"], id);

    let mut corrupt = saved;
    corrupt["value"]["sheet"]["id"] = json!("é".repeat(MAX_ID_CHARS + 1));
    assert!(decode(&corrupt).is_err());
}

#[test]
fn absent_optional_authority_is_rejected_but_explicit_null_is_retained() {
    let saved = export(&character("witness"));
    for (parent, fields) in [
        ("/value/sheet", &["voice_key", "frontbutt", "lore"][..]),
        ("/value/sheet/appearance", &["bespoke"][..]),
        (
            "/value/state",
            &[
                "urgency_since_game_days",
                "debug_urgency",
                "movement",
                "intent",
                "resident",
                "round_edit",
                "active_gesture",
            ][..],
        ),
    ] {
        for field in fields {
            let mut missing = saved.clone();
            let removed = missing
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(*field);
            assert_eq!(removed, Some(Value::Null), "{parent}/{field}");
            assert!(decode(&missing).is_err(), "missing {parent}/{field}");
        }
    }
    assert_eq!(decode(&saved).unwrap(), saved);
}

#[test]
fn duplicate_perspective_identity_is_not_silently_collapsed() {
    let mut saved = export(&character("witness"));
    saved["value"]["state"]["knows"] = json!(["other", "other"]);
    let error = decode(&saved).unwrap_err();
    assert!(error.reason.contains("duplicate"), "{error}");
}
