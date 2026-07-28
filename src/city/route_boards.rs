//! Physical road-supply maps at the four places the route artwork names.
//!
//! All boards share one full-resolution texture and one material.  They are
//! mounted a few centimetres proud of existing solid façades, so they add no
//! navigation obstacle and cannot drift into a baked walkable lane.  Nothing in
//! the build enforces that, though — a board carries no collider and is never
//! checked against the ground — so every placement is derived from the
//! cadastral plan and proved against it in the tests below.

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
/// Air between the backing's rear face and the masonry it is bolted to, so the
/// board reads as a hung panel rather than a decal and never z-fights the wall.
#[cfg(test)]
const BOARD_STANDOFF_M: f32 = 0.03;
/// A placement's `position` therefore sits this far out from the plane of the
/// façade edge it is seated on — the number every placement below is derived
/// with, and the one `every_board_is_flush_on_a_real_facade` measures back.
#[cfg(test)]
const BOARD_SEAT_OFFSET_M: f32 = BACKING_DEPTH_M * 0.5 + BOARD_STANDOFF_M;
/// Half the timber backing, frame included: how far the board reaches sideways
/// along the façade from `position`.
#[cfg(test)]
const BOARD_HALF_SPAN_M: f32 = (BOARD_WIDTH_M + FRAME_WIDTH_M * 2.0) * 0.5;

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

