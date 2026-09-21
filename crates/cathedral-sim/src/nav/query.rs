//! Borrowed street topology plus bounded temporary physical closures. This is
//! ordinary NPC routing, not a permissive player-traversal certificate.
use super::{NavData, Route, WALK_Y};
use crate::Vec3;

pub const MAX_CLOSURES: usize = 32;
const MAX_QUERY_NODES: usize = 32_768;
const MAX_QUERY_ADJACENCY: usize = 131_072;
const MAX_GRID_SPAN: usize = 16_384;
const MAX_EXPANSIONS: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Street,
    Interior(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    UnsupportedSurface,
    InvalidPoint,
    InvalidClosures,
    Capacity,
    Unavailable,
    Stale,
}

/// Already expanded for the querying body's footprint. The owner supplies a
/// stable ID and a revision; this query never creates a second portal state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Closure {
    pub id: u64,
    pub min: [f64; 2],
    pub max: [f64; 2],
}
impl Closure {
    fn valid(&self) -> bool {
        self.id != 0
            && (0..2).all(|i| {
                self.min[i].is_finite() && self.max[i].is_finite() && self.min[i] <= self.max[i]
            })
    }
    fn intersects(&self, a: Vec3, b: Vec3) -> bool {
        let mut lo: f64 = 0.0;
        let mut hi: f64 = 1.0;
        for (i, origin, end) in [(0, a.x, b.x), (1, a.z, b.z)] {
            let delta = end - origin;
            if !delta.is_finite() {
                return true;
            }
            if delta == 0.0 {
                if origin < self.min[i] || origin > self.max[i] {
                    return false;
                }
            } else {
                let t0 = (self.min[i] - origin) / delta;
                let t1 = (self.max[i] - origin) / delta;
                lo = lo.max(t0.min(t1));
                hi = hi.min(t0.max(t1));
                if lo > hi {
                    return false;
                }
            }
        }
        true
    }
}

pub struct StreetQuery<'a> {
    nav: &'a NavData,
    closures: &'a [Closure],
    revision: u64,
}
pub struct RouteTicket {
    graph: [u8; 32],
    revision: u64,
    closures: [Option<Closure>; MAX_CLOSURES],
    route: Route,
}
impl RouteTicket {
    /// No unchecked mutable route projection: using it requires the same
    /// graph and exact access snapshot (revision AND contents).
    pub fn route<'a>(&'a self, query: &StreetQuery<'_>) -> Result<&'a Route, QueryError> {
        if self.graph != query.nav.checkpoint_fingerprint()
            || self.revision != query.revision
            || self
                .closures
                .iter()
                .flatten()
                .copied()
                .ne(query.closures.iter().copied())
        {
            return Err(QueryError::Stale);
        }
        Ok(&self.route)
    }
}
impl<'a> StreetQuery<'a> {
    pub fn new(
        nav: &'a NavData,
        closures: &'a [Closure],
        revision: u64,
    ) -> Result<Self, QueryError> {
        if closures.len() > MAX_CLOSURES || nav.node_count() > MAX_QUERY_NODES {
            return Err(QueryError::Capacity);
        }
        // Bound route materialization's adjacency scans and the exact-segment
        // helper's grid-crossing scratch before either can allocate.
        if nav
            .adjacency()
            .iter()
            .try_fold(0usize, |n, edges| n.checked_add(edges.len()))
            .is_none_or(|n| n > MAX_QUERY_ADJACENCY)
            || nav
                .grid()
                .w
                .checked_add(nav.grid().h)
                .is_none_or(|n| n > MAX_GRID_SPAN)
        {
            return Err(QueryError::Capacity);
        }
        if closures.iter().any(|c| !c.valid()) || closures.windows(2).any(|w| w[0].id >= w[1].id) {
            return Err(QueryError::InvalidClosures);
        }
        Ok(Self {
            nav,
            closures,
            revision,
        })
    }
    fn point(&self, surface: Surface, p: Vec3) -> Result<(), QueryError> {
        if surface != Surface::Street {
            return Err(QueryError::UnsupportedSurface);
        }
        if !p.is_finite() || (p.y - WALK_Y).abs() > 1e-6 {
            return Err(QueryError::InvalidPoint);
        }
        Ok(())
    }
    pub fn position_available(&self, surface: Surface, p: Vec3) -> Result<(), QueryError> {
        self.point(surface, p)?;
        if !self.nav.is_walkable(p.x, p.z) || self.closures.iter().any(|c| c.intersects(p, p)) {
            return Err(QueryError::Unavailable);
        }
        Ok(())
    }
    pub fn segment_available(&self, surface: Surface, a: Vec3, b: Vec3) -> Result<(), QueryError> {
        self.point(surface, a)?;
        self.point(surface, b)?;
        if !self.nav.segment_walkable_exact(a, b)
            || self.closures.iter().any(|c| c.intersects(a, b))
        {
            return Err(QueryError::Unavailable);
        }
        Ok(())
    }
    pub fn route_nodes(
        &self,
        surface: Surface,
        start: usize,
        goal: usize,
    ) -> Result<RouteTicket, QueryError> {
        if surface != Surface::Street {
            return Err(QueryError::UnsupportedSurface);
        }
        if start >= self.nav.node_count() || goal >= self.nav.node_count() {
            return Err(QueryError::InvalidPoint);
        }
        self.position_available(surface, self.nav.node_point(start))?;
        self.position_available(surface, self.nav.node_point(goal))?;
        let route =
            self.nav
                .route_nodes_filtered(start, goal, None, Some(MAX_EXPANSIONS), |a, b| {
                    !self
                        .closures
                        .iter()
                        .any(|c| c.intersects(self.nav.node_point(a), self.nav.node_point(b)))
                })?;
        let mut closures = [None; MAX_CLOSURES];
        for (slot, value) in closures.iter_mut().zip(self.closures) {
            *slot = Some(*value);
        }
        Ok(RouteTicket {
            graph: self.nav.checkpoint_fingerprint(),
            revision: self.revision,
            closures,
            route,
        })
    }
    /// Snapping retains the street graph's cost semantics. Exact endpoint
    /// approach remains the caller's separate local-route obligation.
    pub fn route_between(
        &self,
        surface: Surface,
        from: Vec3,
        to: Vec3,
    ) -> Result<RouteTicket, QueryError> {
        self.point(surface, from)?;
        self.point(surface, to)?;
        if self
            .closures
            .iter()
            .any(|c| c.intersects(from, from) || c.intersects(to, to))
        {
            return Err(QueryError::Unavailable);
        }
        let start = self
            .nav
            .nearest_node(from.x, from.z)
            .ok_or(QueryError::Unavailable)?;
        let goal = self
            .nav
            .nearest_node(to.x, to.z)
            .ok_or(QueryError::Unavailable)?;
        self.route_nodes(surface, start, goal)
    }
}

#[cfg(test)]
mod tests;
