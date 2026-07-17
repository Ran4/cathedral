//! Bespoke monumental sculpture meshes for the cathedral approach.
//!
//! These are modeled from lofted, swept, and feather-shaped surfaces instead
//! of assembling Bevy's geometric primitives. Their faceted bronze finish is
//! deliberate: it keeps the silhouettes legible from both street and skyline.

use std::f32::consts::TAU;

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

use crate::controller::CollisionWorld;

const DAWN_BEARER_POSITION: Vec3 = Vec3::new(-72.0, 0.0, 190.0);
const SERAPH_POSITION: Vec3 = Vec3::new(72.0, 0.0, 190.0);
const MONUMENT_HEIGHT: f32 = 30.0;
const MONUMENT_WIDTH: f32 = 10.0;

pub(super) fn build_approach_monuments(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    collision_world: &mut CollisionWorld,
) {
    let weathered_bronze = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.20, 0.17),
        metallic: 0.88,
        perceptual_roughness: 0.42,
        double_sided: true,
        ..default()
    });
    let dark_bronze = materials.add(StandardMaterial {
        base_color: Color::srgb(0.055, 0.065, 0.065),
        metallic: 0.82,
        perceptual_roughness: 0.5,
        double_sided: true,
        ..default()
    });
    let monument_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.58, 0.12),
        emissive: LinearRgba::rgb(0.12, 0.045, 0.004),
        metallic: 0.95,
        perceptual_roughness: 0.24,
        double_sided: true,
        ..default()
    });
    let impossible_light = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.72, 0.24),
        emissive: LinearRgba::rgb(18.0, 5.0, 0.32),
        perceptual_roughness: 0.12,
        double_sided: true,
        ..default()
    });
    let pedestal_stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.58, 0.53, 0.44),
        perceptual_roughness: 0.9,
        double_sided: true,
        ..default()
    });

    let plinth = meshes.add(plinth_geometry().into_mesh());
    let dawn_bronze = meshes.add(dawn_bearer_bronze_geometry().into_mesh());
    let dawn_gold = meshes.add(dawn_bearer_gold_geometry().into_mesh());
    let dawn_light = meshes.add(dawn_bearer_light_geometry().into_mesh());
    let seraph_bronze = meshes.add(seraph_bronze_geometry().into_mesh());
    let seraph_gold = meshes.add(seraph_gold_geometry().into_mesh());

    spawn_sculpture_part(commands, &plinth, &pedestal_stone, DAWN_BEARER_POSITION);
    spawn_sculpture_part(
        commands,
        &dawn_bronze,
        &weathered_bronze,
        DAWN_BEARER_POSITION,
    );
    spawn_sculpture_part(commands, &dawn_gold, &monument_gold, DAWN_BEARER_POSITION);
    spawn_sculpture_part(
        commands,
        &dawn_light,
        &impossible_light,
        DAWN_BEARER_POSITION,
    );

    spawn_sculpture_part(commands, &plinth, &pedestal_stone, SERAPH_POSITION);
    spawn_sculpture_part(commands, &seraph_bronze, &dark_bronze, SERAPH_POSITION);
    spawn_sculpture_part(commands, &seraph_gold, &monument_gold, SERAPH_POSITION);

    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.52, 0.16),
            intensity: 90_000.0,
            range: 38.0,
            radius: 1.4,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(DAWN_BEARER_POSITION + Vec3::new(1.65, 28.45, -0.2)),
    ));

    for position in [DAWN_BEARER_POSITION, SERAPH_POSITION] {
        collision_world.add_box(
            position + Vec3::new(-MONUMENT_WIDTH * 0.5, 0.0, -4.2),
            position + Vec3::new(MONUMENT_WIDTH * 0.5, MONUMENT_HEIGHT, 4.2),
        );
    }
}

fn spawn_sculpture_part(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    position: Vec3,
) {
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(position),
    ));
}

#[derive(Clone, Copy)]
struct LoftRing {
    y: f32,
    center_x: f32,
    center_z: f32,
    radius_x: f32,
    radius_z: f32,
    fold_depth: f32,
    phase: f32,
}

