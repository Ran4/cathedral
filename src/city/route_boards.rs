//! Physical road-supply maps at the four places the route artwork names.
//!
//! All boards share one full-resolution texture and one material.  They are
//! mounted a few centimetres proud of existing solid façades, so they add no
//! navigation obstacle and cannot drift into a baked walkable lane.

use bevy::{
    camera::visibility::VisibilityRange,
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    light::NotShadowCaster,
    prelude::*,
};

pub(super) const TEXTURE_PATH: &str = "textures/road_supply_routes.png";
#[cfg(test)]
const TEXTURE_WIDTH_PX: u32 = 1448;
#[cfg(test)]
const TEXTURE_HEIGHT_PX: u32 = 1086;

const BOARD_WIDTH_M: f32 = 5.6;
const BOARD_HEIGHT_M: f32 = 4.2;
const BOARD_CENTER_Y_M: f32 = 3.25;
const BACKING_DEPTH_M: f32 = 0.14;
const FRAME_WIDTH_M: f32 = 0.18;
const FRAME_DEPTH_M: f32 = 0.20;
const MAP_FACE_OFFSET_M: f32 = BACKING_DEPTH_M * 0.5 + 0.012;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RoadSupplyRouteBoard {
    pub location: &'static str,
}

#[derive(Component, Debug)]
pub(super) struct RoadSupplyRouteMapFace;

#[derive(Debug, Clone, Copy)]
struct BoardPlacement {
    location: &'static str,
    /// World-space centre of the timber backing.
    position: [f32; 3],
    /// Front normal in world XZ. `Rectangle` faces local +Z.
    front_xz: [f32; 2],
}

/// Each board sits on a door-free wall edge and faces the public side:
///
/// - Wool Gate: inner face of the northern tower;
/// - Stone Gate: inner face of the eastern tower;
/// - Seven Lofts: the courtyard face of bay 4; and
/// - Draper's Reach: the street face of cloth hall 4.
const PLACEMENTS: [BoardPlacement; 4] = [
    BoardPlacement {
        location: "The Wool Gate",
        position: [-53.0, BOARD_CENTER_Y_M, 497.40],
        front_xz: [0.0, -1.0],
    },
    BoardPlacement {
        location: "The Stone Gate",
        position: [482.90, BOARD_CENTER_Y_M, 158.0],
        front_xz: [-1.0, 0.0],
    },
    BoardPlacement {
        location: "Seven Lofts",
        position: [341.02, BOARD_CENTER_Y_M, 336.17],
        front_xz: [1.0, 0.0],
    },
    BoardPlacement {
        location: "The Draper's Reach",
        position: [132.15, BOARD_CENTER_Y_M, 256.77],
        front_xz: [-0.743, 0.669],
    },
];

