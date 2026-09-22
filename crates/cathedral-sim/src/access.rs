//! Current authority checks and bounded optional decision receipts.
//! Geometry alone is not a capability. This does not invent property grants,
//! key rights, or authority to search: those await authored portal owners.
use sha2::{Digest, Sha256};

use crate::nav::query::{Closure, QueryError, RouteTicket, StreetQuery, Surface};
use crate::timeline::LogicalTime;
use crate::{ActionError, ActionErrorCode, ActorId, RuntimeGeneration, World};

pub const MAX_AUTHORITY_HOLDERS: usize = 32;
const ID_BYTES: usize = crate::MAX_ID_CHARS * 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Denial {
    AbsentActor,
    CommittedWork,
    LeavingCity,
    InCustody,
    MissingCustody,
    NotKeeper,
    Capacity,
    Invalid,
}
impl Denial {
    pub fn clue(self) -> &'static str {
        match self {
            Self::AbsentActor => "acting character is not part of this world",
            Self::CommittedWork => {
                "committed work owns your movement; it must finish or be explicitly cancelled"
            }
            Self::LeavingCity => {
                "your party is leaving the city; the road controller owns your movement"
            }
            Self::InCustody => {
                "you are in the law's hands and go nowhere of your own accord - speak to whoever holds you"
            }
            Self::MissingCustody => "nobody by that id is in the law's hands",
            Self::NotKeeper => {
                "they are not yours to release - only those who hold them, or the law standing over them, can"
            }
            Self::Capacity => {
                "authority cannot be checked within the supported limit; no action was taken"
            }
            Self::Invalid => "authority state is unavailable; no action was taken",
        }
    }
    pub fn action_error(self) -> ActionError {
        let code = match self {
            Self::AbsentActor => ActionErrorCode::UnknownActor,
            Self::LeavingCity => ActionErrorCode::LeavingCity,
            Self::InCustody => ActionErrorCode::InCustody,
            _ => ActionErrorCode::InvalidAction,
        };
        ActionError::new(code, self.clue())
    }
}

/// These checks are also used by the direct reducers. They preserve the shipped
/// duty ordering; a release receipt never authorizes a different operation.
pub fn voluntary_travel(world: &World, actor: &ActorId) -> Result<(), Denial> {
    Key::new(actor)?;
    let Some(person) = world
        .characters
        .get(actor)
        .filter(|_| world.is_present(actor))
    else {
        return Err(Denial::AbsentActor);
    };
    if !world
        .operations
        .permits(actor, crate::operations::DutyPriority::LlmTravel)
    {
        return Err(Denial::CommittedWork);
    }
    if person.state.leaving_city {
        return Err(Denial::LeavingCity);
    }
    if world.custody.holds(actor) {
        return Err(Denial::InCustody);
    }
    Ok(())
}
pub fn release_custody(world: &World, actor: &ActorId, prisoner: &ActorId) -> Result<(), Denial> {
    Key::new(actor)?;
    Key::new(prisoner)?;
    if !world.is_present(actor) {
        return Err(Denial::AbsentActor);
    }
    world.custody.get(prisoner).ok_or(Denial::MissingCustody)?;
    // Existing custody admits more holders than the optional receipt format.
    // Preserve its release/recovery path; only fingerprint capture is capped.
    if !crate::custody::keeps(world, actor, prisoner) {
        return Err(Denial::NotKeeper);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct Boundary {
    pub generation: RuntimeGeneration,
    pub at: LogicalTime,
}
#[derive(Clone, Copy)]
pub enum Request<'a> {
    VoluntaryTravel(&'a ActorId),
    ReleaseCustody {
        actor: &'a ActorId,
        prisoner: &'a ActorId,
    },
}
impl<'a> Request<'a> {
    fn actor(self) -> &'a ActorId {
        match self {
            Self::VoluntaryTravel(a) | Self::ReleaseCustody { actor: a, .. } => a,
        }
    }
    fn target(self) -> Option<&'a ActorId> {
        match self {
            Self::VoluntaryTravel(_) => None,
            Self::ReleaseCustody { prisoner, .. } => Some(prisoner),
        }
    }
    fn decision(self, world: &World) -> Result<(), Denial> {
        match self {
            Self::VoluntaryTravel(a) => voluntary_travel(world, a),
            Self::ReleaseCustody { actor, prisoner } => release_custody(world, actor, prisoner),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Denied(Denial),
    Stale,
    Geometry(QueryError),
    WrongOperation,
}
impl From<QueryError> for Error {
    fn from(e: QueryError) -> Self {
        Self::Geometry(e)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    bytes: [u8; ID_BYTES],
    len: usize,
}
impl Key {
    fn new(id: &ActorId) -> Result<Self, Denial> {
        if id.as_str().len() > ID_BYTES {
            return Err(Denial::Capacity);
        }
        if !id.is_valid() {
            return Err(Denial::Invalid);
        }
        let mut key = Self {
            bytes: [0; ID_BYTES],
            len: id.as_str().len(),
        };
        key.bytes[..key.len].copy_from_slice(id.as_str().as_bytes());
        Ok(key)
    }
    fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("copied UTF-8")
    }
}

/// Fixed-size immutable decision. It is a current-use check, not a permanent
/// grant, warrant, serialized command receipt, or knowledge claim.
#[derive(Clone, Copy)]
pub struct Receipt {
    actor: Key,
    target: Option<Key>,
    authority: [u8; 32],
    generation: RuntimeGeneration,
    at: u64,
    decision: Result<(), Denial>,
}
impl std::fmt::Debug for Receipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessReceipt")
            .field("decision", &self.decision)
            .finish_non_exhaustive()
    }
}
impl Receipt {
    pub fn capture(
        world: &World,
        request: Request<'_>,
        boundary: Boundary,
    ) -> Result<Self, Denial> {
        let actor = Key::new(request.actor())?;
        let target = request.target().map(Key::new).transpose()?;
        let authority = fingerprint(world, request)?;
        Ok(Self {
            actor,
            target,
            authority,
            generation: boundary.generation,
            at: boundary.at.seconds().to_bits(),
            decision: request.decision(world),
        })
    }
    pub fn decision(&self) -> Result<(), Denial> {
        self.decision
    }
    pub fn denial_clue(&self) -> Option<&'static str> {
        self.decision.err().map(Denial::clue)
    }
    pub fn require_current(&self, world: &World, boundary: Boundary) -> Result<(), Error> {
        if boundary.generation != self.generation || boundary.at.seconds().to_bits() != self.at {
            return Err(Error::Stale);
        }
        let (actor, _) = world
            .characters
            .get_key_value(self.actor.text())
            .ok_or(Error::Stale)?;
        let request = if let Some(target) = &self.target {
            let (prisoner, _) = world
                .characters
                .get_key_value(target.text())
                .ok_or(Error::Stale)?;
            Request::ReleaseCustody { actor, prisoner }
        } else {
            Request::VoluntaryTravel(actor)
        };
        if fingerprint(world, request).ok() != Some(self.authority)
            || request.decision(world) != self.decision
        {
            return Err(Error::Stale);
        }
        self.decision.map_err(Error::Denied)
    }
    /// Admission precedes route scratch. The returned route is still private;
    /// every use requires the same current permission and physical closure set.
    pub fn route_nodes(
        &self,
        world: &World,
        boundary: Boundary,
        closures: &[Closure],
        revision: u64,
        start: usize,
        goal: usize,
    ) -> Result<PermittedRoute, Error> {
        if self.target.is_some() {
            return Err(Error::WrongOperation);
        }
        self.require_current(world, boundary)?;
        let nav = world
            .nav
            .as_deref()
            .ok_or(Error::Geometry(QueryError::Unavailable))?;
        let query = StreetQuery::new(nav, closures, revision)?;
        Ok(PermittedRoute {
            permission: *self,
            route: query.route_nodes(Surface::Street, start, goal)?,
        })
    }
}
pub struct PermittedRoute {
    permission: Receipt,
    route: RouteTicket,
}
impl PermittedRoute {
    /// Checked geometry projection at this call boundary. Consumers must
    /// revalidate before consequential use after any world/authority change;
    /// retaining or cloning a Route does not retain permission to travel.
    pub fn route<'a>(
        &'a self,
        world: &World,
        boundary: Boundary,
        closures: &[Closure],
        revision: u64,
    ) -> Result<&'a crate::nav::Route, Error> {
        self.permission.require_current(world, boundary)?;
        let nav = world
            .nav
            .as_deref()
            .ok_or(Error::Geometry(QueryError::Unavailable))?;
        let query = StreetQuery::new(nav, closures, revision)?;
        Ok(self.route.route(&query)?)
    }
}