impl LoftRing {
    const fn new(y: f32, radius_x: f32, radius_z: f32) -> Self {
        Self {
            y,
            center_x: 0.0,
            center_z: 0.0,
            radius_x,
            radius_z,
            fold_depth: 0.0,
            phase: 0.0,
        }
    }

    const fn shifted(mut self, x: f32, z: f32) -> Self {
        self.center_x = x;
        self.center_z = z;
        self
    }

    const fn folded(mut self, depth: f32, phase: f32) -> Self {
        self.fold_depth = depth;
        self.phase = phase;
        self
    }
}

#[derive(Default)]
struct SculptureMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
}

impl SculptureMesh {
    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let normal = (b - a).cross(c - a).normalize_or(Vec3::Y);
        self.positions
            .extend([a.to_array(), b.to_array(), c.to_array()]);
        self.normals.extend([normal.to_array(); 3]);
        self.uvs.extend([[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]]);
    }

    fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        self.triangle(a, b, c);
        self.triangle(a, c, d);
    }

    fn add_loft(&mut self, rings: &[LoftRing], segments: usize, folds: usize) {
        assert!(rings.len() >= 2);
        let point = |ring: LoftRing, segment: usize| {
            let angle = segment as f32 * TAU / segments as f32;
            let ripple = 1.0
                + ring.fold_depth * (angle * folds as f32 + ring.phase).cos()
                + ring.fold_depth * 0.35 * (angle * (folds + 3) as f32).sin();
            Vec3::new(
                ring.center_x + angle.cos() * ring.radius_x * ripple,
                ring.y,
                ring.center_z + angle.sin() * ring.radius_z * ripple,
            )
        };

        for pair in rings.windows(2) {
            for segment in 0..segments {
                let next = (segment + 1) % segments;
                self.quad(
                    point(pair[0], segment),
                    point(pair[1], segment),
                    point(pair[1], next),
                    point(pair[0], next),
                );
            }
        }

        let bottom_center = Vec3::new(rings[0].center_x, rings[0].y, rings[0].center_z);
        let top = rings[rings.len() - 1];
        let top_center = Vec3::new(top.center_x, top.y, top.center_z);
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            self.triangle(
                bottom_center,
                point(rings[0], next),
                point(rings[0], segment),
            );
            self.triangle(top_center, point(top, segment), point(top, next));
        }
    }

    fn add_tube(&mut self, path: &[Vec3], radii: &[f32], sides: usize) {
        assert!(path.len() >= 2 && path.len() == radii.len());
        let mut rings = Vec::with_capacity(path.len());
        for (index, point) in path.iter().copied().enumerate() {
            let tangent = match index {
                0 => path[1] - path[0],
                i if i + 1 == path.len() => path[i] - path[i - 1],
                i => path[i + 1] - path[i - 1],
            }
            .normalize_or(Vec3::Y);
            let reference = if tangent.dot(Vec3::Y).abs() < 0.92 {
                Vec3::Y
            } else {
                Vec3::X
            };
            let axis_a = tangent.cross(reference).normalize_or(Vec3::Z);
            let axis_b = tangent.cross(axis_a).normalize_or(Vec3::X);
            let ring = (0..sides)
                .map(|side| {
                    let angle = side as f32 * TAU / sides as f32;
                    point + (axis_a * angle.cos() + axis_b * angle.sin()) * radii[index]
                })
                .collect::<Vec<_>>();
            rings.push(ring);
        }

        for pair in rings.windows(2) {
            for side in 0..sides {
                let next = (side + 1) % sides;
                self.quad(pair[0][side], pair[1][side], pair[1][next], pair[0][next]);
            }
        }
        for side in 0..sides {
            let next = (side + 1) % sides;
            self.triangle(path[0], rings[0][side], rings[0][next]);
            let last = rings.len() - 1;
            self.triangle(path[last], rings[last][next], rings[last][side]);
        }
    }

    fn add_extruded_polygon(&mut self, polygon: &[Vec2], z: f32, thickness: f32) {
        assert!(polygon.len() >= 3);
        let front_z = z - thickness * 0.5;
        let back_z = z + thickness * 0.5;
        let front = |point: Vec2| Vec3::new(point.x, point.y, front_z);
        let back = |point: Vec2| Vec3::new(point.x, point.y, back_z);

        for index in 1..polygon.len() - 1 {
            self.triangle(
                front(polygon[0]),
                front(polygon[index + 1]),
                front(polygon[index]),
            );
            self.triangle(
                back(polygon[0]),
                back(polygon[index]),
                back(polygon[index + 1]),
            );
        }
        for index in 0..polygon.len() {
            let next = (index + 1) % polygon.len();
            self.quad(
                front(polygon[index]),
                back(polygon[index]),
                back(polygon[next]),
                front(polygon[next]),
            );
        }
    }

    fn add_feather(&mut self, root: Vec3, tip: Vec3, width: f32, thickness: f32) {
        let direction = (tip.truncate() - root.truncate()).normalize_or(Vec2::Y);
        let perpendicular = Vec2::new(-direction.y, direction.x);
        let root_2d = root.truncate();
        let tip_2d = tip.truncate();
        let middle = root_2d.lerp(tip_2d, 0.48);
        let polygon = [
            root_2d - perpendicular * width * 0.16,
            middle - perpendicular * width * 0.5,
            tip_2d,
            middle + perpendicular * width * 0.5,
            root_2d + perpendicular * width * 0.16,
        ];
        self.add_extruded_polygon(&polygon, (root.z + tip.z) * 0.5, thickness);
    }

    fn add_feather_fan(&mut self, root: Vec3, tips: &[Vec3], width: f32) {
        for (index, tip) in tips.iter().copied().enumerate() {
            let feather_root = root
                + Vec3::new(
                    (index as f32 - (tips.len() - 1) as f32 * 0.5) * 0.12,
                    index as f32 * 0.12,
                    index as f32 * 0.025,
                );
            self.add_feather(
                feather_root,
                tip,
                width * (1.0 - index as f32 * 0.035),
                0.24,
            );
        }
        if let Some(tip) = tips.get(tips.len() / 2) {
            self.add_tube(&[root, root.lerp(*tip, 0.72)], &[0.3, 0.13], 7);
        }
    }

    fn add_almond(&mut self, center: Vec3, half_width: f32, half_height: f32) {
        let polygon = [
            Vec2::new(center.x - half_width, center.y),
            Vec2::new(center.x, center.y - half_height),
            Vec2::new(center.x + half_width, center.y),
            Vec2::new(center.x, center.y + half_height),
        ];
        self.add_extruded_polygon(&polygon, center.z, 0.1);
    }

    fn add_broken_halo(&mut self, center: Vec3, radius: f32) {
        const SEGMENTS: usize = 28;
        const GAPS: [usize; 5] = [2, 3, 12, 20, 21];
        let outer = radius + 0.16;
        let inner = radius - 0.16;
        for segment in 0..SEGMENTS {
            if GAPS.contains(&segment) {
                continue;
            }
            let a = segment as f32 * TAU / SEGMENTS as f32;
            let b = (segment + 1) as f32 * TAU / SEGMENTS as f32;
            let polygon = [
                center.truncate() + Vec2::new(a.cos(), a.sin()) * inner,
                center.truncate() + Vec2::new(a.cos(), a.sin()) * outer,
                center.truncate() + Vec2::new(b.cos(), b.sin()) * outer,
                center.truncate() + Vec2::new(b.cos(), b.sin()) * inner,
            ];
            self.add_extruded_polygon(&polygon, center.z, 0.18);
        }
    }

    #[cfg(test)]
    fn bounds(&self) -> (Vec3, Vec3) {
        self.positions.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), position| {
                let position = Vec3::from_array(*position);
                (min.min(position), max.max(position))
            },
        )
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
    }
}

