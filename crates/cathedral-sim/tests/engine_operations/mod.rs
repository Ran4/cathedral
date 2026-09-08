//! Operation traces use the real Engine command/receipt/Round consumers.
use super::*;
use cathedral_sim::operations::{
    AdapterDeclaration, FixtureDeclaration, InstanceId, OperationConfig, Request,
};
use cathedral_sim::receipts::{CommandId, OperationId, ReceiptState};

fn worker() -> ActorId {
    ActorId::from_raw("sv3n1")
}
fn other() -> ActorId {
    ActorId::from_raw("cb947")
}
fn config() -> OperationConfig {
    let seed = seed();
    OperationConfig {
        fixtures: seed
            .characters
            .iter()
            .filter(|c| c.control != cathedral_sim::Control::Player)
            .enumerate()
            .map(|(n, c)| FixtureDeclaration {
                id: format!("fixture{n}"),
                adapter: AdapterDeclaration::default(),
                position: c.position_m.to_array(),
            })
            .collect(),
    }
}
fn harness() -> Harness {
    Builder {
        operations: config(),
        ..Default::default()
    }
    .build()
}
fn cid(n: u64) -> CommandId {
    OperationId {
        producer: 0,
        sequence: n,
    }
    .command(0)
}
fn start(actor: ActorId, resource: &str, work: f64, recovery: f64) -> Request {
    Request::Start {
        actor,
        resource: resource.into(),
        adapter: AdapterDeclaration::default(),
        work_seconds: work,
        recovery_seconds: recovery,
        retries: 2,
    }
}
fn send(h: &mut Harness, id: CommandId, request: Request) -> Vec<EngineMessage> {
    h.send(EngineCommand::Operation(request).identified(id))
}
fn state(h: &Harness, id: CommandId) -> ReceiptState {
    h.engine
        .world()
        .command_ledger
        .get(id)
        .unwrap()
        .outcome
        .state
}
fn code(h: &Harness, id: CommandId) -> &str {
    &h.engine
        .world()
        .command_ledger
        .get(id)
        .unwrap()
        .outcome
        .code
}
fn tick(h: &mut Harness, now: f64) -> Vec<EngineMessage> {
    h.now = now;
    h.poll()
}

#[test]
fn operation_trace_completes_once_and_releases_both_claims() {
    let mut h = harness();
    h.ready();
    let request = start(worker(), "fixture0", 0.25, 2.0);
    send(&mut h, cid(1), request.clone());
    assert_eq!(state(&h, cid(1)), ReceiptState::Accepted);
    assert_eq!(
        h.engine.world().operations.actor_owner(&worker()),
        Some(InstanceId(cid(1)))
    );
    assert_eq!(
        h.engine.world().operations.resource_owner("fixture0"),
        Some(InstanceId(cid(1)))
    );
    tick(&mut h, 0.1);
    assert_eq!(state(&h, cid(1)), ReceiptState::InProgress);
    let active = h
        .engine
        .world()
        .operations
        .active(InstanceId(cid(1)))
        .unwrap();
    assert_eq!(active.completed_work, 0.1);
    assert!(
        h.engine
            .world()
            .operations
            .validate_continuation(
                &config(),
                h.engine.world(),
                cathedral_sim::timeline::LogicalTime::new(0.1).unwrap()
            )
            .is_ok()
    );
    tick(&mut h, 0.2);
    tick(&mut h, 0.3);
    assert_eq!(state(&h, cid(1)), ReceiptState::Completed);
    assert_eq!(
        h.engine
            .world()
            .operations
            .fixture("fixture0")
            .unwrap()
            .completed_units,
        1
    );
    assert_eq!(h.engine.world().operations.active_count(), 0);
    assert_eq!(h.engine.world().operations.actor_owner(&worker()), None);
    assert_eq!(h.engine.world().operations.resource_owner("fixture0"), None);
    assert!(
        !h.engine
            .world()
            .command_ledger
            .is_protected(cid(1).operation)
    );
    send(&mut h, cid(1), request);
    assert_eq!(
        h.engine
            .world()
            .operations
            .fixture("fixture0")
            .unwrap()
            .completed_units,
        1
    );
    send(&mut h, cid(2), start(worker(), "fixture0", 0.25, 2.0));
    assert_eq!(state(&h, cid(2)), ReceiptState::Accepted);
}

