use super::super::{BoundedText, host::DefinitionsV1};
use super::*;
use crate::{
    ActorId, ItemCatalog, NavData, SoundCatalog,
    areas::AreaMap,
    knowledge::{FactCatalog, SalienceTable},
    marks::MarkCatalog,
    operations::OperationConfig,
    weather::ShelterMap,
};
mod build {
    include!(concat!(env!("OUT_DIR"), "/checkpoint_build.rs"));
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteManifestV1 {
    pub(crate) version: u16,
    pub(crate) source_sha256: [u8; 32],
    pub(crate) implementation_sha256: [u8; 32],
    pub(crate) host_image: [u8; 32],
    pub(crate) toolchain: BoundedText<4096>,
    pub(crate) target: BoundedText<256>,
    pub(crate) procedural_hasher: BoundedText<128>,
    pub(crate) default_hasher_witness: [u64; 4],
    pub(crate) ordered_seed: [u8; 32],
    pub(crate) prompts: [u8; 32],
    pub(crate) world_items: [u8; 32],
    pub(crate) world_climate_definitions: [u8; 32],
    pub(crate) world_area_adjacency: [u8; 32],
    pub(crate) world_marks: [u8; 32],
    pub(crate) world_facts: [u8; 32],
    pub(crate) world_salience: [u8; 32],
    pub(crate) engine_nav: super::super::host::Nullable<[u8; 32]>,
    pub(crate) engine_shelters: [u8; 32],
    pub(crate) engine_configuration: [u8; 32],
    pub(crate) host: DefinitionsV1,
}
impl CompleteManifestV1 {
    pub(crate) fn build(mut self) -> Result<Self> {
        use std::hash::{Hash, Hasher};
        self.version = 1;
        self.source_sha256 = build::SOURCE_SHA256;
        self.implementation_sha256 = build::BUILD_SHA256;
        self.toolchain = BoundedText::new(build::TOOLCHAIN).map_err(error)?;
        self.target = BoundedText::new(build::TARGET).map_err(error)?;
        self.procedural_hasher =
            BoundedText::new("std::collections::hash_map::DefaultHasher/exact-build/v1")
                .map_err(error)?;
        self.default_hasher_witness = std::array::from_fn(|i| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            ("cathedral-complete-v1", i as u64, [0u8, 1, 127, 255]).hash(&mut h);
            h.finish()
        });
        Ok(self)
    }
    pub(crate) fn require_exact(&self, installed: &Self) -> Result<()> {
        check(self.version == 1, "unsupported complete manifest version")?;
        check(
            self.host_image == installed.host_image,
            "exact running host image mismatch",
        )?;
        check(
            self == installed,
            "exact installed complete manifest mismatch",
        )
    }
    pub(crate) fn owned_upper_bytes(&self) -> usize {
        self.toolchain.0.capacity()
            + self.target.0.capacity()
            + self.procedural_hasher.0.capacity()
            + 128
    }
}
/// Explicit immutable roles, never a reference to live mutable World authority.
/// Candidate decoding uses saved backbone/ledger/clock for every mutable input.
/// Construction requires caller-owned definition scratch: complete capture and
/// CompleteCheckpointInput::prepare_definition_resolution reserve64 MiB first.
pub struct InstalledCheckpointDefinitions<'a> {
    pub(crate) manifest: CompleteManifestV1,
    pub(crate) player: &'a ActorId,
    pub(crate) items: &'a ItemCatalog,
    pub(crate) world_nav: Option<&'a NavData>,
    pub(crate) world_shelters: &'a ShelterMap,
    pub(crate) areas: &'a AreaMap,
    pub(crate) sounds: &'a SoundCatalog,
    pub(crate) marks: &'a MarkCatalog,
    pub(crate) facts: &'a FactCatalog,
    pub(crate) salience: &'a SalienceTable,
    pub(crate) engine_nav: Option<&'a NavData>,
    pub(crate) operations: &'a OperationConfig,
    pub(crate) engine_config: &'a crate::engine::EngineConfig,
}
impl InstalledCheckpointDefinitions<'_> {
    pub fn manifest(&self) -> &CompleteManifestV1 {
        &self.manifest
    }
}
