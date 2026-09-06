//! Exact local routes. Separate from the street graph: no snapped endpoints,
//! unvalidated final stride, widening search, or fallback city journey.
use super::{HeapEntry, NavData, WALK_Y};
use crate::{Movement, math::Vec3};
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Copy)]
pub struct LocalPathBudget {
    pub max_distance_m: f64,
    pub max_expansions: usize,
}

impl LocalPathBudget {
    pub const CHANGE_SPOT: Self = Self {
        max_distance_m: 15.0,
        max_expansions: 4096,
    };
    pub const VISIT: Self = Self {
        max_distance_m: 30.0,
        max_expansions: 8192,
    };
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalRoute {
    /// Includes the exact origin and destination, on WALK_Y.
    pub points: Vec<Vec3>,
    pub length_m: f64,
    pub expanded_cells: usize,
}

impl LocalRoute {
    pub fn into_movement(self, gait_phase: f64) -> Movement {
        Movement {
            path: self.points.into_iter().skip(1).collect(),
            speed: 0.0,
            gait_phase,
            patrol: None,
            choke_wait: 0.0,
            exact_local: true,
        }
    }
}

impl NavData {
    /// Initial placement only, before a body is published. Imported authored
    /// coordinates can be in a wall or precisely on a conservative blocked
    /// boundary. Return the closest safe cell centre within a hard local bound;
    /// this is never an escape/teleport operation for a live mover.
    pub fn initial_surface_position(
        &self,
        position: Vec3,
        limit_m: f64,
        occupied: &[Vec3],
    ) -> Option<Vec3> {
        if !position.is_finite() || !limit_m.is_finite() || !(0.0..=40.0).contains(&limit_m) {
            return None;
        }
        if self.segment_walkable_exact(position, position) {
            return Some(position);
        }
        let g = self.grid;
        let col = ((position.x - g.x0) / g.cell_m).floor() as isize;
        let row = ((position.z - g.z0) / g.cell_m).floor() as isize;
        let cells = (limit_m / g.cell_m).ceil() as isize + 1;
        let mut best: Option<(f64, Vec3)> = None;
        for radius in 0..=cells {
            for r in row - radius..=row + radius {
                for c in col - radius..=col + radius {
                    if (r - row).abs() != radius && (c - col).abs() != radius {
                        continue;
                    }
                    let p = Vec3::new(
                        g.x0 + (c as f64 + 0.5) * g.cell_m,
                        WALK_Y,
                        g.z0 + (r as f64 + 0.5) * g.cell_m,
                    );
                    let distance = p.distance(Vec3::new(position.x, WALK_Y, position.z));
                    if distance > limit_m || best.is_some_and(|(d, _)| distance >= d) {
                        continue;
                    }
                    if occupied.iter().any(|other| p.distance(*other) < 1.2) {
                        continue;
                    }
                    // An extra 15cm in eight directions protects a standing
                    // initial body beyond the already agent-eroded bitset.
                    if (0..8).all(|i| {
                        let angle = i as f64 * std::f64::consts::TAU / 8.0;
                        self.segment_walkable_exact(
                            p,
                            p + Vec3::new(angle.cos() * 0.15, 0.0, angle.sin() * 0.15),
                        )
                    }) {
                        best = Some((distance, p));
                    }
                }
            }
            if best.is_some_and(|(d, _)| radius as f64 * g.cell_m - g.cell_m > d) {
                break;
            }
        }
        best.map(|(_, p)| p)
    }

