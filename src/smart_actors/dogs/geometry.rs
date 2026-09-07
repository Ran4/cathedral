//! Small, closed dog surfaces. The profile may run in either direction:
//! winding is chosen from the actual axial direction, never from ring order.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy)]
pub(super) struct Section {
    pub center: Vec3,
    pub radii: Vec2,
}

impl Section {
    pub fn new(x: f32, y: f32, z: f32, a: f32, b: f32) -> Self {
        Self {
            center: Vec3::new(x, y, z),
            radii: Vec2::new(a, b),
        }
    }
}

#[derive(Default)]
pub(super) struct Surface {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl Surface {
    /// A bounded sculpt of an existing surface, transporting normals through
    /// the deformation's Jacobian so a socket is a recess in the skull.
    pub fn deform(&mut self, warp: impl Fn(Vec3) -> Vec3) {
        let epsilon = 0.0002;
        for (position, normal) in self.positions.iter_mut().zip(&mut self.normals) {
            let p = Vec3::from_array(*position);
            let jacobian = Mat3::from_cols(
                (warp(p + Vec3::X * epsilon) - warp(p - Vec3::X * epsilon)) / (epsilon * 2.0),
                (warp(p + Vec3::Y * epsilon) - warp(p - Vec3::Y * epsilon)) / (epsilon * 2.0),
                (warp(p + Vec3::Z * epsilon) - warp(p - Vec3::Z * epsilon)) / (epsilon * 2.0),
            );
            *position = warp(p).to_array();
            // Cofactors also transport a sole that has been flattened in one
            // direction: inverse-transpose would be singular at that pad.
            let cofactors = Mat3::from_cols(
                jacobian.y_axis.cross(jacobian.z_axis),
                jacobian.z_axis.cross(jacobian.x_axis),
                jacobian.x_axis.cross(jacobian.y_axis),
            );
            *normal = (cofactors * Vec3::from_array(*normal))
                .normalize_or(Vec3::from_array(*normal))
                .to_array();
        }
    }
    pub fn vertex(&mut self, p: Vec3, n: Vec3, color: Vec3) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push(p.to_array());
        self.normals.push(n.normalize_or(Vec3::Y).to_array());
        self.uvs.push([p.x * 6.25, p.z * 6.25]);
        self.colors.push(
            Color::srgb(color.x, color.y, color.z)
                .to_linear()
                .to_f32_array(),
        );
        i
    }

    /// Cubic Hermite interpolation keeps anatomical landmarks smooth without
    /// the visible stacked cones of a sparse polygon profile.
    pub fn tube(
        &mut self,
        points: &[Section],
        u: Vec3,
        v: Vec3,
        sectors: usize,
        subdivisions: usize,
        paint: impl Fn(Vec3) -> Vec3,
    ) {
        let mut rings = Vec::new();
        for segment in 0..points.len() - 1 {
            let a = points[segment.saturating_sub(1)];
            let b = points[segment];
            let c = points[segment + 1];
            let d = points[(segment + 2).min(points.len() - 1)];
            for step in 0..subdivisions {
                let t = step as f32 / subdivisions as f32;
                let h = Vec4::new(
                    2.0 * t * t * t - 3.0 * t * t + 1.0,
                    t * t * t - 2.0 * t * t + t,
                    -2.0 * t * t * t + 3.0 * t * t,
                    t * t * t - t * t,
                );
                rings.push(Section {
                    center: b.center * h.x
                        + (c.center - a.center) * 0.5 * h.y
                        + c.center * h.z
                        + (d.center - b.center) * 0.5 * h.w,
                    radii: (b.radii * h.x
                        + (c.radii - a.radii) * 0.5 * h.y
                        + c.radii * h.z
                        + (d.radii - b.radii) * 0.5 * h.w)
                        .max(Vec2::splat(0.001)),
                });
            }
        }
        rings.push(*points.last().unwrap());
        let offset = self.positions.len() as u32;
        let axial = u.cross(v);
        let direction = (points.last().unwrap().center - points[0].center)
            .dot(axial)
            .signum();
        let point = |r: Section, angle: f32| {
            r.center + u * r.radii.x * angle.cos() + v * r.radii.y * angle.sin()
        };
        let circumference = TAU
            * points
                .iter()
                .map(|p| (p.radii.x + p.radii.y) * 0.5)
                .sum::<f32>()
            / points.len() as f32;
        let mut distance = 0.0;
        for (i, ring) in rings.iter().enumerate() {
            if i > 0 {
                distance += (ring.center - rings[i - 1].center).length();
            }
            for j in 0..=sectors {
                let angle = j as f32 * TAU / sectors as f32;
                let p = point(*ring, angle);
                let around = -u * ring.radii.x * angle.sin() + v * ring.radii.y * angle.cos();
                let along = point(rings[(i + 1).min(rings.len() - 1)], angle)
                    - point(rings[i.saturating_sub(1)], angle);
                let index = self.vertex(p, around.cross(along) * direction, paint(p));
                self.uvs[index as usize] = [
                    j as f32 / sectors as f32 * circumference / 0.31,
                    distance / 0.31,
                ];
            }
        }
        for i in 0..rings.len() - 1 {
            for j in 0..sectors {
                let a = offset + (i * (sectors + 1) + j) as u32;
                let b = a + 1;
                let c = a + (sectors + 1) as u32;
                let d = b + (sectors + 1) as u32;
                if direction > 0.0 {
                    self.indices.extend([a, b, c, b, d, c]);
                } else {
                    self.indices.extend([a, c, b, b, c, d]);
                }
            }
        }
        // Independent cap vertices preserve a flat end normal. Every limb
        // is closed even when its joint temporarily becomes visible.
        for (ring, sign) in [(rings[0], -direction), (*rings.last().unwrap(), direction)] {
            let c = self.vertex(ring.center, axial * sign, paint(ring.center));
            for j in 0..sectors {
                let p = point(ring, j as f32 * TAU / sectors as f32);
                self.vertex(p, axial * sign, paint(p));
            }
            for j in 0..sectors as u32 {
                let a = c + 1 + j;
                let b = c + 1 + (j + 1) % sectors as u32;
                if sign > 0.0 {
                    self.indices.extend([c, a, b]);
                } else {
                    self.indices.extend([c, b, a]);
                }
            }
        }
    }

