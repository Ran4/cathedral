//! Independent complete-checkpoint future witnesses. The CPU Host is explicit;
//! these cases do not implement Bevy controller or application adoption.
use super::continuation::future_harness::{Host, Restored, Unavailable, fixture, initial_host};
use super::*;
use crate::{
    checkpoint::{
        HostTimeV1,
        host::{FrameV1, Nullable},
    },
    receipts::{CommandId, HOST_PRODUCER, OperationId, ReceiptState},
    timeline::AcceptedTime,
};
use std::time::Duration;

fn in_generation(e: &Engine, command: EngineCommand) -> EngineCommand {
    EngineCommand::InGeneration {
        generation: e.config.runtime_generation,
        command: Box::new(command),
    }
}

fn setup() -> (Engine, Host) {
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            ..Default::default()
        },
        Box::new(Unavailable),
    );
    e.scheduler.close();
    let p = e.world.characters[&e.config.player_id].position_m();
    e.poll(
        0.0,
        vec![EngineCommand::SpatialUpdate {
            spatial_seq: 1,
            updates: vec![crate::SpatialActorUpdate::new(
                e.config.player_id.clone(),
                p,
                Some(0.0),
            )],
        }],
    );
    let mut host = initial_host(&e);
    host.after_poll(&e, 0.0);
    (e, host)
}

fn accept(e: &Engine, id: CommandId, item: &ItemId) -> EngineCommand {
    in_generation(
        e,
        EngineCommand::Identified {
            id,
            command: Box::new(EngineCommand::PlayerAccept {
                request_id: "m2d-same-intention".into(),
                item_id: item.clone(),
                position_m: e.world.characters[&e.config.player_id].position_m(),
                spatial_seq: e.world.spatial_sequence,
            }),
        },
    )
}

fn transfers(events: &[EngineMessage], item: &ItemId) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(event,
        EngineMessage::WorldEvent {kind, item_id: Some(found), ..}
            if kind == "accept_offered_item" && found == item)
        })
        .count()
}

#[test]
fn future_identified_accept_stays_spent_after_a_second_complete_restore() {
    let (mut control, mut host) = setup();
    let giver = ActorId::from_raw("sv3n1");
    let item = ItemId::from_raw("fzbn9");
    let player = control.config.player_id.clone();
    assert!(control.world.characters[&giver].holds().contains(&item));
    assert!(!control.world.characters[&player].holds().contains(&item));
    crate::actions::apply_action(
        &mut control.world,
        &giver,
        "offer_item",
        &serde_json::json!({"item_id": item.as_str(), "target": player.as_str()}),
    )
    .unwrap();
    control.poll(0.0, vec![]);
    host.after_poll(&control, 0.0);
    assert!(
        control.world.offers.contains_key(&item),
        "capture must contain the pending offer"
    );

    let mut restored = Restored::new(&control, &host, Box::new(Unavailable));
    restored.assert_eq(&control, &host);
    let id = OperationId {
        producer: HOST_PRODUCER,
        sequence: 1,
    }
    .command(0);
    let left_command = accept(&control, id, &item);
    let right_command = accept(restored.engine(), id, &item);
    let left = control.poll(0.1, vec![left_command]);
    host.after_poll(&control, 0.1);
    let right = restored.poll(0.1, vec![right_command]);
    assert_eq!(left, right);
    assert_eq!(transfers(&left, &item), 1);
    assert!(control.world.offers.is_empty());
    assert!(control.world.characters[&player].holds().contains(&item));
    assert!(!control.world.characters[&giver].holds().contains(&item));
    let receipt = control.world.command_ledger.get(id).unwrap().clone();
    assert_eq!(receipt.outcome.state, ReceiptState::Completed);
    restored.assert_eq(&control, &host);

    // Independent fixture budgets retain each real owner through construction;
    // this is a behavior witness, not an aggregate multi-host heap measurement.
    let mut again = Restored::new(restored.engine(), &restored.host, Box::new(Unavailable));
    drop(restored);
    assert_eq!(again.engine().world.command_ledger.get(id), Some(&receipt));
    let left_command = accept(&control, id, &item);
    let right_command = accept(again.engine(), id, &item);
    let left = control.poll(0.2, vec![left_command]);
    host.after_poll(&control, 0.2);
    let right = again.poll(0.2, vec![right_command]);
    assert_eq!(left, right);
    assert_eq!(
        transfers(&left, &item),
        0,
        "replayed command must not transfer twice"
    );
    assert!(right.iter().any(|event| matches!(event,
        EngineMessage::ActionReceipt(found) if found == &receipt)));
    assert_eq!(again.engine().world.command_ledger.get(id), Some(&receipt));
    assert_eq!(
        again.engine().world.characters[&player]
            .holds()
            .iter()
            .filter(|found| *found == &item)
            .count(),
        1
    );
    again.assert_eq(&control, &host);
}

