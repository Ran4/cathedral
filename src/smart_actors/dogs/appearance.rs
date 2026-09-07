//! Authored street-dog anatomy and coat masks, baked once into shared meshes.
use super::geometry::{Section as S, Surface};
use bevy::mesh::{VertexAttributeValues, skinning::SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use cathedral_sim::DogCoat;

pub(super) struct Form {
    pub body: Handle<Mesh>,
    pub head: Handle<Mesh>,
    pub features: Handle<Mesh>,
    pub eyes: Handle<Mesh>,
    pub ears: [Handle<Mesh>; 2],
    pub front_upper: [Handle<Mesh>; 2],
    pub rear_upper: [Handle<Mesh>; 2],
    pub front_lower: Handle<Mesh>,
    pub rear_lower: Handle<Mesh>,
    pub tail: Handle<Mesh>,
    pub width: f32,
    pub length: f32,
    pub head_scale: Vec3,
    pub neck_bind: Handle<SkinnedMeshInverseBindposes>,
    pub upper_bind: [Handle<SkinnedMeshInverseBindposes>; 4],
}

fn blend(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a.lerp(b, t.clamp(0.0, 1.0))
}
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn eye_frame(sign: f32) -> (Vec3, Vec3, Vec3, Vec3) {
    let center = Vec3::new(sign * 0.052, 0.187, -0.193);
    let normal = Vec3::new(sign * 0.67, 0.26, -0.694).normalize();
    let across = Vec3::new(normal.z, 0.0, -normal.x).normalize();
    let up = normal.cross(across).normalize();
    (center, across, up, normal)
}

/// The lid follows the skull's curvature around a narrow almond opening.
/// This is a continuous fur/skin collar; the visible eye is a shallow lens
/// within it rather than an exposed spherical bead.
fn orbital_ring(
    s: &mut Surface,
    sign: f32,
    rings: &[(f32, f32, f32)],
    paint: impl Fn(Vec3, usize) -> Vec3,
) {
    const SECTORS: usize = 16;
    let (center, across, up, normal) = eye_frame(sign);
    let start = s.positions.len() as u32;
    for (ring, &(width, height, depth)) in rings.iter().enumerate() {
        for j in 0..SECTORS {
            let angle = std::f32::consts::TAU * j as f32 / SECTORS as f32;
            let x = width * angle.cos();
            let y = height * angle.sin() * (0.75 + 0.25 * angle.sin().abs());
            let p = center + across * x + up * y + normal * depth;
            let n = (normal + across * (x / 0.045) + up * (y / 0.040)).normalize();
            s.vertex(p, n, paint(p, ring));
        }
    }
    for ring in 0..rings.len() - 1 {
        for j in 0..SECTORS {
            let a = start + (ring * SECTORS + j) as u32;
            let b = start + (ring * SECTORS + (j + 1) % SECTORS) as u32;
            let c = a + SECTORS as u32;
            let d = b + SECTORS as u32;
            // The rings run from the outer contour toward the pupil.
            s.indices.extend([a, b, c, b, d, c]);
        }
    }
}

/// Pigment lives in anatomical coordinates, so joints do not change color
/// abruptly. Broad masks stay legible at distance; bounded mottling softens
/// their edges without a texture, extra draw, or any update-loop work.
fn coat(coat: DogCoat, p: Vec3) -> Vec3 {
    let grain = (p.x * 109.0 + (p.z * 71.0).sin() * 1.6).sin() * (p.y * 93.0 - p.z * 31.0).sin();
    let soft = (p.z * 21.0 + p.y * 14.0).sin() * (p.x * 28.0 - p.y * 6.0).sin();
    let lower = 1.0 - smooth(0.23, 0.46, p.y);
    let underside = smooth(0.0, 0.09, -p.y + 0.40) * (1.0 - smooth(0.03, 0.13, p.x.abs()));
    let color = match coat {
        DogCoat::Brindle => {
            let c = blend(
                Vec3::new(0.46, 0.32, 0.195),
                Vec3::new(0.29, 0.245, 0.19),
                smooth(0.43, 0.56, p.y) * 0.35,
            );
            blend(
                c,
                Vec3::new(0.58, 0.45, 0.30),
                lower * 0.24 + underside * 0.20,
            )
        }
        DogCoat::Black => {
            let c = Vec3::new(0.115, 0.108, 0.099) + Vec3::splat(soft * 0.009);
            let bib = smooth(-0.24, -0.33, p.z)
                * (1.0 - smooth(0.025, 0.065, p.x.abs()))
                * (1.0 - smooth(0.49, 0.58, p.y));
            blend(c, Vec3::new(0.51, 0.46, 0.38), bib * 0.9)
        }
        DogCoat::Grey => {
            let dorsal = smooth(0.35, 0.56, p.y);
            blend(
                Vec3::new(0.47, 0.46, 0.43),
                Vec3::new(0.265, 0.285, 0.29),
                dorsal * 0.8,
            )
        }
        DogCoat::Fawn => {
            let c = blend(
                Vec3::new(0.61, 0.44, 0.255),
                Vec3::new(0.73, 0.59, 0.40),
                lower * 0.35 + underside * 0.5,
            );
            let mask = smooth(-0.43, -0.57, p.z) * smooth(0.48, 0.58, p.y);
            blend(c, Vec3::new(0.23, 0.205, 0.18), mask * 0.9)
        }
        DogCoat::White => {
            let dirt = (1.0 - smooth(0.01, 0.16, p.y)) * 0.24;
            blend(
                Vec3::new(0.80, 0.775, 0.69),
                Vec3::new(0.46, 0.40, 0.31),
                dirt,
            )
        }
        DogCoat::Pied => {
            let patch_a = ((p.z + 0.025) / 0.23).powi(2)
                + ((p.y - 0.51) / 0.18).powi(2)
                + ((p.x - 0.07) / 0.30).powi(2);
            let patch_b = ((p.z - 0.29) / 0.14).powi(2)
                + ((p.y - 0.48) / 0.20).powi(2)
                + ((p.x + 0.09) / 0.21).powi(2);
            let head = smooth(-0.38, -0.47, p.z)
                * smooth(0.52, 0.59, p.y)
                * smooth(0.012, 0.035, p.x.abs());
            let mask = (1.0 - smooth(0.82, 1.11, patch_a + soft * 0.15))
                .max(1.0 - smooth(0.80, 1.10, patch_b + soft * 0.13))
                .max(head);
            blend(
                Vec3::new(0.78, 0.745, 0.65),
                Vec3::new(0.19, 0.17, 0.14),
                mask,
            )
        }
    };
    (color + Vec3::splat(grain * 0.008 + soft * 0.010)).clamp(Vec3::splat(0.02), Vec3::splat(0.92))
}

fn body_mesh(kind: DogCoat, width: f32, length: f32) -> Mesh {
    let mut s = Surface::default();
    // The chest turns upward into the neck in the same continuous surface.
    let points = [
        S::new(0.0, 0.190, -0.470, 0.022, 0.025),
        S::new(0.0, 0.165, -0.435, 0.054, 0.064),
        S::new(0.0, 0.105, -0.390, 0.071, 0.112),
        S::new(0.0, 0.040, -0.340, 0.084, 0.135),
        S::new(0.0, 0.009, -0.265, 0.106, 0.139),
        S::new(0.0, -0.005, -0.155, 0.119, 0.138),
        S::new(0.0, 0.008, -0.025, 0.113, 0.120),
        S::new(0.0, 0.037, 0.095, 0.091, 0.097),
        S::new(0.0, 0.040, 0.185, 0.088, 0.096),
        S::new(0.0, 0.021, 0.270, 0.108, 0.109),
        S::new(0.0, 0.010, 0.330, 0.083, 0.082),
        S::new(0.0, 0.004, 0.365, 0.025, 0.035),
    ]
    .map(|mut p| {
        p.center.z *= length;
        p.radii.x *= width;
        p
    });
    s.tube(&points, Vec3::X, Vec3::Y, 20, 2, |p| {
        coat(kind, p + Vec3::Y * 0.40)
    });
    skin(s.finish(), |p| smooth(-0.20, -0.40, p.z / length))
}

fn head_mesh(kind: DogCoat) -> Mesh {
    let mut s = Surface::default();
    let paint = |p| coat(kind, p + Vec3::new(0.0, 0.47, -0.27));
    // Continuous nose bridge, forehead stop, cheek and occiput. The muzzle
    // leaves the skull horizontally, with a separate lower-jaw contour.
    s.tube(
        &[
            S::new(0.0, 0.131, -0.317, 0.034, 0.025),
            S::new(0.0, 0.134, -0.285, 0.047, 0.034),
            S::new(0.0, 0.141, -0.228, 0.050, 0.040),
            S::new(0.0, 0.155, -0.198, 0.060, 0.054),
            S::new(0.0, 0.167, -0.175, 0.066, 0.064),
            S::new(0.0, 0.169, -0.148, 0.074, 0.071),
            S::new(0.0, 0.171, -0.11, 0.075, 0.072),
            S::new(0.0, 0.165, -0.049, 0.062, 0.066),
            S::new(0.0, 0.147, 0.006, 0.026, 0.034),
        ],
        Vec3::X,
        Vec3::Y,
        20,
        2,
        |p| {
            let inner_roof = (1.0 - smooth(0.109, 0.120, p.y)) * smooth(-0.15, -0.19, p.z);
            blend(paint(p), Vec3::new(0.050, 0.029, 0.026), inner_roof)
        },
    );
    s.deform(|p| {
        let side = p.x.signum();
        let gaussian = |center: Vec3, radii: Vec3| {
            let q = (Vec3::new(p.x.abs(), p.y, p.z) - center) / radii;
            (-q.length_squared() * 2.0).exp()
        };
        let socket = gaussian(
            Vec3::new(0.052, 0.187, -0.193),
            Vec3::new(0.035, 0.024, 0.035),
        );
        let brow = gaussian(
            Vec3::new(0.049, 0.208, -0.183),
            Vec3::new(0.032, 0.014, 0.032),
        );
        let cheek = gaussian(
            Vec3::new(0.066, 0.143, -0.154),
            Vec3::new(0.027, 0.035, 0.046),
        );
        p + Vec3::new(
            side * (cheek * 0.006 + brow * 0.004 - socket * 0.008),
            brow * 0.004 - socket * 0.002,
            socket * 0.005,
        )
    });
    for sign in [-1.0, 1.0] {
        orbital_ring(
            &mut s,
            sign,
            &[
                (0.024, 0.017, -0.008),
                (0.019, 0.0115, -0.0025),
                (0.015, 0.0085, -0.0010),
            ],
            |p, ring| paint(p) * [1.0, 0.86, 0.64][ring],
        );
    }
    let jaw_start = s.positions.len();
    s.tube(
        &[
            S::new(0.0, 0.110, -0.306, 0.028, 0.007),
            S::new(0.0, 0.107, -0.255, 0.040, 0.012),
            S::new(0.0, 0.110, -0.192, 0.046, 0.017),
            S::new(0.0, 0.12, -0.143, 0.051, 0.026),
        ],
        Vec3::X,
        Vec3::Y,
        12,
        1,
        |p| {
            let interior = smooth(0.108, 0.115, p.y) * smooth(-0.16, -0.20, p.z);
            let tongue = (1.0 - smooth(0.010, 0.024, p.x.abs())) * smooth(-0.31, -0.27, p.z);
            let mouth = blend(
                Vec3::new(0.047, 0.028, 0.025),
                Vec3::new(0.30, 0.15, 0.14),
                tongue,
            );
            blend(paint(p), mouth, interior)
        },
    );
    let mut mesh = s.finish();
    let weights: Vec<_> = (0..mesh.count_vertices())
        .map(|index| {
            if index < jaw_start {
                [1.0, 0.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0, 0.0]
            }
        })
        .collect();
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(vec![[0, 1, 0, 0]; mesh.count_vertices()]),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    mesh.with_generated_skinned_mesh_bounds().unwrap()
}

pub(super) fn face_mesh() -> Mesh {
    let mut s = Surface::default();
    let dark = |_: Vec3| Vec3::new(0.043, 0.035, 0.03);
    s.ellipsoid(
        Vec3::new(0.0, 0.137, -0.325),
        Vec3::new(0.036, 0.019, 0.014),
        Quat::IDENTITY,
        12,
        6,
        |p| Vec3::splat(0.042 + smooth(0.125, 0.155, p.y) * 0.022),
    );
    for sign in [-1.0, 1.0] {
        s.ellipsoid(
            Vec3::new(sign * 0.021, 0.139, -0.338),
            Vec3::new(0.010, 0.005, 0.0025),
            Quat::from_rotation_z(sign * 0.25),
            8,
            4,
            |_| Vec3::splat(0.011),
        );
        s.tube(
            &[
                S::new(sign * 0.026, 0.108, -0.306, 0.002, 0.0015),
                S::new(sign * 0.044, 0.106, -0.275, 0.002, 0.0017),
                S::new(sign * 0.050, 0.106, -0.23, 0.0019, 0.0018),
                S::new(sign * 0.056, 0.11, -0.19, 0.001, 0.001),
            ],
            Vec3::X,
            Vec3::Y,
            6,
            1,
            dark,
        );
        orbital_ring(
            &mut s,
            sign,
            &[(0.0155, 0.009, -0.001), (0.013, 0.0068, -0.0005)],
            |_, _| Vec3::new(0.075, 0.054, 0.043),
        );
    }
    s.finish()
}

pub(super) fn eye_mesh() -> Mesh {
    let mut s = Surface::default();
    for sign in [-1.0, 1.0] {
        let (center, _, _, normal) = eye_frame(sign);
        let rotation = Quat::from_rotation_arc(Vec3::Z, normal);
        orbital_ring(
            &mut s,
            sign,
            &[
                (0.013, 0.0068, -0.0008),
                (0.007, 0.0055, 0.0004),
                (0.001, 0.001, 0.0012),
            ],
            |_, ring| {
                [
                    Vec3::new(0.075, 0.063, 0.048),
                    Vec3::new(0.20, 0.125, 0.052),
                    Vec3::new(0.12, 0.079, 0.038),
                ][ring]
            },
        );
        s.ellipsoid(
            center + normal * 0.0011,
            Vec3::new(0.0048, 0.0048, 0.0007),
            rotation,
            8,
            4,
            |_| Vec3::new(0.014, 0.012, 0.010),
        );
    }
    s.finish()
}

fn ear_mesh(kind: DogCoat, sign: f32, upright: bool) -> Mesh {
    let mut s = Surface::default();
    let points = if upright {
        vec![
            S::new(0.0, -0.01, 0.0, 0.026, 0.014),
            S::new(sign * 0.008, 0.032, 0.004, 0.033, 0.011),
            S::new(sign * 0.025, 0.077, 0.013, 0.020, 0.007),
            S::new(sign * 0.031, 0.11, 0.025, 0.002, 0.002),
        ]
    } else {
        vec![
            S::new(0.0, 0.012, 0.0, 0.020, 0.018),
            S::new(sign * 0.020, -0.004, -0.002, 0.033, 0.013),
            S::new(sign * 0.031, -0.041, -0.014, 0.029, 0.009),
            S::new(sign * 0.034, -0.080, -0.022, 0.021, 0.007),
            S::new(sign * 0.031, -0.098, -0.022, 0.006, 0.003),
        ]
    };
    s.tube(&points, Vec3::X, Vec3::Z, 12, 2, |p| {
        let base = coat(kind, p + Vec3::new(sign * 0.061, 0.70, -0.32));
        let inner = if upright {
            (1.0 - smooth(-0.003, 0.005, p.z))
                * smooth(0.008, 0.04, p.y)
                * (1.0 - smooth(0.008, 0.03, (p.x - sign * 0.014).abs()))
        } else {
            0.0
        };
        blend(base * 0.78, Vec3::new(0.27, 0.195, 0.155), inner * 0.75)
    });
    s.finish()
}

fn upper_mesh(kind: DogCoat, rear: bool, sign: f32, mass: f32, width: f32, length: f32) -> Mesh {
    let mut s = Surface::default();
    let mut points = if rear {
        vec![
            S::new(-0.073 * width, 0.067, 0.006, 0.012 * mass, 0.033),
            S::new(-0.023 * width, 0.023, 0.006, 0.043 * mass, 0.067),
            S::new(-0.008, -0.035, -0.006, 0.050 * mass, 0.073),
            S::new(0.0, -0.098, -0.029, 0.044 * mass, 0.056),
            S::new(0.0, -0.157, -0.064, 0.033 * mass, 0.039),
            // The stifle turns back into the tibia. Its closing ring sits
            // inside that lower-leg envelope, so no flat thigh cap projects
            // forward of the bent joint in a side or rear view.
            S::new(0.0, -0.192, -0.043, 0.023 * mass, 0.027),
        ]
    } else {
        vec![
            S::new(-0.065 * width, 0.085, 0.032, 0.012 * mass, 0.032),
            S::new(-0.028 * width, 0.043, 0.025, 0.026 * mass, 0.057),
            S::new(0.0, 0.002, 0.014, 0.036 * mass, 0.062),
            S::new(0.0, -0.065, 0.024, 0.038 * mass, 0.049),
            S::new(0.0, -0.145, 0.026, 0.034 * mass, 0.040),
            S::new(0.0, -0.20, 0.020, 0.028 * mass, 0.032),
        ]
    };
    for point in &mut points {
        point.center.x *= sign;
    }
    let origin = Vec3::new(
        sign * (if rear { 0.09 } else { 0.088 }) * width,
        if rear { 0.455 } else { 0.445 },
        if rear {
            0.255 * length
        } else {
            -0.235 * length
        },
    );
    s.tube(&points, Vec3::X, Vec3::Z, 12, 2, |p| coat(kind, p + origin));
    // The muscle over the shoulder/hip follows the barrel; the distal leg
    // follows its joint. Folding a leg must not pull its top cap out of the
    // dog's back or leave an exposed ball at a moving shoulder.
    let mut mesh = skin(s.finish(), |p| smooth(0.04, -0.10, p.y));
    if rear {
        let VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            unreachable!()
        };
        let weights: Vec<_> = positions
            .iter()
            .map(|p| {
                let upper = smooth(0.04, -0.10, p[1]);
                let knee = smooth(-0.12, -0.192, p[1]);
                [1.0 - upper, upper * (1.0 - knee), upper * knee, 0.0]
            })
            .collect();
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(vec![[0, 1, 2, 0]; mesh.count_vertices()]),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
        mesh.generate_skinned_mesh_bounds().unwrap();
    }
    mesh
}

