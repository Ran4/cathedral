//! Complete-engine continuation test seam; never compiled into the public API.
//! The actual prepared owner and its asset/service/typed leases remain together.
//! `host` is an explicit CPU fixture host. It starts as the saved Host and advances
//! its own clocks after ordinary bounded polls. The quarantined owner's retained
//! Host and boundary stay untouched; this does not implement M3 host adoption.
use super::*;

use crate::checkpoint::{
    Admitted, CheckpointBudget, Cohort, HostTimeV1, Reservation,
    complete::{
        self, CheckpointProfile, CompleteCheckpointInput, ContinuationServices,
        InstalledCheckpointDefinitions, PreparedContinuation, WorldIdentity,
    },
    host::{FrameV1, HostCheckpointSource, Nullable, RecordRef, RecordV1, ScalarsV1},
};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct Host(pub(crate) ScalarsV1, Vec<RecordV1<String>>);
impl HostCheckpointSource for Host {
    fn scalars(&self) -> crate::checkpoint::Result<ScalarsV1> {
        Ok(self.0)
    }
    fn records(
        &self,
        f: &mut dyn FnMut(RecordRef<'_>) -> crate::checkpoint::Result<()>,
    ) -> crate::checkpoint::Result<()> {
        self.complete_records(f)
    }
    fn complete_records<'a>(
        &'a self,
        f: &mut dyn FnMut(RecordRef<'a>) -> crate::checkpoint::Result<()>,
    ) -> crate::checkpoint::Result<()> {
        use RecordV1 as R;
        for row in &self.1 {
            f(match row {
                R::Draft { text } => R::Draft { text },
                R::SelectedItem { item } => R::SelectedItem {
                    item: Nullable(item.0.as_deref()),
                },
                R::Hud {
                    slot,
                    text,
                    remaining,
                } => R::Hud {
                    slot: *slot,
                    text,
                    remaining: *remaining,
                },
                R::ChalkHold {
                    intent: Nullable(None),
                } => R::ChalkHold {
                    intent: Nullable(None),
                },
                R::ChalkPen { present } => R::ChalkPen { present: *present },
                R::ChalkAnchor {
                    handle,
                    label,
                    kinds,
                } => R::ChalkAnchor {
                    handle,
                    label,
                    kinds: *kinds,
                },
                R::Journal { attribution, word } => R::Journal { attribution, word },
                R::JournalStanding { text } => R::JournalStanding { text },
                R::Notice {
                    id,
                    line,
                    rung,
                    clears_when,
                } => R::Notice {
                    id: *id,
                    line,
                    rung: *rung,
                    clears_when,
                },
                R::Holder { actor } => R::Holder { actor },
                R::Custody {
                    officer,
                    officer_name,
                    station_name,
                    anchor,
                    closing,
                    strain_seconds,
                    held,
                    committed,
                    fee_sparks,
                    release_office,
                    booked_as,
                } => R::Custody {
                    officer: Nullable(officer.0.as_deref()),
                    officer_name,
                    station_name,
                    anchor: *anchor,
                    closing: *closing,
                    strain_seconds: *strain_seconds,
                    held: *held,
                    committed: *committed,
                    fee_sparks: *fee_sparks,
                    release_office: Nullable(release_office.0.as_deref()),
                    booked_as: Nullable(booked_as.0.as_deref()),
                },
                _ => panic!("unsupported CPU fixture Host record"),
            })?;
        }
        Ok(())
    }
    fn validate_boundary(&self) -> crate::checkpoint::Result<()> {
        Ok(())
    }
}
impl Host {
    fn refresh_publications(&mut self, e: &Engine) {
        use RecordV1 as R;
        self.1.retain(|r| {
            !matches!(
                r,
                R::Journal { .. }
                    | R::JournalStanding { .. }
                    | R::Notice { .. }
                    | R::Custody { .. }
                    | R::Holder { .. }
                    | R::ChalkPen { .. }
                    | R::ChalkAnchor { .. }
            )
        });
        if let Some(EngineMessage::Journal { entries, standing }) = &e.last_journal {
            for entry in entries {
                let (attribution, word) = crate::checkpoint::host::resolved_journal_row(entry);
                self.1.push(R::Journal { attribution, word });
            }
            self.1.extend(
                standing
                    .iter()
                    .cloned()
                    .map(|text| R::JournalStanding { text }),
            );
        }
        if let Some(EngineMessage::LawStanding { notices, custody }) = &e.last_law_standing {
            if let Some(p) = custody {
                self.1.push(R::Custody {
                    officer: Nullable(p.officer_id.as_ref().map(ToString::to_string)),
                    officer_name: p.officer_name.clone(),
                    station_name: p.station_name.clone(),
                    anchor: [
                        p.anchor_m.x as f32,
                        p.anchor_m.y as f32,
                        p.anchor_m.z as f32,
                    ],
                    closing: p.closing,
                    strain_seconds: p.strain_seconds as f32,
                    held: p.held,
                    committed: p.committed,
                    fee_sparks: p.fee_sparks,
                    release_office: Nullable(p.release_office.clone()),
                    booked_as: Nullable(p.booked_as.clone()),
                });
                self.1.extend(p.holder_ids.iter().map(|id| R::Holder {
                    actor: id.to_string(),
                }));
            }
            self.1.extend(notices.iter().map(|n| R::Notice {
                id: n.notice_id,
                line: n.line.clone(),
                rung: n.rung.into(),
                clears_when: n.clears_when.clone(),
            }));
        }
        if let Some(EngineMessage::ChalkStanding { pen, anchors }) = &e.last_chalk_standing {
            self.1.push(R::ChalkPen { present: *pen });
            for a in anchors {
                let mut kinds = [Nullable(None); 3];
                for (to, from) in kinds.iter_mut().zip(&a.kinds) {
                    to.0 = Some((*from).into());
                }
                self.1.push(R::ChalkAnchor {
                    handle: a.handle.clone(),
                    label: a.label.clone(),
                    kinds,
                });
            }
        } else {
            self.1.push(R::ChalkPen { present: false });
        }
    }
}

pub(crate) struct Unavailable;
impl Cognition for Unavailable {
    fn request(
        &mut self,
        _: String,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        Err(crate::CognitionBusy)
    }
}
pub(crate) fn fixture(mut config: EngineConfig, cognition: Box<dyn Cognition>) -> Engine {
    if config.nav.is_none() {
        config.nav = Some(crate::dogs::checkpoint::tests::nav());
    }
    config.checkpoint_host_image = Some([7; 32]);
    config.checkpoint_world_identity = Some(WorldIdentity::from_bytes([9; 16]).unwrap());
    let seed = crate::WorldSeed::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/demo_seed.json"
    )))
    .unwrap();
    Engine::new(
        config,
        &seed,
        AreaMap::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/world/areas.json"
        )))
        .unwrap(),
        SoundCatalog::from_toml_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sounds/catalog.toml"
        )))
        .unwrap(),
        crate::night::checkpoint::tests::env(),
        cognition,
        Box::new(crate::NullTranscription),
        Box::new(crate::NullTts),
        Box::new(crate::NullSight),
        Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(0.0, f64::from(0.91_f32), 111.0), 0.0),
        0,
        0.0,
    )
    .unwrap()
}
pub(crate) fn initial_host(e: &Engine) -> Host {
    let old = super::complete_tests::host(e);
    let mut records = Vec::new();
    old.complete_records(&mut |r| {
        records.push(serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap());
        Ok(())
    })
    .unwrap();
    let mut h = Host(old.0, records);
    h.refresh_publications(e);
    h.0.clock.seconds_per_day = e.clock.seconds_per_day();
    h.0.boundary.generation = e.config.runtime_generation.0.max(1);
    h
}
impl Host {
    /// CPU host fixture: preserve all saved records/counters/debt, advance only
    /// accepted clocks, the sampled physical body, command watermark and clock
    /// projection. No controller integration or unread-presentation claim.
    pub(crate) fn after_poll(&mut self, e: &Engine, now: f64) {
        let old = self.0.time.accepted.accepted().unwrap();
        let elapsed = Duration::from_secs_f64(now);
        assert_eq!(
            now.to_bits(),
            elapsed.as_secs_f64().to_bits(),
            "poll must use accepted Duration seconds"
        );
        let delta = elapsed.checked_sub(old.elapsed).unwrap();
        assert!(
            delta <= crate::timeline::MAX_ACCEPTED_FRAME,
            "unbounded fixture time step"
        );
        assert!(
            old.debt.is_zero() || delta == crate::timeline::MAX_ACCEPTED_FRAME,
            "nonzero debt needs a full accepted step in this fixture host"
        );
        let step = self.0.time.accepted.fixed_step().unwrap();
        let residual = self.0.time.accepted.fixed_residual().unwrap() + delta;
        let residual = Duration::from_nanos((residual.as_nanos() % step.as_nanos()) as u64);
        let accepted = crate::timeline::AcceptedTime {
            elapsed,
            debt: old.debt,
            wall: old.wall + delta,
        };
        self.0.time.accepted =
            HostTimeV1::from_accepted(accepted, step, residual, now - e.movement_now).unwrap();
        self.0.time.virtual_elapsed = elapsed;
        self.0.time.virtual_delta = delta;
        self.0.time.fixed_elapsed = elapsed - residual;
        self.0.time.fixed_delta = step.min(delta);
        self.0.time.last_frame = Nullable(Some(FrameV1 {
            wall_delta: delta,
            accepted_delta: delta,
            elapsed,
            debt: old.debt,
        }));
        self.0.controller.previous = self.0.controller.current;
        let p = e.world.characters[&e.config.player_id].position_m();
        self.0.controller.current = [p.x as f32, p.y as f32, p.z as f32];
        self.0.controller.yaw = e.world.characters[&e.config.player_id].state.facing_yaw as f32;
        self.0.spatial.sequence = e.world.spatial_sequence as u64;
        self.0.spatial.last_position = Nullable(Some(self.0.controller.current));
        self.0.spatial.last_yaw = Nullable(Some(self.0.controller.yaw));
        self.0.boundary.physical_sequence = self.0.spatial.sequence;
        self.0.boundary.issued = self.0.boundary.issued.max(
            e.world.command_ledger.producers[crate::receipts::HOST_PRODUCER as usize].high_water,
        );
        let time = e.clock.at(now);
        self.0.clock.day = time.day;
        self.0.clock.fraction = time.fraction;
        self.0.clock.office = time.office.into();
        self.0.clock.weekday = time.weekday.into();
        self.0.clock.brightness = e.clock.brightness(now);
        self.0.clock.scale = e.clock.scale();
        self.refresh_publications(e);
    }
}
pub(crate) fn wire(e: &Engine, h: &Host, budget: &CheckpointBudget) -> Value {
    let saved = complete::capture(
        e,
        h,
        CheckpointProfile::Authored,
        budget.reserve(Cohort::SavePayload, 4096).unwrap(),
    )
    .unwrap();
    serde_json::from_slice(saved.value().bytes()).unwrap()
}
pub(crate) fn assert_wire_eq(mut a: Value, mut b: Value) {
    // The ONLY ordinary full-equality exclusion is the new execution fence.
    // Runtime generation is serialized exactly here; no recursive name filter.
    for v in [&mut a, &mut b] {
        *v.pointer_mut("/host/scalars/boundary/generation").unwrap() = Value::from(1);
    }
    for (key, expected) in a.as_object().unwrap() {
        let expected = serde_json::to_vec(expected).unwrap();
        let actual = serde_json::to_vec(&b[key]).unwrap();
        if expected != actual {
            let at = expected
                .iter()
                .zip(&actual)
                .position(|(a, b)| a != b)
                .unwrap_or(expected.len().min(actual.len()));
            let context = |bytes: &[u8]| {
                String::from_utf8_lossy(&bytes[at.saturating_sub(96)..(at + 96).min(bytes.len())])
                    .into_owned()
            };
            panic!(
                "complete owner {key}: expected {} bytes, actual {} bytes, first difference at byte {at}; bounded expected {:?}, actual {:?}",
                expected.len(),
                actual.len(),
                context(&expected),
                context(&actual),
            );
        }
    }
    assert_eq!(a.as_object().unwrap().len(), b.as_object().unwrap().len());
}
pub(crate) struct Restored {
    prepared: Option<Admitted<PreparedContinuation>>,
    pub(crate) host: Host,
    pub(crate) budget: CheckpointBudget,
    _running: Reservation,
}
impl Restored {
    pub(crate) fn new(e: &Engine, h: &Host, cognition: Box<dyn Cognition>) -> Self {
        let budget = CheckpointBudget::default();
        let running = budget
            .reserve(Cohort::Running, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
            .unwrap();
        let saved = complete::capture(
            e,
            h,
            CheckpointProfile::Authored,
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
        let input = CompleteCheckpointInput::copy_from(
            saved.value().bytes(),
            budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
        )
        .unwrap();
        drop(saved);
        let now = crate::timeline::LogicalTime::new(
            h.0.time.accepted.accepted().unwrap().elapsed.as_secs_f64(),
        )
        .unwrap();
        let definitions =
            InstalledCheckpointDefinitions::from_engine(e, h.0.definitions, now).unwrap();
        let validated = complete::validate(input, &definitions).unwrap();
        drop(definitions);
        let generation = crate::RuntimeGeneration(h.0.boundary.generation + 1);
        let prepared = validated
            .prepare_hydration(64 * 1024 * 1024)
            .unwrap()
            .hydrate(
                || super::complete_tests::assets(e),
                h.0.definitions,
                generation,
            )
            .unwrap()
            .prepare_continuation()
            .unwrap()
            .bind_services(4096, |generation| {
                Ok(ContinuationServices {
                    generation,
                    cognition,
                    transcription: Box::new(crate::NullTranscription),
                    tts: Box::new(crate::NullTts),
                    sight: Box::new(crate::NullSight),
                    capabilities: e.capabilities,
                    runtime_dir: e.config.runtime_dir.clone(),
                })
            })
            .unwrap();
        // Exact saved Host remains inside prepared. The explicit fixture host is
        // bound here to the new execution fence, without reading any wall clock.
        let mut host = Host(
            *prepared.value().host().scalars(),
            prepared.value().host().records().to_vec(),
        );
        host.0.boundary.generation = generation.0;
        Self {
            prepared: Some(prepared),
            host,
            budget,
            _running: running,
        }
    }
    pub(crate) fn engine(&self) -> &Engine {
        &self.prepared.as_ref().unwrap().value().engine
    }
    pub(crate) fn with_engine<R>(&mut self, f: impl FnOnce(&mut Engine) -> R) -> R {
        let mut output = None;
        self.prepared = Some(
            self.prepared
                .take()
                .unwrap()
                .try_map(|mut p, _| {
                    output = Some(f(&mut p.engine));
                    Ok(p)
                })
                .unwrap(),
        );
        output.unwrap()
    }
    pub(crate) fn poll(&mut self, now: f64, commands: Vec<EngineCommand>) -> Vec<EngineMessage> {
        let events = self.with_engine(|e| e.poll(now, commands));
        let mut host = self.host.clone();
        host.after_poll(self.engine(), now);
        self.host = host;
        events
    }
    pub(crate) fn assert_eq(&self, control: &Engine, host: &Host) {
        assert_wire_eq(
            wire(control, host, &self.budget),
            wire(self.engine(), &self.host, &self.budget),
        );
    }
    pub(crate) fn wire(&self) -> Value {
        wire(self.engine(), &self.host, &self.budget)
    }
}
