//! Quarantined complete hydration. External obligations are retained but cannot
//! execute until M2c prepares them and M3 adopts a complete host generation.
use super::*;
use crate::{
    AreaMap, ItemCatalog, NavData, PromptEnv, RuntimeGeneration, SoundCatalog, WorldSeed,
    engine::{
        EngineConfig, cognition_inputs_checkpoint::EngineCognitionInputsCandidate,
        speech_checkpoint::EngineSpeechCandidate,
    },
    knowledge::{AreaAdjacency, FactCatalog, SalienceTable},
    marks::MarkCatalog,
    weather::ShelterMap,
};
use std::sync::Arc;

/// Actual immutable World roles. They may differ from EngineConfig navigation
/// and shelters. Construct/clone these only after hydration preparation admits
/// the factory. Any caller-retained shared Arcs require their own coordinated
/// admission for their entire lifetime, including interior navigation caches.
pub struct HydrationWorldAssets {
    pub areas: AreaMap,
    pub sounds: SoundCatalog,
    pub items: Arc<ItemCatalog>,
    pub nav: Option<Arc<NavData>>,
    pub shelters: Arc<ShelterMap>,
    pub marks: Arc<MarkCatalog>,
    pub facts: Arc<FactCatalog>,
    pub salience: Arc<SalienceTable>,
    pub area_adjacency: Arc<AreaAdjacency>,
}
/// Owned parsed definitions, never a seeded World. The factory's allowance
/// includes parsing/compilation scratch, spare capacity and any shared Arcs that
/// can survive their original owner. This trusted host bound is not inferred
/// from a raw checkpoint or from the minimum Running reservation.
pub struct HydrationAssets {
    pub(crate) seed_identity: [u8; 32],
    pub(crate) config: EngineConfig,
    pub(crate) env: PromptEnv,
    pub(crate) world: HydrationWorldAssets,
}
impl HydrationAssets {
    pub fn new(
        seed: &WorldSeed,
        config: EngineConfig,
        env: PromptEnv,
        world: HydrationWorldAssets,
    ) -> Result<Self> {
        Ok(Self {
            seed_identity: hash(seed)?,
            config,
            env,
            world,
        })
    }
}
/// Additional complete typed storage: Engine/World/wrapper roots, Arc control
/// blocks and the at-most-eight reconstructed speech semantic BTree entries.
/// This joins (does not replace) the unchanged cumulative 128 MiB ceiling.
pub(crate) const HYDRATION_STRUCTURAL_BYTES: usize = 256 * 1024;