fn lower_mesh(kind: DogCoat, rear: bool, mass: f32, width: f32, length: f32) -> Mesh {
    let mut s = Surface::default();
    let origin = Vec3::new(
        (if rear { 0.09 } else { 0.088 }) * width,
        if rear { 0.285 } else { 0.255 },
        if rear {
            0.255 * length - 0.07
        } else {
            -0.235 * length + 0.02
        },
    );
    let points = if rear {
        vec![
            S::new(0.0, 0.025, 0.008, 0.029 * mass, 0.034),
            S::new(0.0, -0.055, 0.065, 0.029 * mass, 0.034),
            S::new(0.0, -0.132, 0.130, 0.024 * mass, 0.028),
            S::new(0.0, -0.174, 0.149, 0.025 * mass, 0.028),
            S::new(0.0, -0.228, 0.135, 0.023 * mass, 0.027),
            S::new(0.0, -0.255, 0.118, 0.025 * mass, 0.032),
        ]
    } else {
        vec![
            S::new(0.0, 0.015, 0.0, 0.028 * mass, 0.032),
            S::new(0.0, -0.07, 0.002, 0.025 * mass, 0.028),
            S::new(0.0, -0.16, -0.001, 0.022 * mass, 0.024),
            S::new(0.0, -0.197, -0.007, 0.024 * mass, 0.025),
            S::new(0.0, -0.225, -0.021, 0.026 * mass, 0.034),
        ]
    };
    let paint = |p| coat(kind, p + origin);
    s.tube(&points, Vec3::X, Vec3::Z, 10, 2, paint);
    let paw_z = if rear { 0.106 } else { -0.025 };
    // Crosswise sections make four toe knuckles in one planted envelope.
    // The gaps shorten/lower the toe edge; they are actual silhouette, not
    // painted lines or spheres attached to the end of a leg.
    let toe_sections: Vec<S> = [
        -1.0_f32, -0.84, -0.65, -0.45, -0.22, 0.0, 0.22, 0.45, 0.65, 0.84, 1.0,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, x)| {
        let edge = x.abs();
        let groove = matches!(i, 3 | 5 | 7);
        let front = paw_z - if groove { 0.044 } else { 0.051 - edge * 0.008 };
        let rear = paw_z + 0.026;
        let height = if groove { 0.040 } else { 0.049 - edge * 0.007 };
        if i == 0 || i == 10 {
            S::new(
                x * 0.036 * mass,
                -origin.y + 0.016,
                paw_z - 0.008,
                0.006,
                0.008,
            )
        } else {
            S::new(
                x * 0.036 * mass,
                -origin.y + height * 0.5 + 0.001,
                (front + rear) * 0.5,
                (rear - front) * 0.5,
                height * 0.5,
            )
        }
    })
    .collect();
    let mut paw = Surface::default();
    paw.tube(&toe_sections, Vec3::Z, Vec3::Y, 12, 1, |p| {
        let tip = 1.0 - smooth(paw_z - 0.044, paw_z - 0.030, p.z);
        let normalized_x = p.x / mass;
        let groove = [-0.0162_f32, 0.0, 0.0162]
            .into_iter()
            .map(|x| (-((normalized_x - x) / 0.0028).powi(2)).exp())
            .fold(0.0_f32, f32::max);
        paint(p) * (1.0 - groove * tip * 0.33)
    });
    paw.deform(|p| {
        let height = p.y + origin.y;
        // Broad, level contact under the digital pad, with a curved top.
        let planted = if height <= 0.008 {
            0.001
        } else {
            0.001 + (height - 0.008) * (0.049 / 0.042)
        };
        Vec3::new(p.x, -origin.y + planted, p.z)
    });
    let offset = s.positions.len() as u32;
    s.positions.extend(paw.positions);
    s.normals.extend(paw.normals);
    s.colors.extend(paw.colors);
    s.uvs.extend(paw.uvs);
    s.indices
        .extend(paw.indices.into_iter().map(|i| i + offset));
    let paw_start = offset as usize;
    let mut mesh = s.finish();
    let VertexAttributeValues::Float32x3(positions) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
    else {
        unreachable!()
    };
    let weights: Vec<_> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if rear {
                if i >= paw_start {
                    return [0.0, 0.0, 1.0, 0.0];
                }
                let hock = smooth(-0.14, -0.19, p[1]);
                let paw = smooth(-0.22, -0.255, p[1]);
                return [1.0 - hock, hock * (1.0 - paw), hock * paw, 0.0];
            }
            let w = if i >= paw_start {
                1.0
            } else {
                smooth(
                    if rear { -0.18 } else { -0.16 },
                    if rear { -0.255 } else { -0.225 },
                    p[1],
                )
            };
            [1.0 - w, w, 0.0, 0.0]
        })
        .collect();
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(vec![
            if rear { [0, 1, 2, 0] } else { [0, 1, 0, 0] };
            mesh.count_vertices()
        ]),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    mesh.with_generated_skinned_mesh_bounds().unwrap()
}

