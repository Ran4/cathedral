//! Complete closed checkpoint validation, without Engine hydration or adoption.
//! A candidate owns one immutable raw envelope; every private component and its
//! references have been validated against that same envelope before admission.
pub(crate) mod manifest;
pub(crate) mod meter;
pub(crate) mod wire;
use super::{Admitted, CheckpointError, Cohort, Reservation, Result};
use crate::{Engine, timeline::LogicalTime};
pub use manifest::{CompleteManifestV1, InstalledCheckpointDefinitions};
use serde::{Deserialize, Serialize};
use std::io::Write;

/// Minimum reservation for the scoped complete-checkpoint caller contract.
/// This is not a universal Engine/ECS heap bound. The trusted host must keep
/// Running charged for its actual retained authority (including spare capacity),
/// reserve more when required, and retain this owner through capture/validation.
/// M3 additionally owns ECS/assets/backends and retiring-generation admission.
pub const RUNNING_AUTHORITY_ALLOWANCE_BYTES: usize = 512 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorldIdentity([u8; 16]);
impl WorldIdentity {
    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self> {
        check(bytes != [0; 16], "world lineage identity is unavailable")?;
        Ok(Self(bytes))
    }
    pub fn bytes(self) -> [u8; 16] {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointProfile {
    Authored,
    Populated,
}
impl<'de> Deserialize<'de> for CheckpointProfile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Profile;
        impl serde::de::Visitor<'_> for Profile {
            type Value = CheckpointProfile;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a supported complete profile string")
            }
            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> std::result::Result<Self::Value, E> {
                match v {
                    "authored" => Ok(CheckpointProfile::Authored),
                    "populated" => Ok(CheckpointProfile::Populated),
                    _ => Err(E::custom("unsupported complete profile")),
                }
            }
        }
        d.deserialize_str(Profile)
    }
}
impl CheckpointProfile {
    pub(crate) fn limit(self) -> usize {
        match self {
            Self::Authored => super::AUTHORED_PAYLOAD_BYTES,
            Self::Populated => super::POPULATED_PAYLOAD_BYTES,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointCategory {
    Ledger,
    Operations,
    Backbone,
    Round,
    Climate,
    Knowledge,
    Law,
    Marks,
    Animals,
    Social,
    Continuity,
    Scheduler,
    Night,
    Speech,
    CognitionInputs,
    Host,
}
impl CheckpointCategory {
    pub(crate) const ALL: [Self; 16] = [
        Self::Ledger,
        Self::Operations,
        Self::Backbone,
        Self::Round,
        Self::Climate,
        Self::Knowledge,
        Self::Law,
        Self::Marks,
        Self::Animals,
        Self::Social,
        Self::Continuity,
        Self::Scheduler,
        Self::Night,
        Self::Speech,
        Self::CognitionInputs,
        Self::Host,
    ];
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ledger => "ledger",
            Self::Operations => "operations",
            Self::Backbone => "backbone",
            Self::Round => "round",
            Self::Climate => "climate",
            Self::Knowledge => "knowledge",
            Self::Law => "law",
            Self::Marks => "marks",
            Self::Animals => "animals",
            Self::Social => "social",
            Self::Continuity => "continuity",
            Self::Scheduler => "scheduler",
            Self::Night => "night",
            Self::Speech => "speech",
            Self::CognitionInputs => "cognition_inputs",
            Self::Host => "host",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CompleteCheckpointCost {
    pub encoded_bytes: usize,
    pub raw_capacity_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub category_expansion_upper_bytes: [usize; 16],
    pub validation_scratch_bytes: usize,
    pub diagnostic_scratch_bytes: usize,
    pub validation_peak_bytes: usize,
    pub retained_candidate_bytes: usize,
    pub characters: usize,
    pub categories: usize,
}
pub struct CompleteCheckpointInput {
    pub(crate) bytes: Vec<u8>,
    pub(crate) reservation: Reservation,
}
impl CompleteCheckpointInput {
    /// Reserve before constructing installed definition metadata in the host.
    /// The resolver streams borrowed definitions and allocates only bounded
    /// manifest text/config scalars; it shares the complete scratch allowance.
    pub fn prepare_definition_resolution(&mut self) -> Result<()> {
        self.reservation
            .require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
        let wanted = self
            .bytes
            .capacity()
            .checked_add(meter::VALIDATION_SCRATCH)
            .ok_or_else(|| error("resolver admission overflow"))?;
        if self.reservation.bytes() < wanted {
            self.reservation.resize(wanted)?;
        }
        Ok(())
    }

    /// The caller reserves before reading/allocating. Actual capacity, including
    /// spare allocation beyond len, is checked; this is not retrospective IO
    /// admission. No input storage can be detached from its cohort afterwards.
    pub fn from_owned(bytes: Vec<u8>, reservation: Reservation) -> Result<Self> {
        let input = Self { bytes, reservation };
        input.reservation.require(
            Cohort::LoadCandidate,
            input.bytes.capacity().saturating_add(4096),
        )?;
        check(
            input.bytes.len() <= super::POPULATED_PAYLOAD_BYTES,
            "encoded complete payload limit exceeded",
        )?;
        Ok(input)
    }
    /// Copies a borrowed slice after reserving the new allocation and readable
    /// slice bytes. Any excess backing capacity remains the caller's separately
    /// owned allocation; primary host file loading uses from_owned above.
    pub fn copy_from(bytes: &[u8], mut reservation: Reservation) -> Result<Self> {
        reservation.require(Cohort::LoadCandidate, 4096)?;
        check(
            bytes.len() <= super::POPULATED_PAYLOAD_BYTES,
            "encoded complete payload limit exceeded",
        )?;
        let needed = bytes.len().saturating_mul(2).saturating_add(4096);
        if reservation.bytes() < needed {
            reservation.resize(needed)?;
        }
        let mut owned = Vec::with_capacity(bytes.len());
        owned.extend_from_slice(bytes);
        Self::from_owned(owned, reservation)
    }
}
#[derive(Debug)]
pub struct CompleteCheckpointCandidate {
    bytes: Vec<u8>,
    offsets: [(usize, usize); 16],
    world_identity: WorldIdentity,
    boundary: LogicalTime,
    manifest: CompleteManifestV1,
    cost: CompleteCheckpointCost,
}
impl CompleteCheckpointCandidate {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn category(&self, category: CheckpointCategory) -> &[u8] {
        let (start, end) = self.offsets[category as usize];
        &self.bytes[start..end]
    }
    pub fn world_identity(&self) -> WorldIdentity {
        self.world_identity
    }
    pub fn boundary(&self) -> LogicalTime {
        self.boundary
    }
    pub fn manifest(&self) -> &CompleteManifestV1 {
        &self.manifest
    }
    pub fn cost(&self) -> CompleteCheckpointCost {
        self.cost
    }
    /// Later M2b hydration must rebind the exact immutable roles. A historical
    /// validation proof never authorizes a changed resolver automatically.
    pub fn require_definitions(
        &self,
        definitions: &InstalledCheckpointDefinitions<'_>,
    ) -> Result<()> {
        self.manifest.require_exact(&definitions.manifest)
    }
}
/// Requires the trusted host's actual Running ownership to remain charged in
/// this same budget; the named minimum is not an introspected live-heap bound.
/// Borrowed resolver metadata remains caller-owned and separately charged for
/// its whole lifetime; hosts using input scratch use the owned resolver path.
pub fn validate(
    input: CompleteCheckpointInput,
    definitions: &InstalledCheckpointDefinitions<'_>,
) -> Result<Admitted<CompleteCheckpointCandidate>> {
    validate_observed(input, definitions, &mut |_| {})
}
/// Same ownership contract as `validate`, with host-side completion milestones.
pub fn validate_observed(
    mut input: CompleteCheckpointInput,
    definitions: &InstalledCheckpointDefinitions<'_>,
    observer: &mut impl FnMut(CompleteCheckpointStage),
) -> Result<Admitted<CompleteCheckpointCandidate>> {
    input
        .reservation
        .require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
    let proof = validate_raw(
        &input.bytes,
        input.bytes.capacity(),
        &mut input.reservation,
        definitions,
        observer,
    )?;
    finish_validation(input, proof, observer)
}
/// Host path for a resolver allocated under the input's definition scratch.
/// Both resolver and raw storage remain owned until every fallible check ends;
/// resolver text drops before the input's admission may shrink or disappear.
pub fn validate_owned_definitions_observed(
    input: CompleteCheckpointInput,
    definitions: InstalledCheckpointDefinitions<'_>,
    observer: &mut impl FnMut(CompleteCheckpointStage),
) -> Result<Admitted<CompleteCheckpointCandidate>> {
    struct Owner<'a> {
        definitions: InstalledCheckpointDefinitions<'a>,
        input: CompleteCheckpointInput,
    }
    let mut owner = Owner { definitions, input };
    owner
        .input
        .reservation
        .require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
    let proof = validate_raw(
        &owner.input.bytes,
        owner.input.bytes.capacity(),
        &mut owner.input.reservation,
        &owner.definitions,
        observer,
    )?;
    let Owner { definitions, input } = owner;
    drop(definitions);
    finish_validation(input, proof, observer)
}
fn finish_validation(
    input: CompleteCheckpointInput,
    proof: Proof,
    observer: &mut impl FnMut(CompleteCheckpointStage),
) -> Result<Admitted<CompleteCheckpointCandidate>> {
    let CompleteCheckpointInput {
        bytes,
        mut reservation,
    } = input;
    let candidate = CompleteCheckpointCandidate {
        bytes,
        offsets: proof.offsets,
        world_identity: proof.world_identity,
        boundary: proof.boundary,
        manifest: proof.manifest,
        cost: proof.cost,
    };
    // All typed owners and temporary indexes have been disposed inside validate_raw.
    reservation.resize(candidate.cost.retained_candidate_bytes.max(1))?;
    observer(CompleteCheckpointStage::CandidateRetention);
    Ok(Admitted::new(candidate, reservation))
}
/// Requires the trusted host's actual Running ownership to remain charged in
/// this same budget; the named minimum is not an introspected live-heap bound.
pub fn capture(
    engine: &Engine,
    source: &impl super::host::HostCheckpointSource,
    profile: CheckpointProfile,
    reservation: Reservation,
) -> Result<Admitted<CompleteCheckpointCandidate>> {
    capture_observed(engine, source, profile, reservation, &mut |_| {})
}
/// Completion milestones for host-side timing; the simulation reads no clock.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum CompleteCheckpointStage {
    Definitions,
    Preflight,
    Encode,
    OuterParse,
    TypedValidation,
    TypedDisposal,
    FinalRecheck,
    CandidateRetention,
}
/// Same admission/capture contract as `capture`. The observer may record host
/// timings. The final source recheck is the capture linearization point, before
/// the FinalRecheck and CandidateRetention notifications.
pub fn capture_observed(
    engine: &Engine,
    source: &impl super::host::HostCheckpointSource,
    profile: CheckpointProfile,
    mut reservation: Reservation,
    observer: &mut impl FnMut(CompleteCheckpointStage),
) -> Result<Admitted<CompleteCheckpointCandidate>> {
    reservation.require(Cohort::SavePayload, 4096)?;
    reservation.require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
    if reservation.bytes() < meter::VALIDATION_SCRATCH {
        reservation.resize(meter::VALIDATION_SCRATCH)?;
    }
    let scalars = source.scalars()?;
    let now = LogicalTime::new(scalars.time.virtual_elapsed.as_secs_f64())
        .ok_or_else(|| error("invalid capture elapsed"))?;
    let definitions =
        InstalledCheckpointDefinitions::from_engine(engine, scalars.definitions, now)?;
    let identity = engine
        .config()
        .checkpoint_world_identity
        .ok_or_else(|| error("world lineage identity is unavailable"))?;
    WorldIdentity::from_bytes(identity.bytes())?;
    source.validate_boundary()?;
    observer(CompleteCheckpointStage::Definitions);
    let mut count = wire::Count::new(profile.limit());
    wire::write_envelope(
        engine,
        source,
        profile,
        identity,
        now,
        &definitions.manifest,
        &mut count,
        &mut reservation,
    )?;
    let (len, digest) = count.finish();
    observer(CompleteCheckpointStage::Preflight);
    reservation.resize(meter::VALIDATION_SCRATCH + len)?;
    let mut output = wire::Output::new(len);
    wire::write_envelope(
        engine,
        source,
        profile,
        identity,
        now,
        &definitions.manifest,
        &mut output,
        &mut reservation,
    )?;
    let bytes = output.finish()?;
    check(
        hash_bytes(&bytes) == digest,
        "borrowed capture source changed during encoding",
    )?;
    source.validate_boundary()?;
    check(
        source.scalars()? == scalars,
        "capture host boundary changed",
    )?;
    observer(CompleteCheckpointStage::Encode);
    let proof = validate_raw(
        &bytes,
        bytes.capacity(),
        &mut reservation,
        &definitions,
        observer,
    )?;
    // Recheck the borrowed source after all validation. The actual host adapter
    // is immutable; generic source implementations receive the same M15 fences.
    let mut final_count = wire::Count::new(profile.limit());
    wire::write_envelope(
        engine,
        source,
        profile,
        identity,
        now,
        &definitions.manifest,
        &mut final_count,
        &mut reservation,
    )?;
    check(
        final_count.finish() == (len, digest),
        "capture source changed during validation",
    )?;
    source.validate_boundary()?;
    check(
        source.scalars()? == scalars,
        "capture final host boundary changed",
    )?;
    drop(definitions);
    observer(CompleteCheckpointStage::FinalRecheck);
    let candidate = CompleteCheckpointCandidate {
        bytes,
        offsets: proof.offsets,
        world_identity: proof.world_identity,
        boundary: proof.boundary,
        manifest: proof.manifest,
        cost: proof.cost,
    };
    reservation.resize(candidate.cost.retained_candidate_bytes.max(1))?;
    observer(CompleteCheckpointStage::CandidateRetention);
    Ok(Admitted::new(candidate, reservation))
}
pub(crate) fn write_json<W: Write, T: Serialize + ?Sized>(writer: &mut W, value: &T) -> Result<()> {
    serde_json::to_writer(writer, value).map_err(|e| error(e.to_string()))
}
pub(crate) fn hash<T: Serialize + ?Sized>(value: &T) -> Result<[u8; 32]> {
    let mut sink = wire::Count::new(usize::MAX);
    write_json(&mut sink, value)?;
    Ok(sink.finish().1)
}
fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}
/// A constant-size owned error survives cancellation without retaining a copy
/// of hostile input text or the original deserializer's diagnostic capacity.
pub(crate) fn diagnostic(value: impl std::fmt::Display) -> CheckpointError {
    struct Limited(String);
    impl std::fmt::Write for Limited {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let mut end = text.len().min(512 - self.0.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.0.push_str(&text[..end]);
            Ok(())
        }
    }
    let mut output = Limited(String::with_capacity(512));
    let _ = std::fmt::write(&mut output, format_args!("{value}"));
    error(output.0)
}
pub(crate) fn error(reason: impl Into<String>) -> CheckpointError {
    CheckpointError::new("complete", reason)
}
pub(crate) fn check(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(error(reason)) }
}
struct Proof {
    offsets: [(usize, usize); 16],
    world_identity: WorldIdentity,
    boundary: LogicalTime,
    manifest: CompleteManifestV1,
    cost: CompleteCheckpointCost,
}
fn validate_raw(
    bytes: &[u8],
    capacity: usize,
    reservation: &mut Reservation,
    definitions: &InstalledCheckpointDefinitions<'_>,
    observer: &mut impl FnMut(CompleteCheckpointStage),
) -> Result<Proof> {
    let meter = meter::DecodeMeter::new(reservation, capacity)?;
    meter.prepare_diagnostics(bytes)?;
    meter.charge(16 * 1024)?; // fixed retained manifest plus bounded metadata
    let wire = wire::parse(bytes)?;
    wire.manifest.require_exact(&definitions.manifest)?;
    observer(CompleteCheckpointStage::OuterParse);
    let counts = crate::engine::complete_checkpoint::validate_components(
        &wire,
        definitions,
        &meter,
        observer,
    )?;
    observer(CompleteCheckpointStage::TypedDisposal);
    let expansion = meter.expanded();
    let retained = capacity
        + std::mem::size_of::<CompleteCheckpointCandidate>()
        + wire.manifest.owned_upper_bytes();
    Ok(Proof {
        offsets: wire.offsets(bytes),
        world_identity: wire.world_identity,
        boundary: wire.boundary,
        manifest: wire.manifest,
        cost: CompleteCheckpointCost {
            encoded_bytes: bytes.len(),
            raw_capacity_bytes: capacity,
            expanded_upper_bytes: expansion,
            category_expansion_upper_bytes: meter.categories(),
            validation_scratch_bytes: meter::VALIDATION_SCRATCH,
            diagnostic_scratch_bytes: meter.diagnostic_scratch(),
            validation_peak_bytes: 3 * capacity
                + meter::VALIDATION_SCRATCH
                + meter.diagnostic_scratch()
                + expansion,
            retained_candidate_bytes: retained,
            characters: counts,
            categories: 16,
        },
    })
}

#[cfg(test)]
mod tests;
