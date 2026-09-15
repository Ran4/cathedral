use super::*;
use cathedral_backends::{LocalTime, checkpoint_preparation::CheckpointPreparation};
use cathedral_sim::checkpoint::{CheckpointBudget, Cohort};
use std::{
    thread::ThreadId,
    time::{Duration, Instant},
};

struct DelayedClockDrop {
    entered: Sender<ThreadId>,
    release: Receiver<()>,
}
impl Drop for DelayedClockDrop {
    fn drop(&mut self) {
        self.entered.send(std::thread::current().id()).unwrap();
        self.release.recv_timeout(Duration::from_secs(20)).unwrap();
    }
}
fn eventually(mut check: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    while !check() {
        assert!(Instant::now() < end, "retirement deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn real_local_retirement_drains_queues_flushes_promptlog_and_pins_external_owners() {
    let budget = Arc::new(CheckpointBudget::default());
    // Generous fixture allowance, not a populated production heap census.
    let running = budget.reserve(Cohort::Running, 512 * 1024 * 1024).unwrap();
    let service = CheckpointPreparation::start(budget.clone()).unwrap();
    let config = SmartActorsConfig {
        fake_backend: true,
        tts_backend: "off".into(),
        ..Default::default()
    };
    let (handle, inbox, guard, mut engine) = spawn(&config, &WeatherSettings::default());
    handle
        .try_send(BridgeCommand::Hello {
            position_m: Position {
                x: 0.0,
                y: 0.91,
                z: 111.0,
            },
            spatial_seq: 1,
        })
        .unwrap();
    engine.pump(0.0);
    assert!(engine.engine.is_some());
    let fake = Arc::downgrade(&engine.fake_cognition.as_ref().unwrap().0);
    let session = handle.runtime_dir().to_path_buf();
    assert!(session.is_dir());
    let external_command = handle.command_sender();
    let external_callback = engine.completions.as_ref().unwrap().clone();
    let publication_bytes = engine.publication_bytes.clone();
    let publication = match inbox.try_recv().unwrap() {
        BridgeEvent::InGeneration { allocation, .. } => allocation,
        _ => panic!("actual publication envelope"),
    };
    let external_bytes = publication_bytes.load(Ordering::Acquire);
    assert!(external_bytes > 0);
    external_command
        .try_send(BridgeCommand::PlayerAudioChunk {
            wav_basename: "retire.wav".into(),
            seq: 0,
            samples: vec![7; 2048].into(),
        })
        .unwrap();
    let command_queue = engine.commands.clone();
    let (entered, waiting) = bounded(1);
    let (release, resume) = bounded(1);
    let delay = DelayedClockDrop {
        entered,
        release: resume,
    };
    let prompt_dir = session.join("retirement-prompts");
    engine.prompt_log = PromptLog::with_clock(
        Some(prompt_dir.clone()),
        Some("fake".into()),
        Box::new(move || {
            let _keep = &delay;
            LocalTime {
                year: 2026,
                month: 9,
                day: 15,
                hour: 12,
                minute: 0,
                second: 0,
            }
        }),
    );
    engine.prompt_log.record(PromptExchange {
        actor_id: "probe".into(),
        actor_name: "Retirement".into(),
        prompt: "retired prompt must flush".into(),
        answer: Some("wait {}".into()),
        duration_seconds: 0.0,
        error: None,
    });
    println!(
        "retirement fixture inventory={} assumptions=authored cast, fake cognition, no provider/device startup, embedded navigation, default weather/knowledge/marks, off TTS; trusted_bound={} is not an independent heap measurement; global writer/runtime closure remains M3b2",
        engine.checkpoint_storage_inventory(),
        160 * 1024 * 1024
    );
    let permit = service.reserve_retirement(160 * 1024 * 1024).unwrap();
    let observer = permit.lease().release_observer();
    let begin = Instant::now();
    let id = engine
        .retire_to_worker(guard, handle, inbox, permit)
        .unwrap_or_else(|(e, ..)| panic!("{e}"));
    let detach_seconds = begin.elapsed().as_secs_f64();
    assert_ne!(
        waiting.recv_timeout(Duration::from_secs(20)).unwrap(),
        std::thread::current().id()
    );
    assert!(command_queue.is_empty());
    drop(command_queue);
    assert!(publication_bytes.load(Ordering::Acquire) < external_bytes);
    assert_eq!(
        std::fs::read_dir(&prompt_dir).unwrap().count(),
        2,
        "real PromptLog flush preceded clock Drop"
    );
    assert!(fake.upgrade().is_some());
    assert!(!service.retirement_status(id).unwrap().owners_disposed);
    assert!(!observer.released());
    assert!(
        external_command
            .try_send(BridgeCommand::PlayerAudioChunk {
                wav_basename: "late.wav".into(),
                seq: 0,
                samples: vec![1; 1024].into()
            })
            .is_err()
    );
    release.send(()).unwrap();
    eventually(|| service.retirement_status(id).unwrap().owners_disposed);
    assert!(fake.upgrade().is_none());
    assert!(!session.exists(), "real SessionDir cleaned on worker");
    assert!(!service.retirement_status(id).unwrap().released);
    drop(external_callback);
    drop(external_command);
    assert!(
        !observer.released(),
        "external publication still retains cohort"
    );
    drop(publication);
    eventually(|| service.retirement_status(id).unwrap().released);
    let status = service.take_retirement_result(id).unwrap();
    println!(
        "local retirement detach_seconds={detach_seconds} worker={:?}; injected PromptLog destructor gate included",
        status.disposal_seconds
    );
    assert!(status.owners_disposed && !status.panicked);
    assert_eq!(publication_bytes.load(Ordering::Acquire), 0);
    assert!(service.join().is_ok());
    assert_eq!(budget.retained_bytes(), running.bytes());
}

#[test]
fn retirement_generation_refusal_returns_whole_local_bundle_unchanged() {
    let budget = Arc::new(CheckpointBudget::default());
    let _running = budget.reserve(Cohort::Running, 512 * 1024 * 1024).unwrap();
    let service = CheckpointPreparation::start(budget).unwrap();
    let config = SmartActorsConfig {
        fake_backend: true,
        ..Default::default()
    };
    let (handle, inbox, guard, mut engine) = spawn(&config, &WeatherSettings::default());
    handle
        .try_send(BridgeCommand::Hello {
            position_m: Position {
                x: 0.0,
                y: 0.91,
                z: 111.0,
            },
            spatial_seq: 1,
        })
        .unwrap();
    engine.pump(0.0);
    let (commands, _) = bounded(1);
    let wrong = BridgeHandle::new_for_generation(
        commands,
        PathBuf::new(),
        cathedral_sim::RuntimeGeneration(0),
    );
    let permit = service.reserve_retirement(160 * 1024 * 1024).unwrap();
    let Err((_, mut engine, guard, wrong, inbox, permit)) =
        engine.retire_to_worker(guard, wrong, inbox, permit)
    else {
        panic!("mismatch")
    };
    drop(wrong);
    engine.pump(0.5);
    assert!(!engine.dead);
    let id = engine
        .retire_to_worker(guard, handle, inbox, permit)
        .unwrap_or_else(|(e, ..)| panic!("{e}"));
    eventually(|| service.take_retirement_result(id).is_some());
    assert!(service.join().is_ok());
}