fn plinth_geometry() -> SculptureMesh {
    let mut mesh = SculptureMesh::default();
    mesh.add_loft(
        &[
            LoftRing::new(0.0, 4.5, 4.1),
            LoftRing::new(0.65, 4.5, 4.1),
            LoftRing::new(0.9, 4.05, 3.7),
            LoftRing::new(2.2, 3.55, 3.25),
            LoftRing::new(2.55, 3.85, 3.5),
            LoftRing::new(3.0, 3.15, 2.9),
        ],
        12,
        0,
    );
    mesh
}

fn dawn_bearer_bronze_geometry() -> SculptureMesh {
    let mut mesh = SculptureMesh::default();

    mesh.add_loft(
        &[
            LoftRing::new(3.0, 3.1, 2.25)
                .shifted(-0.1, 0.0)
                .folded(0.08, 0.0),
            LoftRing::new(6.0, 2.9, 2.05)
                .shifted(-0.35, 0.0)
                .folded(0.1, 0.5),
            LoftRing::new(10.5, 2.45, 1.72)
                .shifted(0.1, 0.0)
                .folded(0.11, 1.0),
            LoftRing::new(14.5, 1.85, 1.45)
                .shifted(-0.15, -0.05)
                .folded(0.08, 1.4),
            LoftRing::new(16.2, 1.5, 1.25)
                .shifted(0.1, -0.1)
                .folded(0.06, 1.8),
        ],
        18,
        9,
    );
    mesh.add_loft(
        &[
            LoftRing::new(15.5, 1.55, 1.15).shifted(0.1, -0.05),
            LoftRing::new(18.3, 1.85, 1.15).shifted(0.15, -0.1),
            LoftRing::new(20.8, 1.55, 1.0).shifted(0.25, -0.15),
            LoftRing::new(22.0, 1.18, 0.86).shifted(0.2, -0.18),
        ],
        16,
        0,
    );
    mesh.add_loft(
        &[
            LoftRing::new(21.2, 1.08, 0.88).shifted(0.1, -0.2),
            LoftRing::new(22.1, 1.18, 0.94).shifted(0.05, -0.24),
            LoftRing::new(23.35, 0.94, 0.78).shifted(0.05, -0.28),
            LoftRing::new(24.05, 0.42, 0.38).shifted(0.02, -0.2),
        ],
        14,
        0,
    );
    // A distinct brow and nose keep the monumental face readable from below.
    mesh.add_extruded_polygon(
        &[
            Vec2::new(-0.64, 22.85),
            Vec2::new(0.66, 22.85),
            Vec2::new(0.45, 23.18),
            Vec2::new(-0.48, 23.18),
        ],
        -1.02,
        0.22,
    );
    mesh.add_tube(
        &[
            Vec3::new(0.05, 23.15, -1.02),
            Vec3::new(0.02, 22.65, -1.2),
            Vec3::new(0.08, 22.42, -1.05),
        ],
        &[0.18, 0.22, 0.12],
        7,
    );

    // The lifted lantern arm and the balancing, wind-swept arm.
    mesh.add_tube(
        &[
            Vec3::new(1.3, 20.3, -0.05),
            Vec3::new(2.1, 22.8, -0.35),
            Vec3::new(1.72, 25.7, -0.28),
            Vec3::new(1.65, 27.0, -0.22),
        ],
        &[0.72, 0.62, 0.48, 0.35],
        9,
    );
    mesh.add_tube(
        &[
            Vec3::new(-1.25, 20.1, 0.0),
            Vec3::new(-2.35, 18.2, -0.65),
            Vec3::new(-2.72, 15.65, -0.78),
        ],
        &[0.7, 0.58, 0.36],
        9,
    );

    for side in [-1.0, 1.0] {
        let root = Vec3::new(side * 0.82, 20.5, 0.7);
        let tips = [
            Vec3::new(side * 2.5, 27.0, 0.85),
            Vec3::new(side * 3.55, 26.0, 0.88),
            Vec3::new(side * 4.35, 24.65, 0.9),
            Vec3::new(side * 4.85, 22.9, 0.92),
            Vec3::new(side * 4.55, 21.0, 0.94),
            Vec3::new(side * 3.9, 19.5, 0.96),
        ];
        mesh.add_feather_fan(root, &tips, 1.55);
    }

    mesh
}

