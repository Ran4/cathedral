//! Bounded sampled visual-presence receipts. This is an explicit geometry
//! fixture seam, not the still-unmigrated gameplay Sight hook or a fact archive.
use sha2::{Digest, Sha256};

use crate::runtime_generation::RuntimeGeneration;
use crate::{ActorId, Character, Presence, Vec3, WALK_Y, World, timeline::LogicalTime};

pub const MAX_OBSERVERS: usize = 32;
pub const MAX_OCCLUDERS: usize = 128;
const ID_BYTES: usize = crate::MAX_ID_CHARS * 4;
const LABEL_BYTES: usize = 128;
const EYE_OFFSET_M: f64 = 0.65;
const RANGE_M: f64 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationError {
    Capacity,
    Invalid,
    MissingEvent,
    MissingActor,
    Stale,
    Unavailable,
}

/// Supplied by the observing host/engine at the synchronous sample boundary.
#[derive(Clone, Copy, Debug)]
pub struct ObservationBoundary {
    pub generation: RuntimeGeneration,
    pub at: LogicalTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualSupport {
    Visible,
    Occluded,
    OutsideView,
    Unavailable,
}

/// A complete fixture may certify only segments wholly inside this volume.
/// These are opaque solids: no walking-bitset or NullSight substitution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub min: Vec3,
    pub max: Vec3,
}
impl Volume {
    fn valid(self) -> bool {
        self.min.is_finite()
            && self.max.is_finite()
            && self.min.x <= self.max.x
            && self.min.y <= self.max.y
            && self.min.z <= self.max.z
    }
    fn contains(self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }
    fn intersects(self, a: Vec3, b: Vec3) -> bool {
        let mut enter: f64 = 0.0;
        let mut exit: f64 = 1.0;
        for (origin, end, min, max) in [
            (a.x, b.x, self.min.x, self.max.x),
            (a.y, b.y, self.min.y, self.max.y),
            (a.z, b.z, self.min.z, self.max.z),
        ] {
            let delta = end - origin;
            if !delta.is_finite() {
                return true;
            }
            if delta == 0.0 {
                if origin < min || origin > max {
                    return false;
                }
            } else {
                let near = (min - origin) / delta;
                let far = (max - origin) / delta;
                enter = enter.max(near.min(far));
                exit = exit.min(near.max(far));
                if enter > exit {
                    return false;
                }
            }
        }
        true
    }
}

