//! Joint preparation of real pending owners, still quarantined before M3.
use super::*;
use crate::{Capabilities, Cognition, RuntimeGeneration, Sight, Transcription, Tts};
use std::path::PathBuf;

/// New roots, bounded queue/vector growth and at most eight receipt advances
/// (including exact-copy, update-index and serialization scratch). Existing
/// prompts/input buffers move. This joins the unchanged complete typed ceiling.
pub const CONTINUATION_STRUCTURAL_BYTES: usize = 128 * 1024;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ContinuationReport {
    pub scheduler_retries: usize,
    pub scheduler_held: bool,
    pub scheduler_resumed: bool,
    pub night_retry: bool,
    pub night_held: bool,
    pub interrupted_inputs: usize,
    pub interruption_receipts: usize,
    pub released_audio_waits: usize,
    pub typed_upper_bytes: usize,
    pub services_bound: bool,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub enum ContinuationStage {
    Admission,
    Inputs,
    Speech,
    Floor,
    Prepared,
    Services,
}

/// The host factory supplies handles already scoped to this immutable runtime
/// generation and their current capabilities. This does not allocate backend
/// threads/devices, query availability, or submit a request in cathedral-sim.
/// The trusted allowance includes owned/shared service storage and factory work;
/// M3 must additionally retain the actual backend runtime/generation lease.
pub struct ContinuationServices {
    pub generation: RuntimeGeneration,
    pub cognition: Box<dyn Cognition>,
    pub transcription: Box<dyn Transcription>,
    pub tts: Box<dyn Tts>,
    pub sight: Box<dyn Sight>,
    pub capabilities: Capabilities,
    pub runtime_dir: PathBuf,
}

/// Actual prepared owners. No public Engine/World borrow, extraction or poll.
/// Host time remains unbound; retained Host is the saved authoritative boundary.
pub struct PreparedContinuation {
    pub(crate) engine: Engine,
    host: super::super::host::HostCandidate,
    boundary: LogicalTime,
    world_identity: WorldIdentity,
    report: ContinuationReport,
    // All service, asset and semantic owners above die before these leases.
    asset_reservation: Reservation,
    service_reservation: Option<Reservation>,
}
impl Admitted<HydratedEngine> {
    pub fn prepare_continuation(self) -> Result<Admitted<PreparedContinuation>> {
        self.prepare_continuation_observed(&mut |_| {})
    }
    pub fn prepare_continuation_observed(
        self,
        observer: &mut impl FnMut(ContinuationStage),
    ) -> Result<Admitted<PreparedContinuation>> {
        self.try_map_mut(|mut h, r| {
            r.require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
            let typed = h
                .cost
                .decoded_upper_bytes
                .checked_add(CONTINUATION_STRUCTURAL_BYTES)
                .filter(|n| *n <= meter::MAX_EXPANSION)
                .ok_or_else(|| error("continuation typed expansion limit exceeded"))?;
            r.resize(typed.max(r.bytes()))?;
            h.engine.preflight_continuation(&h.cognition, &h.speech)?;
            observer(ContinuationStage::Admission);
            let report = h.engine.prepare_pending_work(
                h.speech,
                h.host.records(),
                h.boundary,
                typed,
                observer,
            )?;
            let prepared = PreparedContinuation {
                engine: h.engine,
                host: h.host,
                boundary: h.boundary,
                world_identity: h.world_identity,
                report,
                asset_reservation: h.asset_reservation,
                service_reservation: None,
            };
            drop(h.cognition);
            observer(ContinuationStage::Prepared);
            Ok(prepared)
        })
    }
}
impl Admitted<PreparedContinuation> {
    pub fn bind_services(
        self,
        upper_bytes: usize,
        factory: impl FnOnce(RuntimeGeneration) -> Result<ContinuationServices>,
    ) -> Result<Admitted<PreparedContinuation>> {
        self.try_map(|mut prepared, r| {
            check(
                !prepared.report.services_bound,
                "continuation services already bound",
            )?;
            prepared.service_reservation = Some(r.sublease(upper_bytes)?);
            // Explicit owner order protects handles on mismatch/error/unwind.
            struct Owner {
                services: Option<ContinuationServices>,
                prepared: PreparedContinuation,
            }
            let mut owner = Owner {
                services: None,
                prepared,
            };
            owner.services = Some(factory(owner.prepared.runtime_generation())?);
            check(
                owner.services.as_ref().unwrap().generation == owner.prepared.runtime_generation(),
                "continuation service generation mismatch",
            )?;
            let services = owner.services.take().unwrap();
            owner.prepared.engine.bind_continuation_services(services);
            owner.prepared.report.services_bound = true;
            Ok(owner.prepared)
        })
    }
}
impl PreparedContinuation {
    /// Save the exact prepared instant without adopting, polling, or exposing
    /// mutable World authority. Host records retain their original boundary;
    /// new interruption receipts are serialized by the speech V2 owner.
    pub fn capture(
        &self,
        profile: CheckpointProfile,
        mut r: Reservation,
    ) -> Result<Admitted<CompleteCheckpointCandidate>> {
        r.require_shared(&self.asset_reservation)?;
        r.require(Cohort::SavePayload, 4096)?;
        r.require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
        if r.bytes() < meter::VALIDATION_SCRATCH {
            r.resize(meter::VALIDATION_SCRATCH)?;
        }
        let definitions = InstalledCheckpointDefinitions::from_engine(
            &self.engine,
            self.host.scalars().definitions,
            self.boundary,
        )?;
        let mut count = wire::Count::new(profile.limit());
        wire::write_envelope_with_host(
            &self.engine,
            profile,
            self.world_identity,
            self.boundary,
            &definitions.manifest,
            &mut count,
            &mut r,
            |w| self.host.complete_write_retained(w),
        )?;
        let (len, digest) = count.finish();
        r.resize(meter::VALIDATION_SCRATCH + len)?;
        let mut output = wire::Output::new(len);
        wire::write_envelope_with_host(
            &self.engine,
            profile,
            self.world_identity,
            self.boundary,
            &definitions.manifest,
            &mut output,
            &mut r,
            |w| self.host.complete_write_retained(w),
        )?;
        let bytes = output.finish()?;
        check(
            hash_bytes(&bytes) == digest,
            "prepared checkpoint encoding disagreement",
        )?;
        let proof = validate_raw(&bytes, bytes.capacity(), &mut r, &definitions, &mut |_| {})?;
        drop(definitions);
        let candidate = CompleteCheckpointCandidate {
            bytes,
            offsets: proof.offsets,
            world_identity: proof.world_identity,
            boundary: proof.boundary,
            manifest: proof.manifest,
            cost: proof.cost,
        };
        r.resize(candidate.cost.retained_candidate_bytes.max(1))?;
        Ok(Admitted::new(candidate, r))
    }
    pub fn boundary(&self) -> LogicalTime {
        self.boundary
    }
    pub fn runtime_generation(&self) -> RuntimeGeneration {
        self.engine.config().runtime_generation
    }
    pub fn world_identity(&self) -> WorldIdentity {
        self.world_identity
    }
    pub fn host(&self) -> &super::super::host::HostCandidate {
        &self.host
    }
    pub fn report(&self) -> ContinuationReport {
        self.report
    }
    pub fn interrupted_speech(&self) -> &[crate::speech_router::checkpoint::InterruptedSpeech] {
        self.engine.interrupted_speech()
    }
    pub fn protects_operation(&self, id: crate::receipts::OperationId) -> bool {
        self.engine.world().command_ledger.is_protected(id)
    }
    pub fn category_digest(
        &self,
        category: CheckpointCategory,
        mut r: Reservation,
    ) -> Result<Admitted<[u8; 32]>> {
        r.require_shared(&self.asset_reservation)?;
        r.require(Cohort::SavePayload, 4096)?;
        if r.bytes() < meter::VALIDATION_SCRATCH {
            r.resize(meter::VALIDATION_SCRATCH)?;
        }
        let mut sink = wire::Count::new(super::super::POPULATED_PAYLOAD_BYTES);
        if category == CheckpointCategory::Host {
            self.host.complete_write_retained(&mut sink)?;
        } else {
            self.engine
                .complete_write_category(category, self.boundary, &mut sink, &mut r)?;
        }
        Ok(Admitted::new(sink.finish().1, r))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn continuation_structural_admission_bounds_growth_and_receipt_working_storage() {
        use std::mem::size_of;
        // Decoded old queue capacities remain charged by the original meter.
        // One push can allocate at most 128 new slots; stable sorting <=64
        // elements gets another full 64-slot scratch allocation.
        let retries = 192 * size_of::<crate::scheduler::continuation::LoadRetry>();
        // Archived groups are nonempty, so the input count cap also limits
        // groups. One push grows at most to 128 slots, in addition to old storage.
        let speech = 128 * size_of::<crate::speech_router::checkpoint::InterruptedSpeech>();
        // <=8 accepted recordings, each <=1024-byte original ledger entry;
        // allow three 2048-byte buffers per row (advance serialization/clone,
        // drain clone, update tree/vector slots) despite sequential advance.
        let receipts = 8 * 3 * 2048;
        let wrappers = 4096;
        let total = retries + speech + receipts + wrappers;
        println!(
            "continuation structural retries={retries} speech={speech} receipts={receipts} wrappers={wrappers} total={total} bound={}",
            super::CONTINUATION_STRUCTURAL_BYTES
        );
        assert!(total <= super::CONTINUATION_STRUCTURAL_BYTES);
    }
}