fn dawn_bearer_gold_geometry() -> SculptureMesh {
    let mut mesh = SculptureMesh::default();
    let center = Vec3::new(1.65, 28.35, -0.2);
    mesh.add_loft(
        &[
            LoftRing::new(27.0, 0.72, 0.62).shifted(center.x, center.z),
            LoftRing::new(27.25, 1.02, 0.86).shifted(center.x, center.z),
            LoftRing::new(29.35, 0.92, 0.78).shifted(center.x, center.z),
            LoftRing::new(29.55, 0.48, 0.42).shifted(center.x, center.z),
        ],
        8,
        0,
    );
    for x in [-0.72, 0.72] {
        for z in [-0.56, 0.56] {
            mesh.add_tube(
                &[
                    Vec3::new(center.x + x, 27.25, center.z + z),
                    Vec3::new(center.x + x * 0.92, 29.35, center.z + z * 0.92),
                ],
                &[0.09, 0.07],
                6,
            );
        }
    }
    mesh.add_tube(
        &[
            Vec3::new(center.x - 0.55, 29.45, center.z),
            Vec3::new(center.x, 30.0, center.z),
            Vec3::new(center.x + 0.55, 29.45, center.z),
        ],
        &[0.11, 0.1, 0.11],
        7,
    );
    mesh
}