    /// A necessary long journey with exact, bounded local connectors at each
    /// end. Only graph search is city-wide; failed connectors do not widen the
    /// local search. Optional resident walks never call this method.
    pub fn connected_route(&self, from: Vec3, to: Vec3, lane: f64) -> Option<LocalRoute> {
        if let Some(local) = self.local_route(from, to, LocalPathBudget::VISIT) {
            return Some(local);
        }
        let connector = |point: Vec3| {
            let mut nearest: Vec<_> = (0..self.node_count())
                .map(|n| (point.distance(self.node_point(n)), n))
                .filter(|(distance, _)| *distance <= 30.0)
                .collect();
            nearest.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            nearest.into_iter().take(16).find_map(|(_, n)| {
                self.local_route(point, self.node_point(n), LocalPathBudget::VISIT)
                    .map(|route| (n, route))
            })
        };
        let (start, mut first) = connector(from)?;
        let (end, last) = connector(to)?;
        let route = self.route_nodes(start, end)?;
        let lane_path = self.offset_route(&route, lane);
        let lane_start = *lane_path.first()?;
        // The graph start may itself be shifted. This sideways connector is
        // checked independently, not assumed valid from the adjacent lane.
        if !self.segment_walkable_exact(self.node_point(start), lane_start) {
            return None;
        }
        first.points.extend(lane_path);
        first.points.extend(last.points.into_iter().rev());
        first.points.dedup();
        if !first
            .points
            .windows(2)
            .all(|p| self.segment_walkable_exact(p[0], p[1]))
        {
            return None;
        }
        first.length_m = first.points.windows(2).map(|p| p[0].distance(p[1])).sum();
        first.expanded_cells += last.expanded_cells;
        Some(first)
    }

    /// Conservative supercover of the eroded surface. Check every grid crossing
    /// and each interval between crossings, including BOTH cells along a grid
    /// boundary and all FOUR cells at a corner. A brief corner clip cannot hide
    /// between samples. Local connectors, shared lanes and actual humanoid
    /// movement all use this same swept predicate.
    pub fn segment_walkable_exact(&self, a: Vec3, b: Vec3) -> bool {
        if !a.is_finite()
            || !b.is_finite()
            || !self.is_walkable(a.x, a.z)
            || !self.is_walkable(b.x, b.z)
        {
            return false;
        }
        let g = self.grid;
        let ax = (a.x - g.x0) / g.cell_m;
        let az = (a.z - g.z0) / g.cell_m;
        let bx = (b.x - g.x0) / g.cell_m;
        let bz = (b.z - g.z0) / g.cell_m;
        let mut times = vec![0.0, 1.0];
        for (start, end) in [(ax, bx), (az, bz)] {
            if (end - start).abs() < 1e-12 {
                continue;
            }
            for line in start.min(end).ceil() as i64..=start.max(end).floor() as i64 {
                let t = (line as f64 - start) / (end - start);
                if t > 0.0 && t < 1.0 {
                    times.push(t);
                }
            }
        }
        times.sort_unstable_by(f64::total_cmp);
        let clear = |t: f64| {
            let x = ax + (bx - ax) * t;
            let z = az + (bz - az) * t;
            let cols = if (x - x.round()).abs() < 1e-8 {
                [x.round() as isize - 1, x.round() as isize]
            } else {
                [x.floor() as isize; 2]
            };
            let rows = if (z - z.round()).abs() < 1e-8 {
                [z.round() as isize - 1, z.round() as isize]
            } else {
                [z.floor() as isize; 2]
            };
            rows.into_iter().all(|r| {
                cols.into_iter().all(|c| {
                    r >= 0
                        && c >= 0
                        && (r as usize) < g.h
                        && (c as usize) < g.w
                        && self.cell_walkable(r as usize, c as usize)
                })
            })
        };
        times.iter().all(|&t| clear(t)) && times.windows(2).all(|ts| clear((ts[0] + ts[1]) * 0.5))
    }