pub(super) fn spawn_route_boards(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cube: &Handle<Mesh>,
    dark_wood: &Handle<StandardMaterial>,
) {
    let map_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let map_texture = load_map_texture(asset_server);
    let map_material = materials.add(map_material(map_texture));

    for placement in PLACEMENTS {
        let front = Vec2::from_array(placement.front_xz).normalize();
        let yaw = front.x.atan2(front.y);
        commands
            .spawn((
                Name::new(format!("Road supply route board: {}", placement.location)),
                RoadSupplyRouteBoard {
                    location: placement.location,
                },
                Transform::from_translation(Vec3::from_array(placement.position))
                    .with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
            ))
            .with_children(|board| {
                board.spawn((
                    Name::new("Road supply map timber backing"),
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(dark_wood.clone()),
                    Transform::from_scale(Vec3::new(
                        BOARD_WIDTH_M + FRAME_WIDTH_M * 2.0,
                        BOARD_HEIGHT_M + FRAME_WIDTH_M * 2.0,
                        BACKING_DEPTH_M,
                    )),
                    board_fade(),
                ));
                board.spawn((
                    Name::new("Road supply map artwork"),
                    RoadSupplyRouteMapFace,
                    Mesh3d(map_mesh.clone()),
                    MeshMaterial3d(map_material.clone()),
                    Transform::from_xyz(0.0, 0.0, MAP_FACE_OFFSET_M).with_scale(Vec3::new(
                        BOARD_WIDTH_M,
                        BOARD_HEIGHT_M,
                        1.0,
                    )),
                    board_fade(),
                    NotShadowCaster,
                ));

                for y in [
                    -(BOARD_HEIGHT_M + FRAME_WIDTH_M) * 0.5,
                    (BOARD_HEIGHT_M + FRAME_WIDTH_M) * 0.5,
                ] {
                    board.spawn((
                        Name::new("Road supply map horizontal frame"),
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(dark_wood.clone()),
                        Transform::from_xyz(0.0, y, MAP_FACE_OFFSET_M + 0.025).with_scale(
                            Vec3::new(
                                BOARD_WIDTH_M + FRAME_WIDTH_M * 2.0,
                                FRAME_WIDTH_M,
                                FRAME_DEPTH_M,
                            ),
                        ),
                        board_fade(),
                    ));
                }
                for x in [
                    -(BOARD_WIDTH_M + FRAME_WIDTH_M) * 0.5,
                    (BOARD_WIDTH_M + FRAME_WIDTH_M) * 0.5,
                ] {
                    board.spawn((
                        Name::new("Road supply map vertical frame"),
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(dark_wood.clone()),
                        Transform::from_xyz(x, 0.0, MAP_FACE_OFFSET_M + 0.025)
                            .with_scale(Vec3::new(FRAME_WIDTH_M, BOARD_HEIGHT_M, FRAME_DEPTH_M)),
                        board_fade(),
                    ));
                }
            });
    }
}

/// Smooth minification keeps the ink legible while the board recedes.  All
/// four materials refer to this one handle; there is no duplicate low-res
/// texture allocation.
fn load_map_texture(asset_server: &AssetServer) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            let mut sampler = ImageSamplerDescriptor::linear();
            sampler
                .set_address_mode(ImageAddressMode::ClampToEdge)
                .set_anisotropic_filter(16);
            settings.sampler = ImageSampler::Descriptor(sampler);
        })
        .load(TEXTURE_PATH)
}

/// Unlit presentation preserves the authored orange/blue ink in shadowed gate
/// tunnels instead of multiplying it by the surrounding stone's light level.
fn map_material(texture: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(texture),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        unlit: true,
        ..default()
    }
}

/// Past this range the lettering is below useful screen size.  Fading the
/// whole prop avoids a bright unlit postage stamp on the distant skyline.
fn board_fade() -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: 80.0..100.0,
        use_aabb: true,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const PNG: &[u8] = include_bytes!("../../assets/textures/road_supply_routes.png");

    #[test]
    fn reviewed_texture_is_full_resolution_and_matches_the_board_aspect() {
        assert_eq!(&PNG[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes(PNG[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(PNG[20..24].try_into().unwrap());
        assert_eq!((width, height), (TEXTURE_WIDTH_PX, TEXTURE_HEIGHT_PX));
        assert!(PNG.len() > 2_000_000, "the reviewed artwork was downscaled");
        assert_eq!(width * 3, height * 4);
        assert!((BOARD_WIDTH_M / BOARD_HEIGHT_M - width as f32 / height as f32).abs() < 1e-6);
    }

    #[test]
    fn exactly_the_four_annotated_places_receive_a_board() {
        let locations = PLACEMENTS
            .iter()
            .map(|placement| placement.location)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            locations,
            BTreeSet::from([
                "Seven Lofts",
                "The Draper's Reach",
                "The Stone Gate",
                "The Wool Gate",
            ])
        );
        for placement in PLACEMENTS {
            let front = Vec2::from_array(placement.front_xz);
            assert!((front.length() - 1.0).abs() < 0.001);
            assert_eq!(placement.position[1], BOARD_CENTER_Y_M);
        }
    }

    #[test]
    fn artwork_material_preserves_the_reviewed_colors() {
        let texture = Handle::<Image>::default();
        let material = map_material(texture.clone());
        assert_eq!(material.base_color_texture, Some(texture));
        assert_eq!(material.base_color, Color::WHITE);
        assert!(material.unlit);
        assert_eq!(material.reflectance, 0.0);
    }
}
