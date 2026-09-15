use super::*;
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvelope<'a> {
    #[serde(borrow)]
    version: &'a RawValue,
    #[serde(borrow)]
    profile: &'a RawValue,
    #[serde(borrow)]
    world_identity: &'a RawValue,
    #[serde(borrow)]
    manifest: &'a RawValue,
    #[serde(borrow)]
    boundary: &'a RawValue,
    #[serde(borrow)]
    ledger: &'a RawValue,
    #[serde(borrow)]
    operations: &'a RawValue,
    #[serde(borrow)]
    backbone: &'a RawValue,
    #[serde(borrow)]
    round: &'a RawValue,
    #[serde(borrow)]
    climate: &'a RawValue,
    #[serde(borrow)]
    knowledge: &'a RawValue,
    #[serde(borrow)]
    law: &'a RawValue,
    #[serde(borrow)]
    marks: &'a RawValue,
    #[serde(borrow)]
    animals: &'a RawValue,
    #[serde(borrow)]
    social: &'a RawValue,
    #[serde(borrow)]
    continuity: &'a RawValue,
    #[serde(borrow)]
    scheduler: &'a RawValue,
    #[serde(borrow)]
    night: &'a RawValue,
    #[serde(borrow)]
    speech: &'a RawValue,
    #[serde(borrow)]
    cognition_inputs: &'a RawValue,
    #[serde(borrow)]
    host: &'a RawValue,
}
pub(crate) struct Envelope<'a> {
    pub(crate) version: u16,
    pub(crate) profile: CheckpointProfile,
    pub(crate) world_identity: WorldIdentity,
    pub(crate) manifest: CompleteManifestV1,
    pub(crate) boundary: LogicalTime,
    pub(crate) ledger: &'a RawValue,
    pub(crate) operations: &'a RawValue,
    pub(crate) backbone: &'a RawValue,
    pub(crate) round: &'a RawValue,
    pub(crate) climate: &'a RawValue,
    pub(crate) knowledge: &'a RawValue,
    pub(crate) law: &'a RawValue,
    pub(crate) marks: &'a RawValue,
    pub(crate) animals: &'a RawValue,
    pub(crate) social: &'a RawValue,
    pub(crate) continuity: &'a RawValue,
    pub(crate) scheduler: &'a RawValue,
    pub(crate) night: &'a RawValue,
    pub(crate) speech: &'a RawValue,
    pub(crate) cognition_inputs: &'a RawValue,
    pub(crate) host: &'a RawValue,
}
impl Envelope<'_> {
    pub(crate) fn categories(&self) -> [&RawValue; 16] {
        [
            self.ledger,
            self.operations,
            self.backbone,
            self.round,
            self.climate,
            self.knowledge,
            self.law,
            self.marks,
            self.animals,
            self.social,
            self.continuity,
            self.scheduler,
            self.night,
            self.speech,
            self.cognition_inputs,
            self.host,
        ]
    }
    pub(crate) fn offsets(&self, bytes: &[u8]) -> [(usize, usize); 16] {
        self.categories().map(|raw| {
            let start = raw.get().as_ptr() as usize - bytes.as_ptr() as usize;
            (start, start + raw.get().len())
        })
    }
}
pub(crate) fn parse(bytes: &[u8]) -> Result<Envelope<'_>> {
    check(
        bytes.len() <= super::super::POPULATED_PAYLOAD_BYTES,
        "encoded complete payload limit exceeded",
    )?;
    check(
        bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{'),
        "complete envelope must be an object",
    )?;
    // No allocation and no recursive walk before bounded typed field decoding.
    // RawValue validates JSON syntax; this independent scan enforces depth64
    // even for opaque category bodies skipped by serde's outer field decoder.
    let (mut depth, mut quoted, mut escape, mut string_start) = (0usize, false, false, 0usize);
    for (index, &b) in bytes.iter().enumerate() {
        if quoted {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                quoted = false;
                let next = bytes[index + 1..].iter().find(|b| !b.is_ascii_whitespace());
                if next == Some(&b':') {
                    check(
                        index - string_start <= 4096,
                        "complete JSON key byte limit exceeded",
                    )?;
                }
            }
        } else {
            match b {
                b'"' => {
                    quoted = true;
                    string_start = index + 1;
                }
                b'{' | b'[' => {
                    depth += 1;
                    check(depth <= 64, "complete nesting limit exceeded")?;
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    // All 21 fields are initially borrowed RawValue. Generated outer errors
    // therefore only contain bounded keys, never a hostile typed value string.
    let raw: RawEnvelope<'_> = serde_json::from_slice(bytes).map_err(super::diagnostic)?;
    fn small<T: serde::de::DeserializeOwned>(raw: &RawValue) -> Result<T> {
        // Covers the maximum escaped bounded manifest texts plus fixed digests.
        // Error rendering and copying of any malformed metadata stays bounded.
        check(
            raw.get().len() <= 64 * 1024,
            "complete metadata byte limit exceeded",
        )?;
        serde_json::from_str(raw.get()).map_err(super::diagnostic)
    }
    let value = Envelope {
        version: small(raw.version)?,
        profile: small(raw.profile)?,
        world_identity: small(raw.world_identity)?,
        manifest: small(raw.manifest)?,
        boundary: small(raw.boundary)?,
        ledger: raw.ledger,
        operations: raw.operations,
        backbone: raw.backbone,
        round: raw.round,
        climate: raw.climate,
        knowledge: raw.knowledge,
        law: raw.law,
        marks: raw.marks,
        animals: raw.animals,
        social: raw.social,
        continuity: raw.continuity,
        scheduler: raw.scheduler,
        night: raw.night,
        speech: raw.speech,
        cognition_inputs: raw.cognition_inputs,
        host: raw.host,
    };
    check(value.version == 1, "unsupported complete envelope version")?;
    check(
        bytes.len() <= value.profile.limit(),
        "encoded profile payload limit exceeded",
    )?;
    WorldIdentity::from_bytes(value.world_identity.bytes())?;
    super::super::logical("complete", value.boundary.seconds())?;
    for raw in value.categories() {
        check(
            raw.get().as_bytes().first() == Some(&b'{'),
            "complete category must be a non-null object",
        )?;
    }
    Ok(value)
}
pub(crate) struct Count {
    bytes: usize,
    limit: usize,
    hash: Sha256,
}
impl Count {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            bytes: 0,
            limit,
            hash: Sha256::new(),
        }
    }
    pub(crate) fn finish(self) -> (usize, [u8; 32]) {
        (self.bytes, self.hash.finalize().into())
    }
}
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| std::io::Error::other("complete encoded allocation limit exceeded"))?;
        self.hash.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) struct Output {
    bytes: Vec<u8>,
    limit: usize,
}
impl Output {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit),
            limit,
        }
    }
    pub(crate) fn finish(self) -> Result<Vec<u8>> {
        check(
            self.bytes.len() == self.limit,
            "capture encoded length changed",
        )?;
        Ok(self.bytes)
    }
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(std::io::Error::other(
                "capture source outgrew admitted output",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_envelope<W: Write>(
    engine: &Engine,
    source: &impl super::super::host::HostCheckpointSource,
    profile: CheckpointProfile,
    identity: WorldIdentity,
    now: LogicalTime,
    manifest: &CompleteManifestV1,
    writer: &mut W,
    reservation: &mut Reservation,
) -> Result<()> {
    write_envelope_with_host(
        engine,
        profile,
        identity,
        now,
        manifest,
        writer,
        reservation,
        |writer| super::super::host::complete_write(source, writer),
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_envelope_with_host<W: Write>(
    engine: &Engine,
    profile: CheckpointProfile,
    identity: WorldIdentity,
    now: LogicalTime,
    manifest: &CompleteManifestV1,
    writer: &mut W,
    reservation: &mut Reservation,
    mut host: impl FnMut(&mut W) -> Result<()>,
) -> Result<()> {
    writer
        .write_all(b"{\"version\":1,\"profile\":")
        .map_err(super::diagnostic)?;
    write_json(writer, &profile)?;
    writer
        .write_all(b",\"world_identity\":")
        .map_err(super::diagnostic)?;
    write_json(writer, &identity)?;
    writer
        .write_all(b",\"manifest\":")
        .map_err(super::diagnostic)?;
    write_json(writer, manifest)?;
    writer
        .write_all(b",\"boundary\":")
        .map_err(super::diagnostic)?;
    write_json(writer, &now)?;
    for category in CheckpointCategory::ALL {
        writer.write_all(b",").map_err(super::diagnostic)?;
        write_json(writer, category.name())?;
        writer.write_all(b":").map_err(super::diagnostic)?;
        if category == CheckpointCategory::Host {
            host(writer)?;
        } else {
            engine.complete_write_category(category, now, writer, reservation)?;
        }
    }
    writer.write_all(b"}").map_err(super::diagnostic)?;
    Ok(())
}