/// Each board is centred on a door-free edge of a cadastral building in
/// `lore/places/ombreval_buildings.json`, standing [`BOARD_SEAT_OFFSET_M`] out
/// from that edge's plane along its outward normal, so the backing hangs
/// [`BOARD_STANDOFF_M`] proud of real masonry and the map faces the public side:
///
/// - Wool Gate: `gate_wool_1` edge 3, the northern tower's passage wall, under
///   the gatehouse — the west approach road starts at (-24.5, 357) between it
///   and its twin, and the passage is 14 m of clear ground to read from;
/// - Stone Gate: `gate_stone_1` edge 3, the eastern tower's city-facing wall
///   (*not* the passage wall — the working leaves fold flat against that one
///   whenever the gate stands open, `gates.rs`);
/// - Seven Lofts: `named_seven_lofts_4` edge 1, the courtyard end of bay 4,
///   looking back across the yard the seven bays enclose; and
/// - Draper's Reach: `named_cloth_hall_4` edge 0, the street front, facing the
///   Reach's own 5 m passage 10 m away over the cloth halls' open frontage.
///
/// The city was shrunk 0.7× in 2026-07 and these are *not* the old numbers
/// scaled: each was re-derived from the current plan polygon it names, because
/// the shrink moved buildings by class, not by one factor (`features/AGENTS.md`).
/// `every_board_is_flush_on_a_real_facade` proves all four against the
/// committed plan, collision export and baked walkable surface.
const PLACEMENTS: [BoardPlacement; 4] = [
    BoardPlacement {
        location: "The Wool Gate",
        // gate_wool_1 spans x -17.5..4.5, z 344.5..369.5; edge 3 is x = -17.5.
        position: [-17.60, BOARD_CENTER_Y_M, 357.00],
        front_xz: [-1.0, 0.0],
    },
    BoardPlacement {
        location: "The Stone Gate",
        // gate_stone_1 spans x 334.5..358.5, z 60.5..82.5; edge 3 is x = 334.5.
        position: [334.40, BOARD_CENTER_Y_M, 71.50],
        front_xz: [-1.0, 0.0],
    },
    BoardPlacement {
        location: "Seven Lofts",
        // Bay 4's east end runs (232.136, 226.704) -> (233.704, 244.635); the
        // bays sit on a 5-degree skew, so the board does too.
        position: [233.0197, BOARD_CENTER_Y_M, 235.661],
        front_xz: [0.996195, -0.087156],
    },
    BoardPlacement {
        location: "The Draper's Reach",
        // Cloth hall 4's street front runs (109.871, 223.757) -> (131.423,
        // 204.352): the Reach's 42-degree diagonal through the Cloth Ward.
        position: [120.58, BOARD_CENTER_Y_M, 213.9805],
        front_xz: [-0.669131, -0.743145],
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
    const AREAS_JSON: &str = include_str!("../../assets/world/areas.json");
    const COLLISION_JSON: &str = include_str!("../../assets/world/collision_footprints.json");
    const NAV_JSON: &str = include_str!("../../assets/world/navigation.json");
    const NAV_BIN: &[u8] = include_bytes!("../../assets/world/navigation.bin");

    /// The cadastral building and polygon edge each board is bolted to, named
    /// rather than searched for, so a placement cannot quietly migrate onto
    /// whatever wall happens to be nearest after a plan edit.
    const HOSTS: [(&str, &str, usize); 4] = [
        ("The Wool Gate", "gate_wool_1", 3),
        ("The Stone Gate", "gate_stone_1", 3),
        ("Seven Lofts", "named_seven_lofts_4", 1),
        ("The Draper's Reach", "named_cloth_hall_4", 0),
    ];

    #[derive(serde::Deserialize)]
    struct CollisionExport {
        footprints: Vec<Vec<[f32; 2]>>,
    }

    /// Does anything stop the standing player here?  `collision_footprints.json`
    /// is the exact XZ of every collider crossing the walk band, so this is the
    /// same surface `no_walkable_cell_is_solid` measures the nav bake against.
    fn is_solid(point: Vec2, collision: &CollisionExport) -> bool {
        collision
            .footprints
            .iter()
            .any(|footprint| super::super::point_in_polygon(point, footprint))
    }

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

    /// Where the boards actually hang.  A board carries no collider and the
    /// build never tests one against the ground, so nothing stops a placement
    /// being authored in mid-air or inside somebody's house — which is what
    /// happened when the city was shrunk 0.7× and these constants were left at
    /// their pre-shrink values.  Measure all four against the committed world
    /// instead: on the ground the city stands on, seated flush on the cadastral
    /// façade they name, tall wall behind them, and clear ground in front to
    /// read them from.
    #[test]
    fn every_board_is_flush_on_a_real_facade() {
        let plan = super::super::plan::load();
        let collision: CollisionExport =
            serde_json::from_str(COLLISION_JSON).expect("the committed collision export loads");
        let areas =
            cathedral_sim::AreaMap::from_json_str(AREAS_JSON).expect("the committed areas load");
        let nav = cathedral_sim::NavData::from_parts(NAV_JSON, NAV_BIN)
            .expect("the committed navigation artifact loads");
        let half_height = (BOARD_HEIGHT_M + FRAME_WIDTH_M * 2.0) * 0.5;

        for placement in PLACEMENTS {
            let place = placement.location;
            let position = Vec2::new(placement.position[0], placement.position[2]);
            let front = Vec2::from_array(placement.front_xz).normalize();
            let along = Vec2::new(-front.y, front.x);

            // Inside the world at all — the ground quad the city is laid on.
            assert!(
                (super::super::GROUND_MIN_X..=super::super::GROUND_MAX_X).contains(&position.x)
                    && (super::super::GROUND_MIN_Z..=super::super::GROUND_MAX_Z)
                        .contains(&position.y),
                "{place}'s board hangs at {position} — off the end of the ground mesh"
            );

            // Seated on the façade it names, facing the way that wall faces.
            let (host, edge) = HOSTS
                .iter()
                .find_map(|(named, host, edge)| (*named == place).then_some((*host, *edge)))
                .unwrap_or_else(|| panic!("{place} has no host façade in HOSTS"));
            let building = plan
                .buildings
                .iter()
                .find(|building| building.id == host)
                .unwrap_or_else(|| panic!("{place}'s host '{host}' is not in the cadastral plan"));
            let corners = &building.polygon;
            let start = Vec2::from_array(corners[edge]);
            let end = Vec2::from_array(corners[(edge + 1) % corners.len()]);
            let run = end - start;
            let centre = corners
                .iter()
                .fold(Vec2::ZERO, |sum, corner| sum + Vec2::from_array(*corner))
                / corners.len() as f32;
            let outward = {
                let normal = Vec2::new(run.y, -run.x).normalize();
                if ((start + end) * 0.5 - centre).dot(normal) < 0.0 {
                    -normal
                } else {
                    normal
                }
            };
            assert!(
                front.distance(outward) < 1.0e-3,
                "{place}'s board faces {front}, but {host} edge {edge} faces {outward}"
            );

            let seated = (position - start).dot(outward);
            assert!(
                (seated - BOARD_SEAT_OFFSET_M).abs() < 1.0e-3,
                "{place}'s board stands {seated} m off {host} edge {edge}, \
                 not the {BOARD_SEAT_OFFSET_M} m that hangs its backing \
                 {BOARD_STANDOFF_M} m proud"
            );
            let slid = (position - start).dot(run.normalize());
            assert!(
                slid >= BOARD_HALF_SPAN_M - 1.0e-3
                    && slid <= run.length() - BOARD_HALF_SPAN_M + 1.0e-3,
                "{place}'s board runs past the end of {host} edge {edge}"
            );
            let (base_y, eave_y) = super::super::building_verticals(building);
            assert!(
                base_y <= BOARD_CENTER_Y_M - half_height && BOARD_CENTER_Y_M + half_height < eave_y,
                "{place}'s board reaches above {host}'s wall ({base_y}..{eave_y} m)"
            );

            // Masonry the whole width behind it, open street the whole width in
            // front: proud of the wall, not sunk into it and not floating.
            for step in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
                let on_wall = position + along * (BOARD_HALF_SPAN_M * step);
                let behind = on_wall - front * (BACKING_DEPTH_M * 0.5 + BOARD_STANDOFF_M * 2.0);
                assert!(
                    is_solid(behind, &collision),
                    "{place}'s board has open air behind it at {behind}"
                );
                for reach in [0.25_f32, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0] {
                    let ahead = on_wall + front * reach;
                    assert!(
                        !is_solid(ahead, &collision),
                        "{place}'s board is buried in a solid {reach} m out at {ahead}"
                    );
                }
            }

            // At the place it is a map of.
            let area = areas
                .areas
                .iter()
                .find(|area| area.label == place)
                .unwrap_or_else(|| panic!("'{place}' is not an area in areas.json"));
            let distance = area
                .boxes
                .iter()
                .map(|bounds| {
                    let dx = ((bounds.min_m.x as f32) - position.x)
                        .max(position.x - bounds.max_m.x as f32)
                        .max(0.0);
                    let dz = ((bounds.min_m.z as f32) - position.y)
                        .max(position.y - bounds.max_m.z as f32)
                        .max(0.0);
                    Vec2::new(dx, dz).length()
                })
                .fold(f32::INFINITY, f32::min);
            assert!(
                distance <= 20.0,
                "{place}'s board is {distance} m from the area it names"
            );

            // The baked NPC surface is clipped to the curtain, so a board
            // outside the walls — the Wool Gate's court, between the towers and
            // the wall line — has no cells at all to stand on.  Inside, the
            // reader's ground has to be part of that surface too.
            if super::super::point_in_polygon(position, &plan.wall_polygon_xz) {
                let reader = position + front * 2.0;
                assert!(
                    nav.is_walkable(reader.x as f64, reader.y as f64),
                    "{place}'s board faces ground no one can stand on at {reader}"
                );
            }
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
