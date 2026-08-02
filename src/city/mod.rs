//! The cadastral city of Ombreval.
//!
//! `lore/places/ombreval_buildings.json` is the same plan that produces the
//! zoomable bird's-eye SVG.  The game consumes those exact parcels, routes,
//! sites, fixtures, storey counts, materials, and stable IDs instead of
//! inventing a second procedural grid here.

mod gates;
mod marks;
pub use marks::MarkFocus;
mod monuments;
mod plan;
mod route_boards;
mod smoke;
mod surfaces;
mod trade_props;
mod vermin;
pub mod water;

pub(crate) use surfaces::CobbleRoadNetwork;

use std::{
    collections::{BTreeMap, HashMap},
    f32::consts::{PI, SQRT_2},
};

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::VisibilityRange,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::ExtendedMaterial,
    prelude::*,
};

use crate::{
    controller::CollisionWorld,
    materials::{
        FLOOR_TEXTURE_SPAN_METERS, WindowGlassExtension, WindowGlassMaterial,
        load_repeating_texture,
    },
    weather::{WeatherReactiveMaterials, WetResponse},
};

use monuments::build_approach_monuments;
use plan::{Building, CityPlan, Fixture, Road, Site};

const GROUND_MIN_X: f32 = -497.0;
const GROUND_MAX_X: f32 = 385.0;
const GROUND_MIN_Z: f32 = -521.5;
const GROUND_MAX_Z: f32 = 455.0;
const WALL_HEIGHT: f32 = 14.0;
const WALL_THICKNESS: f32 = 3.2;
const BUILDING_FLOOR_HEIGHT: f32 = 3.15;

pub struct CityPlugin;

impl Plugin for CityPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WindowGlassMaterial>::default())
            .add_message::<crate::soundscape::SoundscapeCue>()
            .init_resource::<gates::GateRuntime>()
            .init_resource::<marks::MarkFocus>()
            .init_resource::<marks::ChalkHold>()
            .init_resource::<marks::MarkCatalogRes>()
            .init_resource::<trade_props::TradePropRuntime>()
            // The vermin waypoints validate against the collision world the
            // city build just populated.
            .add_systems(
                Startup,
                (build_city, vermin::spawn_vermin, marks::spawn_marks).chain(),
            )
            .add_systems(
                Update,
                (
                    smoke::animate_chimney_smoke,
                    // The chalk is slow state: this rebuilds the batch only
                    // when the sim's world revision has moved, so an unchalked
                    // city costs one integer comparison a frame.
                    (
                        marks::sync_marks,
                        marks::update_mark_focus,
                        marks::chalk_hold,
                    )
                        .chain(),
                    // The boil is announced before it is drawn, so the log line
                    // and the percept precede the frame that triples the batch.
                    (
                        vermin::announce_vermin_boil,
                        vermin::trigger_vermin_scatter,
                        vermin::animate_vermin,
                    )
                        .chain(),
                    gates::animate_gate_mechanisms
                        .in_set(crate::soundscape::SoundscapeSet::EmitCues),
                    water::animate_well_mechanisms
                        .after(crate::soundscape::SoundscapeSet::ProjectActivity),
                    (
                        trade_props::handle_trade_cues
                            .in_set(crate::soundscape::SoundscapeSet::IngestCues),
                        trade_props::animate_trade_props,
                    )
                        .chain(),
                ),
            );
    }
}

#[derive(Resource, Debug, Clone, Copy)]
#[allow(dead_code)] // Runtime inventory evidence, read directly by regression tests and inspectors.
struct CityBuildStats {
    planned_buildings: usize,
    rendered_plan_buildings: usize,
    named_places: usize,
    roads: usize,
    sites: usize,
    fixtures: usize,
    wharf_sheds: usize,
}

#[derive(Component, Debug)]
#[allow(dead_code)] // Stable lore metadata for debug picking and regression coverage.
struct LorePlaceNumber(u8);

#[derive(Clone)]
struct CityMeshes {
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    sphere: Handle<Mesh>,
    pyramid: Handle<Mesh>,
    /// A cylinder with its middle taken out: the hollow mouth of a well.
    curb_ring: Handle<Mesh>,
}

#[derive(Clone)]
struct CityMaterials {
    ground: Handle<StandardMaterial>,
    cobbles: Handle<StandardMaterial>,
    paving: Handle<StandardMaterial>,
    dry_cut: Handle<StandardMaterial>,
    /// The Cut's bank outside the kerb line: the same flags the squares are
    /// laid in, weathered greyer and duller so the pale cart-worn dust of
    /// `dry_cut` reads as a lane running between two darker bands.
    cut_margin: Handle<StandardMaterial>,
    yard: Handle<StandardMaterial>,
    limestone: Handle<StandardMaterial>,
    fieldstone: Handle<StandardMaterial>,
    plaster: Handle<StandardMaterial>,
    half_timber: Handle<StandardMaterial>,
    terracotta: Handle<StandardMaterial>,
    slate: Handle<StandardMaterial>,
    thatch: Handle<StandardMaterial>,
    timber: Handle<StandardMaterial>,
    dark_wood: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    bronze: Handle<StandardMaterial>,
    window: Handle<WindowGlassMaterial>,
    /// What a near-clear pane shows: the dark room shell behind the glass.
    window_room: Handle<StandardMaterial>,
    /// The warm pane of a hanging lantern: lit from within, always.
    lantern_glass: Handle<StandardMaterial>,
    cloth_ochre: Handle<StandardMaterial>,
    cloth_russet: Handle<StandardMaterial>,
    /// Washing on the lines: woven linen artwork, tinted per piece by vertex brush.
    linen: Handle<StandardMaterial>,
    /// Awning cloth: coarse patched hemp, dyed per sheet by vertex brush.
    canvas: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
    /// The wet lining you see when you lean over a curb.
    well_shaft: Handle<StandardMaterial>,
    /// Water at the bottom of a shaft or behind a draw hatch: the same stuff as
    /// `water`, read in the dark.
    well_water: Handle<StandardMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum WallKind {
    Limestone,
    Fieldstone,
    Plaster,
    HalfTimber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RoofKind {
    Terracotta,
    Slate,
    Thatch,
}

struct MeshData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
    /// Multiplied into every vertex written while it is set — the per-building
    /// tint jitter and grime bands ride this brush into the batched mesh.
    brush: [f32; 4],
}

impl Default for MeshData {
    fn default() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            brush: [1.0; 4],
        }
    }
}

impl MeshData {
    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Every vertex written until the next call is multiplied by this colour.
    /// One value per building breaks the 2,500-clones monotony for free; the
    /// per-vertex grime gradient in `add_extruded_walls` stacks on top of it.
    fn set_brush(&mut self, color: [f32; 3]) {
        self.brush = [color[0], color[1], color[2], 1.0];
    }

    fn reset_brush(&mut self) {
        self.brush = [1.0; 4];
    }

    fn vertex(&mut self, position: Vec3, normal: Vec3, uv: Vec2) -> u32 {
        self.vertex_shaded(position, normal, uv, 1.0)
    }

    /// `shade` darkens toward 0.0 on top of the brush — the grime dial.
    fn vertex_shaded(&mut self, position: Vec3, normal: Vec3, uv: Vec2, shade: f32) -> u32 {
        let index = self.positions.len() as u32;
        self.positions.push(position.to_array());
        self.normals.push(normal.normalize_or(Vec3::Y).to_array());
        self.uvs.push(uv.to_array());
        self.colors.push([
            self.brush[0] * shade,
            self.brush[1] * shade,
            self.brush[2] * shade,
            self.brush[3],
        ]);
        index
    }

    fn quad(&mut self, points: [Vec3; 4], normal: Vec3, uvs: [Vec2; 4]) {
        let first = self.positions.len() as u32;
        for (point, uv) in points.into_iter().zip(uvs) {
            self.vertex(point, normal, uv);
        }
        self.indices
            .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    fn triangle(&mut self, mut points: [Vec3; 3], uvs: [Vec2; 3], upward: bool) {
        let mut normal = (points[1] - points[0]).cross(points[2] - points[0]);
        let mut uvs = uvs;
        if upward && normal.y < 0.0 {
            points.swap(1, 2);
            uvs.swap(1, 2);
            normal = -normal;
        }
        let first = self.positions.len() as u32;
        for (point, uv) in points.into_iter().zip(uvs) {
            self.vertex(point, normal, uv);
        }
        self.indices
            .extend_from_slice(&[first, first + 1, first + 2]);
    }

    fn into_mesh(self) -> Mesh {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_indices(Indices::U32(self.indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        if self.colors.iter().any(|color| *color != [1.0; 4]) {
            mesh.with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        } else {
            mesh
        }
    }
}

fn build_city(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut glass_materials: ResMut<Assets<WindowGlassMaterial>>,
    mut collision_world: ResMut<CollisionWorld>,
) {
    let plan = plan::load();
    commands.insert_resource(CobbleRoadNetwork::from_roads(&plan.roads));
    let doors = door_edges();
    let city_meshes = create_meshes(&mut meshes);
    let city_materials = create_materials(&asset_server, &mut materials, &mut glass_materials);
    commands.insert_resource(WeatherReactiveMaterials::capture(
        &materials,
        [
            (city_materials.ground.clone(), WetResponse::GROUND),
            (city_materials.cobbles.clone(), WetResponse::GROUND),
            (city_materials.paving.clone(), WetResponse::PAVING),
            (city_materials.dry_cut.clone(), WetResponse::GROUND),
            (city_materials.cut_margin.clone(), WetResponse::PAVING),
            (city_materials.yard.clone(), WetResponse::GROUND),
            (city_materials.terracotta.clone(), WetResponse::ROOF),
            (city_materials.slate.clone(), WetResponse::ROOF),
            (city_materials.thatch.clone(), WetResponse::TIMBER),
            (city_materials.timber.clone(), WetResponse::TIMBER),
            (city_materials.canvas.clone(), WetResponse::CANVAS),
        ],
    ));

    build_ground_context(
        &mut commands,
        &mut meshes,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_sites_and_roads(
        &mut commands,
        &mut meshes,
        &city_materials,
        &plan.sites,
        &plan.roads,
    );
    let (rendered_plan_buildings, chimney_anchors) = build_buildings(
        &mut commands,
        &mut meshes,
        &city_materials,
        &plan,
        &doors,
        &mut collision_world,
    );
    let smoking = smoke::build_chimney_smoke(
        &mut commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &chimney_anchors,
    );
    info!(
        "lit {smoking} of {} chimneys with drifting smoke",
        chimney_anchors.len()
    );
    build_fixtures(
        &mut commands,
        &city_meshes,
        &city_materials,
        &plan.fixtures,
        &mut collision_world,
    );
    build_named_details(
        &mut commands,
        &city_meshes,
        &mut meshes,
        &city_materials,
        &plan,
        &mut collision_world,
    );
    build_street_galleries(
        &mut commands,
        &city_meshes,
        &city_materials,
        &plan,
        &mut collision_world,
    );
    build_covered_passages(&mut commands, &mut meshes, &city_materials, &plan);
    build_bellfoot_passage(
        &mut commands,
        &mut meshes,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_bridge_arches(&mut commands, &mut meshes, &city_materials, &plan);
    build_square_arcades(&mut commands, &mut meshes, &city_materials, &plan, &doors);
    build_yard_stairs(
        &mut commands,
        &mut meshes,
        &city_materials,
        &plan,
        &doors,
        &mut collision_world,
    );
    build_open_balconies(
        &mut commands,
        &mut meshes,
        &city_materials,
        &plan,
        &doors,
        &mut collision_world,
    );
    build_street_props(&mut commands, &mut meshes, &city_materials, &plan, &doors);
    build_shopfront_awnings(&mut commands, &mut meshes, &city_materials, &plan, &doors);
    build_laundry_lines(&mut commands, &mut meshes, &city_materials, &plan);
    build_hoist_gantries(&mut commands, &mut meshes, &city_materials, &plan, &doors);
    build_fortifications(
        &mut commands,
        &city_meshes,
        &city_materials,
        &plan,
        &mut collision_world,
    );
    route_boards::spawn_route_boards(
        &mut commands,
        &asset_server,
        &mut meshes,
        &mut materials,
        &city_meshes.cube,
        &city_materials.dark_wood,
    );
    build_approach_monuments(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut collision_world,
    );
    spawn_place_markers(&mut commands, &plan);

    commands.insert_resource(CityBuildStats {
        planned_buildings: plan.buildings.len(),
        rendered_plan_buildings,
        named_places: plan.named_place_index.len(),
        roads: plan.roads.len(),
        sites: plan.sites.len(),
        fixtures: plan.fixtures.len(),
        wharf_sheds: 11,
    });
}

fn create_meshes(meshes: &mut Assets<Mesh>) -> CityMeshes {
    CityMeshes {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16).build()),
        sphere: meshes.add(Sphere::new(1.0).mesh().uv(16, 10)),
        pyramid: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(4).build()),
        curb_ring: meshes.add(water::curb_ring_mesh()),
    }
}

fn create_materials(
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    glass_materials: &mut Assets<WindowGlassMaterial>,
) -> CityMaterials {
    let textured = |materials: &mut Assets<StandardMaterial>,
                    path: &'static str,
                    tint: Color,
                    roughness: f32| {
        materials.add(StandardMaterial {
            base_color: tint,
            base_color_texture: Some(load_repeating_texture(asset_server, path)),
            perceptual_roughness: roughness,
            reflectance: 0.28,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    };

    CityMaterials {
        ground: textured(
            materials,
            "textures/ombreval_yard_ground.png",
            Color::srgb(0.62, 0.60, 0.54),
            0.97,
        ),
        cobbles: textured(
            materials,
            "textures/ombreval_cobbles.png",
            Color::srgb(0.72, 0.70, 0.66),
            0.9,
        ),
        paving: textured(
            materials,
            "textures/ombreval_paving.png",
            Color::srgb(0.78, 0.75, 0.69),
            0.86,
        ),
        dry_cut: textured(
            materials,
            "textures/ombreval_dry_cut.png",
            Color::srgb(0.72, 0.68, 0.59),
            0.98,
        ),
        cut_margin: textured(
            materials,
            "textures/ombreval_paving.png",
            Color::srgb(0.60, 0.57, 0.50),
            0.95,
        ),
        yard: textured(
            materials,
            "textures/ombreval_yard_ground.png",
            Color::srgb(0.62, 0.58, 0.50),
            0.97,
        ),
        limestone: textured(
            materials,
            "textures/ombreval_limestone.png",
            Color::srgb(0.90, 0.87, 0.80),
            0.82,
        ),
        fieldstone: textured(
            materials,
            "textures/ombreval_fieldstone.png",
            Color::srgb(0.72, 0.69, 0.65),
            0.91,
        ),
        plaster: textured(
            materials,
            "textures/ombreval_plaster.png",
            Color::srgb(0.86, 0.81, 0.70),
            0.9,
        ),
        half_timber: textured(
            materials,
            "textures/ombreval_half_timber.png",
            Color::srgb(0.82, 0.78, 0.70),
            0.87,
        ),
        terracotta: textured(
            materials,
            "textures/ombreval_terracotta.png",
            Color::srgb(0.78, 0.70, 0.62),
            0.88,
        ),
        slate: textured(
            materials,
            "textures/ombreval_slate.png",
            Color::srgb(0.72, 0.75, 0.78),
            0.8,
        ),
        thatch: textured(
            materials,
            "textures/ombreval_thatch.png",
            Color::srgb(0.72, 0.67, 0.55),
            0.96,
        ),
        timber: textured(
            materials,
            "textures/ombreval_timber.png",
            Color::srgb(0.72, 0.65, 0.56),
            0.88,
        ),
        dark_wood: materials.add(StandardMaterial {
            base_color: Color::srgb(0.075, 0.045, 0.028),
            perceptual_roughness: 0.86,
            // Door leaves and shutters are authored as single wall-plane
            // panels; without this, half of them face the wrong way and
            // vanish, leaving pale see-through doorways.
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        iron: matte(materials, Color::srgb(0.055, 0.06, 0.06), 0.68),
        bronze: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.12, 0.075),
            metallic: 0.8,
            perceptual_roughness: 0.46,
            ..default()
        }),
        window: glass_materials.add(ExtendedMaterial {
            base: StandardMaterial {
                // A cool quarrel tint with enough reflectance that the
                // atmosphere environment map lands on the panes as a sky sheen
                // instead of leaving black holes; still rough enough not to
                // become a flat sky-mirror. Alpha rides the distance fade in
                // `window_glass.wgsl`: see-through up close, opaque at range —
                // past the fade nothing behind a pane is ever on screen.
                base_color: Color::srgb(0.08, 0.10, 0.115),
                emissive: LinearRgba::rgb(0.018, 0.021, 0.024),
                perceptual_roughness: 0.28,
                reflectance: 0.62,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            },
            // Clearest within 12 m at alpha 0.06, fully opaque again past
            // 22 m. Point-blank the pane must nearly vanish: at 0.3 — and
            // still at 0.12 — the residual sky sheen veiled the room behind
            // it and near windows kept reading as blank dark glass.
            extension: WindowGlassExtension {
                fade: Vec4::new(12.0, 22.0, 0.06, 0.0),
            },
        }),
        window_room: materials.add(StandardMaterial {
            // Rooms behind the panes are UNLIT: the atmosphere environment
            // light has no occlusion, so a lit interior surface facing the
            // horizon renders as bright as the open street — scene lighting
            // cannot keep a shell interior dark. The chamber look is baked
            // into the vertex colors instead (`add_window_room`). Bright
            // enough to read instantly from the street in any daylight —
            // tuned darker twice and the rooms vanished into the panes.
            base_color: Color::srgb(0.36, 0.29, 0.21),
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        lantern_glass: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.55),
            emissive: LinearRgba::rgb(5.5, 2.8, 0.9),
            perceptual_roughness: 0.3,
            ..default()
        }),
        cloth_ochre: materials.add(StandardMaterial {
            base_color: Color::srgb(0.33, 0.24, 0.12),
            perceptual_roughness: 0.92,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        cloth_russet: materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.10, 0.07),
            perceptual_roughness: 0.92,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        linen: materials.add(StandardMaterial {
            base_color: Color::srgb(0.93, 0.92, 0.88),
            base_color_texture: Some(load_repeating_texture(
                asset_server,
                "textures/ombreval_linen.png",
            )),
            perceptual_roughness: 0.94,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        canvas: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.90, 0.86),
            base_color_texture: Some(load_repeating_texture(
                asset_server,
                "textures/ombreval_canvas.png",
            )),
            perceptual_roughness: 0.96,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        water: materials.add(StandardMaterial {
            base_color: Color::srgba(0.12, 0.27, 0.30, 0.94),
            metallic: 0.05,
            perceptual_roughness: 0.2,
            reflectance: 0.62,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        well_shaft: materials.add(StandardMaterial {
            base_color: Color::srgb(0.11, 0.11, 0.10),
            perceptual_roughness: 0.95,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        // A shaft is a hole with a roof over it, so nothing down there catches
        // the sun. The faint emissive is what a real well surface gets for free
        // and this one cannot: the sky, bounced back up at whoever leans in.
        well_water: materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.19, 0.21),
            emissive: LinearRgba::rgb(0.02, 0.05, 0.06),
            metallic: 0.15,
            perceptual_roughness: 0.08,
            reflectance: 0.85,
            ..default()
        }),
    }
}

fn matte(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
    roughness: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: roughness,
        ..default()
    })
}

fn build_ground_context(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    primitives: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let mut ground = MeshData::default();
    add_surface_quad(
        &mut ground,
        GROUND_MIN_X,
        GROUND_MAX_X,
        GROUND_MIN_Z,
        GROUND_MAX_Z,
        -0.035,
        9.0,
    );
    spawn_batch(
        commands,
        meshes,
        &materials.ground,
        ground,
        "Ombreval ground",
    );
    collision_world.add_box(
        Vec3::new(GROUND_MIN_X, -1.2, GROUND_MIN_Z),
        Vec3::new(GROUND_MAX_X, 0.0, GROUND_MAX_Z),
    );

    // The Serle remains wholly beyond the south wall, exactly as on the map.
    let mut river = MeshData::default();
    add_surface_quad(&mut river, -483.0, -402.5, -514.5, 448.0, 0.025, 18.0);
    spawn_batch(commands, meshes, &materials.water, river, "The Serle");

    // The SVG includes eleven individual wharf sheds and quay reaches outside
    // the machine-readable urban-building inventory.  They are nevertheless
    // authored map buildings and therefore belong in the 3D context.
    for index in 0_usize..11 {
        let z = 66.5 - index as f32 * 38.0;
        let center = Vec3::new(-397.6, 3.0, z);
        spawn_box_named(
            commands,
            primitives,
            if index.is_multiple_of(2) {
                &materials.fieldstone
            } else {
                &materials.timber
            },
            center,
            Vec3::new(24.0, 6.0, 27.0),
            format!("Outer wharf shed {:02}", index + 1),
        );
        spawn_mesh_named(
            commands,
            &primitives.pyramid,
            &materials.terracotta,
            Transform::from_xyz(-397.6, 7.3, z).with_scale(Vec3::new(18.5, 3.0, 20.5)),
            format!("Outer wharf shed {:02} roof", index + 1),
        );
        collision_world.add_box(
            Vec3::new(-409.6, 0.0, z - 13.5),
            Vec3::new(-385.6, 8.5, z + 13.5),
        );

        spawn_box_named(
            commands,
            primitives,
            &materials.timber,
            Vec3::new(-419.6, 0.18, z),
            Vec3::new(16.0, 0.35, 27.0),
            format!("Outer wharf quay {:02}", index + 1),
        );
        for post_z in [z - 12.0, z + 12.0] {
            spawn_cylinder(
                commands,
                primitives,
                &materials.dark_wood,
                Vec3::new(-427.6, 1.5, post_z),
                0.35,
                3.0,
            );
        }
    }
}

fn build_sites_and_roads(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    sites: &[Site],
    roads: &[Road],
) {
    let mut paved_sites = MeshData::default();
    let mut yard_sites = MeshData::default();

    for site in sites {
        let center = polygon_center(&site.polygon);
        commands.spawn((
            Name::new(format!("{} site [{}]", site.name, site.id)),
            Transform::from_xyz(center.x, 0.012, center.y),
            Visibility::default(),
        ));
        // The cathedral has its own non-overlapping floor and apron meshes.
        if site.id == "lanthorn_precinct" {
            continue;
        }
        let target = match site.kind.as_str() {
            "square" | "monument" | "precinct" => &mut paved_sites,
            _ => &mut yard_sites,
        };
        add_polygon_surface(target, &site.polygon, 0.012, FLOOR_TEXTURE_SPAN_METERS);
    }
    spawn_batch(
        commands,
        meshes,
        &materials.paving,
        paved_sites,
        "Named paved places",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.yard,
        yard_sites,
        "Named working grounds",
    );

    let mut cobbles = MeshData::default();
    let mut cut = MeshData::default();
    for road in roads {
        let marker = road.points[road.points.len() / 2];
        let prefix = if road.label { "Named route" } else { "Street" };
        commands.spawn((
            Name::new(format!("{prefix}: {} [{}]", road.name, road.id)),
            Transform::from_xyz(marker[0], 0.024, marker[1]),
            Visibility::default(),
        ));
        let target = if road.tier == "cut" {
            &mut cut
        } else {
            &mut cobbles
        };
        add_road_ribbon(target, road, 0.024);
    }
    spawn_batch(
        commands,
        meshes,
        &materials.cobbles,
        cobbles,
        "The forty-eight streets of Ombreval",
    );
    spawn_batch(commands, meshes, &materials.dry_cut, cut, "The dry Cut");
}

/// The building → door-edge map baked into `navigation.json`. The renderer draws
/// each door on the same polygon edge the sim walks to, so the visible door and
/// the nav door are the same door; a building with no reachable edge has no door.
fn door_edges() -> HashMap<String, usize> {
    cathedral_sim::door_edges_from_json(include_str!("../../assets/world/navigation.json"))
        .expect("the committed navigation.json parses")
}

fn build_buildings(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
    collision_world: &mut CollisionWorld,
) -> (usize, Vec<smoke::ChimneyAnchor>) {
    let mut walls = BTreeMap::<WallKind, MeshData>::new();
    let mut roofs = BTreeMap::<RoofKind, MeshData>::new();
    let mut windows = MeshData::default();
    let mut rooms = MeshData::default();
    let mut doors = MeshData::default();
    let mut frames = MeshData::default();
    let mut timber_frames = MeshData::default();
    let mut chimney_anchors = Vec::new();
    let mut rendered = 0;
    // The Cut margin's ground, for the doorways that open onto its raised
    // flags (`add_door_module`). Built again here rather than shared with
    // `build_kerb`'s resource because the two run in no fixed order and the
    // construction is a one-off pass over the plan.
    let cut_profile = cut_margin_profile(plan);

    for building in &plan.buildings {
        // The fixed Lanthorn shell is built by `scene.rs`, with the authored
        // interior, openings, towers, dome, and real collision.  Its cadastral
        // polygon is still validated and counted, but must not be filled solid.
        if building.id == "named_lanthorn" {
            continue;
        }

        let (base_y, eave_y) = building_verticals(building);
        let dominant_wall = wall_kind(&building.material);
        let tint = building_tint(building);
        let openings = plan_facade_openings(
            building,
            door_edges.get(&building.id).copied(),
            base_y,
            eave_y,
        );
        let bands = jetty_bands(building, base_y, eave_y);
        let roof_polygon: Vec<[f32; 2]> = match &bands {
            Some(bands) => {
                add_jettied_walls(
                    &mut walls,
                    &mut frames,
                    &mut timber_frames,
                    stable_hash(&building.id),
                    bands,
                    tint,
                    base_y,
                    &openings,
                );
                bands
                    .last()
                    .expect("jetty_bands never returns an empty stack")
                    .polygon
                    .clone()
            }
            None => {
                add_building_walls(
                    &mut walls,
                    &mut timber_frames,
                    building,
                    dominant_wall,
                    base_y,
                    eave_y,
                    tint,
                    &openings,
                );
                building.polygon.clone()
            }
        };

        // The Bellstand tower ends in an authored open belfry, not a gable.
        let roof_height = if building.id == "named_bellstand_tower" {
            0.0
        } else {
            let roof_kind = roof_kind(building);
            // Half-timber gables sit over plaster now, like their storeys.
            let gable_kind = if dominant_wall == WallKind::HalfTimber {
                WallKind::Plaster
            } else {
                dominant_wall
            };
            let roof_mesh = roofs.entry(roof_kind).or_default();
            roof_mesh.set_brush(tint);
            let gable_mesh = walls.entry(gable_kind).or_default();
            gable_mesh.set_brush(tint);
            let (roof_height, ridge) =
                add_building_roof(roof_mesh, gable_mesh, &roof_polygon, eave_y);
            roofs.entry(roof_kind).or_default().reset_brush();
            walls.entry(gable_kind).or_default().reset_brush();
            if let Some(ridge) = ridge {
                add_chimneys(
                    walls.entry(WallKind::Fieldstone).or_default(),
                    building,
                    ridge,
                    &mut chimney_anchors,
                );
            }
            roof_height
        };
        match &bands {
            Some(bands) => {
                for band in bands {
                    let scoped = band_openings(&openings, band);
                    add_facade_openings_on(
                        &mut windows,
                        &mut rooms,
                        &mut doors,
                        &mut frames,
                        &band.polygon,
                        &scoped,
                        &cut_profile,
                    );
                }
            }
            None => add_facade_openings_on(
                &mut windows,
                &mut rooms,
                &mut doors,
                &mut frames,
                &building.polygon,
                &openings,
                &cut_profile,
            ),
        }
        add_footprint_colliders(
            collision_world,
            &building.polygon,
            base_y,
            eave_y + roof_height,
        );
        rendered += 1;

        if building.named {
            let center = polygon_center(&building.polygon);
            commands.spawn((
                Name::new(format!(
                    "{} [{}]",
                    building
                        .name
                        .as_deref()
                        .expect("validated named building must have a name"),
                    building.id
                )),
                Transform::from_xyz(center.x, base_y, center.y),
                Visibility::default(),
            ));
        }
    }

    for (kind, mesh) in walls {
        let (material, name) = match kind {
            WallKind::Limestone => (&materials.limestone, "Limestone buildings"),
            WallKind::Fieldstone => (&materials.fieldstone, "Fieldstone buildings"),
            WallKind::Plaster => (&materials.plaster, "Plastered buildings"),
            WallKind::HalfTimber => (&materials.half_timber, "Half-timbered buildings"),
        };
        spawn_batch(commands, meshes, material, mesh, name);
    }
    for (kind, mesh) in roofs {
        let (material, name) = match kind {
            RoofKind::Terracotta => (&materials.terracotta, "Clay-tiled roofs"),
            RoofKind::Slate => (&materials.slate, "Slate roofs"),
            RoofKind::Thatch => (&materials.thatch, "Thatch roofs"),
        };
        spawn_batch(commands, meshes, material, mesh, name);
    }
    spawn_batch(
        commands,
        meshes,
        &materials.window,
        windows,
        "Ombreval windows",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.window_room,
        rooms,
        "Ombreval window rooms",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        doors,
        "Ombreval doors and shutters",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        frames,
        "Ombreval reveals, sills and lintels",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        timber_frames,
        "Ombreval timber framing",
    );

    (rendered, chimney_anchors)
}

/// A stable per-building colour multiplier: small value and warm/cool swings
/// that keep 1,100 same-material façades from rendering as one wall.
fn building_tint(building: &Building) -> [f32; 3] {
    let hash = stable_hash(&building.id);
    let value = 0.86 + (hash % 61) as f32 / 60.0 * 0.20;
    let warmth = 0.965 + ((hash >> 8) % 41) as f32 / 40.0 * 0.07;
    [value * warmth, value, value / warmth]
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OpeningKind {
    Window { shutters: bool },
    Door,
}

/// One rectangular hole in one façade edge, in edge-local coordinates:
/// `along` metres from the edge's first vertex, `center_y` in world height.
#[derive(Debug, Clone, Copy)]
struct FacadeOpening {
    along: f32,
    center_y: f32,
    width: f32,
    height: f32,
    hash: u32,
    kind: OpeningKind,
}

impl FacadeOpening {
    fn min_y(&self) -> f32 {
        self.center_y - self.height * 0.5
    }

    fn max_y(&self) -> f32 {
        self.center_y + self.height * 0.5
    }
}

/// Decide every opening of a building up front, per polygon edge, so the wall
/// builder can punch the holes the modules then line. Openings are the reason
/// the walls can no longer be four blind quads.
fn plan_facade_openings(
    building: &Building,
    door_edge: Option<usize>,
    base_y: f32,
    eave_y: f32,
) -> Vec<Vec<FacadeOpening>> {
    let mut all = vec![Vec::new(); building.polygon.len()];
    if building.use_name == "bridge" || building.id == "named_malt_house" {
        return all;
    }
    let building_hash = stable_hash(&building.id);
    let shutters_allowed = matches!(
        wall_kind(&building.material),
        WallKind::Plaster | WallKind::HalfTimber
    );
    for (edge_index, (a, b)) in building
        .polygon
        .iter()
        .zip(building.polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let length = Vec2::from_array(*a).distance(Vec2::from_array(*b));
        if length < 3.2 {
            continue;
        }
        let openings = &mut all[edge_index];
        let door_here = door_edge == Some(edge_index);
        if door_here {
            openings.push(FacadeOpening {
                along: length * 0.5,
                center_y: base_y + 1.25,
                width: 1.35,
                height: 2.5,
                hash: building_hash,
                kind: OpeningKind::Door,
            });
        }

        let count = ((length - 1.0) / 4.2).floor().clamp(1.0, 4.0) as usize;
        let floors = ((eave_y - base_y) / BUILDING_FLOOR_HEIGHT)
            .floor()
            .clamp(1.0, 4.0) as usize;
        for floor in 0..floors {
            let y = base_y + 2.05 + floor as f32 * BUILDING_FLOOR_HEIGHT;
            if y + 0.75 >= eave_y {
                continue;
            }
            for index in 0..count {
                let opening_hash = building_hash
                    ^ (edge_index as u32).wrapping_mul(0x9E37_79B9)
                    ^ (floor as u32).wrapping_mul(0x85EB_CA6B)
                    ^ (index as u32).wrapping_mul(0xC2B2_AE35);
                // A skipped window here and there keeps the grid from reading
                // as a punch card; the jitter keeps storeys off the plumb line.
                if opening_hash % 9 == 0 {
                    continue;
                }
                let jitter = ((opening_hash >> 4) % 61) as f32 / 60.0 - 0.5;
                let along = (length * (index as f32 + 1.0) / (count as f32 + 1.0) + jitter * 0.7)
                    .clamp(0.9, length - 0.9);
                // Medieval ground floors are wall, not glass: smaller, higher
                // openings on the street level, generous casements above.
                let (width, height) = if floor == 0 {
                    (0.78, 1.02)
                } else {
                    (1.0, 1.35)
                };
                // Nothing may overlap the doorway.
                if door_here
                    && floor == 0
                    && (along - length * 0.5).abs() < (1.35 + width) * 0.5 + 0.3
                {
                    continue;
                }
                openings.push(FacadeOpening {
                    along,
                    center_y: y,
                    width,
                    height,
                    hash: opening_hash,
                    kind: OpeningKind::Window {
                        shutters: shutters_allowed && floor <= 1 && opening_hash % 100 < 42,
                    },
                });
            }
        }
    }
    all
}

/// One storey band of a (possibly jettied) building: its own footprint and the
/// per-edge shift that maps original-edge `along` coordinates onto it.
struct StoreyBand {
    polygon: Vec<[f32; 2]>,
    bottom: f32,
    top: f32,
    /// Outward offset from the cadastral footprint (0 on the ground floor).
    offset: f32,
    /// Per edge: how far this band's edge start slid backward, i.e. what to add
    /// to an original-polygon `along` to land on the same wall point here.
    start_extensions: Vec<f32>,
}

/// Cantilever per jetty step. Two steps on a three-storey house add up to
/// two-thirds of a metre of street closing in overhead.
const JETTY_STEP: f32 = 0.34;

/// Offset a convex polygon outward by `distance`, mitring the corners. Returns
/// the new ring and, per edge, how far its start vertex slid backward along
/// the edge direction (needed to keep openings on the same wall point).
fn offset_convex_polygon(polygon: &[[f32; 2]], distance: f32) -> Option<(Vec<[f32; 2]>, Vec<f32>)> {
    let n = polygon.len();
    let orientation = plan::signed_area(polygon).signum();
    let mut ring = Vec::with_capacity(n);
    let mut extensions = vec![0.0_f32; n];
    for i in 0..n {
        let prev = Vec2::from_array(polygon[(i + n - 1) % n]);
        let here = Vec2::from_array(polygon[i]);
        let next = Vec2::from_array(polygon[(i + 1) % n]);
        let dir_in = (here - prev).normalize_or_zero();
        let dir_out = (next - here).normalize_or_zero();
        let normal_in = Vec2::new(dir_in.y, -dir_in.x) * orientation;
        let normal_out = Vec2::new(dir_out.y, -dir_out.x) * orientation;
        let miter = (normal_in + normal_out).normalize_or_zero();
        let denominator = miter.dot(normal_out);
        if denominator < 0.4 {
            // Sharper than ~130° of turn: the miter would spike; skip jetties
            // on this footprint rather than render a blade.
            return None;
        }
        let miter_length = distance / denominator;
        ring.push((here + miter * miter_length).to_array());
        // The offset corner moves against the outgoing edge direction by the
        // projection of the miter onto it.
        extensions[i] = -(miter * miter_length).dot(dir_out);
    }
    Some((ring, extensions))
}

/// The jettied storey stack for a building, or `None` for the plain path.
/// Only ordinary convex half-timber quads of 2+ storeys cantilever.
fn jetty_bands(building: &Building, base_y: f32, eave_y: f32) -> Option<Vec<StoreyBand>> {
    if base_y > 0.1
        || building.named
        || building.polygon.len() != 4
        || !polygon_is_convex(&building.polygon)
        || !matches!(wall_kind(&building.material), WallKind::HalfTimber)
        || building.levels < 2
        || stable_hash(&building.id) % 10 >= 8
    {
        return None;
    }
    let mut bands = Vec::new();
    let mut bottom = base_y;
    let mut storey = 0;
    while bottom < eave_y - 0.05 {
        let top = (bottom + BUILDING_FLOOR_HEIGHT).min(eave_y);
        // Ground floor sits on the cadastral line; each storey above steps out
        // one jetty, capped after two steps so alleys stay passable.
        let offset = JETTY_STEP * storey.min(2) as f32;
        let (polygon, start_extensions) = if offset > 0.0 {
            offset_convex_polygon(&building.polygon, offset)?
        } else {
            (building.polygon.clone(), vec![0.0; building.polygon.len()])
        };
        bands.push(StoreyBand {
            polygon,
            bottom,
            top,
            offset,
            start_extensions,
        });
        bottom = top;
        storey += 1;
    }
    (bands.len() >= 2).then_some(bands)
}

/// Openings re-addressed onto one storey band: only the rows inside the band,
/// with `along` corrected for the band's slid edge starts.
fn band_openings(openings: &[Vec<FacadeOpening>], band: &StoreyBand) -> Vec<Vec<FacadeOpening>> {
    openings
        .iter()
        .enumerate()
        .map(|(edge, list)| {
            list.iter()
                .filter(|opening| opening.center_y > band.bottom && opening.center_y < band.top)
                .map(|opening| FacadeOpening {
                    along: opening.along + band.start_extensions[edge],
                    ..*opening
                })
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn add_building_walls(
    walls: &mut BTreeMap<WallKind, MeshData>,
    timber: &mut MeshData,
    building: &Building,
    dominant: WallKind,
    base_y: f32,
    eave_y: f32,
    tint: [f32; 3],
    openings: &[Vec<FacadeOpening>],
) {
    let hash = stable_hash(&building.id);
    let tinted = |walls: &mut BTreeMap<WallKind, MeshData>,
                  kind: WallKind,
                  polygon: &[[f32; 2]],
                  bottom: f32,
                  top: f32,
                  edge_openings: &[Vec<FacadeOpening>]| {
        let mesh = walls.entry(kind).or_default();
        mesh.set_brush(tint);
        add_extruded_walls(mesh, polygon, bottom, top, base_y, edge_openings);
        mesh.reset_brush();
    };
    match dominant {
        WallKind::HalfTimber if base_y < 0.1 => {
            let stone_top = (base_y + 3.0).min(eave_y);
            tinted(
                walls,
                WallKind::Fieldstone,
                &building.polygon,
                base_y,
                stone_top,
                openings,
            );
            if stone_top < eave_y {
                // Plain plaster carries the storeys; the frame is geometry.
                tinted(
                    walls,
                    WallKind::Plaster,
                    &building.polygon,
                    stone_top,
                    eave_y,
                    openings,
                );
                add_timber_framing(
                    timber,
                    &building.polygon,
                    stone_top,
                    eave_y,
                    base_y,
                    openings,
                    hash,
                    tint,
                    true,
                    true,
                );
            }
        }
        WallKind::Plaster if base_y < 0.1 => {
            let plinth_top = (base_y + 0.65).min(eave_y);
            tinted(
                walls,
                WallKind::Fieldstone,
                &building.polygon,
                base_y,
                plinth_top,
                openings,
            );
            tinted(
                walls,
                WallKind::Plaster,
                &building.polygon,
                plinth_top,
                eave_y,
                openings,
            );
        }
        _ => {
            // Elevated half-timber shells (the bridge upper storeys, the
            // malt-house) frame their whole height over plaster.
            let kind = if dominant == WallKind::HalfTimber {
                WallKind::Plaster
            } else {
                dominant
            };
            tinted(walls, kind, &building.polygon, base_y, eave_y, openings);
            if dominant == WallKind::HalfTimber {
                add_timber_framing(
                    timber,
                    &building.polygon,
                    base_y,
                    eave_y,
                    base_y,
                    openings,
                    hash,
                    tint,
                    true,
                    true,
                );
            }
        }
    }
}

/// Walls for a jettied building: fieldstone ground floor on the cadastral
/// line, then half-timber storeys stepping out over the street, each step
/// closed underneath by a soffit ring and faced with a bressummer beam.
#[allow(clippy::too_many_arguments)]
fn add_jettied_walls(
    walls: &mut BTreeMap<WallKind, MeshData>,
    frames: &mut MeshData,
    timber: &mut MeshData,
    hash: u32,
    bands: &[StoreyBand],
    tint: [f32; 3],
    base_y: f32,
    openings: &[Vec<FacadeOpening>],
) {
    for (index, band) in bands.iter().enumerate() {
        let kind = if index == 0 {
            WallKind::Fieldstone
        } else {
            WallKind::Plaster
        };
        let scoped = band_openings(openings, band);
        let mesh = walls.entry(kind).or_default();
        mesh.set_brush(tint);
        add_extruded_walls(mesh, &band.polygon, band.bottom, band.top, base_y, &scoped);
        mesh.reset_brush();

        if index == 0 {
            continue;
        }
        let below = &bands[index - 1];
        let stepped = band.offset > below.offset + 0.01;
        // The frame of the storey: where the band steps out, the bressummer
        // below stands in for the bottom rail.
        add_timber_framing(
            timber,
            &band.polygon,
            band.bottom,
            band.top,
            base_y,
            &scoped,
            hash.wrapping_add(index as u32),
            tint,
            !stepped,
            index + 1 == bands.len(),
        );
        if !stepped {
            continue;
        }
        // Soffit: the visible underside of the cantilever, joist-dark.
        let mesh = walls.entry(WallKind::Plaster).or_default();
        mesh.set_brush(tint);
        let inner = &below.polygon;
        let outer = &band.polygon;
        let count = inner.len();
        for i in 0..count {
            let j = (i + 1) % count;
            let quad = [
                Vec3::new(outer[i][0], band.bottom, outer[i][1]),
                Vec3::new(outer[j][0], band.bottom, outer[j][1]),
                Vec3::new(inner[j][0], band.bottom, inner[j][1]),
                Vec3::new(inner[i][0], band.bottom, inner[i][1]),
            ];
            let first = mesh.positions.len() as u32;
            for point in quad {
                mesh.vertex_shaded(
                    point,
                    Vec3::NEG_Y,
                    Vec2::new(point.x / 7.0, point.z / 7.0),
                    0.5,
                );
            }
            mesh.indices.extend_from_slice(&[
                first,
                first + 1,
                first + 2,
                first,
                first + 2,
                first + 3,
            ]);
        }
        mesh.reset_brush();

        // Bressummer: the beam that carries the overhung wall.
        for (a, b) in outer.iter().zip(outer.iter().cycle().skip(1)) {
            let a2 = Vec2::from_array(*a);
            let b2 = Vec2::from_array(*b);
            let length = a2.distance(b2);
            if length < 0.4 {
                continue;
            }
            let center = (a2 + b2) * 0.5;
            add_oriented_box(
                frames,
                Vec3::new(center.x, band.bottom + 0.1, center.y),
                Vec3::new(length * 0.5, 0.1, 0.085),
                (b2 - a2) / length,
            );
        }
    }
}

fn building_verticals(building: &Building) -> (f32, f32) {
    if building.use_name == "bridge" {
        return (4.25, 9.0);
    }
    if building.id == "named_malt_house" {
        return (3.8, 11.2);
    }

    let eave = match building.id.as_str() {
        // The stone shaft only: the open belfry, bell, and spire that crown it
        // are authored in `build_bellstand_belfry`.
        "named_bellstand_tower" => 23.5,
        "named_old_sluice" => 12.5,
        "named_saint_marens" => 11.2,
        id if id.starts_with("gate_") => {
            if id.starts_with("gate_reed") {
                17.0
            } else {
                20.0
            }
        }
        id if id.starts_with("reserve_church_") => 10.5,
        _ => building.levels as f32 * BUILDING_FLOOR_HEIGHT + 0.45,
    };
    (0.0, eave)
}

fn wall_kind(material: &str) -> WallKind {
    match material {
        "limestone" => WallKind::Limestone,
        "fieldstone" => WallKind::Fieldstone,
        "half_timber" | "stone_timber" => WallKind::HalfTimber,
        "plaster" => WallKind::Plaster,
        other => panic!("unknown Ombreval wall material '{other}'"),
    }
}

fn roof_kind(building: &Building) -> RoofKind {
    if matches!(
        building.use_name.as_str(),
        "ecclesiastical" | "fortification" | "civic" | "guild" | "bridge"
    ) || building.material == "limestone"
    {
        RoofKind::Slate
    } else {
        let hash = stable_hash(&building.id);
        let wall_margin = building.district == "City wall"
            || building.district.contains("Reed")
            || building.district.contains("Sluice");
        if wall_margin && building.levels <= 2 && hash.is_multiple_of(11) {
            RoofKind::Thatch
        } else {
            RoofKind::Terracotta
        }
    }
}

/// Street filth climbs about a storey up a wall and then stops; below the knee
/// every façade in the references is visibly darker than above it.
fn grime_shade(y: f32, ground: f32) -> f32 {
    0.74 + 0.26 * ((y - ground) / 2.8).clamp(0.0, 1.0)
}

/// Extrude the footprint into walls, leaving genuine holes where the façade
/// plan put openings (an empty slice keeps every face blind).
fn add_extruded_walls(
    mesh: &mut MeshData,
    polygon: &[[f32; 2]],
    bottom: f32,
    top: f32,
    ground: f32,
    openings: &[Vec<FacadeOpening>],
) {
    if top <= bottom + 0.01 {
        return;
    }
    let orientation = plan::signed_area(polygon).signum();
    for (edge_index, (a, b)) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let a2 = Vec2::from_array(*a);
        let b2 = Vec2::from_array(*b);
        let edge = b2 - a2;
        let length = edge.length();
        if length < 0.01 {
            continue;
        }
        let mut normal = Vec3::new(edge.y, 0.0, -edge.x).normalize();
        if orientation < 0.0 {
            normal = -normal;
        }
        let edge_openings = openings.get(edge_index).map(Vec::as_slice).unwrap_or(&[]);
        add_wall_face_with_holes(
            mesh,
            a2,
            edge / length,
            normal,
            length,
            bottom,
            top,
            ground,
            edge_openings,
        );
    }
}

/// Emit one wall face as the rectangle complement of its openings: horizontal
/// bands where nothing opens, vertical piers between openings where they do.
/// The scanline is over the y-extents of the openings clipped to this band.
#[allow(clippy::too_many_arguments)]
fn add_wall_face_with_holes(
    mesh: &mut MeshData,
    origin: Vec2,
    direction: Vec2,
    normal: Vec3,
    length: f32,
    bottom: f32,
    top: f32,
    ground: f32,
    openings: &[FacadeOpening],
) {
    // Clip openings to this band and keep only the ones that actually cut it.
    let mut cuts: Vec<(f32, f32, f32, f32)> = openings
        .iter()
        .filter(|opening| opening.max_y() > bottom + 0.01 && opening.min_y() < top - 0.01)
        .map(|opening| {
            (
                opening.along - opening.width * 0.5,
                opening.along + opening.width * 0.5,
                opening.min_y().max(bottom),
                opening.max_y().min(top),
            )
        })
        .collect();
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut emit = |x0: f32, x1: f32, y0: f32, y1: f32| {
        if x1 - x0 < 0.005 || y1 - y0 < 0.005 {
            return;
        }
        let shade0 = grime_shade(y0, ground);
        let shade1 = grime_shade(y1, ground);
        let p0 = origin + direction * x0;
        let p1 = origin + direction * x1;
        let first = mesh.vertex_shaded(
            Vec3::new(p0.x, y0, p0.y),
            normal,
            Vec2::new(x0 / 7.0, y0 / 7.0),
            shade0,
        );
        mesh.vertex_shaded(
            Vec3::new(p1.x, y0, p1.y),
            normal,
            Vec2::new(x1 / 7.0, y0 / 7.0),
            shade0,
        );
        mesh.vertex_shaded(
            Vec3::new(p1.x, y1, p1.y),
            normal,
            Vec2::new(x1 / 7.0, y1 / 7.0),
            shade1,
        );
        mesh.vertex_shaded(
            Vec3::new(p0.x, y1, p0.y),
            normal,
            Vec2::new(x0 / 7.0, y1 / 7.0),
            shade1,
        );
        mesh.indices
            .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
    };

    if cuts.is_empty() {
        // Blind wall: still split at the grime knee so the gradient keeps its
        // elbow instead of smearing evenly to the eaves.
        let knee = (bottom + 2.8).min(top);
        emit(0.0, length, bottom, knee);
        emit(0.0, length, knee, top);
        return;
    }

    // Scanline over the distinct y-levels the openings introduce.
    let mut levels: Vec<f32> = vec![bottom, top];
    for &(_, _, y0, y1) in &cuts {
        levels.push(y0);
        levels.push(y1);
    }
    levels.sort_by(f32::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 0.005);

    for pair in levels.windows(2) {
        let (y0, y1) = (pair[0], pair[1]);
        let mid = (y0 + y1) * 0.5;
        let mut cursor = 0.0_f32;
        for &(x0, x1, cy0, cy1) in &cuts {
            if cy0 > mid || cy1 < mid {
                continue;
            }
            emit(cursor, x0, y0, y1);
            cursor = cursor.max(x1);
        }
        emit(cursor, length, y0, y1);
    }
}

/// How far each roof plane continues past the wall face, following its own
/// pitch. The shadow line this casts under the eaves is one of the strongest
/// "real building" cues there is.
const EAVES_OVERHANG: f32 = 0.55;
/// The verge: how far the roof oversails the gable ends along the ridge.
const VERGE_OVERHANG: f32 = 0.32;

fn add_building_roof(
    roof: &mut MeshData,
    gable_wall: &mut MeshData,
    polygon: &[[f32; 2]],
    eave_y: f32,
) -> (f32, Option<[Vec3; 2]>) {
    if polygon.len() != 4 {
        add_polygon_surface(roof, polygon, eave_y + 0.08, 7.0);
        return (0.16, None);
    }

    let p = [
        Vec2::from_array(polygon[0]),
        Vec2::from_array(polygon[1]),
        Vec2::from_array(polygon[2]),
        Vec2::from_array(polygon[3]),
    ];
    let edge_01 = p[0].distance(p[1]);
    let edge_12 = p[1].distance(p[2]);
    let roof_height = edge_01.min(edge_12).mul_add(0.32, 0.65).clamp(1.25, 4.2);
    let y_ridge = eave_y + roof_height;

    // The ridge spans the midpoints of the two short edges; each plane's eave
    // pair is listed (near, far) relative to the `ridge_a` end, and the gables
    // fill the short walls up to the ridge.
    let (ridge_a, ridge_b, eave_pairs, gables) = if edge_01 <= edge_12 {
        let a = (p[0] + p[1]) * 0.5;
        let b = (p[2] + p[3]) * 0.5;
        (
            a,
            b,
            [(p[0], p[3]), (p[1], p[2])],
            [(p[0], p[1], a), (p[3], p[2], b)],
        )
    } else {
        let a = (p[1] + p[2]) * 0.5;
        let b = (p[3] + p[0]) * 0.5;
        (
            a,
            b,
            [(p[1], p[0]), (p[2], p[3])],
            [(p[1], p[2], a), (p[0], p[3], b)],
        )
    };

    let ridge_dir = (ridge_b - ridge_a).normalize_or_zero();
    let mid_ridge = (ridge_a + ridge_b) * 0.5;
    for (near, far) in eave_pairs {
        // Push the eave edge out along the plane's own pitch so the overhang
        // droops instead of floating flat, and oversail both gable ends.
        let out = (near + far) * 0.5 - mid_ridge;
        let half_span = out.length().max(0.05);
        let out = out / half_span;
        let drop = EAVES_OVERHANG * roof_height / half_span;
        let eave_low = eave_y - drop;
        let e_near = near + out * EAVES_OVERHANG - ridge_dir * VERGE_OVERHANG;
        let e_far = far + out * EAVES_OVERHANG + ridge_dir * VERGE_OVERHANG;
        let r_near = ridge_a - ridge_dir * VERGE_OVERHANG;
        let r_far = ridge_b + ridge_dir * VERGE_OVERHANG;
        let points = [
            Vec3::new(e_near.x, eave_low, e_near.y),
            Vec3::new(e_far.x, eave_low, e_far.y),
            Vec3::new(r_far.x, y_ridge, r_far.y),
            Vec3::new(r_near.x, y_ridge, r_near.y),
        ];
        let mut normal = (points[1] - points[0])
            .cross(points[3] - points[0])
            .normalize_or(Vec3::Y);
        if normal.y < 0.0 {
            normal = -normal;
        }
        // Tile courses run with the pitch: u along the eave, v up the slope —
        // the old top-down planar map stretched tiles on every steep roof.
        let eave_dir = (e_far - e_near).normalize_or_zero();
        let slope_len =
            ((half_span + EAVES_OVERHANG).powi(2) + (roof_height + drop).powi(2)).sqrt();
        let u = |point: Vec2| (point - e_near).dot(eave_dir) / 7.0;
        roof.quad(
            points,
            normal,
            [
                Vec2::new(u(e_near), 0.0),
                Vec2::new(u(e_far), 0.0),
                Vec2::new(u(r_far), slope_len / 7.0),
                Vec2::new(u(r_near), slope_len / 7.0),
            ],
        );
    }

    // A half-round ridge cap: two pitched strips meeting a touch above the
    // ridge line, sailing the same verge as the planes.
    let cap_a = ridge_a - ridge_dir * VERGE_OVERHANG;
    let cap_b = ridge_b + ridge_dir * VERGE_OVERHANG;
    let cap_side = Vec2::new(ridge_dir.y, -ridge_dir.x);
    for side in [-1.0, 1.0] {
        let skirt = cap_side * side * 0.20;
        let points = [
            Vec3::new(cap_a.x, y_ridge + 0.09, cap_a.y),
            Vec3::new(cap_b.x, y_ridge + 0.09, cap_b.y),
            Vec3::new(cap_b.x + skirt.x, y_ridge - 0.05, cap_b.y + skirt.y),
            Vec3::new(cap_a.x + skirt.x, y_ridge - 0.05, cap_a.y + skirt.y),
        ];
        let mut normal = (points[1] - points[0])
            .cross(points[3] - points[0])
            .normalize_or(Vec3::Y);
        if normal.y < 0.0 {
            normal = -normal;
        }
        roof.quad(
            points,
            normal,
            [
                Vec2::ZERO,
                Vec2::new(cap_a.distance(cap_b) / 7.0, 0.0),
                Vec2::new(cap_a.distance(cap_b) / 7.0, 0.05),
                Vec2::new(0.0, 0.05),
            ],
        );
    }

    for (a, b, ridge) in gables {
        gable_wall.triangle(
            [
                Vec3::new(a.x, eave_y, a.y),
                Vec3::new(b.x, eave_y, b.y),
                Vec3::new(ridge.x, y_ridge, ridge.y),
            ],
            [Vec2::ZERO, Vec2::X, Vec2::new(0.5, roof_height / 4.0)],
            false,
        );
    }

    debug_assert!(ridge_a.distance(ridge_b) > 0.1);
    (
        roof_height,
        Some([
            Vec3::new(ridge_a.x, y_ridge, ridge_a.y),
            Vec3::new(ridge_b.x, y_ridge, ridge_b.y),
        ]),
    )
}

/// Chimneys are what a skyline is made of. One or two fieldstone stacks per
/// gabled building, planted on the ridge at a stable per-building spot. Every
/// stack reports its flue top so `smoke::build_chimney_smoke` can light a
/// hash-picked subset of hearths.
fn add_chimneys(
    mesh: &mut MeshData,
    building: &Building,
    ridge: [Vec3; 2],
    anchors: &mut Vec<smoke::ChimneyAnchor>,
) {
    if building.use_name == "bridge" {
        return;
    }
    let [ridge_a, ridge_b] = ridge;
    let ridge_len = ridge_a.distance(ridge_b);
    if ridge_len < 2.5 {
        return;
    }
    let hash = stable_hash(&building.id);
    let along = Vec2::new(ridge_b.x - ridge_a.x, ridge_b.z - ridge_a.z) / ridge_len;
    let count = if ridge_len > 15.0 && hash % 3 == 0 {
        2
    } else {
        1
    };
    for index in 0..count {
        let t = if count == 2 {
            0.26 + 0.48 * index as f32
        } else {
            0.28 + (hash % 45) as f32 / 100.0
        };
        let base = ridge_a.lerp(ridge_b, t);
        // Stack sunk into the ridge, flaring into a cap slab above.
        add_oriented_box(
            mesh,
            base + Vec3::Y * 0.25,
            Vec3::new(0.36, 1.0, 0.36),
            along,
        );
        add_oriented_box(
            mesh,
            base + Vec3::Y * 1.3,
            Vec3::new(0.48, 0.08, 0.48),
            along,
        );
        let seed = stable_hash(&format!("smoke-{}-{index}", building.id));
        anchors.push(smoke::ChimneyAnchor {
            top: base + Vec3::Y * 1.5,
            seed,
            early: smoke::early_hearth(seed, &building.use_name),
            cold: building.use_name == "storage",
        });
    }
}

/// An axis-defined box written straight into a batched mesh: `along` is the
/// local +X direction in the ground plane, `half` the half-extents.
/// One box, oriented in the XZ plane by the unit vector `along`.
///
/// **The corner ring is emitted in reverse.** This table is written as
/// `(normal, right, up)` but populated with the two side vectors transposed, so
/// `right × up == -normal` for all six faces — the opposite of the outward
/// winding `MeshData::quad` needs. The ring is therefore walked backwards here.
/// Fixing it the other way, by swapping `right` and `up` to restore
/// `right × up == normal` (the invariant `add_dressed_stone` states), also swaps
/// `half_r` and `half_u` and so rotates every face's texture 90° — visible as
/// cross-grain on the balcony rails, stair stringers and hoist beams this
/// builds. Reversing the ring keeps U horizontal and V vertical on every face.
///
/// This was latent, not visible, and it is worth being exact about that: fixing
/// it changed the rendered city by under 1/255 mean luminance. Nothing looked
/// wrong because every city material carries `double_sided: true,
/// cull_mode: None`, and the *vertex* normal written by `MeshData::quad` was
/// always the correct outward one — so both faces rasterize and both shade off a
/// good normal. The winding was simply never consulted.
///
/// Nor is the consequence dramatic if culling is ever switched on, which was
/// measured rather than guessed: forcing `cull_mode: Some(Back)` on the textured
/// materials moved this frame by 0.33% of pixels at more than 16/255. An
/// inverted *closed* box does not disappear under culling — its near face is
/// culled and its far face, now front-facing, draws in place, so you get the
/// inside of the back wall instead of the outside of the front one. Subtle, and
/// wrong. (That experiment also showed the city goes x-ray under culling for an
/// unrelated reason: the wall panels themselves are single-sided. See
/// `dark_wood`, set `double_sided` because otherwise "half of them face the
/// wrong way and vanish".)
///
/// So this is hygiene, not a bug fix with a screenshot: 43 call sites (chimneys,
/// door and window modules, balconies, yard stairs, covered passages, arcades,
/// hoists, street props) now emit meshes that describe what they actually are,
/// and `add_oriented_box` agrees with the invariant `add_dressed_stone` states
/// three thousand lines down. Anything that reads geometric facing rather than
/// the stored normal — culling, a mesh export, tooling that recomputes normals —
/// gets the right answer now and did not before. Guarded by
/// `every_oriented_box_face_is_wound_outward`.
fn add_oriented_box(mesh: &mut MeshData, center: Vec3, half: Vec3, along: Vec2) {
    let ax = Vec3::new(along.x, 0.0, along.y);
    let az = Vec3::new(-along.y, 0.0, along.x);
    let ay = Vec3::Y;
    for (normal, right, up, half_n, half_r, half_u) in [
        (ax, az, ay, half.x, half.z, half.y),
        (-ax, -az, ay, half.x, half.z, half.y),
        (az, -ax, ay, half.z, half.x, half.y),
        (-az, ax, ay, half.z, half.x, half.y),
        (ay, ax, az, half.y, half.x, half.z),
        (-ay, -ax, az, half.y, half.x, half.z),
    ] {
        let face_center = center + normal * half_n;
        let points = [
            face_center - right * half_r + up * half_u,
            face_center + right * half_r + up * half_u,
            face_center + right * half_r - up * half_u,
            face_center - right * half_r - up * half_u,
        ];
        mesh.quad(
            points,
            normal,
            [
                Vec2::new(0.0, half_u / 3.5),
                Vec2::new(half_r / 3.5, half_u / 3.5),
                Vec2::new(half_r / 3.5, 0.0),
                Vec2::ZERO,
            ],
        );
    }
}

/// One timber member on a wall face, in edge-local coordinates: from
/// `(along, y)` `a` to `(along, y)` `b`, `half_width` across the member in the
/// face plane, standing `proud` of the wall with its back buried. Front and
/// side faces only unless `ends` — the buried back face is never emitted.
#[allow(clippy::too_many_arguments)]
fn add_face_member(
    mesh: &mut MeshData,
    origin: Vec2,
    direction: Vec2,
    normal2: Vec2,
    a: Vec2,
    b: Vec2,
    half_width: f32,
    proud: f32,
    ends: bool,
) {
    let to_world = |p: Vec2| {
        let flat = origin + direction * p.x;
        Vec3::new(flat.x, p.y, flat.y)
    };
    let normal = Vec3::new(normal2.x, 0.0, normal2.y);
    let axis = (to_world(b) - to_world(a)).normalize_or_zero();
    if axis == Vec3::ZERO {
        return;
    }
    let across = normal.cross(axis).normalize_or(Vec3::Y);
    // The back sits a touch inside the wall so no seam ever opens.
    let depth_half = (proud + 0.04) * 0.5;
    let shift = normal * (proud - depth_half);
    let start = to_world(a) + shift;
    let end = to_world(b) + shift;
    let face = |mesh: &mut MeshData, points: [Vec3; 4], face_normal: Vec3| {
        let u = points[0].distance(points[1]) / 3.5;
        let v = points[1].distance(points[2]) / 3.5;
        mesh.quad(
            points,
            face_normal,
            [
                Vec2::ZERO,
                Vec2::new(u, 0.0),
                Vec2::new(u, v),
                Vec2::new(0.0, v),
            ],
        );
    };
    let w = across * half_width;
    let d = normal * depth_half;
    face(
        mesh,
        [start - w + d, end - w + d, end + w + d, start + w + d],
        normal,
    );
    face(
        mesh,
        [start - w - d, end - w - d, end - w + d, start - w + d],
        -across,
    );
    face(
        mesh,
        [start + w + d, end + w + d, end + w - d, start + w - d],
        across,
    );
    if ends {
        face(
            mesh,
            [start - w - d, start - w + d, start + w + d, start + w - d],
            -axis,
        );
        face(
            mesh,
            [end - w - d, end + w - d, end + w + d, end - w + d],
            axis,
        );
    }
}

/// The structural skeleton of a half-timber storey band, drawn as real
/// geometry instead of a painted grid: corner posts, rails on the storey
/// lines, hash-jittered studs that step around the openings, and a diagonal
/// brace where a corner leaves room for one.
#[allow(clippy::too_many_arguments)]
fn add_timber_framing(
    timber: &mut MeshData,
    polygon: &[[f32; 2]],
    bottom: f32,
    top: f32,
    ground: f32,
    openings: &[Vec<FacadeOpening>],
    hash: u32,
    tint: [f32; 3],
    bottom_rail: bool,
    top_rail: bool,
) {
    let height = top - bottom;
    if height < 0.4 {
        return;
    }
    timber.set_brush(tint);
    let orientation = plan::signed_area(polygon).signum();
    for (edge_index, (a, b)) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let a2 = Vec2::from_array(*a);
        let b2 = Vec2::from_array(*b);
        let edge = b2 - a2;
        let length = edge.length();
        if length < 0.6 {
            continue;
        }
        let direction = edge / length;
        let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
        if orientation < 0.0 {
            normal2 = -normal2;
        }
        let edge_hash = hash ^ (edge_index as u32).wrapping_mul(0x9E37_79B9);

        // Corner post on the shared vertex, proud of both meeting faces.
        add_oriented_box(
            timber,
            Vec3::new(a2.x, (bottom + top) * 0.5, a2.y),
            Vec3::new(0.11, height * 0.5, 0.11),
            direction,
        );

        // The openings that actually pierce this band on this edge.
        let cuts: Vec<&FacadeOpening> = openings
            .get(edge_index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .filter(|opening| opening.max_y() > bottom + 0.05 && opening.min_y() < top - 0.05)
            .collect();

        // Rails: the sill beam over the masonry, one on every storey line the
        // band crosses, and the wall plate under the eaves. The storey lines
        // thread between the window rows by construction, so rails never cross
        // glass.
        let mut rail_ys = Vec::new();
        if bottom_rail {
            rail_ys.push(bottom + 0.09);
        }
        let mut line = (((bottom - ground) / BUILDING_FLOOR_HEIGHT).floor() + 1.0)
            * BUILDING_FLOOR_HEIGHT
            + ground;
        while line < top - 0.45 {
            if line > bottom + 0.45 {
                rail_ys.push(line);
            }
            line += BUILDING_FLOOR_HEIGHT;
        }
        if top_rail {
            rail_ys.push(top - 0.10);
        }
        for y in rail_ys {
            add_face_member(
                timber,
                a2,
                direction,
                normal2,
                Vec2::new(0.10, y),
                Vec2::new(length - 0.10, y),
                0.075,
                0.13,
                false,
            );
        }

        if height < 1.1 {
            continue;
        }

        // Studs: jittered 1.2–1.8 m spacing, skipped where a window interrupts.
        let mut along = 0.85 + (edge_hash % 37) as f32 / 60.0;
        let mut stud_index = 0u32;
        while along < length - 0.7 {
            let clear = cuts
                .iter()
                .all(|opening| (along - opening.along).abs() > opening.width * 0.5 + 0.17);
            if clear {
                add_face_member(
                    timber,
                    a2,
                    direction,
                    normal2,
                    Vec2::new(along, bottom + 0.06),
                    Vec2::new(along, top - 0.06),
                    0.055,
                    0.12,
                    false,
                );
            }
            let step_hash = edge_hash ^ stud_index.wrapping_mul(0x85EB_CA6B);
            along += 1.2 + (step_hash % 61) as f32 / 100.0;
            stud_index += 1;
        }

        // A diagonal brace off a corner post — the classic K/Z patterns, two
        // orientations picked by hash, only where no opening blocks the run.
        let rise = (height - 0.3).min(2.35);
        let run = rise.mul_add(0.55, 0.35);
        let variants = [
            (0.16, bottom + 0.12, 0.16 + run, bottom + 0.12 + rise),
            (
                length - 0.16,
                bottom + 0.12,
                length - 0.16 - run,
                bottom + 0.12 + rise,
            ),
        ];
        let pick = (edge_hash >> 6) % 3;
        for (variant, (x0, y0, x1, y1)) in variants.into_iter().enumerate() {
            if pick != 2 && pick != variant as u32 {
                continue;
            }
            let (lo, hi) = (x0.min(x1), x0.max(x1));
            let clear = lo > 0.1
                && hi < length - 0.1
                && cuts.iter().all(|opening| {
                    opening.along + opening.width * 0.5 + 0.15 < lo
                        || opening.along - opening.width * 0.5 - 0.15 > hi
                });
            if clear {
                add_face_member(
                    timber,
                    a2,
                    direction,
                    normal2,
                    Vec2::new(x0, y0),
                    Vec2::new(x1, y1),
                    0.065,
                    0.115,
                    true,
                );
            }
        }
    }
    timber.reset_brush();
}

/// How deep every opening sits behind the wall face. The reveal this exposes
/// is the difference between a window and a sticker.
const OPENING_DEPTH: f32 = 0.15;

/// Line the holes the wall builder left: glass, reveals, sills, lintels,
/// shutters and door leaves. Purely decorative — the openings themselves were
/// cut by `add_wall_face_with_holes`. Works on whichever footprint the walls
/// actually used (a jettied storey's ring, or the cadastral polygon).
///
/// `cut_profile` is the Cut margin's ground (`CutMarginProfile`), sampled at
/// each doorway so a door opening onto the raised flags keeps its threshold
/// on them — see `add_door_module` for what rides up and why.
fn add_facade_openings_on(
    windows: &mut MeshData,
    rooms: &mut MeshData,
    doors: &mut MeshData,
    frames: &mut MeshData,
    polygon: &[[f32; 2]],
    openings: &[Vec<FacadeOpening>],
    cut_profile: &CutMarginProfile,
) {
    let orientation = plan::signed_area(polygon).signum();
    for (edge_index, (a, b)) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let edge_openings = match openings.get(edge_index) {
            Some(list) if !list.is_empty() => list,
            _ => continue,
        };
        let a = Vec2::from_array(*a);
        let b = Vec2::from_array(*b);
        let edge = b - a;
        let length = edge.length();
        if length < 0.01 {
            continue;
        }
        let direction = edge / length;
        let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
        if orientation < 0.0 {
            normal2 = -normal2;
        }
        for opening in edge_openings {
            let wall_point = a + direction * opening.along;
            match opening.kind {
                OpeningKind::Window { shutters } => add_window_module(
                    windows,
                    rooms,
                    doors,
                    frames,
                    wall_point,
                    opening.center_y,
                    direction,
                    normal2,
                    opening.width,
                    opening.height,
                    opening.hash,
                    shutters,
                ),
                OpeningKind::Door => {
                    // Sampled where the threshold slab actually lies — 0.3 m
                    // out from the wall face — which for a Cut-margin door is
                    // the outermost flag lane, never a feathered strip end.
                    let step = wall_point + normal2 * 0.3;
                    add_door_module(
                        doors,
                        frames,
                        wall_point,
                        opening.min_y(),
                        cut_profile.ground_lift(step.x, step.y),
                        direction,
                        normal2,
                    );
                }
            }
        }
    }
}

/// One real window: glass sunk behind the wall face, reveal returns, a
/// projecting sill and lintel, a mullion cross, and sometimes open shutters.
#[allow(clippy::too_many_arguments)]
fn add_window_module(
    windows: &mut MeshData,
    rooms: &mut MeshData,
    doors: &mut MeshData,
    frames: &mut MeshData,
    wall_point: Vec2,
    center_y: f32,
    direction: Vec2,
    normal2: Vec2,
    width: f32,
    height: f32,
    hash: u32,
    shutters_allowed: bool,
) {
    let normal = Vec3::new(normal2.x, 0.0, normal2.y);
    add_window_room(rooms, wall_point, center_y, direction, normal2, width, height, hash);
    // The glass slightly overlaps the hole so no slit into the hollow shell
    // survives at the reveal borders.
    let glass_center = wall_point - normal2 * OPENING_DEPTH;
    add_facade_panel(
        windows,
        glass_center,
        center_y,
        direction,
        normal,
        width + 0.06,
        height + 0.06,
    );
    add_reveal(
        frames,
        wall_point,
        center_y,
        direction,
        normal2,
        width,
        height,
        OPENING_DEPTH,
        false,
    );

    // Mullion cross on the glass plane.
    let mullion_center = wall_point - normal2 * (OPENING_DEPTH - 0.03);
    add_facade_panel(
        frames,
        mullion_center,
        center_y,
        direction,
        normal,
        0.055,
        height - 0.08,
    );
    add_facade_panel(
        frames,
        mullion_center,
        center_y,
        direction,
        normal,
        width - 0.08,
        0.055,
    );

    // Sill: a slab proud of the wall below the opening; lintel above.
    add_oriented_box(
        frames,
        Vec3::new(wall_point.x, center_y - height * 0.5 - 0.04, wall_point.y) + normal * 0.05,
        Vec3::new(width * 0.5 + 0.08, 0.045, 0.09),
        direction,
    );
    add_oriented_box(
        frames,
        Vec3::new(wall_point.x, center_y + height * 0.5 + 0.05, wall_point.y) + normal * 0.03,
        Vec3::new(width * 0.5 + 0.06, 0.055, 0.07),
        direction,
    );

    // Shutters folded back against the wall — one leaf or both.
    if shutters_allowed {
        let leaf_width = width * 0.52;
        let sides: &[f32] = if hash % 5 < 3 { &[-1.0, 1.0] } else { &[1.0] };
        for side in sides {
            let leaf_center =
                wall_point + direction * side * (width * 0.5 + leaf_width * 0.5 + 0.04);
            add_facade_panel(
                doors,
                leaf_center + normal2 * 0.045,
                center_y,
                direction,
                normal,
                leaf_width,
                height - 0.04,
            );
        }
    }
}

/// How deep the room shell behind every pane extends into the building.
/// Shallow enough to stay inside any plausible parcel; two facing windows'
/// rooms may interpenetrate mid-building, which nothing can ever see.
const ROOM_DEPTH: f32 = 1.5;

/// The chamber a near-clear pane reveals: back wall, floor, ceiling and
/// cheeks in soot-dark plaster, sized past the opening so an angled look
/// still lands on interior surface. The shells have nothing else renderable
/// inside (their walls are single-sided), so this box bounds everything the
/// distance-faded glass can show.
#[allow(clippy::too_many_arguments)]
fn add_window_room(
    rooms: &mut MeshData,
    wall_point: Vec2,
    center_y: f32,
    direction: Vec2,
    normal2: Vec2,
    width: f32,
    height: f32,
    hash: u32,
) {
    let along = Vec3::new(direction.x, 0.0, direction.y);
    let inward = -Vec3::new(normal2.x, 0.0, normal2.y);
    let anchor = Vec3::new(wall_point.x, 0.0, wall_point.y);
    let near = anchor + inward * (OPENING_DEPTH + 0.02);
    let far = anchor + inward * ROOM_DEPTH;
    let half = width * 0.5 + 0.45;
    let floor_y = center_y - height * 0.5 - 0.75;
    let ceil_y = center_y + height * 0.5 + 0.45;
    // A stable warm shade per room, so a terrace of panes doesn't read as one
    // repeated cell; always darker than any daylit surface outside.
    let shade = 0.72 + ((hash >> 9) % 33) as f32 / 100.0;
    rooms.set_brush([shade, shade * 0.93, shade * 0.82]);
    let corner = |base: Vec3, side: f32, y: f32| Vec3::new(base.x, y, base.z) + along * side;
    let uvs = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y];
    // The material is unlit, so the chamber's light is painted here: the back
    // wall catches the window light, the floor a little less, the cheeks fall
    // off, and the ceiling sits in the dark — a hand-shaded room interior.
    let mut shaded_quad = |points: [Vec3; 4], normal: Vec3, surface_shade: f32| {
        let first = rooms.positions.len() as u32;
        for (point, uv) in points.into_iter().zip(uvs) {
            rooms.vertex_shaded(point, normal, uv, surface_shade);
        }
        rooms
            .indices
            .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
    };
    // Back wall, facing the pane.
    shaded_quad(
        [
            corner(far, -half, floor_y),
            corner(far, half, floor_y),
            corner(far, half, ceil_y),
            corner(far, -half, ceil_y),
        ],
        -inward,
        1.0,
    );
    // Floor and ceiling.
    shaded_quad(
        [
            corner(near, -half, floor_y),
            corner(near, half, floor_y),
            corner(far, half, floor_y),
            corner(far, -half, floor_y),
        ],
        Vec3::Y,
        0.78,
    );
    shaded_quad(
        [
            corner(near, -half, ceil_y),
            corner(near, half, ceil_y),
            corner(far, half, ceil_y),
            corner(far, -half, ceil_y),
        ],
        Vec3::NEG_Y,
        0.35,
    );
    // Cheeks.
    for side in [-1.0_f32, 1.0] {
        shaded_quad(
            [
                corner(near, side * half, floor_y),
                corner(far, side * half, floor_y),
                corner(far, side * half, ceil_y),
                corner(near, side * half, ceil_y),
            ],
            along * -side,
            0.58,
        );
    }
    rooms.reset_brush();
}

/// The door: leaf recessed behind the face, reveal returns, a proud lintel and
/// a worn threshold slab at the foot.
///
/// `ground_lift` is how far the street's own ground stands above the
/// building's `base_y` at the doorway — zero everywhere except the Cut's
/// raised margin (`the_cut_kerb.md` M3), whose flags run right up to the
/// façade lines at `CUT_MARGIN_Y + CUT_STEP_M` while the buildings behind
/// them keep `base_y = 0`. The visible module — leaf bottom, reveal, and the
/// threshold slab — starts at the lifted ground, so the sill stands the same
/// 0.065 m proud of the flags that it stands proud of the old margin
/// everywhere else: the gazetteer's *"slightly raised thresholds"*, rather
/// than a sill buried 0.185 m under the paving and a doorway a cart climbing
/// the kerb break's ramp would then drop into. The lintel and the wall's own
/// cut opening stay put — everything below the flag top is occluded by the
/// slab the flags are drawn as, so only the parts that read had to move.
fn add_door_module(
    doors: &mut MeshData,
    frames: &mut MeshData,
    wall_point: Vec2,
    base_y: f32,
    ground_lift: f32,
    direction: Vec2,
    normal2: Vec2,
) {
    let normal = Vec3::new(normal2.x, 0.0, normal2.y);
    let width = 1.35;
    let height = 2.5;
    let foot_y = base_y + ground_lift;
    let leaf_height = height - ground_lift;
    let center_y = foot_y + leaf_height * 0.5;
    add_facade_panel(
        doors,
        wall_point - normal2 * (OPENING_DEPTH - 0.03),
        center_y,
        direction,
        normal,
        width + 0.06,
        leaf_height + 0.04,
    );
    add_reveal(
        frames,
        wall_point,
        center_y,
        direction,
        normal2,
        width,
        leaf_height,
        OPENING_DEPTH - 0.03,
        true,
    );
    add_oriented_box(
        frames,
        Vec3::new(wall_point.x, base_y + height + 0.07, wall_point.y) + normal * 0.04,
        Vec3::new(width * 0.5 + 0.12, 0.07, 0.09),
        direction,
    );
    // Threshold: a step slab proud of the wall at ground level.
    add_oriented_box(
        frames,
        Vec3::new(wall_point.x, foot_y + 0.045, wall_point.y) + normal * 0.14,
        Vec3::new(width * 0.5 + 0.05, 0.05, 0.24),
        direction,
    );
}

/// The four (or three, for a door) return faces connecting the wall plane to a
/// recessed opening. This is what makes an opening read as a hole.
#[allow(clippy::too_many_arguments)]
fn add_reveal(
    frames: &mut MeshData,
    wall_point: Vec2,
    center_y: f32,
    direction: Vec2,
    normal2: Vec2,
    width: f32,
    height: f32,
    depth: f32,
    skip_bottom: bool,
) {
    let normal = Vec3::new(normal2.x, 0.0, normal2.y);
    let along = Vec3::new(direction.x, 0.0, direction.y);
    let center = Vec3::new(wall_point.x, center_y, wall_point.y);
    let half_w = along * (width * 0.5);
    let half_h = Vec3::Y * (height * 0.5);
    let inward = -normal * depth;

    // Side returns face each other across the opening.
    for side in [-1.0, 1.0] {
        let outer_top = center + half_w * side + half_h;
        let outer_bottom = center + half_w * side - half_h;
        frames.quad(
            [
                outer_bottom,
                outer_top,
                outer_top + inward,
                outer_bottom + inward,
            ],
            along * -side,
            [
                Vec2::ZERO,
                Vec2::new(height / 7.0, 0.0),
                Vec2::new(height / 7.0, depth / 7.0),
                Vec2::new(0.0, depth / 7.0),
            ],
        );
    }
    // Head return faces down; sill return faces up.
    let head_a = center - half_w + half_h;
    let head_b = center + half_w + half_h;
    frames.quad(
        [head_a, head_b, head_b + inward, head_a + inward],
        Vec3::NEG_Y,
        [
            Vec2::ZERO,
            Vec2::new(width / 7.0, 0.0),
            Vec2::new(width / 7.0, depth / 7.0),
            Vec2::new(0.0, depth / 7.0),
        ],
    );
    if !skip_bottom {
        let foot_a = center - half_w - half_h;
        let foot_b = center + half_w - half_h;
        frames.quad(
            [foot_a, foot_b, foot_b + inward, foot_a + inward],
            Vec3::Y,
            [
                Vec2::ZERO,
                Vec2::new(width / 7.0, 0.0),
                Vec2::new(width / 7.0, depth / 7.0),
                Vec2::new(0.0, depth / 7.0),
            ],
        );
    }
}

fn add_facade_panel(
    mesh: &mut MeshData,
    center: Vec2,
    center_y: f32,
    along: Vec2,
    normal: Vec3,
    width: f32,
    height: f32,
) {
    let horizontal = Vec3::new(along.x, 0.0, along.y) * (width * 0.5);
    let vertical = Vec3::Y * (height * 0.5);
    let center = Vec3::new(center.x, center_y, center.y);
    mesh.quad(
        [
            center - horizontal - vertical,
            center + horizontal - vertical,
            center + horizontal + vertical,
            center - horizontal + vertical,
        ],
        normal,
        [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
    );
}

/// Register the visible footprint itself as collision geometry.
///
/// Almost every cadastral building is a convex quadrilateral. The two concave
/// non-cathedral footprints are decomposed into triangles so their passages
/// remain open instead of being filled by an oversized convex hull.
fn add_footprint_colliders(
    collision_world: &mut CollisionWorld,
    polygon: &[[f32; 2]],
    min_y: f32,
    max_y: f32,
) {
    if polygon_is_convex(polygon) {
        collision_world.add_convex_prism(polygon, min_y, max_y);
        return;
    }

    for triangle in triangulate_polygon(polygon) {
        collision_world.add_convex_prism(&triangle.map(|vertex| polygon[vertex]), min_y, max_y);
    }
}

fn build_fixtures(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    fixtures: &[Fixture],
    collision_world: &mut CollisionWorld,
) {
    for (index, fixture) in fixtures.iter().enumerate() {
        if fixture.kind == "statue" {
            continue;
        }
        let position = Vec3::new(fixture.position[0], 0.0, fixture.position[1]);
        let angle = fixture.angle_deg.to_radians();
        let name = fixture
            .label
            .clone()
            .unwrap_or_else(|| format!("{} [{}]", fixture.kind, fixture.id));
        match fixture.kind.as_str() {
            "stall" => spawn_market_stall(
                commands,
                meshes,
                materials,
                collision_world,
                position,
                Vec2::from_array(fixture.size),
                angle,
                index,
                name,
            ),
            "stone_stack" => {
                spawn_rotated_box_named(
                    commands,
                    meshes,
                    &materials.limestone,
                    position + Vec3::Y * 0.65,
                    Vec3::new(fixture.size[0], 1.3, fixture.size[1]),
                    angle,
                    name,
                );
                add_rotated_box_collider(
                    collision_world,
                    position,
                    Vec2::from_array(fixture.size),
                    angle,
                    1.3,
                );
            }
            "smoke_rack" => spawn_smoke_rack(commands, meshes, materials, position, angle, &name),
            "well" | "chain_well" | "three_curb_well" | "lodge_well" | "cistern"
            | "step_cistern" | "fire_tanks" => water::spawn_water_fixture(
                commands,
                meshes,
                materials,
                collision_world,
                &fixture.id,
                &fixture.kind,
                position,
                Vec2::from_array(fixture.size),
                angle,
            ),
            "stone" => {
                spawn_mesh_named(
                    commands,
                    &meshes.sphere,
                    &materials.fieldstone,
                    Transform::from_translation(position + Vec3::Y * 0.55).with_scale(Vec3::new(
                        fixture.size[0] * 0.38,
                        0.55,
                        fixture.size[1] * 0.38,
                    )),
                    name,
                );
                add_rotated_box_collider(
                    collision_world,
                    position,
                    Vec2::new(fixture.size[0] * 0.65, fixture.size[1] * 0.65),
                    0.0,
                    1.2,
                );
            }
            "platform" => {
                spawn_rotated_box_named(
                    commands,
                    meshes,
                    &materials.limestone,
                    position + Vec3::Y * 0.4,
                    Vec3::new(fixture.size[0], 0.8, fixture.size[1]),
                    angle,
                    name,
                );
                add_rotated_box_collider(
                    collision_world,
                    position,
                    Vec2::from_array(fixture.size),
                    angle,
                    0.8,
                );
            }
            "weighbeam" => spawn_weighbeam(commands, meshes, materials, position, angle, &name),
            "tracing" => spawn_rotated_box_named(
                commands,
                meshes,
                &materials.paving,
                position + Vec3::Y * 0.055,
                Vec3::new(fixture.size[0], 0.1, fixture.size[1]),
                angle,
                name,
            ),
            "crane" => spawn_yard_crane(commands, meshes, materials, position, angle, &name),
            other => warn!("unrendered Ombreval fixture kind: {other}"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_market_stall(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    position: Vec3,
    size: Vec2,
    angle: f32,
    variant: usize,
    name: String,
) {
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.timber,
        position + Vec3::Y * 0.65,
        Vec3::new(size.x, 1.3, size.y),
        angle,
        name,
    );
    let canopy = if variant.is_multiple_of(2) {
        &materials.cloth_ochre
    } else {
        &materials.cloth_russet
    };
    spawn_rotated_box_named(
        commands,
        meshes,
        canopy,
        position + Vec3::Y * 2.45,
        Vec3::new(size.x + 0.55, 0.18, size.y + 0.55),
        angle,
        "Stall awning",
    );
    add_rotated_box_collider(collision_world, position, size, angle, 1.35);
}

fn spawn_smoke_rack(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    position: Vec3,
    angle: f32,
    name: &str,
) {
    let right = Quat::from_rotation_y(angle) * Vec3::X;
    for side in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.dark_wood,
            position + right * side * 2.0 + Vec3::Y * 1.5,
            Vec3::new(0.18, 3.0, 0.18),
            name,
        );
    }
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        position + Vec3::Y * 2.85,
        Vec3::new(4.4, 0.18, 0.18),
        angle,
        "Smoke rack beam",
    );
}

fn spawn_weighbeam(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    position: Vec3,
    angle: f32,
    name: &str,
) {
    let right = Quat::from_rotation_y(angle) * Vec3::X;
    for side in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.timber,
            position + right * side * 5.8 + Vec3::Y * 2.6,
            Vec3::new(0.45, 5.2, 0.45),
            name,
        );
    }
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        position + Vec3::Y * 5.0,
        Vec3::new(14.0, 0.5, 0.5),
        angle,
        "Tallage weighing beam",
    );
    trade_props::spawn_weighbeam_rig(commands, meshes, materials, position, angle);
}

fn spawn_yard_crane(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    position: Vec3,
    angle: f32,
    name: &str,
) {
    spawn_box_named(
        commands,
        meshes,
        &materials.timber,
        position + Vec3::Y * 4.5,
        Vec3::new(0.75, 9.0, 0.75),
        name,
    );
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.timber,
        position + Vec3::Y * 8.0,
        Vec3::new(8.0, 0.55, 0.55),
        angle - 0.2,
        "Yard crane arm",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        position + Quat::from_rotation_y(angle - 0.2) * Vec3::X * 3.7 + Vec3::Y * 6.3,
        Vec3::new(0.08, 3.4, 0.08),
        "Yard crane chain",
    );
}

fn build_named_details(
    commands: &mut Commands,
    meshes: &CityMeshes,
    mesh_assets: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    build_bellstand_belfry(commands, meshes, materials, collision_world);
    build_stone_house(commands, meshes, materials, collision_world);
    build_saint_maren_tower(commands, meshes, materials, collision_world);
    build_parish_towers(commands, meshes, materials, plan, collision_world);
    build_old_sluice_face(commands, meshes, materials);
    build_charnel_and_ilvane_details(commands, meshes, materials);
    build_bridge_supports(commands, meshes, materials, plan, collision_world);
    build_ropewalk(commands, meshes, materials);
    build_kerb(commands, mesh_assets, materials, plan, collision_world);
    build_osanne_stall(commands, meshes, materials, collision_world);
    build_wharf_cranes(commands, meshes, materials);
}

/// The Stone House (`features/law_and_order.md` M5a) — the civic gaol, in the
/// side court behind the Bellstand square and at the foot of the watch-bell
/// tower.
///
/// The name is older than the building. `lore/core_lore/secular_government.md`
/// puts the first Stone House by the River Gate; it was condemned in the
/// Hammering and custody moved to the watch's own yard, and the old still call
/// this one by the old name. The gameplay argument is the same one M2 and M4
/// make about every station: the bench is *already posted here*
/// (`rounds.json: workplaces["bailiff_and_gaoler"] = ["Bellstand watch-bell
/// tower"]`), so guards and gaol in one yard costs nothing, escorts stay short,
/// and a prisoner told they go at Lamplight hears the Scold ring Lamplight over
/// their own head.
///
/// **It is a room, not a mass.** Only the walls are colliders, so the nav bake —
/// which erodes the exported collider footprints rather than the plan
/// (`scripts/bake_navigation.py::build_walkable`) — leaves the interior walkable
/// and joins it to the city through the doorway. That matters twice: the eight
/// authored inmates stand on real graph inside it (M5b), and an escort can walk
/// somebody in. It is therefore deliberately *not* an entry in
/// `lore/places/ombreval_buildings.json`'s `buildings` array, which would render
/// a solid block; only its `named_place_index` anchor is authored there, which
/// is what gives it the `pl_` id `custody::stone_house` resolves.
///
/// The door has no leaf. Ede Clove's authored goal is *"Replace a broken stone
/// house lock"*, so the lock is broken in the shipped world state — which is
/// both the lore and the requirement, since a solid leaf would cut the interior
/// out of the walkable main component and strand everyone in it. You are
/// confined by a person here, exactly as at a gate arch; what the Stone House
/// adds is a room worth staying in.
fn build_stone_house(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    // The court behind the tower is open ground from x 30 to about x 60 between
    // z -211 and z -203, and this rectangle fills the middle of it: 3.5 m clear
    // of the nearest building (`omb_f0097`, a rotated parcel to the south-east),
    // 1.7 m off the tower's own south face, and leaving the court walkable down
    // both flanks — x 30-39 toward Step Cistern, x 50-60 toward the yard — so
    // nothing that used to be a way through stops being one.
    //
    // It does stand on part of `harne_road`'s nominal carriageway: that road is
    // one long diagonal from (25.8, -174) to (56, -252), and its centreline
    // clips the west wall around z -208. That is normal in this plan rather than
    // a mistake — `scripts/bake_navigation.py` says so in as many words ("a road
    // centreline is a schematic hint, not gospel: several cut straight through a
    // solid building"), validates every graph edge against the walkable bitset
    // and re-routes the blocked ones with a windowed A*. The rebake did exactly
    // that here: the street graph came back as one component, and the court's two
    // flanks are what carry the traffic past.
    let (x0, x1) = (39.0_f32, 50.0_f32);
    let (z0, z1) = (-211.2_f32, -203.2_f32);
    let thickness = 0.7_f32;
    let wall_top = 4.4_f32;
    // The doorway, in the wall that faces the court and the way round to the
    // Bellstand square. 1.6 m wide, so the agent-radius erosion still leaves
    // 0.9 m of walkable throat and the room joins the main component.
    let (door_z0, door_z1) = (-207.6_f32, -206.0_f32);
    let door_top = 2.3_f32;

    let mut wall = |a: Vec3, b: Vec3, name: &str| {
        let center = (a + b) * 0.5;
        let size = (b - a).abs();
        spawn_box_named(commands, meshes, &materials.fieldstone, center, size, name);
        collision_world.add_box(a.min(b), a.max(b));
    };

    // The two long walls, and the short one at the back.
    wall(
        Vec3::new(x1 - thickness, 0.0, z0),
        Vec3::new(x1, wall_top, z1),
        "Stone House wall",
    );
    wall(
        Vec3::new(x0 + thickness, 0.0, z0),
        Vec3::new(x1 - thickness, wall_top, z0 + thickness),
        "Stone House wall",
    );
    wall(
        Vec3::new(x0 + thickness, 0.0, z1 - thickness),
        Vec3::new(x1 - thickness, wall_top, z1),
        "Stone House wall",
    );
    // The court wall, in two pieces around the door.
    wall(
        Vec3::new(x0, 0.0, z0),
        Vec3::new(x0 + thickness, wall_top, door_z0),
        "Stone House wall",
    );
    wall(
        Vec3::new(x0, 0.0, door_z1),
        Vec3::new(x0 + thickness, wall_top, z1),
        "Stone House wall",
    );
    // The lintel over the doorway sits above head height, so it is absent from
    // the walk-band export and the throat below it stays open.
    wall(
        Vec3::new(x0, door_top, door_z0),
        Vec3::new(x0 + thickness, wall_top, door_z1),
        "Stone House lintel",
    );

    // Trodden flags. Visual only: a floor collider inside the walk band would
    // be exported as a footprint and would blot the whole room off the graph.
    spawn_box_named(
        commands,
        meshes,
        &materials.paving,
        Vec3::new(
            (x0 + x1) * 0.5,
            0.02,
            (z0 + z1) * 0.5,
        ),
        Vec3::new(x1 - x0 - thickness * 2.0, 0.08, z1 - z0 - thickness * 2.0),
        "Stone House floor",
    );

    // A shallow slate roof on a limestone eaves band, both clear of the walk
    // band and therefore invisible to the nav bake.
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        Vec3::new((x0 + x1) * 0.5, wall_top + 0.2, (z0 + z1) * 0.5),
        Vec3::new(x1 - x0 + 0.5, 0.4, z1 - z0 + 0.5),
        "Stone House eaves",
    );
    collision_world.add_box(
        Vec3::new(x0 - 0.25, wall_top, z0 - 0.25),
        Vec3::new(x1 + 0.25, wall_top + 0.4, z1 + 0.25),
    );
    let ridge = wall_top + 1.9;
    for sz in [-1.0_f32, 1.0] {
        let pitch = 0.42_f32 * sz;
        spawn_mesh_named(
            commands,
            &meshes.cube,
            &materials.slate,
            Transform::from_translation(Vec3::new(
                (x0 + x1) * 0.5,
                (wall_top + 0.4 + ridge) * 0.5,
                (z0 + z1) * 0.5 + sz * (z1 - z0) * 0.25,
            ))
            .with_rotation(Quat::from_rotation_x(pitch))
            .with_scale(Vec3::new(x1 - x0 + 0.6, 0.24, (z1 - z0) * 0.58)),
            "Stone House roof",
        );
    }

    // The barred grate beside the door — where kin stand to pass in bread and a
    // blanket, and M5d's visitors talk through. The wall behind it is one
    // unbroken collider, so it is a window to look through and not a way out.
    // Everything here has to sit **proud of the court face** (x = x0): the wall
    // is one solid box from x0 to x0 + thickness, so anything at or inside that
    // face is buried in masonry and simply never renders.
    let grate_z = -204.8_f32;
    spawn_box_named(
        commands,
        meshes,
        &materials.window_room,
        Vec3::new(x0 - 0.02, 1.7, grate_z),
        Vec3::new(0.1, 1.0, 1.1),
        "Stone House grate recess",
    );
    for offset in [-0.44_f32, -0.22, 0.0, 0.22, 0.44] {
        spawn_box_named(
            commands,
            meshes,
            &materials.iron,
            Vec3::new(x0 - 0.1, 1.7, grate_z + offset),
            Vec3::new(0.1, 1.05, 0.07),
            "Stone House grate bar",
        );
    }

    // The door itself: a heavy oak leaf standing open against the jamb, and the
    // hasp Ede Clove has been meaning to replace all year.
    // Thin across the wall and wide along it, standing open flat against the
    // jamb on the court side — a leaf modelled thin along z would lie *in* the
    // doorway, and one at x0 would be inside the wall.
    spawn_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        Vec3::new(x0 - 0.09, door_top * 0.5, door_z0 - 0.82),
        Vec3::new(0.16, door_top - 0.1, 1.5),
        "Stone House door",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        Vec3::new(x0 - 0.12, 1.15, door_z1 + 0.06),
        Vec3::new(0.12, 0.16, 0.34),
        "Stone House broken hasp",
    );
    // A lantern that burns whatever the hour, as at the Bellfoot.
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        Vec3::new(x0 - 0.18, 2.75, door_z1 + 0.4),
        Vec3::new(0.5, 0.08, 0.08),
        "Stone House lantern bracket",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.lantern_glass,
        Vec3::new(x0 - 0.42, 2.55, door_z1 + 0.4),
        Vec3::new(0.26, 0.34, 0.26),
        "Stone House lantern",
    );

    // **The keeper's lamp, inside.** The lore is explicit that a prisoner is
    // given nothing — no bedding, no rations, *no candle*, because families
    // bring those — so nothing here belongs to the people held in it. This is
    // the keeper's own light at her threshold, and it is the difference between
    // a room and a hole: the whole of M5's argument is that the gaol is the
    // densest social scene in the game, and eight faces you cannot see are not
    // a scene. Warm, low and short-ranged, so the corners stay dark.
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        Vec3::new(x0 + thickness + 0.24, 2.5, door_z1 + 0.5),
        Vec3::new(0.5, 0.07, 0.07),
        "Stone House keeper's lamp bracket",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.lantern_glass,
        Vec3::new(x0 + thickness + 0.46, 2.32, door_z1 + 0.5),
        Vec3::new(0.24, 0.3, 0.24),
        "Stone House keeper's lamp",
    );
    commands.spawn((
        Name::new("Stone House keeper's lamp glow"),
        PointLight {
            color: Color::srgb(1.0, 0.66, 0.32),
            intensity: 26_000.0,
            range: 13.0,
            radius: 0.12,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(x0 + thickness + 0.5, 2.3, door_z1 + 0.5),
    ));
}

/// A cast bronze bell assembled from primitives, mouth down, hung from a
/// headstock beam. `scale` 1.0 is the great Bellstand bell (~2.6 m mouth).
fn spawn_bell(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    center: Vec3,
    scale: f32,
    name: &str,
) {
    // Headstock the bell swings from.
    spawn_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        center + Vec3::Y * 2.6 * scale,
        Vec3::new(5.2 * scale, 0.5 * scale, 0.55 * scale),
        format!("{name} headstock"),
    );
    // Crown, shoulder, waist, flare, lip — the classic profile, coarsely.
    spawn_mesh_named(
        commands,
        &meshes.sphere,
        &materials.bronze,
        Transform::from_translation(center + Vec3::Y * 2.15 * scale)
            .with_scale(Vec3::splat(0.5 * scale)),
        format!("{name} crown"),
    );
    spawn_mesh_named(
        commands,
        &meshes.sphere,
        &materials.bronze,
        Transform::from_translation(center + Vec3::Y * 1.55 * scale).with_scale(Vec3::new(
            1.02 * scale,
            0.85 * scale,
            1.02 * scale,
        )),
        format!("{name} shoulder"),
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        center + Vec3::Y * 0.85 * scale,
        0.95 * scale,
        1.5 * scale,
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        center + Vec3::Y * 0.18 * scale,
        1.22 * scale,
        0.45 * scale,
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        center + Vec3::Y * 0.02 * scale,
        1.3 * scale,
        0.16 * scale,
    );
    // Clapper, just visible under the lip.
    spawn_cylinder(
        commands,
        meshes,
        &materials.iron,
        center + Vec3::Y * 0.55 * scale,
        0.09 * scale,
        1.4 * scale,
    );
    spawn_mesh_named(
        commands,
        &meshes.sphere,
        &materials.iron,
        Transform::from_translation(center - Vec3::Y * 0.18 * scale)
            .with_scale(Vec3::splat(0.22 * scale)),
        format!("{name} clapper"),
    );
}

/// The open bell stage crowning the Bellstand tower: piers, parapet,
/// entablature, corner pinnacles, the great bell, and a slate spire — the
/// silhouette `the_bellstand_001.png` promises. The stage floor is walkable.
fn build_bellstand_belfry(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let center = Vec2::new(44.8, -189.0);
    let (half_x, half_z) = (11.0, 12.5);
    let floor_y = 23.5;
    let stage_top = 31.2;

    // Stage floor caps the shaft; you can land and stand on it.
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        Vec3::new(center.x, floor_y + 0.25, center.y),
        Vec3::new(half_x * 2.0, 0.5, half_z * 2.0),
        "Bellstand stage floor",
    );
    collision_world.add_box(
        Vec3::new(center.x - half_x, floor_y, center.y - half_z),
        Vec3::new(center.x + half_x, floor_y + 0.5, center.y + half_z),
    );

    // Corner and mid-face piers carry the entablature.
    let pier_height = stage_top - floor_y;
    let pier_y = floor_y + pier_height * 0.5;
    let corner_inset = 1.1;
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            let position = Vec3::new(
                center.x + sx * (half_x - corner_inset),
                pier_y,
                center.y + sz * (half_z - corner_inset),
            );
            spawn_box_named(
                commands,
                meshes,
                &materials.limestone,
                position,
                Vec3::new(1.8, pier_height, 1.8),
                "Bellstand corner pier",
            );
            collision_world.add_box(
                position - Vec3::new(0.9, pier_height * 0.5, 0.9),
                position + Vec3::new(0.9, pier_height * 0.5, 0.9),
            );
        }
    }
    for sz in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x, pier_y, center.y + sz * (half_z - 0.8)),
            Vec3::new(1.5, pier_height, 1.6),
            "Bellstand mid pier",
        );
    }
    for sx in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x + sx * (half_x - 0.8), pier_y, center.y),
            Vec3::new(1.6, pier_height, 1.5),
            "Bellstand mid pier",
        );
    }

    // Waist-high parapet between the piers.
    for sz in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x, floor_y + 1.0, center.y + sz * (half_z - 0.55)),
            Vec3::new(half_x * 2.0 - 1.4, 1.1, 0.5),
            "Bellstand parapet",
        );
    }
    for sx in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x + sx * (half_x - 0.55), floor_y + 1.0, center.y),
            Vec3::new(0.5, 1.1, half_z * 2.0 - 1.4),
            "Bellstand parapet",
        );
    }
    collision_world.add_box(
        Vec3::new(center.x - half_x, floor_y, center.y - half_z),
        Vec3::new(center.x + half_x, floor_y + 1.55, center.y - half_z + 0.6),
    );
    collision_world.add_box(
        Vec3::new(center.x - half_x, floor_y, center.y + half_z - 0.6),
        Vec3::new(center.x + half_x, floor_y + 1.55, center.y + half_z),
    );
    collision_world.add_box(
        Vec3::new(center.x - half_x, floor_y, center.y - half_z),
        Vec3::new(center.x - half_x + 0.6, floor_y + 1.55, center.y + half_z),
    );
    collision_world.add_box(
        Vec3::new(center.x + half_x - 0.6, floor_y, center.y - half_z),
        Vec3::new(center.x + half_x, floor_y + 1.55, center.y + half_z),
    );

    // Entablature ring above the arcade.
    for sz in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x, stage_top + 0.5, center.y + sz * (half_z - 0.7)),
            Vec3::new(half_x * 2.0 + 0.7, 1.0, 1.7),
            "Bellstand entablature",
        );
    }
    for sx in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x + sx * (half_x - 0.7), stage_top + 0.5, center.y),
            Vec3::new(1.7, 1.0, half_z * 2.0 + 0.7),
            "Bellstand entablature",
        );
    }
    // Corner pinnacles for the skyline.
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            spawn_mesh_named(
                commands,
                &meshes.pyramid,
                &materials.slate,
                Transform::from_xyz(
                    center.x + sx * (half_x - corner_inset),
                    stage_top + 1.9,
                    center.y + sz * (half_z - corner_inset),
                )
                .with_scale(Vec3::new(1.5, 1.8, 1.5)),
                "Bellstand pinnacle",
            );
        }
    }

    // The great bell, hung from the middle of the stage.
    spawn_bell(
        commands,
        meshes,
        materials,
        Vec3::new(center.x, 26.4, center.y),
        1.0,
        "The Bellstand watch-bell",
    );

    // Slate spire and finial.
    spawn_mesh_named(
        commands,
        &meshes.pyramid,
        &materials.slate,
        Transform::from_xyz(center.x, stage_top + 1.0 + 4.5, center.y)
            .with_scale(Vec3::new(15.5, 9.0, 17.5)),
        "Bellstand spire",
    );
    collision_world.add_box(
        Vec3::new(center.x - 7.0, stage_top + 1.0, center.y - 8.0),
        Vec3::new(center.x + 7.0, stage_top + 8.0, center.y + 8.0),
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        Vec3::new(center.x, stage_top + 10.6, center.y),
        0.07,
        2.6,
    );
    spawn_mesh_named(
        commands,
        &meshes.sphere,
        &materials.bronze,
        Transform::from_xyz(center.x, stage_top + 12.0, center.y).with_scale(Vec3::splat(0.38)),
        "Bellstand finial",
    );
}

/// Bellfoot Passage — the covered way at the foot of the Bellstand tower's
/// external stair, after `bellfoot_passage_001.png`. A stone porch projects
/// north from the tower's civic masonry into the bright Bellstand square: solid
/// side walls, a boarded soffit slung on joists, posted notices, and lanterns
/// that burn day and night. The great stair rides over the east flank on a low
/// spandrel and climbs the tower face to an upper watch door; heavy oak bracing
/// frames the mouth, and the square glows through it.
fn build_bellfoot_passage(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    city_meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    // --- Frame ------------------------------------------------------------
    // The tower's north face; the porch projects from it into the square.
    let face_z = -176.5_f32;
    let mouth_z = -163.5_f32;
    let x_w = 34.8_f32; // west wall centreline
    let x_e = 40.3_f32; // east (spandrel) wall centreline
    let wall_th = 0.7_f32;
    let in_w = x_w + wall_th * 0.5; // interior west face
    let in_e = x_e - wall_th * 0.5; // interior east face
    let ceil_y = 3.15_f32; // boarded soffit underside
    let wall_top = 3.95_f32;
    let spandrel_top = 2.45_f32; // the east wall the stair springs from
    let board_e = 38.5_f32; // boards stop short so the stair underside shows

    // One batch per material, committed at the end.
    let mut stone = MeshData::default();
    let mut boards = MeshData::default();
    let mut iron = MeshData::default();
    let mut glass = MeshData::default();
    let mut notices = MeshData::default();

    // Axis-aligned box between two opposite corners, appended to `mesh`.
    fn ab(mesh: &mut MeshData, x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32) {
        add_oriented_box(
            mesh,
            Vec3::new((x0 + x1) * 0.5, (y0 + y1) * 0.5, (z0 + z1) * 0.5),
            Vec3::new(
                (x1 - x0).abs() * 0.5,
                (y1 - y0).abs() * 0.5,
                (z1 - z0).abs() * 0.5,
            ),
            Vec2::X,
        );
    }

    // --- Worn flag floor and doorsteps -----------------------------------
    // Kept dark and grimy so it reads as trodden flags in shade, not a fresh
    // slab dropped on the square.
    stone.set_brush([0.58, 0.56, 0.53]);
    ab(&mut stone, in_w, in_e, -0.03, 0.05, face_z, mouth_z);
    ab(
        &mut stone,
        in_w + 0.2,
        in_e - 0.2,
        0.0,
        0.12,
        mouth_z - 0.55,
        mouth_z + 0.02,
    );
    ab(
        &mut stone,
        in_w + 0.3,
        in_e - 0.3,
        0.0,
        0.14,
        face_z - 0.02,
        face_z + 0.55,
    );
    stone.reset_brush();

    // --- Side walls -------------------------------------------------------
    // West: full-height solid stone, carrying the posted notices.
    stone.set_brush([0.86, 0.84, 0.80]);
    ab(
        &mut stone,
        x_w - wall_th * 0.5,
        x_w + wall_th * 0.5,
        0.0,
        wall_top,
        face_z,
        mouth_z,
    );
    collision_world.add_box(
        Vec3::new(x_w - wall_th * 0.5, 0.0, face_z),
        Vec3::new(x_w + wall_th * 0.5, wall_top, mouth_z),
    );
    // East: a low spandrel the stair springs from; open above so the square's
    // light spills in beneath the rising steps.
    ab(
        &mut stone,
        x_e - wall_th * 0.5,
        x_e + wall_th * 0.5,
        0.0,
        spandrel_top,
        face_z,
        mouth_z,
    );
    collision_world.add_box(
        Vec3::new(x_e - wall_th * 0.5, 0.0, face_z),
        Vec3::new(x_e + wall_th * 0.5, spandrel_top, mouth_z),
    );
    stone.reset_brush();

    // --- Boarded soffit over the west bay --------------------------------
    let run = mouth_z - face_z;
    boards.set_brush([0.42, 0.36, 0.30]);
    ab(
        &mut boards,
        in_w,
        board_e,
        ceil_y,
        ceil_y + 0.09,
        face_z,
        mouth_z,
    );
    for i in 1..7 {
        let z = face_z + run * (i as f32 / 7.0);
        ab(
            &mut boards,
            in_w,
            board_e + 0.06,
            ceil_y - 0.14,
            ceil_y - 0.01,
            z - 0.06,
            z + 0.06,
        );
    }
    boards.reset_brush();
    // Stone rib where the boards meet the open stair strip.
    stone.set_brush([0.80, 0.78, 0.73]);
    ab(
        &mut stone,
        board_e - 0.05,
        board_e + 0.22,
        ceil_y - 0.06,
        ceil_y + 0.34,
        face_z,
        mouth_z,
    );
    stone.reset_brush();

    // --- Mouth: oak posts, lintel, knee braces, stone header -------------
    let post_half = 0.20_f32;
    for px in [x_w, x_e] {
        spawn_box_named(
            commands,
            city_meshes,
            &materials.timber,
            Vec3::new(px, (wall_top - 0.1) * 0.5, mouth_z),
            Vec3::new(post_half * 2.0, wall_top - 0.1, post_half * 2.0),
            "Bellfoot mouth post",
        );
        collision_world.add_box(
            Vec3::new(px - post_half, 0.0, mouth_z - post_half),
            Vec3::new(px + post_half, wall_top - 0.1, mouth_z + post_half),
        );
    }
    spawn_box_named(
        commands,
        city_meshes,
        &materials.timber,
        Vec3::new((x_w + x_e) * 0.5, wall_top - 0.32, mouth_z),
        Vec3::new(x_e - x_w + post_half * 2.0, 0.44, 0.4),
        "Bellfoot mouth lintel",
    );
    // Knee braces from each post up to the lintel, in the plane of the mouth.
    for (px, sign) in [(x_w, 1.0_f32), (x_e, -1.0)] {
        let brace_len = 1.25_f32;
        let dir = Vec3::new(
            sign * std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
        );
        let center =
            Vec3::new(px + sign * 0.42, wall_top - 0.95, mouth_z) + dir * (brace_len * 0.5);
        spawn_mesh_named(
            commands,
            &city_meshes.cube,
            &materials.timber,
            Transform::from_translation(center)
                .with_rotation(Quat::from_rotation_arc(Vec3::X, dir))
                .with_scale(Vec3::new(brace_len, 0.16, 0.16)),
            "Bellfoot mouth brace",
        );
    }
    // Stone header/parapet over the mouth, carried on the walls.
    stone.set_brush([0.84, 0.82, 0.77]);
    ab(
        &mut stone,
        x_w - wall_th * 0.5,
        x_e + wall_th * 0.5,
        wall_top - 0.15,
        wall_top + 0.55,
        mouth_z - 0.2,
        mouth_z + 0.18,
    );
    stone.reset_brush();

    // Heavy oak tie-beams span the whole passage — the reference's thick
    // timber bracing, and what the soffit boards hang from.
    for z in [-166.6_f32, -171.4] {
        spawn_box_named(
            commands,
            city_meshes,
            &materials.timber,
            Vec3::new((in_w + in_e) * 0.5, ceil_y - 0.14, z),
            Vec3::new(in_e - in_w + 0.2, 0.26, 0.3),
            "Bellfoot tie-beam",
        );
    }

    // --- The great external stair over the east flank --------------------
    // A solid stone flight: filled step blocks between stone stringer parapets,
    // climbing the tower face to the watch door. Never floating treads over a
    // plank — the earlier build read as a hollow wooden ramp.
    let x_stair = 40.5_f32;
    let stair_half = 1.55_f32;
    let foot_z = -158.0_f32;
    let foot_y = 0.1_f32;
    let n_steps = 42_usize;
    let going = 0.42_f32;
    let rise = 0.205_f32;
    let tread_z = |i: f32| foot_z - going * i; // north edge of tread i
    let tread_y = |i: f32| foot_y + rise * i;
    let top_z = tread_z(n_steps as f32);
    let top_y = tread_y(n_steps as f32);
    let span = Vec3::new(0.0, top_y - foot_y, top_z - foot_z);
    let mid = Vec3::new(x_stair, (foot_y + top_y) * 0.5, (foot_z + top_z) * 0.5);
    let rake = Quat::from_rotation_arc(Vec3::NEG_Z, span.normalize());
    let flight_len = span.length();

    // Broad bottom step where the flight lands in the square.
    stone.set_brush([0.78, 0.76, 0.72]);
    ab(
        &mut stone,
        x_stair - stair_half - 0.45,
        x_stair + stair_half + 0.45,
        -0.12,
        foot_y + 0.04,
        foot_z - 0.15,
        foot_z + 0.85,
    );
    // Filled steps: each block reaches well below its own tread, so the flight
    // is a solid mass with a corbelled stone soffit — no hollow underside.
    stone.set_brush([0.83, 0.81, 0.77]);
    for i in 0..n_steps {
        let zc = tread_z(i as f32);
        let yc = tread_y(i as f32);
        ab(
            &mut stone,
            x_stair - stair_half,
            x_stair + stair_half,
            yc - 0.66,
            yc,
            zc - going - 0.03,
            zc + 0.03,
        );
        collision_world.add_box(
            Vec3::new(x_stair - stair_half, yc - 0.3, zc - going - 0.03),
            Vec3::new(x_stair + stair_half, yc, zc + 0.03),
        );
    }
    stone.reset_brush();

    // Solid stone stringer parapets flanking the flight — the guard, and what
    // hides the step ends.
    for s in [-1.0_f32, 1.0] {
        spawn_mesh_named(
            commands,
            &city_meshes.cube,
            &materials.limestone,
            Transform::from_translation(mid + Vec3::X * s * (stair_half + 0.18) + Vec3::Y * 0.26)
                .with_rotation(rake)
                .with_scale(Vec3::new(0.34, 1.25, flight_len + 0.2)),
            "Bellfoot stair stringer",
        );
    }

    // Landing against the tower face, and its collider.
    stone.set_brush([0.83, 0.81, 0.77]);
    ab(
        &mut stone,
        x_stair - stair_half,
        x_stair + stair_half,
        top_y - 0.75,
        top_y,
        face_z,
        top_z + 0.1,
    );
    stone.reset_brush();
    collision_world.add_box(
        Vec3::new(x_stair - stair_half, top_y - 0.3, face_z),
        Vec3::new(x_stair + stair_half, top_y, top_z + 0.1),
    );

    // Studded oak watch door set into the tower face above the landing.
    boards.set_brush([0.24, 0.18, 0.13]);
    ab(
        &mut boards,
        x_stair - 0.62,
        x_stair + 0.62,
        top_y + 0.05,
        top_y + 2.15,
        face_z - 0.18,
        face_z - 0.02,
    );
    boards.reset_brush();
    for dz in [-0.35_f32, 0.0, 0.35] {
        for dy in [0.5_f32, 1.1, 1.7] {
            ab(
                &mut iron,
                x_stair + dz - 0.04,
                x_stair + dz + 0.04,
                top_y + dy - 0.04,
                top_y + dy + 0.04,
                face_z - 0.22,
                face_z - 0.16,
            );
        }
    }

    // Rear porch door + a shallow relieving lintel, at ground under the stair.
    boards.set_brush([0.24, 0.18, 0.13]);
    ab(
        &mut boards,
        (in_w + board_e) * 0.5 - 0.6,
        (in_w + board_e) * 0.5 + 0.6,
        0.05,
        2.15,
        face_z + 0.02,
        face_z + 0.16,
    );
    boards.reset_brush();

    // --- Posted notices: a wall papered with unreadable bills ------------
    // Thin proud boards, not flat decals, so they catch the lantern light on
    // more than one face and read as posted paper.
    notices.set_brush([1.0, 0.96, 0.86]);
    let mut tag = 0u32;
    for col in 0..7 {
        for row in 0..2 {
            tag += 1;
            let h = stable_hash(&format!("bellfoot_west_notice_{tag}"));
            let z = -165.3 - col as f32 * 1.12 - (h % 22) as f32 / 100.0;
            let cy = 1.12 + row as f32 * 0.82 + ((h >> 5) % 26) as f32 / 100.0;
            let hw = 0.17 + (h % 12) as f32 / 100.0;
            let hh = 0.22 + ((h >> 3) % 20) as f32 / 100.0;
            ab(
                &mut notices,
                in_w + 0.01,
                in_w + 0.06,
                cy - hh,
                cy + hh,
                z - hw,
                z + hw,
            );
        }
    }
    for col in 0..4 {
        let h = stable_hash(&format!("bellfoot_east_notice_{col}"));
        let z = -166.6 - col as f32 * 1.55;
        let cy = 1.2 + (h % 40) as f32 / 100.0;
        let hw = 0.19 + (h % 10) as f32 / 200.0;
        ab(
            &mut notices,
            in_e - 0.06,
            in_e - 0.01,
            cy - 0.26,
            cy + 0.26,
            z - hw,
            z + hw,
        );
    }
    notices.reset_brush();

    // --- Lanterns that burn day and night --------------------------------
    let porch_mid_x = (in_w + board_e) * 0.5;
    for z in [-166.8_f32, -172.2] {
        let head_y = ceil_y - 0.75;
        ab(
            &mut iron,
            porch_mid_x - 0.02,
            porch_mid_x + 0.02,
            head_y + 0.13,
            ceil_y,
            z - 0.02,
            z + 0.02,
        );
        ab(
            &mut iron,
            porch_mid_x - 0.1,
            porch_mid_x + 0.1,
            head_y + 0.11,
            head_y + 0.16,
            z - 0.1,
            z + 0.1,
        );
        ab(
            &mut iron,
            porch_mid_x - 0.085,
            porch_mid_x + 0.085,
            head_y - 0.16,
            head_y - 0.11,
            z - 0.085,
            z + 0.085,
        );
        glass.set_brush([1.0, 0.85, 0.55]);
        ab(
            &mut glass,
            porch_mid_x - 0.07,
            porch_mid_x + 0.07,
            head_y - 0.11,
            head_y + 0.11,
            z - 0.07,
            z + 0.07,
        );
        glass.reset_brush();
        commands.spawn((
            Name::new("Bellfoot passage lantern"),
            PointLight {
                color: Color::srgb(1.0, 0.62, 0.28),
                intensity: 22_000.0,
                range: 12.0,
                radius: 0.1,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(porch_mid_x, head_y - 0.2, z),
        ));
    }
    // Iron wall bracket + lantern by the mouth (the reference's hanging lamp).
    let wl_x = in_w + 0.05;
    let wl_z = -164.7_f32;
    let wl_y = 2.65_f32;
    ab(
        &mut iron,
        in_w,
        wl_x + 0.5,
        wl_y + 0.28,
        wl_y + 0.34,
        wl_z - 0.03,
        wl_z + 0.03,
    );
    ab(
        &mut iron,
        wl_x + 0.5,
        wl_x + 0.56,
        wl_y - 0.06,
        wl_y + 0.34,
        wl_z - 0.03,
        wl_z + 0.03,
    );
    glass.set_brush([1.0, 0.85, 0.55]);
    ab(
        &mut glass,
        wl_x + 0.44,
        wl_x + 0.62,
        wl_y - 0.06,
        wl_y + 0.18,
        wl_z - 0.09,
        wl_z + 0.09,
    );
    glass.reset_brush();
    commands.spawn((
        Name::new("Bellfoot mouth lantern"),
        PointLight {
            color: Color::srgb(1.0, 0.62, 0.28),
            intensity: 18_000.0,
            range: 10.0,
            radius: 0.1,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(wl_x + 0.53, wl_y + 0.06, wl_z),
    ));

    // --- A little market clutter, inhabiting the shade -------------------
    for (bx, bz, r, hgt) in [
        (in_w + 0.55, -170.0_f32, 0.34_f32, 0.42_f32),
        (in_w + 0.5, -171.1, 0.30, 0.36),
    ] {
        spawn_cylinder(
            commands,
            city_meshes,
            &materials.canvas,
            Vec3::new(bx, hgt * 0.5, bz),
            r,
            hgt,
        );
    }
    spawn_box_named(
        commands,
        city_meshes,
        &materials.timber,
        Vec3::new(in_w + 0.55, 0.32, -168.6),
        Vec3::new(0.7, 0.64, 0.5),
        "Bellfoot crate",
    );

    // --- Tower-base plinth: heavy civic masonry on the north face --------
    stone.set_brush([0.80, 0.78, 0.73]);
    ab(&mut stone, 33.4, 56.2, 0.0, 1.2, face_z, face_z + 0.5);
    stone.reset_brush();

    // --- Commit the batches ----------------------------------------------
    spawn_batch(
        commands,
        meshes,
        &materials.limestone,
        stone,
        "Bellfoot masonry",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        boards,
        "Bellfoot boards",
    );
    spawn_batch(commands, meshes, &materials.iron, iron, "Bellfoot ironwork");
    spawn_batch(
        commands,
        meshes,
        &materials.lantern_glass,
        glass,
        "Bellfoot lantern panes",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_ochre,
        notices,
        "Bellfoot notices",
    );
}

fn build_saint_maren_tower(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let center = Vec3::new(-158.5, 8.5, -285.6);
    spawn_box_named(
        commands,
        meshes,
        &materials.fieldstone,
        center,
        Vec3::new(8.5, 17.0, 8.5),
        "Saint Maren's modest bell tower",
    );
    add_open_bell_stage(
        commands,
        meshes,
        materials,
        Vec2::new(center.x, center.z),
        17.0,
        4.25,
        0.62,
        "Saint Maren's",
    );
    collision_world.add_box(
        Vec3::new(center.x - 4.25, 0.0, center.z - 4.25),
        Vec3::new(center.x + 4.25, 24.5, center.z + 4.25),
    );
}

/// A small open lantern for the parish landmarks: corner posts, a visible
/// swinging bell, and the slate pyramid lifted back on top.
#[allow(clippy::too_many_arguments)]
fn add_open_bell_stage(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    center: Vec2,
    base_y: f32,
    half_width: f32,
    bell_scale: f32,
    name: &str,
) {
    let stage_height = 3.4;
    let post_inset = 0.55;
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            spawn_box_named(
                commands,
                meshes,
                &materials.fieldstone,
                Vec3::new(
                    center.x + sx * (half_width - post_inset),
                    base_y + stage_height * 0.5,
                    center.y + sz * (half_width - post_inset),
                ),
                Vec3::new(0.9, stage_height, 0.9),
                format!("{name} bell-stage post"),
            );
        }
    }
    // Low rail between the posts.
    for sz in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.fieldstone,
            Vec3::new(center.x, base_y + 0.45, center.y + sz * (half_width - 0.35)),
            Vec3::new(half_width * 2.0 - 1.0, 0.9, 0.35),
            format!("{name} bell-stage rail"),
        );
    }
    for sx in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.fieldstone,
            Vec3::new(center.x + sx * (half_width - 0.35), base_y + 0.45, center.y),
            Vec3::new(0.35, 0.9, half_width * 2.0 - 1.0),
            format!("{name} bell-stage rail"),
        );
    }
    spawn_bell(
        commands,
        meshes,
        materials,
        Vec3::new(center.x, base_y + 0.9, center.y),
        bell_scale,
        name,
    );
    spawn_mesh_named(
        commands,
        &meshes.pyramid,
        &materials.slate,
        Transform::from_xyz(center.x, base_y + stage_height + 1.9, center.y).with_scale(Vec3::new(
            half_width * 2.0 - 0.6,
            4.0,
            half_width * 2.0 - 0.6,
        )),
        format!("{name} tower roof"),
    );
}

fn build_parish_towers(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    for building in plan
        .buildings
        .iter()
        .filter(|building| building.id.starts_with("reserve_church_"))
    {
        let center2 = polygon_center(&building.polygon);
        let center = Vec3::new(center2.x, 6.7, center2.y);
        let name = building.name.as_deref().unwrap_or(&building.id);
        spawn_box_named(
            commands,
            meshes,
            &materials.fieldstone,
            center,
            Vec3::new(6.5, 13.4, 6.5),
            format!("{name} parish tower reserve"),
        );
        add_open_bell_stage(
            commands,
            meshes,
            materials,
            Vec2::new(center.x, center.z),
            13.4,
            3.25,
            0.5,
            name,
        );
        collision_world.add_box(
            center + Vec3::new(-3.25, -6.7, -3.25),
            center + Vec3::new(3.25, 13.5, 3.25),
        );
    }
}

fn build_old_sluice_face(commands: &mut Commands, meshes: &CityMeshes, materials: &CityMaterials) {
    let face_z = -405.86;
    for x in [-226.5, -200.5] {
        spawn_box_named(
            commands,
            meshes,
            &materials.iron,
            Vec3::new(x, 3.4, face_z),
            Vec3::new(16.0, 6.2, 0.18),
            "Blocked dry arch of the Old Sluice",
        );
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(x, 7.0, face_z + 0.1),
            Vec3::new(18.0, 1.0, 0.75),
            "Old Sluice arch lintel",
        );
    }
    for x in [-237.5, -213.5, -189.5] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(x, 3.5, face_z + 0.1),
            Vec3::new(2.0, 7.0, 0.8),
            "Old Sluice arch pier",
        );
    }
}

fn build_charnel_and_ilvane_details(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
) {
    spawn_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        Vec3::new(-189.58, 1.35, -248.6),
        Vec3::new(0.16, 2.7, 1.65),
        "Saint Maren's charnel door",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        Vec3::new(-189.7, 2.95, -248.6),
        Vec3::new(0.5, 0.45, 2.4),
        "Saint Maren's worn charnel lintel",
    );

    // The chapel's public openings are visibly mortared; the occupied cell's
    // tiny north-facing squint is the sole living aperture.
    spawn_box_named(
        commands,
        meshes,
        &materials.fieldstone,
        Vec3::new(122.0, 1.7, -38.85),
        Vec3::new(2.6, 3.4, 0.22),
        "Mortared Ilvane Chapel door",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.window,
        Vec3::new(145.86, 2.0, -64.4),
        Vec3::new(0.12, 0.65, 0.45),
        "Ilvane anchorhold north squint",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.timber,
        Vec3::new(146.05, 1.05, -64.4),
        Vec3::new(0.55, 0.15, 1.2),
        "Ilvane anchorhold alms shelf",
    );
}

/// The spine pier standing in every bridge mouth, in plan: `WIDTH` across the
/// mouth (the flanks the two half-mouth arches spring off) and `DEPTH` back
/// into the passage. The depth is the shell's to give — a mouth 27 m wide is
/// still only a mouth, and a pier sized from it would be a wall laid across the
/// road it is meant to leave open.
const BRIDGE_PIER_WIDTH: f32 = 1.25;
const BRIDGE_PIER_DEPTH: f32 = 1.25;

fn build_bridge_supports(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    for building in plan
        .buildings
        .iter()
        .filter(|building| building.use_name == "bridge" || building.id == "named_malt_house")
    {
        if building.polygon.len() != 4 {
            continue;
        }
        let p = building
            .polygon
            .iter()
            .map(|point| Vec2::from_array(*point))
            .collect::<Vec<_>>();
        let edge_01 = p[0].distance(p[1]);
        let edge_12 = p[1].distance(p[2]);
        // The mouths are the two short edges — the same reading the passage
        // dressing and the mouth arches stand on.
        let ends = if edge_01 >= edge_12 {
            [(p[0] + p[3]) * 0.5, (p[1] + p[2]) * 0.5]
        } else {
            [(p[0] + p[1]) * 0.5, (p[2] + p[3]) * 0.5]
        };
        let long = (ends[1] - ends[0]).normalize_or_zero();
        let angle = long.x.atan2(long.y);
        let size = Vec3::new(BRIDGE_PIER_WIDTH, 4.2, BRIDGE_PIER_DEPTH);
        for (end_index, end) in ends.into_iter().enumerate() {
            // Set the pier back half its depth, so its outer face is flush with
            // the mouth rather than half of it standing out in the street: this
            // stone runs from the ground to the shell, straight through the
            // walk band, and everything outside the footprint is road.
            let inward = if end_index == 0 { long } else { -long };
            let footing = end + inward * (BRIDGE_PIER_DEPTH * 0.5);
            let center = Vec3::new(footing.x, 2.1, footing.y);
            spawn_rotated_box_named(
                commands,
                meshes,
                if building.material == "limestone" {
                    &materials.limestone
                } else {
                    &materials.timber
                },
                center,
                size,
                angle,
                format!(
                    "{} support",
                    building.name.as_deref().unwrap_or(&building.id)
                ),
            );
            add_rotated_box_collider_at(collision_world, center, size, angle);
        }
    }
}

/// Dress the underside of every space a road-goer crosses beneath a building —
/// the three bridge upper storeys and the malt-house over Malt Passage — after
/// `bellfoot_passage_001.png`: a boarded ceiling with joists, a fascia over
/// each mouth, hanging lanterns that burn day and night, posted notices on the
/// spine piers, and a worn stone doorstep strip at each end.
fn build_covered_passages(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
) {
    let mut boards = MeshData::default();
    let mut ironwork = MeshData::default();
    let mut glass = MeshData::default();
    let mut notices = MeshData::default();
    let mut steps = MeshData::default();

    for building in plan
        .buildings
        .iter()
        .filter(|building| building.use_name == "bridge" || building.id == "named_malt_house")
    {
        if building.polygon.len() != 4 {
            continue;
        }
        let (base_y, _) = building_verticals(building);
        let p: Vec<Vec2> = building
            .polygon
            .iter()
            .map(|point| Vec2::from_array(*point))
            .collect();
        let edge_01 = p[0].distance(p[1]);
        let edge_12 = p[1].distance(p[2]);
        // The passage runs the long way; the mouths are the two short edges —
        // the same reading `build_bridge_supports` stands its piers on.
        let (ends, width) = if edge_01 >= edge_12 {
            ([(p[0] + p[3]) * 0.5, (p[1] + p[2]) * 0.5], edge_12)
        } else {
            ([(p[0] + p[1]) * 0.5, (p[2] + p[3]) * 0.5], edge_01)
        };
        let long_dir = (ends[1] - ends[0]).normalize_or_zero();
        let across = Vec2::new(-long_dir.y, long_dir.x);
        let run = ends[0].distance(ends[1]);
        let hash = stable_hash(&building.id);

        // Boarded ceiling, dark as the soffits, joists slung across it.
        let ceiling_y = base_y - 0.03;
        let first = boards.positions.len() as u32;
        for corner in &p {
            boards.vertex_shaded(
                Vec3::new(corner.x, ceiling_y, corner.y),
                Vec3::NEG_Y,
                Vec2::new(corner.x / 3.5, corner.y / 3.5),
                0.45,
            );
        }
        boards.indices.extend_from_slice(&[
            first,
            first + 1,
            first + 2,
            first,
            first + 2,
            first + 3,
        ]);
        let joists = ((run - 1.6) / 1.7).floor().max(1.0) as usize;
        for index in 0..joists {
            let t = (index as f32 + 1.0) / (joists as f32 + 1.0);
            let center = ends[0].lerp(ends[1], t);
            add_oriented_box(
                &mut boards,
                Vec3::new(center.x, ceiling_y - 0.07, center.y),
                Vec3::new(width * 0.5 - 0.15, 0.075, 0.09),
                across,
            );
        }

        for (end_index, end) in ends.iter().enumerate() {
            let inward = if end_index == 0 { long_dir } else { -long_dir };
            // Fascia board across the head of the mouth.
            let fascia = *end + inward * 0.12;
            add_oriented_box(
                &mut boards,
                Vec3::new(fascia.x, base_y - 0.19, fascia.y),
                Vec3::new(width * 0.5, 0.20, 0.055),
                across,
            );
            // A worn stone doorstep strip where the covered dark begins.
            add_oriented_box(
                &mut steps,
                Vec3::new(end.x, 0.045, end.y),
                Vec3::new(width * 0.5 - 0.35, 0.045, 0.42),
                across,
            );

            // Posted notices on both faces of the spine pier at this mouth —
            // set back with the pier, and sharing out its depth, since that
            // narrow flank is all the board there is to nail them to.
            let pier_center = *end + inward * (BRIDGE_PIER_DEPTH * 0.5);
            for side in [-1.0, 1.0] {
                let face_normal = across * side;
                let count = 1 + (hash >> (end_index as u32 * 4 + (side as i32 + 1) as u32)) % 3;
                let slot = BRIDGE_PIER_DEPTH / count as f32;
                for notice in 0..count {
                    let notice_hash = hash
                        ^ (end_index as u32 * 41)
                        ^ ((side as i32 + 2) as u32 * 97)
                        ^ notice.wrapping_mul(0x9E37_79B9);
                    let notice_width = (0.28 + (notice_hash % 18) as f32 / 100.0).min(slot - 0.06);
                    let margin = (BRIDGE_PIER_DEPTH - notice_width) * 0.5;
                    let along = ((notice as f32 - (count as f32 - 1.0) * 0.5) * slot
                        + ((notice_hash >> 5) % 9) as f32 / 100.0
                        - 0.04)
                        .clamp(-margin, margin);
                    let spot = pier_center
                        + long_dir * along
                        + face_normal * (BRIDGE_PIER_WIDTH * 0.5 + 0.04);
                    add_facade_panel(
                        &mut notices,
                        spot,
                        1.45 + ((notice_hash >> 7) % 50) as f32 / 100.0,
                        long_dir,
                        Vec3::new(face_normal.x, 0.0, face_normal.y),
                        notice_width,
                        0.36 + ((notice_hash >> 3) % 22) as f32 / 100.0,
                    );
                }
            }
        }

        // Lanterns down the centreline, chained to the boards.
        let lantern_count = if run > 26.0 { 3 } else { 2 };
        for index in 0..lantern_count {
            let t = (index as f32 + 1.0) / (lantern_count as f32 + 1.0);
            let drift = (((hash >> (index * 5)) % 3) as f32 - 1.0) * 0.35;
            let spot = ends[0].lerp(ends[1], t) + across * drift;
            let head_y = base_y - 0.78;
            add_oriented_box(
                &mut ironwork,
                Vec3::new(spot.x, (ceiling_y + head_y + 0.14) * 0.5, spot.y),
                Vec3::new(0.02, (ceiling_y - head_y - 0.14) * 0.5, 0.02),
                long_dir,
            );
            add_oriented_box(
                &mut ironwork,
                Vec3::new(spot.x, head_y + 0.13, spot.y),
                Vec3::new(0.10, 0.025, 0.10),
                long_dir,
            );
            add_oriented_box(
                &mut ironwork,
                Vec3::new(spot.x, head_y - 0.13, spot.y),
                Vec3::new(0.085, 0.025, 0.085),
                long_dir,
            );
            add_oriented_box(
                &mut glass,
                Vec3::new(spot.x, head_y, spot.y),
                Vec3::new(0.065, 0.105, 0.065),
                long_dir,
            );
            commands.spawn((
                Name::new(format!(
                    "Passage lantern: {}",
                    building.name.as_deref().unwrap_or(&building.id)
                )),
                PointLight {
                    color: Color::srgb(1.0, 0.62, 0.28),
                    intensity: 20_000.0,
                    range: 11.0,
                    radius: 0.1,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(spot.x, head_y - 0.25, spot.y),
            ));
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        boards,
        "Passage ceilings and boards",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.iron,
        ironwork,
        "Passage lantern ironwork",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.lantern_glass,
        glass,
        "Passage lantern panes",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_ochre,
        notices,
        "Passage posted notices",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.limestone,
        steps,
        "Passage doorsteps",
    );
}

/// The masonry face of every visible bridge-mouth, after reference image A:
/// each open half of a mouth gets a segmental voussoir ring whose crown tucks
/// just under the shell base and whose springings land on the spine pier and
/// the abutment corner, spandrel infill up to the shell base line, and an
/// impost band where the ring meets the pier. Face dressing only: no
/// colliders, and nothing below the 3.2 m road clearance except the impost
/// hugging the existing pier head.
fn build_bridge_arches(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
) {
    let mut limestone = MeshData::default();
    let mut fieldstone = MeshData::default();

    for (building_index, building) in plan.buildings.iter().enumerate() {
        if !(building.use_name == "bridge" || building.id == "named_malt_house")
            || building.polygon.len() != 4
        {
            continue;
        }
        let (base_y, _) = building_verticals(building);
        let p: Vec<Vec2> = building
            .polygon
            .iter()
            .map(|point| Vec2::from_array(*point))
            .collect();
        let edge_01 = p[0].distance(p[1]);
        let edge_12 = p[1].distance(p[2]);
        // The mouths are the two short edges — the same reading the passage
        // dressing and the bridge piers stand on.
        let (ends, width) = if edge_01 >= edge_12 {
            ([(p[0] + p[3]) * 0.5, (p[1] + p[2]) * 0.5], edge_12)
        } else {
            ([(p[0] + p[1]) * 0.5, (p[2] + p[3]) * 0.5], edge_01)
        };
        let long_dir = (ends[1] - ends[0]).normalize_or_zero();
        let across = Vec2::new(-long_dir.y, long_dir.x);

        // The spine pier `build_bridge_supports` stands in every mouth: each
        // half-mouth arch springs off its flank and the mouth corner.
        let pier_half = BRIDGE_PIER_WIDTH * 0.5;
        let spring_y = 3.2;
        let ring = 0.3;
        // Intrados crown; the ring riding 0.01 + `ring` outside it tucks its
        // extrados 0.09 under the shell base. The crown must stay below the
        // passage fascia's underside (base_y - 0.39) so the board never pokes
        // through the stone soffit.
        let crown_y = base_y - 0.40;
        let inner = pier_half - 0.01;
        let outer = width * 0.5 - 0.05;
        // A segmental circle needs more half-span than rise.
        if outer - inner < 2.0 * (crown_y - spring_y) + 0.2 {
            continue;
        }

        let mesh = if building.material == "limestone" {
            &mut limestone
        } else {
            &mut fieldstone
        };
        mesh.set_brush(building_tint(building));

        for (end_index, end) in ends.iter().enumerate() {
            let outward = if end_index == 0 { -long_dir } else { long_dir };
            let mut dressed = false;
            for side in [-1.0_f32, 1.0] {
                let u_dir = across * side;
                // A half-mouth buried in a neighbour stays undressed: both
                // Tally mouths, the Chain Bridge's north-west half and the
                // Eel Bridge's south end vanish into adjoining buildings.
                let buried = [0.05_f32, 0.25, 0.5, 0.75, 0.95].into_iter().any(|t| {
                    let probe = *end + u_dir * (inner + (outer - inner) * t) + outward * 0.35;
                    plan.buildings.iter().enumerate().any(|(index, other)| {
                        index != building_index && point_in_polygon(probe, &other.polygon)
                    })
                });
                if buried {
                    continue;
                }
                add_mouth_arch(
                    mesh, *end, u_dir, outward, inner, outer, spring_y, crown_y, ring, base_y,
                );
                dressed = true;
            }
            // A modest impost band wrapping the pier head at springing
            // height; it alone may dip under 3.2 m, merged into the pier.
            if dressed {
                add_oriented_box(
                    mesh,
                    Vec3::new(end.x, spring_y - 0.09, end.y),
                    Vec3::new(pier_half + 0.08, 0.09, 0.40),
                    across,
                );
            }
        }
        mesh.reset_brush();
    }

    spawn_batch(
        commands,
        meshes,
        &materials.limestone,
        limestone,
        "Bridge mouth arches (limestone)",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.fieldstone,
        fieldstone,
        "Bridge mouth arches (fieldstone)",
    );
}

/// One segmental arch dressing half a bridge-mouth, in mouth-local
/// coordinates: `u_dir` runs from the mouth centre toward this half's outer
/// corner, `out` faces the street. A spandrel panel hangs from the shell base
/// with the opening cut under it, a proud voussoir ring follows the intrados,
/// and a curved soffit closes the panel's thickness; everything stays within
/// 0.3 m of the face plane.
#[allow(clippy::too_many_arguments)]
fn add_mouth_arch(
    mesh: &mut MeshData,
    origin: Vec2,
    u_dir: Vec2,
    out: Vec2,
    inner: f32,
    outer: f32,
    spring_y: f32,
    crown_y: f32,
    ring: f32,
    top_y: f32,
) {
    // The circle through both springings and the intrados crown.
    let half_span = (outer - inner) * 0.5;
    let rise = crown_y - spring_y;
    let drop = (half_span * half_span - rise * rise) / (2.0 * rise);
    let radius = drop + rise;
    let center_u = (inner + outer) * 0.5;
    let center_y = spring_y - drop;
    let theta = (half_span / radius).asin();

    let front = 0.12;
    let back = -0.18;
    let out3 = Vec3::new(out.x, 0.0, out.y);
    let u_axis = Vec3::new(u_dir.x, 0.0, u_dir.y);
    let point = |u: f32, y: f32, n: f32| {
        let flat = origin + u_dir * u + out * n;
        Vec3::new(flat.x, y, flat.y)
    };
    // `u_dir` flips per mouth half, so windings are fixed against the normal.
    let quad_toward = |mesh: &mut MeshData,
                       mut points: [Vec3; 4],
                       mut uvs: [Vec2; 4],
                       normal: Vec3,
                       shade: f32| {
        if (points[1] - points[0])
            .cross(points[2] - points[0])
            .dot(normal)
            < 0.0
        {
            points.swap(1, 3);
            uvs.swap(1, 3);
        }
        let first = mesh.positions.len() as u32;
        for (point, uv) in points.into_iter().zip(uvs) {
            mesh.vertex_shaded(point, normal, uv, shade);
        }
        mesh.indices
            .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
    };

    // Voussoir-sized steps; the soffit facets share their seams with the ring
    // segments so the cut reads as one build.
    let segments = ((2.0 * theta * radius / 0.55).round() as usize).clamp(10, 16);
    let arc = |index: usize, r: f32| {
        let angle = -theta + 2.0 * theta * index as f32 / segments as f32;
        (center_u + r * angle.sin(), center_y + r * angle.cos())
    };
    for index in 0..segments {
        let (u0, y0) = arc(index, radius);
        let (u1, y1) = arc(index + 1, radius);
        // Spandrel infill from the opening up to the shell base line.
        for (n, normal, shade) in [(front, out3, 1.0), (back, -out3, 0.85)] {
            quad_toward(
                mesh,
                [
                    point(u0, y0, n),
                    point(u1, y1, n),
                    point(u1, top_y, n),
                    point(u0, top_y, n),
                ],
                [
                    Vec2::new(u0 / 3.5, y0 / 3.5),
                    Vec2::new(u1 / 3.5, y1 / 3.5),
                    Vec2::new(u1 / 3.5, top_y / 3.5),
                    Vec2::new(u0 / 3.5, top_y / 3.5),
                ],
                normal,
                shade,
            );
        }
        // Soffit closing the panel thickness along the intrados.
        let mid = -theta + 2.0 * theta * (index as f32 + 0.5) / segments as f32;
        quad_toward(
            mesh,
            [
                point(u0, y0, front),
                point(u1, y1, front),
                point(u1, y1, back),
                point(u0, y0, back),
            ],
            [
                Vec2::new(u0 / 3.5, 0.0),
                Vec2::new(u1 / 3.5, 0.0),
                Vec2::new(u1 / 3.5, (front - back) / 3.5),
                Vec2::new(u0 / 3.5, (front - back) / 3.5),
            ],
            -(u_axis * mid.sin() + Vec3::Y * mid.cos()),
            0.55,
        );
        // The voussoir band, proud of the panel, a whisker off the soffit.
        let band = radius + 0.01 + ring * 0.5;
        let (v0, w0) = arc(index, band);
        let (v1, w1) = arc(index + 1, band);
        add_face_member(
            mesh,
            origin,
            u_dir,
            out,
            Vec2::new(v0, w0),
            Vec2::new(v1, w1),
            ring * 0.5,
            front + 0.14,
            false,
        );
    }
    // The outer edge of the panel, visible where no abutment stands flush.
    quad_toward(
        mesh,
        [
            point(outer, spring_y, front),
            point(outer, top_y, front),
            point(outer, top_y, back),
            point(outer, spring_y, back),
        ],
        [
            Vec2::new(spring_y / 3.5, 0.0),
            Vec2::new(top_y / 3.5, 0.0),
            Vec2::new(top_y / 3.5, (front - back) / 3.5),
            Vec2::new(spring_y / 3.5, (front - back) / 3.5),
        ],
        u_axis,
        1.0,
    );
}

fn build_ropewalk(commands: &mut Commands, meshes: &CityMeshes, materials: &CityMaterials) {
    for z in (162..=202).step_by(8) {
        spawn_box_named(
            commands,
            meshes,
            &materials.timber,
            Vec3::new(-182.0, 1.35, z as f32),
            Vec3::new(0.18, 2.7, 0.18),
            "The Cut ropewalk post",
        );
    }
    for x in [-182.8, -182.25, -181.7] {
        spawn_box_named(
            commands,
            meshes,
            &materials.dark_wood,
            Vec3::new(x, 1.9, 182.0),
            Vec3::new(0.035, 0.035, 40.6),
            "The Cut ropewalk line",
        );
    }
}

/// The Cut's centreline in world X. The street is one straight segment from
/// `z +325.5` to `z -422.0` and does not bend, which is why it can carry a line
/// at all.
const CUT_CENTRE_X: f32 = -213.5;
/// Half the cartway: the kerb faces stand five metres off the centreline, so
/// the lane the Bench protects is ten metres — mid-range of the 8–12 m the
/// gazetteer gives the working cartway.
const CUT_KERB_OFFSET_M: f32 = 5.0;
const CUT_KERB_WIDTH_M: f32 = 0.30;
/// A tenth of a metre, not the quarter the section plate draws. See
/// `build_kerb`.
const CUT_KERB_RISE_M: f32 = 0.10;
/// The housefront lines. Measured off every plan building fronting the street
/// they are exact to the centimetre for all three reaches, so the margin can be
/// drawn to them rather than inferred per block.
const CUT_FACADE_WEST_X: f32 = -225.2;
const CUT_FACADE_EAST_X: f32 = -201.8;
/// The margin flags' own seat over their ground: a whisker over the road
/// ribbon (`y = 0.024`) when everything stood at grade, and since M3 the same
/// whisker on top of the step — the flags are drawn at `CUT_MARGIN_Y +
/// CUT_STEP_M`. The square markings sit a whisker over the ribbon still.
const CUT_MARGIN_Y: f32 = 0.030;
const CUT_MARKER_TOP_Y: f32 = 0.034;
/// Nominal kerbstone; the run divides its length into a whole number of these.
const CUT_KERB_STONE_M: f32 = 1.3;
/// The three reaches where a kerb was actually laid, north to south:
/// Chain Bridge quarter to the Tallage, the Tallage to Maren's Green, and
/// Maren's Green down to the Old Sluice.
const CUT_LAID_REACHES: [(f32, f32); 3] = [(105.0, 325.5), (-216.3, 23.8), (-422.0, -294.7)];
/// The two squares the ribbon runs through, where the boundary is a rule the
/// Bench asserts rather than a stone somebody laid: `tallage` and
/// `marens_green`, at their own polygon extents.
const CUT_MARKED_REACHES: [(f32, f32); 2] = [(23.8, 105.0), (-294.7, -216.3)];
/// Spacing of the flush marker blocks through the squares.
const CUT_MARKER_PITCH_M: f32 = 6.0;
/// Width of one lane of margin flagging. The margin is emitted as a handful of
/// lanes rather than one 6.7 m band so that a street running *along* the back
/// of it only takes away the ground it actually stands on — see
/// `cut_margin_strips`.
const CUT_MARGIN_LANE_M: f32 = 0.85;

/// How high the old bank still stands over the filled channel, at the head of a
/// blocked water stair. The section plate's 0.40 m: enough for a flight of six
/// treads to have somewhere to go, low enough that walking through it does not
/// read as walking through a building — the flight is walk-through stone, like
/// every solid on this street except the riser and the bollards (M3's two
/// colliders; see `build_kerb`). Anything else made solid here means re-running
/// the whole four-step nav chain.
const CUT_BANK_TOP_Y: f32 = 0.40;
/// The head tread's width across the flight, and how much each tread below it
/// loses to the battering cheek walls.
const CUT_STAIR_WIDTH_M: f32 = 4.4;
const CUT_STAIR_BATTER_M: f32 = 0.11;
/// Treads below the head landing. The sixth comes out at 0.057 m — a stone the
/// fill has all but taken — and two drowned slabs past it finish in the
/// cartway.
const CUT_STAIR_TREADS: usize = 6;
const CUT_STAIR_TREAD_M: f32 = 0.40;
/// The mooring stone at a stair head: set four and a half times the height of
/// the kerb it stood among when the channel was filled, because a ring set in
/// a 0.10 m kerb would be a ring underground. It keeps its absolute height
/// through M3 — the stone belongs to the *old* bank, like the stair landing it
/// stands beside (`CUT_BANK_TOP_Y`), so the raised margin has merely caught it
/// up: a tenth over the stepped kerb's top, its ring still at rope height off
/// the cartway.
const CUT_MOORING_STONE_Y: f32 = 0.45;
/// How far past the edge of the flight the mooring stone's centre stands, and
/// how long the stone is along the line. Both feed the clearance
/// `CutProp::kerb_gap` has to leave for it on *either* hand.
const CUT_MOORING_STANDOFF_M: f32 = 0.8;
const CUT_MOORING_STONE_HALF_Z: f32 = 0.45;
const CUT_BOLLARD_Y: f32 = 0.87;
/// A lawful crossing of the line at a warehouse door: three metres of the same
/// stone, laid flush, so a cart can cross where the Bench says it may.
const CUT_KERB_BREAK_M: f32 = 3.0;

/// M3 — the real step. How far the margin's ground stands above the cartway:
/// the quarter-metre the section plate always drew and §2.2 deliberately did
/// not ship until something needed it. The margin flags are drawn at
/// `CUT_MARGIN_Y + CUT_STEP_M`, the kerbstones carry their whole M0–M2 height
/// stack up by the same amount (their *seat* stays at `y -CUT_KERB_SEAT_M`, so
/// the stones' cartway faces are the drawn riser), and `CutMarginProfile` puts
/// the same step under the player's and the puppets' feet.
pub const CUT_STEP_M: f32 = 0.25;
/// The riser's collider top: a centimetre *below* the step, for two reasons.
/// Walking off the margin must never snag — feet on the margin stand at
/// `CUT_STEP_M` and a collider flush with them would clip the sweep — and the
/// nav bake does not read height at all (any footprint with `max.y >= 0.01`
/// blocks a cell outright), so nothing is bought by making it taller. From the
/// cartway it is a wall the player cannot step up (the controller has no
/// step-up logic — a hop clears it, which is what a kerb is); from above it is
/// under everything.
const CUT_RISER_TOP_M: f32 = CUT_STEP_M - 0.01;
/// How far behind the kerb line a kerb break's ramp runs before it meets the
/// flags: exactly **two margin lanes** (`2 × 6.7 m / 8`), so the flag gap the
/// ramp descends through ends on a lane boundary and no sliver of bare ground
/// opens beside it. ~14 % grade over the run — steep, but a laid stone pitch a
/// cart can take, which is what the break is for.
const CUT_BREAK_RAMP_RUN_M: f32 = 1.675;
/// Open margin edges (junction mouths, the reach ends at the two squares) are
/// drawn as honest 0.28 m stone edges, but feet ramp through them over this
/// distance instead of teleporting the full step in one frame: the profile
/// feathers the lift near a strip's *open* edges. Open z-ends are the common
/// case, but a road running *along* the back of the margin (the wall lane
/// behind the west bank, and the sliver each diagonal junction mouth leaves
/// where one lane's rect outlives its neighbour's) removes whole lanes and
/// leaves an open **x-edge** in the middle of the flags — those feather the
/// same way, by distance to the edge. Deliberately NOT applied across the kerb
/// line (the riser is a collider there, and a feathered lift would sink feet
/// into the flags), at the façade line (a wall), or at an edge abutting a
/// stair trench or ramp gap (their own profiles carry on at their own height).
const CUT_STEP_FEATHER_M: f32 = 0.45;

/// The deep end of the envelope `the_cut_kerb.md` M2 gives a sounding. It is
/// also the scale the per-stone disturbance in `cut_kerbstone_top` is taken
/// against; the rest of the envelope is asserted in
/// `the_cut_soundings_stay_inside_the_authored_envelope`.
const CUT_SAG_MAX_M: f32 = 0.25;
/// The full peak-to-peak scatter a stone that has gone down may show out of the
/// line it was laid in, at the deepest sounding — so ±0.010 m about the profile.
/// Ground that moved did not move in a plane, so the drowned stretch reads as
/// broken masonry rather than as a deliberately flush line, which is what the
/// *squares* mean and the exact opposite of a sounding.
///
/// **Two-sided, not one.** A scatter that only ever lifts is a bias, and a bias
/// proportional to the sag lifts the bottom of a trough more than its shoulders
/// — i.e. it fills in the very thing it was added to sell. So it is centred
/// about the profile. With M3's step under the line the deepest heaved-down
/// stone still rides `CUT_STEP_M + 0.092 − 0.25 ≈ 0.09 m` proud of the
/// cartway, so no scatter can take a stone under the road.
const CUT_KERB_HEAVE_M: f32 = 0.020;
/// How much darker the same limestone is drawn at the deepest sounding. Not
/// weathering for its own sake: a kerb standing 0.10 m proud sheds and is swept,
/// and one lying at street level takes the dust, the traffic and the filled
/// joints. It is the one cue that survives being sighted at from two hundred
/// metres, and it means the trough can be read without a true reach beside it to
/// compare against.
const CUT_KERB_DROWNED_TINT: f32 = 0.16;
/// How far below grade a kerbstone is seated. The block's *bottom* is pinned
/// here, at `y -0.42`, for every stone on the street; the sag moves only its
/// top, so the block grows and shrinks with the profile instead of pivoting
/// about `y = 0`. That is what guarantees no stone ever floats and no gap opens
/// under the line, and it is the seam M3 has to move if it lowers the ground:
/// the bottom is an absolute, not an offset from the top face.
const CUT_KERB_SEAT_M: f32 = 0.42;
/// One authored sounding — a stretch where the line has gone down because the
/// ground under it is the filled channel of the Serle rather than the old bank.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CutSounding {
    /// Deepest point, in world z.
    z: f32,
    /// Half the length of the dip; the profile is zero at `z ± half_length_m`.
    half_length_m: f32,
    /// How far each line has gone down at `z`. They differ because the channel
    /// was narrower than the street and did not run down its middle.
    west_m: f32,
    east_m: f32,
}

/// The soundings — `the_cut_kerb.md` M2.
///
/// **What the lore fixes, and what it does not.** `lore/wells_and_water.md` is
/// law on what is under this street and it is explicit: *"No plan of the old
/// channel was ever drawn."* The soundings are an unwritten oral map the
/// boat-families sell and the Cut landlords deny, so nothing anywhere in `lore/`
/// gives a z for the deep line. What it does fix, and what these four agree
/// with:
///
/// - **The Old Sluice** (`z ≈ -427`) is a gate, and a gate stands across the
///   channel. Water held at a gate scours a pool above it, and that pool is the
///   deepest hole and the youngest fill on the street — hence the sounding at
///   `z -362`, the deepest of the four, and deeper on the west. It stops sixty
///   metres short of the sluice itself: the true arch is the Alders' to sell and
///   this feature does not publish it.
/// - **The Chain Bridge** (`z +297.5`) takes its working name from the harbour
///   chain-house, and a chain closes *navigable* water. The channel was
///   therefore deep and near mid-stream under it, which is why the sounding at
///   `z +286` takes both lines almost equally.
/// - **M1's blocked water stairs.** A flight was built where the channel came
///   alongside that bank, so the four stairs that fall on a sounding at all
///   stand on its *shoulder*: `z +160` east opens the north reach's sounding,
///   `z -100` east and `z -168` west are the two ends of the middle reach's, and
///   `z -330` west stands on the north lip of the sluice pool. None of them
///   stands in a trough — see
///   `the_cut_soundings_leave_the_laid_furniture_standing`.
///
/// Between those the channel ran a little east of centre through the middle
/// reach and crossed back below Maren's Green, which is the whole editorial
/// claim being made here and the one thing a mason, a landlord or Wyn Alder
/// could argue with. §7 of the feature doc lists writing it into
/// `the_dry_boatmen.md` as the follow-up.
///
/// **Four, and only in the laid reaches.** The squares are drawn flush, so a
/// sounding inside one would have nothing to sag; and a fifth would start to
/// read as noise rather than as three or four specific places. The middle reach
/// gets the longest and the emptiest, because it is the stretch a player
/// actually walks and sights down.
const CUT_SOUNDINGS: [CutSounding; 4] = [
    // The chain reach: the harbour head, and the deepest water the Serle
    // carried inside the walls.
    CutSounding { z: 286.0, half_length_m: 26.0, west_m: 0.18, east_m: 0.19 },
    // The wool quarter, opening at the one blocked stair on this reach.
    CutSounding { z: 190.0, half_length_m: 30.0, west_m: 0.16, east_m: 0.21 },
    // Below the Tallage, bracketed by the middle reach's two stairs. The
    // longest of the four, because this is the stretch a player walks and
    // sights down.
    CutSounding { z: -134.0, half_length_m: 36.0, west_m: 0.17, east_m: 0.22 },
    // The pool above the sluice gate.
    CutSounding { z: -362.0, half_length_m: 34.0, west_m: 0.24, east_m: 0.20 },
];

/// Half the width of one stair's head tread, jittered off that stair's seed so
/// five flights along a straight street are not five copies of one object.
///
/// It lives out here, and is bounded, because `CutProp::kerb_gap` has to open
/// enough line for the *widest* flight the jitter can produce plus the mooring
/// stone that stands beside its head — see
/// `a_cut_water_stair_breaks_the_line_it_descends_through`.
fn cut_stair_half_head(seed: u32) -> f32 {
    CUT_STAIR_WIDTH_M * 0.5 * (0.92 + (seed % 15) as f32 * 0.010)
}

/// The one seed a water stair owns. `add_water_stair`, the margin flagging
/// (which stops at the flight's cheek walls) and the ground profile under feet
/// all jitter off the same number, so the three can never disagree about how
/// wide one flight came out.
fn cut_stair_seed(bank: CutBank, z: f32) -> u32 {
    stable_hash(&format!("cut-stair-{:.1}-{z:.1}", bank.kerb_x()))
}

/// Which side of the Cut a piece of margin furniture stands on. Everything in
/// `CUT_FURNITURE` is authored as (kind, bank, z), and the bank supplies the
/// three x values the piece needs: the kerb line it stands on or breaks, the
/// housefront it stands against, and which way "out of the cartway" points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CutBank {
    West,
    East,
}

impl CutBank {
    fn kerb_x(self) -> f32 {
        match self {
            CutBank::West => CUT_CENTRE_X - CUT_KERB_OFFSET_M,
            CutBank::East => CUT_CENTRE_X + CUT_KERB_OFFSET_M,
        }
    }

    fn facade_x(self) -> f32 {
        match self {
            CutBank::West => CUT_FACADE_WEST_X,
            CutBank::East => CUT_FACADE_EAST_X,
        }
    }

    /// The sign that walks from the cartway out across the margin toward the
    /// housefronts. Every piece of furniture is written once, in these terms,
    /// and comes out mirrored on the far bank.
    fn outward(self) -> f32 {
        match self {
            CutBank::West => -1.0,
            CutBank::East => 1.0,
        }
    }
}

/// One piece of authored margin furniture — `the_cut_kerb.md` M1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CutProp {
    /// A flight of stone descending out of the old bank, through the kerb line
    /// and into the cartway, where the fill takes it. The fossil of the river.
    WaterStair,
    /// A short stone post standing in the line, at a square's threshold or
    /// beside a bridge pier.
    Bollard,
    /// Three metres of flush stone where a warehouse door faces the street.
    KerbBreak,
    /// A double-leaf timber hatch let into the margin against a housefront.
    CellarHatch,
    /// A barred stone vent at the foot of a wall — the cheaper answer to the
    /// same problem.
    CellarVent,
}

impl CutProp {
    /// How much of the kerb line this piece takes out of the run, if any,
    /// measured from the piece's own z.
    ///
    /// A water stair's gap is wider than the flight and **symmetric about it,
    /// deliberately**: the mooring stone stands to one side of the head rather
    /// than astride it, and which side comes off the stair's seed
    /// (`add_water_stair`'s `mooring_side`), so both sides have to be open. The
    /// widest the stone's far edge ever reaches is `half_head + 0.8 + 0.45 =
    /// 3.582 m`, 18 mm inside the 3.6 m the gap opens. Do not narrow one end to
    /// match the drawing — half the stairs would then be clipped by their own
    /// kerb.
    fn kerb_gap(self) -> Option<(f32, f32)> {
        match self {
            CutProp::WaterStair => {
                Some((-CUT_STAIR_WIDTH_M * 0.5 - 1.4, CUT_STAIR_WIDTH_M * 0.5 + 1.4))
            }
            CutProp::Bollard => Some((-0.35, 0.35)),
            CutProp::KerbBreak => Some((-CUT_KERB_BREAK_M * 0.5, CUT_KERB_BREAK_M * 0.5)),
            CutProp::CellarHatch | CutProp::CellarVent => None,
        }
    }
}

/// Everything hand-placed on the Cut's margins, north to south.
///
/// **Hand-placed, not spaced.** The three reaches are different places and a
/// procedural rhythm would read as wallpaper on all of them at once — which is
/// the failure M0 already avoided once by *not* filling the street. So:
///
/// - **North** (`z +105 … +325`) is the rope, wool and hides quarter and the
///   busiest of the three. It gets four kerb breaks, every one of them at a
///   real Cut-facing door in `lore/places/ombreval_buildings.json` — the
///   `storage` house at `z +137.8`, the `trade` houses at `+141.3`, `+152.9`
///   and `+215.1` — so a break always sits under the hoist gantry
///   `build_hoist_gantries` has already rigged over that doorway. It gets one
///   water stair only: a working quarter builds over its stairs.
/// - **Middle** (`z -216 … +23.8`) is the emptiest stretch in the game, so the
///   fossil river carries it: three of the five water stairs are here, spread
///   over 240 m, with a warehouse pair at the Maren's Green end and almost
///   nothing else.
/// - **South** (`z -422 … -294.7`) is poorer and quieter. One stair, two vents,
///   no kerb breaks at all — the whole reach has two Cut-facing doors,
///   `omb_f0053` at `z -325.2` and `omb_f0023` at `z -377.7`, and both are
///   houses rather than warehouses, so nothing here has a cart to bring across
///   the line. Its emptiness is authored, not an oversight.
///
/// Every z here was read off the plan rather than invented: the kerb breaks and
/// the cellar openings sit at (or deliberately beside) the door midpoints
/// `plan_facade_openings` puts on each Cut-facing façade edge, the bollards
/// stand at the two squares' thresholds and either side of the Chain and Tally
/// bridge crossings, and `the_cut_margin_furniture_stands_on_its_own_street`
/// checks all of it against the plan on every run.
const CUT_FURNITURE: [(CutProp, CutBank, f32); 39] = [
    // --- north reach: the trade quarter ---
    (CutProp::Bollard, CutBank::West, 305.5),
    (CutProp::Bollard, CutBank::East, 305.5),
    (CutProp::Bollard, CutBank::West, 289.5),
    (CutProp::Bollard, CutBank::East, 289.5),
    (CutProp::KerbBreak, CutBank::East, 215.1),
    (CutProp::CellarHatch, CutBank::East, 212.9),
    (CutProp::WaterStair, CutBank::East, 160.0),
    (CutProp::KerbBreak, CutBank::West, 152.9),
    (CutProp::CellarHatch, CutBank::West, 149.0),
    (CutProp::KerbBreak, CutBank::West, 141.3),
    (CutProp::CellarHatch, CutBank::East, 141.0),
    (CutProp::KerbBreak, CutBank::East, 137.8),
    (CutProp::CellarVent, CutBank::West, 133.8),
    (CutProp::Bollard, CutBank::West, 106.4),
    (CutProp::Bollard, CutBank::East, 106.4),
    // --- the Tallage: no kerb, but the Bench still guards the bridge piers ---
    (CutProp::Bollard, CutBank::West, 82.5),
    (CutProp::Bollard, CutBank::East, 82.5),
    (CutProp::Bollard, CutBank::West, 64.5),
    (CutProp::Bollard, CutBank::East, 64.5),
    // --- middle reach: the emptiest stretch, so the river carries it ---
    (CutProp::Bollard, CutBank::West, 22.4),
    (CutProp::Bollard, CutBank::East, 22.4),
    (CutProp::CellarHatch, CutBank::West, -33.6),
    (CutProp::WaterStair, CutBank::West, -40.0),
    (CutProp::KerbBreak, CutBank::West, -45.8),
    (CutProp::WaterStair, CutBank::East, -100.0),
    (CutProp::CellarVent, CutBank::East, -124.6),
    (CutProp::CellarHatch, CutBank::West, -155.4),
    (CutProp::WaterStair, CutBank::West, -168.0),
    (CutProp::CellarHatch, CutBank::East, -195.6),
    (CutProp::KerbBreak, CutBank::East, -198.3),
    (CutProp::CellarHatch, CutBank::West, -199.9),
    (CutProp::KerbBreak, CutBank::West, -203.1),
    (CutProp::Bollard, CutBank::West, -215.0),
    (CutProp::Bollard, CutBank::East, -215.0),
    // --- south reach: poorer, quieter, rougher ---
    (CutProp::Bollard, CutBank::West, -296.1),
    (CutProp::Bollard, CutBank::East, -296.1),
    (CutProp::CellarVent, CutBank::East, -322.6),
    (CutProp::WaterStair, CutBank::West, -330.0),
    (CutProp::CellarVent, CutBank::West, -374.6),
];

/// How far the line has gone down at `z` on `bank` — the sounding profile.
///
/// A raised cosine rather than a straight-sided trough: a settling street has no
/// edges, and a break of slope in a straightedge reads as a broken stone, not as
/// ground. Zero everywhere outside a sounding, so the great majority of the 748
/// m is dead true and the four places that are not can be seen against it. The
/// soundings do not overlap (asserted in `the_cut_soundings_stay_inside_the
/// _authored_envelope`); `max` rather than a sum is what keeps a future pair of
/// them from adding into a hole.
fn cut_sounding_sag(bank: CutBank, z: f32) -> f32 {
    let mut sag = 0.0_f32;
    for sounding in CUT_SOUNDINGS {
        let along = (z - sounding.z).abs();
        if along >= sounding.half_length_m {
            continue;
        }
        let depth = match bank {
            CutBank::West => sounding.west_m,
            CutBank::East => sounding.east_m,
        };
        sag = sag.max(0.5 * depth * (1.0 + (PI * along / sounding.half_length_m).cos()));
    }
    sag
}

/// The top face of the kerbstone centred at `z`, and the height of the block
/// that carries it — both in the street's *old* datum, before M3's step:
/// `add_kerbstone_run` draws the stone `CUT_STEP_M` higher.
///
/// Three things at once, which is why it is one function and not three: the
/// nominal 0.10 m ridge with its own per-stone weathering; the sounding, at
/// its **full authored depth**; and the heave, a two-sided per-stone scatter
/// about the profile so a drowned stretch reads as broken masonry rather than
/// as a suspiciously level trace.
///
/// M2 shipped this scaled into a six-centimetre budget (`CUT_KERB_DROWNED_Y`,
/// since deleted), because with everything at grade a stone taken the full
/// 0.20 m down was a hole under the road plane, not a dip. M3's step is what
/// M2's notes said it would be: the whole budget. The kerb's true top now
/// stands `CUT_STEP_M + rise` over the cartway, so the deepest authored
/// sounding (0.24 m) still leaves its stones ~0.09 m proud of the road — sunk
/// below the margin flags behind them, whose exposed edge faces are exactly
/// the claim: the bank stands, the line drowned. Where there is no sounding
/// there is neither drop nor heave, so the true line keeps its nominal rise
/// exactly and the tests can read it.
fn cut_kerbstone_top(bank: CutBank, z: f32, seed: u32) -> (f32, f32) {
    let rise = CUT_KERB_RISE_M * (0.92 + ((seed >> 8) % 13) as f32 * 0.013);
    let sag = cut_sounding_sag(bank, z);
    let drop = (sag / CUT_SAG_MAX_M).clamp(0.0, 1.0);
    let heave = CUT_KERB_HEAVE_M * 0.5 * drop * (((seed >> 16) % 9) as f32 - 4.0) / 4.0;
    let top = rise - sag + heave;
    (top, top + CUT_KERB_SEAT_M)
}

/// The vertex brush one kerbstone carries: quarried limestone, per-stone, dirtied
/// in proportion to how far the sounding has taken it down. See
/// `CUT_KERB_DROWNED_TINT` for why the tone is doing work the geometry cannot.
fn cut_kerbstone_shade(sag: f32, seed: u32) -> f32 {
    let quarried = 0.81 + (seed % 14) as f32 * 0.01;
    quarried * (1.0 - CUT_KERB_DROWNED_TINT * (sag / CUT_SAG_MAX_M).clamp(0.0, 1.0))
}

/// One stretch of one side of the Cut's kerb line.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CutKerbRun {
    /// Always `CUT_CENTRE_X ± CUT_KERB_OFFSET_M` — the line does not wander.
    /// M2 varies the stones' `y` along the run and nothing else; `x` is the
    /// invariant the whole feature rests on.
    x: f32,
    /// Which line this is. The sounding profile is asked per bank, because the
    /// channel was narrower than the street.
    bank: CutBank,
    z0: f32,
    z1: f32,
    /// A laid ridge with its margin behind it, or the flush marking a square
    /// gets instead.
    laid: bool,
}

/// The Cut's kerb line — `features/the_cut_improvements/the_cut_kerb.md` M0.
///
/// Two lines of stone five metres off the centreline, and a different ground
/// outside them. The Cut is the widest surface in Ombreval (20 m bank to bank,
/// 2.4× the next widest street) and until now the only one with nothing drawn
/// on it at all. This does not fill it; it divides it, into a cartway the
/// Bench keeps clear and a margin where the street's life is permitted to
/// happen.
///
/// **The riser is the only collider, and its gaps are load-bearing (M3).**
/// `scripts/bake_navigation.py::build_walkable` erodes *every* exported
/// collider footprint by the agent radius (0.35 m) and then keeps only the
/// largest connected component, and `solid_footprints_in_band(WALK_BAND_LO,
/// ..)` exports anything at all whose top reaches `y = 0.01`. Two unbroken
/// 748 m riser walls would therefore sever the margin from the cartway along
/// the whole street; the margin would become its own component and be
/// **dropped**, stranding Tam Rud, every housefront door on the Cut, the
/// ropewalk approach and every stall we ever pitch here. What keeps it in the
/// main component is the gaps `cut_kerb_plan` already opens: ten junction
/// mouths (a street's own width each), seven 3 m kerb breaks (2.3 m survive
/// the erosion — nine cells wide) and five 7.2 m water-stair gaps. The margin
/// itself is *never* a collider — a raised slab would export as a footprint
/// and carve the whole margin out of the walkable surface — which is why the
/// step under feet is `CutMarginProfile`, a pure ground-height function, not a
/// floor solid. Anything that changes these colliders means re-running the
/// whole four-step nav chain and re-pinning `shelters.json` by hand.
///
/// **The step is real now (M3).** M0–M2 shipped everything at grade because
/// the player resolved only against `CollisionWorld` and the puppets stood on
/// the flat nav graph. M3 lifts the margin the section plate's 0.25 m
/// (`CUT_STEP_M`): the flags are drawn as edged slabs at `CUT_MARGIN_Y +
/// CUT_STEP_M`, the kerbstones ride up with their seats still buried (their
/// cartway faces are the drawn riser), the kerb breaks become laid stone
/// ramps a cart could take, and the water stairs — head landing untouched at
/// `CUT_BANK_TOP_Y`, treads unchanged — now genuinely descend from the bank
/// through the open kerb gap into the road. The player and the puppets stand
/// on it via `CutMarginProfile::ground_lift`. The cambered cartway still
/// belongs to the hoop game, which will want to pick its own profile.
///
/// **Inside the squares the line is marked, not built.** The gazetteer is
/// specific that on Lowmarket the Cut stays open to through carts down its
/// middle and that stalls use *marked* margins; so through the Tallage and
/// Maren's Green the same line is flush blocks at six-metre intervals, with no
/// ridge and no margin flagging, and crossing from street to square you can see
/// the law get weaker.
///
/// **The line breaks at every side street**, computed from the plan rather than
/// hand-listed (`cut_side_gaps`): ten junctions open onto the Cut, and a kerb
/// run straight across a street mouth would bury the last six metres of its
/// cobbles and read as a mistake. The gaps are where carts cross the line
/// lawfully; M1's authored kerb breaks at warehouse doors are the same idea
/// placed by hand. The same pass stops the line at anything the plan stands
/// *across* the street — which is only the Old Sluice, whose shell the south
/// reach's nominal end runs sixteen metres into.
///
/// Everything here goes through `MeshData`/`spawn_batch`: ~900 kerbstones and
/// the margin strips are three batched meshes tiled at `BATCH_TILE_M`, not 900
/// entities. The margin is deliberately *not* added to `CobbleRoadNetwork` and
/// not rasterized as a puddle surface — the Cut is made ground over a filled
/// river bed, it drains, and its footsteps are dust rather than cobble; the
/// margin is the same ground with flags on it, so it keeps the Cut's behaviour
/// rather than the cobbled city's.
///
/// **M1, the margin furniture, is hung off the same line and is walk-through
/// stone** — with one exception since M3: the bollards, which M1 shipped
/// drawn-only, now carry the colliders the feature doc always promised them,
/// riding the riser's own rebake. See `add_bollard` for the argument in full.
///
/// **M2, the soundings, moves the kerbstones and nothing else.** Four authored
/// stretches where the line has gone down because it is standing on the filled
/// channel rather than on the old bank (`CUT_SOUNDINGS`, and `cut_sounding_sag`
/// for the profile). M2 had to scale the profile into a six-centimetre budget
/// because everything stood at grade; M3's step *is* the budget, so the stones
/// now sink their full authored 0.15–0.25 m below the true line — still proud
/// of the cartway, drowned against the flags standing behind them — and the
/// two companion cues stay: the drowned stretch heaves out of true and it
/// dirties, the consequences a settling kerb actually has. A single leaning
/// house says nothing in a city where everything leans; 748 m of straightedge
/// that dips in four places is the one public record of where the river was,
/// which is exactly why the Cut landlords would rather it had never been laid.
fn build_kerb(
    commands: &mut Commands,
    mesh_assets: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    let mut kerbstones = MeshData::default();
    let mut markings = MeshData::default();
    let mut margin = MeshData::default();
    let mut furniture = MeshData::default();
    let mut ironwork = MeshData::default();
    let mut hatch_leaves = MeshData::default();

    for run in cut_kerb_plan(plan) {
        if run.laid {
            add_kerbstone_run(&mut kerbstones, run);
            // M3: the riser. One thin wall per laid run — the gaps between
            // runs (junction mouths, kerb breaks, water stairs) are the
            // crossings the nav bake keeps the margin connected through, so
            // they are exactly the drawn ones.
            collision_world.add_box(
                Vec3::new(run.x - CUT_KERB_WIDTH_M * 0.5, 0.0, run.z0),
                Vec3::new(run.x + CUT_KERB_WIDTH_M * 0.5, CUT_RISER_TOP_M, run.z1),
            );
        } else {
            add_kerb_marking(&mut markings, run);
        }
    }
    for [x0, z0, x1, z1] in cut_margin_strips(plan) {
        add_margin_slab(&mut margin, [x0, z0, x1, z1]);
    }
    for (prop, bank, z) in CUT_FURNITURE {
        match prop {
            CutProp::WaterStair => add_water_stair(&mut furniture, &mut ironwork, bank, z),
            CutProp::Bollard => add_bollard(&mut furniture, collision_world, bank, z),
            CutProp::KerbBreak => add_kerb_break(&mut markings, &mut ironwork, bank, z),
            CutProp::CellarHatch => {
                add_cellar_hatch(&mut furniture, &mut hatch_leaves, &mut ironwork, bank, z);
            }
            CutProp::CellarVent => add_cellar_vent(&mut furniture, &mut ironwork, bank, z),
        }
    }
    // The ground the step puts under feet — see `CutMarginProfile`.
    commands.insert_resource(cut_margin_profile(plan));

    spawn_batch(
        commands,
        mesh_assets,
        &materials.cut_margin,
        margin,
        "The Cut margin",
    );
    spawn_batch(
        commands,
        mesh_assets,
        &materials.limestone,
        kerbstones,
        "The Cut kerbstone",
    );
    spawn_batch(
        commands,
        mesh_assets,
        &materials.limestone,
        markings,
        "The Cut kerb marking",
    );
    spawn_batch(
        commands,
        mesh_assets,
        &materials.limestone,
        furniture,
        "The Cut margin furniture",
    );
    spawn_batch(
        commands,
        mesh_assets,
        &materials.dark_wood,
        hatch_leaves,
        "The Cut cellar hatches",
    );
    spawn_batch(
        commands,
        mesh_assets,
        &materials.iron,
        ironwork,
        "The Cut margin ironwork",
    );
}

/// Where the authored furniture of one bank takes the ridge out of the line: a
/// water stair descends *through* it, a kerb break replaces it with flush
/// stone, a bollard is set into it. Kept separate from `cut_side_gaps` so the
/// tests can tell a metre lost to the plan from a metre lost on purpose.
fn cut_furniture_kerb_gaps(bank: CutBank) -> Vec<(f32, f32)> {
    CUT_FURNITURE
        .iter()
        .filter(|(_, at, _)| *at == bank)
        .filter_map(|(prop, _, z)| prop.kerb_gap().map(|(lo, hi)| (z + lo, z + hi)))
        .collect()
}

/// Every stretch of kerb line the Cut carries, both sides, laid and marked,
/// with the side-street mouths already taken out. Pure geometry over the plan
/// so the invariants in `mod tests` can read the same authored line the
/// renderer draws.
fn cut_kerb_plan(plan: &CityPlan) -> Vec<CutKerbRun> {
    let mut runs = Vec::new();
    for bank in [CutBank::West, CutBank::East] {
        let x = bank.kerb_x();
        // Against the line itself, not the whole band: a lane running *parallel*
        // behind the kerb (`south_inner_wall` hugs x -224 for twenty-four
        // metres) is not a junction and must not open one.
        let mut gaps = cut_side_gaps(plan, x, x);
        gaps.extend(cut_furniture_kerb_gaps(bank));
        for (reaches, laid) in [(&CUT_LAID_REACHES[..], true), (&CUT_MARKED_REACHES[..], false)] {
            for &(z0, z1) in reaches {
                runs.extend(
                    subtract_gaps((z0, z1), &gaps)
                        .into_iter()
                        .map(|(z0, z1)| CutKerbRun { x, bank, z0, z1, laid }),
                );
            }
        }
    }
    runs
}

/// The margin flagging behind each laid reach, as `[x0, z0, x1, z1]` rectangles
/// from the kerb line out to the housefront.
///
/// A street that reaches the margin must keep its own cobbles rather than be
/// flagged over — but only where it actually runs. Asking `cut_side_gaps` about
/// the whole 6.7 m band at once cannot express that: it answers in z alone, so
/// *any* road touching the band anywhere across its width deletes the margin
/// for the full depth. `south_inner_wall` runs **parallel** to the Cut at
/// `x -224` for twenty-four metres, and that single lane, four metres wide and
/// five and a half metres behind the kerb, used to strip thirty-six metres of
/// the north reach's west margin — leaving a laid ridge with identical cartway
/// dust on both sides, which is the one distinction this feature exists to make.
///
/// So the margin is emitted as `CUT_MARGIN_LANE_M`-wide lanes and each lane
/// asks the question for its own strip of x. A road crossing the Cut still
/// spans every lane and opens the whole margin at its mouth; a road running
/// behind the margin only takes the lanes it stands in, and the flags survive
/// right up to the kerb. The lanes are coplanar, share edges exactly and take
/// their UVs from world position, so they are invisible as seams; a diagonal
/// approach gets a margin edge that follows its diagonal instead of a single
/// square cut at its widest point.
///
/// The squares get no margin at all: the Tallage and Maren's Green are already
/// paved, and their line is a rule, not a pitch boundary somebody laid a floor
/// for.
fn cut_margin_strips(plan: &CityPlan) -> Vec<[f32; 4]> {
    let mut strips = Vec::new();
    for bank in [CutBank::West, CutBank::East] {
        let (kerb_x, facade_x) = (bank.kerb_x(), bank.facade_x());
        let (lo_x, hi_x) = (kerb_x.min(facade_x), kerb_x.max(facade_x));
        let lanes = ((hi_x - lo_x) / CUT_MARGIN_LANE_M).ceil().max(1.0) as usize;
        let lane_width = (hi_x - lo_x) / lanes as f32;
        for lane in 0..lanes {
            let x0 = lo_x + lane_width * lane as f32;
            let x1 = x0 + lane_width;
            let mut gaps = cut_side_gaps(plan, x0, x1);
            let far = ((x0 - kerb_x) * bank.outward()).max((x1 - kerb_x) * bank.outward());
            gaps.extend(cut_furniture_flag_gaps(bank, far));
            for (z0, z1) in CUT_LAID_REACHES {
                strips.extend(
                    subtract_gaps((z0, z1), &gaps)
                        .into_iter()
                        .map(|(z0, z1)| [x0, z0, x1, z1]),
                );
            }
        }
    }
    strips
}

/// The z ranges the raised margin's own furniture takes out of a lane of
/// flagging whose *outer* edge stands `far_u` metres beyond the kerb line —
/// M3, where the flags stopped being a flat texture and started being ground
/// that has to part around anything that is not at its level.
///
/// A **kerb break**'s ramp climbs through the two lanes nearest the line
/// (`CUT_BREAK_RAMP_RUN_M` is exactly two lanes, so the gap ends on a lane
/// boundary and no bare sliver opens beside the ramp head). A **water stair**
/// is a trench down through the raised bank, so every lane out to the head
/// landing (`u = 3.45`) stops at the flight's cheek walls instead of roofing
/// the treads at flag height; the gap follows the cheeks' batter lane by lane,
/// and lanes wholly behind the landing keep their flags — the strip they run
/// across the trench there is buried inside the landing slab, which stands
/// proud of them. `cut_margin_profile` asks this same function, so the ground
/// under feet and the drawn flags can never disagree about where a gap is.
fn cut_furniture_flag_gaps(bank: CutBank, far_u: f32) -> Vec<(f32, f32)> {
    let mut gaps = Vec::new();
    for (prop, at, z) in CUT_FURNITURE {
        if at != bank {
            continue;
        }
        match prop {
            CutProp::KerbBreak if far_u <= CUT_BREAK_RAMP_RUN_M + 1.0e-3 => {
                gaps.push((z - CUT_KERB_BREAK_M * 0.5, z + CUT_KERB_BREAK_M * 0.5));
            }
            CutProp::WaterStair if far_u <= 3.45 + 1.0e-3 => {
                let half_head = cut_stair_half_head(cut_stair_seed(bank, z));
                // The widest cheek this lane runs beside: index 0 (the full
                // head width) anywhere along the landing, one more course per
                // tread below it.
                let treads_above = if far_u >= 1.15 {
                    0
                } else {
                    (((1.15 - far_u) / CUT_STAIR_TREAD_M) as usize + 1).min(CUT_STAIR_TREADS)
                };
                let half = half_head - CUT_STAIR_BATTER_M * treads_above as f32 + 0.34;
                gaps.push((z - half, z + half));
            }
            _ => {}
        }
    }
    gaps
}

/// The z ranges where something in the plan interrupts one band of the Cut —
/// a side street's ribbon crossing it, widened by half its own width so the
/// mouth stays open, or a building standing across the street. Merged and
/// sorted.
fn cut_side_gaps(plan: &CityPlan, band_lo_x: f32, band_hi_x: f32) -> Vec<(f32, f32)> {
    let mut gaps: Vec<(f32, f32)> = Vec::new();
    // A building that spans the centreline is not a frontage, it is the end of
    // the street: the Old Sluice's shell reaches from `z -448` to `z -406`,
    // sixteen metres inside the south reach, and kerb drawn inside a solid is
    // kerb nobody can ever see. Bridges are excluded by their base height — the
    // Chain and Tally decks also span the centreline, but they pass *over* the
    // Cut and the line runs on underneath them.
    for building in &plan.buildings {
        if building_verticals(building).0 > 0.1 {
            continue;
        }
        let (mut x0, mut x1) = (f32::MAX, f32::MIN);
        let (mut z0, mut z1) = (f32::MAX, f32::MIN);
        for point in &building.polygon {
            x0 = x0.min(point[0]);
            x1 = x1.max(point[0]);
            z0 = z0.min(point[1]);
            z1 = z1.max(point[1]);
        }
        if x0 > CUT_CENTRE_X || x1 < CUT_CENTRE_X || x1 < band_lo_x || x0 > band_hi_x {
            continue;
        }
        gaps.push((z0 - 0.6, z1 + 0.6));
    }
    for road in &plan.roads {
        if road.tier == "cut" {
            continue;
        }
        let half = road.width_m * 0.5;
        let (lo, hi) = (band_lo_x - half, band_hi_x + half);
        for pair in road.points.windows(2) {
            let a = Vec2::from_array(pair[0]);
            let b = Vec2::from_array(pair[1]);
            let dx = b.x - a.x;
            let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
            if dx.abs() < 1.0e-4 {
                if a.x < lo || a.x > hi {
                    continue;
                }
            } else {
                let ta = (lo - a.x) / dx;
                let tb = (hi - a.x) / dx;
                t0 = t0.max(ta.min(tb));
                t1 = t1.min(ta.max(tb));
                if t0 >= t1 {
                    continue;
                }
            }
            let za = a.y + (b.y - a.y) * t0;
            let zb = a.y + (b.y - a.y) * t1;
            gaps.push((za.min(zb) - half - 0.6, za.max(zb) + half + 0.6));
        }
    }
    gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for gap in gaps {
        match merged.last_mut() {
            Some(last) if gap.0 <= last.1 => last.1 = last.1.max(gap.1),
            _ => merged.push(gap),
        }
    }
    merged
}

/// `span` with `gaps` cut out of it; stubs under half a metre are dropped
/// rather than drawn as an orphan stone.
fn subtract_gaps(span: (f32, f32), gaps: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut spans = vec![span];
    for gap in gaps {
        let mut next = Vec::new();
        for (lo, hi) in spans {
            if gap.1 <= lo || gap.0 >= hi {
                next.push((lo, hi));
                continue;
            }
            if gap.0 > lo {
                next.push((lo, gap.0));
            }
            if gap.1 < hi {
                next.push((gap.1, hi));
            }
        }
        spans = next;
    }
    spans.retain(|(lo, hi)| hi - lo > 0.5);
    spans
}

/// The ground the Cut's raised margin (M3) puts under feet, answerable per XZ.
///
/// The player resolves height against `CollisionWorld` floors and the puppets
/// stand on the flat nav graph, so a drawn 0.25 m step is a step both would
/// glide through — and the margin cannot simply *be* a collider slab, because
/// `solid_footprints_in_band` exports every solid topping `y ≥ 0.01` and the
/// nav bake would then carve the whole margin out of the walkable surface,
/// stranding every Cut-facing door. So the riser alone is collided (a thin
/// wall the bake erodes into a line, crossed at the kerb breaks, the water
/// stairs and the junction mouths), and *this* is the floor: a pure function
/// from XZ to extra ground height, built once from the plan in `build_kerb`,
/// read by the player controller and by the actor projection.
///
/// Three region kinds, first match wins: a water stair's flight (feet follow
/// the drawn tread tops exactly, so the five stairs become genuinely walkable
/// stairs), a kerb break's ramp (linear, road to flags over
/// `CUT_BREAK_RAMP_RUN_M`), and the margin strips themselves (flat
/// `CUT_STEP_M`, feathered over `CUT_STEP_FEATHER_M` at their open edges — a
/// junction mouth's z-end, or an x-edge a road along the back of the margin
/// has stripped the neighbouring lanes from — so crossing one is a quick ramp
/// rather than a one-frame teleport).
/// Everywhere else — the cartway, the squares, the rest of the city — it is
/// zero and the two cheap rejects at the top make it free.
#[derive(Resource, Default, Clone)]
pub struct CutMarginProfile {
    /// The margin strips — `cut_margin_strips`' output split per x-neighbour
    /// coverage — each knowing which of its edges is *open* (a junction mouth,
    /// a reach end, or an x-edge whose neighbouring lane a parallel road took;
    /// feet feather down to grade there) and which abuts flags or a furniture
    /// gap whose own profile (a stair's treads, a ramp's pitch) carries
    /// straight on, where a feather would teleport feet a quarter-metre at the
    /// seam.
    rects: Vec<CutMarginRect>,
    /// Kerb-break ramps: `(kerb_x, outward, z0, z1)`.
    ramps: Vec<(f32, f32, f32, f32)>,
    /// Water-stair flights: `(kerb_x, outward, z0, z1)`, z spanning the flight
    /// *and* its cheek walls — the same trench `cut_furniture_flag_gaps` opens
    /// in the flags, so skirting a stair along the margin follows the trench
    /// profile through the (walk-through, as everything on this street was
    /// before M3) cheek rather than falling into a hole between flag edges.
    stairs: Vec<(f32, f32, f32, f32)>,
}

/// One flagged rectangle of raised margin, with the feathering decision baked
/// per edge. See [`CutMarginProfile::rects`].
///
/// The z flags are decided per strip end as the strips come out of
/// `cut_margin_strips`; the x flags need a second pass (`cut_margin_profile`
/// splits each strip by neighbour coverage), because a lane's rect can outlive
/// its neighbour's — a road running along the back of the margin deletes the
/// outer lanes for a stretch and leaves the surviving lane's x-edge as a
/// 0.28 m stone face standing in open ground, which feet would otherwise
/// teleport up in one frame exactly as they would at an unfeathered z-end.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CutMarginRect {
    x0: f32,
    z0: f32,
    x1: f32,
    z1: f32,
    feather_lo: bool,
    feather_hi: bool,
    /// Feather toward `x0` / `x1`: set only where the neighbouring lane at
    /// this rect's z-run carries no flags *and* the missing ground is not a
    /// stair trench or ramp (whose own profiles meet the flags at height) —
    /// and never on the kerb or façade lines.
    feather_x_lo: bool,
    feather_x_hi: bool,
}

impl CutMarginProfile {
    pub fn ground_lift(&self, x: f32, z: f32) -> f32 {
        if !(CUT_FACADE_WEST_X..=CUT_FACADE_EAST_X).contains(&x)
            || !(CUT_LAID_REACHES[2].0..=CUT_LAID_REACHES[0].1).contains(&z)
        {
            return 0.0;
        }
        for &(kerb_x, out, z0, z1) in &self.stairs {
            let u = (x - kerb_x) * out;
            if (z0..=z1).contains(&z) && (-2.05..=3.45).contains(&u) {
                return cut_stair_ground(u);
            }
        }
        for &(kerb_x, out, z0, z1) in &self.ramps {
            let u = (x - kerb_x) * out;
            if (z0..=z1).contains(&z)
                && (-CUT_KERB_WIDTH_M * 0.5..=CUT_BREAK_RAMP_RUN_M).contains(&u)
            {
                let run = CUT_BREAK_RAMP_RUN_M + CUT_KERB_WIDTH_M * 0.5;
                return CUT_STEP_M * ((u + CUT_KERB_WIDTH_M * 0.5) / run).clamp(0.0, 1.0);
            }
        }
        let mut lift = 0.0_f32;
        for rect in &self.rects {
            if (rect.x0..=rect.x1).contains(&x) && (rect.z0..=rect.z1).contains(&z) {
                let lo = if rect.feather_lo { z - rect.z0 } else { f32::MAX };
                let hi = if rect.feather_hi { rect.z1 - z } else { f32::MAX };
                let x_lo = if rect.feather_x_lo { x - rect.x0 } else { f32::MAX };
                let x_hi = if rect.feather_x_hi { rect.x1 - x } else { f32::MAX };
                let inside = lo.min(hi).min(x_lo).min(x_hi);
                lift = lift.max(CUT_STEP_M * (inside / CUT_STEP_FEATHER_M).clamp(0.0, 1.0));
            }
        }
        lift
    }
}

/// The stair flight's ground height at `u` metres outward of the kerb line —
/// the same piecewise tops `add_water_stair` draws: the landing at
/// `CUT_BANK_TOP_Y`, six treads walking it down to a riser above grade, the
/// two drowned slabs, then the cartway. Feet land *on* the drawn stone, so a
/// player or puppet taking a water stair descends it step by step.
fn cut_stair_ground(u: f32) -> f32 {
    if u >= 1.15 {
        return CUT_BANK_TOP_Y;
    }
    let index = ((1.15 - u) / CUT_STAIR_TREAD_M) as usize;
    if index < CUT_STAIR_TREADS {
        CUT_BANK_TOP_Y * (1.0 - (index + 1) as f32 / (CUT_STAIR_TREADS + 1) as f32)
    } else if index < CUT_STAIR_TREADS + 2 {
        CUT_MARGIN_Y - 0.002 * (index - CUT_STAIR_TREADS) as f32
    } else {
        0.0
    }
}

/// Build the profile the same way the renderer builds the street, so the two
/// can never disagree: the strips are `cut_margin_strips`' own output and the
/// stair spans reuse the seeded head width `add_water_stair` draws.
fn cut_margin_profile(plan: &CityPlan) -> CutMarginProfile {
    let mut profile = CutMarginProfile {
        rects: Vec::new(),
        ramps: Vec::new(),
        stairs: Vec::new(),
    };
    let mut strips = Vec::new();
    for [x0, z0, x1, z1] in cut_margin_strips(plan) {
        // A strip's own bank and its outer edge's distance from the line, so
        // the abutment question is asked of exactly the gaps this lane was cut
        // against. A z-end that lands on a furniture gap's edge must not
        // feather — the stair or ramp profile continues at its own height.
        let bank = if (x0 + x1) * 0.5 < CUT_CENTRE_X {
            CutBank::West
        } else {
            CutBank::East
        };
        let far =
            ((x0 - bank.kerb_x()) * bank.outward()).max((x1 - bank.kerb_x()) * bank.outward());
        let gaps = cut_furniture_flag_gaps(bank, far);
        strips.push(CutMarginRect {
            x0,
            z0,
            x1,
            z1,
            feather_lo: !gaps.iter().any(|(_, hi)| (hi - z0).abs() < 0.05),
            feather_hi: !gaps.iter().any(|(lo, _)| (lo - z1).abs() < 0.05),
            feather_x_lo: false,
            feather_x_hi: false,
        });
    }
    // Second pass: the x-edges. A strip's z-ends know whether they are open,
    // but an x-edge can be open too — a road running along the back of the
    // margin deletes whole lanes for a stretch, and each diagonal junction
    // mouth staggers the lanes' ends so one rect outlives its neighbour by a
    // sliver — and the openness varies *along* the rect. So each strip is
    // split at the boundaries of its open x-stretches and the flags are baked
    // per piece; flagging a whole rect instead would feather its edge where
    // the neighbouring lane is present too, grooving the lift along a lane
    // boundary the drawn flags cross seamlessly. What the split does not buy
    // is a true 2D distance field: at the corner where a piece's openness
    // changes, feet within the feather band pop the remaining lift in one
    // frame — confined to a 0.45 m corner per open stretch, the same class
    // of shin-in-stone moment as walking lengthwise out an open z-end.
    for rect in &strips {
        let open_lo = cut_open_x_stretches(rect, true, &strips);
        let open_hi = cut_open_x_stretches(rect, false, &strips);
        if open_lo.is_empty() && open_hi.is_empty() {
            profile.rects.push(*rect);
            continue;
        }
        let mut seams = vec![rect.z0, rect.z1];
        for &(a, b) in open_lo.iter().chain(open_hi.iter()) {
            seams.push(a);
            seams.push(b);
        }
        seams.sort_by(f32::total_cmp);
        seams.dedup_by(|a, b| (*a - *b).abs() < 1.0e-3);
        for pair in seams.windows(2) {
            let mid = (pair[0] + pair[1]) * 0.5;
            profile.rects.push(CutMarginRect {
                z0: pair[0],
                z1: pair[1],
                feather_lo: rect.feather_lo && pair[0] == rect.z0,
                feather_hi: rect.feather_hi && pair[1] == rect.z1,
                feather_x_lo: open_lo.iter().any(|&(a, b)| (a..b).contains(&mid)),
                feather_x_hi: open_hi.iter().any(|&(a, b)| (a..b).contains(&mid)),
                ..*rect
            });
        }
    }
    for (prop, bank, z) in CUT_FURNITURE {
        match prop {
            CutProp::KerbBreak => profile.ramps.push((
                bank.kerb_x(),
                bank.outward(),
                z - CUT_KERB_BREAK_M * 0.5,
                z + CUT_KERB_BREAK_M * 0.5,
            )),
            CutProp::WaterStair => {
                let half_head = cut_stair_half_head(cut_stair_seed(bank, z));
                profile.stairs.push((
                    bank.kerb_x(),
                    bank.outward(),
                    z - half_head - 0.34,
                    z + half_head + 0.34,
                ));
            }
            _ => {}
        }
    }
    profile
}

/// The z stretches of one x-side of `rect` where the neighbouring lane
/// carries no flags and nothing of the street's own furniture stands in for
/// them — the stretches `CutMarginRect::feather_x_lo`/`feather_x_hi` record.
///
/// Covered means: another strip adjoins this edge (lanes share their boundary
/// x exactly, so adjacency is equality up to float noise), or the missing
/// flags on this bank are a stair trench or a kerb break's ramp — ground
/// whose own profile meets the flags at its own height, where a feather would
/// sink feet through drawn stone, the same reasoning as the z-end flags. The
/// kerb and façade lines are never open: the riser collider stands on one and
/// the housefronts on the other. Open stretches under half a metre fall to
/// `subtract_gaps`' stub rule — a sliver that short lives inside its own
/// rect's z-end feather anyway.
fn cut_open_x_stretches(
    rect: &CutMarginRect,
    lo_side: bool,
    strips: &[CutMarginRect],
) -> Vec<(f32, f32)> {
    let edge_x = if lo_side { rect.x0 } else { rect.x1 };
    let sealed = [
        CutBank::West.kerb_x(),
        CutBank::East.kerb_x(),
        CUT_FACADE_WEST_X,
        CUT_FACADE_EAST_X,
    ];
    if sealed.iter().any(|line| (edge_x - line).abs() < 0.02) {
        return Vec::new();
    }
    let bank = if edge_x < CUT_CENTRE_X {
        CutBank::West
    } else {
        CutBank::East
    };
    let mut covered: Vec<(f32, f32)> = strips
        .iter()
        .filter(|other| {
            let other_edge = if lo_side { other.x1 } else { other.x0 };
            (other_edge - edge_x).abs() < 0.02
        })
        .map(|other| (other.z0, other.z1))
        .collect();
    for (prop, at, z) in CUT_FURNITURE {
        if at != bank {
            continue;
        }
        match prop {
            CutProp::KerbBreak => {
                covered.push((z - CUT_KERB_BREAK_M * 0.5, z + CUT_KERB_BREAK_M * 0.5));
            }
            CutProp::WaterStair => {
                let half = cut_stair_half_head(cut_stair_seed(bank, z)) + 0.34;
                covered.push((z - half, z + half));
            }
            _ => {}
        }
    }
    subtract_gaps((rect.z0, rect.z1), &covered)
}

/// One margin strip as an edged slab: the flag surface at `CUT_MARGIN_Y +
/// CUT_STEP_M` with real side faces down past the ground. M0 drew the margin
/// as flat top-only quads, which a raised margin cannot be — its quarter-metre
/// edges are *seen*: end-on at every junction mouth and reach end, and from
/// the cartway behind a drowned kerbstone. The top keeps
/// `FLOOR_TEXTURE_SPAN_METERS` so the flags tile exactly as they did at
/// grade; the edges run their UVs off world position like every dressed stone.
/// No bottom face — it is a metre underground.
fn add_margin_slab(mesh: &mut MeshData, [x0, z0, x1, z1]: [f32; 4]) {
    let top = CUT_MARGIN_Y + CUT_STEP_M;
    let bottom = -0.06;
    let center = Vec3::new((x0 + x1) * 0.5, (top + bottom) * 0.5, (z0 + z1) * 0.5);
    let half = Vec3::new((x1 - x0) * 0.5, (top - bottom) * 0.5, (z1 - z0) * 0.5);
    // The top, at the floor span; then the four edges, wound exactly as
    // `add_dressed_stone` winds its faces (`right × up == normal`).
    let flat = Vec3::new(center.x, top, center.z);
    let top_points = [
        flat - Vec3::Z * half.z - Vec3::X * half.x,
        flat + Vec3::Z * half.z - Vec3::X * half.x,
        flat + Vec3::Z * half.z + Vec3::X * half.x,
        flat - Vec3::Z * half.z + Vec3::X * half.x,
    ];
    mesh.quad(
        top_points,
        Vec3::Y,
        top_points.map(|point| {
            Vec2::new(
                point.x / FLOOR_TEXTURE_SPAN_METERS,
                point.z / FLOOR_TEXTURE_SPAN_METERS,
            )
        }),
    );
    for (normal, right, up) in [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::Y, Vec3::X),
    ] {
        let face_center = center + normal * half.dot(normal.abs());
        let (half_r, half_u) = (half.dot(right.abs()), half.dot(up.abs()));
        let points = [
            face_center - right * half_r - up * half_u,
            face_center + right * half_r - up * half_u,
            face_center + right * half_r + up * half_u,
            face_center - right * half_r + up * half_u,
        ];
        mesh.quad(
            points,
            normal,
            points.map(|point| Vec2::new(point.dot(right) / 2.4, point.dot(up) / 2.4)),
        );
    }
}

/// A laid stretch, cut into individual stones. Kerb is quarried and set by the
/// piece: a single 240 m extrusion reads as a plastic strip, whereas stones of
/// slightly different height and weathering read as masonry from any distance
/// the fog leaves open.
///
/// This is also where the soundings are drawn (M2) and the step (M3). Each
/// stone takes its top from `cut_kerbstone_top` and its tone from
/// `cut_kerbstone_shade`, both at its own centre, lifted whole by `CUT_STEP_M`
/// — and every block still bottoms at the same `y -CUT_KERB_SEAT_M` however
/// far the profile has taken its top: the block grows and shrinks rather than
/// pivoting about `y = 0`, so a dipping stone stays seated in the ground
/// instead of hanging over a gap, and its cartway face *is* the drawn riser.
/// The line itself never moves in `x`.
fn add_kerbstone_run(mesh: &mut MeshData, run: CutKerbRun) {
    let length = run.z1 - run.z0;
    let count = ((length / CUT_KERB_STONE_M).round() as usize).max(1);
    let pitch = length / count as f32;
    for index in 0..count {
        let z0 = run.z0 + pitch * index as f32;
        let centre_z = z0 + pitch * 0.5;
        let seed = stable_hash(&format!("cut-kerb-{:.1}-{z0:.2}", run.x));
        let shade = cut_kerbstone_shade(cut_sounding_sag(run.bank, centre_z), seed);
        let (top, height) = cut_kerbstone_top(run.bank, centre_z, seed);
        let (top, height) = (CUT_STEP_M + top, CUT_STEP_M + height);
        mesh.set_brush([shade; 3]);
        add_dressed_stone(
            mesh,
            Vec3::new(run.x, top - height * 0.5, centre_z),
            Vec3::new(
                CUT_KERB_WIDTH_M * 0.5,
                height * 0.5,
                (pitch - 0.035).max(0.05) * 0.5,
            ),
        );
    }
    mesh.reset_brush();
}

/// A marked stretch: the same line, flush. One block per `CUT_MARKER_PITCH_M`,
/// snapped to the pitch so the two squares' markings share one grid and the
/// line reads as continuous with the reaches either end of it.
fn add_kerb_marking(mesh: &mut MeshData, run: CutKerbRun) {
    let first = (run.z0 / CUT_MARKER_PITCH_M).ceil() * CUT_MARKER_PITCH_M;
    let mut z = first;
    while z <= run.z1 - 0.9 {
        let seed = stable_hash(&format!("cut-mark-{:.1}-{z:.1}", run.x));
        mesh.set_brush([0.84 + (seed % 11) as f32 * 0.01; 3]);
        add_dressed_stone(
            mesh,
            Vec3::new(run.x, CUT_MARKER_TOP_Y * 0.5, z + 0.45),
            Vec3::new(CUT_KERB_WIDTH_M * 0.5, CUT_MARKER_TOP_Y * 0.5, 0.45),
        );
        z += CUT_MARKER_PITCH_M;
    }
    mesh.reset_brush();
}

/// An axis-aligned block of masonry, wound so that every face's triangle order
/// agrees with the normal it carries, and textured from world position.
///
/// The general-purpose `add_oriented_box` does neither, and both matter here.
/// Its faces are wound inside-out and only draw at all because every city
/// material is `double_sided` with `cull_mode: None` — and *that* is what makes
/// them dark: double-sided shading flips the normal it lights a back face with,
/// which nobody notices on a wall seen against the sky but turns a horizontal
/// top face lit by a low sun black. A kerbstone is nearly all top face. Its
/// per-face UVs also start at zero, so every one of the nine hundred stones
/// would sample the same corner of the limestone and the line would strobe;
/// running the UV off world position instead lets the courses carry along the
/// street.
fn add_dressed_stone(mesh: &mut MeshData, center: Vec3, half: Vec3) {
    const UV_SPAN: f32 = 2.4;
    // `right × up == normal` for each face, so the emitted winding is outward.
    for (normal, right, up) in [
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::Y, Vec3::X),
    ] {
        let face_center = center + normal * half.dot(normal.abs());
        let (half_r, half_u) = (half.dot(right.abs()), half.dot(up.abs()));
        let points = [
            face_center - right * half_r - up * half_u,
            face_center + right * half_r - up * half_u,
            face_center + right * half_r + up * half_u,
            face_center - right * half_r + up * half_u,
        ];
        mesh.quad(
            points,
            normal,
            points.map(|point| {
                Vec2::new(point.dot(right) / UV_SPAN, point.dot(up) / UV_SPAN)
            }),
        );
    }
}

/// A blocked water stair — `the_cut_kerb.md` M1, and the strongest single
/// object the margin carries.
///
/// The Serle ran where the cartway is. Every warehouse on the bank had steps
/// down to it, and when the channel was filled the steps were not taken up:
/// they were left to be buried by the made ground the carts now run on. So the
/// flight starts high, on the last 0.40 m of true bank still standing at the
/// housefront side of the margin, drops six treads that each lose a little
/// width to the battering cheek walls, crosses the kerb line — which is why the
/// kerb *breaks* here rather than running across the stair's head — and then
/// dies: the last two slabs come out barely a finger above the cartway and stop
/// in the middle of nothing.
///
/// That is the whole of the argument. A single leaning house tells the player
/// nothing because houses lean everywhere; a stair walking down into a street
/// tells them there was water here, and it is the same claim the kerb's sag
/// (M2) will publish along the whole 748 m.
///
/// The mooring ring is set in a **mooring stone** — a raised block in the line,
/// four and a half kerb heights tall — and not in the kerb face the drawing
/// shows: when the kerb shipped 0.10 m proud (M0) a face-set ring would have
/// been a ring underground, and M3's stepped kerb does not change the
/// arithmetic, because the stone belongs to the *old* bank and keeps its
/// absolute height while the made ground rises around it (see
/// `CUT_MOORING_STONE_Y`) — still a head over the stepped line, its ring still
/// at rope height off the cartway. A mooring ring on a raised stone at the head
/// of a stair is what the thing actually looked like anyway.
fn add_water_stair(stone: &mut MeshData, iron: &mut MeshData, bank: CutBank, z: f32) {
    let (kerb_x, out) = (bank.kerb_x(), bank.outward());
    let seed = cut_stair_seed(bank, z);
    // Five stairs cut to one drawing would be five copies of one object seen
    // along a straight street. The width, the height of the cheeks' coping and
    // which hand the mooring stone stands on all come off the seed, so no two
    // of them are the same flight and none of them is a different idea.
    let half_head = cut_stair_half_head(seed);
    let coping = 0.30 + ((seed >> 3) % 11) as f32 * 0.008;
    let mooring_side = if seed & 0x20 == 0 { 1.0 } else { -1.0 };

    // The head landing: the surviving lip of the old bank, two and a bit metres
    // of it, standing clear of the housefronts by three metres so no door opens
    // onto a drop.
    let landing_near = kerb_x + out * 1.15;
    let landing_far = kerb_x + out * 3.45;
    stone.set_brush([0.78 + (seed % 9) as f32 * 0.008; 3]);
    add_dressed_stone(
        stone,
        Vec3::new(
            (landing_near + landing_far) * 0.5,
            CUT_BANK_TOP_Y * 0.5,
            z,
        ),
        Vec3::new(
            (landing_far - landing_near).abs() * 0.5,
            CUT_BANK_TOP_Y * 0.5,
            half_head,
        ),
    );

    // Treads, then the two drowned slabs. `tread_top` walks the bank height
    // down to zero in `CUT_STAIR_TREADS + 1` equal risers, so the last real
    // tread is one riser above grade and the fill has already all but taken it.
    let mut near = landing_near;
    for index in 0..CUT_STAIR_TREADS {
        let far = near;
        near = kerb_x + out * (1.15 - CUT_STAIR_TREAD_M * (index + 1) as f32);
        let top = CUT_BANK_TOP_Y
            * (1.0 - (index + 1) as f32 / (CUT_STAIR_TREADS + 1) as f32);
        let half_z = half_head - CUT_STAIR_BATTER_M * (index + 1) as f32;
        stone.set_brush([0.74 + ((seed >> index) % 11) as f32 * 0.009; 3]);
        add_dressed_stone(
            stone,
            Vec3::new((near + far) * 0.5, top * 0.5, z),
            Vec3::new((far - near).abs() * 0.5, top * 0.5, half_z),
        );
    }
    for index in 0..2 {
        let far = near;
        near = kerb_x + out * (1.15 - CUT_STAIR_TREAD_M * (CUT_STAIR_TREADS + index + 1) as f32);
        let top = CUT_MARGIN_Y - 0.002 * index as f32;
        let half_z = half_head
            - CUT_STAIR_BATTER_M * (CUT_STAIR_TREADS + index + 1) as f32
            - 0.25 * index as f32;
        stone.set_brush([0.58 - 0.05 * index as f32; 3]);
        add_dressed_stone(
            stone,
            Vec3::new((near + far) * 0.5, top * 0.5, z),
            Vec3::new((far - near).abs() * 0.5, top * 0.5, half_z),
        );
    }

    // The cheek walls, one course per tread, battering inward and downward with
    // the flight. They are what makes a stair read as a stair from thirty metres
    // rather than as a stack of paving.
    for side in [-1.0_f32, 1.0] {
        let mut near = landing_near + out * 2.30;
        for index in 0..=CUT_STAIR_TREADS {
            let far = near;
            near = kerb_x + out * (1.15 - CUT_STAIR_TREAD_M * index as f32);
            let (top, half_z) = if index == 0 {
                (CUT_BANK_TOP_Y, half_head)
            } else {
                (
                    CUT_BANK_TOP_Y * (1.0 - index as f32 / (CUT_STAIR_TREADS + 1) as f32),
                    half_head - CUT_STAIR_BATTER_M * index as f32,
                )
            };
            let crest = top + coping;
            stone.set_brush([0.70 + ((seed >> (index + 4)) % 13) as f32 * 0.008; 3]);
            add_dressed_stone(
                stone,
                Vec3::new((near + far) * 0.5, crest * 0.5, z + side * (half_z + 0.17)),
                Vec3::new((far - near).abs() * 0.5, crest * 0.5, 0.17),
            );
        }
    }

    // The mooring stone stands in the line at the head of the flight, inside the
    // gap `CutProp::kerb_gap` opened for it.
    let mooring_z = z + mooring_side * (half_head + CUT_MOORING_STANDOFF_M);
    stone.set_brush([0.72; 3]);
    add_dressed_stone(
        stone,
        Vec3::new(kerb_x, CUT_MOORING_STONE_Y * 0.5, mooring_z),
        Vec3::new(0.19, CUT_MOORING_STONE_Y * 0.5, CUT_MOORING_STONE_HALF_Z),
    );
    stone.reset_brush();

    // Staple and ring on the cartway face, where a rope came up out of the water.
    let face_x = kerb_x - out * 0.19;
    add_dressed_stone(
        iron,
        Vec3::new(face_x - out * 0.035, 0.34, mooring_z),
        Vec3::new(0.035, 0.045, 0.05),
    );
    add_iron_ring(
        iron,
        Vec3::new(face_x - out * 0.075, 0.215, mooring_z),
        Vec3::X,
        0.125,
        0.028,
    );
}

/// A ring of iron standing in the plane whose normal is `axis`, drawn as twelve
/// short bars around the circle. A torus would need its own mesh asset and its
/// own entity; twelve cubes in a batch cost nothing and, at the two metres this
/// is ever read from, are a ring.
fn add_iron_ring(iron: &mut MeshData, center: Vec3, axis: Vec3, radius: f32, bar: f32) {
    let (u, v) = if axis.x.abs() > 0.5 {
        (Vec3::Y, Vec3::Z)
    } else {
        (Vec3::X, Vec3::Z)
    };
    for index in 0..12 {
        let angle = index as f32 * PI / 6.0;
        add_dressed_stone(
            iron,
            center + (u * angle.cos() + v * angle.sin()) * radius,
            Vec3::splat(bar),
        );
    }
}

/// A stone post set into the kerb line at a square's threshold or beside a
/// bridge pier — `the_cut_kerb.md` §3, drafted from the start as the one piece
/// of margin furniture that would be **collided**.
///
/// M1 shipped it drawn-only, deferring the collider to M3 with the reasoning
/// kept in the feature doc: colliding anything here costs the whole four-step
/// nav chain, and a single solid post on a street of walk-through stairs and
/// hatches would have taught "some of this is real", which is worse than
/// nothing being solid. **M3 is where both arguments expire at once.** The
/// riser makes the street's stone real under hands and feet — the margin is a
/// step you climb through a break, not a texture — so a post that stops you is
/// now consistent rather than an exception; and the riser forces the rebake
/// regardless, so the bollards ride a chain that is being re-run and re-pinned
/// anyway. The cost that remains is the one M1 measured and it is unchanged:
/// each 0.42 m post erodes to a roughly one-metre hole in the walkable grid,
/// sixteen holes that sever nothing across a 10 m cartway and a 6.7 m margin
/// (the eroded bollard merely seals its own 0.7 m slot in the riser line,
/// which was never a crossing). `the_cut_margin_stays_connected_to_its_cartway`
/// proves the street survives them.
fn add_bollard(
    stone: &mut MeshData,
    collision_world: &mut CollisionWorld,
    bank: CutBank,
    z: f32,
) {
    let x = bank.kerb_x();
    let seed = stable_hash(&format!("cut-bollard-{x:.1}-{z:.1}"));
    stone.set_brush([0.70 + (seed % 12) as f32 * 0.009; 3]);
    // A plinth, three courses that taper, and a chamfered cap: a turned post
    // would need a cylinder mesh and therefore its own entity, and this reads
    // the same from the cartway.
    let courses = [
        (0.000, 0.110, 0.210),
        (0.110, 0.370, 0.155),
        (0.370, 0.630, 0.135),
        (0.630, CUT_BOLLARD_Y - 0.09, 0.115),
        (CUT_BOLLARD_Y - 0.09, CUT_BOLLARD_Y, 0.075),
    ];
    for (bottom, top, half) in courses {
        add_dressed_stone(
            stone,
            Vec3::new(x, (bottom + top) * 0.5, z),
            Vec3::new(half, (top - bottom) * 0.5, half),
        );
    }
    stone.reset_brush();
    // One box at the plinth's own girth. The plinth is the widest course, so
    // the collider never pokes out of the drawn stone.
    collision_world.add_box(
        Vec3::new(x - 0.21, 0.0, z - 0.21),
        Vec3::new(x + 0.21, CUT_BOLLARD_Y, z + 0.21),
    );
}

/// A lawful crossing of the line: three metres of the same stone laid flush,
/// where a warehouse door faces the street.
///
/// Every one of these is at a door the plan actually puts on a Cut-facing
/// façade — `plan_facade_openings` sets a building's door at the midpoint of
/// its `door_edge`, and `build_hoist_gantries` rigs its beam over the same
/// doorway — so a break is always the ground under a hoist and a doorway, not a
/// gap somebody spaced along the street. It is drawn in the *marking* batch
/// rather than the kerbstone one because that is exactly what it is: the same
/// line, with the ridge taken out of it.
fn add_kerb_break(markings: &mut MeshData, iron: &mut MeshData, bank: CutBank, z: f32) {
    let x = bank.kerb_x();
    let half = CUT_KERB_BREAK_M * 0.5;
    markings.set_brush([0.80; 3]);
    add_dressed_stone(
        markings,
        Vec3::new(x, CUT_MARKER_TOP_Y * 0.5, z),
        Vec3::new(CUT_KERB_WIDTH_M * 0.5, CUT_MARKER_TOP_Y * 0.5, half),
    );
    markings.reset_brush();
    // M3: the laid stone pitch behind the sill, climbing the quarter-metre
    // from the cartway's level up to the flags over the two lanes
    // `cut_furniture_flag_gaps` keeps clear of flagging. Four slabs rather
    // than one sloped wedge: `add_dressed_stone` is axis-aligned, the
    // profile under feet (`CutMarginProfile`) is the true incline, and four
    // 6 cm courses read as a pitched crossing an iron-shod wheel has been
    // taking for a hundred years. Each drawn top is the walked line at the
    // course's *centre* plus the flags' own 3 cm seat, and the walked line
    // falls ~5.2 cm across one course, so feet seat 0.4–5.6 cm into the
    // drawn stone — worst at each course's near edge — against the flat
    // 3 cm the flags give feet everywhere. Invisible at the 6 cm scale of
    // the risers themselves; a fifth course would buy back two centimetres
    // nobody would ever see.
    let out = bank.outward();
    let run = (CUT_BREAK_RAMP_RUN_M - CUT_KERB_WIDTH_M * 0.5) / 4.0;
    for course in 0..4 {
        let near = CUT_KERB_WIDTH_M * 0.5 + run * course as f32;
        let centre_u = near + run * 0.5;
        let top = CUT_STEP_M * ((centre_u + CUT_KERB_WIDTH_M * 0.5)
            / (CUT_BREAK_RAMP_RUN_M + CUT_KERB_WIDTH_M * 0.5))
            + CUT_MARGIN_Y;
        markings.set_brush([0.76 + course as f32 * 0.012; 3]);
        add_dressed_stone(
            markings,
            Vec3::new(x + out * centre_u, (top - 0.02) * 0.5, z),
            Vec3::new(run * 0.5, (top + 0.02) * 0.5, half),
        );
    }
    markings.reset_brush();
    // Iron over each end, where iron-shod wheels have been crossing the line
    // for as long as the house behind it has had a door.
    for side in [-1.0_f32, 1.0] {
        add_dressed_stone(
            iron,
            Vec3::new(x, CUT_MARKER_TOP_Y + 0.006, z + side * (half - 0.22)),
            Vec3::new(CUT_KERB_WIDTH_M * 0.5 + 0.02, 0.008, 0.22),
        );
    }
}

/// A double-leaf cellar hatch let flush into the margin against a housefront —
/// the goods door of the undercroft, which on the Cut is the part of the
/// building that is *older than the street*, because the undercrofts were cut
/// into the bank before the channel was filled.
///
/// Placed a metre out from the façade line and never within reach of the
/// building's own doorway: `plan_facade_openings` puts that at the midpoint of
/// the door edge, and every z in `CUT_FURNITURE` is offset clear of it.
fn add_cellar_hatch(
    stone: &mut MeshData,
    leaves: &mut MeshData,
    iron: &mut MeshData,
    bank: CutBank,
    z: f32,
) {
    // M3: the hatch rides the margin up wholesale — its surround stays the
    // same 8 mm proud of the flags it always was, just a quarter-metre higher.
    let x = bank.facade_x() - bank.outward() * 0.95;
    stone.set_brush([0.66; 3]);
    add_dressed_stone(
        stone,
        Vec3::new(x, CUT_STEP_M + 0.019, z),
        Vec3::new(0.95, 0.019, 1.00),
    );
    stone.reset_brush();
    for side in [-1.0_f32, 1.0] {
        leaves.set_brush([0.62 + side * 0.03; 3]);
        add_dressed_stone(
            leaves,
            Vec3::new(x, CUT_STEP_M + 0.045, z + side * 0.47),
            Vec3::new(0.72, 0.007, 0.44),
        );
        // A strap hinge along the outer edge of each leaf, and the ring the
        // cellarman lifts it by.
        add_dressed_stone(
            iron,
            Vec3::new(x, CUT_STEP_M + 0.054, z + side * 0.86),
            Vec3::new(0.60, 0.006, 0.045),
        );
        add_iron_ring(
            iron,
            Vec3::new(x - bank.outward() * 0.42, CUT_STEP_M + 0.056, z + side * 0.20),
            Vec3::Y,
            0.075,
            0.018,
        );
    }
    leaves.reset_brush();
}

/// A barred stone vent at the foot of a housefront — the cheap answer where a
/// hatch was never worth cutting. Half a metre of light and air for a cellar
/// that is below the old river level and knows it.
fn add_cellar_vent(stone: &mut MeshData, iron: &mut MeshData, bank: CutBank, z: f32) {
    // M3: the vent stands on the raised margin, so the whole assembly rides up
    // by the step — a mouth half-buried in the new flags would be a cellar
    // that had flooded itself.
    let out = bank.outward();
    let x = bank.facade_x() - out * 0.30;
    stone.set_brush([0.63; 3]);
    add_dressed_stone(
        stone,
        Vec3::new(x, CUT_STEP_M + 0.26, z),
        Vec3::new(0.28, 0.26, 0.55),
    );
    // The mouth: a recessed dark face, then five bars across it.
    stone.set_brush([0.16; 3]);
    add_dressed_stone(
        stone,
        Vec3::new(x - out * 0.20, CUT_STEP_M + 0.28, z),
        Vec3::new(0.09, 0.14, 0.36),
    );
    stone.reset_brush();
    for index in 0..5 {
        let across = (index as f32 / 4.0 - 0.5) * 0.62;
        add_dressed_stone(
            iron,
            Vec3::new(x - out * 0.285, CUT_STEP_M + 0.28, z + across),
            Vec3::new(0.018, 0.13, 0.021),
        );
    }
}

fn build_osanne_stall(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    spawn_market_stall(
        commands,
        meshes,
        materials,
        collision_world,
        Vec3::new(12.6, 0.0, 245.0),
        Vec2::new(5.2, 3.0),
        -8.0_f32.to_radians(),
        1,
        "Osanne Vell's stall".into(),
    );
}

fn build_wharf_cranes(commands: &mut Commands, meshes: &CityMeshes, materials: &CityMaterials) {
    for (index, z) in [36.4, -123.2, -282.8].into_iter().enumerate() {
        spawn_yard_crane(
            commands,
            meshes,
            materials,
            Vec3::new(-430.6, 0.0, z),
            PI * 0.5,
            &format!("Outer Serle wharf crane {}", index + 1),
        );
    }
}

/// Timber galleries spanning the narrower streets at first-floor height — the
/// bridges full of onlookers in `the_bellstand_001.png`. A gallery only spawns
/// where a 2+-storey building actually stands on each side, so every one is
/// seated in masonry rather than floating.
fn build_street_galleries(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    let tall_building_at = |point: Vec2| {
        plan.buildings.iter().any(|building| {
            building.levels >= 2
                && building.use_name != "bridge"
                && point_in_polygon(point, &building.polygon)
        })
    };

    let mut built = 0;
    for road in &plan.roads {
        if !(2.0..=5.5).contains(&road.width_m) {
            continue;
        }
        for (segment_index, pair) in road.points.windows(2).enumerate() {
            let a = Vec2::from_array(pair[0]);
            let b = Vec2::from_array(pair[1]);
            let length = a.distance(b);
            if length < 12.0 {
                continue;
            }
            let hash = stable_hash(&format!("gallery-{}-{segment_index}", road.id));
            if hash % 2 != 0 {
                continue;
            }
            let t = 0.3 + (hash % 41) as f32 / 100.0;
            let center = a.lerp(b, t);
            let street_dir = (b - a) / length;
            let across = Vec2::new(-street_dir.y, street_dir.x);
            // Both flanks must carry a tall building right at the street edge;
            // façades sit at varying setbacks, so probe outward until one is
            // found and seat the gallery that deep into it.
            let seat_depth = |side: f32| {
                [0.7_f32, 1.6, 2.6].into_iter().find(|extra| {
                    tall_building_at(center + across * side * (road.width_m * 0.5 + extra))
                })
            };
            let (Some(seat_a), Some(seat_b)) = (seat_depth(1.0), seat_depth(-1.0)) else {
                continue;
            };

            let span = road.width_m + seat_a + seat_b + 1.6;
            let yaw = (-across.y).atan2(across.x);
            let floor_y = 4.55;
            let shifted = center + across * (seat_a - seat_b) * 0.5;
            let base = Vec3::new(shifted.x, floor_y, shifted.y);
            spawn_rotated_box_named(
                commands,
                meshes,
                &materials.timber,
                base,
                Vec3::new(span, 0.26, 2.5),
                yaw,
                format!("Street gallery over {}", road.name),
            );
            // Half-timbered parapet walls and a slate hood.
            for side in [-1.0, 1.0] {
                spawn_rotated_box_named(
                    commands,
                    meshes,
                    &materials.half_timber,
                    base + Vec3::new(street_dir.x, 0.0, street_dir.y) * side * 1.13
                        + Vec3::Y * 1.05,
                    Vec3::new(span, 1.85, 0.24),
                    yaw,
                    "Street gallery parapet",
                );
            }
            spawn_rotated_box_named(
                commands,
                meshes,
                &materials.slate,
                base + Vec3::Y * 2.35,
                Vec3::new(span + 0.6, 0.16, 3.1),
                yaw,
                "Street gallery roof",
            );
            spawn_rotated_box_named(
                commands,
                meshes,
                &materials.slate,
                base + Vec3::Y * 2.55,
                Vec3::new(span + 0.6, 0.14, 1.1),
                yaw,
                "Street gallery roof ridge",
            );
            add_rotated_box_collider_at(collision_world, base, Vec3::new(span, 0.26, 2.5), yaw);
            built += 1;
        }
    }
    info!("spanned {built} street galleries");
}

/// Ground-floor arcade strips on the buildings that front the town squares:
/// timber posts at ~2.4 m spacing carrying a beam and a slate pentice roof,
/// with the walkable colonnade between the posts and the façade. The posts are
/// scenery like the street props — the baked navigation predates them, so they
/// must not collide — and the roof rides above head height, so nothing at
/// street level changes.
fn build_square_arcades(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
) {
    let squares: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| site.kind == "square")
        .collect();
    let mut woodwork = MeshData::default();
    let mut hoods = MeshData::default();
    let mut arcaded = 0;

    for building in &plan.buildings {
        let (base_y, eave_y) = building_verticals(building);
        // Towers, gates, churches and the overhead shells keep their bare
        // faces; the arcade belongs to the ordinary fabric around a square.
        if base_y > 0.1
            || eave_y < 6.0
            || building.use_name == "bridge"
            || building.id == "named_bellstand_tower"
            || building.id == "named_saint_marens"
            || building.id == "named_old_sluice"
            || building.id.starts_with("gate_")
            || building.id.starts_with("reserve_church_")
        {
            continue;
        }
        let tint = building_tint(building);
        let door_edge = door_edges.get(&building.id).copied();
        let orientation = plan::signed_area(&building.polygon).signum();
        for (edge_index, (a, b)) in building
            .polygon
            .iter()
            .zip(building.polygon.iter().cycle().skip(1))
            .enumerate()
        {
            let a2 = Vec2::from_array(*a);
            let b2 = Vec2::from_array(*b);
            let edge = b2 - a2;
            let length = edge.length();
            if length < 4.5 {
                continue;
            }
            let direction = edge / length;
            let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
            if orientation < 0.0 {
                normal2 = -normal2;
            }
            // The colonnade must stand on the square itself.
            let probe = a2 + direction * (length * 0.5) + normal2 * 1.6;
            if !squares
                .iter()
                .any(|square| point_in_polygon(probe, &square.polygon))
            {
                continue;
            }

            woodwork.set_brush(tint);
            hoods.set_brush(tint);
            let door_here = door_edge == Some(edge_index);
            let count = ((length - 1.6) / 2.4).floor().max(1.0) as usize;
            for index in 0..count {
                let along = length * (index as f32 + 1.0) / (count as f32 + 1.0);
                // Never plant a post in front of the doorway.
                if door_here && (along - length * 0.5).abs() < 1.6 {
                    continue;
                }
                let foot = a2 + direction * along + normal2 * 1.35;
                add_oriented_box(
                    &mut woodwork,
                    Vec3::new(foot.x, 1.62, foot.y),
                    Vec3::new(0.09, 1.62, 0.09),
                    direction,
                );
            }
            // The beam the posts carry.
            let beam_center = a2 + direction * (length * 0.5) + normal2 * 1.35;
            add_oriented_box(
                &mut woodwork,
                Vec3::new(beam_center.x, 3.33, beam_center.y),
                Vec3::new(length * 0.5 - 0.55, 0.09, 0.11),
                direction,
            );
            woodwork.reset_brush();

            // The pentice: a slate strip pitched off the façade over the walk.
            let inner_a = a2 + direction * 0.25;
            let inner_b = a2 + direction * (length - 0.25);
            let outer_a = inner_a + normal2 * 1.62;
            let outer_b = inner_b + normal2 * 1.62;
            let slope = Vec3::new(normal2.x * 1.62, -0.45, normal2.y * 1.62);
            let mut roof_normal = slope
                .cross(Vec3::new(direction.x, 0.0, direction.y))
                .normalize_or(Vec3::Y);
            if roof_normal.y < 0.0 {
                roof_normal = -roof_normal;
            }
            hoods.quad(
                [
                    Vec3::new(inner_a.x, 3.78, inner_a.y),
                    Vec3::new(inner_b.x, 3.78, inner_b.y),
                    Vec3::new(outer_b.x, 3.33, outer_b.y),
                    Vec3::new(outer_a.x, 3.33, outer_a.y),
                ],
                roof_normal,
                [
                    Vec2::ZERO,
                    Vec2::new(length / 7.0, 0.0),
                    Vec2::new(length / 7.0, 0.25),
                    Vec2::new(0.0, 0.25),
                ],
            );
            hoods.reset_brush();
            arcaded += 1;
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        woodwork,
        "Square arcade posts",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.slate,
        hoods,
        "Square arcade hoods",
    );
    info!("raised {arcaded} arcade strips on the squares");
}

/// The street-life kit: barrels, crates, sacks, firewood and hanging signs
/// hugging the façades beside doors. Everything is merged into five batched
/// meshes (one per material), and none of it collides: the baked navigation
/// predates these, so they are scenery for the eye, not walls for the feet —
/// exactly like the NPCs, which never collide with props either.
///
/// Each clutter spot stands on the sampled street ground rather than on
/// `y = 0`: the Cut's raised margin (`CutMarginProfile`, M3) runs its flags
/// at a quarter-metre, and a rick or sack pair seated at grade there is
/// two-thirds buried in the paving. One sample per spot, not per piece — a
/// cluster straddling a feather edge should lean as one thing, not tear.
fn build_street_props(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
) {
    let mut timber = MeshData::default();
    let mut iron = MeshData::default();
    let mut dark_wood = MeshData::default();
    let mut ochre = MeshData::default();
    let mut russet = MeshData::default();
    let mut placed = 0;
    let cut_profile = cut_margin_profile(plan);

    for building in &plan.buildings {
        let Some(&edge_index) = door_edges.get(&building.id) else {
            continue;
        };
        let hash = stable_hash(&building.id).rotate_left(9);
        // Two thirds of doorways get something standing outside.
        if hash % 3 == 0 {
            continue;
        }
        let polygon = &building.polygon;
        let a = Vec2::from_array(polygon[edge_index]);
        let b = Vec2::from_array(polygon[(edge_index + 1) % polygon.len()]);
        let edge = b - a;
        let length = edge.length();
        if length < 4.5 {
            continue;
        }
        let direction = edge / length;
        let orientation = plan::signed_area(polygon).signum();
        let mut normal = Vec2::new(edge.y, -edge.x).normalize() * orientation;
        // `plan` polygons wind either way; make sure the props step outward.
        if point_in_polygon(a + direction * (length * 0.5) + normal * 0.5, polygon) {
            normal = -normal;
        }
        let door = a + direction * (length * 0.5);

        // One or two clutter spots flanking the door, tight to the wall.
        let spots = if hash % 5 < 2 { 2 } else { 1 };
        for spot in 0..spots {
            let side = if (spot == 0) == (hash % 2 == 0) {
                1.0
            } else {
                -1.0
            };
            let spot_hash = hash.rotate_left(5 + spot as u32 * 7) ^ 0xA5A5_5A5A;
            let along_offset = 1.6 + (spot_hash % 90) as f32 / 100.0;
            let position2 = door + direction * side * along_offset + normal * 0.55;
            let lift = cut_profile.ground_lift(position2.x, position2.y);
            let position = Vec3::new(position2.x, lift, position2.y);
            match spot_hash % 4 {
                // A barrel, sometimes two.
                0 => {
                    add_barrel(&mut timber, &mut iron, position);
                    if spot_hash % 7 == 0 {
                        add_barrel(
                            &mut timber,
                            &mut iron,
                            position + Vec3::new(direction.x, 0.0, direction.y) * side * 0.72,
                        );
                    }
                }
                // Crates, stacked when the hash feels like it.
                1 => {
                    let skew = rotate2(direction, (spot_hash % 7) as f32 * 0.1);
                    add_oriented_box(
                        &mut timber,
                        position + Vec3::Y * 0.29,
                        Vec3::new(0.30, 0.29, 0.30),
                        skew,
                    );
                    if spot_hash % 3 == 0 {
                        add_oriented_box(
                            &mut timber,
                            position + Vec3::Y * 0.82,
                            Vec3::new(0.25, 0.24, 0.25),
                            rotate2(skew, 0.35),
                        );
                    }
                }
                // Sacks slumped against the wall.
                2 => {
                    let cloth = if spot_hash % 2 == 0 {
                        &mut ochre
                    } else {
                        &mut russet
                    };
                    for (offset, squash) in [(Vec2::ZERO, 0.24), (Vec2::new(0.42, 0.06), 0.19)] {
                        let sack2 = position2 + direction * offset.x + normal * offset.y;
                        add_sack(
                            cloth,
                            Vec3::new(sack2.x, lift + squash * 0.8, sack2.y),
                            Vec3::new(0.30, squash, 0.27),
                        );
                    }
                }
                // A firewood rick: split logs against the plinth.
                _ => {
                    for row in 0..3 {
                        for column in 0..2 {
                            add_log(
                                &mut dark_wood,
                                firewood_log_center(position2, normal, row, column)
                                    + Vec3::Y * lift,
                                0.115,
                                1.05,
                                direction,
                            );
                        }
                    }
                }
            }
            placed += 1;
        }

        // A hanging trade sign over some doors: bracket arm and swinging board.
        if hash % 8 == 0 {
            let arm_center = door + normal * 0.55;
            add_oriented_box(
                &mut iron,
                Vec3::new(arm_center.x, 3.35, arm_center.y),
                Vec3::new(0.03, 0.03, 0.5),
                direction,
            );
            let board2 = door + normal * 0.78;
            add_oriented_box(
                &mut dark_wood,
                Vec3::new(board2.x, 2.78, board2.y),
                Vec3::new(0.29, 0.33, 0.03),
                direction,
            );
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        timber,
        "Street props: cooperage",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.iron,
        iron,
        "Street props: ironmongery",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        dark_wood,
        "Street props: firewood and signs",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_ochre,
        ochre,
        "Street props: sacks",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_russet,
        russet,
        "Street props: more sacks",
    );
    info!("scattered {placed} doorway prop clusters");
}

/// Sloped canvas awnings over a quarter of the trade-house doors — the
/// street-level cloth of reference image A. Each is a sagging 3x3 vertex
/// sheet pitched off the facade with a batten or two under its side hems,
/// on the hemp canvas artwork with a per-awning dye brush. Two
/// city-wide batches, no colliders and never a ground post: the outer hem
/// rides at 2.1 m or higher, above the walk band, so the baked navigation
/// never hears about them.
fn build_shopfront_awnings(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
) {
    let squares: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| site.kind == "square")
        .collect();
    // The overhead keep-outs the laundry lines use: the bridge shells and the
    // malt house with swing room, plus every street-gallery candidate spot —
    // an over-approximation (candidates whose seat probes failed never got a
    // gallery) that only thins the awnings near those streets.
    let shell_boxes: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .filter(|building| building_verticals(building).0 > 0.1)
        .map(|building| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for point in &building.polygon {
                min = min.min(Vec2::from_array(*point));
                max = max.max(Vec2::from_array(*point));
            }
            (min - Vec2::splat(1.2), max + Vec2::splat(1.2))
        })
        .collect();
    let gallery_spots: Vec<(Vec2, f32)> = plan
        .roads
        .iter()
        .filter(|road| (2.0..=5.5).contains(&road.width_m))
        .flat_map(|road| {
            road.points
                .windows(2)
                .enumerate()
                .filter_map(move |(segment_index, pair)| {
                    let a = Vec2::from_array(pair[0]);
                    let b = Vec2::from_array(pair[1]);
                    if a.distance(b) < 12.0 {
                        return None;
                    }
                    let hash = stable_hash(&format!("gallery-{}-{segment_index}", road.id));
                    if hash % 2 != 0 {
                        return None;
                    }
                    let t = 0.3 + (hash % 41) as f32 / 100.0;
                    Some((a.lerp(b, t), road.width_m * 0.5 + 4.2))
                })
        })
        .collect();
    let overhead_blocked = |point: Vec2| {
        shell_boxes.iter().any(|(min, max)| {
            point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
        }) || gallery_spots
            .iter()
            .any(|(center, reach)| center.distance_squared(point) < reach * reach)
    };
    // Market stalls raise their own cloth to 2.5 m: no awning over a stall.
    let fixture_near = |point: Vec2| {
        plan.fixtures.iter().any(|fixture| {
            let angle = fixture.angle_deg.to_radians();
            let delta = point - Vec2::from_array(fixture.position);
            let local_x = delta.x * angle.cos() - delta.y * angle.sin();
            let local_z = delta.x * angle.sin() + delta.y * angle.cos();
            local_x.abs() <= fixture.size[0] * 0.5 + 0.6
                && local_z.abs() <= fixture.size[1] * 0.5 + 0.6
        })
    };

    let mut canvas = MeshData::default();
    let mut battens = MeshData::default();
    let mut stretched = 0;

    for building in &plan.buildings {
        if building.use_name != "trade" {
            continue;
        }
        let Some(&edge_index) = door_edges.get(&building.id) else {
            continue;
        };
        let hash = stable_hash(&format!("awning-{}", building.id));
        if hash % 4 != 0 {
            continue;
        }
        let (base_y, eave_y) = building_verticals(building);
        if base_y > 0.1 {
            continue;
        }
        let polygon = &building.polygon;
        let a = Vec2::from_array(polygon[edge_index]);
        let b = Vec2::from_array(polygon[(edge_index + 1) % polygon.len()]);
        let edge = b - a;
        let length = edge.length();
        // Shorter door edges never rendered their door module.
        if length < 3.2 {
            continue;
        }
        // A hanging trade sign's board swings exactly through the canvas:
        // mirror its gate and let the sign keep those doors.
        let sign_hash = stable_hash(&building.id).rotate_left(9);
        if length >= 4.5 && sign_hash % 3 != 0 && sign_hash % 8 == 0 {
            continue;
        }
        // A first-floor open balcony over the door drops its brackets into
        // the same air: mirror that gate too (over-approximated — candidates
        // whose probes failed never hung a deck).
        if length >= 4.5
            && building.levels >= 2
            && stable_hash(&format!("balcony-{}-{edge_index}", building.id)) % 10 == 0
        {
            continue;
        }
        let direction = edge / length;
        let orientation = plan::signed_area(polygon).signum();
        let mut normal = Vec2::new(edge.y, -edge.x).normalize() * orientation;
        // `plan` polygons wind either way; the canvas must pitch outward.
        if point_in_polygon(a + direction * (length * 0.5) + normal * 0.5, polygon) {
            normal = -normal;
        }
        let door = a + direction * (length * 0.5);
        // Square-fronting facades carry arcade pentices over the walk.
        if eave_y >= 6.0
            && length >= 4.5
            && squares
                .iter()
                .any(|square| point_in_polygon(door + normal * 1.6, &square.polygon))
        {
            continue;
        }

        let half_width = (0.9 + ((hash >> 3) % 61) as f32 / 100.0).min(length * 0.5 - 0.7);
        let depth = 0.9 + ((hash >> 9) % 51) as f32 / 100.0;
        // Wall edge above the 2.64 m door lintel; outer hem above the walk
        // band top even before the pitch is counted.
        let wall_y = 2.7 + ((hash >> 14) % 31) as f32 / 100.0;
        let outer_y = (wall_y - 0.5 - ((hash >> 19) % 21) as f32 / 100.0).clamp(2.1, 2.4);
        if overhead_blocked(door)
            || overhead_blocked(door + normal * (0.16 + depth))
            || fixture_near(door + normal * (0.16 + depth))
        {
            continue;
        }

        let dir3 = Vec3::new(direction.x, 0.0, direction.y);
        let out3 = Vec3::new(normal.x, 0.0, normal.y);
        let door3 = Vec3::new(door.x, 0.0, door.y);
        let sag = 0.06 + ((hash >> 24) % 7) as f32 / 100.0;
        let slope = out3 * depth + Vec3::Y * (outer_y - wall_y);
        let mut sheet_normal = slope.cross(dir3).normalize_or(Vec3::Y);
        if sheet_normal.y < 0.0 {
            sheet_normal = -sheet_normal;
        }
        canvas.set_brush(awning_tint(hash.rotate_left(13)));
        // Seamless tile: a door-anchored sample window keeps the repair seams
        // from repeating identically on every awning.
        let uv_shift = Vec2::new((door3.x * 0.31).fract(), (door3.z * 0.23).fract());
        let first = canvas.positions.len() as u32;
        for row in 0..3 {
            let t = row as f32 * 0.5;
            // The mid row bows below the pitch line: cloth, not board. The
            // wall row starts 0.16 out, proud of the deepest framing member.
            let y = wall_y + (outer_y - wall_y) * t - sag * 4.0 * t * (1.0 - t);
            for column in 0..3 {
                canvas.vertex(
                    door3
                        + dir3 * ((column as f32 - 1.0) * half_width)
                        + out3 * (0.16 + depth * t)
                        + Vec3::Y * y,
                    sheet_normal,
                    Vec2::new(column as f32 * 0.5, t) + uv_shift,
                );
            }
        }
        for row in 0..2u32 {
            for column in 0..2u32 {
                let corner = first + row * 3 + column;
                canvas.indices.extend_from_slice(&[
                    corner,
                    corner + 3,
                    corner + 4,
                    corner,
                    corner + 4,
                    corner + 1,
                ]);
            }
        }
        canvas.reset_brush();

        // One or two battens under the sloping side hems, wall to hem edge.
        let single = (hash >> 27) % 3 == 0;
        for (spar, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
            if single && spar as u32 != (hash >> 29) % 2 {
                continue;
            }
            let flank = dir3 * (side * (half_width - 0.06));
            add_awning_batten(
                &mut battens,
                door3 + flank + out3 * 0.14 + Vec3::Y * (wall_y - 0.045),
                door3 + flank + out3 * (0.16 + depth) + Vec3::Y * (outer_y - 0.045),
                0.03,
            );
        }
        stretched += 1;
    }

    spawn_batch(
        commands,
        meshes,
        &materials.canvas,
        canvas,
        "Shopfront awnings",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        battens,
        "Shopfront awning battens",
    );
    info!("stretched {stretched} shopfront awnings");
}

/// Mostly undyed canvas, with the odd faded madder, dull ochre or grey-green
/// sheet — multipliers over the hemp canvas artwork.
fn awning_tint(hash: u32) -> [f32; 3] {
    match hash % 8 {
        0 | 1 => [0.62, 0.33, 0.27],
        2 => [0.72, 0.56, 0.30],
        3 => [0.52, 0.58, 0.50],
        _ => {
            let value = 0.88 + ((hash >> 16) % 13) as f32 / 100.0;
            [value * 1.02, value, value * 0.92]
        }
    }
}

/// A square-sectioned spar between two points — the batten under an awning's
/// side hem. Four long faces and no caps: the 6 cm section never shows its
/// ends.
fn add_awning_batten(mesh: &mut MeshData, from: Vec3, to: Vec3, half: f32) {
    let axis = (to - from).normalize_or_zero();
    if axis == Vec3::ZERO {
        return;
    }
    let side = axis.cross(Vec3::Y).normalize_or(Vec3::X);
    let lift = side.cross(axis).normalize_or(Vec3::Y);
    let length = from.distance(to);
    for (spread, face) in [(side, lift), (lift, side)] {
        for flip in [-1.0, 1.0] {
            let offset = face * (half * flip);
            mesh.quad(
                [
                    from + offset - spread * half,
                    to + offset - spread * half,
                    to + offset + spread * half,
                    from + offset + spread * half,
                ],
                face * flip,
                [
                    Vec2::ZERO,
                    Vec2::new(length / 3.5, 0.0),
                    Vec2::new(length / 3.5, half / 1.75),
                    Vec2::new(0.0, half / 1.75),
                ],
            );
        }
    }
}

/// External timber stairs up to first-floor balconies — the left edge of
/// `the_bellstand_001.png` — on a hash-picked tenth of the taller ordinary
/// houses, and only where the flight provably stands in a yard: clear of every
/// other footprint, every road, every fixture, the squares and the curtain
/// wall. The flight itself is scenery (the baked navigation predates it, like
/// the props); only the landing high above the walk band gets a collider, so
/// a flying player can put down on it.
fn build_yard_stairs(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
    collision_world: &mut CollisionWorld,
) {
    const RUN_ALONG: f32 = 6.4;
    let bounds: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .map(|building| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for point in &building.polygon {
                min = min.min(Vec2::from_array(*point));
                max = max.max(Vec2::from_array(*point));
            }
            (min, max)
        })
        .collect();
    let squares: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| site.kind == "square")
        .collect();

    let clear = |probe: Vec2, skip: usize| {
        plan.buildings.iter().enumerate().all(|(index, building)| {
            index == skip
                || probe.x < bounds[index].0.x
                || probe.y < bounds[index].0.y
                || probe.x > bounds[index].1.x
                || probe.y > bounds[index].1.y
                || !point_in_polygon(probe, &building.polygon)
        }) && plan.roads.iter().all(|road| {
            let margin = road.width_m * 0.5 + 0.8;
            road.points.windows(2).all(|pair| {
                segment_distance_squared(
                    probe,
                    Vec2::from_array(pair[0]),
                    Vec2::from_array(pair[1]),
                ) > margin * margin
            })
        }) && plan.fixtures.iter().all(|fixture| {
            let angle = fixture.angle_deg.to_radians();
            let delta = probe - Vec2::from_array(fixture.position);
            let local_x = delta.x * angle.cos() - delta.y * angle.sin();
            let local_z = delta.x * angle.sin() + delta.y * angle.cos();
            local_x.abs() > fixture.size[0] * 0.5 + 0.8
                || local_z.abs() > fixture.size[1] * 0.5 + 0.8
        }) && !squares
            .iter()
            .any(|square| point_in_polygon(probe, &square.polygon))
            && point_in_polygon(probe, &plan.wall_polygon_xz)
            && plan
                .wall_polygon_xz
                .windows(2)
                .chain(std::iter::once(
                    &[
                        *plan.wall_polygon_xz.last().unwrap(),
                        plan.wall_polygon_xz[0],
                    ][..],
                ))
                .all(|pair| {
                    segment_distance_squared(
                        probe,
                        Vec2::from_array(pair[0]),
                        Vec2::from_array(pair[1]),
                    ) > 3.2 * 3.2
                })
    };

    let mut timber = MeshData::default();
    let mut raised = 0;
    for (building_index, building) in plan.buildings.iter().enumerate() {
        let hash = stable_hash(&building.id).rotate_left(3);
        if building.named || building.levels < 2 || !hash.is_multiple_of(10) {
            continue;
        }
        let door_edge = door_edges.get(&building.id).copied();
        let orientation = plan::signed_area(&building.polygon).signum();
        let edge_count = building.polygon.len();
        let mut placed_here = false;
        for edge_offset in 0..edge_count {
            if placed_here {
                break;
            }
            let edge_index = (edge_offset + hash as usize) % edge_count;
            if door_edge == Some(edge_index) {
                continue;
            }
            let a2 = Vec2::from_array(building.polygon[edge_index]);
            let b2 = Vec2::from_array(building.polygon[(edge_index + 1) % edge_count]);
            let edge = b2 - a2;
            let length = edge.length();
            if length < RUN_ALONG + 0.6 {
                continue;
            }
            let direction = edge / length;
            let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
            if orientation < 0.0 {
                normal2 = -normal2;
            }
            let probes_clear = [0.6_f32, 2.4, 4.2, 6.2]
                .into_iter()
                .flat_map(|along| [0.6_f32, 1.7].map(|out| (along, out)))
                .all(|(along, out)| clear(a2 + direction * along + normal2 * out, building_index));
            if !probes_clear {
                continue;
            }

            add_yard_stair(
                &mut timber,
                collision_world,
                a2,
                direction,
                normal2,
                building_tint(building),
            );
            raised += 1;
            placed_here = true;
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        timber,
        "Yard stairs and balconies",
    );
    info!("raised {raised} yard stairs");
}

/// One straight flight against a wall — stringers, treads, handrail — up to a
/// railed landing at first-floor height, with a dark upper door behind it.
fn add_yard_stair(
    timber: &mut MeshData,
    collision_world: &mut CollisionWorld,
    a2: Vec2,
    direction: Vec2,
    normal2: Vec2,
    tint: [f32; 3],
) {
    const LANDING_Y: f32 = 3.15;
    let at = |along: f32, out: f32| a2 + direction * along + normal2 * out;
    timber.set_brush(tint);

    // Stringers on both flanks of the flight.
    for out in [0.38, 1.22] {
        add_face_member(
            timber,
            a2 + normal2 * out,
            direction,
            normal2,
            Vec2::new(0.4, 0.12),
            Vec2::new(4.6, LANDING_Y - 0.04),
            0.10,
            0.10,
            true,
        );
    }
    // Treads.
    for step in 0..11 {
        let along = 0.66 + step as f32 * 0.36;
        let rise = 0.18 + step as f32 * 0.27;
        let center = at(along, 0.8);
        add_oriented_box(
            timber,
            Vec3::new(center.x, rise, center.y),
            Vec3::new(0.19, 0.032, 0.44),
            direction,
        );
    }
    // Handrail and balusters on the open side.
    add_face_member(
        timber,
        a2 + normal2 * 1.26,
        direction,
        normal2,
        Vec2::new(0.4, 1.07),
        Vec2::new(4.6, LANDING_Y + 0.91),
        0.045,
        0.05,
        true,
    );
    for along in [1.4_f32, 2.6, 3.8] {
        let height = 0.12 + (along - 0.4) / 4.2 * (LANDING_Y - 0.16);
        let foot = at(along, 1.24);
        add_oriented_box(
            timber,
            Vec3::new(foot.x, height + 0.48, foot.y),
            Vec3::new(0.038, 0.48, 0.038),
            direction,
        );
    }

    // The landing: platform, two full-height posts, rails, and the door it
    // serves.
    let platform = at(5.5, 0.85);
    add_oriented_box(
        timber,
        Vec3::new(platform.x, LANDING_Y, platform.y),
        Vec3::new(0.92, 0.055, 0.72),
        direction,
    );
    for along in [4.72_f32, 6.27] {
        let foot = at(along, 1.46);
        add_oriented_box(
            timber,
            Vec3::new(foot.x, 2.05, foot.y),
            Vec3::new(0.07, 2.05, 0.07),
            direction,
        );
    }
    // Outer rail along the balcony edge, and the closed far end.
    add_face_member(
        timber,
        a2 + normal2 * 1.5,
        direction,
        normal2,
        Vec2::new(4.72, LANDING_Y + 0.92),
        Vec2::new(6.27, LANDING_Y + 0.92),
        0.045,
        0.05,
        true,
    );
    let end_origin = at(6.34, 0.0);
    add_face_member(
        timber,
        end_origin,
        normal2,
        direction,
        Vec2::new(0.15, LANDING_Y + 0.92),
        Vec2::new(1.45, LANDING_Y + 0.92),
        0.045,
        0.05,
        true,
    );
    // The dark upper door the stair exists for.
    let door = at(5.5, 0.05);
    timber.set_brush([tint[0] * 0.30, tint[1] * 0.28, tint[2] * 0.26]);
    add_oriented_box(
        timber,
        Vec3::new(door.x, LANDING_Y + 1.08, door.y),
        Vec3::new(0.52, 1.02, 0.05),
        direction,
    );
    timber.reset_brush();

    // Only the landing carries collision — it floats far above the walk band,
    // so the baked navigation below stays honest.
    add_rotated_box_collider_at(
        collision_world,
        Vec3::new(platform.x, LANDING_Y, platform.y),
        Vec3::new(1.84, 0.11, 1.44),
        (-direction.y).atan2(direction.x),
    );
}

/// Open railed balconies — the loggia storeys of the reference courtyard — on
/// the facades that front a court, a square or one of the wider streets (never
/// the narrow lanes: the jetties own those). A hash-picked tenth of the
/// eligible facades carries one; on the 3–4 storey hosts a further minority
/// stacks two, floor over floor. The deck hangs on angled wall brackets —
/// never ground posts, so the street below stays exactly as the baked
/// navigation knows it — and, like the yard-stair landing, only the deck slab
/// collides, far above the walk band, so a flying player can perch on it.
fn build_open_balconies(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
    collision_world: &mut CollisionWorld,
) {
    let bounds: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .map(|building| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for point in &building.polygon {
                min = min.min(Vec2::from_array(*point));
                max = max.max(Vec2::from_array(*point));
            }
            (min, max)
        })
        .collect();
    let building_at = |point: Vec2, skip: usize| {
        plan.buildings.iter().enumerate().any(|(index, other)| {
            index != skip
                && point.x >= bounds[index].0.x
                && point.y >= bounds[index].0.y
                && point.x <= bounds[index].1.x
                && point.y <= bounds[index].1.y
                && point_in_polygon(point, &other.polygon)
        })
    };
    // The overhead keep-outs the laundry lines use: the bridge shells and the
    // malt house with swing room, plus every street-gallery candidate spot.
    let shell_boxes: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .enumerate()
        .filter(|(_, building)| building_verticals(building).0 > 0.1)
        .map(|(index, _)| {
            (
                bounds[index].0 - Vec2::splat(1.2),
                bounds[index].1 + Vec2::splat(1.2),
            )
        })
        .collect();
    let gallery_spots: Vec<(Vec2, f32)> = plan
        .roads
        .iter()
        .filter(|road| (2.0..=5.5).contains(&road.width_m))
        .flat_map(|road| {
            road.points
                .windows(2)
                .enumerate()
                .filter_map(move |(segment_index, pair)| {
                    let a = Vec2::from_array(pair[0]);
                    let b = Vec2::from_array(pair[1]);
                    if a.distance(b) < 12.0 {
                        return None;
                    }
                    let hash = stable_hash(&format!("gallery-{}-{segment_index}", road.id));
                    if hash % 2 != 0 {
                        return None;
                    }
                    let t = 0.3 + (hash % 41) as f32 / 100.0;
                    Some((a.lerp(b, t), road.width_m * 0.5 + 4.2))
                })
        })
        .collect();
    let overhead_blocked = |point: Vec2| {
        shell_boxes.iter().any(|(min, max)| {
            point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
        }) || gallery_spots
            .iter()
            .any(|(center, reach)| center.distance_squared(point) < reach * reach)
    };
    let fixture_near = |point: Vec2| {
        plan.fixtures.iter().any(|fixture| {
            let angle = fixture.angle_deg.to_radians();
            let delta = point - Vec2::from_array(fixture.position);
            let local_x = delta.x * angle.cos() - delta.y * angle.sin();
            let local_z = delta.x * angle.sin() + delta.y * angle.cos();
            local_x.abs() <= fixture.size[0] * 0.5 + 0.6
                && local_z.abs() <= fixture.size[1] * 0.5 + 0.6
        })
    };
    // The open ground a balcony wants to overlook: the squares, the courts and
    // the yards, or a street wide enough that the jetties leave it alone.
    let open_sites: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| matches!(site.kind.as_str(), "square" | "court" | "yard"))
        .collect();
    let squares: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| site.kind == "square")
        .collect();
    let wide_street_at = |probe: Vec2| {
        plan.roads.iter().any(|road| {
            road.width_m >= 5.0
                && road.points.windows(2).any(|pair| {
                    segment_distance_squared(
                        probe,
                        Vec2::from_array(pair[0]),
                        Vec2::from_array(pair[1]),
                    ) < (road.width_m * 0.5).powi(2)
                })
        })
    };

    let mut timber = MeshData::default();
    // Decks already hung this pass, per floor, as facade-aligned rectangles.
    let mut placed: Vec<(u8, Vec2, Vec2, f32, f32)> = Vec::new();
    let mut hung = 0;

    for (building_index, building) in plan.buildings.iter().enumerate() {
        let (base_y, eave_y) = building_verticals(building);
        if building.named
            || building.levels < 2
            || base_y > 0.1
            || matches!(
                building.use_name.as_str(),
                "bridge" | "ecclesiastical" | "fortification"
            )
            // The yard-stair gate: those buildings may carry the flight and
            // its railed landing on any of their facades.
            || stable_hash(&building.id).rotate_left(3).is_multiple_of(10)
        {
            continue;
        }
        let door_edge = door_edges.get(&building.id).copied();
        let orientation = plan::signed_area(&building.polygon).signum();
        let edge_count = building.polygon.len();
        for edge_index in 0..edge_count {
            let hash = stable_hash(&format!("balcony-{}-{edge_index}", building.id));
            if hash % 10 != 0 {
                continue;
            }
            let a2 = Vec2::from_array(building.polygon[edge_index]);
            let b2 = Vec2::from_array(building.polygon[(edge_index + 1) % edge_count]);
            let edge = b2 - a2;
            let length = edge.length();
            if length < 4.5 {
                continue;
            }
            let direction = edge / length;
            let mut normal2 = Vec2::new(edge.y, -edge.x).normalize() * orientation;
            // `plan` polygons wind either way; the deck must hang outward.
            if point_in_polygon(
                a2 + direction * (length * 0.5) + normal2 * 0.5,
                &building.polygon,
            ) {
                normal2 = -normal2;
            }
            // A door edge is welcome — the reference hangs its balconies over
            // doorways — unless the trade-house hoist gantry may rig it, with
            // a load swinging right through the deck's air: mirror that gate.
            if door_edge == Some(edge_index)
                && matches!(building.use_name.as_str(), "trade" | "storage")
                && length >= 5.0
                && stable_hash(&format!("gantry-{}", building.id)) % 8 == 0
            {
                continue;
            }
            let mid = a2 + direction * (length * 0.5);
            let fronts_open_ground = open_sites.iter().any(|site| {
                [1.2_f32, 2.4]
                    .into_iter()
                    .any(|out| point_in_polygon(mid + normal2 * out, &site.polygon))
            }) || [1.5_f32, 3.0]
                .into_iter()
                .any(|out| wide_street_at(mid + normal2 * out));
            if !fronts_open_ground {
                continue;
            }
            // Square-fronting facades carry arcade pentices (top 3.78 m) at
            // exactly the first storey line: those keep the upper floor only.
            let arcade_edge = eave_y >= 6.0
                && squares
                    .iter()
                    .any(|square| point_in_polygon(mid + normal2 * 1.6, &square.polygon));
            let stacked = (3..=4).contains(&building.levels) && (hash >> 17) % 3 == 0;
            let mut floors: Vec<u8> = Vec::new();
            if !arcade_edge {
                floors.push(1);
            }
            if building.levels >= 3 && (stacked || arcade_edge) {
                floors.push(2);
            }
            if floors.is_empty() {
                continue;
            }

            let deck_len = (2.5 + ((hash >> 7) % 31) as f32 * 0.1).min(length - 2.0);
            let deck_out = 1.0 + ((hash >> 3) % 4) as f32 * 0.1;
            let lo = 1.0 + deck_len * 0.5;
            let hi = length - 1.0 - deck_len * 0.5;
            let frac = ((hash >> 11) % 41) as f32 / 40.0;
            // A hanging trade sign (arm out to 1.05 m at 3.35 m) would spear a
            // deck hung right over its door: mirror the sign gate and step to
            // one flank of a 1.4 m door zone when it fires.
            let sign_hash = stable_hash(&building.id).rotate_left(9);
            let centre_along =
                if door_edge == Some(edge_index) && sign_hash % 3 != 0 && sign_hash % 8 == 0 {
                    let pick = |window_lo: f32, window_hi: f32| {
                        (window_hi >= window_lo).then(|| window_lo + (window_hi - window_lo) * frac)
                    };
                    let left = pick(lo, length * 0.5 - 1.4 - deck_len * 0.5);
                    let right = pick(length * 0.5 + 1.4 + deck_len * 0.5, hi);
                    let choice = if (hash >> 15) % 2 == 0 {
                        left.or(right)
                    } else {
                        right.or(left)
                    };
                    match choice {
                        Some(along) => along,
                        None => continue,
                    }
                } else {
                    lo + (hi - lo) * frac
                };
            let centre2 = a2 + direction * centre_along;

            for &floor in &floors {
                let deck_top = BUILDING_FLOOR_HEIGHT * floor as f32 + 0.10;
                // A jettied facade steps out under the deck: hang it flush
                // outside the band face at its height and let the brackets
                // reach back to the recessed wall a storey below.
                let face_deck = jetty_face_offset(building, deck_top);
                let face_low = jetty_face_offset(building, deck_top - 0.95);
                // The deck volume wants open air: clear of every other
                // footprint, our own concave folds, the overhead shells, the
                // gallery spots and the ground fixtures below.
                let deck_clear = [-0.5_f32, -0.25, 0.0, 0.25, 0.5].into_iter().all(|s| {
                    [0.4_f32, deck_out + 0.2].into_iter().all(|out| {
                        let probe =
                            centre2 + direction * (s * deck_len) + normal2 * (face_deck + out);
                        !building_at(probe, building_index)
                            && !point_in_polygon(probe, &building.polygon)
                            && !overhead_blocked(probe)
                            && !fixture_near(probe)
                    })
                });
                // No other deck may share this air (a stacked pair is fine:
                // its floors sit a full storey apart).
                let deck_centre = centre2 + normal2 * (face_deck + deck_out * 0.5);
                let occupied = [-0.5_f32, -0.25, 0.0, 0.25, 0.5].into_iter().any(|s| {
                    let probe = deck_centre + direction * (s * deck_len);
                    placed
                        .iter()
                        .any(|(other_floor, centre, dir, half_along, half_out)| {
                            *other_floor == floor && {
                                let local = probe - *centre;
                                local.dot(*dir).abs() < half_along + 0.4
                                    && local.dot(Vec2::new(-dir.y, dir.x)).abs() < half_out + 0.4
                            }
                        })
                });
                if !deck_clear || occupied {
                    continue;
                }
                add_open_balcony(
                    &mut timber,
                    collision_world,
                    centre2 + normal2 * face_deck,
                    direction,
                    normal2,
                    deck_len,
                    deck_out,
                    deck_top,
                    face_deck - face_low,
                    building_tint(building),
                    hash,
                );
                placed.push((
                    floor,
                    deck_centre,
                    direction,
                    deck_len * 0.5,
                    deck_out * 0.5,
                ));
                hung += 1;
            }
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        timber,
        "Open balconies",
    );
    info!("hung {hung} open balconies");
}

/// One open balcony: the bracket-carried deck slab, corner posts, a handrail
/// with balusters round the three open sides, and the dark door it serves.
/// `centre2` sits on the wall face at deck height; on a jettied facade the
/// brackets reach `bracket_setback` further back to seat on the wall below.
/// Only the slab collides — the yard-stair landing precedent — a perch far
/// above the walk band.
#[allow(clippy::too_many_arguments)]
fn add_open_balcony(
    timber: &mut MeshData,
    collision_world: &mut CollisionWorld,
    centre2: Vec2,
    direction: Vec2,
    normal2: Vec2,
    deck_len: f32,
    deck_out: f32,
    deck_top: f32,
    bracket_setback: f32,
    tint: [f32; 3],
    hash: u32,
) {
    let at = |along: f32, out: f32| centre2 + direction * along + normal2 * out;
    let half_len = deck_len * 0.5;
    timber.set_brush(tint);

    let deck = at(0.0, deck_out * 0.5 + 0.01);
    add_oriented_box(
        timber,
        Vec3::new(deck.x, deck_top - 0.05, deck.y),
        Vec3::new(half_len, 0.05, deck_out * 0.5),
        direction,
    );
    // Angled brackets out of the wall carry it; their wall feet stop at
    // deck−0.95, well above head height.
    let brackets = ((deck_len / 1.6).floor() as usize).clamp(2, 4);
    for index in 0..brackets {
        let along = (index as f32 / (brackets - 1) as f32 - 0.5) * (deck_len - 0.5);
        add_face_member(
            timber,
            at(along, 0.0),
            normal2,
            direction,
            Vec2::new(-bracket_setback - 0.06, deck_top - 0.95),
            Vec2::new(deck_out - 0.16, deck_top - 0.12),
            0.055,
            0.06,
            true,
        );
    }

    // Corner posts, the rail at hand height, and the balusters beneath it.
    let rail_y = deck_top + 0.99;
    for side in [-1.0_f32, 1.0] {
        let post = at(side * (half_len - 0.05), deck_out - 0.06);
        add_oriented_box(
            timber,
            Vec3::new(post.x, deck_top + 0.5, post.y),
            Vec3::new(0.045, 0.5, 0.045),
            direction,
        );
    }
    let outer = at(0.0, deck_out - 0.06);
    add_oriented_box(
        timber,
        Vec3::new(outer.x, rail_y, outer.y),
        Vec3::new(half_len, 0.032, 0.05),
        direction,
    );
    let end_run = (deck_out - 0.08) * 0.5;
    for side in [-1.0_f32, 1.0] {
        let end = at(side * (half_len - 0.05), end_run + 0.02);
        add_oriented_box(
            timber,
            Vec3::new(end.x, rail_y, end.y),
            Vec3::new(0.045, 0.032, end_run),
            direction,
        );
        let foot = at(side * (half_len - 0.05), deck_out * 0.5);
        add_oriented_box(
            timber,
            Vec3::new(foot.x, deck_top + 0.48, foot.y),
            Vec3::new(0.028, 0.48, 0.028),
            direction,
        );
    }
    let balusters = ((deck_len / 0.55).floor() as usize).max(3);
    for index in 0..balusters {
        let along = (index as f32 / (balusters - 1) as f32 - 0.5) * (deck_len - 0.55);
        let foot = at(along, deck_out - 0.06);
        add_oriented_box(
            timber,
            Vec3::new(foot.x, deck_top + 0.48, foot.y),
            Vec3::new(0.028, 0.48, 0.028),
            direction,
        );
    }

    // The dark door the balcony exists for, slid a little off the middle.
    let door_along = (((hash >> 21) % 21) as f32 / 20.0 - 0.5) * (deck_len - 1.6).max(0.0);
    let door = at(door_along, 0.055);
    timber.set_brush([tint[0] * 0.30, tint[1] * 0.28, tint[2] * 0.26]);
    add_oriented_box(
        timber,
        Vec3::new(door.x, deck_top + 1.02, door.y),
        Vec3::new(0.50, 0.98, 0.045),
        direction,
    );
    timber.reset_brush();

    // Only the deck slab carries collision, entirely above the walk band.
    add_rotated_box_collider_at(
        collision_world,
        Vec3::new(deck.x, deck_top - 0.05, deck.y),
        Vec3::new(deck_len, 0.10, deck_out),
        (-direction.y).atan2(direction.x),
    );
}

/// Washing strung high over the narrow streets and back courts: a sagging
/// rope between two flanking parcels with a few cloth pieces pegged over it.
/// Everything lands in two city-wide batches and nothing collides — the lines
/// live far above the walk band, so the baked navigation never hears about
/// them.
fn build_laundry_lines(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
) {
    let bounds: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .map(|building| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for point in &building.polygon {
                min = min.min(Vec2::from_array(*point));
                max = max.max(Vec2::from_array(*point));
            }
            (min, max)
        })
        .collect();
    let inside = |point: Vec2, index: usize, building: &Building| {
        point.x >= bounds[index].0.x
            && point.y >= bounds[index].0.y
            && point.x <= bounds[index].1.x
            && point.y <= bounds[index].1.y
            && point_in_polygon(point, &building.polygon)
    };
    // A rope wants an ordinary dwelling wall on both sides: two storeys or
    // more, standing on the ground, never a church, tower or overhead shell.
    let ordinary = |building: &Building| {
        building.levels >= 2
            && !matches!(
                building.use_name.as_str(),
                "bridge" | "ecclesiastical" | "fortification"
            )
            && building_verticals(building).0 <= 0.1
    };
    let flank_at = |point: Vec2| -> Option<&Building> {
        plan.buildings
            .iter()
            .enumerate()
            .find(|(index, building)| ordinary(building) && inside(point, *index, building))
            .map(|(_, building)| building)
    };
    let any_building_at = |point: Vec2, skip: usize| {
        plan.buildings
            .iter()
            .enumerate()
            .any(|(index, building)| index != skip && inside(point, index, building))
    };

    // Keep-out blobs for what already hangs over the streets: the bridge
    // houses and the malt house, plus every street-gallery candidate — the
    // same hash formula `build_street_galleries` draws from. Candidates whose
    // seat probes failed never got a gallery, so this over-approximates; that
    // only thins the washing near those spots.
    let shell_boxes: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .enumerate()
        .filter(|(_, building)| building_verticals(building).0 > 0.1)
        .map(|(index, _)| {
            (
                bounds[index].0 - Vec2::splat(1.2),
                bounds[index].1 + Vec2::splat(1.2),
            )
        })
        .collect();
    let gallery_spots: Vec<(Vec2, f32)> = plan
        .roads
        .iter()
        .filter(|road| (2.0..=5.5).contains(&road.width_m))
        .flat_map(|road| {
            road.points
                .windows(2)
                .enumerate()
                .filter_map(move |(segment_index, pair)| {
                    let a = Vec2::from_array(pair[0]);
                    let b = Vec2::from_array(pair[1]);
                    if a.distance(b) < 12.0 {
                        return None;
                    }
                    let hash = stable_hash(&format!("gallery-{}-{segment_index}", road.id));
                    if hash % 2 != 0 {
                        return None;
                    }
                    let t = 0.3 + (hash % 41) as f32 / 100.0;
                    Some((a.lerp(b, t), road.width_m * 0.5 + 4.2))
                })
        })
        .collect();
    let overhead_blocked = |point: Vec2| {
        shell_boxes.iter().any(|(min, max)| {
            point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
        }) || gallery_spots
            .iter()
            .any(|(center, reach)| center.distance_squared(point) < reach * reach)
    };
    // Mirror of the `jetty_bands` gate. Rope anchors top out at 5.2 m — inside
    // the first jettied band — so a jettied flank leans at most one step in.
    let jetty_reach = |building: &Building| -> f32 {
        if !building.named
            && building.polygon.len() == 4
            && polygon_is_convex(&building.polygon)
            && matches!(wall_kind(&building.material), WallKind::HalfTimber)
            && stable_hash(&building.id) % 10 < 8
        {
            JETTY_STEP
        } else {
            0.0
        }
    };

    let squares: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| site.kind == "square")
        .collect();
    let near_road = |point: Vec2| {
        plan.roads.iter().any(|road| {
            let margin = road.width_m * 0.5 + 0.6;
            road.points.windows(2).any(|pair| {
                segment_distance_squared(
                    point,
                    Vec2::from_array(pair[0]),
                    Vec2::from_array(pair[1]),
                ) < margin * margin
            })
        })
    };
    let near_wall = |point: Vec2| {
        plan.wall_polygon_xz
            .windows(2)
            .chain(std::iter::once(
                &[
                    *plan.wall_polygon_xz.last().unwrap(),
                    plan.wall_polygon_xz[0],
                ][..],
            ))
            .any(|pair| {
                segment_distance_squared(
                    point,
                    Vec2::from_array(pair[0]),
                    Vec2::from_array(pair[1]),
                ) < 2.5 * 2.5
            })
    };
    // The eaves cap the anchors; two-storey flanks already put them at 6.75 m.
    let eave_cap = |flank_a: &Building, flank_b: &Building| {
        (building_verticals(flank_a)
            .1
            .min(building_verticals(flank_b).1)
            - 0.6)
            .max(3.7)
    };

    let mut rope = MeshData::default();
    let mut cloth = MeshData::default();
    let mut strung = 0;

    // Street lines: strung wall to wall over the narrow streets.
    for road in &plan.roads {
        if !(2.0..=4.5).contains(&road.width_m) {
            continue;
        }
        for (segment_index, pair) in road.points.windows(2).enumerate() {
            let a = Vec2::from_array(pair[0]);
            let b = Vec2::from_array(pair[1]);
            let length = a.distance(b);
            if length < 7.0 {
                continue;
            }
            let across = Vec2::new(-(b.y - a.y), b.x - a.x) / length;
            let slots = (length / 2.0).floor().max(1.0) as usize;
            for slot in 0..slots {
                let hash = stable_hash(&format!("laundry-{}-{segment_index}-{slot}", road.id));
                if hash % 6 != 0 {
                    continue;
                }
                let t = (slot as f32 + 0.25 + (hash % 51) as f32 / 100.0) / slots as f32;
                let center = a.lerp(b, t);
                // Façades sit at varying setbacks; probe outward until a wall
                // is found and bury the anchor that deep in it.
                let seat = |side: f32| {
                    [0.55_f32, 1.3, 2.2].into_iter().find_map(|extra| {
                        flank_at(center + across * side * (road.width_m * 0.5 + extra))
                            .map(|building| (extra, building))
                    })
                };
                let (Some((seat_a, flank_a)), Some((seat_b, flank_b))) = (seat(1.0), seat(-1.0))
                else {
                    continue;
                };
                let half_a = road.width_m * 0.5 + seat_a;
                let half_b = road.width_m * 0.5 + seat_b;
                let anchor_a = center + across * half_a;
                let anchor_b = center - across * half_b;
                if [0.15_f32, 0.3, 0.5, 0.7, 0.85]
                    .into_iter()
                    .any(|t| overhead_blocked(anchor_a.lerp(anchor_b, t)))
                {
                    continue;
                }

                let cap = eave_cap(flank_a, flank_b);
                let y_a = (3.6 + ((hash >> 4) % 17) as f32 * 0.1).min(cap);
                let y_b = (y_a + ((hash >> 9) % 9) as f32 * 0.1 - 0.4).clamp(3.6, cap.min(5.2));
                add_laundry_line(
                    &mut rope,
                    &mut cloth,
                    Vec3::new(anchor_a.x, y_a, anchor_a.y),
                    Vec3::new(anchor_b.x, y_b, anchor_b.y),
                    (
                        seat_a + jetty_reach(flank_a) + 0.12,
                        half_a + half_b - seat_b - jetty_reach(flank_b) - 0.12,
                    ),
                    hash,
                );
                strung += 1;
            }
        }
    }

    // Court lines: pairs of taller parcels facing each other across a yard,
    // away from any road — the washing of the back courts.
    for (building_index, building) in plan.buildings.iter().enumerate() {
        if !ordinary(building) {
            continue;
        }
        let hash = stable_hash(&format!("courtline-{}", building.id));
        if hash % 5 != 0 {
            continue;
        }
        let orientation = plan::signed_area(&building.polygon).signum();
        let edge_count = building.polygon.len();
        for edge_offset in 0..edge_count {
            let edge_index = (edge_offset + hash as usize) % edge_count;
            let a2 = Vec2::from_array(building.polygon[edge_index]);
            let b2 = Vec2::from_array(building.polygon[(edge_index + 1) % edge_count]);
            let edge = b2 - a2;
            let length = edge.length();
            if length < 4.0 {
                continue;
            }
            let direction = edge / length;
            let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
            if orientation < 0.0 {
                normal2 = -normal2;
            }
            let origin = a2 + direction * (length * (0.35 + ((hash >> 5) % 31) as f32 / 100.0));
            let mut facing: Option<(f32, &Building)> = None;
            for step in 0..9 {
                let Some(candidate) = flank_at(origin + normal2 * (2.6 + step as f32 * 0.8)) else {
                    continue;
                };
                if candidate.id != building.id {
                    facing = Some((2.6 + step as f32 * 0.8, candidate));
                }
                // Hitting our own wall means a concave court fold: abandon.
                break;
            }
            // The probe steps by 0.8 m, so the facing façade lies within that
            // much short of the hit; 3.8 keeps the true gap at 3 m or more.
            let Some((gap, other)) = facing else { continue };
            if gap < 3.8 {
                continue;
            }
            let facade = gap - 0.8;
            let court_clear = [0.2_f32, 0.4, 0.6, 0.8]
                .into_iter()
                .all(|s| !any_building_at(origin + normal2 * (facade * s), building_index))
                && [0.25_f32, 0.5, 0.75].into_iter().all(|s| {
                    let probe = origin + normal2 * (facade * s);
                    !near_road(probe)
                        && !near_wall(probe)
                        && !squares
                            .iter()
                            .any(|square| point_in_polygon(probe, &square.polygon))
                        && !overhead_blocked(probe)
                });
            if !court_clear {
                continue;
            }

            let anchor_a = origin - normal2 * 0.45;
            let anchor_b = origin + normal2 * (gap + 0.3);
            let cap = eave_cap(building, other);
            let y_a = (3.6 + ((hash >> 4) % 17) as f32 * 0.1).min(cap);
            let y_b = (y_a + ((hash >> 9) % 9) as f32 * 0.1 - 0.4).clamp(3.6, cap.min(5.2));
            add_laundry_line(
                &mut rope,
                &mut cloth,
                Vec3::new(anchor_a.x, y_a, anchor_a.y),
                Vec3::new(anchor_b.x, y_b, anchor_b.y),
                (
                    0.45 + jetty_reach(building) + 0.12,
                    0.45 + facade - jetty_reach(other) - 0.12,
                ),
                hash,
            );
            strung += 1;
            break;
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        rope,
        "Laundry ropes",
    );
    spawn_batch(commands, meshes, &materials.linen, cloth, "Laundry washing");
    info!("strung {strung} laundry lines");
}

/// One strung line: the sagging rope between two buried wall anchors and the
/// washing pegged over its open middle. `clear` is the along-span window
/// (metres from the A anchor) the cloth may occupy — wall seats and jetty
/// overhangs have already been subtracted by the caller.
fn add_laundry_line(
    rope: &mut MeshData,
    cloth: &mut MeshData,
    start: Vec3,
    end: Vec3,
    clear: (f32, f32),
    hash: u32,
) {
    let flat = Vec2::new(end.x - start.x, end.z - start.z);
    let span = flat.length();
    if span < 1.0 {
        return;
    }
    let along = flat / span;
    let sag = 0.25 + ((hash >> 7) % 21) as f32 / 100.0;
    let drape = |t: f32| {
        let mut point = start.lerp(end, t);
        point.y -= sag * 4.0 * t * (1.0 - t);
        point
    };
    let segments = 8 + (hash % 5) as usize;
    let mut previous = drape(0.0);
    for step in 1..=segments {
        let next = drape(step as f32 / segments as f32);
        add_rope_ribbons(rope, previous, next, 0.016);
        previous = next;
    }

    let corridor = clear.1 - clear.0;
    if corridor < 0.45 {
        return;
    }
    let count = (2 + (hash >> 11) % 4).min((corridor / 0.62) as u32).max(1);
    for piece in 0..count {
        let piece_hash = hash ^ piece.wrapping_mul(0x9E37_79B9);
        let slot_width = corridor / count as f32;
        let at =
            clear.0 + slot_width * (piece as f32 + 0.28 + ((piece_hash >> 3) % 45) as f32 / 100.0);
        let half_width = (0.25 + ((piece_hash >> 8) % 26) as f32 / 100.0)
            .min(slot_width * 0.44)
            .min(at - clear.0)
            .min(clear.1 - at);
        if half_width < 0.18 {
            continue;
        }
        let top = [
            drape((at - half_width) / span),
            drape(at / span),
            drape((at + half_width) / span),
        ];
        let drop = (0.7 + ((piece_hash >> 13) % 61) as f32 / 100.0).min(top[1].y - 1.95);
        let lean = Vec3::new(-along.y, 0.0, along.x)
            * (0.05 + ((piece_hash >> 6) % 11) as f32 / 150.0)
            * if piece_hash & 1 == 0 { 1.0 } else { -1.0 };
        cloth.set_brush(cloth_tint(piece_hash));
        add_cloth_piece(cloth, top, drop, lean);
    }
    cloth.reset_brush();
}

/// Mostly undyed linen, with the odd dull madder, ochre or grey piece.
fn cloth_tint(hash: u32) -> [f32; 3] {
    match hash % 12 {
        0 | 1 => [0.52, 0.27, 0.23],
        2 => [0.61, 0.48, 0.27],
        3 | 4 => [0.50, 0.52, 0.55],
        _ => {
            let value = 0.80 + ((hash >> 16) % 23) as f32 / 100.0;
            [value * 1.02, value, value * 0.93]
        }
    }
}

/// One rope link as two crossed ribbons — enough to read as a dark line from
/// every direction without a full box per link (the dark-wood material culls
/// nothing, so single windings show both faces).
fn add_rope_ribbons(rope: &mut MeshData, from: Vec3, to: Vec3, radius: f32) {
    let axis = (to - from).normalize_or_zero();
    if axis == Vec3::ZERO {
        return;
    }
    let side = axis.cross(Vec3::Y).normalize_or(Vec3::X);
    let lift = side.cross(axis).normalize_or(Vec3::Y);
    for (spread, normal) in [(lift, side), (side, lift)] {
        rope.quad(
            [
                from - spread * radius,
                to - spread * radius,
                to + spread * radius,
                from + spread * radius,
            ],
            normal,
            [
                Vec2::ZERO,
                Vec2::new(0.2, 0.0),
                Vec2::new(0.2, 0.05),
                Vec2::new(0.0, 0.05),
            ],
        );
    }
}

/// One washed piece pegged over the rope: a 3x2 vertex strip whose top edge
/// rides the rope's sag and whose bottom mid-point drops and leans a little,
/// so it hangs rather than stands.
fn add_cloth_piece(cloth: &mut MeshData, top: [Vec3; 3], drop: f32, lean: Vec3) {
    let bottom = [
        top[0] - Vec3::Y * drop,
        top[1] - Vec3::Y * (drop + 0.05) + lean,
        top[2] - Vec3::Y * drop,
    ];
    let normal = (top[2] - top[0]).cross(Vec3::NEG_Y).normalize_or(Vec3::Z);
    // The linen tile repeats seamlessly, so anchoring the sample window to the
    // piece's position gives every sheet its own stains for free.
    let shift = Vec2::new((top[0].x * 0.37).fract(), (top[0].z * 0.29).fract());
    let first = cloth.positions.len() as u32;
    for (index, point) in top.iter().chain(bottom.iter()).enumerate() {
        let uv = Vec2::new((index % 3) as f32 * 0.5, if index < 3 { 0.0 } else { 1.0 });
        cloth.vertex(*point, normal, uv + shift);
    }
    cloth.indices.extend_from_slice(&[
        first,
        first + 1,
        first + 4,
        first,
        first + 4,
        first + 3,
        first + 1,
        first + 2,
        first + 5,
        first + 1,
        first + 5,
        first + 4,
    ]);
}

/// One computed street-gallery placement — the mirror of what
/// `build_street_galleries` builds (same hash, same seat probes), so the side
/// gantries land on real galleries and the facade beams keep out of all of
/// them.
struct GallerySpan {
    shifted: Vec2,
    street_dir: Vec2,
    across: Vec2,
    span: f32,
    road_width: f32,
    hash: u32,
}

/// Mirror of the `jetty_bands` gate: how far a ground building's wall face at
/// height `y` stands out from its cadastral line. Bands are one floor tall
/// from the ground, stepping out one jetty per storey, capped after two.
fn jetty_face_offset(building: &Building, y: f32) -> f32 {
    if building.named
        || building.polygon.len() != 4
        || !polygon_is_convex(&building.polygon)
        || !matches!(wall_kind(&building.material), WallKind::HalfTimber)
        || building.levels < 2
        || stable_hash(&building.id) % 10 >= 8
    {
        return 0.0;
    }
    JETTY_STEP * ((y / BUILDING_FLOOR_HEIGHT).floor() as i32).clamp(0, 2) as f32
}

/// Hoist gantries — the signature overhead-life prop of the references: a
/// squared beam projecting high from a wall, a pulley block at the tip, and a
/// rope with a load swinging over the street. Rigged on a hash-picked eighth
/// of the taller trade and storage houses, on a mouth gable of each bridge
/// shell and the malt house, and on a few street galleries. Everything is
/// batched and nothing collides; every load bottoms out at 3.5 m or more over
/// a road, far above the walk band.
fn build_hoist_gantries(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
) {
    let tall_building_at = |point: Vec2| {
        plan.buildings.iter().any(|building| {
            building.levels >= 2
                && building.use_name != "bridge"
                && point_in_polygon(point, &building.polygon)
        })
    };
    let mut galleries = Vec::new();
    for road in &plan.roads {
        if !(2.0..=5.5).contains(&road.width_m) {
            continue;
        }
        for (segment_index, pair) in road.points.windows(2).enumerate() {
            let a = Vec2::from_array(pair[0]);
            let b = Vec2::from_array(pair[1]);
            let length = a.distance(b);
            if length < 12.0 {
                continue;
            }
            let hash = stable_hash(&format!("gallery-{}-{segment_index}", road.id));
            if hash % 2 != 0 {
                continue;
            }
            let t = 0.3 + (hash % 41) as f32 / 100.0;
            let center = a.lerp(b, t);
            let street_dir = (b - a) / length;
            let across = Vec2::new(-street_dir.y, street_dir.x);
            let seat_depth = |side: f32| {
                [0.7_f32, 1.6, 2.6].into_iter().find(|extra| {
                    tall_building_at(center + across * side * (road.width_m * 0.5 + extra))
                })
            };
            let (Some(seat_a), Some(seat_b)) = (seat_depth(1.0), seat_depth(-1.0)) else {
                continue;
            };
            galleries.push(GallerySpan {
                shifted: center + across * (seat_a - seat_b) * 0.5,
                street_dir,
                across,
                span: road.width_m + seat_a + seat_b + 1.6,
                road_width: road.width_m,
                hash,
            });
        }
    }
    let gallery_blocked = |point: Vec2| {
        galleries.iter().any(|gallery| {
            gallery.shifted.distance_squared(point) < (gallery.span * 0.5 + 1.0).powi(2)
        })
    };
    // The overhead shells (bridges, malt house) with room for the load to
    // swing clear of their walls.
    let shell_boxes: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .filter(|building| building_verticals(building).0 > 0.1)
        .map(|building| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for point in &building.polygon {
                min = min.min(Vec2::from_array(*point));
                max = max.max(Vec2::from_array(*point));
            }
            (min - Vec2::splat(1.2), max + Vec2::splat(1.2))
        })
        .collect();
    let overhead_blocked = |point: Vec2| {
        gallery_blocked(point)
            || shell_boxes.iter().any(|(min, max)| {
                point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
            })
    };
    let squares: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| site.kind == "square")
        .collect();
    let bounds: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .map(|building| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for point in &building.polygon {
                min = min.min(Vec2::from_array(*point));
                max = max.max(Vec2::from_array(*point));
            }
            (min, max)
        })
        .collect();
    let building_at = |point: Vec2, skip: usize| {
        plan.buildings.iter().enumerate().any(|(index, other)| {
            index != skip
                && point.x >= bounds[index].0.x
                && point.y >= bounds[index].0.y
                && point.x <= bounds[index].1.x
                && point.y <= bounds[index].1.y
                && point_in_polygon(point, &other.polygon)
        })
    };

    let mut dark_wood = MeshData::default();
    let mut timber = MeshData::default();
    let mut iron = MeshData::default();
    let mut ochre = MeshData::default();
    let mut russet = MeshData::default();
    let mut rigged = 0;

    // (a) Street facades of the working trade and storage houses: the beam
    // rides just under the eave — or up in the gable where the door edge is a
    // clear short face — over the same street the door serves.
    for (building_index, building) in plan.buildings.iter().enumerate() {
        if !matches!(building.use_name.as_str(), "trade" | "storage") || building.levels < 2 {
            continue;
        }
        let (base_y, eave_y) = building_verticals(building);
        if base_y > 0.1 {
            continue;
        }
        let hash = stable_hash(&format!("gantry-{}", building.id));
        if hash % 8 != 0 {
            continue;
        }
        let Some(&edge_index) = door_edges.get(&building.id) else {
            continue;
        };
        let polygon = &building.polygon;
        let a = Vec2::from_array(polygon[edge_index]);
        let b = Vec2::from_array(polygon[(edge_index + 1) % polygon.len()]);
        let edge = b - a;
        let length = edge.length();
        if length < 5.0 {
            continue;
        }
        let direction = edge / length;
        let orientation = plan::signed_area(polygon).signum();
        let mut normal = Vec2::new(edge.y, -edge.x).normalize() * orientation;
        // `plan` polygons wind either way; the beam must project outward.
        if point_in_polygon(a + direction * (length * 0.5) + normal * 0.5, polygon) {
            normal = -normal;
        }
        let along = (length * 0.5 + (((hash >> 5) % 31) as f32 / 30.0 - 0.5) * 0.3 * length)
            .clamp(1.5, length - 1.5);
        let anchor2 = a + direction * along;
        // Square-fronting facades carry arcade pentices at exactly the height
        // the load would swing through.
        if squares
            .iter()
            .any(|square| point_in_polygon(anchor2 + normal * 1.6, &square.polygon))
        {
            continue;
        }
        // Window heads stop at eave−0.875 and the drooping eave overhang
        // bottoms out near eave−0.6: the beam threads between them. A clearly
        // short quad edge is a gable, where the beam climbs to the peak;
        // near-square footprints stay on the safe eave line because the roof
        // may read their long axis either way.
        let beam_y = if polygon.len() == 4 {
            let edge_01 = Vec2::from_array(polygon[0]).distance(Vec2::from_array(polygon[1]));
            let edge_12 = Vec2::from_array(polygon[1]).distance(Vec2::from_array(polygon[2]));
            let gable = (edge_01 + 1.5 < edge_12 && edge_index % 2 == 0)
                || (edge_12 + 1.5 < edge_01 && edge_index % 2 == 1);
            if gable { eave_y + 0.45 } else { eave_y - 0.75 }
        } else {
            eave_y - 0.45
        };
        let face_offset = jetty_face_offset(building, beam_y);
        // The beam wants open air in front: step outward until a neighbouring
        // footprint answers, then let that neighbour's possible jetty and a
        // swing margin eat into what the probes proved clear.
        let clear_out = [0.6_f32, 1.05, 1.5, 1.95, 2.4, 2.85, 3.3]
            .into_iter()
            .take_while(|out| !building_at(anchor2 + normal * *out, building_index))
            .last();
        let Some(clear_out) = clear_out else {
            continue;
        };
        let max_reach = clear_out - JETTY_STEP * 2.0 - 0.2 - face_offset;
        if max_reach < 1.0 {
            continue;
        }
        let reach = (1.05 + ((hash >> 9) % 56) as f32 / 100.0).min(max_reach.min(1.6));
        let hang2 = anchor2 + normal * (face_offset + reach - 0.1);
        if overhead_blocked(hang2) {
            continue;
        }

        add_hoist_beam(
            &mut dark_wood,
            anchor2,
            normal,
            direction,
            face_offset,
            reach,
            beam_y,
        );
        let bottom_y = (3.65 + ((hash >> 13) % 15) as f32 * 0.1).min(beam_y - 1.7);
        let variant = if hash % 7 == 0 { 4 } else { (hash >> 3) % 4 };
        add_hoist_load(
            &mut dark_wood,
            &mut timber,
            &mut iron,
            &mut ochre,
            &mut russet,
            hang2,
            beam_y - 0.31,
            bottom_y,
            normal,
            variant,
            hash,
        );
        rigged += 1;
    }

    // (b) The bridge shells and the malt house: one beam high on a mouth
    // gable, its load swinging over the road that runs beneath — the
    // bridge-house hoist of reference image A.
    for (building_index, building) in plan.buildings.iter().enumerate() {
        if !(building.use_name == "bridge" || building.id == "named_malt_house")
            || building.polygon.len() != 4
        {
            continue;
        }
        let (_, eave_y) = building_verticals(building);
        let p: Vec<Vec2> = building
            .polygon
            .iter()
            .map(|point| Vec2::from_array(*point))
            .collect();
        let edge_01 = p[0].distance(p[1]);
        let edge_12 = p[1].distance(p[2]);
        // The mouths are the two short edges — the same reading the passage
        // dressing and the bridge piers stand on.
        let (ends, width) = if edge_01 >= edge_12 {
            ([(p[0] + p[3]) * 0.5, (p[1] + p[2]) * 0.5], edge_12)
        } else {
            ([(p[0] + p[1]) * 0.5, (p[2] + p[3]) * 0.5], edge_01)
        };
        let long_dir = (ends[1] - ends[0]).normalize_or_zero();
        let across = Vec2::new(-long_dir.y, long_dir.x);
        let hash = stable_hash(&format!("gantry-{}", building.id));
        let jitter = across * ((((hash >> 4) % 21) as f32 / 20.0 - 0.5) * width * 0.24);
        let reach = 1.25 + ((hash >> 11) % 26) as f32 / 100.0;
        let beam_y = eave_y + 0.5;
        for mouth in 0..2 {
            let end_index = (hash as usize + mouth) % 2;
            let outward = if end_index == 0 { -long_dir } else { long_dir };
            let anchor2 = ends[end_index] + jitter;
            let hang2 = anchor2 + outward * (reach - 0.1);
            // A bridge mouth can butt straight into its neighbour (the Tally
            // Bridge serves the toll house door to door): the beam needs open
            // air, not a facade.
            if gallery_blocked(hang2)
                || [0.4_f32, 0.9, reach + 0.5]
                    .into_iter()
                    .any(|out| building_at(anchor2 + outward * out, building_index))
            {
                continue;
            }
            add_hoist_beam(&mut dark_wood, anchor2, outward, across, 0.0, reach, beam_y);
            let bottom_y = 3.7 + ((hash >> 13) % 8) as f32 * 0.1;
            add_hoist_load(
                &mut dark_wood,
                &mut timber,
                &mut iron,
                &mut ochre,
                &mut russet,
                hang2,
                beam_y - 0.31,
                bottom_y,
                outward,
                (hash >> 3) % 4,
                hash,
            );
            rigged += 1;
            break;
        }
    }

    // (c) A few of the street galleries carry a small side gantry with a pail
    // on the rope — the footbridge hoist of reference image B.
    let mut side_gantries = 0;
    for gallery in &galleries {
        if side_gantries >= 4 || gallery.hash.rotate_left(13) % 5 != 0 {
            continue;
        }
        let side = if (gallery.hash >> 3) % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        let window = (gallery.road_width * 0.5 - 0.7).max(0.0);
        let lateral =
            ((((gallery.hash >> 16) % 33) as f32 / 32.0 - 0.5) * 1.6).clamp(-window, window);
        let base2 = gallery.shifted + gallery.across * lateral;
        let out_dir = gallery.street_dir * side;
        // Anchored in the parapet (faces 1.01–1.25 m out, top 6.53 m), the
        // beam ducks under the slate hood (underside 6.82 m, edge 1.55 m out)
        // and drops its rope past it.
        let beam_y = 6.08;
        let anchor2 = base2 + out_dir * 1.13;
        add_hoist_beam(
            &mut dark_wood,
            anchor2,
            out_dir,
            gallery.across,
            0.0,
            0.95,
            beam_y,
        );
        let bottom_y = 3.6 + ((gallery.hash >> 19) % 6) as f32 * 0.1;
        add_hoist_load(
            &mut dark_wood,
            &mut timber,
            &mut iron,
            &mut ochre,
            &mut russet,
            anchor2 + out_dir * 0.85,
            beam_y - 0.31,
            bottom_y,
            out_dir,
            3,
            gallery.hash,
        );
        side_gantries += 1;
        rigged += 1;
    }

    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        dark_wood,
        "Hoist gantries and ropes",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        timber,
        "Hoist loads: crates and pails",
    );
    spawn_batch(commands, meshes, &materials.iron, iron, "Hoist ironwork");
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_ochre,
        ochre,
        "Hoist loads: sacks and bales",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_russet,
        russet,
        "Hoist loads: more sacks",
    );
    info!("rigged {rigged} hoist gantries");
}

/// The projecting arm itself: squared timber out of the wall face at
/// `face_offset`, a diagonal brace back down into it, and the pulley block
/// under the tip. `normal` points out of the wall, `direction` along it.
fn add_hoist_beam(
    dark_wood: &mut MeshData,
    anchor2: Vec2,
    normal: Vec2,
    direction: Vec2,
    face_offset: f32,
    reach: f32,
    beam_y: f32,
) {
    let tip = face_offset + reach;
    // The beam spans from 0.35 inside the face to the tip.
    let center2 = anchor2 + normal * (face_offset + (reach - 0.35) * 0.5);
    add_oriented_box(
        dark_wood,
        Vec3::new(center2.x, beam_y, center2.y),
        Vec3::new((reach + 0.35) * 0.5, 0.09, 0.09),
        normal,
    );
    add_face_member(
        dark_wood,
        anchor2,
        normal,
        direction,
        Vec2::new(face_offset - 0.08, beam_y - 1.05),
        Vec2::new(face_offset + reach * 0.55, beam_y - 0.12),
        0.055,
        0.06,
        true,
    );
    let block = anchor2 + normal * (tip - 0.1);
    add_oriented_box(
        dark_wood,
        Vec3::new(block.x, beam_y - 0.21, block.y),
        Vec3::new(0.055, 0.10, 0.038),
        normal,
    );
}

/// What swings from the pulley: the rope and a load — grain sack, corded
/// bale, crate, pail — or, for `variant` 4, nothing but a tied-off hook.
/// `bottom_y` is the load's lowest point; callers keep it 3.5 m over a road.
#[allow(clippy::too_many_arguments)]
fn add_hoist_load(
    dark_wood: &mut MeshData,
    timber: &mut MeshData,
    iron: &mut MeshData,
    ochre: &mut MeshData,
    russet: &mut MeshData,
    drop2: Vec2,
    rope_top: f32,
    bottom_y: f32,
    along: Vec2,
    variant: u32,
    hash: u32,
) {
    let rope = |dark_wood: &mut MeshData, to_y: f32| {
        add_rope_ribbons(
            dark_wood,
            Vec3::new(drop2.x, rope_top, drop2.y),
            Vec3::new(drop2.x, to_y, drop2.y),
            0.017,
        );
    };
    let center = |y: f32| Vec3::new(drop2.x, y, drop2.y);
    match variant {
        // A grain sack roped at the neck: an `add_sack` dome and its mirror.
        0 => {
            let cloth = if (hash >> 8) % 2 == 0 { ochre } else { russet };
            rope(dark_wood, bottom_y + 0.55);
            add_sack(cloth, center(bottom_y + 0.30), Vec3::new(0.24, 0.33, 0.21));
            add_sack(cloth, center(bottom_y + 0.30), Vec3::new(0.24, -0.30, 0.21));
        }
        // A corded bale: cloth block with dark straps crossing it.
        1 => {
            let cloth = if (hash >> 8) % 2 == 0 { ochre } else { russet };
            rope(dark_wood, bottom_y + 0.50);
            add_oriented_box(
                cloth,
                center(bottom_y + 0.26),
                Vec3::new(0.30, 0.26, 0.27),
                along,
            );
            add_oriented_box(
                dark_wood,
                center(bottom_y + 0.26),
                Vec3::new(0.315, 0.255, 0.05),
                along,
            );
            add_oriented_box(
                dark_wood,
                center(bottom_y + 0.26),
                Vec3::new(0.05, 0.255, 0.285),
                along,
            );
        }
        // A small crate.
        2 => {
            rope(dark_wood, bottom_y + 0.42);
            add_oriented_box(
                timber,
                center(bottom_y + 0.22),
                Vec3::new(0.24, 0.22, 0.24),
                along,
            );
        }
        // A pail on a rope bail.
        3 => {
            rope(dark_wood, bottom_y + 0.46);
            let side = Vec3::new(-along.y, 0.0, along.x) * 0.13;
            for flip in [-1.0, 1.0] {
                add_rope_ribbons(
                    dark_wood,
                    center(bottom_y + 0.46),
                    center(bottom_y + 0.27) + side * flip,
                    0.014,
                );
            }
            add_drum(timber, center(bottom_y + 0.15), 0.15, 0.30, true);
            add_drum(iron, center(bottom_y + 0.25), 0.157, 0.035, false);
        }
        // A bare hook, rope tied off.
        _ => {
            rope(dark_wood, rope_top - 0.62);
            add_oriented_box(
                iron,
                Vec3::new(drop2.x, rope_top - 0.68, drop2.y),
                Vec3::new(0.022, 0.07, 0.022),
                along,
            );
            add_oriented_box(
                iron,
                Vec3::new(drop2.x, rope_top - 0.755, drop2.y),
                Vec3::new(0.05, 0.02, 0.02),
                along,
            );
        }
    }
}

/// Squared distance from `point` to the segment `a`–`b`.
fn segment_distance_squared(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let edge = b - a;
    let length_squared = edge.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance_squared(a);
    }
    let along = ((point - a).dot(edge) / length_squared).clamp(0.0, 1.0);
    point.distance_squared(a + edge * along)
}

fn rotate2(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// A coopered barrel: ten-sided drum with a lid and two iron hoops.
fn add_barrel(timber: &mut MeshData, iron: &mut MeshData, base: Vec3) {
    add_drum(timber, base + Vec3::Y * 0.42, 0.32, 0.84, true);
    add_drum(iron, base + Vec3::Y * 0.22, 0.335, 0.06, false);
    add_drum(iron, base + Vec3::Y * 0.62, 0.335, 0.06, false);
}

/// A vertical cylinder written into a batch: `cap` adds the top disc.
fn add_drum(mesh: &mut MeshData, center: Vec3, radius: f32, height: f32, cap: bool) {
    const SEGMENTS: usize = 10;
    let half = height * 0.5;
    for segment in 0..SEGMENTS {
        let a0 = segment as f32 / SEGMENTS as f32 * (PI * 2.0);
        let a1 = (segment + 1) as f32 / SEGMENTS as f32 * (PI * 2.0);
        let n0 = Vec3::new(a0.cos(), 0.0, a0.sin());
        let n1 = Vec3::new(a1.cos(), 0.0, a1.sin());
        let mid = ((n0 + n1) * 0.5).normalize_or(Vec3::X);
        mesh.quad(
            [
                center + n0 * radius - Vec3::Y * half,
                center + n1 * radius - Vec3::Y * half,
                center + n1 * radius + Vec3::Y * half,
                center + n0 * radius + Vec3::Y * half,
            ],
            mid,
            [
                Vec2::new(0.0, 0.0),
                Vec2::new(0.06, 0.0),
                Vec2::new(0.06, height / 7.0),
                Vec2::new(0.0, height / 7.0),
            ],
        );
        if cap {
            let top = center + Vec3::Y * half;
            mesh.triangle(
                [top, top + n0 * radius, top + n1 * radius],
                [Vec2::ZERO, Vec2::new(0.05, 0.0), Vec2::new(0.05, 0.05)],
                true,
            );
        }
    }
}

/// A horizontal log lying along `along`, split-firewood scale.
fn add_log(mesh: &mut MeshData, center: Vec3, radius: f32, length: f32, along: Vec2) {
    const SEGMENTS: usize = 7;
    let axis = Vec3::new(along.x, 0.0, along.y);
    let side = Vec3::new(-along.y, 0.0, along.x);
    let half = axis * (length * 0.5);
    for segment in 0..SEGMENTS {
        let a0 = segment as f32 / SEGMENTS as f32 * (PI * 2.0);
        let a1 = (segment + 1) as f32 / SEGMENTS as f32 * (PI * 2.0);
        let r0 = (side * a0.cos() + Vec3::Y * a0.sin()) * radius;
        let r1 = (side * a1.cos() + Vec3::Y * a1.sin()) * radius;
        let normal = ((r0 + r1) * 0.5).normalize_or(Vec3::Y);
        mesh.quad(
            [
                center - half + r0,
                center - half + r1,
                center + half + r1,
                center + half + r0,
            ],
            normal,
            [
                Vec2::ZERO,
                Vec2::new(0.04, 0.0),
                Vec2::new(0.04, 0.15),
                Vec2::new(0.0, 0.15),
            ],
        );
    }
    // End discs so the rick reads as cut wood.
    for (end, direction) in [(center + half, axis), (center - half, -axis)] {
        for segment in 0..SEGMENTS {
            let a0 = segment as f32 / SEGMENTS as f32 * (PI * 2.0);
            let a1 = (segment + 1) as f32 / SEGMENTS as f32 * (PI * 2.0);
            let r0 = (side * a0.cos() + Vec3::Y * a0.sin()) * radius;
            let r1 = (side * a1.cos() + Vec3::Y * a1.sin()) * radius;
            mesh.triangle(
                [end, end + r0, end + r1],
                [Vec2::ZERO, Vec2::new(0.03, 0.0), Vec2::new(0.03, 0.03)],
                false,
            );
        }
        let _ = direction;
    }
}

/// Where one split log of a firewood rick lies: `base` is the clutter spot
/// beside the door, `normal` points out of the façade, and the rick is three
/// courses high by two deep.
///
/// The courses step 0.24 m up on a 0.23 m log, and the two columns step 0.3 m
/// along `normal` about the rick's 0.05 m stand-off from the plinth. The
/// doorway's along-wall direction is deliberately not a parameter: every log in
/// the rick is laid along it, so offsetting the second column that way would
/// only slide a 1.05 m log along its own length and bury three quarters of it
/// inside its twin.
fn firewood_log_center(base: Vec2, normal: Vec2, row: usize, column: usize) -> Vec3 {
    let stacked = base + normal * (0.05 + column as f32 * 0.3 - 0.15);
    Vec3::new(stacked.x, 0.14 + row as f32 * 0.24, stacked.y)
}

/// A slumped sack: a low-resolution squashed dome, batched.
fn add_sack(mesh: &mut MeshData, center: Vec3, radii: Vec3) {
    const SECTORS: usize = 8;
    const RINGS: usize = 4;
    for ring in 0..RINGS {
        let v0 = ring as f32 / RINGS as f32 * FRAC_PI_2_SACK;
        let v1 = (ring + 1) as f32 / RINGS as f32 * FRAC_PI_2_SACK;
        for sector in 0..SECTORS {
            let u0 = sector as f32 / SECTORS as f32 * (PI * 2.0);
            let u1 = (sector + 1) as f32 / SECTORS as f32 * (PI * 2.0);
            let point = |u: f32, v: f32| {
                center
                    + Vec3::new(
                        radii.x * v.cos() * u.cos(),
                        radii.y * v.sin(),
                        radii.z * v.cos() * u.sin(),
                    )
            };
            let normal = (point(u0, v0) + point(u1, v1) - center * 2.0).normalize_or(Vec3::Y);
            mesh.quad(
                [point(u0, v0), point(u1, v0), point(u1, v1), point(u0, v1)],
                normal,
                [
                    Vec2::ZERO,
                    Vec2::new(0.05, 0.0),
                    Vec2::new(0.05, 0.05),
                    Vec2::new(0.0, 0.05),
                ],
            );
        }
    }
}

const FRAC_PI_2_SACK: f32 = PI * 0.5;

fn point_in_polygon(point: Vec2, polygon: &[[f32; 2]]) -> bool {
    let mut inside = false;
    for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
        if (a[1] > point.y) != (b[1] > point.y)
            && point.x < (b[0] - a[0]) * (point.y - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// The five ways through the curtain, as (centre, clear width in metres).  The
/// wall is cut for each of them and the tower ring is kept out of each of them,
/// so they live here rather than inside one of the two.
const WALL_OPENINGS: [(Vec2, f32); 5] = [
    (Vec2::new(-24.5, 357.0), 18.0),  // the Wool Gate
    (Vec2::new(346.5, 94.5), 28.0),   // the Stone Gate
    (Vec2::new(10.5, -465.5), 18.0),  // the Harne Gate
    (Vec2::new(-353.5, -94.5), 37.0), // the River Gate
    (Vec2::new(-318.5, -374.5), 6.0), // the Reed Postern
];

fn build_fortifications(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    for (start, end) in plan
        .wall_polygon_xz
        .iter()
        .zip(plan.wall_polygon_xz.iter().cycle().skip(1))
    {
        let start = Vec2::from_array(*start);
        let end = Vec2::from_array(*end);
        for (segment_start, segment_end) in wall_ranges_around_gates(start, end, &WALL_OPENINGS) {
            spawn_wall_segment(
                commands,
                meshes,
                materials,
                collision_world,
                segment_start,
                segment_end,
            );
        }
    }

    // The map places a tower at every wall vertex and at roughly 115 m along
    // long curtains.  Keep the same deterministic rule in 3D.
    let mut tower_points = plan
        .wall_polygon_xz
        .iter()
        .map(|point| Vec2::from_array(*point))
        .collect::<Vec<_>>();
    for (start, end) in plan
        .wall_polygon_xz
        .iter()
        .zip(plan.wall_polygon_xz.iter().cycle().skip(1))
    {
        let start = Vec2::from_array(*start);
        let end = Vec2::from_array(*end);
        let divisions = (start.distance(end) / 115.0).floor() as usize + 1;
        for step in 1..divisions {
            tower_points.push(start.lerp(end, step as f32 / divisions as f32));
        }
    }
    // A tower is a 12 m square set corner-on, so it reaches this far from its
    // centre in the worst direction — out along a diagonal, to a corner.  The
    // collider below is that diamond exactly; the gate test just under here
    // wants the circumscribing radius instead, because a corner poking into an
    // arch bricks it up as surely as a face would, and which way the diagonal
    // happens to point is not worth the arithmetic.
    let tower_reach = 12.0 * SQRT_2 * 0.5;
    // That rule is blind to the gates, and at four of the five it plants a tower
    // in the very arch the curtain was just cut for: the Stone and River gates
    // end on a wall vertex, the Harne gate sits under a 115 m division, and the
    // next division along clips the Reed postern.  Leave those out rather than
    // shove them aside.  Every gate already carries its own flanking pair in the
    // cadastral plan (`gate_stone_1`/`_2` and kin, four storeys of limestone),
    // standing just past the shoulders of the arch, so the gateway keeps its
    // towers and the two corners keep theirs; a nudged curtain tower would only
    // crowd up against one of those.  The index is taken before the skip so a
    // surviving tower keeps the number — and the height — the bird's-eye map
    // gives it as `wall-tower-NN`.
    let arches = gate_arches(&plan.wall_polygon_xz, &WALL_OPENINGS);
    for (index, point) in tower_points.into_iter().enumerate() {
        if arches.iter().any(|(arch_start, arch_end)| {
            segment_distance_squared(point, *arch_start, *arch_end) < tower_reach * tower_reach
        }) {
            continue;
        }
        let height = 18.0 + (stable_hash(&format!("wall-tower-{index}")) % 500) as f32 / 100.0;
        spawn_rotated_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(point.x, height * 0.5, point.y),
            Vec3::new(12.0, height, 12.0),
            PI * 0.25,
            format!("Wall tower {:02}", index + 1),
        );
        spawn_mesh_named(
            commands,
            &meshes.pyramid,
            &materials.slate,
            Transform::from_xyz(point.x, height + 3.1, point.y)
                .with_rotation(Quat::from_rotation_y(PI * 0.25))
                .with_scale(Vec3::new(9.4, 5.2, 9.4)),
            format!("Wall tower {:02} roof", index + 1),
        );
        // The masonry is a diamond, so give it the diamond: its four corners lie
        // on the world axes, `tower_reach` out.  The circumscribing box this used
        // to register is twice the area — 288 m² of solid against 144 m² of
        // stone — and the surplus sits exactly where the player walks past a
        // tower face, stopping them against open paving.  This is the same
        // exact-footprint path every cadastral building takes.
        collision_world.add_convex_prism(
            &[
                [point.x + tower_reach, point.y],
                [point.x, point.y + tower_reach],
                [point.x - tower_reach, point.y],
                [point.x, point.y - tower_reach],
            ],
            0.0,
            height + 5.7,
        );
    }

    build_gatehouses(commands, meshes, materials, collision_world);
    gates::spawn_gate_mechanisms(
        commands,
        &meshes.cube,
        &meshes.cylinder,
        &materials.timber,
        &materials.dark_wood,
        &materials.iron,
    );
}

fn wall_ranges_around_gates(start: Vec2, end: Vec2, openings: &[(Vec2, f32)]) -> Vec<(Vec2, Vec2)> {
    let edge = end - start;
    let length = edge.length();
    let mut gaps = Vec::new();
    for (point, width) in openings {
        let t = (*point - start).dot(edge) / edge.length_squared();
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let projected = start + edge * t;
        if projected.distance(*point) <= 32.0 {
            let half_t = width * 0.5 / length;
            gaps.push(((t - half_t).clamp(0.0, 1.0), (t + half_t).clamp(0.0, 1.0)));
        }
    }
    gaps.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut ranges = Vec::new();
    let mut cursor = 0.0_f32;
    for (gap_start, gap_end) in gaps {
        if gap_start > cursor + 0.001 {
            ranges.push((start + edge * cursor, start + edge * gap_start));
        }
        cursor = cursor.max(gap_end);
    }
    if cursor < 0.999 {
        ranges.push((start + edge * cursor, end));
    }
    ranges
}

/// The stretches of curtain the gates take out, in world space — the complement
/// of what `wall_ranges_around_gates` leaves standing, and on the same terms
/// (an opening only belongs to the edge it projects onto within 32 m).  Nothing
/// solid may sit on one of these segments: they are the arches themselves.
fn gate_arches(polygon: &[[f32; 2]], openings: &[(Vec2, f32)]) -> Vec<(Vec2, Vec2)> {
    let mut arches = Vec::new();
    for (start, end) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
        let start = Vec2::from_array(*start);
        let end = Vec2::from_array(*end);
        let edge = end - start;
        let length = edge.length();
        for (point, width) in openings {
            let t = (*point - start).dot(edge) / edge.length_squared();
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            if (start + edge * t).distance(*point) > 32.0 {
                continue;
            }
            let half_t = width * 0.5 / length;
            arches.push((
                start + edge * (t - half_t).clamp(0.0, 1.0),
                start + edge * (t + half_t).clamp(0.0, 1.0),
            ));
        }
    }
    arches
}

fn spawn_wall_segment(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    start: Vec2,
    end: Vec2,
) {
    let edge = end - start;
    let length = edge.length();
    if length < 0.2 {
        return;
    }
    let center = (start + end) * 0.5;
    let yaw = -edge.y.atan2(edge.x);
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.limestone,
        Vec3::new(center.x, WALL_HEIGHT * 0.5, center.y),
        Vec3::new(length, WALL_HEIGHT, WALL_THICKNESS),
        yaw,
        "Ombreval city wall",
    );
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.paving,
        Vec3::new(center.x, WALL_HEIGHT + 0.12, center.y),
        Vec3::new(length, 0.24, WALL_THICKNESS + 1.5),
        yaw,
        "Ombreval wall walk",
    );

    let chunks = (length / 8.0).ceil().max(1.0) as usize;
    for chunk in 0..chunks {
        let a = start.lerp(end, chunk as f32 / chunks as f32);
        let b = start.lerp(end, (chunk + 1) as f32 / chunks as f32);
        let min = a.min(b) - Vec2::splat(WALL_THICKNESS * 0.65);
        let max = a.max(b) + Vec2::splat(WALL_THICKNESS * 0.65);
        collision_world.add_box(
            Vec3::new(min.x, 0.0, min.y),
            Vec3::new(max.x, WALL_HEIGHT + 0.5, max.y),
        );
    }
}

fn build_gatehouses(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let houses = [
        (
            "Wool Gate upper store",
            Vec3::new(-24.5, 12.0, 357.0),
            Vec3::new(58.0, 7.0, 7.0),
        ),
        (
            "Stone Gate upper store",
            Vec3::new(346.5, 12.0, 94.5),
            Vec3::new(7.0, 7.0, 68.0),
        ),
        (
            "Harne Gate upper store",
            Vec3::new(10.5, 12.0, -465.5),
            Vec3::new(58.0, 7.0, 7.0),
        ),
        (
            "River Gate upper store",
            Vec3::new(-353.5, 12.0, -94.5),
            Vec3::new(7.0, 7.0, 82.0),
        ),
    ];
    for (name, center, size) in houses {
        spawn_box_named(commands, meshes, &materials.limestone, center, size, name);
        spawn_mesh_named(
            commands,
            &meshes.pyramid,
            &materials.slate,
            Transform::from_translation(center + Vec3::Y * 5.7).with_scale(Vec3::new(
                size.x * 0.72,
                3.2,
                size.z * 0.72,
            )),
            format!("{name} roof"),
        );
        collision_world.add_box(center - size * 0.5, center + size * 0.5 + Vec3::Y * 3.5);
    }
}

fn spawn_place_markers(commands: &mut Commands, plan: &CityPlan) {
    for place in &plan.named_place_index {
        commands.spawn((
            Name::new(format!(
                "Place {:02}: {} ({})",
                place.number, place.name, place.kind
            )),
            LorePlaceNumber(place.number),
            Transform::from_xyz(place.anchor[0], 0.05, place.anchor[1]),
            Visibility::default(),
        ));
    }
}

fn add_surface_quad(
    mesh: &mut MeshData,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    y: f32,
    uv_span: f32,
) {
    mesh.quad(
        [
            Vec3::new(min_x, y, min_z),
            Vec3::new(min_x, y, max_z),
            Vec3::new(max_x, y, max_z),
            Vec3::new(max_x, y, min_z),
        ],
        Vec3::Y,
        [
            Vec2::new(min_x / uv_span, min_z / uv_span),
            Vec2::new(min_x / uv_span, max_z / uv_span),
            Vec2::new(max_x / uv_span, max_z / uv_span),
            Vec2::new(max_x / uv_span, min_z / uv_span),
        ],
    );
}

fn add_polygon_surface(mesh: &mut MeshData, polygon: &[[f32; 2]], y: f32, uv_span: f32) {
    for [a, b, c] in triangulate_polygon(polygon) {
        let points = [a, b, c].map(|index| {
            let point = polygon[index];
            Vec3::new(point[0], y, point[1])
        });
        mesh.triangle(
            points,
            points.map(|point| Vec2::new(point.x / uv_span, point.z / uv_span)),
            true,
        );
    }
}

fn add_road_ribbon(mesh: &mut MeshData, road: &Road, y: f32) {
    let half_width = road.width_m * 0.5;
    for pair in road.points.windows(2) {
        let a = Vec2::from_array(pair[0]);
        let b = Vec2::from_array(pair[1]);
        let direction = (b - a).normalize_or_zero();
        let side = Vec2::new(-direction.y, direction.x) * half_width;
        let points = [
            Vec3::new((a - side).x, y, (a - side).y),
            Vec3::new((a + side).x, y, (a + side).y),
            Vec3::new((b + side).x, y, (b + side).y),
            Vec3::new((b - side).x, y, (b - side).y),
        ];
        mesh.quad(
            points,
            Vec3::Y,
            points.map(|point| {
                Vec2::new(
                    point.x / FLOOR_TEXTURE_SPAN_METERS,
                    point.z / FLOOR_TEXTURE_SPAN_METERS,
                )
            }),
        );
    }
    for point in &road.points {
        add_disc_surface(mesh, Vec2::from_array(*point), half_width, y + 0.0002, 12);
    }
}

fn add_disc_surface(mesh: &mut MeshData, center: Vec2, radius: f32, y: f32, segments: usize) {
    for segment in 0..segments {
        let angle_a = segment as f32 * 2.0 * PI / segments as f32;
        let angle_b = (segment + 1) as f32 * 2.0 * PI / segments as f32;
        let a = center + Vec2::new(angle_a.cos(), angle_a.sin()) * radius;
        let b = center + Vec2::new(angle_b.cos(), angle_b.sin()) * radius;
        let points = [
            Vec3::new(center.x, y, center.y),
            Vec3::new(a.x, y, a.y),
            Vec3::new(b.x, y, b.y),
        ];
        mesh.triangle(
            points,
            points.map(|point| {
                Vec2::new(
                    point.x / FLOOR_TEXTURE_SPAN_METERS,
                    point.z / FLOOR_TEXTURE_SPAN_METERS,
                )
            }),
            true,
        );
    }
}

fn polygon_is_convex(polygon: &[[f32; 2]]) -> bool {
    let winding = plan::signed_area(polygon).signum();
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .zip(polygon.iter().cycle().skip(2))
        .all(|((a, b), c)| {
            let a = Vec2::from_array(*a);
            let b = Vec2::from_array(*b);
            let c = Vec2::from_array(*c);
            cross_2d(b - a, c - b) * winding >= -0.001
        })
}

fn triangulate_polygon(polygon: &[[f32; 2]]) -> Vec<[usize; 3]> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let mut remaining = if plan::signed_area(polygon) > 0.0 {
        (0..polygon.len()).collect::<Vec<_>>()
    } else {
        (0..polygon.len()).rev().collect::<Vec<_>>()
    };
    let mut triangles = Vec::with_capacity(polygon.len() - 2);
    let mut guard = polygon.len() * polygon.len();

    while remaining.len() > 3 && guard > 0 {
        guard -= 1;
        let mut clipped = false;
        for cursor in 0..remaining.len() {
            let previous = remaining[(cursor + remaining.len() - 1) % remaining.len()];
            let current = remaining[cursor];
            let next = remaining[(cursor + 1) % remaining.len()];
            let a = Vec2::from_array(polygon[previous]);
            let b = Vec2::from_array(polygon[current]);
            let c = Vec2::from_array(polygon[next]);
            if cross_2d(b - a, c - b) <= 0.0001 {
                continue;
            }
            if remaining.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_triangle(Vec2::from_array(polygon[candidate]), a, b, c)
            }) {
                continue;
            }
            triangles.push([previous, current, next]);
            remaining.remove(cursor);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if remaining.len() == 3 {
        triangles.push([remaining[0], remaining[1], remaining[2]]);
    }
    if triangles.len() != polygon.len() - 2 {
        warn!("falling back to fan triangulation for a plan polygon");
        (1..polygon.len() - 1)
            .map(|index| [0, index, index + 1])
            .collect()
    } else {
        triangles
    }
}

fn cross_2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn point_in_triangle(point: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let ab = cross_2d(b - a, point - a);
    let bc = cross_2d(c - b, point - b);
    let ca = cross_2d(a - c, point - c);
    ab >= -0.0001 && bc >= -0.0001 && ca >= -0.0001
}

fn polygon_center(polygon: &[[f32; 2]]) -> Vec2 {
    polygon
        .iter()
        .map(|point| Vec2::from_array(*point))
        .sum::<Vec2>()
        / polygon.len() as f32
}

fn stable_hash(text: &str) -> u32 {
    text.bytes().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ byte as u32).wrapping_mul(16_777_619)
    })
}

/// Tile edge for the static city batches. One mesh per material previously
/// spanned the whole 1.2×1.0 km map, so its AABB defeated every culling pass:
/// the full city was vertex-processed in the main view, in all four sun
/// cascades, and in every local shadow view, every frame (measured at ~6.8 M
/// vertex invocations per pass on the 2026-07 profiling night). Splitting each
/// batch into ground tiles keeps the shared material (draws still batch) while
/// giving the culling passes AABBs small enough to actually reject.
const BATCH_TILE_M: f32 = 128.0;

/// Distance fade for the small-detail batches, by batch name. The distance
/// fog is fully opaque past ~300 m, so geometry that only reads at close
/// range can stop being drawn well before that without a visible pop. The
/// margins are generous because a 128 m tile's AABB center can sit ~90 m
/// behind its nearest corner.
fn detail_fade_range(name: &str) -> Option<VisibilityRange> {
    const FINE: [&str; 12] = [
        "Street props",
        "Laundry",
        "strung ",
        "Hoist",
        "Shopfront awning",
        "Open balconies",
        "Yard stairs",
        "Passage posted notices",
        "Passage lantern",
        "Bellfoot notices",
        "Bellfoot lantern",
        "Square arcade posts",
    ];
    const MEDIUM: [&str; 4] = [
        "Ombreval doors and shutters",
        "Ombreval reveals, sills and lintels",
        "Ombreval timber framing",
        "Ombreval windows",
    ];
    let fade = |start: f32, end: f32| VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: start..end,
        use_aabb: true,
    };
    // `use_aabb` gauges against the AABB *center*, so a 128 m tile's nearest
    // corner sits up to ~90 m inside the printed band — these numbers are
    // chosen so the worst-case corner still fades where the fog has already
    // taken most of the contrast.
    if name.starts_with("Ombreval window rooms") {
        // Only the near-fade glass (opaque past ~22 m) can reveal a room, so
        // these tiles drop far earlier than any other batch: 22 m of pane
        // fade plus the ~91 m worst case from a window in one tile corner to
        // the tile-center anchor the fades gauge against.
        return Some(fade(120.0, 150.0));
    }
    if FINE.iter().any(|prefix| name.starts_with(prefix)) {
        return Some(fade(330.0, 390.0));
    }
    if MEDIUM.iter().any(|prefix| name.starts_with(prefix)) {
        return Some(fade(430.0, 490.0));
    }
    None
}

fn spawn_batch<M: Material>(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<M>,
    data: MeshData,
    name: impl Into<String>,
) {
    if data.is_empty() {
        return;
    }
    let name = name.into();
    let fade = detail_fade_range(&name);
    // The room shells sit inside the building shells: no shadow view can see
    // them, so keep their vertices out of every shadow cascade.
    let no_shadow = name.starts_with("Ombreval window rooms");
    for (tile_x, tile_z, mut tile) in split_batch_into_tiles(data, BATCH_TILE_M) {
        // Anchor each tile entity at its tile center. The GPU cross-fade for
        // `VisibilityRange` gauges camera distance against the ENTITY
        // TRANSLATION (mesh.wgsl passes `world_from_local[3]`), not the AABB:
        // a batch parked at the origin fades by the camera's distance to the
        // world origin, which dithered whole batches out at the city's edges
        // and made every band tighter than ~|camera| invisible everywhere.
        let center = Vec3::new(
            (tile_x as f32 + 0.5) * BATCH_TILE_M,
            0.0,
            (tile_z as f32 + 0.5) * BATCH_TILE_M,
        );
        for position in &mut tile.positions {
            position[0] -= center.x;
            position[2] -= center.z;
        }
        let mut entity = commands.spawn((
            Name::new(format!("{name} [{tile_x},{tile_z}]")),
            Mesh3d(meshes.add(tile.into_mesh())),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center),
        ));
        if let Some(fade) = fade.clone() {
            entity.insert(fade);
        }
        if no_shadow {
            entity.insert(NotShadowCaster);
        }
    }
}

/// Buckets a batch's triangles into ground tiles by centroid. Vertices are
/// re-indexed per tile; the builder never shares vertices between triangles,
/// so this duplicates nothing in practice.
fn split_batch_into_tiles(data: MeshData, tile_m: f32) -> Vec<(i32, i32, MeshData)> {
    let mut tiles: BTreeMap<(i32, i32), (MeshData, HashMap<u32, u32>)> = BTreeMap::new();
    for triangle in data.indices.chunks_exact(3) {
        let centroid = triangle
            .iter()
            .map(|&index| Vec3::from_array(data.positions[index as usize]))
            .sum::<Vec3>()
            / 3.0;
        let key = (
            (centroid.x / tile_m).floor() as i32,
            (centroid.z / tile_m).floor() as i32,
        );
        let (tile, remap) = tiles.entry(key).or_default();
        for &index in triangle {
            let mapped = *remap.entry(index).or_insert_with(|| {
                let next = tile.positions.len() as u32;
                let source = index as usize;
                tile.positions.push(data.positions[source]);
                tile.normals.push(data.normals[source]);
                tile.uvs.push(data.uvs[source]);
                tile.colors.push(data.colors[source]);
                next
            });
            tile.indices.push(mapped);
        }
    }
    tiles
        .into_iter()
        .map(|((tile_x, tile_z), (tile, _))| (tile_x, tile_z, tile))
        .collect()
}

fn spawn_box_named<M: Material>(
    commands: &mut Commands,
    meshes: &CityMeshes,
    material: &Handle<M>,
    center: Vec3,
    size: Vec3,
    name: impl Into<String>,
) {
    spawn_mesh_named(
        commands,
        &meshes.cube,
        material,
        Transform::from_translation(center).with_scale(size),
        name,
    );
}

fn spawn_rotated_box_named(
    commands: &mut Commands,
    meshes: &CityMeshes,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    yaw: f32,
    name: impl Into<String>,
) {
    spawn_mesh_named(
        commands,
        &meshes.cube,
        material,
        Transform::from_translation(center)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(size),
        name,
    );
}

fn spawn_cylinder(
    commands: &mut Commands,
    meshes: &CityMeshes,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    radius: f32,
    height: f32,
) {
    spawn_mesh_named(
        commands,
        &meshes.cylinder,
        material,
        Transform::from_translation(center).with_scale(Vec3::new(radius, height, radius)),
        "Cylindrical city detail",
    );
}

fn spawn_mesh_named<M: Material>(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    material: &Handle<M>,
    transform: Transform,
    name: impl Into<String>,
) {
    commands.spawn((
        Name::new(name.into()),
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        transform,
    ));
}

fn add_rotated_box_collider(
    collision_world: &mut CollisionWorld,
    base: Vec3,
    size: Vec2,
    yaw: f32,
    height: f32,
) {
    add_rotated_box_collider_at(
        collision_world,
        base + Vec3::Y * height * 0.5,
        Vec3::new(size.x, height, size.y),
        yaw,
    );
}

fn add_rotated_box_collider_at(
    collision_world: &mut CollisionWorld,
    center: Vec3,
    size: Vec3,
    yaw: f32,
) {
    let (sin, cos) = yaw.sin_cos();
    let half_x = (cos.abs() * size.x + sin.abs() * size.z) * 0.5;
    let half_z = (sin.abs() * size.x + cos.abs() * size.z) * 0.5;
    collision_world.add_box(
        center - Vec3::new(half_x, size.y * 0.5, half_z),
        center + Vec3::new(half_x, size.y * 0.5, half_z),
    );
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use bevy::asset::{AssetApp, AssetPlugin};

    use super::*;
    use crate::controller::{WALK_BAND_HI, WALK_BAND_LO};

    const NAV_JSON: &str = include_str!("../../assets/world/navigation.json");
    const NAV_BIN: &[u8] = include_bytes!("../../assets/world/navigation.bin");

    fn built_collision_world() -> CollisionWorld {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();
        std::mem::take(app.world_mut().resource_mut::<CollisionWorld>().as_mut())
    }

    /// Closes the loop between the two obstacle sets: the sim bakes the walkable
    /// surface from the cadastral plan, and this proves the bake agrees with what
    /// actually stops the player. No cell that an NPC may stand on is inside a
    /// `CollisionWorld` solid at walking height (02_navigation.md §2, §8).
    #[test]
    fn no_walkable_cell_is_solid() {
        let collision = built_collision_world();
        let nav = cathedral_sim::NavData::from_parts(NAV_JSON, NAV_BIN)
            .expect("the committed navigation artifact loads");
        let grid = nav.grid();

        // The player is a standing AABB, not a point at WALK_Y, so a cell is solid
        // if any collider whose vertical extent overlaps the walk band
        // [WALK_BAND_LO, WALK_BAND_HI] covers its XZ. A collider that tops out
        // below WALK_Y (a water trough, the bellstand platform, a cistern rim)
        // still stops the player and so must not be walkable — the earlier
        // single-plane `contains_point(_, WALK_Y, _)` check was blind to those.
        let mut violations = Vec::new();
        for footprint in collision.solid_footprints_in_band(WALK_BAND_LO, WALK_BAND_HI) {
            let min_x = footprint.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
            let max_x = footprint
                .iter()
                .map(|p| p.x)
                .fold(f32::NEG_INFINITY, f32::max);
            let min_z = footprint.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
            let max_z = footprint
                .iter()
                .map(|p| p.y)
                .fold(f32::NEG_INFINITY, f32::max);
            let col_lo = (((min_x as f64 - grid.x0) / grid.cell_m).floor()).max(0.0) as usize;
            let col_hi = (((max_x as f64 - grid.x0) / grid.cell_m).ceil() as usize).min(grid.w - 1);
            let row_lo = (((min_z as f64 - grid.z0) / grid.cell_m).floor()).max(0.0) as usize;
            let row_hi = (((max_z as f64 - grid.z0) / grid.cell_m).ceil() as usize).min(grid.h - 1);
            for row in row_lo..=row_hi {
                for col in col_lo..=col_hi {
                    let (cx, cz) = grid.centre(row, col);
                    if nav.is_walkable(cx, cz)
                        && collision.blocks_walk_band(
                            cx as f32,
                            cz as f32,
                            WALK_BAND_LO,
                            WALK_BAND_HI,
                        )
                    {
                        violations.push((cx, cz));
                    }
                }
            }
        }
        assert!(
            violations.is_empty(),
            "{} baked walkable cells are inside a collider, e.g. {:?}",
            violations.len(),
            &violations[..violations.len().min(12)]
        );
    }

    /// `the_cut_kerb.md` M3 — the Cut collides **exactly** its riser walls and
    /// its sixteen bollards, and nothing else it did not always collide.
    ///
    /// This is the successor to M0–M2's `the_cut_kerb_adds_nothing_to_the_
    /// collision_world`, which guarded the opposite invariant while the street
    /// was drawn-only. Both halves stay load-bearing. The export equality
    /// forces the four-step nav chain to be re-run whenever these colliders
    /// move — the committed walkable surface must always be the complement of
    /// what actually stops the player. The exact accounting keeps the kerb
    /// line from quietly growing solids the bake would erode into a wall: the
    /// margin's connection to its cartway hangs entirely on the gaps between
    /// these boxes (ten junction mouths, seven kerb breaks at 3 m — 2.3 m
    /// after erosion by the 0.35 m agent radius — and five 7.2 m stair gaps),
    /// so an unplanned collider here is how the whole margin gets dropped from
    /// the main component and every Cut-facing door strands.
    #[test]
    fn the_cut_collides_exactly_the_riser_and_the_bollards() {
        let collision = built_collision_world();
        let built: Vec<Vec<[f32; 2]>> = collision
            .solid_footprints_in_band(WALK_BAND_LO, WALK_BAND_HI)
            .into_iter()
            .map(|poly| poly.into_iter().map(|point| [point.x, point.y]).collect())
            .collect();
        let committed: serde_json::Value =
            serde_json::from_str(include_str!("../../assets/world/collision_footprints.json"))
                .expect("the committed collision export parses");
        let committed: Vec<Vec<[f32; 2]>> =
            serde_json::from_value(committed["footprints"].clone())
                .expect("the committed collision export has footprints");
        assert_eq!(
            built.len(),
            committed.len(),
            "the scene now exports {} collider footprints, not the {} the \
             committed navigation was baked against — re-run the four-step nav \
             chain",
            built.len(),
            committed.len()
        );
        assert_eq!(built, committed, "the collider export has drifted");

        // The expected set: one box per laid run, at the kerbstone's own
        // width, and one 0.42 m box per bollard.
        let plan = plan::load();
        let mut expected: Vec<[f32; 4]> = cut_kerb_plan(&plan)
            .iter()
            .filter(|run| run.laid)
            .map(|run| {
                [
                    run.x - CUT_KERB_WIDTH_M * 0.5,
                    run.z0,
                    run.x + CUT_KERB_WIDTH_M * 0.5,
                    run.z1,
                ]
            })
            .collect();
        expected.extend(
            CUT_FURNITURE
                .iter()
                .filter(|(prop, _, _)| *prop == CutProp::Bollard)
                .map(|(_, bank, z)| {
                    [bank.kerb_x() - 0.21, z - 0.21, bank.kerb_x() + 0.21, z + 0.21]
                }),
        );

        // Every footprint *contained* in either kerb line's half-metre band is
        // one of the expected boxes, and every expected box is found exactly
        // once. Containment rather than overlap keeps the Old Sluice out of
        // the accounting: its fifty-metre shell straddles both lines where the
        // south reach nominally ends, as it always has.
        let bbox = |footprint: &Vec<[f32; 2]>| {
            footprint.iter().fold(
                [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
                |[x0, z0, x1, z1], [x, z]| [x0.min(*x), z0.min(*z), x1.max(*x), z1.max(*z)],
            )
        };
        let on_the_line: Vec<[f32; 4]> = built
            .iter()
            .map(bbox)
            .filter(|[x0, _, x1, _]| {
                [CutBank::West, CutBank::East].iter().any(|bank| {
                    *x0 >= bank.kerb_x() - 0.5 && *x1 <= bank.kerb_x() + 0.5
                })
            })
            .collect();
        for want in &expected {
            let hits = on_the_line
                .iter()
                .filter(|have| {
                    want.iter()
                        .zip(have.iter())
                        .all(|(a, b)| (a - b).abs() < 1.0e-3)
                })
                .count();
            assert_eq!(
                hits, 1,
                "the collider ({}, {})..({}, {}) the kerb intends is exported \
                 {hits} times",
                want[0], want[1], want[2], want[3]
            );
        }
        assert_eq!(
            on_the_line.len(),
            expected.len(),
            "{} colliders stand on the kerb lines but the kerb intends only \
             {}: something else has grown a solid on the one strip whose gaps \
             keep the margin connected",
            on_the_line.len(),
            expected.len()
        );
    }

    /// `the_cut_kerb.md` M3 — after the rebake, the raised margin is still
    /// *ground*: walkable in the committed bitset along all three laid
    /// reaches, on both banks, and crossable at every kerb break. The bake
    /// keeps only the single largest connected component, so a walkable
    /// margin cell in `navigation.bin` **is** a connected margin cell — if
    /// the riser had severed the margin, the component would have been
    /// dropped and these cells would read blocked. This is the proof the
    /// erosion arithmetic (3 m break − 2 × 0.35 m radius = 2.3 m of surviving
    /// crossing) actually held on the shipped artifact.
    #[test]
    fn the_cut_margin_stays_connected_to_its_cartway() {
        let plan = plan::load();
        let nav = cathedral_sim::NavData::from_parts(NAV_JSON, NAV_BIN)
            .expect("the committed navigation artifact loads");
        let strips = cut_margin_strips(&plan);
        let walkable_near = |x: f32, z: f32| {
            (-1..=1).any(|dx| {
                (-1..=1).any(|dz| {
                    nav.is_walkable(f64::from(x) + f64::from(dx) * 0.25, f64::from(z)
                        + f64::from(dz) * 0.25)
                })
            })
        };

        let mut sampled = 0;
        for bank in [CutBank::West, CutBank::East] {
            // Two metres behind the line: clear of the riser's eroded metre,
            // clear of the housefronts' own erosion.
            let x = bank.kerb_x() + bank.outward() * 2.0;
            for (z0, z1) in CUT_LAID_REACHES {
                let steps = ((z1 - z0) / 3.0).ceil() as usize;
                for step in 0..=steps {
                    let z = z0 + (z1 - z0) * step as f32 / steps as f32;
                    // Only where the margin actually has flags — junction
                    // mouths, stair trenches and ramp lanes answer for
                    // themselves.
                    if !strips
                        .iter()
                        .any(|[sx0, sz0, sx1, sz1]| {
                            *sx0 <= x && x <= *sx1 && *sz0 + 0.5 <= z && z <= *sz1 - 0.5
                        })
                    {
                        continue;
                    }
                    sampled += 1;
                    assert!(
                        walkable_near(x, z),
                        "the {bank:?} margin at ({x}, {z}) is not walkable in \
                         the committed bake — the riser has cut it off"
                    );
                }
            }
        }
        assert!(
            sampled > 250,
            "only {sampled} margin points were sampled; the strips have \
             collapsed and the test is vacuous"
        );

        // The load-bearing crossings themselves: straight through every kerb
        // break, cartway to flags.
        for (prop, bank, z) in CUT_FURNITURE {
            if prop != CutProp::KerbBreak {
                continue;
            }
            for u in [-0.9_f32, 0.0, 0.9] {
                let x = bank.kerb_x() + bank.outward() * u;
                assert!(
                    walkable_near(x, z),
                    "the kerb break at z {z} on the {bank:?} bank is not \
                     crossable at u {u} — the load-bearing gap did not survive \
                     the erosion"
                );
            }
        }

        // And the cartway is still the cartway.
        for z in [-380.0, -180.0, -60.0, 150.0, 300.0] {
            assert!(
                walkable_near(CUT_CENTRE_X, z),
                "the cartway itself is blocked at z {z}"
            );
        }
    }

    /// `the_cut_kerb.md` M3 — the step under feet agrees with the drawn
    /// street: flat `CUT_STEP_M` on the flags, the true incline on a kerb
    /// break's ramp, the tread tops down a water stair, zero on the cartway
    /// and everywhere else in the city — and **no feather where a stair or
    /// ramp abuts the flags**, because the feather is for open edges
    /// (junction mouths, reach ends) and a feather at a furniture seam would
    /// drop feet a quarter-metre and hand them straight back.
    #[test]
    fn the_cut_step_is_under_feet_exactly_where_the_street_is_raised() {
        let plan = plan::load();
        let profile = cut_margin_profile(&plan);
        let west = CutBank::West;
        let kerb_x = west.kerb_x();

        // Deep in a flagged strip on the dead-true middle stretch.
        assert_eq!(profile.ground_lift(kerb_x - 3.0, -60.0), CUT_STEP_M);
        // The cartway, the far side of the city, and a square.
        assert_eq!(profile.ground_lift(CUT_CENTRE_X, -60.0), 0.0);
        assert_eq!(profile.ground_lift(0.0, 95.0), 0.0);
        assert_eq!(profile.ground_lift(CUT_CENTRE_X - 3.0, 60.0), 0.0);

        // The west kerb break at z -45.8: halfway up the ramp is half a step,
        // within the tolerance of the sill's own width.
        let mid = profile.ground_lift(kerb_x - (CUT_BREAK_RAMP_RUN_M + 0.15) * 0.5, -45.8);
        assert!(
            (mid - CUT_STEP_M * 0.5).abs() < 0.03,
            "the ramp's midpoint lifts {mid}, not about half the step"
        );

        // The west water stair at z -40: the head landing under feet is the
        // drawn landing, and a mid-flight tread is the drawn tread.
        assert_eq!(profile.ground_lift(kerb_x - 2.0, -40.0), CUT_BANK_TOP_Y);
        assert_eq!(
            profile.ground_lift(kerb_x - 0.5, -40.0),
            cut_stair_ground(0.5)
        );

        // A step off the stair trench's edge onto the flags is flat flags —
        // the strip end abutting the trench must not feather.
        let half_head = cut_stair_half_head(cut_stair_seed(west, -40.0));
        let beside = -40.0 - half_head - 0.34 - 0.01;
        assert_eq!(
            profile.ground_lift(kerb_x - 2.0, beside),
            CUT_STEP_M,
            "the flags feather into the stair trench they should meet at height"
        );

        // The wall lane behind the west bank (`south_inner_wall`, x -224,
        // z 188..212) deletes the margin's outer lanes for that stretch, so
        // the surviving lane's x-edge at -221.85 stands open in the middle of
        // the margin: feet must feather through it exactly as they do at an
        // open z-end, not teleport the full step crossing a 31 m line.
        assert_eq!(
            profile.ground_lift(-222.0, 200.0),
            0.0,
            "the wall lane should still bare the outer margin lanes"
        );
        let near_edge = profile.ground_lift(-221.70, 200.0);
        let expected = CUT_STEP_M * (0.15 / CUT_STEP_FEATHER_M);
        assert!(
            (near_edge - expected).abs() < 0.02,
            "0.15 m inside the open x-edge should feather to ~{expected}, not {near_edge}"
        );
        assert_eq!(
            profile.ground_lift(-221.2, 200.0),
            CUT_STEP_M,
            "past the feather band the surviving lane is full flags"
        );
        // The kerb-side edge never feathers, wall lane or no wall lane: the
        // riser collider stands there and a feathered lift would sink feet
        // into the flags.
        assert_eq!(
            profile.ground_lift(kerb_x - 0.05, 200.0),
            CUT_STEP_M,
            "the kerb line's own edge must stay full height"
        );
    }

    /// M3 lifts the whole street's furniture with the ground, and the doors
    /// are furniture too: the gazetteer has the Cut's housefronts on
    /// *"slightly raised thresholds"*, and before this contract the margin's
    /// flags rose a quarter-metre while every Cut-facing sill stayed at
    /// `y 0.095` — buried 0.185 m under the paving, with the kerb breaks
    /// lawfully delivering carts into a doorway they would drop into. So:
    /// every door whose threshold stands on the raised margin must get the
    /// full step under it (`add_door_module`'s `ground_lift`), never a
    /// feathered fraction (the drawn flags are flat — a partial lift is a
    /// sill drawn *inside* the stone), and the sill must come out proud of
    /// the flag tops by the same 0.065 m it stands proud of grade everywhere
    /// else in the city.
    #[test]
    fn the_cut_facing_doors_keep_their_thresholds_proud_of_the_flags() {
        let plan = plan::load();
        let profile = cut_margin_profile(&plan);
        let doors = door_edges();
        let mut lifted = 0;
        for building in &plan.buildings {
            let Some(&edge_index) = doors.get(&building.id) else {
                continue;
            };
            let polygon = &building.polygon;
            let a = Vec2::from_array(polygon[edge_index]);
            let b = Vec2::from_array(polygon[(edge_index + 1) % polygon.len()]);
            let edge = b - a;
            let length = edge.length();
            if length < 0.01 {
                continue;
            }
            let direction = edge / length;
            let orientation = plan::signed_area(polygon).signum();
            let mut normal = Vec2::new(edge.y, -edge.x).normalize() * orientation;
            if point_in_polygon(a + direction * (length * 0.5) + normal * 0.5, polygon) {
                normal = -normal;
            }
            // The door sits at the door edge's midpoint (`plan_facade_openings`)
            // and the threshold is sampled 0.3 m out from the wall, exactly as
            // `add_facade_openings_on` samples it.
            let step = a + direction * (length * 0.5) + normal * 0.3;
            let lift = profile.ground_lift(step.x, step.y);
            if lift <= 0.0 {
                continue;
            }
            lifted += 1;
            assert_eq!(
                lift, CUT_STEP_M,
                "{}'s doorway at ({:.1}, {:.1}) gets a partial lift — a sill \
                 drawn inside the flags",
                building.id, step.x, step.y
            );
            let (base_y, _) = building_verticals(building);
            // `add_door_module`'s slab: centre `foot_y + 0.045`, half 0.05.
            let threshold_top = base_y + lift + 0.095;
            assert!(
                threshold_top > CUT_MARGIN_Y + CUT_STEP_M,
                "{}'s threshold top {threshold_top} is not proud of the flags",
                building.id
            );
        }
        // Counted off the shipped plan: every Cut-facing door inside the three
        // laid reaches stands dead on a façade line, and for every one of them
        // the outermost flag lane reaches its wall (a kerb break's ramp and a
        // stair's trench part the flags nearer the kerb, never against the
        // housefronts). A change here means a façade or a reach moved —
        // re-count before amending.
        assert_eq!(lifted, 22, "the Cut margin should carry 22 doorways");
    }

    /// `the_cut_kerb.md` §2.4 / §6.2 — a laid ridge never crosses one of the two
    /// squares the ribbon runs through. Inside the Tallage and Maren's Green the
    /// boundary is a rule the Bench asserts, so it is drawn flush; a stone there
    /// would say the line is older and harder than it is.
    #[test]
    fn the_cut_kerb_is_never_a_ridge_inside_a_square() {
        let plan = plan::load();
        let squares: Vec<&Site> = plan
            .sites
            .iter()
            .filter(|site| site.id == "tallage" || site.id == "marens_green")
            .collect();
        assert_eq!(squares.len(), 2, "both squares are still in the plan");

        let runs = cut_kerb_plan(&plan);
        for run in runs.iter().filter(|run| run.laid) {
            let steps = ((run.z1 - run.z0) / 0.5).ceil().max(1.0) as usize;
            for step in 0..=steps {
                let z = run.z0 + (run.z1 - run.z0) * step as f32 / steps as f32;
                for square in &squares {
                    assert!(
                        !point_in_polygon(Vec2::new(run.x, z), &square.polygon),
                        "a laid kerbstone at ({}, {z}) stands inside {}",
                        run.x,
                        square.id
                    );
                }
            }
        }
        assert!(
            runs.iter().any(|run| !run.laid),
            "the squares still get a marked line"
        );
    }

    /// `the_cut_kerb.md` §3 / §6.3 — both lines stay exactly five metres off the
    /// centreline for the whole length of every reach. The Cut does not bend, and
    /// a kerb that wandered would leave the cartway wider in one place than
    /// another, which is precisely the distinction the feature exists to make.
    ///
    /// The offset is measured against the `cut` road **as the plan ships it**,
    /// not against `CUT_CENTRE_X`: `cut_kerb_plan` builds every run as
    /// `CUT_CENTRE_X ± CUT_KERB_OFFSET_M`, so comparing the two would only ever
    /// re-check that addition. What can actually go wrong is the plan moving
    /// under a hard-coded line — the 0.7× city shrink of 2026-07 rewrote every
    /// coordinate in the file — and that is what this reads.
    #[test]
    fn the_cut_kerb_holds_five_metres_off_the_plans_own_centreline() {
        let plan = plan::load();
        let cut = plan
            .roads
            .iter()
            .find(|road| road.tier == "cut")
            .expect("the plan still has the Cut");
        assert_eq!(cut.points.len(), 2, "the Cut is still one straight segment");
        assert_eq!(
            cut.points[0][0], cut.points[1][0],
            "the Cut is still parallel to z"
        );
        assert_eq!(
            cut.points[0][0], CUT_CENTRE_X,
            "the plan moved the Cut's centreline; the kerb is still drawn at {CUT_CENTRE_X}"
        );
        assert!(
            CUT_KERB_OFFSET_M * 2.0 <= cut.width_m,
            "the ten-metre cartway no longer fits inside the {} m ribbon",
            cut.width_m
        );

        // Every authored reach lies inside the ribbon the plan draws, and the
        // laid reaches plus the marked ones tile it end to end with no third
        // kind of ground in between.
        let (ribbon_lo, ribbon_hi) = (
            cut.points[0][1].min(cut.points[1][1]),
            cut.points[0][1].max(cut.points[1][1]),
        );
        let mut authored: Vec<(f32, f32)> = CUT_LAID_REACHES
            .iter()
            .chain(CUT_MARKED_REACHES.iter())
            .copied()
            .collect();
        authored.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(authored[0].0, ribbon_lo, "the reaches start where the Cut does");
        assert_eq!(
            authored[authored.len() - 1].1,
            ribbon_hi,
            "the reaches end where the Cut does"
        );
        for pair in authored.windows(2) {
            assert_eq!(pair[0].1, pair[1].0, "a gap between authored reaches");
        }

        // The marked reaches are the two squares, at their own polygon extents.
        for id in ["tallage", "marens_green"] {
            let square = plan
                .sites
                .iter()
                .find(|site| site.id == id)
                .unwrap_or_else(|| panic!("{id} is still in the plan"));
            let lo = square
                .polygon
                .iter()
                .fold(f32::MAX, |lo, point| lo.min(point[1]));
            let hi = square
                .polygon
                .iter()
                .fold(f32::MIN, |hi, point| hi.max(point[1]));
            assert!(
                CUT_MARKED_REACHES
                    .iter()
                    .any(|(z0, z1)| (z0 - lo).abs() < 0.05 && (z1 - hi).abs() < 0.05),
                "{id} spans z {lo}..{hi}, which is not one of the marked reaches"
            );
        }

        let runs = cut_kerb_plan(&plan);
        assert!(!runs.is_empty(), "the kerb is authored");
        for run in &runs {
            assert_eq!(
                (run.x - cut.points[0][0]).abs(),
                CUT_KERB_OFFSET_M,
                "a kerb run wandered to x {}",
                run.x
            );
            assert!(run.z1 > run.z0, "a kerb run runs the wrong way");
        }

        // Each reach is covered end to end apart from its openings, and every
        // metre it loses is one `cut_side_gaps` accounts for — a run silently
        // dropped for any other reason shows up here.
        for bank in [CutBank::West, CutBank::East] {
            let x = bank.kerb_x();
            let mut gaps = cut_side_gaps(&plan, x, x);
            gaps.extend(cut_furniture_kerb_gaps(bank));
            for (z0, z1) in CUT_LAID_REACHES {
                let covered: f32 = runs
                    .iter()
                    .filter(|run| run.x == x && run.laid && run.z0 >= z0 - 0.1 && run.z1 <= z1 + 0.1)
                    .map(|run| run.z1 - run.z0)
                    .sum();
                let opened: f32 = gaps
                    .iter()
                    .map(|(lo, hi)| (hi.min(z1) - lo.max(z0)).max(0.0))
                    .sum();
                let length = z1 - z0;
                assert!(
                    covered + opened > length - 2.0,
                    "the reach {z0}..{z1} at x {x} loses {:.1} m to nothing at all",
                    length - covered - opened
                );
                assert!(
                    covered > length * 0.7,
                    "the reach {z0}..{z1} at x {x} is only kerbed for {covered:.1} of {length:.1} m"
                );
            }
        }
    }

    /// The rest of the sounding envelope from `the_cut_kerb.md` M2 — the
    /// shallow end and the two lengths. They live here rather than beside
    /// `CUT_SOUNDINGS` because only the assertions read them; the renderer needs
    /// `CUT_SAG_MAX_M` alone.
    const CUT_SAG_MIN_M: f32 = 0.15;
    const CUT_SOUNDING_MIN_LENGTH_M: f32 = 40.0;
    const CUT_SOUNDING_MAX_LENGTH_M: f32 = 80.0;
    /// The most a sounding may have taken the line at a piece of authored M1
    /// furniture that is *laid* rather than driven — a water stair's mooring
    /// stone, a kerb break's flush slab. Both are drawn at their own fixed
    /// height, so a trough under either leaves the mooring stone standing alone
    /// in a drowned line and the kerb break unreadable against it. Bollards are
    /// exempt: a post is driven into the ground, not laid on it.
    const CUT_SAG_AT_LAID_FURNITURE_M: f32 = 0.06;

    /// `the_cut_kerb.md` M2 — the soundings stay inside the envelope the feature
    /// gives them, and stay in the places the rest of the street can carry.
    ///
    /// The envelope is not decoration. Under 0.15 m the dip is not a dip, it is
    /// per-stone weathering. Over 0.25 m, or shorter than forty metres, it stops
    /// being a settling street and becomes a step somebody would report as a
    /// bug. Outside a laid reach it has nothing to sag — the squares are drawn
    /// flush — and two soundings overlapping would sum into a hole neither of
    /// them authored.
    #[test]
    fn the_cut_soundings_stay_inside_the_authored_envelope() {
        assert!(
            (3..=4).contains(&CUT_SOUNDINGS.len()),
            "M2 authors three or four soundings, not {}",
            CUT_SOUNDINGS.len()
        );

        let mut spans: Vec<(f32, f32)> = Vec::new();
        for sounding in CUT_SOUNDINGS {
            let length = sounding.half_length_m * 2.0;
            assert!(
                (CUT_SOUNDING_MIN_LENGTH_M..=CUT_SOUNDING_MAX_LENGTH_M).contains(&length),
                "the sounding at z {} is {length} m long",
                sounding.z
            );
            for (side, depth) in [("west", sounding.west_m), ("east", sounding.east_m)] {
                assert!(
                    (CUT_SAG_MIN_M..=CUT_SAG_MAX_M).contains(&depth),
                    "the sounding at z {} takes the {side} line down {depth} m",
                    sounding.z
                );
            }
            let span = (sounding.z - sounding.half_length_m, sounding.z + sounding.half_length_m);
            assert!(
                CUT_LAID_REACHES
                    .iter()
                    .any(|(z0, z1)| *z0 <= span.0 && span.1 <= *z1),
                "the sounding at z {} runs from {} to {}, which is not inside one laid reach",
                sounding.z,
                span.0,
                span.1
            );
            for (z0, z1) in CUT_MARKED_REACHES {
                assert!(
                    span.1 <= z0 || span.0 >= z1,
                    "the sounding at z {} reaches into the marked reach {z0}..{z1}, \
                     where there is no ridge to take down",
                    sounding.z
                );
            }
            spans.push(span);
        }
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in spans.windows(2) {
            assert!(
                pair[0].1 < pair[1].0,
                "the soundings {:?} and {:?} overlap",
                pair[0],
                pair[1]
            );
        }

        // The profile itself: zero at and beyond the shoulders, exactly the
        // authored depth at the deepest point, and monotonic in between, so the
        // dip has one bottom rather than a rippled floor.
        for sounding in CUT_SOUNDINGS {
            for (bank, depth) in [
                (CutBank::West, sounding.west_m),
                (CutBank::East, sounding.east_m),
            ] {
                assert!(
                    (cut_sounding_sag(bank, sounding.z) - depth).abs() < 1.0e-5,
                    "the sounding at z {} does not reach its own depth",
                    sounding.z
                );
                for edge in [
                    sounding.z - sounding.half_length_m,
                    sounding.z + sounding.half_length_m,
                    sounding.z - sounding.half_length_m - 5.0,
                    sounding.z + sounding.half_length_m + 5.0,
                ] {
                    assert_eq!(
                        cut_sounding_sag(bank, edge),
                        0.0,
                        "the sounding at z {} is still {} m down at z {edge}",
                        sounding.z,
                        cut_sounding_sag(bank, edge)
                    );
                }
                let steps = 40;
                let mut previous = 0.0_f32;
                for step in 0..=steps {
                    let z = sounding.z - sounding.half_length_m
                        + sounding.half_length_m * step as f32 / steps as f32;
                    let sag = cut_sounding_sag(bank, z);
                    assert!(
                        sag >= previous - 1.0e-6,
                        "the sounding at z {} rises again on its way down, at z {z}",
                        sounding.z
                    );
                    previous = sag;
                }
            }
        }

        // And most of the street is dead true — that is the whole reason four
        // dips can be read at all.
        let mut sagging = 0.0_f32;
        let mut sampled = 0.0_f32;
        for (z0, z1) in CUT_LAID_REACHES {
            let steps = ((z1 - z0) / 1.0).ceil() as usize;
            for step in 0..=steps {
                let z = z0 + (z1 - z0) * step as f32 / steps as f32;
                sampled += 1.0;
                if cut_sounding_sag(CutBank::West, z) > 0.005
                    || cut_sounding_sag(CutBank::East, z) > 0.005
                {
                    sagging += 1.0;
                }
            }
        }
        assert!(
            sagging / sampled < 0.45,
            "{:.0}% of the laid line is sagging; the soundings are the exception, \
             not the profile",
            100.0 * sagging / sampled
        );
    }

    /// `the_cut_kerb.md` M2 — the sag moves the kerbstone and nothing else.
    ///
    /// The M0 invariant (§6.3, `|x + 213.5| = 5.0` for the full length of every
    /// reach) has to survive it: the easiest way to draw a settling line is to
    /// let the stones wander, and a line that wanders is not a straightedge and
    /// cannot publish anything. This also pins the heights the drawing depends
    /// on. Since M3 put the step under the line the profile is drawn at its
    /// **full authored depth** — the six-centimetre budget (`CUT_KERB_DROWNED_Y`,
    /// deleted with it) is gone — so the deepest stone must genuinely go the
    /// 0.15–0.25 m down that M2 could only trace, while `CUT_STEP_M` beneath it
    /// keeps every stone proud of the cartway rather than a hole in the road.
    /// The two companion cues stay: the drowned stretch is heaved out of true
    /// rather than flush, and it is dirtier than the line either side of it.
    /// Neither may touch a stone the soundings do not reach, or the true line
    /// stops being a straightedge to read the dips against.
    #[test]
    fn the_cut_sag_moves_the_kerbstone_down_and_never_sideways() {
        let plan = plan::load();
        let runs = cut_kerb_plan(&plan);
        let mut lowest = f32::MAX;
        let mut highest = f32::MIN;
        let mut deepest_drawn = f32::MAX;
        let mut dirtiest = f32::MAX;
        let mut cleanest_in_a_trough = f32::MAX;

        for run in runs.iter().filter(|run| run.laid) {
            assert_eq!(
                (run.x - CUT_CENTRE_X).abs(),
                CUT_KERB_OFFSET_M,
                "a sagging run wandered to x {}",
                run.x
            );
            assert_eq!(
                run.x,
                run.bank.kerb_x(),
                "a run's bank disagrees with its own x"
            );
            let steps = ((run.z1 - run.z0) / 0.65).ceil().max(1.0) as usize;
            for step in 0..=steps {
                let z = run.z0 + (run.z1 - run.z0) * step as f32 / steps as f32;
                let seed = stable_hash(&format!("cut-kerb-{:.1}-{z:.2}", run.x));
                let (top, height) = cut_kerbstone_top(run.bank, z, seed);
                assert!(
                    CUT_STEP_M + top > CUT_MARGIN_Y + 0.02,
                    "a kerbstone at ({}, {z}) is drawn into the cartway: its \
                     stepped top {} clears the road by under two centimetres",
                    run.x,
                    CUT_STEP_M + top
                );
                assert!(
                    ((top - height) + CUT_KERB_SEAT_M).abs() < 1.0e-5,
                    "a kerbstone at ({}, {z}) bottoms at {} rather than at the \
                     seat every stone on the street shares",
                    run.x,
                    top - height
                );

                let sag = cut_sounding_sag(run.bank, z);
                let shade = cut_kerbstone_shade(sag, seed);
                if sag == 0.0 {
                    assert!(
                        top >= CUT_KERB_RISE_M * 0.91,
                        "a kerbstone at ({}, {z}) is off the true line at y {top} \
                         with no sounding under it",
                        run.x
                    );
                    // Against the undirtied brush itself, not against
                    // `cut_kerbstone_shade(0.0, seed)`: comparing the function
                    // with itself inside the `sag == 0.0` arm is a tautology
                    // that a tint applied unconditionally would still satisfy.
                    assert_eq!(
                        shade,
                        0.81 + (seed % 14) as f32 * 0.01,
                        "a kerbstone at ({}, {z}) is dirtied with no sounding under it",
                        run.x
                    );
                } else {
                    cleanest_in_a_trough = cleanest_in_a_trough.min(shade);
                }
                dirtiest = dirtiest.min(shade);
                lowest = lowest.min(top);
                highest = highest.max(top);
                if sag > 0.0 {
                    deepest_drawn = deepest_drawn.min(top);
                }
            }
        }

        assert!(
            cleanest_in_a_trough < 0.81,
            "no stone in a sounding is dirtied below the cleanest quarried shade"
        );
        assert!(
            dirtiest > 0.6,
            "a kerbstone is drawn at brush {dirtiest}; the soundings are a dirty \
             line, not a stain"
        );

        assert!(
            (highest - CUT_KERB_RISE_M).abs() < 0.02,
            "the true line no longer stands its nominal 0.10 m; it tops out at {highest}"
        );
        assert_eq!(
            lowest, deepest_drawn,
            "the lowest stone on the street is not one the soundings put there"
        );
        // M3: the profile is drawn at full depth. The deepest stone must sink
        // well past anything M2's six-centimetre budget could express — at
        // least a full CUT_SAG_MIN_M below the true line, less the scatter —
        // while the step under it keeps the stone proud of the road, so the
        // trough is a dip in a standing line and never a hole in the cartway.
        assert!(
            deepest_drawn < CUT_KERB_RISE_M - CUT_SAG_MIN_M + CUT_KERB_HEAVE_M,
            "the deepest sounding is drawn at y {deepest_drawn} over the old \
             grade; the authored profile is not reaching the ground"
        );
        assert!(
            CUT_STEP_M + deepest_drawn > CUT_MARGIN_Y + 0.02,
            "the deepest sounding takes its stones into the cartway: stepped \
             top {}",
            CUT_STEP_M + deepest_drawn
        );

        // The four soundings must be four depths, not one floor. Sample each
        // one's deepest point on each bank and require the extremes to differ by
        // more than the per-stone scatter can account for.
        let mut drawn: Vec<(f32, f32)> = Vec::new();
        for sounding in CUT_SOUNDINGS {
            for (bank, depth) in [
                (CutBank::West, sounding.west_m),
                (CutBank::East, sounding.east_m),
            ] {
                let (top, _) = cut_kerbstone_top(bank, sounding.z, heave_bucket_seed(4));
                drawn.push((depth, top));
            }
        }
        drawn.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (shallowest_depth, shallowest_top) = drawn[0];
        let (deepest_depth, deepest_top) = drawn[drawn.len() - 1];
        assert!(
            shallowest_top - deepest_top > 0.010,
            "the {shallowest_depth} m sounding is drawn at y {shallowest_top} and \
             the {deepest_depth} m one at y {deepest_top}; on the ground they are \
             the same hole"
        );

        // And most of the street is dead true — that is the whole reason four
        // dips can be read at all.
        let mut sagging = 0.0_f32;
        let mut sampled = 0.0_f32;
        for (z0, z1) in CUT_LAID_REACHES {
            let steps = ((z1 - z0) / 1.0).ceil() as usize;
            for step in 0..=steps {
                let z = z0 + (z1 - z0) * step as f32 / steps as f32;
                sampled += 1.0;
                if cut_sounding_sag(CutBank::West, z) > 0.005
                    || cut_sounding_sag(CutBank::East, z) > 0.005
                {
                    sagging += 1.0;
                }
            }
        }
        assert!(
            sagging / sampled < 0.45,
            "{:.0}% of the laid line is sagging; the soundings are the exception, \
             not the profile",
            100.0 * sagging / sampled
        );
    }

    /// `the_cut_kerb.md` M2 — the heave is a scatter about the profile, not a
    /// lift off it.
    ///
    /// A one-sided disturbance scaled by the sag is a bias proportional to
    /// depth, which fills in the bottom of a trough more than its shoulders —
    /// the trough's centre would be drawn *above* its own lip, on the one
    /// stretch the milestone wants lowest. So the nine buckets are driven
    /// directly, with the rest of the seed held constant so only the heave
    /// moves: the middle bucket must be the profile itself and the outer two
    /// must sit the same distance either side of it.
    #[test]
    fn the_cut_kerb_heave_is_centred_on_the_profile_it_scatters() {
        let sounding = CUT_SOUNDINGS
            .iter()
            .max_by(|a, b| a.west_m.total_cmp(&b.west_m))
            .expect("the soundings are authored");
        let tops: Vec<f32> = (0..9u32)
            .map(|bucket| cut_kerbstone_top(CutBank::West, sounding.z, heave_bucket_seed(bucket)).0)
            .collect();
        for pair in tops.windows(2) {
            assert!(
                pair[1] > pair[0],
                "the heave buckets do not run monotonically through the profile: {tops:?}"
            );
        }
        let (low, mid, high) = (tops[0], tops[4], tops[8]);
        assert!(
            ((high - mid) - (mid - low)).abs() < 1.0e-6,
            "the heave is one-sided: {low} / {mid} / {high} about the profile"
        );
        assert!(
            high - low > CUT_KERB_HEAVE_M * 0.5,
            "the heave has collapsed to {} at the deepest sounding; the drowned \
             stretch reads as a deliberately flush marking",
            high - low
        );
        assert!(
            CUT_STEP_M + low > CUT_MARGIN_Y + 0.02,
            "a stone heaved downward at the deepest sounding is drawn at \
             stepped top {}, into the cartway",
            CUT_STEP_M + low
        );
        let off = sounding.z + sounding.half_length_m;
        for bucket in 0..9u32 {
            assert_eq!(
                cut_kerbstone_top(CutBank::West, off, heave_bucket_seed(bucket)).0,
                CUT_KERB_RISE_M * 0.92,
                "a stone off the soundings is heaved; the true line is not true"
            );
        }
    }

    /// A kerbstone seed that drives `cut_kerbstone_top`'s heave bucket to
    /// `bucket` while leaving its per-stone rise alone.
    ///
    /// The two fields overlap — the rise reads `(seed >> 8) % 13` and the heave
    /// `(seed >> 16) % 9` — so walking the heave through its nine buckets with a
    /// naive `bucket << 16` also walks the rise, and the two moving together is
    /// exactly what the assertions above must not confuse. `256 % 13 == 9`, so
    /// putting `(4 * bucket) % 13` in the low byte cancels the carry and every
    /// seed here quarries the same 0.92 stone.
    fn heave_bucket_seed(bucket: u32) -> u32 {
        (bucket << 16) | (((4 * bucket) % 13) << 8)
    }

    /// Every triangle the two box builders emit must face the way its own
    /// vertex normal claims: `cross(b - a, c - a)` has to point along the stored
    /// normal, not against it.
    ///
    /// This contract has to be a test precisely because it is invisible on
    /// screen. `add_oriented_box` shipped inverted on all six faces for as long
    /// as it has existed, across 43 call sites, and nobody noticed — every city
    /// material is `double_sided: true, cull_mode: None`, and the vertex normal
    /// was always the correct outward one, so the geometry both drew and shaded
    /// correctly off a mesh that was inside out. Correcting it moved the
    /// rendered image by under 1/255 mean luminance.
    ///
    /// So what is guarded here is not how the city looks — correcting it moved
    /// the frame by under 1/255 mean luminance, and under forced backface culling
    /// by 0.33% of pixels. It is that the mesh should mean what it says, so that
    /// culling, an export or any tool that recomputes normals gets the right
    /// answer. A dot product costs nothing and fails loudly; the alternative is
    /// re-deriving all of this from a screenshot in two years.
    #[test]
    fn every_oriented_box_face_is_wound_outward() {
        fn inverted(mesh: &MeshData) -> Vec<usize> {
            mesh.indices
                .chunks(3)
                .enumerate()
                .filter(|(_, tri)| {
                    let point = |i: u32| Vec3::from_array(mesh.positions[i as usize]);
                    let normal = Vec3::from_array(mesh.normals[tri[0] as usize]);
                    let (a, b, c) = (point(tri[0]), point(tri[1]), point(tri[2]));
                    (b - a).cross(c - a).dot(normal) <= 0.0
                })
                .map(|(index, _)| index)
                .collect()
        }

        // Off-axis, non-cubic and off-origin, so a sign error cannot cancel.
        let along = Vec2::new(3.0, -1.0).normalize();
        let mut oriented = MeshData::default();
        add_oriented_box(
            &mut oriented,
            Vec3::new(-7.5, 2.25, 11.0),
            Vec3::new(0.4, 1.6, 0.9),
            along,
        );
        assert_eq!(oriented.indices.len() / 3, 12, "a box is twelve triangles");
        assert_eq!(
            inverted(&oriented),
            Vec::<usize>::new(),
            "add_oriented_box emits inside-out triangles; those faces take no sun"
        );

        let mut dressed = MeshData::default();
        add_dressed_stone(
            &mut dressed,
            Vec3::new(4.0, 0.05, -2.5),
            Vec3::new(0.15, 0.05, 0.65),
        );
        assert_eq!(
            inverted(&dressed),
            Vec::<usize>::new(),
            "add_dressed_stone emits inside-out triangles"
        );

        // The UVs must stay U-horizontal / V-vertical on the side faces — the
        // reason the ring is reversed rather than the side vectors swapped.
        let side = &oriented.uvs[..4];
        let span_u = side.iter().fold(0.0_f32, |acc, uv| acc.max(uv[0]));
        let span_v = side.iter().fold(0.0_f32, |acc, uv| acc.max(uv[1]));
        assert!(
            (span_u - 0.9 / 3.5).abs() < 1.0e-5 && (span_v - 1.6 / 3.5).abs() < 1.0e-5,
            "a side face's texture is transposed: U spans {span_u}, V spans {span_v}"
        );
    }

    /// `the_cut_kerb.md` M2 — the drawn stones, not the three functions behind
    /// them.
    ///
    /// The rest of M2 asserts the profile, and a renderer wired to the wrong
    /// bank, seated off the wrong face or brushed with the wrong shade would
    /// leave all of it green. So this drives `add_kerbstone_run` itself into a
    /// scratch mesh — one run inside the middle reach's sounding on each bank,
    /// and one on the dead-true stretch north of it — and reads the vertices
    /// that come out. The two banks are asked for at the same `z` on purpose:
    /// the middle sounding takes the east line five centimetres further down
    /// than the west, so a renderer that hard-coded a bank shows up here as two
    /// identical lines.
    #[test]
    fn a_drawn_kerb_run_seats_every_stone_and_carries_the_sounding_under_it() {
        fn stones(run: CutKerbRun) -> Vec<(f32, f32, f32)> {
            let mut mesh = MeshData::default();
            add_kerbstone_run(&mut mesh, run);
            assert_eq!(
                mesh.positions.len() % 24,
                0,
                "a kerbstone is not a six-faced block"
            );
            mesh.positions
                .chunks(24)
                .zip(mesh.colors.chunks(24))
                .map(|(block, colors)| {
                    let top = block.iter().fold(f32::MIN, |acc, v| acc.max(v[1]));
                    let bottom = block.iter().fold(f32::MAX, |acc, v| acc.min(v[1]));
                    for vertex in block {
                        assert!(
                            (vertex[0] - run.x).abs() - CUT_KERB_WIDTH_M * 0.5 < 1.0e-4,
                            "a drawn kerbstone reaches x {} off a line at {}",
                            vertex[0],
                            run.x
                        );
                    }
                    assert!(
                        (bottom + CUT_KERB_SEAT_M).abs() < 1.0e-4,
                        "a drawn kerbstone bottoms at {bottom}, not at the seat"
                    );
                    (top, bottom, colors[0][0])
                })
                .collect()
        }

        let true_line = stones(CutKerbRun {
            x: CutBank::West.kerb_x(),
            bank: CutBank::West,
            z0: -60.0,
            z1: -30.0,
            laid: true,
        });
        let west = stones(CutKerbRun {
            x: CutBank::West.kerb_x(),
            bank: CutBank::West,
            z0: -145.0,
            z1: -123.0,
            laid: true,
        });
        let east = stones(CutKerbRun {
            x: CutBank::East.kerb_x(),
            bank: CutBank::East,
            z0: -145.0,
            z1: -123.0,
            laid: true,
        });

        assert!(
            true_line.len() > 20 && west.len() > 14 && east.len() > 14,
            "a run came out as {} / {} / {} stones",
            true_line.len(),
            west.len(),
            east.len()
        );

        let mean = |stones: &[(f32, f32, f32)], pick: fn(&(f32, f32, f32)) -> f32| {
            stones.iter().map(pick).sum::<f32>() / stones.len() as f32
        };
        let (true_top, west_top, east_top) =
            (mean(&true_line, |s| s.0), mean(&west, |s| s.0), mean(&east, |s| s.0));
        assert!(
            true_top - west_top > 0.02,
            "the drawn line over the middle sounding stands at y {west_top} against \
             y {true_top} on the true reach; the sounding is not being drawn"
        );
        assert!(
            west_top - east_top > 0.005,
            "both banks are drawn at the same depth over a sounding that takes them \
             down 0.17 m and 0.22 m; the renderer is asking for one bank"
        );

        let (true_shade, east_shade) = (mean(&true_line, |s| s.2), mean(&east, |s| s.2));
        assert!(
            true_shade - east_shade > 0.05,
            "the drowned stone is brushed at {east_shade} against {true_shade} on \
             the true line; the tint is not reaching the vertices"
        );
    }

    /// `the_cut_kerb.md` M2 — a sounding never opens under a piece of M1
    /// furniture that is *laid* on the line.
    ///
    /// A water stair's mooring stone is a kerbstone four and a half times the
    /// usual height and a kerb break is three metres of the same stone laid
    /// flush; both are drawn at their own fixed `y`. Take the line down 0.20 m
    /// under either and the mooring stone is left standing alone in a trough and
    /// the kerb break stops reading as a break at all. So the soundings are
    /// authored to put the stairs on their *shoulders* — which is also where a
    /// landing belongs, on the shelving edge of the channel rather than in the
    /// scour. Bollards are exempt: a post is driven, not laid.
    #[test]
    fn the_cut_soundings_leave_the_laid_furniture_standing() {
        let mut on_a_shoulder = 0;
        for (prop, bank, z) in CUT_FURNITURE {
            let sag = match prop {
                CutProp::WaterStair | CutProp::KerbBreak => cut_sounding_sag(bank, z),
                _ => continue,
            };
            assert!(
                sag <= CUT_SAG_AT_LAID_FURNITURE_M,
                "{prop:?} at z {z} on the {bank:?} bank stands where the line has \
                 gone down {sag} m"
            );
            if prop == CutProp::WaterStair
                && CUT_SOUNDINGS.iter().any(|sounding| {
                    let depth = match bank {
                        CutBank::West => sounding.west_m,
                        CutBank::East => sounding.east_m,
                    };
                    depth > 0.0 && (z - sounding.z).abs() <= sounding.half_length_m + 0.5
                })
            {
                on_a_shoulder += 1;
            }
        }
        assert!(
            on_a_shoulder >= 3,
            "only {on_a_shoulder} blocked water stairs stand on a sounding's shoulder; \
             the stairs are the one in-game witness the soundings agree with"
        );
    }

    /// A laid ridge always has margin behind it. The whole claim of M0 is that
    /// the kerb divides the Cut into two kinds of ground, so a kerbstone with
    /// cartway dust on *both* sides is worse than no kerbstone at all — and it
    /// is the easy failure, because the margin and the line answer the "does a
    /// street cross here" question over different widths. `south_inner_wall`
    /// running parallel at `x -224` used to strip thirty-six metres of flags
    /// from behind an unbroken run of the north reach.
    #[test]
    fn every_laid_kerbstone_has_flagged_margin_behind_it() {
        let plan = plan::load();
        let strips = cut_margin_strips(&plan);
        let mut bare = Vec::new();
        for run in cut_kerb_plan(&plan).iter().filter(|run| run.laid) {
            let behind = if run.x < CUT_CENTRE_X {
                run.x - 0.45
            } else {
                run.x + 0.45
            };
            // Sampled a metre inside each end: the margin's lanes follow a
            // diagonal approach where the ridge's single line meets it square,
            // so the two disagree by a fraction of a metre at a junction mouth
            // and nowhere else.
            let (lo, hi) = (run.z0 + 1.0, run.z1 - 1.0);
            if hi < lo {
                continue;
            }
            let steps = ((hi - lo) / 1.0).ceil().max(1.0) as usize;
            for step in 0..=steps {
                let z = lo + (hi - lo) * step as f32 / steps as f32;
                let flagged = strips.iter().any(|[x0, z0, x1, z1]| {
                    *x0 <= behind && behind <= *x1 && *z0 <= z && z <= *z1
                });
                if !flagged {
                    bare.push((behind, z));
                }
            }
        }
        assert!(
            bare.is_empty(),
            "{} sampled metres of laid kerb have no margin behind them, e.g. {:?}",
            bare.len(),
            &bare[..bare.len().min(12)]
        );
    }

    /// The line breaks where a side street opens onto the Cut, so a junction
    /// mouth keeps its own cobbles and a cart can turn out of the cartway
    /// lawfully. `east_cut_to_bell` leaves the Cut eastward at `z -154`, so the
    /// east line is cut there and the west line is not.
    #[test]
    fn the_cut_kerb_opens_at_every_side_street() {
        let plan = plan::load();
        let runs = cut_kerb_plan(&plan);
        let covered = |x: f32, z: f32| {
            runs.iter()
                .any(|run| run.x == x && run.z0 <= z && z <= run.z1)
        };
        assert!(
            !covered(CUT_CENTRE_X + CUT_KERB_OFFSET_M, -154.0),
            "the east line should open for east_cut_to_bell"
        );
        assert!(
            covered(CUT_CENTRE_X - CUT_KERB_OFFSET_M, -154.0),
            "the west line has no junction at z -154 and should be unbroken"
        );
    }

    /// Which of the five authored reaches a z belongs to, or `None` if it is off
    /// the Cut altogether.
    fn cut_reach_of(z: f32) -> Option<(f32, f32)> {
        CUT_LAID_REACHES
            .iter()
            .chain(CUT_MARKED_REACHES.iter())
            .copied()
            .find(|(z0, z1)| *z0 <= z && z <= *z1)
    }

    /// `the_cut_kerb.md` M1 — every hand-placed piece of margin furniture stands
    /// on the Cut, on ground the Cut actually has, and inside no building.
    ///
    /// The whole point of hand-placing rather than spacing is that each z was
    /// read off the plan; the risk that buys is that the plan moves under a
    /// hard-coded number (the 0.7× city shrink of 2026-07 rewrote every
    /// coordinate in the file) and a stair ends up inside a wall or hanging over
    /// a junction mouth with no ground under it.
    #[test]
    fn the_cut_margin_furniture_stands_on_its_own_street() {
        let plan = plan::load();
        let strips = cut_margin_strips(&plan);
        let flagged = |x: f32, z: f32| {
            strips
                .iter()
                .any(|[x0, z0, x1, z1]| *x0 <= x && x <= *x1 && *z0 <= z && z <= *z1)
        };
        assert!(!CUT_FURNITURE.is_empty(), "the furniture is authored");
        for (prop, bank, z) in CUT_FURNITURE {
            let reach = cut_reach_of(z)
                .unwrap_or_else(|| panic!("{prop:?} at z {z} is off the end of the Cut"));
            // Sample the ground the piece actually stands on: a stair's head
            // landing shoulders, a hatch or vent's own footprint, a bollard's
            // post. (A stair's flight centre is asserted separately below —
            // since M3 it stands in a trench the flags deliberately part
            // around, not on flagging.)
            let samples: Vec<(f32, f32)> = match prop {
                CutProp::WaterStair => vec![
                    (bank.kerb_x() + bank.outward() * 3.4, z - 2.1),
                    (bank.kerb_x() + bank.outward() * 3.4, z + 2.1),
                ],
                CutProp::CellarHatch => vec![
                    (bank.facade_x() - bank.outward() * 0.95, z - 0.95),
                    (bank.facade_x() - bank.outward() * 0.95, z + 0.95),
                ],
                CutProp::CellarVent => vec![(bank.facade_x() - bank.outward() * 0.55, z)],
                CutProp::Bollard | CutProp::KerbBreak => Vec::new(),
            };
            for (x, at) in &samples {
                assert!(
                    flagged(*x, *at),
                    "{prop:?} at z {z} puts a corner on ({x}, {at}), which has no margin under it"
                );
            }
            let mut standing = samples.clone();
            standing.push((bank.kerb_x(), z));
            if prop == CutProp::WaterStair {
                // M3: the flight descends through a trench the flags part
                // around — flagging *over* the treads would roof the stair at
                // flag height. Its centre must be open ground, and still on
                // the street.
                let flight_centre = (bank.kerb_x() + bank.outward() * 1.3, z);
                assert!(
                    !flagged(flight_centre.0, flight_centre.1),
                    "{prop:?} at z {z}: the flags roof the flight at ({}, {z}) \
                     instead of parting around its trench",
                    flight_centre.0
                );
                standing.push(flight_centre);
            }
            for building in &plan.buildings {
                for (x, at) in &standing {
                    assert!(
                        !point_in_polygon(Vec2::new(*x, *at), &building.polygon),
                        "{prop:?} at z {z} stands inside {} at ({x}, {at})",
                        building.id
                    );
                }
            }
            // Nothing may hang off the end of the reach it was authored into.
            let overhang = prop.kerb_gap().unwrap_or((0.0, 0.0));
            assert!(
                z + overhang.0 > reach.0 - 0.01 && z + overhang.1 < reach.1 + 0.01,
                "{prop:?} at z {z} straddles the end of the reach {reach:?}"
            );
        }
    }

    /// A kerb break is only lawful because there is a door behind it. Each one
    /// must sit at the midpoint of a Cut-facing façade edge that `door_edges()`
    /// actually nominates — the same midpoint `plan_facade_openings` cuts the
    /// doorway at and `build_hoist_gantries` rigs its beam over.
    #[test]
    fn every_cut_kerb_break_is_at_a_real_cut_facing_door() {
        let plan = plan::load();
        let doors = door_edges();
        let mut breaks = 0;
        for (prop, bank, z) in CUT_FURNITURE {
            if prop != CutProp::KerbBreak {
                continue;
            }
            breaks += 1;
            let matched = plan.buildings.iter().any(|building| {
                let Some(&edge) = doors.get(&building.id) else {
                    return false;
                };
                let a = Vec2::from_array(building.polygon[edge]);
                let b = Vec2::from_array(building.polygon[(edge + 1) % building.polygon.len()]);
                let middle = (a + b) * 0.5;
                (middle.x - bank.facade_x()).abs() < 0.5 && (middle.y - z).abs() < 0.5
            });
            assert!(
                matched,
                "the kerb break at z {z} on the {bank:?} bank faces no Cut-facing door"
            );
        }
        assert!(breaks >= 5, "the warehouse doors still get their breaks");
    }

    /// Nothing in the margin may be laid across the doorway it belongs to. A
    /// cellar hatch is a metre out from the housefront and a vent is against it,
    /// so both are in reach of the door `plan_facade_openings` cuts at the
    /// midpoint of the same edge; a hatch under a door is a hatch nobody can
    /// open and a door nobody can walk out of.
    #[test]
    fn cut_cellar_openings_keep_clear_of_the_doors_they_belong_to() {
        let plan = plan::load();
        let doors = door_edges();
        for (prop, bank, z) in CUT_FURNITURE {
            if !matches!(prop, CutProp::CellarHatch | CutProp::CellarVent) {
                continue;
            }
            for building in &plan.buildings {
                let Some(&edge) = doors.get(&building.id) else {
                    continue;
                };
                let a = Vec2::from_array(building.polygon[edge]);
                let b = Vec2::from_array(building.polygon[(edge + 1) % building.polygon.len()]);
                let middle = (a + b) * 0.5;
                if (middle.x - bank.facade_x()).abs() > 0.5 {
                    continue;
                }
                assert!(
                    (middle.y - z).abs() > 1.6,
                    "{prop:?} at z {z} is laid across {}'s doorway at z {}",
                    building.id,
                    middle.y
                );
            }
        }
    }

    /// The furniture on one bank never asks the kerb for the same metre twice.
    /// Overlapping gaps would not fail loudly — `subtract_gaps` merges them —
    /// but a stair and a break sharing stone means one of the two was placed by
    /// arithmetic rather than by looking.
    #[test]
    fn no_two_pieces_of_cut_furniture_share_the_same_kerb() {
        for bank in [CutBank::West, CutBank::East] {
            let mut gaps = cut_furniture_kerb_gaps(bank);
            gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in gaps.windows(2) {
                assert!(
                    pair[0].1 < pair[1].0,
                    "two pieces of {bank:?} furniture overlap at z {:?} / {:?}",
                    pair[0],
                    pair[1]
                );
            }
        }
    }

    /// The three reaches must read as three different places — that is the whole
    /// reason M1 is hand-placed instead of spaced. North is the trade quarter and
    /// carries the kerb breaks; the middle is the emptiest stretch in the game
    /// and carries the fossil river; the south is poorer and gets neither.
    #[test]
    fn the_three_reaches_of_the_cut_are_furnished_differently() {
        let count = |reach: (f32, f32), kind: CutProp| {
            CUT_FURNITURE
                .iter()
                .filter(|(prop, _, z)| *prop == kind && reach.0 <= *z && *z <= reach.1)
                .count()
        };
        let [north, middle, south] = CUT_LAID_REACHES;
        assert!(
            count(north, CutProp::KerbBreak) >= 3,
            "the trade quarter is where the warehouse doors are"
        );
        assert_eq!(
            count(south, CutProp::KerbBreak),
            0,
            "the south reach has no warehouse door on it and gets no break"
        );
        assert!(
            count(middle, CutProp::WaterStair) > count(north, CutProp::WaterStair)
                && count(middle, CutProp::WaterStair) > count(south, CutProp::WaterStair),
            "the emptiest reach is the one the river has to carry"
        );
        assert!(
            count(south, CutProp::CellarHatch) == 0 && count(south, CutProp::CellarVent) > 0,
            "the poor end gets vents, not hatches"
        );
        assert!(
            count(middle, CutProp::Bollard) + count(north, CutProp::Bollard)
                > count(south, CutProp::Bollard),
            "the south reach is the quiet one"
        );
    }

    /// A water stair descends *through* the line, so the line has to be absent
    /// where it does. A ridge running across the head of a flight would be a
    /// kerb laid over a stair, which is the opposite of the thing being said.
    #[test]
    fn a_cut_water_stair_breaks_the_line_it_descends_through() {
        let plan = plan::load();
        let runs = cut_kerb_plan(&plan);
        let mut stairs = 0;
        for (prop, bank, z) in CUT_FURNITURE {
            if prop != CutProp::WaterStair {
                continue;
            }
            stairs += 1;
            let x = bank.kerb_x();
            for step in 0..=8 {
                let at = z - CUT_STAIR_WIDTH_M * 0.5
                    + CUT_STAIR_WIDTH_M * step as f32 / 8.0;
                assert!(
                    !runs
                        .iter()
                        .any(|run| run.x == x && run.z0 <= at && at <= run.z1),
                    "the {bank:?} line still runs across the water stair at z {at}"
                );
            }
        }
        assert_eq!(stairs, 5, "the Cut carries five blocked water stairs");

        // The gap is symmetric on purpose: which hand the mooring stone stands
        // on comes off each stair's seed, so the widest flight the jitter can
        // make must clear the line on *both* sides. At the shipped numbers the
        // stone's far edge lands 18 mm inside the gap; narrowing either end
        // would clip whichever stairs drew the other seed bit.
        let widest = (0..15).map(cut_stair_half_head).fold(0.0_f32, f32::max);
        let reach = widest + CUT_MOORING_STANDOFF_M + CUT_MOORING_STONE_HALF_Z;
        let (lo, hi) = CutProp::WaterStair
            .kerb_gap()
            .expect("a water stair takes the line out");
        assert!(
            -lo >= reach && hi >= reach,
            "a mooring stone reaching {reach} m from the stair head does not fit \
             the gap ({lo}, {hi})"
        );
    }

    /// Regenerate `assets/world/collision_footprints.json` — the exact XZ
    /// footprints of everything that stops the player at walking height (walls,
    /// towers, gatehouses, buildings, fixtures, bridge piers, the ropewalk). The
    /// navigation bake subtracts these, so the walkable surface is the true
    /// complement of the collision world. Overhead structures (the bridges, the
    /// malt-house) are absent because their collider starts above head height.
    ///
    /// Run when scene collision changes:
    ///   cargo test export_collision_footprints -- --ignored --nocapture
    /// then re-run `scripts/bake_navigation.py`.
    #[test]
    #[ignore = "writes an asset; run manually when scene collision changes"]
    fn export_collision_footprints() {
        let collision = built_collision_world();
        let footprints: Vec<Vec<[f32; 2]>> = collision
            .solid_footprints_in_band(WALK_BAND_LO, WALK_BAND_HI)
            .into_iter()
            .map(|poly| poly.into_iter().map(|p| [p.x, p.y]).collect())
            .collect();
        let doc = serde_json::json!({
            "walk_band": [WALK_BAND_LO, WALK_BAND_HI],
            "note": "XZ footprints of colliders overlapping the standing player's \
                     walk band; generated by `cargo test export_collision_footprints -- --ignored`",
            "footprints": footprints,
        });
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/world/collision_footprints.json"
        );
        std::fs::write(path, serde_json::to_string(&doc).unwrap()).expect("write footprints");
        println!("wrote {} footprints to {path}", footprints.len());
    }

    /// Re-running the deterministic bake reproduces the committed artifact byte
    /// for byte, so `navigation.json` / `navigation.bin` cannot silently drift
    /// from the plan and collision export they are baked from (02_navigation.md
    /// §8). Ignored because it shells out to `uv` + Python.
    #[test]
    #[ignore = "requires uv + python; re-runs the bake and checks it is byte-stable"]
    fn bake_is_reproducible() {
        let root = env!("CARGO_MANIFEST_DIR");
        let bake = std::process::Command::new("uv")
            .args(["run", "scripts/bake_navigation.py"])
            .current_dir(root)
            .output()
            .expect("run the navigation bake");
        assert!(
            bake.status.success(),
            "bake failed: {}",
            String::from_utf8_lossy(&bake.stderr)
        );
        let clean = std::process::Command::new("git")
            .args([
                "diff",
                "--quiet",
                "--",
                "assets/world/navigation.json",
                "assets/world/navigation.bin",
            ])
            .current_dir(root)
            .status()
            .expect("run git diff");
        assert!(
            clean.success(),
            "re-baking changed the committed navigation artifact — it is not reproducible"
        );
    }

    /// The door the sim walks to and the door the player sees are the same door:
    /// every baked door sits on a render-eligible polygon edge, and its walkable
    /// node is one pace outward from that edge's midpoint — exactly where
    /// `add_facade_openings` now draws the panel (02_navigation.md §1, §8).
    #[test]
    fn the_door_you_see_is_the_door_you_walk_to() {
        let plan = plan::load();
        let nav = cathedral_sim::NavData::from_parts(NAV_JSON, NAV_BIN)
            .expect("the committed navigation artifact loads");
        let by_id: HashMap<&str, &Building> =
            plan.buildings.iter().map(|b| (b.id.as_str(), b)).collect();

        for door in nav.doors() {
            let building = by_id[door.building.as_str()];

            // add_facade_openings early-returns (renders no door) for bridges and
            // the malt-house, so the bake must not emit one either — a baked door
            // here would be a phantom the player can walk to but never sees, on
            // open ground under overhead scenery.
            assert!(
                building.use_name != "bridge" && building.id != "named_malt_house",
                "{} is a phantom door: the renderer draws none for bridges or the \
                 malt-house, so the bake must skip it",
                door.building
            );

            let poly = &building.polygon;
            let n = poly.len();
            assert!(
                door.edge < n,
                "door edge index in range for {}",
                door.building
            );

            let a = Vec2::from_array(poly[door.edge]);
            let b = Vec2::from_array(poly[(door.edge + 1) % n]);
            let edge = b - a;
            let length = edge.length();
            assert!(
                length >= 3.2,
                "{} door is on a {length:.2} m edge the renderer would skip",
                door.building
            );

            let mut normal = Vec2::new(edge.y, -edge.x).normalize();
            if plan::signed_area(poly).signum() < 0.0 {
                normal = -normal;
            }
            let stand = a + edge * 0.5 + normal * 0.8;
            let node = nav.node_xz(door.node);
            let offset = (Vec2::new(node[0] as f32, node[1] as f32) - stand).length();
            assert!(
                offset < 0.5,
                "{} door node {node:?} is {offset:.2} m from its edge's threshold",
                door.building
            );
        }
    }

    #[test]
    fn city_builds_every_cadastral_building_and_named_place() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);

        app.update();

        let world = app.world_mut();
        let stats = *world.resource::<CityBuildStats>();
        // 0.7x city-shrink revision (see plan.rs): fewer ordinary houses and
        // market stalls; named/road/place/site inventories preserved.
        assert_eq!(stats.planned_buildings, 1_108);
        // The Lanthorn is rendered by CathedralPlugin; every other footprint is
        // rendered by this plugin from the authoritative plan.
        assert_eq!(stats.rendered_plan_buildings, 1_107);
        assert_eq!(stats.named_places, 70);
        assert_eq!(stats.roads, 49);
        assert_eq!(stats.sites, 23);
        assert_eq!(stats.fixtures, 72);
        // wharf_sheds is built by the 3D wharf loop in this file (x=-397.6,
        // 11 sheds at the kept 38 m pitch), matching the regenerated SVG.
        assert_eq!(stats.wharf_sheds, 11);

        let place_markers = world
            .query::<&LorePlaceNumber>()
            .iter(world)
            .map(|number| number.0)
            .collect::<Vec<_>>();
        assert_eq!(place_markers.len(), 70);
        assert!(place_markers.contains(&1));
        // 70 is the Stone House (`law_and_order.md` M5a).
        assert!(place_markers.contains(&70));

        let route_boards = world
            .query::<&route_boards::RoadSupplyRouteBoard>()
            .iter(world)
            .map(|board| board.location)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            route_boards,
            BTreeSet::from([
                "Seven Lofts",
                "The Draper's Reach",
                "The Stone Gate",
                "The Wool Gate",
            ])
        );
        assert_eq!(
            world
                .query::<&route_boards::RoadSupplyRouteMapFace>()
                .iter(world)
                .count(),
            4
        );
        let collider_count = world.resource::<CollisionWorld>().len();
        // ~1,032 buildings post-shrink (was 2,566 / > 3_000 colliders).
        assert!(collider_count > 1_400, "{collider_count} colliders");
    }

    /// Every water fixture in the plan is built, marked for its loop, and asks
    /// for a loop the catalog can actually synthesize — a typo here would be a
    /// silent well rather than a failed build.
    #[test]
    fn every_water_source_is_built_and_sounds_like_itself() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();

        let catalog = cathedral_sim::SoundCatalog::from_toml_str(include_str!(
            "../../assets/sounds/catalog.toml"
        ))
        .expect("the shipped catalog loads");
        let ambient_ids = catalog
            .ambients()
            .iter()
            .map(|ambient| ambient.sound_id.as_str())
            .collect::<BTreeSet<_>>();

        let world = app.world_mut();
        let sources = world
            .query::<(&water::WaterAmbience, &Transform)>()
            .iter(world)
            .map(|(ambience, transform)| {
                assert!(
                    ambient_ids.contains(ambience.sound_id),
                    "'{}' is not an [[ambients]] row in the sound catalog",
                    ambience.sound_id
                );
                assert!(ambience.audible_distance > 0.0);
                [transform.translation.x, transform.translation.z]
            })
            .collect::<Vec<_>>();

        // The nine named ward sources (Ford plus the eight of the ward network),
        // the Shambles well, and the Seven Lofts fire tanks.
        assert_eq!(sources.len(), 11);
        let plan = plan::load();
        for fixture in plan.fixtures.iter().filter(|fixture| {
            matches!(
                fixture.kind.as_str(),
                "well"
                    | "chain_well"
                    | "three_curb_well"
                    | "lodge_well"
                    | "cistern"
                    | "step_cistern"
                    | "fire_tanks"
            )
        }) {
            assert!(
                sources.contains(&fixture.position),
                "{} has no water ambience",
                fixture.id
            );
        }
    }

    #[test]
    fn batched_city_keeps_render_entity_count_bounded() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();

        let world = app.world_mut();
        let count = world
            .query_filtered::<Entity, With<Mesh3d>>()
            .iter(world)
            .count();
        assert!(count > 150, "expected authored details, got {count}");
        // Each material batch is split into ~128 m ground tiles so culling
        // works (2026-07 perf work): ~2,600 tiles today. The ceiling guards
        // against regressing to per-feature entities (tens of thousands), not
        // against tiles.
        assert!(
            count < 4_000,
            "cadastral geometry should stay batched into tiles, got {count}"
        );
    }

    #[test]
    fn wall_openings_interrupt_the_curtain_at_every_gate() {
        let plan = plan::load();
        for (gate, width) in WALL_OPENINGS {
            let mut matched_wall = false;
            for (a, b) in plan
                .wall_polygon_xz
                .iter()
                .zip(plan.wall_polygon_xz.iter().cycle().skip(1))
            {
                let start = Vec2::from_array(*a);
                let end = Vec2::from_array(*b);
                let edge = end - start;
                let t = (gate - start).dot(edge) / edge.length_squared();
                if !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let projected_gate = start + edge * t;
                if projected_gate.distance(gate) > 32.0 {
                    continue;
                }

                matched_wall = true;
                let ranges = wall_ranges_around_gates(start, end, &[(gate, width)]);
                assert!(
                    ranges.iter().all(|(range_start, range_end)| {
                        let range = *range_end - *range_start;
                        let range_t =
                            (projected_gate - *range_start).dot(range) / range.length_squared();
                        !(0.0..=1.0).contains(&range_t)
                            || (*range_start + range * range_t).distance(projected_gate) > 0.01
                    }),
                    "gate at {gate:?} is still covered by a wall segment"
                );
            }
            assert!(matched_wall, "gate at {gate:?} does not meet the curtain");
        }
    }

    /// Cutting the curtain for a gate is only half the job: the tower ring is
    /// laid on the same wall line by a rule that knows nothing about openings,
    /// and a 12 m tower set corner-on in an arch walls it straight back up.
    /// Before this was caught, four of the five gates had one — the Harne arch
    /// was down to a 1 m slot and the Stone Gate to 6 m of a 24 m passage.
    /// Read off the built entities, because it is the tower's collider, not the
    /// tower rule, that stops the player.
    #[test]
    fn no_wall_tower_is_planted_in_a_gate_opening() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();

        let world = app.world_mut();
        let towers = world
            .query::<(&Name, &Transform)>()
            .iter(world)
            .filter(|(name, _)| {
                name.as_str().starts_with("Wall tower") && !name.as_str().ends_with("roof")
            })
            .map(|(name, transform)| {
                (
                    name.to_string(),
                    Vec2::new(transform.translation.x, transform.translation.z),
                )
            })
            .collect::<Vec<_>>();

        let plan = plan::load();
        let arches = gate_arches(&plan.wall_polygon_xz, &WALL_OPENINGS);
        assert_eq!(arches.len(), WALL_OPENINGS.len());
        let tower_reach = 12.0 * SQRT_2 * 0.5;
        for (name, point) in &towers {
            for (arch_start, arch_end) in &arches {
                let clearance = segment_distance_squared(*point, *arch_start, *arch_end).sqrt();
                assert!(
                    clearance >= tower_reach,
                    "{name} at {point:?} reaches {:.2} m into the arch \
                     {arch_start:?}–{arch_end:?}",
                    tower_reach - clearance
                );
            }
        }
        // 28 vertex-and-division points, less the four the gates claim — so a
        // future change cannot quietly buy the clearance above by dropping the
        // whole ring.
        assert_eq!(towers.len(), 24, "{towers:?}");
    }

    /// A tower is a 12 m square set *corner-on*, so the ground off the middle of
    /// each of its four faces is open paving. The collider used to be the
    /// axis-aligned box that circumscribes that diamond — 288 m² of solid for
    /// 144 m² of stone, the surplus sitting in four triangles exactly where the
    /// player walks past a face and stops against nothing. The nav bake
    /// subtracts these footprints, so it ate the phantom corners out of the
    /// walkable set too. Take the tower's own solid out of the exported set
    /// before probing it: the curtain it stands on is a separate box, and that
    /// one is deliberately loose.
    #[test]
    fn a_wall_tower_is_solid_only_where_its_masonry_is() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();
        let collision = std::mem::take(app.world_mut().resource_mut::<CollisionWorld>().as_mut());

        let world = app.world_mut();
        let towers = world
            .query::<(&Name, &Transform)>()
            .iter(world)
            .filter(|(name, _)| {
                name.as_str().starts_with("Wall tower") && !name.as_str().ends_with("roof")
            })
            .map(|(name, transform)| {
                (
                    name.to_string(),
                    Vec2::new(transform.translation.x, transform.translation.z),
                )
            })
            .collect::<Vec<_>>();
        assert!(!towers.is_empty(), "the curtain has no towers to check");

        let footprints: Vec<Vec<[f32; 2]>> = collision
            .solid_footprints_in_band(WALK_BAND_LO, WALK_BAND_HI)
            .into_iter()
            .map(|polygon| polygon.into_iter().map(|v| [v.x, v.y]).collect())
            .collect();
        for (name, point) in &towers {
            // A tower can share its centre with the 8 m curtain chunk it stands
            // on, so of the solids centred there take the largest: at 144 m² the
            // tower dwarfs any chunk of wall (96 m² at its fattest, where the
            // curtain runs diagonally), and the box it wrongly used to register
            // was centred on the tower too — so this picks the tower out either
            // way and the assertions below get to name the real fault.
            let footprint = footprints
                .iter()
                .filter(|polygon| {
                    let centroid = polygon
                        .iter()
                        .fold(Vec2::ZERO, |sum, v| sum + Vec2::from_array(*v))
                        / polygon.len() as f32;
                    centroid.distance(*point) < 0.05
                })
                .max_by(|a, b| {
                    plan::signed_area(a)
                        .abs()
                        .total_cmp(&plan::signed_area(b).abs())
                })
                .unwrap_or_else(|| panic!("{name} at {point:?} registered no collider"));

            let area = plan::signed_area(footprint).abs();
            assert!(
                (area - 144.0).abs() < 1.0,
                "{name} is solid over {area:.0} m² for 12 × 12 m of stone"
            );

            // 6 m out along a diagonal is 2.5 m clear of the face it crosses,
            // and 2.5 m inside the bounding box that used to be registered.
            for (dx, dz) in [(6.0, 6.0), (6.0, -6.0), (-6.0, 6.0), (-6.0, -6.0)] {
                let probe = Vec2::new(point.x + dx, point.y + dz);
                assert!(
                    !point_in_polygon(probe, footprint),
                    "{name} is solid at {probe:?}, which is open ground off its face"
                );
            }
        }
    }

    /// A bridge's spine pier holds the shell up from inside its own footprint;
    /// everything outside that footprint is road. Before this was caught the
    /// pier was sized from the mouth's *width* and centred on the mouth
    /// midpoint, so the malt-house piers ran 26 m along Malt Passage with 13 m
    /// of that standing out in Fabric Way, solid from the ground to 4.2 m. Read
    /// the pier corners off the built entities, because it is the collider, not
    /// the sizing rule, that severs the street.
    #[test]
    fn no_bridge_pier_stands_outside_its_shell() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();

        let world = app.world_mut();
        let piers = world
            .query::<(&Name, &Transform)>()
            .iter(world)
            .filter(|(name, _)| name.as_str().ends_with(" support"))
            .map(|(name, transform)| (name.to_string(), *transform))
            .collect::<Vec<_>>();

        let plan = plan::load();
        let boundary_distance = |point: Vec2, polygon: &[[f32; 2]]| {
            polygon
                .iter()
                .zip(polygon.iter().cycle().skip(1))
                .map(|(a, b)| {
                    segment_distance_squared(point, Vec2::from_array(*a), Vec2::from_array(*b))
                        .sqrt()
                })
                .fold(f32::INFINITY, f32::min)
        };
        for (name, transform) in &piers {
            let owner = name.trim_end_matches(" support");
            let building = plan
                .buildings
                .iter()
                .find(|building| building.name.as_deref().unwrap_or(&building.id) == owner)
                .unwrap_or_else(|| panic!("{name} belongs to no cadastral building"));

            let center = Vec2::new(transform.translation.x, transform.translation.z);
            let across = transform.rotation * (Vec3::X * transform.scale.x * 0.5);
            let along = transform.rotation * (Vec3::Z * transform.scale.z * 0.5);
            for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let offset = across * u + along * v;
                let corner = center + Vec2::new(offset.x, offset.z);
                let outside = boundary_distance(corner, &building.polygon);
                assert!(
                    point_in_polygon(corner, &building.polygon) || outside <= 0.02,
                    "{name} corner {corner:?} stands {outside:.2} m outside {}",
                    building.id
                );
            }
            // And it stands *in* the mouth, its outer face flush with it: that
            // is where the half-mouth arches spring off its flanks.
            let setback = boundary_distance(center, &building.polygon);
            assert!(
                (setback - BRIDGE_PIER_DEPTH * 0.5).abs() < 0.02,
                "{name} sits {setback:.2} m from the mouth, not {:.2} m",
                BRIDGE_PIER_DEPTH * 0.5
            );
        }
        // Both mouths of the three bridges and the malt house, so a future
        // change cannot buy the clearance above by dropping the piers.
        assert_eq!(piers.len(), 8, "{piers:?}");
    }

    #[test]
    fn concave_gaunt_footprint_triangulates_without_filling_its_passage() {
        let plan = plan::load();
        let gaunt = plan
            .buildings
            .iter()
            .find(|building| building.id == "named_gaunt_house")
            .unwrap();
        assert_eq!(gaunt.polygon.len(), 8);
        assert_eq!(triangulate_polygon(&gaunt.polygon).len(), 6);

        let passage = Vec2::new(-161.8, 10.1);
        assert!(!point_in_polygon_for_test(passage, &gaunt.polygon));

        let mut collision_world = CollisionWorld::default();
        add_footprint_colliders(&mut collision_world, &gaunt.polygon, 0.0, 8.0);
        assert!(
            collision_world
                .nearest_ray_hit(Vec3::new(-165.8, 1.0, 10.1), Vec3::X, 7.0)
                .is_none(),
            "the triangulated collider must not seal Gaunt House's passage"
        );
    }

    #[test]
    fn every_building_collider_starts_at_its_visible_facades() {
        let plan = plan::load();
        for building in &plan.buildings {
            if building.id == "named_lanthorn" {
                continue;
            }

            let (base_y, eave_y) = building_verticals(building);
            let mut collision_world = CollisionWorld::default();
            add_footprint_colliders(&mut collision_world, &building.polygon, base_y, eave_y);
            assert!(
                !collision_world.is_empty(),
                "{} has no collider",
                building.id
            );

            let winding = plan::signed_area(&building.polygon).signum();
            for (a, b) in building
                .polygon
                .iter()
                .zip(building.polygon.iter().cycle().skip(1))
            {
                let a = Vec2::from_array(*a);
                let b = Vec2::from_array(*b);
                let midpoint = (a + b) * 0.5;
                let edge = b - a;
                let outward = Vec2::new(edge.y, -edge.x).normalize() * winding;
                let ray_start = midpoint + outward * 0.75;
                let distance = collision_world
                    .nearest_ray_hit(
                        Vec3::new(ray_start.x, base_y + 0.5, ray_start.y),
                        Vec3::new(-outward.x, 0.0, -outward.y),
                        1.5,
                    )
                    .unwrap_or_else(|| panic!("{} has an unprotected facade", building.id));
                assert!(
                    (distance - 0.75).abs() < 0.002,
                    "{} collider is {distance:.3} m from its facade",
                    building.id
                );
            }
        }
    }

    /// A firewood rick is three courses high by two deep, so its six logs stand
    /// at six distinct places. The logs are laid *along* the wall, which is the
    /// one axis the second column must not step on: a 0.3 m offset there slides
    /// a 1.05 m log along its own length and leaves it 71% swallowed by its
    /// twin. Check the whole arrangement — three heights, two depths, and every
    /// pair of logs clear of each other in the plane they share.
    #[test]
    fn firewood_rick_stacks_across_the_wall_and_not_along_the_logs() {
        // A doorway on a wall running along x, so the logs lie along x and the
        // rick steps out into the street along +z.
        let base = Vec2::new(4.0, -11.0);
        let normal = Vec2::Y;
        const LOG_DIAMETER: f32 = 0.23;

        let logs: Vec<Vec3> = (0..3)
            .flat_map(|row| (0..2).map(move |column| firewood_log_center(base, normal, row, column)))
            .collect();

        let heights: BTreeSet<String> = logs.iter().map(|log| format!("{:.3}", log.y)).collect();
        assert_eq!(heights.len(), 3, "expected three courses, got {heights:?}");
        let depths: BTreeSet<String> = logs.iter().map(|log| format!("{:.3}", log.z)).collect();
        assert_eq!(depths.len(), 2, "expected two depths, got {depths:?}");

        for log in &logs {
            assert!(
                (log.x - base.x).abs() < 1.0e-4,
                "a log at {log:?} was offset along its own axis, away from x={}",
                base.x
            );
        }

        // Two logs sharing a course must be a diameter apart across the wall,
        // and two sharing a depth a diameter apart in height; nothing may end
        // up in the same place twice.
        for (i, a) in logs.iter().enumerate() {
            for b in &logs[i + 1..] {
                let gap = ((a.y - b.y).abs()).max((a.z - b.z).abs());
                assert!(
                    gap >= LOG_DIAMETER,
                    "logs at {a:?} and {b:?} are only {gap:.3} m apart and overlap"
                );
            }
        }
    }

    fn point_in_polygon_for_test(point: Vec2, polygon: &[[f32; 2]]) -> bool {
        let mut inside = false;
        for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
            let a = Vec2::from_array(*a);
            let b = Vec2::from_array(*b);
            if (a.y > point.y) != (b.y > point.y)
                && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
            {
                inside = !inside;
            }
        }
        inside
    }
}