#[test]
fn actor_and_resource_contention_refuse_before_mutation_and_siblings_do_not_alias() {
    let mut h = harness();
    h.ready();
    // Put both NPCs at the same declared fixture for a genuine resource conflict.
    let at = h.engine.world().characters[&worker()].position_m();
    h.engine
        .world_mut()
        .characters
        .get_mut(&other())
        .unwrap()
        .state
        .position_m = at;
    let root = OperationId {
        producer: 1,
        sequence: 1,
    };
    send(
        &mut h,
        root.command(1),
        start(worker(), "fixture0", 1.0, 5.0),
    );
    let before = h.engine.world().operations.clone();
    send(&mut h, cid(2), start(worker(), "fixture0", 1.0, 5.0));
    assert_eq!(code(&h, cid(2)), "actor_claimed");
    send(&mut h, cid(3), start(other(), "fixture0", 1.0, 5.0));
    assert_eq!(code(&h, cid(3)), "resource_claimed");
    assert_eq!(h.engine.world().operations, before);
    let at = Vec3::from_array(config().fixtures[1].position);
    h.engine
        .world_mut()
        .characters
        .get_mut(&other())
        .unwrap()
        .state
        .position_m = at;
    send(
        &mut h,
        root.command(2),
        start(other(), "fixture1", 1.0, 5.0),
    );
    assert_eq!(state(&h, root.command(2)), ReceiptState::Accepted);
    assert_ne!(
        h.engine.world().operations.actor_owner(&worker()),
        h.engine.world().operations.actor_owner(&other())
    );
    send(
        &mut h,
        cid(4),
        Request::Cancel {
            instance: InstanceId(root.command(1)),
        },
    );
    assert_eq!(state(&h, root.command(1)), ReceiptState::Interrupted);
    assert!(h.engine.world().command_ledger.is_protected(root));
    assert!(
        h.engine
            .world()
            .operations
            .active(InstanceId(root.command(2)))
            .is_some()
    );
    send(
        &mut h,
        cid(5),
        Request::Cancel {
            instance: InstanceId(root.command(2)),
        },
    );
    assert!(!h.engine.world().command_ledger.is_protected(root));
}

#[test]
fn replan_never_renews_progress_step_identity_or_operation_recovery_budget() {
    let mut h = harness();
    h.ready();
    send(&mut h, cid(1), start(worker(), "fixture0", 0.5, 1.0));
    tick(&mut h, 0.1);
    let original = h
        .engine
        .world()
        .operations
        .active(InstanceId(cid(1)))
        .unwrap()
        .clone();
    send(
        &mut h,
        cid(2),
        Request::SetObstructed {
            instance: InstanceId(cid(1)),
            obstructed: true,
        },
    );
    tick(&mut h, 0.2);
    send(
        &mut h,
        cid(3),
        Request::Replan {
            instance: InstanceId(cid(1)),
        },
    );
    tick(&mut h, 0.3);
    send(
        &mut h,
        cid(4),
        Request::Replan {
            instance: InstanceId(cid(1)),
        },
    );
    let op = h
        .engine
        .world()
        .operations
        .active(InstanceId(cid(1)))
        .unwrap();
    assert_eq!(op.step, original.step);
    assert_eq!(op.plan_revision, 2);
    assert_eq!(op.retries_spent, 2);
    assert_eq!(op.obstruction_revision, 1);
    assert_eq!(op.completed_work, original.completed_work);
    assert_eq!(op.last_progress_at, original.last_progress_at);
    assert_eq!(op.accepted_at, original.accepted_at);
    assert_eq!(op.recovery_deadline, original.recovery_deadline);
    send(
        &mut h,
        cid(5),
        Request::Replan {
            instance: InstanceId(cid(1)),
        },
    );
    assert_eq!(code(&h, cid(1)), "operation_retry_exhausted");
    assert_eq!(state(&h, cid(1)), ReceiptState::Interrupted);
    assert_eq!(h.engine.world().operations.active_count(), 0);
}

#[test]
fn obstruction_expiry_is_exclusive_and_finish_precedes_same_time_controls() {
    let mut h = harness();
    h.ready();
    send(&mut h, cid(1), start(worker(), "fixture0", 0.25, 0.5));
    send(
        &mut h,
        cid(2),
        Request::SetObstructed {
            instance: InstanceId(cid(1)),
            obstructed: true,
        },
    );
    for i in 1..5 {
        tick(&mut h, f64::from(i) / 10.0);
    }
    h.now = 0.5;
    send(
        &mut h,
        cid(3),
        Request::SetObstructed {
            instance: InstanceId(cid(1)),
            obstructed: false,
        },
    );
    assert_eq!(code(&h, cid(1)), "operation_recovery_exhausted");
    assert_eq!(code(&h, cid(3)), "operation_inactive");
    assert_eq!(
        h.engine
            .world()
            .operations
            .fixture("fixture0")
            .unwrap()
            .completed_units,
        0
    );
    send(&mut h, cid(4), start(worker(), "fixture0", 0.125, 1.0));
    tick(&mut h, 0.5625);
    h.now = 0.625;
    send(
        &mut h,
        cid(5),
        Request::Cancel {
            instance: InstanceId(cid(4)),
        },
    );
    assert_eq!(state(&h, cid(4)), ReceiptState::Completed);
    assert_eq!(code(&h, cid(5)), "operation_inactive");
}