fn dawn_bearer_light_geometry() -> SculptureMesh {
    let mut mesh = SculptureMesh::default();
    mesh.add_loft(
        &[
            LoftRing::new(27.55, 0.5, 0.42).shifted(1.65, -0.2),
            LoftRing::new(28.35, 0.73, 0.62).shifted(1.65, -0.2),
            LoftRing::new(29.05, 0.45, 0.38).shifted(1.65, -0.2),
        ],
        8,
        0,
    );
    mesh
}

fn seraph_bronze_geometry() -> SculptureMesh {
    let mut mesh = SculptureMesh::default();
    mesh.add_loft(
        &[
            LoftRing::new(3.0, 3.0, 2.2).folded(0.08, 0.4),
            LoftRing::new(7.0, 2.7, 1.9)
                .shifted(0.1, 0.05)
                .folded(0.1, 0.8),
            LoftRing::new(12.5, 2.05, 1.55)
                .shifted(-0.12, 0.0)
                .folded(0.09, 1.2),
            LoftRing::new(16.8, 1.55, 1.2)
                .shifted(0.05, -0.05)
                .folded(0.06, 1.6),
        ],
        18,
        8,
    );
    mesh.add_loft(
        &[
            LoftRing::new(16.0, 1.5, 1.12),
            LoftRing::new(19.2, 1.82, 1.18).shifted(0.0, -0.05),
            LoftRing::new(21.7, 1.35, 0.96).shifted(0.0, -0.1),
            LoftRing::new(22.5, 1.0, 0.82).shifted(0.0, -0.15),
        ],
        16,
        0,
    );
    mesh.add_loft(
        &[
            LoftRing::new(21.6, 1.0, 0.82).shifted(0.0, -0.18),
            LoftRing::new(22.7, 1.1, 0.86).shifted(0.0, -0.2),
            LoftRing::new(23.8, 0.84, 0.7).shifted(0.0, -0.18),
            LoftRing::new(24.3, 0.34, 0.3).shifted(0.0, -0.1),
        ],
        14,
        0,
    );

    // The face is fully veiled; the long point makes the absence intentional.
    mesh.add_extruded_polygon(
        &[
            Vec2::new(-0.92, 23.25),
            Vec2::new(-0.5, 21.95),
            Vec2::new(0.0, 20.9),
            Vec2::new(0.5, 21.95),
            Vec2::new(0.92, 23.25),
            Vec2::new(0.0, 23.75),
        ],
        -0.95,
        0.26,
    );

    // Crossed arms make the still figure contrast with the striding bearer.
    mesh.add_tube(
        &[
            Vec3::new(-1.3, 20.1, -0.1),
            Vec3::new(-0.8, 18.2, -1.05),
            Vec3::new(0.58, 17.25, -1.25),
        ],
        &[0.68, 0.52, 0.31],
        9,
    );
    mesh.add_tube(
        &[
            Vec3::new(1.3, 20.1, -0.08),
            Vec3::new(0.8, 18.15, -1.08),
            Vec3::new(-0.58, 17.15, -1.3),
        ],
        &[0.68, 0.52, 0.31],
        9,
    );

    for side in [-1.0, 1.0] {
        // Upper wings rise around the broken halo.
        mesh.add_feather_fan(
            Vec3::new(side * 0.72, 21.5, 0.72),
            &[
                Vec3::new(side * 1.7, 29.8, 0.92),
                Vec3::new(side * 2.45, 29.2, 0.95),
                Vec3::new(side * 3.1, 28.35, 0.98),
                Vec3::new(side * 3.65, 27.2, 1.0),
                Vec3::new(side * 3.9, 25.9, 1.02),
            ],
            1.35,
        );
        // Middle wings form the ten-metre-wide heraldic silhouette.
        mesh.add_feather_fan(
            Vec3::new(side * 0.95, 19.8, 0.82),
            &[
                Vec3::new(side * 4.15, 25.3, 1.02),
                Vec3::new(side * 4.75, 24.2, 1.04),
                Vec3::new(side * 5.0, 22.85, 1.06),
                Vec3::new(side * 4.8, 21.45, 1.08),
                Vec3::new(side * 4.35, 20.2, 1.1),
            ],
            1.45,
        );
        // Lower wings wrap downward like a second feathered robe.
        mesh.add_feather_fan(
            Vec3::new(side * 0.9, 18.5, 0.72),
            &[
                Vec3::new(side * 4.25, 16.4, 0.98),
                Vec3::new(side * 4.3, 14.4, 1.0),
                Vec3::new(side * 4.05, 12.2, 1.02),
                Vec3::new(side * 3.6, 10.15, 1.04),
                Vec3::new(side * 2.9, 8.3, 1.06),
            ],
            1.45,
        );
    }

    mesh
}

