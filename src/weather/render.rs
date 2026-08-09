//! Bounded weather presentation: one rain mesh, one impact mesh, one static
//! puddle batch, two cloud sheets, and two broad fog volumes.

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    light::{
        AtmosphereEnvironmentMapLight, FogVolume, NotShadowCaster, VolumetricFog, VolumetricLight,
        light_consts::lux,
    },
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::{
    controller::PlayerCamera,
    mesh_batch::{batch_mesh, idle_batch_mesh, write_batch_mesh},
    scene::Sun,
    smart_actors::WorldClockState,
};

use super::{
    PrecipitationOcclusionMap, SmoothedWeather, WeatherLightning, WeatherQuality,
    WeatherRoseWindow, WeatherSettings,
};

const NIGHT_FLOOR: f32 = 0.05;
const NIGHT_AMBIENT: f32 = 14.0;
const DAY_AMBIENT: f32 = 110.0;
const SUN_LATITUDE_DEG: f32 = 48.0;
const SUN_DECLINATION_DEG: f32 = 23.44;
const LIGHTNING_FLASH_SECONDS: f64 = 0.24;
const CLOUD_TILE_M: f32 = 190.0;
/// The volumetric fog/light passes are forced on (at near-zero density) for
/// this long after startup so their pipelines compile inside the load window
/// instead of at the first mid-play fog onset.
const VOLUMETRIC_WARMUP_SECONDS: f32 = 5.0;

#[derive(Component)]
pub(super) struct RainBatch;

#[derive(Component)]
pub(super) struct RainImpactBatch;

#[derive(Component)]
pub(super) struct PuddleBatch;

#[derive(Component)]
pub(super) struct CloudLayer {
    depth: f32,
}

#[derive(Component)]
pub(super) struct WeatherFogLayer {
    strength: f32,
}

#[derive(Component)]
pub(super) struct LightningFlash {
    started_at: f64,
    strength: f32,
    secondary: bool,
}

#[derive(Resource)]
pub(super) struct WeatherMeshes {
    rain: Handle<Mesh>,
    impacts: Handle<Mesh>,
    rain_material: Handle<StandardMaterial>,
    impact_material: Handle<StandardMaterial>,
    puddle_material: Handle<StandardMaterial>,
    cloud_materials: [Handle<StandardMaterial>; 2],
    quality: WeatherQuality,
    capacity: usize,
    splash_stride: usize,
    runoff_anchors: Vec<RunoffAnchor>,
    puddle_centers: Vec<Vec3>,
}

#[derive(Debug, Clone, Copy)]
struct RunoffAnchor {
    top: Vec3,
    ground_y: f32,
}