#[test]
fn declaration_and_continuation_validation_refuse_unknown_or_changed_adapters() {
    let mut h = harness();
    h.ready();
    for (n, adapter) in [
        (
            1,
            AdapterDeclaration {
                name: "missing".into(),
                version: 1,
            },
        ),
        (
            2,
            AdapterDeclaration {
                name: "timed_fixture".into(),
                version: 2,
            },
        ),
    ] {
        let mut request = start(worker(), "fixture0", 1.0, 5.0);
        if let Request::Start { adapter: a, .. } = &mut request {
            *a = adapter;
        }
        send(&mut h, cid(n), request);
        assert_eq!(state(&h, cid(n)), ReceiptState::Rejected);
        assert_eq!(h.engine.world().operations.active_count(), 0);
    }
    send(&mut h, cid(3), start(worker(), "fixture0", 1.0, 5.0));
    let mut bad = config();
    bad.fixtures[0].adapter.version = 2;
    assert!(cathedral_sim::operations::OperationKernel::from_config(&bad).is_err());
    assert!(
        h.engine
            .world()
            .operations
            .validate_continuation(
                &bad,
                h.engine.world(),
                cathedral_sim::timeline::LogicalTime::new(0.0).unwrap()
            )
            .is_err()
    );
}

#[test]
fn presence_incarnation_loss_interrupts_before_work_completion() {
    let mut h = harness();
    h.ready();
    send(&mut h, cid(1), start(worker(), "fixture0", 0.2, 2.0));
    tick(&mut h, 0.1);
    h.engine
        .world_mut()
        .transition_presence(
            &[worker()],
            cathedral_sim::Presence::BeyondTheWalls,
            &Default::default(),
        )
        .unwrap();
    tick(&mut h, 0.2);
    assert_eq!(code(&h, cid(1)), "operation_incarnation_ended");
    assert_eq!(
        h.engine
            .world()
            .operations
            .fixture("fixture0")
            .unwrap()
            .completed_units,
        0
    );
    assert_eq!(h.engine.world().operations.active_count(), 0);
}

#[test]
fn operation_custody_interruption_preserves_real_prisoner_and_escort_care() {
    let mut h = harness();
    h.ready();
    send(&mut h, cid(1), start(worker(), "fixture0", 0.2, 2.0));
    tick(&mut h, 0.1);
    let at = h.engine.world().characters[&worker()].position_m();
    h.engine.world_mut().custody.seize(
        other(),
        worker(),
        None,
        cathedral_sim::custody::Station {
            place_id: cathedral_sim::PlaceId::from_raw("station"),
            name: "station".into(),
            point: at + Vec3::X * 5.0,
            stone_house: false,
        },
        0.1,
    );
    tick(&mut h, 0.2);
    assert_eq!(code(&h, cid(1)), "operation_custody");
    assert!(h.engine.world().custody.holds(&other()));
    assert!(h.engine.world().custody.is_escorting(&worker()));
    assert_eq!(
        h.engine
            .world()
            .operations
            .fixture("fixture0")
            .unwrap()
            .completed_units,
        0
    );
    tick(&mut h, 0.3);
    assert!(h.engine.world().custody.holds(&other()));
}
#[test]
fn rejected_sibling_recording_does_not_unprotect_a_live_fixture_root() {
    let mut h = harness();
    h.ready();
    let root = cid(1).operation;
    send(
        &mut h,
        root.command(0),
        start(worker(), "fixture0", 1.0, 5.0),
    );
    h.send(
        EngineCommand::PlayerRecording {
            request_id: "no-stt".into(),
            wav_basename: "missing.wav".into(),
            stt_backend: SttBackendKind::Cloud,
            position_m: PLAYER_SPAWN,
            spatial_seq: 1,
        }
        .identified(root.command(1)),
    );
    assert_eq!(state(&h, root.command(1)), ReceiptState::Rejected);
    assert!(h.engine.world().command_ledger.is_protected(root));
    assert!(
        h.engine
            .world()
            .operations
            .active(InstanceId(root.command(0)))
            .is_some()
    );
}
#[test]
fn nonzero_time_budgets_validate_continuation_and_unrepresentable_budget_refuses_before_claims() {
    let mut h = harness();
    h.ready();
    h.now = 1234.125;
    h.poll();
    send(&mut h, cid(1), start(worker(), "fixture0", 0.25, 2.0));
    assert!(
        h.engine
            .world()
            .operations
            .validate_continuation(
                &config(),
                h.engine.world(),
                cathedral_sim::timeline::LogicalTime::new(h.now).unwrap()
            )
            .is_ok()
    );
    send(
        &mut h,
        cid(2),
        Request::Cancel {
            instance: InstanceId(cid(1)),
        },
    );
    let before = h.engine.world().operations.clone();
    send(&mut h, cid(3), start(worker(), "fixture0", 1e-30, 2e-30));
    assert_eq!(code(&h, cid(3)), "operation_arguments");
    assert_eq!(h.engine.world().operations, before);
}