pub struct HydrationPreparation {
    bytes: Vec<u8>,
    // Candidate raw storage drops before this subordinate admission owner.
    asset_reservation: Reservation,
}
impl Admitted<CompleteCheckpointCandidate> {
    /// Reserve a disjoint same-cohort lease before calling any asset factory.
    /// The caller must bound actual assets plus construction scratch, including
    /// spare capacities. A failed reserve never invokes or retains a factory.
    pub fn prepare_hydration(
        self,
        assets_upper_bytes: usize,
    ) -> Result<Admitted<HydrationPreparation>> {
        self.try_map(|candidate, r| {
            r.require(
                Cohort::LoadCandidate,
                candidate.cost.retained_candidate_bytes.max(1),
            )?;
            r.require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
            let asset_reservation = r.sublease(assets_upper_bytes)?;
            Ok(HydrationPreparation {
                bytes: candidate.bytes,
                asset_reservation,
            })
        })
    }
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct HydrationCost {
    pub decoded_upper_bytes: usize,
    pub structural_upper_bytes: usize,
    pub assets_upper_bytes: usize,
    pub retained_upper_bytes: usize,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub enum HydrationStage {
    Assets,
    Definitions,
    TypedOwners,
    Construction,
    RawDisposal,
    Retention,
}

/// An actual newly constructed Engine with pending external work quarantined.
/// There is deliberately no public Engine/World borrow, mutable access, poll or
/// extraction: even &World would expose navigation's interior-mutable caches.
/// Speech interruption/drafts/accepted recordings, exact accepted cognition
/// inputs and the entire host continuation remain owned until M2c/M3.
/// The Engine service trait objects keep this wrapper non-Send. DecodedHydration
/// supplies the worker boundary; this wrapper is constructed on the host thread.
/// Whole-App adoption and measured frame acceptance remain separate.
pub struct HydratedEngine {
    pub(crate) engine: Engine,
    pub(crate) continuation_failed: bool,
    pub(crate) speech: EngineSpeechCandidate,
    pub(crate) host: super::super::host::HostCandidate,
    pub(crate) cognition: EngineCognitionInputsCandidate,
    pub(crate) boundary: LogicalTime,
    pub(crate) world_identity: WorldIdentity,
    pub(crate) cost: HydrationCost,
    // Last field: all assets held by Engine and all continuation owners above
    // are disposed before their subordinate asset lease can release capacity.
    pub(crate) asset_reservation: Reservation,
}
/// Fully validated, owned typed graph. This is Send without carrying an Engine
/// or a service trait object. Raw bytes and resolver scratch have already died;
/// its asset and typed leases follow it through worker delivery/cancellation.
pub struct DecodedHydration {
    owners: crate::engine::complete_checkpoint::ValidatedOwners,
    assets: HydrationAssets,
    boundary: LogicalTime,
    world_identity: WorldIdentity,
    generation: RuntimeGeneration,
    cost: HydrationCost,
    asset_reservation: Reservation,
}
impl DecodedHydration {
    pub fn boundary(&self) -> LogicalTime {
        self.boundary
    }
    pub fn world_identity(&self) -> WorldIdentity {
        self.world_identity
    }
    pub fn runtime_generation(&self) -> RuntimeGeneration {
        self.generation
    }
    pub fn cost(&self) -> HydrationCost {
        self.cost
    }
    pub fn host(&self) -> &super::super::host::HostCandidate {
        &self.owners.host
    }
}
impl CompleteCheckpointInput {
    /// Direct file-input path. The existing complete decoder validates all
    /// sixteen owners and their exact installed agreement before returning typed
    /// authority. No intermediate unchecked candidate or second world copy.
    pub fn prepare_hydration(
        self,
        assets_upper_bytes: usize,
    ) -> Result<Admitted<HydrationPreparation>> {
        let Self { bytes, reservation } = self;
        Admitted::new(bytes, reservation).try_map(|bytes, r| {
            r.require(Cohort::LoadCandidate, bytes.capacity().saturating_add(4096))?;
            r.require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
            let asset_reservation = r.sublease(assets_upper_bytes)?;
            Ok(HydrationPreparation {
                bytes,
                asset_reservation,
            })
        })
    }
}
impl Admitted<HydrationPreparation> {
    pub fn hydrate(
        self,
        factory: impl FnOnce() -> Result<HydrationAssets>,
        host_definitions: super::super::host::DefinitionsV1,
        generation: RuntimeGeneration,
    ) -> Result<Admitted<HydratedEngine>> {
        self.hydrate_observed(factory, host_definitions, generation, &mut |_| {})
    }
    pub fn hydrate_observed(
        self,
        factory: impl FnOnce() -> Result<HydrationAssets>,
        host_definitions: super::super::host::DefinitionsV1,
        generation: RuntimeGeneration,
        observer: &mut impl FnMut(HydrationStage),
    ) -> Result<Admitted<HydratedEngine>> {
        Ok(self
            .decode_observed(factory, host_definitions, generation, observer)?
            .construct_observed(observer))
    }
    pub fn decode(
        self,
        factory: impl FnOnce() -> Result<HydrationAssets>,
        host_definitions: super::super::host::DefinitionsV1,
        generation: RuntimeGeneration,
    ) -> Result<Admitted<DecodedHydration>> {
        self.decode_observed(factory, host_definitions, generation, &mut |_| {})
    }
    /// Eligible for a worker. The factory's existing captured input must already
    /// be admitted by the caller; the asset lease covers its produced assets and
    /// work. Resolver/diagnostic scratch is admitted before factory or resolver.
    pub fn decode_observed(
        self,
        factory: impl FnOnce() -> Result<HydrationAssets>,
        host_definitions: super::super::host::DefinitionsV1,
        generation: RuntimeGeneration,
        observer: &mut impl FnMut(HydrationStage),
    ) -> Result<Admitted<DecodedHydration>> {
        self.try_map_mut(|preparation, reservation| {
            struct Owner {
                assets: Option<HydrationAssets>,
                preparation: HydrationPreparation,
            }
            let mut owner = Owner {
                assets: None,
                preparation,
            };
            reservation.require_running(RUNNING_AUTHORITY_ALLOWANCE_BYTES)?;
            check(
                generation.0 != 0,
                "hydration requires a nonzero runtime generation",
            )?;
            // Includes prepare_definition_resolution's full allowance before
            // any owned resolver is built, also on the direct M3a input path.
            let meter = meter::DecodeMeter::new(reservation, owner.preparation.bytes.capacity())?;
            meter.prepare_diagnostics(&owner.preparation.bytes)?;
            meter.charge(16 * 1024)?;
            meter.charge(HYDRATION_STRUCTURAL_BYTES)?;
            owner.assets = Some(factory()?);
            observer(HydrationStage::Assets);
            let wire = wire::parse(&owner.preparation.bytes)?;
            let d = InstalledCheckpointDefinitions::from_assets(
                owner.assets.as_ref().unwrap(),
                host_definitions,
                wire.boundary,
            )?;
            wire.manifest.require_exact(&d.manifest)?;
            observer(HydrationStage::Definitions);
            let owners = crate::engine::complete_checkpoint::decode_components(
                &wire,
                &d,
                &meter,
                &mut |_| {},
            )?;
            check(
                generation.0 != owners.host.scalars().boundary.generation,
                "hydration runtime generation reuses saved execution fence",
            )?;
            observer(HydrationStage::TypedOwners);
            let decoded = meter.expanded();
            let boundary = wire.boundary;
            let world_identity = wire.world_identity;
            drop(meter);
            drop(d);
            drop(wire);
            let assets_upper_bytes = owner.preparation.asset_reservation.bytes();
            let cost = HydrationCost {
                decoded_upper_bytes: decoded,
                structural_upper_bytes: HYDRATION_STRUCTURAL_BYTES,
                assets_upper_bytes,
                retained_upper_bytes: decoded + assets_upper_bytes,
            };
            let result = DecodedHydration {
                owners,
                assets: owner.assets.take().unwrap(),
                boundary,
                world_identity,
                generation,
                cost,
                asset_reservation: owner.preparation.asset_reservation,
            };
            drop(owner.preparation.bytes);
            observer(HydrationStage::RawDisposal);
            reservation.resize(decoded.max(1))?;
            observer(HydrationStage::Retention);
            Ok(result)
        })
    }
}
impl Admitted<DecodedHydration> {
    /// Constant-count exhaustive moves into the non-Send Engine. No typed decode,
    /// asset loading, raw disposal, ordinary seeding, polling or service submission.
    pub fn construct(self) -> Admitted<HydratedEngine> {
        self.construct_observed(&mut |_| {})
    }
    pub fn construct_observed(
        self,
        observer: &mut impl FnMut(HydrationStage),
    ) -> Admitted<HydratedEngine> {
        self.map(|decoded| {
            let (engine, speech, host, cognition) = crate::engine::hydration::construct(
                decoded.owners,
                decoded.assets,
                decoded.world_identity,
                decoded.generation,
            );
            let result = HydratedEngine {
                engine,
                speech,
                host,
                cognition,
                continuation_failed: false,
                boundary: decoded.boundary,
                world_identity: decoded.world_identity,
                cost: decoded.cost,
                asset_reservation: decoded.asset_reservation,
            };
            observer(HydrationStage::Construction);
            result
        })
    }
}
impl HydratedEngine {
    pub fn boundary(&self) -> LogicalTime {
        self.boundary
    }
    pub fn world_identity(&self) -> WorldIdentity {
        self.world_identity
    }
    pub fn runtime_generation(&self) -> RuntimeGeneration {
        self.engine.config().runtime_generation
    }
    pub fn cost(&self) -> HydrationCost {
        self.cost
    }
    pub fn speech(&self) -> &EngineSpeechCandidate {
        &self.speech
    }
    pub fn host(&self) -> &super::super::host::HostCandidate {
        &self.host
    }
    pub fn cognition_inputs(&self) -> &EngineCognitionInputsCandidate {
        &self.cognition
    }
    pub fn character_count(&self) -> usize {
        self.engine.world().characters.len()
    }
    pub fn item_count(&self) -> usize {
        self.engine.world().items.len()
    }
    /// Observe the reconstructed mutable index independently of its source
    /// speech candidate; this never exposes mutable World authority.
    pub fn protected_speech_action_count(&self) -> usize {
        self.engine.world().speech_actions.len()
    }
    pub fn protects_speech_action(&self, id: crate::receipts::CommandId) -> bool {
        self.engine.world().speech_actions.contains(&id)
    }
    /// Recompute from the actual new domain owners. This never reads retained
    /// raw bytes (none survive hydration) or runs a simulation/provider poll.
    pub fn category_digest(
        &self,
        category: CheckpointCategory,
        mut reservation: Reservation,
    ) -> Result<Admitted<[u8; 32]>> {
        reservation.require_shared(&self.asset_reservation)?;
        reservation.require(Cohort::SavePayload, 4096)?;
        if reservation.bytes() < meter::VALIDATION_SCRATCH {
            reservation.resize(meter::VALIDATION_SCRATCH)?;
        }
        let mut sink = wire::Count::new(super::super::POPULATED_PAYLOAD_BYTES);
        match category {
            CheckpointCategory::Speech => self.speech.complete_write_retained(&mut sink)?,
            CheckpointCategory::Host => self.host.complete_write_retained(&mut sink)?,
            CheckpointCategory::CognitionInputs => {
                self.cognition.complete_write_retained(&mut sink)?
            }
            _ => self.engine.complete_write_category(
                category,
                self.boundary,
                &mut sink,
                &mut reservation,
            )?,
        }
        Ok(Admitted::new(sink.finish().1, reservation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoded_hydration_is_send_without_engine_services() {
        fn send<T: Send>() {}
        send::<Admitted<DecodedHydration>>();
    }
    #[test]
    fn hydration_structural_allowance_covers_new_roots_and_speech_index() {
        use std::mem::size_of;
        // Eight active recording identities need at most two BTree nodes;
        // reserve four full nodes conservatively, plus allocator/control space.
        let speech_index = 4 * (11 * size_of::<crate::receipts::CommandId>() + 256);
        let roots = size_of::<DecodedHydration>()
            + size_of::<HydratedEngine>()
            + size_of::<Engine>()
            + size_of::<crate::World>()
            + size_of::<HydrationAssets>()
            + size_of::<crate::world::checkpoint::HydrationOwners>()
            + size_of::<crate::engine::complete_checkpoint::ValidatedOwners>();
        let wrappers = 4 * 1024;
        println!(
            "hydration structural roots={roots} speech_index={speech_index} wrappers={wrappers} bound={HYDRATION_STRUCTURAL_BYTES}"
        );
        assert!(roots + speech_index + wrappers <= HYDRATION_STRUCTURAL_BYTES);
    }
    #[test]
    fn hydration_shared_budget_identity_does_not_depend_on_equal_charge() {
        let a = super::super::super::CheckpointBudget::default();
        let b = super::super::super::CheckpointBudget::default();
        let parent = a.reserve(Cohort::LoadCandidate, 4096).unwrap();
        let child = parent.sublease(4096).unwrap();
        let foreign = b.reserve(Cohort::SavePayload, 4096).unwrap();
        assert!(child.require_shared(&parent).is_ok());
        assert!(child.require_shared(&foreign).is_err());
        drop(parent);
        assert!(a.reserve(Cohort::LoadCandidate, 1).is_err());
        drop(child);
        assert_eq!(a.retained_bytes(), 0);
    }
}
