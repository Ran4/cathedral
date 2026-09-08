use super::{BoundedText, CheckpointError, Result, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};

/// Procedural algorithm identity includes its implementation/build fingerprint.
/// DefaultHasher has no stable cross-toolchain algorithm promise: naming it
/// alone is insufficient. The host supplies the exact accepted implementation
/// digest as part of the immutable asset/behavior resolver contract, including
/// compiler/toolchain, target triple and relevant build configuration whenever
/// the implementation depends on std hashing or target-sensitive behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedAlgorithmV1 {
    name: BoundedText<64>,
    version: u32,
    implementation_sha256: [u8; 32],
}
impl VersionedAlgorithmV1 {
    pub fn new(name: &str, version: u32, implementation_sha256: [u8; 32]) -> Result<Self> {
        let value = Self {
            name: BoundedText::new(name).map_err(|e| CheckpointError::new("manifest", e))?,
            version,
            implementation_sha256,
        };
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        if self.name.0.is_empty() || self.name.0.chars().any(char::is_control) || self.version == 0
        {
            return Err(CheckpointError::new(
                "manifest",
                "invalid algorithm identity",
            ));
        }
        Ok(())
    }
}

/// Exact compatibility component for the future complete envelope. Digests are
/// SHA-256 of the host's canonical content/geometry/behavior manifests, including
/// every effective override. They are not a world seed or PublicSnapshot hash.
/// World identity and the private domain/host payload are separate later fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityManifestV1 {
    schema_version: u16,
    content_sha256: [u8; 32],
    geometry_sha256: [u8; 32],
    behavior_sha256: [u8; 32],
    generator: VersionedAlgorithmV1,
    procedural_hash: VersionedAlgorithmV1,
    command_payload_version: u16,
    provider_reply_version: u16,
}
impl CompatibilityManifestV1 {
    pub fn new(
        content_sha256: [u8; 32],
        geometry_sha256: [u8; 32],
        behavior_sha256: [u8; 32],
        generator: VersionedAlgorithmV1,
        procedural_hash: VersionedAlgorithmV1,
    ) -> Result<Self> {
        let manifest = Self {
            schema_version: SCHEMA_VERSION,
            content_sha256,
            geometry_sha256,
            behavior_sha256,
            generator,
            procedural_hash,
            command_payload_version: crate::receipts::PAYLOAD_VERSION,
            provider_reply_version: crate::receipts::PROVIDER_REPLY_VERSION,
        };
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION
            || self.command_payload_version != crate::receipts::PAYLOAD_VERSION
            || self.provider_reply_version != crate::receipts::PROVIDER_REPLY_VERSION
        {
            return Err(CheckpointError::new(
                "manifest",
                "unsupported schema or digest version",
            ));
        }
        self.generator.validate()?;
        self.procedural_hash.validate()
    }
    /// Reject mismatches before any asset resolution/index construction. This
    /// cut deliberately has no guessed migrations or fallback content lookup.
    pub fn require_exact(&self, installed: &Self) -> Result<()> {
        self.validate()?;
        installed.validate()?;
        if self != installed {
            Err(CheckpointError::new(
                "manifest",
                "exact installed manifest mismatch",
            ))
        } else {
            Ok(())
        }
    }
}