#[test]
fn same_root_controls_release_the_last_operation_after_their_own_ticket_finishes() {
    for exhaust in [false, true] {
        let mut h = harness();
        h.ready();
        let root = cid(1).operation;
        send(
            &mut h,
            root.command(0),
            start(worker(), "fixture0", 1.0, 5.0),
        );
        if exhaust {
            for step in 1..=3 {
                send(
                    &mut h,
                    root.command(step),
                    Request::Replan {
                        instance: InstanceId(root.command(0)),
                    },
                );
            }
            assert_eq!(code(&h, root.command(0)), "operation_retry_exhausted");
            let repeated = EngineCommand::Operation(Request::Replan {
                instance: InstanceId(root.command(0)),
            })
            .identified(root.command(3));
            h.send(repeated);
        } else {
            send(
                &mut h,
                root.command(1),
                Request::Cancel {
                    instance: InstanceId(root.command(0)),
                },
            );
            send(
                &mut h,
                root.command(1),
                Request::Cancel {
                    instance: InstanceId(root.command(0)),
                },
            );
        }
        assert_eq!(h.engine.world().operations.active_count(), 0);
        assert!(!h.engine.world().command_ledger.is_protected(root));
    }
}

#[test]
fn recording_and_fixture_siblings_protect_each_other_until_both_are_terminal() {
    for fixture_first in [false, true] {
        let stt = GenerationStt::default();
        let mut h = Builder {
            operations: config(),
            transcription: Some(Box::new(stt.clone())),
            ..Default::default()
        }
        .build();
        h.ready();
        let root = cid(1).operation;
        send(
            &mut h,
            root.command(0),
            start(worker(), "fixture0", 1.0, 5.0),
        );
        h.send(
            EngineCommand::PlayerRecording {
                request_id: "sibling".into(),
                wav_basename: "sibling.wav".into(),
                stt_backend: SttBackendKind::Cloud,
                position_m: PLAYER_SPAWN,
                spatial_seq: 1,
            }
            .identified(root.command(1)),
        );
        assert_eq!(state(&h, root.command(1)), ReceiptState::Accepted);
        let job = stt.0.borrow()[0];
        if fixture_first {
            send(
                &mut h,
                root.command(2),
                Request::Cancel {
                    instance: InstanceId(root.command(0)),
                },
            );
            assert!(h.engine.world().command_ledger.is_protected(root));
        }
        h.send(EngineCommand::Transcription(
            cathedral_sim::TranscriptionOutcome::Done {
                job,
                result: Ok("A brief statement.".into()),
            },
        ));
        assert_eq!(state(&h, root.command(1)), ReceiptState::Completed);
        if !fixture_first {
            assert!(h.engine.world().command_ledger.is_protected(root));
            send(
                &mut h,
                root.command(2),
                Request::Cancel {
                    instance: InstanceId(root.command(0)),
                },
            );
        }
        assert!(!h.engine.world().command_ledger.is_protected(root));
        let receipt = h
            .engine
            .world()
            .command_ledger
            .get(root.command(1))
            .unwrap()
            .clone();
        h.send(EngineCommand::Transcription(
            cathedral_sim::TranscriptionOutcome::Done {
                job,
                result: Ok("A duplicate.".into()),
            },
        ));
        assert_eq!(
            h.engine.world().command_ledger.get(root.command(1)),
            Some(&receipt)
        );
        assert!(!h.engine.world().command_ledger.is_protected(root));
    }
}