    /// A bounded eight-neighbour A* on the quarter-metre surface. The route
    /// budget includes both exact endpoint connectors. Failure is ordinary;
    /// callers retain their current spot and back off before another attempt.
    pub fn local_route(&self, from: Vec3, to: Vec3, budget: LocalPathBudget) -> Option<LocalRoute> {
        let limit = budget.max_distance_m;
        // Public callers cannot accidentally request a city-sized grid search.
        if !limit.is_finite()
            || !(0.0..=30.0).contains(&limit)
            || budget.max_expansions == 0
            || budget.max_expansions > 8192
            || !from.is_finite()
            || !to.is_finite()
        {
            return None;
        }
        let from = Vec3::new(from.x, WALK_Y, from.z);
        let to = Vec3::new(to.x, WALK_Y, to.z);
        let direct = from.distance(to);
        if direct > limit {
            return None;
        }
        if self.segment_walkable_exact(from, to) {
            return Some(LocalRoute {
                points: vec![from, to],
                length_m: direct,
                expanded_cells: 0,
            });
        }
        let g = self.grid;
        let (sr, sc) = g.cell(from.x, from.z)?;
        let (tr, tc) = g.cell(to.x, to.z)?;
        let centre = |r, c| {
            let (x, z) = g.centre(r, c);
            Vec3::new(x, WALK_Y, z)
        };
        let start = centre(sr, sc);
        let target = centre(tr, tc);
        if !self.segment_walkable_exact(from, start) || !self.segment_walkable_exact(target, to) {
            return None;
        }
        let radius = (limit / g.cell_m).ceil() as usize + 1;
        let lo_r = sr.saturating_sub(radius);
        let lo_c = sc.saturating_sub(radius);
        let hi_r = (sr + radius).min(g.h - 1);
        let hi_c = (sc + radius).min(g.w - 1);
        if tr < lo_r || tr > hi_r || tc < lo_c || tc > hi_c {
            return None;
        }
        let w = hi_c - lo_c + 1;
        let cells = w.checked_mul(hi_r - lo_r + 1)?;
        if cells > 65536 {
            return None;
        }
        let index = |r, c| (r - lo_r) * w + c - lo_c;
        let cell = |i: usize| (i / w + lo_r, i % w + lo_c);
        let s = index(sr, sc);
        let goal = index(tr, tc);
        let mut costs = vec![f64::INFINITY; cells];
        let mut came = vec![usize::MAX; costs.len()];
        let mut closed = vec![false; costs.len()];
        let mut open = BinaryHeap::new();
        costs[s] = from.distance(start);
        open.push(HeapEntry {
            cost: costs[s] + start.distance(to),
            node: s,
        });
        let mut expanded = 0;
        while let Some(HeapEntry { node, .. }) = open.pop() {
            if closed[node] {
                continue;
            }
            if expanded == budget.max_expansions {
                return None;
            }
            expanded += 1;
            closed[node] = true;
            let (r, c) = cell(node);
            if node == goal {
                let mut raw = vec![to, target];
                let mut cur = node;
                while cur != s {
                    cur = came[cur];
                    let (r, c) = cell(cur);
                    raw.push(centre(r, c));
                }
                raw.push(from);
                raw.reverse();
                raw.dedup();
                let mut points = vec![from];
                let mut anchor = 0;
                for next in 1..raw.len() {
                    if !self.segment_walkable_exact(raw[anchor], raw[next]) {
                        if next == anchor + 1 {
                            return None;
                        }
                        points.push(raw[next - 1]);
                        anchor = next - 1;
                    }
                }
                if points.last() != Some(&to) {
                    points.push(to);
                }
                let length_m = points.windows(2).map(|p| p[0].distance(p[1])).sum();
                return (length_m <= limit).then_some(LocalRoute {
                    points,
                    length_m,
                    expanded_cells: expanded,
                });
            }
            for (dr, dc) in [
                (-1isize, 0isize),
                (0, -1),
                (0, 1),
                (1, 0),
                (-1, -1),
                (-1, 1),
                (1, -1),
                (1, 1),
            ] {
                let nr = r as isize + dr;
                let nc = c as isize + dc;
                if nr < lo_r as isize
                    || nr > hi_r as isize
                    || nc < lo_c as isize
                    || nc > hi_c as isize
                {
                    continue;
                }
                let (nr, nc) = (nr as usize, nc as usize);
                let next = index(nr, nc);
                if closed[next] || !self.cell_walkable(nr, nc) {
                    continue;
                }
                if dr != 0 && dc != 0 && (!self.cell_walkable(r, nc) || !self.cell_walkable(nr, c))
                {
                    continue;
                }
                let p = centre(nr, nc);
                let cost = costs[node]
                    + g.cell_m
                        * if dr != 0 && dc != 0 {
                            std::f64::consts::SQRT_2
                        } else {
                            1.0
                        };
                if cost < costs[next] && cost + p.distance(to) <= limit {
                    costs[next] = cost;
                    came[next] = node;
                    open.push(HeapEntry {
                        cost: cost + p.distance(to),
                        node: next,
                    });
                }
            }
        }
        None
    }
}