pub(super) fn setup_weather_rendering(
    mut commands: Commands,
    settings: Res<WeatherSettings>,
    cover: Res<PrecipitationOcclusionMap>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    images: Option<ResMut<Assets<Image>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let quality = WeatherQuality::from_name(&settings.quality);
    let (capacity, splash_stride) = match quality {
        WeatherQuality::Low => (800, 18),
        WeatherQuality::Medium => (1_450, 10),
        WeatherQuality::High => (2_200, 7),
    };

    let rain = meshes.add(idle_batch_mesh());
    let impacts = meshes.add(idle_batch_mesh());
    let rain_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.70, 0.82, 0.92, 0.78),
        emissive: LinearRgba::rgb(0.025, 0.035, 0.045),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let impact_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.67, 0.78, 0.86, 0.45),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let puddle_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.10, 0.17, 0.20, 0.0),
        alpha_mode: AlphaMode::Blend,
        metallic: 0.08,
        perceptual_roughness: 0.16,
        reflectance: 0.72,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let (cloud_textures, fog_texture): ([Option<Handle<Image>>; 2], Option<Handle<Image>>) =
        if let Some(mut images) = images {
            (
                [
                    Some(images.add(cloud_noise_image(0x41c6))),
                    Some(images.add(cloud_noise_image(0xa92d))),
                ],
                Some(images.add(fog_noise_image(0x7f03))),
            )
        } else {
            ([None, None], None)
        };
    let cloud_materials = [0, 1].map(|layer| {
        materials.add(StandardMaterial {
            base_color: if layer == 0 {
                Color::srgba(0.64, 0.68, 0.72, 0.0)
            } else {
                Color::srgba(0.48, 0.52, 0.57, 0.0)
            },
            base_color_texture: cloud_textures[layer].clone(),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    });

    commands.spawn((
        Name::new("Camera-local weather rain streak batch"),
        RainBatch,
        Mesh3d(rain.clone()),
        MeshMaterial3d(rain_material.clone()),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
    ));
    commands.spawn((
        Name::new("Weather rain impact and ripple batch"),
        RainImpactBatch,
        Mesh3d(impacts.clone()),
        MeshMaterial3d(impact_material.clone()),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
    ));

    let (puddle_mesh, puddle_centers) = if quality == WeatherQuality::Low {
        (idle_batch_mesh(), Vec::new())
    } else {
        puddle_mesh(&cover)
    };
    let puddles = meshes.add(puddle_mesh);
    commands.spawn((
        Name::new("Static weather puddle batch"),
        PuddleBatch,
        Mesh3d(puddles),
        MeshMaterial3d(puddle_material.clone()),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));

    for (index, height) in [175.0, 215.0].into_iter().enumerate() {
        commands.spawn((
            Name::new(format!("Weather cloud sheet {}", index + 1)),
            CloudLayer {
                depth: index as f32,
            },
            Mesh3d(meshes.add(cloud_plane_mesh(1_900.0, 10.0))),
            MeshMaterial3d(cloud_materials[index].clone()),
            Transform::from_xyz(0.0, height, 0.0),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }

    // A low city-wide bank and a denser Cut/Reed/canal bank. Density starts at
    // zero; distance fog remains the fallback at every quality.
    commands.spawn((
        Name::new("Weather city fog sea"),
        WeatherFogLayer { strength: 0.55 },
        FogVolume {
            fog_color: Color::srgb(0.62, 0.69, 0.72),
            density_factor: 0.0,
            density_texture: fog_texture.clone(),
            absorption: 0.42,
            scattering: 0.58,
            scattering_asymmetry: 0.62,
            ..default()
        },
        Transform::from_xyz(-21.0, 10.0, -49.0).with_scale(Vec3::new(805.0, 21.0, 826.0)),
        Visibility::Hidden,
    ));
    commands.spawn((
        Name::new("Weather Cut and Reed fog bank"),
        WeatherFogLayer { strength: 1.0 },
        FogVolume {
            fog_color: Color::srgb(0.56, 0.64, 0.67),
            density_factor: 0.0,
            density_texture: fog_texture,
            absorption: 0.48,
            scattering: 0.62,
            scattering_asymmetry: 0.54,
            ..default()
        },
        Transform::from_xyz(-213.5, 7.0, -101.5).with_scale(Vec3::new(273.0, 15.0, 686.0)),
        Visibility::Hidden,
    ));

    commands.insert_resource(WeatherMeshes {
        rain,
        impacts,
        rain_material,
        impact_material,
        puddle_material,
        cloud_materials,
        quality,
        capacity,
        splash_stride,
        runoff_anchors: if quality == WeatherQuality::High {
            build_runoff_anchors(&cover)
        } else {
            Vec::new()
        },
        puddle_centers,
    });
}

/// Sparse deterministic eave points, derived once from covered/open grid
/// boundaries. Runtime filters this bounded list around the camera and writes
/// the selected streams into the existing impact batch—never one emitter per
/// building.
fn build_runoff_anchors(cover: &PrecipitationOcclusionMap) -> Vec<RunoffAnchor> {
    let (min, max) = cover.bounds();
    let mut anchors = Vec::new();
    let step = 4.0;
    let mut z = min.y + step;
    while z < max.y - step {
        let mut x = min.x + step;
        while x < max.x - step {
            let roof = cover.sample(x, z);
            if roof.material != super::CoverMaterial::Open && roof.impact_y > roof.ground_y + 2.0 {
                for offset in [
                    Vec2::new(step, 0.0),
                    Vec2::new(-step, 0.0),
                    Vec2::new(0.0, step),
                    Vec2::new(0.0, -step),
                ] {
                    let open = cover.sample(x + offset.x, z + offset.y);
                    let hash = mix64(
                        u64::from(x.to_bits()).rotate_left(17)
                            ^ u64::from(z.to_bits())
                            ^ u64::from(offset.x.to_bits()).rotate_left(41)
                            ^ u64::from(offset.y.to_bits()).rotate_left(7),
                    );
                    if open.material == super::CoverMaterial::Open && hash.is_multiple_of(29) {
                        anchors.push(RunoffAnchor {
                            top: Vec3::new(
                                x + offset.x * 0.48,
                                roof.impact_y - 0.08,
                                z + offset.y * 0.48,
                            ),
                            ground_y: open.ground_y,
                        });
                        break;
                    }
                }
            }
            x += step;
        }
        z += step;
    }
    anchors
}

/// A small tileable fractal field breaks the cloud shell into broad lobes. It
/// is generated once, shared by the whole sheet and sampled linearly; the two
/// layers use different seeds and travel at different speeds down the same
/// authoritative wind vector.
fn cloud_noise_image(seed: u64) -> Image {
    const SIZE: u32 = 192;
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let nx = x as f32 / SIZE as f32;
            let ny = y as f32 / SIZE as f32;
            let value = tile_value_noise(nx, ny, 5, seed) * 0.52
                + tile_value_noise(nx, ny, 11, seed.rotate_left(17)) * 0.30
                + tile_value_noise(nx, ny, 23, seed.rotate_left(39)) * 0.18;
            let alpha = (((value - 0.34) / 0.52).clamp(0.0, 1.0).powf(0.72) * 255.0).round() as u8;
            pixels.extend_from_slice(&[245, 249, 255, alpha]);
        }
    }
    let mut image = Image::new_fill(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// Small repeating 3D density field for the broad fog banks. Keeping the
/// texture coarse is intentional: trilinear sampling and two octaves make it
/// read as slow variation rather than voxel detail, at only 32 KiB.
fn fog_noise_image(seed: u64) -> Image {
    const X: u32 = 32;
    const Y: u32 = 16;
    const Z: u32 = 32;
    let mut pixels = Vec::with_capacity((X * Y * Z) as usize);
    for z in 0..Z {
        for y in 0..Y {
            for x in 0..X {
                let broad = unit(mix64(
                    seed ^ u64::from(x % 8).wrapping_mul(0x9e37_79b9)
                        ^ u64::from(y % 4).wrapping_mul(0x85eb_ca6b)
                        ^ u64::from(z % 8).wrapping_mul(0xc2b2_ae35),
                ));
                let fine = unit(mix64(
                    seed.rotate_left(29)
                        ^ u64::from(x).wrapping_mul(0x27d4_eb2d)
                        ^ u64::from(y).wrapping_mul(0x1656_67b1)
                        ^ u64::from(z).wrapping_mul(0xd3a2_646c),
                ));
                pixels.push(((0.32 + broad * 0.48 + fine * 0.20) * 255.0) as u8);
            }
        }
    }
    let mut image = Image::new_fill(
        Extent3d {
            width: X,
            height: Y,
            depth_or_array_layers: Z,
        },
        TextureDimension::D3,
        &pixels,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// A repeated horizontal sheet lets translation wrap by one 190 m noise tile
/// without a visual seam while the 1.9 km mesh remains centred on the camera.
fn cloud_plane_mesh(size: f32, tiles: f32) -> Mesh {
    let half = size * 0.5;
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-half, 0.0, -half],
            [half, 0.0, -half],
            [half, 0.0, half],
            [-half, 0.0, half],
        ],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, -1.0, 0.0]; 4]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [tiles, 0.0], [tiles, tiles], [0.0, tiles]],
    );
    mesh.insert_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]));
    mesh
}