#[test]
fn future_nonzero_wall_debt_and_spatial_samples_survive_successive_complete_loads() {
    let (mut control, mut host) = setup();
    let mut admitted = AcceptedTime::default();
    let first = admitted.admit(Duration::from_millis(137));
    assert_eq!(first.accepted_delta, Duration::from_millis(100));
    assert_eq!(first.debt, Duration::from_millis(37));
    control.poll(first.elapsed.as_secs_f64(), vec![]);
    host.after_poll(&control, first.elapsed.as_secs_f64());
    let step = host.0.time.accepted.fixed_step().unwrap();
    let residual = host.0.time.accepted.fixed_residual().unwrap();
    assert!(
        !residual.is_zero(),
        "capture must exercise a retained fixed-step residual"
    );
    host.0.time.accepted = HostTimeV1::from_accepted(
        admitted,
        step,
        residual,
        first.elapsed.as_secs_f64() - control.movement_now,
    )
    .unwrap();
    host.0.time.last_frame = Nullable(Some(FrameV1 {
        wall_delta: first.wall_delta,
        accepted_delta: first.accepted_delta,
        elapsed: first.elapsed,
        debt: first.debt,
    }));
    let mut restored = Restored::new(&control, &host, Box::new(Unavailable));
    assert_eq!(restored.host.0.time.accepted.accepted().unwrap(), admitted);
    assert_eq!(
        restored.host.0.time.accepted.fixed_residual().unwrap(),
        residual
    );
    restored.assert_eq(&control, &host);

    for sequence in 2..=6 {
        // A 100ms new wall step plus existing debt admits exactly 100ms and
        // preserves that debt under the real AcceptedTime::admit policy.
        let frame = admitted.admit(Duration::from_millis(100));
        assert_eq!(frame.debt, Duration::from_millis(37));
        let now = frame.elapsed.as_secs_f64();
        let p = Vec3::new(
            f64::from(sequence as f32 * 0.125),
            f64::from(0.91_f32),
            111.0,
        );
        let command = |e: &Engine| {
            in_generation(
                e,
                EngineCommand::SpatialUpdate {
                    spatial_seq: sequence,
                    updates: vec![crate::SpatialActorUpdate::new(
                        e.config.player_id.clone(),
                        p,
                        Some(0.0),
                    )],
                },
            )
        };
        let left_command = command(&control);
        let right_command = command(restored.engine());
        let left = control.poll(now, vec![left_command]);
        host.after_poll(&control, now);
        let right = restored.poll(now, vec![right_command]);
        assert_eq!(left, right);
        assert_eq!(restored.engine().world.spatial_sequence, sequence);
        assert_eq!(
            restored.engine().world.characters[&control.config.player_id].position_m(),
            p
        );
        assert_eq!(restored.host.0.time.accepted.accepted().unwrap(), admitted);
        assert_eq!(restored.host.0.spatial.sequence, sequence as u64);
        restored.assert_eq(&control, &host);
        if sequence == 4 {
            let next = Restored::new(restored.engine(), &restored.host, Box::new(Unavailable));
            drop(restored);
            restored = next;
            assert_eq!(restored.host.0.time.accepted.accepted().unwrap(), admitted);
            restored.assert_eq(&control, &host);
        }
    }
    assert_eq!(admitted.elapsed, Duration::from_millis(600));
    assert_eq!(admitted.wall, Duration::from_millis(637));
}