    pub fn ellipsoid(
        &mut self,
        center: Vec3,
        radii: Vec3,
        rotation: Quat,
        sectors: usize,
        latitude: usize,
        paint: impl Fn(Vec3) -> Vec3,
    ) {
        let base = self.positions.len() as u32;
        for i in 0..=latitude {
            let phi = PI * i as f32 / latitude as f32;
            for j in 0..sectors {
                let theta = TAU * j as f32 / sectors as f32;
                let unit = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
                let p = center + rotation * (unit * radii);
                self.vertex(p, rotation * (unit / radii), paint(p));
            }
        }
        for i in 0..latitude {
            for j in 0..sectors {
                let a = base + (i * sectors + j) as u32;
                let b = base + (i * sectors + (j + 1) % sectors) as u32;
                let c = a + sectors as u32;
                let d = b + sectors as u32;
                if i > 0 {
                    self.indices.extend([a, b, c]);
                }
                if i + 1 < latitude {
                    self.indices.extend([b, d, c]);
                }
            }
        }
    }

    pub fn finish(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_closed_surface_normals(surface: &Surface) {
        for (p, n) in surface.positions.iter().zip(&surface.normals) {
            assert!(Vec3::from_array(*p).is_finite());
            let n = Vec3::from_array(*n);
            assert!(n.is_finite() && (n.length() - 1.0).abs() < 0.001);
        }
        for face in surface.indices.chunks_exact(3) {
            let p = [face[0], face[1], face[2]]
                .map(|i| Vec3::from_array(surface.positions[i as usize]));
            let geometric = (p[1] - p[0]).cross(p[2] - p[0]);
            if geometric.length_squared() < 1e-15 {
                continue;
            }
            let shaded: Vec3 = face
                .iter()
                .map(|i| Vec3::from_array(surface.normals[*i as usize]))
                .sum();
            assert!(
                geometric.dot(shaded) > 0.0,
                "winding opposes the vertex normals"
            );
        }
    }

    #[test]
    fn descending_and_ascending_limbs_have_outward_sides_and_caps() {
        for direction in [-1.0, 1.0] {
            let mut s = Surface::default();
            s.tube(
                &[
                    Section::new(0.0, 0.0, 0.0, 0.05, 0.045),
                    Section::new(0.0, 0.1 * direction, 0.0, 0.04, 0.035),
                    Section::new(0.0, 0.2 * direction, 0.0, 0.03, 0.025),
                ],
                Vec3::X,
                Vec3::Z,
                16,
                3,
                |_| Vec3::ONE,
            );
            assert_closed_surface_normals(&s);
            for (p, n) in s.positions.iter().zip(&s.normals).take(17 * 7) {
                assert!(p[0] * n[0] + p[2] * n[2] > 0.0);
            }
            let cap_start = 17 * 7;
            assert!(s.normals[cap_start][1] * direction < -0.99);
            assert!(s.normals[cap_start + 17][1] * direction > 0.99);
        }
    }

    #[test]
    fn ellipsoid_faces_and_smooth_normals_agree() {
        let mut s = Surface::default();
        s.ellipsoid(
            Vec3::ZERO,
            Vec3::new(0.1, 0.03, 0.06),
            Quat::from_rotation_y(0.6),
            16,
            8,
            |_| Vec3::ONE,
        );
        assert_closed_surface_normals(&s);
        for (p, n) in s.positions.iter().zip(&s.normals) {
            assert!(Vec3::from_array(*p).dot(Vec3::from_array(*n)) > 0.0);
        }
    }
}