fn hash_id(hash: &mut Sha256, id: &ActorId) -> Result<(), Denial> {
    Key::new(id)?;
    hash.update((id.as_str().len() as u64).to_le_bytes());
    hash.update(id.as_str().as_bytes());
    Ok(())
}
fn fingerprint(world: &World, request: Request<'_>) -> Result<[u8; 32], Denial> {
    if world.world_revision < 0 || world.spatial_sequence < -1 || world.event_sequence < 0 {
        return Err(Denial::Invalid);
    }
    let mut hash = Sha256::new();
    hash.update([u8::from(request.target().is_some())]);
    for n in [
        world.world_revision,
        world.spatial_sequence,
        world.event_sequence,
    ] {
        hash.update(n.to_le_bytes());
    }
    hash.update([u8::from(world.nav.is_some())]);
    if let Some(nav) = &world.nav {
        hash.update(nav.checkpoint_fingerprint());
    }
    for id in std::iter::once(request.actor()).chain(request.target()) {
        hash_id(&mut hash, id)?;
        let actor = world.characters.get(id).ok_or(Denial::AbsentActor)?;
        if actor.id() != id || !actor.position_m().is_finite() {
            return Err(Denial::Invalid);
        }
        let p = actor.position_m();
        for n in [p.x, p.y, p.z] {
            hash.update(n.to_bits().to_le_bytes());
        }
        hash.update(actor.state.presence_epoch.to_le_bytes());
        hash.update([
            u8::from(actor.state.presence == crate::Presence::InCity),
            u8::from(actor.state.leaving_city),
            u8::from(crate::notices::is_law(actor)),
        ]);
        let owner = world.operations.actor_owner(id);
        hash.update([u8::from(owner.is_some())]);
        if let Some(owner) = owner {
            hash.update([owner.0.operation.producer]);
            hash.update(owner.0.operation.sequence.to_le_bytes());
            hash.update(owner.0.step.to_le_bytes());
        }
        let record = world.custody.get(id);
        hash.update([u8::from(record.is_some())]);
        if let Some(record) = record {
            if record.holders.len() > MAX_AUTHORITY_HOLDERS {
                return Err(Denial::Capacity);
            }
            hash.update([
                u8::from(record.state == crate::custody::Confinement::Committed),
                u8::from(record.officer.is_some()),
            ]);
            if let Some(officer) = &record.officer {
                hash_id(&mut hash, officer)?;
            }
            hash.update((record.holders.len() as u64).to_le_bytes());
            for holder in &record.holders {
                hash_id(&mut hash, holder)?;
            }
        }
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests;