pub struct ObservationGeometry<'a> {
    coverage: Option<Volume>,
    occluders: &'a [Volume],
    revision: u64,
    fingerprint: [u8; 32],
}
impl<'a> ObservationGeometry<'a> {
    /// Explicit fixture coverage is a caller assertion, never inferred from an
    /// empty obstacle list. Production structural coverage is not installed yet.
    pub fn complete_fixture(
        coverage: Volume,
        occluders: &'a [Volume],
        revision: u64,
    ) -> Result<Self, ObservationError> {
        if occluders.len() > MAX_OCCLUDERS {
            return Err(ObservationError::Capacity);
        }
        if !coverage.valid() || occluders.iter().any(|v| !v.valid()) {
            return Err(ObservationError::Invalid);
        }
        let mut hash = Sha256::new();
        for volume in std::iter::once(&coverage).chain(occluders) {
            for n in [
                volume.min.x,
                volume.min.y,
                volume.min.z,
                volume.max.x,
                volume.max.y,
                volume.max.z,
            ] {
                hash.update(n.to_bits().to_le_bytes());
            }
        }
        Ok(Self {
            coverage: Some(coverage),
            occluders,
            revision,
            fingerprint: hash.finalize().into(),
        })
    }
    pub fn unavailable(revision: u64) -> Self {
        Self {
            coverage: None,
            occluders: &[],
            revision,
            fingerprint: [0; 32],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
}
impl<const N: usize> Text<N> {
    fn new(value: &str) -> Result<Self, ObservationError> {
        if value.len() > N {
            return Err(ObservationError::Capacity);
        }
        let mut result = Self {
            bytes: [0; N],
            len: value.len(),
        };
        result.bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(result)
    }
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("copied UTF-8")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    world: i64,
    spatial: i64,
    event: i64,
    navigation: [u8; 32],
    access: u64,
    geometry: [u8; 32],
    view_cone: u64,
}
fn stamp(world: &World, geometry: &ObservationGeometry<'_>) -> Result<Stamp, ObservationError> {
    let nav = world.nav.as_ref().ok_or(ObservationError::Unavailable)?;
    if world.world_revision < 0
        || world.spatial_sequence < -1
        || world.event_sequence < 1
        || !world.view_cone_degrees.is_finite()
        || !(0.0..=360.0).contains(&world.view_cone_degrees)
    {
        return Err(ObservationError::Invalid);
    }
    Ok(Stamp {
        world: world.world_revision,
        spatial: world.spatial_sequence,
        event: world.event_sequence,
        navigation: nav.checkpoint_fingerprint(),
        access: geometry.revision,
        geometry: geometry.fingerprint,
        view_cone: world.view_cone_degrees.to_bits(),
    })
}
fn pose(actor: &Character) -> [u64; 5] {
    let p = actor.position_m();
    [
        p.x.to_bits(),
        p.y.to_bits(),
        p.z.to_bits(),
        actor.facing_yaw().to_bits(),
        actor.state.presence_epoch,
    ]
}

/// Immutable supported presence at one sample boundary, associated with a
/// still-owed source event. DomainEvent has no capture-time coverage: this does
/// not retroactively certify the act or a continuous interval. The private
/// subject key is never a claim of recognition or inventory/act detail.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ObservationReceipt {
    stamp: Stamp,
    at: u64,
    generation: RuntimeGeneration,
    observer: Text<ID_BYTES>,
    subject: Text<ID_BYTES>,
    label: Text<LABEL_BYTES>,
    observer_pose: [u64; 5],
    subject_pose: [u64; 5],
    known: bool,
    support: VisualSupport,
}
impl std::fmt::Debug for ObservationReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObservationReceipt")
            .field("source_event", &self.stamp.event)
            .field("support", &self.support)
            .finish_non_exhaustive()
    }
}
impl ObservationReceipt {
    pub fn support(&self) -> VisualSupport {
        self.support
    }
    pub fn source_event(&self) -> i64 {
        self.stamp.event
    }
    pub fn generation(&self) -> RuntimeGeneration {
        self.generation
    }
    pub fn observed_at(&self) -> f64 {
        f64::from_bits(self.at)
    }
    pub fn observer(&self) -> &str {
        self.observer.as_str()
    }
    pub fn visible_description(&self) -> Option<&str> {
        (self.support == VisualSupport::Visible).then(|| self.label.as_str())
    }
    /// Fresh-use guard. Historical evidence remains unchanged when this refuses.
    /// Exact poses supplement revisions: ordinary motion/facing need not bump
    /// the public world revision or the host spatial sequence.
    pub fn require_current(
        &self,
        world: &World,
        geometry: &ObservationGeometry<'_>,
        boundary: ObservationBoundary,
    ) -> Result<(), ObservationError> {
        if boundary.generation != self.generation
            || boundary.at.seconds().to_bits() != self.at
            || stamp(world, geometry).ok() != Some(self.stamp)
        {
            return Err(ObservationError::Stale);
        }
        let observer = world
            .characters
            .get(self.observer.as_str())
            .ok_or(ObservationError::Stale)?;
        let subject = world
            .characters
            .get(self.subject.as_str())
            .ok_or(ObservationError::Stale)?;
        let known = observer.id() == subject.id() || observer.knows().contains(subject.id());
        if pose(observer) != self.observer_pose
            || pose(subject) != self.subject_pose
            || observer.state.presence != Presence::InCity
            || subject.state.presence != Presence::InCity
            || known != self.known
            || (known && subject.name() != self.label.as_str())
        {
            return Err(ObservationError::Stale);
        }
        Ok(())
    }
}