fn tile_value_noise(x: f32, y: f32, cells: u32, seed: u64) -> f32 {
    let fx = x * cells as f32;
    let fy = y * cells as f32;
    let x0 = fx.floor() as u32;
    let y0 = fy.floor() as u32;
    let tx = fx.fract();
    let ty = fy.fract();
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let at = |ix: u32, iy: u32| {
        unit(mix64(
            seed ^ u64::from(ix % cells).wrapping_mul(0x9e37_79b9)
                ^ u64::from(iy % cells).wrapping_mul(0x85eb_ca6b),
        )) as f32
    };
    let sx = smooth(tx);
    let sy = smooth(ty);
    let low = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * sx;
    let high = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * sx;
    low + (high - low) * sy
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the sole environment compositor declares each independent ECS input"
)]
pub(super) fn compose_environment(
    mut commands: Commands,
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    weather: Res<SmoothedWeather>,
    cover: Res<PrecipitationOcclusionMap>,
    mut sun: Query<
        (
            Entity,
            &mut Transform,
            &mut DirectionalLight,
            Has<VolumetricLight>,
        ),
        With<Sun>,
    >,
    mut cameras: Query<
        (
            Entity,
            &GlobalTransform,
            &mut DistanceFog,
            Option<&mut AtmosphereEnvironmentMapLight>,
            Has<VolumetricFog>,
        ),
        With<Camera3d>,
    >,
    ambient: Option<ResMut<GlobalAmbientLight>>,
    clear_color: Option<ResMut<ClearColor>>,
    settings: Res<WeatherSettings>,
    mut fog_layers: Query<(&WeatherFogLayer, &mut FogVolume, &mut Visibility)>,
) {
    let clock = clock.as_deref().copied().unwrap_or_default();
    let (fraction, brightness) = if clock.present {
        (clock.fraction as f32, clock.brightness as f32)
    } else {
        (17.0 / 24.0, 1.0)
    };
    let lit = ((brightness - NIGHT_FLOOR) / (1.0 - NIGHT_FLOOR)).clamp(0.0, 1.0);
    let cloud = weather.cloud_cover;
    let storm = weather.thunder;
    let cloud_sun = direct_light_transmission(cloud, storm);

    if let Ok((sun_entity, mut transform, mut light, volumetric)) = sun.single_mut() {
        let direction = solar_direction(fraction);
        *transform = Transform::from_translation(direction * 500.0)
            .looking_at(Vec3::new(0.0, 0.0, 40.0), Vec3::Y);
        light.illuminance = lux::RAW_SUNLIGHT * lit * cloud_sun.max(0.025);
        light.color = Color::srgb(1.0 - 0.18 * cloud, 1.0 - 0.12 * cloud, 1.0 - 0.03 * cloud);
        // Same warm-up + hysteresis as the camera's VolumetricFog below: the
        // sunbeam pipeline variant compiles on first use too.
        let wants_volumetric = settings.volumetric_fog
            && (time.elapsed_secs() < VOLUMETRIC_WARMUP_SECONDS
                || if volumetric {
                    weather.fog > 0.015
                } else {
                    weather.fog > 0.03
                });
        if wants_volumetric != volumetric {
            if wants_volumetric {
                commands.entity(sun_entity).insert(VolumetricLight);
            } else {
                commands.entity(sun_entity).remove::<VolumetricLight>();
            }
        }
    }

    if let Some(mut ambient) = ambient {
        let day_fill = NIGHT_AMBIENT + (DAY_AMBIENT - NIGHT_AMBIENT) * lit.sqrt();
        // Cloud cools and weakens daylight, but never lifts the night floor.
        let weather_fill = 1.0 - cloud * 0.46 - storm * 0.16;
        ambient.brightness = NIGHT_AMBIENT + (day_fill - NIGHT_AMBIENT) * weather_fill.max(0.32);
        ambient.color = Color::srgb(
            0.58 - cloud * 0.10,
            0.66 - cloud * 0.08,
            0.72 - cloud * 0.02,
        );
    }
    if let Some(mut clear) = clear_color {
        clear.0 = Color::srgb(
            0.52 - cloud * 0.28,
            0.67 - cloud * 0.31,
            0.76 - cloud * 0.29,
        );
    }

    for (entity, transform, mut fog, environment, has_volumetric) in &mut cameras {
        let sheltered = cover.is_sheltered(transform.translation());
        let (local_fog, visibility) = fog_response(
            weather.fog,
            weather.visibility_m,
            transform.translation().y,
            sheltered,
        );
        fog.color = Color::srgba(
            0.67 - cloud * 0.13,
            0.75 - cloud * 0.13,
            0.79 - cloud * 0.11,
            1.0,
        );
        fog.directional_light_color = Color::srgba(0.92, 0.82, 0.68, 0.24 * (1.0 - cloud));
        fog.directional_light_exponent = 20.0;
        fog.falloff = FogFalloff::from_visibility_squared(visibility);
        if let Some(mut environment) = environment {
            environment.intensity = (0.65 * (1.0 - cloud * 0.60)).max(0.18);
        }
        // Warm-up: the first insertion of VolumetricFog in a session compiles
        // its pipelines — a classic one-time hitch that used to land at the
        // first mid-play fog onset. Forcing the pass on (at near-zero density)
        // during the opening seconds moves the compile into the load window.
        // Hysteresis afterwards: on past 0.02, off below 0.008, so the
        // threshold never flickers the component (and its pipeline state) on
        // and off frame to frame.
        let warming = time.elapsed_secs() < VOLUMETRIC_WARMUP_SECONDS;
        // Ablation lever (the `CATHEDRAL_NO_*` family): a 64-step raymarch over
        // two city-scale volumes is one of the render costs Bevy's diagnostics
        // do not price, so the way to price it is a run without it.
        let wants_volumetric = std::env::var_os("CATHEDRAL_NO_VOLUMETRIC_FOG").is_none()
            && settings.volumetric_fog
            && WeatherQuality::from_name(&settings.quality) != WeatherQuality::Low
            && (warming
                || if has_volumetric {
                    local_fog > 0.008
                } else {
                    local_fog > 0.02
                });
        if wants_volumetric != has_volumetric {
            if wants_volumetric {
                commands.entity(entity).insert(VolumetricFog {
                    ambient_color: Color::srgb(0.60, 0.67, 0.70),
                    ambient_intensity: 0.11,
                    jitter: 0.1,
                    step_count: match WeatherQuality::from_name(&settings.quality) {
                        WeatherQuality::Medium => 32,
                        WeatherQuality::High => 64,
                        WeatherQuality::Low => 16,
                    },
                });
            } else {
                commands.entity(entity).remove::<VolumetricFog>();
            }
        }
        // The volume itself is global; attenuation at the listener is conveyed
        // through its density so the nave stays readable while exterior fog is
        // still visible through the doors.
        for (layer, mut volume, mut visibility_component) in &mut fog_layers {
            let next_visibility = if wants_volumetric {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility_component != next_visibility {
                *visibility_component = next_visibility;
            }
            // A hidden volume needs no density animation; skipping the write
            // keeps the FogVolume unchanged for the render world.
            if next_visibility == Visibility::Hidden {
                continue;
            }
            volume.density_factor = local_fog * layer.strength * 0.075;
            let drift = weather.wind * time.elapsed_secs() * 0.000_45;
            volume.density_texture_offset = Vec3::new(
                drift.x.rem_euclid(1.0),
                (time.elapsed_secs() * 0.002).rem_euclid(1.0),
                drift.y.rem_euclid(1.0),
            );
        }
    }
}

fn solar_direction(fraction: f32) -> Vec3 {
    let lat = SUN_LATITUDE_DEG.to_radians();
    let decl = SUN_DECLINATION_DEG.to_radians();
    let (sin_lat, cos_lat) = lat.sin_cos();
    let (sin_decl, cos_decl) = decl.sin_cos();
    let hour_angle = (fraction - 0.5) * std::f32::consts::TAU;
    let (sin_h, cos_h) = hour_angle.sin_cos();
    let up = sin_lat * sin_decl + cos_lat * cos_decl * cos_h;
    let east = -cos_decl * sin_h;
    let south = (sin_lat * up - sin_decl) / cos_lat;
    Vec3::new(east, up.max(-0.15), south).normalize()
}

fn direct_light_transmission(cloud: f32, storm: f32) -> f32 {
    ((1.0 - cloud.clamp(0.0, 1.0) * 0.88) * (1.0 - storm.clamp(0.0, 1.0) * 0.55)).max(0.025)
}

/// Morning mist is a low layer rather than a camera-centred white wall. Rain
/// visibility remains altitude-independent; only the fog contribution burns
/// away as the camera climbs above the roofs.
fn fog_response(fog: f32, visibility_m: f32, camera_y: f32, sheltered: bool) -> (f32, f32) {
    let above_layer = ((camera_y - 12.0) / 38.0).clamp(0.0, 1.0);
    let altitude_relief = fog.clamp(0.0, 1.0) * above_layer;
    let local_fog = fog * (1.0 - above_layer) * if sheltered { 0.22 } else { 1.0 };
    let visibility =
        visibility_m.max(40.0) + (340.0 - visibility_m.max(40.0)).max(0.0) * altitude_relief;
    (
        local_fog.clamp(0.0, 1.0),
        visibility * if sheltered { 1.45 } else { 1.0 },
    )
}

pub(super) fn animate_clouds(
    time: Res<Time>,
    weather: Res<SmoothedWeather>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    mut layers: Query<(
        &CloudLayer,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
        &mut Visibility,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(camera) = camera.single() else { return };
    for (layer, mut transform, material_handle, mut visibility) in &mut layers {
        let cover = weather.cloud_cover;
        let next_visibility = if cover > 0.015 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
        // A hidden sheet needs no drift and, crucially, no material touch: a
        // `get_mut` on the shared material re-prepares it every frame.
        if next_visibility == Visibility::Hidden {
            continue;
        }
        // Both layers travel downwind; their unequal speeds shear the field
        // without making one cloud deck visibly contradict rain and smoke.
        let drift = cloud_drift(weather.wind, weather.gust, time.elapsed_secs(), layer.depth);
        transform.translation.x =
            camera.translation().x + drift.x.rem_euclid(CLOUD_TILE_M) - CLOUD_TILE_M * 0.5;
        transform.translation.z =
            camera.translation().z + drift.y.rem_euclid(CLOUD_TILE_M) - CLOUD_TILE_M * 0.5;
        let darkness = weather.thunder.max(weather.precipitation * 0.65);
        let shade = 0.68 - darkness * 0.34 + layer.depth * 0.08;
        let alpha = (cover * (0.24 + cover * 0.48) * (1.0 - layer.depth * 0.18)).clamp(0.0, 0.78);
        set_base_color_if_changed(
            &mut materials,
            &material_handle.0,
            Color::srgba(shade, shade + 0.025, shade + 0.045, alpha),
        );
    }
}

/// Writes a material's base color only when it actually differs:
/// `Assets::get_mut` marks the asset Modified even for an identical value,
/// which re-prepares the material and re-bins everything drawn with it.
fn set_base_color_if_changed(
    materials: &mut Assets<StandardMaterial>,
    handle: &Handle<StandardMaterial>,
    color: Color,
) {
    if materials.get(handle).is_some_and(|m| m.base_color != color)
        && let Some(mut material) = materials.get_mut(handle)
    {
        material.base_color = color;
    }
}

fn cloud_drift(wind: Vec2, gust: f32, elapsed: f32, depth: f32) -> Vec2 {
    let drift_scale = if depth < 0.5 { 1.0 } else { 0.57 };
    // Gust is a small bounded displacement around the monotonic downwind
    // travel. Multiplying total elapsed time by an oscillating speed would
    // eventually make the apparent cloud velocity reverse.
    let amount =
        elapsed * 0.025 * drift_scale + gust * 0.025 * (elapsed * 0.31 + depth * 2.7).sin();
    wind * amount
}

#[allow(
    clippy::too_many_arguments,
    reason = "the bounded precipitation batch is one Bevy system with disjoint ECS inputs"
)]
pub(super) fn animate_precipitation(
    time: Res<Time>,
    weather: Res<SmoothedWeather>,
    cover: Res<PrecipitationOcclusionMap>,
    state: Option<Res<WeatherMeshes>>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rain_visibility: Query<&mut Visibility, (With<RainBatch>, Without<RainImpactBatch>)>,
    mut impact_visibility: Query<&mut Visibility, (With<RainImpactBatch>, Without<RainBatch>)>,
) {
    let Some(state) = state else { return };
    let intensity = weather.precipitation.clamp(0.0, 1.0);
    let active = ((state.capacity as f32) * intensity.powf(0.72)).round() as usize;
    for mut visibility in &mut rain_visibility {
        *visibility = if active > 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let has_aftereffects = (weather.surface_wetness > 0.25 && intensity > 0.12)
        || weather.standing_water > 0.03
        || intensity > 0.72;
    for mut visibility in &mut impact_visibility {
        *visibility = if active > 20 || has_aftereffects {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let Ok(camera) = camera.single() else { return };
    let camera_position = camera.translation();
    let camera_right = camera.right().as_vec3();
    let horizontal_right = Vec3::new(camera_right.x, 0.0, camera_right.z).normalize_or(Vec3::X);
    let elapsed = time.elapsed_secs();
    let radius = 32.0;
    let fall_speed = 18.0 + intensity * 15.0;
    let span = 38.0;
    let anchor_x = (camera_position.x / 4.0).floor() * 4.0;
    let anchor_z = (camera_position.z / 4.0).floor() * 4.0;

    let mut positions = Vec::with_capacity(active * 4);
    let mut normals = Vec::with_capacity(active * 4);
    let mut uvs = Vec::with_capacity(active * 4);
    let mut colors = Vec::with_capacity(active * 4);
    let mut indices = Vec::with_capacity(active * 6);
    let mut splash_positions = Vec::with_capacity(active / state.splash_stride * 4);
    let mut splash_normals = Vec::with_capacity(active / state.splash_stride * 4);
    let mut splash_uvs = Vec::with_capacity(active / state.splash_stride * 4);
    let mut splash_colors = Vec::with_capacity(active / state.splash_stride * 4);
    let mut splash_indices = Vec::with_capacity(active / state.splash_stride * 6);

    for index in 0..active {
        let seed = mix64(index as u64 ^ 0x12f4_98ab_73c1);
        let x = anchor_x + signed_unit(seed) * radius;
        let z = anchor_z + signed_unit(mix64(seed ^ 0x91e1)) * radius;
        let phase = (elapsed * fall_speed / span + unit(mix64(seed ^ 0x3721)) as f32).fract();
        let top_y = camera_position.y + 21.0 - phase * span;
        let impact = cover.sample(x, z).impact_y;
        let long = 0.32 + intensity * 1.25;
        let Some((bottom_y, top_y)) = rain_vertical_segment(top_y, impact, long) else {
            continue;
        };
        let fall_seconds = (top_y - bottom_y) / fall_speed;
        let gust_pulse =
            1.0 + weather.gust * (0.20 + 0.22 * (elapsed * 1.37 + unit(seed) as f32 * 5.1).sin());
        let slant =
            Vec3::new(weather.wind.x, 0.0, weather.wind.y) * fall_seconds * gust_pulse.max(0.55);
        let top = Vec3::new(x, top_y, z);
        // `wind_xz_mps` says where the air travels. A falling drop therefore
        // reaches its lower endpoint downwind of its upper endpoint, matching
        // cloud and chimney-smoke drift instead of leaning against them.
        let bottom = Vec3::new(x, bottom_y, z) + slant;
        let half_width = 0.006 + intensity * 0.010;
        push_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            [
                bottom - horizontal_right * half_width,
                bottom + horizontal_right * half_width,
                top + horizontal_right * half_width,
                top - horizontal_right * half_width,
            ],
            [0.70, 0.82, 0.92, 0.30 + intensity * 0.46],
            -camera.forward().as_vec3(),
        );

        if index % state.splash_stride == 0 && top_y - impact < 1.15 {
            let radius = 0.035 + intensity * 0.10;
            let center = Vec3::new(x, impact + 0.025, z);
            push_quad(
                &mut splash_positions,
                &mut splash_normals,
                &mut splash_uvs,
                &mut splash_colors,
                &mut splash_indices,
                [
                    center + Vec3::new(-radius, 0.0, -radius),
                    center + Vec3::new(radius, 0.0, -radius),
                    center + Vec3::new(radius, 0.0, radius),
                    center + Vec3::new(-radius, 0.0, radius),
                ],
                [0.68, 0.80, 0.88, 0.22 + intensity * 0.30],
                Vec3::Y,
            );
        }
    }
    append_weather_aftereffects(
        state.as_ref(),
        &cover,
        camera_position,
        horizontal_right,
        elapsed,
        weather.as_ref(),
        &mut splash_positions,
        &mut splash_normals,
        &mut splash_uvs,
        &mut splash_colors,
        &mut splash_indices,
    );
    write_batch_mesh(
        &mut meshes,
        &state.rain,
        positions,
        normals,
        uvs,
        colors,
        indices,
    );
    write_batch_mesh(
        &mut meshes,
        &state.impacts,
        splash_positions,
        splash_normals,
        splash_uvs,
        splash_colors,
        splash_indices,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "the single impact mesh owns five parallel vertex buffers"
)]
fn append_weather_aftereffects(
    state: &WeatherMeshes,
    cover: &PrecipitationOcclusionMap,
    camera: Vec3,
    camera_right: Vec3,
    elapsed: f32,
    weather: &SmoothedWeather,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
) {
    if state.quality == WeatherQuality::High
        && weather.precipitation > 0.12
        && weather.surface_wetness > 0.25
    {
        let alpha = (weather.precipitation * weather.surface_wetness).clamp(0.0, 1.0);
        for (sequence, anchor) in state
            .runoff_anchors
            .iter()
            .filter(|anchor| anchor.top.xz().distance_squared(camera.xz()) < 36.0_f32.powi(2))
            .take(24)
            .enumerate()
        {
            let sway = (elapsed * 3.1 + sequence as f32 * 1.73).sin() * 0.018;
            let bottom = Vec3::new(anchor.top.x + sway, anchor.ground_y + 0.04, anchor.top.z);
            let top = anchor.top;
            let width = 0.008 + weather.precipitation * 0.014;
            push_quad(
                positions,
                normals,
                uvs,
                colors,
                indices,
                [
                    bottom - camera_right * width,
                    bottom + camera_right * width,
                    top + camera_right * width,
                    top - camera_right * width,
                ],
                [0.64, 0.76, 0.84, 0.12 + alpha * 0.34],
                -Vec3::Z,
            );
        }
    }

    if state.quality != WeatherQuality::Low && weather.standing_water > 0.03 {
        for (sequence, center) in state
            .puddle_centers
            .iter()
            .filter(|center| center.xz().distance_squared(camera.xz()) < 34.0_f32.powi(2))
            .take(38)
            .enumerate()
        {
            let phase = (elapsed * 0.85 + sequence as f32 * 0.317).fract();
            if phase > 0.42 {
                continue;
            }
            let radius = 0.04 + phase * (0.34 + weather.standing_water * 0.34);
            let alpha = (1.0 - phase / 0.42) * weather.standing_water * 0.34;
            push_quad(
                positions,
                normals,
                uvs,
                colors,
                indices,
                [
                    *center + Vec3::new(-radius, 0.0, -radius),
                    *center + Vec3::new(radius, 0.0, -radius),
                    *center + Vec3::new(radius, 0.0, radius),
                    *center + Vec3::new(-radius, 0.0, radius),
                ],
                [0.68, 0.80, 0.86, alpha],
                Vec3::Y,
            );
        }
    }

    if weather.precipitation > 0.72 {
        let anchor_x = (camera.x / 4.0).floor() * 4.0;
        let anchor_z = (camera.z / 4.0).floor() * 4.0;
        let mist_count = match state.quality {
            WeatherQuality::Low => 8,
            WeatherQuality::Medium => 16,
            WeatherQuality::High => 24,
        };
        for sequence in 0..mist_count {
            let hash = mix64(sequence ^ 0xe74a_11c3);
            let x = anchor_x + signed_unit(hash) * 26.0;
            let z = anchor_z + signed_unit(hash.rotate_left(23)) * 26.0;
            let sample = cover.sample(x, z);
            if sample.material != super::CoverMaterial::Open {
                continue;
            }
            let half = 0.26 + unit(hash.rotate_left(37)) as f32 * 0.48;
            let base = Vec3::new(x, sample.ground_y + 0.035, z);
            push_quad(
                positions,
                normals,
                uvs,
                colors,
                indices,
                [
                    base - camera_right * half,
                    base + camera_right * half,
                    base + camera_right * half + Vec3::Y * 0.18,
                    base - camera_right * half + Vec3::Y * 0.18,
                ],
                [0.72, 0.79, 0.82, 0.025 + weather.precipitation * 0.055],
                -Vec3::Z,
            );
        }
    }
}

fn rain_vertical_segment(top_y: f32, impact_y: f32, length: f32) -> Option<(f32, f32)> {
    if top_y <= impact_y + 0.04 {
        return None;
    }
    let bottom_y = (top_y - length).max(impact_y + 0.035);
    (bottom_y < top_y - 0.015).then_some((bottom_y, top_y))
}

#[allow(clippy::too_many_arguments)]
fn push_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    corners: [Vec3; 4],
    color: [f32; 4],
    normal: Vec3,
) {
    let first = positions.len() as u32;
    positions.extend(corners.map(|corner| corner.to_array()));
    normals.extend([normal.to_array(); 4]);
    uvs.extend([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
    colors.extend([color; 4]);
    indices.extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
}

fn puddle_mesh(cover: &PrecipitationOcclusionMap) -> (Mesh, Vec<Vec3>) {
    let nav = cathedral_sim::NavData::from_parts(
        include_str!("../../assets/world/navigation.json"),
        include_bytes!("../../assets/world/navigation.bin"),
    )
    .expect("the committed navigation bake already validates at startup");
    let (min, max) = cover.bounds();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    let mut centers = Vec::new();
    let mut index = 0_u64;
    let mut z = min.y + 8.0;
    while z < max.y - 8.0 && positions.len() / 4 < 720 {
        let mut x = min.x + 8.0;
        while x < max.x - 8.0 && positions.len() / 4 < 720 {
            let hash = mix64(index ^ 0xa21c_7f9d);
            let center = Vec2::new(
                x + signed_unit(hash) * 5.0,
                z + signed_unit(hash.rotate_left(21)) * 5.0,
            );
            let sample = cover.sample(center.x, center.y);
            // Stable sparse candidates only on exposed, near-ground cells.
            if puddle_candidate(sample, hash) && puddle_location_is_clear(&nav, center) {
                let half_x = 0.35 + unit(hash.rotate_left(9)) as f32 * 0.8;
                let half_z = 0.22 + unit(hash.rotate_left(37)) as f32 * 0.55;
                let y = sample.ground_y + 0.018;
                centers.push(Vec3::new(center.x, y + 0.004, center.y));
                push_quad(
                    &mut positions,
                    &mut normals,
                    &mut uvs,
                    &mut colors,
                    &mut indices,
                    [
                        Vec3::new(center.x - half_x, y, center.y - half_z),
                        Vec3::new(center.x + half_x, y, center.y - half_z),
                        Vec3::new(center.x + half_x, y, center.y + half_z),
                        Vec3::new(center.x - half_x, y, center.y + half_z),
                    ],
                    [1.0, 1.0, 1.0, 1.0],
                    Vec3::Y,
                );
            }
            index += 1;
            x += 13.0;
        }
        z += 13.0;
    }
    (
        batch_mesh(positions, normals, uvs, colors, indices),
        centers,
    )
}

fn puddle_candidate(sample: super::cover::CoverSample, hash: u64) -> bool {
    sample.puddle_surface
        && !sample.sheltered_listener
        && sample.impact_y <= sample.ground_y + 0.05
        && hash.is_multiple_of(11)
}

/// Stay on broad walkable paving while avoiding door thresholds and the
/// narrowest navigation chokes. The puddle is non-colliding, but keeping a dry
/// shoulder around it makes those routes read as deliberate passages.
fn puddle_location_is_clear(nav: &cathedral_sim::NavData, center: Vec2) -> bool {
    const SHOULDER_M: f64 = 1.35;
    [
        [0.0, 0.0],
        [SHOULDER_M, 0.0],
        [-SHOULDER_M, 0.0],
        [0.0, SHOULDER_M],
        [0.0, -SHOULDER_M],
    ]
    .into_iter()
    .all(|offset| {
        nav.is_walkable(
            f64::from(center.x) + offset[0],
            f64::from(center.y) + offset[1],
        )
    })
}

pub(super) fn update_weather_materials(
    weather: Res<SmoothedWeather>,
    state: Option<Res<WeatherMeshes>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut puddles: Query<&mut Visibility, With<PuddleBatch>>,
) {
    let Some(state) = state else { return };
    set_base_color_if_changed(
        &mut materials,
        &state.puddle_material,
        Color::srgba(0.10, 0.17, 0.20, weather.standing_water * 0.72),
    );
    for mut visibility in &mut puddles {
        let next = if state.quality != WeatherQuality::Low && weather.standing_water > 0.015 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    set_base_color_if_changed(
        &mut materials,
        &state.rain_material,
        Color::srgba(0.70, 0.82, 0.92, 0.64 + weather.precipitation * 0.22),
    );
    set_base_color_if_changed(
        &mut materials,
        &state.impact_material,
        Color::srgba(0.67, 0.78, 0.86, 0.28 + weather.precipitation * 0.30),
    );
    // Touch cloud handles here so asset loss is diagnosed by absence rather
    // than a stale resource; color itself is updated by `animate_clouds`.
    let _ = &state.cloud_materials;
}

pub(super) fn present_lightning(
    mut commands: Commands,
    time: Res<Time>,
    mut strikes: MessageReader<WeatherLightning>,
) {
    let now = time.elapsed_secs_f64();
    for strike in strikes.read() {
        let origin = Vec3::new(
            strike.0.origin_m[0] as f32,
            strike.0.origin_m[1] as f32,
            strike.0.origin_m[2] as f32,
        );
        commands.spawn((
            Name::new(format!("Weather lightning flash {}", strike.0.id)),
            LightningFlash {
                started_at: now,
                strength: strike.0.strength as f32,
                secondary: false,
            },
            PointLight {
                color: Color::srgb(0.72, 0.84, 1.0),
                intensity: lightning_flash_intensity(0.0, strike.0.strength as f32),
                // Strike origins sit at y ∈ [360, 680] m, so the range must
                // reach the ground from sky height with slant to spare — a
                // tighter radius silently stopped most flashes lighting the
                // city at all. It costs every cluster for only 0.24 s.
                range: 1_400.0,
                radius: 24.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(origin),
        ));
        // A simultaneous interior flare makes the rose response legible from
        // the nave without rebuilding a shadow map.
        commands.spawn((
            Name::new(format!("Rose-window lightning flare {}", strike.0.id)),
            LightningFlash {
                started_at: now,
                strength: strike.0.strength as f32 * 0.42,
                secondary: true,
            },
            PointLight {
                color: Color::srgb(0.66, 0.73, 1.0),
                intensity: 8_000_000.0 * strike.0.strength as f32,
                range: 115.0,
                radius: 6.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, 23.0, 68.0),
        ));
    }
}

pub(super) fn update_lightning_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &LightningFlash, &mut PointLight)>,
    rose: Res<WeatherRoseWindow>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs_f64();
    let mut rose_boost = 0.0_f32;
    for (entity, flash, mut light) in &mut flashes {
        let elapsed = now - flash.started_at;
        if elapsed >= LIGHTNING_FLASH_SECONDS {
            commands.entity(entity).despawn();
        } else {
            light.intensity = lightning_flash_intensity(elapsed, flash.strength)
                * if flash.secondary { 0.08 } else { 1.0 };
            if flash.secondary {
                rose_boost = rose_boost.max(lightning_pulse(elapsed) * flash.strength * 9.0);
            }
        }
    }
    if let Some(handle) = &rose.handle {
        let target = rose.baseline_emissive * (1.0 + rose_boost);
        // Outside storms rose_boost is 0.0 every frame; leave the shared
        // cathedral material untouched unless the flash actually moves it.
        if materials.get(handle).is_some_and(|m| m.emissive != target)
            && let Some(mut material) = materials.get_mut(handle)
        {
            material.emissive = target;
        }
    }
}

fn lightning_pulse(elapsed: f64) -> f32 {
    match elapsed {
        t if t < 0.045 => 1.0,
        t if t < 0.085 => 0.075,
        t if t < 0.135 => 0.47,
        t if t < LIGHTNING_FLASH_SECONDS => {
            ((LIGHTNING_FLASH_SECONDS - t) / (LIGHTNING_FLASH_SECONDS - 0.135)) as f32 * 0.22
        }
        _ => 0.0,
    }
}

fn lightning_flash_intensity(elapsed: f64, strength: f32) -> f32 {
    lightning_pulse(elapsed) * 320_000_000.0 * strength
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn unit(hash: u64) -> f64 {
    ((hash >> 11) as f64) * (1.0 / ((1_u64 << 53) as f64))
}

fn signed_unit(hash: u64) -> f32 {
    (unit(hash) as f32) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_capacity_is_bounded_and_monotonic() {
        for capacity in [800_usize, 1_450, 2_200] {
            let mut previous = 0;
            for step in 0..=100 {
                let intensity = step as f32 / 100.0;
                let count = (capacity as f32 * intensity.powf(0.72)).round() as usize;
                assert!(count >= previous);
                assert!(count <= capacity);
                previous = count;
            }
        }
    }

    #[test]
    fn representative_cloud_states_dim_direct_light_monotonically() {
        let clear = direct_light_transmission(0.08, 0.0);
        let broken = direct_light_transmission(0.45, 0.0);
        let overcast = direct_light_transmission(0.88, 0.0);
        let storm = direct_light_transmission(1.0, 1.0);
        assert!(clear > broken && broken > overcast && overcast > storm);
        assert!(storm >= 0.025);
    }

    #[test]
    fn streaks_stop_above_cover_and_still_exist_for_a_flying_camera() {
        let (bottom, top) = rain_vertical_segment(10.0, 8.0, 5.0).unwrap();
        assert!(bottom > 8.0 && top > bottom);
        assert!(rain_vertical_segment(8.03, 8.0, 1.0).is_none());
        let (flying_bottom, _) = rain_vertical_segment(105.0, 84.0, 1.5).unwrap();
        assert!(flying_bottom > 84.0);
    }

    #[test]
    fn puddle_candidates_are_stable_and_reject_covered_cells() {
        let open = super::super::cover::CoverSample {
            puddle_surface: true,
            ..default()
        };
        assert!(puddle_candidate(open, 22));
        assert_eq!(puddle_candidate(open, 22), puddle_candidate(open, 22));
        let covered = super::super::cover::CoverSample {
            impact_y: 8.0,
            sheltered_listener: true,
            ..open
        };
        assert!(!puddle_candidate(covered, 22));
    }

    #[test]
    fn committed_puddles_and_runoff_are_static_bounded_batches() {
        let cover = PrecipitationOcclusionMap::default();
        let anchors = build_runoff_anchors(&cover);
        assert!(!anchors.is_empty());
        assert!(anchors.len() < 5_000, "{} runoff anchors", anchors.len());
        let (_, puddles) = puddle_mesh(&cover);
        assert!(!puddles.is_empty());
        assert!(puddles.len() <= 720);
    }

    #[test]
    fn flash_has_two_pulses_and_ends_before_thunder_can_arrive() {
        assert!(lightning_flash_intensity(0.0, 1.0) > lightning_flash_intensity(0.06, 1.0));
        assert!(lightning_flash_intensity(0.10, 1.0) > lightning_flash_intensity(0.06, 1.0));
        assert_eq!(lightning_flash_intensity(LIGHTNING_FLASH_SECONDS, 1.0), 0.0);
    }

    #[test]
    fn fog_is_dense_at_street_level_and_clears_above_the_roofs() {
        let (street_density, street_visibility) = fog_response(0.9, 70.0, 2.0, false);
        let (sky_density, sky_visibility) = fog_response(0.9, 70.0, 70.0, false);
        assert!(street_density > 0.8);
        assert!(street_visibility < 100.0);
        assert_eq!(sky_density, 0.0);
        assert!(sky_visibility > 300.0);
    }

    #[test]
    fn falling_rain_moves_downwind_like_clouds_and_smoke() {
        let wind = Vec3::new(4.0, 0.0, -2.0);
        let top = Vec3::new(1.0, 8.0, 3.0);
        let bottom = Vec3::new(1.0, 3.0, 3.0) + wind * 0.2;
        assert!((bottom - top).xz().dot(wind.xz()) > 0.0);
        let cloud_wind = Vec2::new(wind.x, wind.z);
        for depth in [0.0, 1.0] {
            let early = cloud_drift(cloud_wind, 1.0, 4.0, depth);
            let later = cloud_drift(cloud_wind, 1.0, 5.0, depth);
            assert!((later - early).dot(cloud_wind) > 0.0);
        }
    }
}