fn tail_mesh(kind: DogCoat) -> Mesh {
    let mut s = Surface::default();
    // Neutral hanging arc, carried back off the croup. Animation rotates
    // the root; the rest geometry already has the pliant change of direction.
    s.tube(
        &[
            S::new(0.0, 0.0, 0.0, 0.026, 0.025),
            S::new(0.0, -0.038, 0.085, 0.024, 0.023),
            S::new(0.005, -0.096, 0.164, 0.019, 0.019),
            S::new(0.012, -0.145, 0.226, 0.014, 0.014),
            S::new(0.019, -0.158, 0.275, 0.010, 0.010),
            S::new(0.026, -0.136, 0.312, 0.002, 0.003),
        ],
        Vec3::X,
        Vec3::Y,
        10,
        2,
        |p| coat(kind, p + Vec3::new(0.0, 0.46, 0.34)),
    );
    s.finish()
}

pub(super) fn build(
    kind: DogCoat,
    meshes: &mut Assets<Mesh>,
    features: Handle<Mesh>,
    eyes: Handle<Mesh>,
) -> Form {
    let (width, length, mass, head_scale) = match kind {
        DogCoat::Fawn => (1.28, 0.97, 1.24, Vec3::new(1.25, 1.06, 0.85)),
        DogCoat::Black => (1.10, 1.0, 1.08, Vec3::new(1.06, 1.0, 1.0)),
        DogCoat::Grey => (0.91, 1.07, 0.90, Vec3::new(0.94, 1.0, 1.06)),
        DogCoat::White => (0.93, 0.96, 0.95, Vec3::new(1.0, 1.0, 0.92)),
        DogCoat::Pied => (1.02, 1.02, 1.0, Vec3::new(1.05, 0.98, 0.95)),
        DogCoat::Brindle => (1.0, 1.0, 1.0, Vec3::ONE),
    };
    Form {
        body: meshes.add(body_mesh(kind, width, length)),
        head: meshes.add(head_mesh(kind)),
        features,
        eyes,
        ears: [-1.0, 1.0].map(|sign| {
            meshes.add(ear_mesh(
                kind,
                sign,
                matches!(kind, DogCoat::White) || matches!(kind, DogCoat::Brindle) && sign > 0.0,
            ))
        }),
        front_upper: [1.0, -1.0]
            .map(|sign| meshes.add(upper_mesh(kind, false, sign, mass, width, length))),
        rear_upper: [1.0, -1.0]
            .map(|sign| meshes.add(upper_mesh(kind, true, sign, mass, width, length))),
        front_lower: meshes.add(lower_mesh(kind, false, mass, width, length)),
        rear_lower: meshes.add(lower_mesh(kind, true, mass, width, length)),
        tail: meshes.add(tail_mesh(kind)),
        width,
        length,
        head_scale,
        neck_bind: Handle::default(),
        upper_bind: std::array::from_fn(|_| Handle::default()),
    }
}