pub struct ObservationSnapshot {
    receipts: [Option<ObservationReceipt>; MAX_OBSERVERS],
    len: usize,
}
impl ObservationSnapshot {
    /// Explicit sorted candidate IDs only: no whole-world candidate scan, no
    /// heap allocation, no mutation/knowledge mint and no replay of old events.
    pub fn capture_latest(
        world: &World,
        geometry: &ObservationGeometry<'_>,
        observers: &[&ActorId],
        boundary: ObservationBoundary,
    ) -> Result<Self, ObservationError> {
        if observers.len() > MAX_OBSERVERS {
            return Err(ObservationError::Capacity);
        }
        if observers.iter().any(|id| id.as_str().len() > ID_BYTES) {
            return Err(ObservationError::Capacity);
        }
        if observers.iter().any(|id| !id.is_valid()) {
            return Err(ObservationError::Invalid);
        }
        if observers.windows(2).any(|ids| ids[0] >= ids[1]) {
            return Err(ObservationError::Invalid);
        }
        let event = world
            .latest_observation_event()
            .ok_or(ObservationError::MissingEvent)?;
        let subject_id = event
            .actor_id
            .as_ref()
            .ok_or(ObservationError::MissingActor)?;
        if subject_id.as_str().len() > ID_BYTES {
            return Err(ObservationError::Capacity);
        }
        if !subject_id.is_valid() {
            return Err(ObservationError::Invalid);
        }
        let subject = world
            .characters
            .get(subject_id)
            .ok_or(ObservationError::MissingActor)?;
        if subject.id() != subject_id
            || event
                .position_m
                .is_none_or(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()] != pose(subject)[..3])
        {
            return Err(ObservationError::Invalid);
        }
        let stamp = stamp(world, geometry)?;
        let mut snapshot = Self {
            receipts: [None; MAX_OBSERVERS],
            len: observers.len(),
        };
        for (slot, id) in snapshot.receipts.iter_mut().zip(observers) {
            let observer = world
                .characters
                .get(*id)
                .ok_or(ObservationError::MissingActor)?;
            if observer.id() != *id || !observer.id().is_valid() || !subject.id().is_valid() {
                return Err(ObservationError::Invalid);
            }
            let known = observer.id() == subject.id() || observer.knows().contains(subject.id());
            let support = visual_support(world, geometry, observer, subject);
            *slot = Some(ObservationReceipt {
                stamp,
                at: boundary.at.seconds().to_bits(),
                generation: boundary.generation,
                observer: Text::new(observer.id().as_str())?,
                subject: Text::new(subject.id().as_str())?,
                label: Text::new(if known { subject.name() } else { "a stranger" })?,
                observer_pose: pose(observer),
                subject_pose: pose(subject),
                known,
                support,
            });
        }
        Ok(snapshot)
    }
    pub fn receipts(&self) -> impl ExactSizeIterator<Item = &ObservationReceipt> {
        self.receipts[..self.len]
            .iter()
            .map(|r| r.as_ref().expect("captured row"))
    }
}

fn visual_support(
    world: &World,
    geometry: &ObservationGeometry<'_>,
    observer: &Character,
    subject: &Character,
) -> VisualSupport {
    let Some(coverage) = geometry.coverage else {
        return VisualSupport::Unavailable;
    };
    let a = observer.position_m();
    let b = subject.position_m();
    if !a.is_finite()
        || !b.is_finite()
        || !observer.facing_yaw().is_finite()
        || !subject.facing_yaw().is_finite()
        || observer.state.presence != Presence::InCity
        || subject.state.presence != Presence::InCity
        || (a.y - WALK_Y).abs() > 1e-6
        || (b.y - WALK_Y).abs() > 1e-6
    {
        return VisualSupport::Unavailable;
    }
    let eye = Vec3::new(a.x, a.y + EYE_OFFSET_M, a.z);
    let target = Vec3::new(b.x, b.y + EYE_OFFSET_M, b.z);
    if !coverage.contains(eye) || !coverage.contains(target) {
        return VisualSupport::Unavailable;
    }
    if a.distance(b) > RANGE_M || !super::sees(observer, subject, world.view_cone_degrees) {
        return VisualSupport::OutsideView;
    }
    if geometry.occluders.iter().any(|v| v.intersects(eye, target)) {
        VisualSupport::Occluded
    } else {
        VisualSupport::Visible
    }
}

#[cfg(test)]
mod tests;