fn seraph_gold_geometry() -> SculptureMesh {
    let mut mesh = SculptureMesh::default();
    mesh.add_broken_halo(Vec3::new(0.0, 24.3, 0.68), 1.78);

    // Traditional many-eyed wings, reduced to a few large inlays so the twist
    // reads from street level instead of becoming surface noise.
    for side in [-1.0, 1.0] {
        for (x, y, z) in [
            (2.55, 25.6, -0.01),
            (3.65, 23.15, 0.03),
            (2.75, 20.85, 0.07),
            (3.3, 15.0, 0.09),
            (2.65, 11.7, 0.12),
        ] {
            mesh.add_almond(Vec3::new(side * x, y, z), 0.34, 0.15);
        }
    }
    // A single vertical seam of light is all the veiled face reveals.
    mesh.add_extruded_polygon(
        &[
            Vec2::new(-0.055, 23.35),
            Vec2::new(0.055, 23.35),
            Vec2::new(0.035, 21.75),
            Vec2::new(-0.035, 21.75),
        ],
        -1.105,
        0.08,
    );
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monuments_have_the_requested_scale_and_dense_authored_geometry() {
        let dawn = dawn_bearer_bronze_geometry();
        let seraph = seraph_bronze_geometry();
        let dawn_gold = dawn_bearer_gold_geometry();

        let (dawn_min, dawn_max) = dawn.bounds();
        let (seraph_min, seraph_max) = seraph.bounds();
        let (_, gold_max) = dawn_gold.bounds();

        assert!(dawn_min.y >= 3.0 && seraph_min.y >= 3.0);
        assert!(gold_max.y >= MONUMENT_HEIGHT - 0.1);
        assert!(gold_max.y <= MONUMENT_HEIGHT + 0.25);
        assert!((dawn_max.x - dawn_min.x) <= MONUMENT_WIDTH);
        assert!((seraph_max.x - seraph_min.x) <= MONUMENT_WIDTH);
        assert!(dawn.positions.len() > 2_000);
        assert!(seraph.positions.len() > 3_000);
    }

    #[test]
    fn monuments_flank_the_approach_without_blocking_it() {
        let inner_dawn_edge = DAWN_BEARER_POSITION.x + MONUMENT_WIDTH * 0.5;
        let inner_seraph_edge = SERAPH_POSITION.x - MONUMENT_WIDTH * 0.5;
        assert!(inner_dawn_edge < -GRAND_APPROACH_CLEAR_HALF_WIDTH);
        assert!(inner_seraph_edge > GRAND_APPROACH_CLEAR_HALF_WIDTH);
    }

    const GRAND_APPROACH_CLEAR_HALF_WIDTH: f32 = 12.0;
}
