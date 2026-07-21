//! Lightweight semantic queries over the rendered street ribbons.
//!
//! The road meshes are deliberately batched, so querying the renderer cannot
//! tell a footstep which surface is under it.  This resource is built from the
//! same typed plan and the same half-width rule as `add_road_ribbon`; it is the
//! small read-only seam presentation systems need without duplicating the city
//! plan or ray-casting a kilometre-wide mesh.

use bevy::prelude::*;

use super::plan::Road;

#[derive(Debug, Clone, Copy)]
struct CobbleSegment {
    a: Vec2,
    b: Vec2,
    half_width: f32,
}

/// The union of every non-Cut road ribbon in the authoritative city plan.
#[derive(Resource, Debug, Clone, Default)]
pub(crate) struct CobbleRoadNetwork {
    segments: Vec<CobbleSegment>,
}

impl CobbleRoadNetwork {
    pub(super) fn from_roads(roads: &[Road]) -> Self {
        let segments = roads
            .iter()
            .filter(|road| road.tier != "cut")
            .flat_map(|road| {
                road.points.windows(2).map(|pair| CobbleSegment {
                    a: Vec2::from_array(pair[0]),
                    b: Vec2::from_array(pair[1]),
                    half_width: road.width_m * 0.5,
                })
            })
            .collect();
        Self { segments }
    }

    /// Whether an XZ point lies on the same cobbled union the renderer drew.
    pub(crate) fn contains(&self, point: Vec2) -> bool {
        point.is_finite()
            && self.segments.iter().any(|segment| {
                distance_squared_to_segment(point, segment.a, segment.b)
                    <= segment.half_width * segment.half_width
            })
    }
}

fn distance_squared_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let edge = b - a;
    let length_squared = edge.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance_squared(a);
    }
    let along = ((point - a).dot(edge) / length_squared).clamp(0.0, 1.0);
    point.distance_squared(a + edge * along)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn road(tier: &str, width_m: f32, points: &[[f32; 2]]) -> Road {
        Road {
            id: format!("{tier}-road"),
            name: "test road".into(),
            points: points.to_vec(),
            width_m,
            tier: tier.into(),
            label: false,
        }
    }

    #[test]
    fn network_matches_ribbon_width_and_round_ends() {
        let roads = [road("lane", 4.0, &[[0.0, 0.0], [10.0, 0.0]])];
        let network = CobbleRoadNetwork::from_roads(&roads);

        assert!(network.contains(Vec2::new(5.0, 1.99)));
        assert!(!network.contains(Vec2::new(5.0, 2.01)));
        assert!(network.contains(Vec2::new(-1.9, 0.0)));
        assert!(!network.contains(Vec2::new(-2.1, 0.0)));
    }

    #[test]
    fn dry_cut_is_not_classified_as_cobbles() {
        let roads = [
            road("cut", 8.0, &[[0.0, 0.0], [10.0, 0.0]]),
            road("street", 4.0, &[[20.0, 0.0], [30.0, 0.0]]),
        ];
        let network = CobbleRoadNetwork::from_roads(&roads);

        assert!(!network.contains(Vec2::new(5.0, 0.0)));
        assert!(network.contains(Vec2::new(25.0, 0.0)));
        assert!(!network.contains(Vec2::new(f32::NAN, 0.0)));
    }
}