fn skin(mut mesh: Mesh, weight: impl Fn(Vec3) -> f32) -> Mesh {
    let VertexAttributeValues::Float32x3(positions) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
    else {
        unreachable!()
    };
    let weights: Vec<_> = positions
        .iter()
        .map(|p| {
            let w = weight(Vec3::from_array(*p));
            [1.0 - w, w, 0.0, 0.0]
        })
        .collect();
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(vec![[0, 1, 0, 0]; mesh.count_vertices()]),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    mesh.with_generated_skinned_mesh_bounds().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn all_coats_have_finite_geometry_bounded_cost_and_planted_paws() {
        let mut meshes = Assets::<Mesh>::default();
        let face = meshes.add(face_mesh());
        let eyes = meshes.add(eye_mesh());
        for kind in [
            DogCoat::Brindle,
            DogCoat::Black,
            DogCoat::Grey,
            DogCoat::Fawn,
            DogCoat::White,
            DogCoat::Pied,
        ] {
            let form = build(kind, &mut meshes, face.clone(), eyes.clone());
            let handles = [
                &form.body,
                &form.head,
                &form.features,
                &form.eyes,
                &form.ears[0],
                &form.ears[1],
                &form.front_upper[0],
                &form.front_upper[1],
                &form.rear_upper[0],
                &form.rear_upper[1],
                &form.front_lower,
                &form.front_lower,
                &form.rear_lower,
                &form.rear_lower,
                &form.tail,
            ];
            let mut vertices = 0;
            let mut triangles = 0;
            for handle in handles {
                let mesh = meshes.get(handle).unwrap();
                vertices += mesh.count_vertices();
                triangles += mesh.indices().unwrap().len() / 3;
                for attribute in [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL] {
                    let Some(VertexAttributeValues::Float32x3(values)) = mesh.attribute(attribute)
                    else {
                        panic!("missing position or normal");
                    };
                    assert!(values.iter().all(|v| v.iter().all(|x| x.is_finite())));
                }
            }
            println!("{kind:?}: {vertices} vertices, {triangles} triangles, 15 mesh entities");
            assert!(triangles <= 6_000);
            for (handle, joint_y) in [(&form.front_lower, 0.255), (&form.rear_lower, 0.285)] {
                let Some(VertexAttributeValues::Float32x3(values)) = meshes
                    .get(handle)
                    .unwrap()
                    .attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    unreachable!()
                };
                let sole = values
                    .iter()
                    .map(|p| p[1] + joint_y)
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    (0.0..=0.002).contains(&sole),
                    "paw sole {sole} misses the ground"
                );
                let contact: Vec<_> = values.iter().filter(|p| p[1] + joint_y <= 0.002).collect();
                let span = |axis: usize| {
                    contact
                        .iter()
                        .map(|p| p[axis])
                        .fold(f32::NEG_INFINITY, f32::max)
                        - contact
                            .iter()
                            .map(|p| p[axis])
                            .fold(f32::INFINITY, f32::min)
                };
                assert!(
                    contact.len() >= 6 && span(0) >= 0.040 && span(2) >= 0.024,
                    "paw needs a broad contact pad, not one low vertex"
                );
            }
        }
        assert_eq!(meshes.len(), 68, "shared geometry count changed");
    }
}
