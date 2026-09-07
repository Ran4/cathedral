//! Fixed-camera animal captures. Run with `uv run scripts/capture_animals.py`.
//! This harness compiles the production dog module and extracts the existing
//! loft helpers verbatim; the city/probe adapters only supply a flat stage.
#![allow(dead_code)]

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy::window::{PresentMode, WindowResolution};
use bevy::winit::{UpdateMode, WinitSettings};
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;

// Count allocations made inside the production dog spans. The allocator is
// transparent when disabled; renderer/startup/screenshot work is excluded.
struct CountingAllocator;
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
unsafe impl std::alloc::GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        perf::record_allocation();
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        perf::record_allocation();
        unsafe { std::alloc::System.realloc(ptr, layout, size) }
    }
}

mod body {
    include!(concat!(env!("ANIMAL_STUDIO_BUILD_DIR"), "/loft.rs"));
}
#[path = "../src/smart_actors/dogs.rs"]
mod dogs;
mod rats {
    include!(concat!(env!("ANIMAL_STUDIO_BUILD_DIR"), "/rat.rs"));
}
mod city {
    use bevy::prelude::*;
    #[derive(Resource)]
    pub struct CutMarginProfile;
    impl CutMarginProfile {
        pub fn ground_lift(&self, _: f32, _: f32) -> f32 { 0.0 }
    }
}
mod controller {
    use bevy::prelude::*;
    #[derive(Component)]
    pub struct PlayerCamera;
}
mod perf {
    use std::{sync::atomic::{AtomicU64, Ordering}, time::Instant};
    static NANOS: AtomicU64 = AtomicU64::new(0);
    static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
    std::thread_local! { static TRACKING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    pub enum Probe { Dogs, Rats }
    pub struct Span(Instant);
    impl Drop for Span {
        fn drop(&mut self) {
            TRACKING.set(false);
            NANOS.fetch_add(self.0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
    }
    pub fn span(_: Probe) -> Span { TRACKING.set(true); Span(Instant::now()) }
    pub fn record_allocation() {
        if TRACKING.try_with(|tracking| tracking.get()).unwrap_or(false) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn take() -> u64 { NANOS.swap(0, Ordering::Relaxed) }
    pub fn take_allocations() -> u64 { ALLOCATIONS.swap(0, Ordering::Relaxed) }
}

#[derive(Resource)]
struct Capture {
    out: PathBuf,
    views: Vec<(String, Vec3, Vec3)>,
    view: usize,
    frames: usize,
    awaiting: bool,
    saved: Arc<AtomicBool>,
    rat: bool,
    moving: bool,
    animation: bool,
    steps_per_frame: usize,
    initial_steps: usize,
    motion: String,
    travel: bool,
    ramp: bool,
    speed: f32,
    observer: String,
}

#[derive(Resource, Default)]
struct MotionDriver {
    position: Vec3,
    phase: f32,
    since_sample: f32,
    yaw: f32,
    speed: f32,
    production: Option<ProductionMotion>,
    rat_trace: Option<RatTrace>,
}

struct RatTrace {
    rows: Vec<(f32, Vec3, Vec2, f32)>,
    origin: Vec3,
}

impl RatTrace {
    fn load(path: &str) -> Self {
        let text = std::fs::read_to_string(path).unwrap();
        let rows: Vec<_> = text.lines().skip(1).filter(|s| !s.is_empty()).map(|line| {
            let cells: Vec<f32> = line.split(',').map(|x| x.parse().unwrap()).collect();
            (cells[0], Vec3::new(cells[1], 0.0, cells[2]), Vec2::new(cells[3], cells[4]),
                cells.get(5).copied().unwrap_or(f32::INFINITY))
        }).collect();
        let origin = rows[0].1;
        Self { rows, origin }
    }

    fn sample(&self, elapsed: f32) -> (Vec3, Vec2, f32) {
        let next = self.rows.partition_point(|row| row.0 < elapsed).min(self.rows.len() - 1);
        let a = self.rows[next.saturating_sub(1)];
        let b = self.rows[next];
        let t = if b.0 > a.0 { ((elapsed - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 0.0 };
        (a.1.lerp(b.1, t) - self.origin, if t < 0.5 { a.2 } else { b.2 },
            if t < 0.5 { a.3 } else { b.3 })
    }
}

struct ProductionMotion {
    nav: cathedral_sim::NavData,
    pack: Vec<cathedral_sim::Dog>,
    origin: cathedral_sim::Vec3,
}

impl ProductionMotion {
    fn step(&mut self) -> (Vec3, f32, f32, f32) {
        cathedral_sim::dogs::step_dogs(&mut self.pack, 0.05, &self.nav);
        let dog = &self.pack[0];
        let delta = dog.position_m - self.origin;
        (Vec3::new(delta.x as f32, 0.0, delta.z as f32),
         dog.facing_yaw as f32, dog.speed as f32, dog.gait_phase as f32)
    }
}

#[derive(Component)]
struct StudioRat;

#[derive(Component)]
struct StudioObserver;

#[derive(Resource, Default)]
struct SkinCpuSample {
    nanos: u64,
    meshes: usize,
    joints: usize,
}

fn benchmark_skin_work(
    meshes: Res<Assets<Mesh>>,
    bindposes: Res<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    skins: Query<(&Mesh3d, &bevy::mesh::skinning::SkinnedMesh, Option<&GlobalTransform>)>,
    transforms: Query<&GlobalTransform>,
    mut sample: ResMut<SkinCpuSample>,
) {
    use bevy::mesh::skinning::entity_aabb_from_skinned_mesh_bounds;
    let start = std::time::Instant::now();
    let (mut mesh_count, mut joint_count) = (0, 0);
    for (mesh, skin, global) in &skins {
        let mesh = meshes.get(&mesh.0).unwrap();
        let inverse = bindposes.get(&skin.inverse_bindposes).unwrap();
        // Actual Bevy bound calculation. Also do the same changed-joint matrix
        // arithmetic used by skin extraction, conservatively for every joint.
        // This excludes render-world bookkeeping, staging buffers and uploads.
        std::hint::black_box(entity_aabb_from_skinned_mesh_bounds(&transforms, mesh, skin, inverse, global).unwrap());
        for (joint, inverse) in skin.joints.iter().zip(inverse.iter()) {
            std::hint::black_box(transforms.get(*joint).unwrap().affine() * *inverse);
            joint_count += 1;
        }
        mesh_count += 1;
    }
    sample.nanos = start.elapsed().as_nanos() as u64;
    sample.meshes = mesh_count;
    sample.joints = joint_count;
}

fn main() {
    let out = std::env::var("ANIMAL_CAPTURE_OUT").expect("ANIMAL_CAPTURE_OUT");
    std::fs::create_dir_all(&out).unwrap();
    if std::env::var_os("ANIMAL_BENCH").is_some() {
        benchmark(PathBuf::from(out));
        return;
    }
    let rat = std::env::var("ANIMAL_SPECIES").as_deref() == Ok("rat");
    let motion = std::env::var("ANIMAL_MOTION").unwrap_or_else(|_| "still".into());
    let animation = motion != "still";
    let mut app = App::new();
    app.add_plugins(DefaultPlugins
        .set(WindowPlugin { primary_window: Some(Window {
            title: "Cathedral animal capture (invisible)".into(),
            visible: false,
            resolution: WindowResolution::new(960, 720).with_scale_factor_override(1.0),
            present_mode: PresentMode::AutoNoVsync,
            ..default()
        }), ..default() })
        .disable::<bevy::audio::AudioPlugin>())
        .insert_resource(WinitSettings { focused_mode: UpdateMode::Continuous, unfocused_mode: UpdateMode::Continuous })
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(ClearColor(Color::srgb(0.105, 0.12, 0.135)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.83, 0.88, 1.0), brightness: 230.0, ..default() })
        .init_resource::<dogs::DogInbox>()
        .init_resource::<MotionDriver>()
        .init_resource::<rats::StudioState>()
        .insert_resource(Capture {
            out: out.into(), view: 0, frames: 0, awaiting: false,
            saved: Arc::new(AtomicBool::new(false)),
            rat, moving: motion == "trot", animation,
            steps_per_frame: 60 / std::env::var("ANIMAL_FPS").unwrap_or_else(|_| "12".into()).parse::<usize>().unwrap(),
            initial_steps: std::env::var("ANIMAL_INITIAL_STEPS").unwrap_or_else(|_| "24".into()).parse().unwrap(),
            motion: motion.clone(),
            travel: std::env::var("ANIMAL_TRAVEL").as_deref() == Ok("1"),
            ramp: std::env::var("ANIMAL_RAMP").as_deref() == Ok("1"),
            speed: std::env::var("ANIMAL_SPEED").unwrap_or_else(|_| "1.7".into()).parse().unwrap(),
            observer: std::env::var("ANIMAL_OBSERVER").unwrap_or_else(|_| "camera".into()),
            views: vec![
                ("three_quarter".into(), Vec3::new(1.35, 0.85, -1.8), Vec3::new(0.0, 0.38, -0.04)),
                ("side".into(), Vec3::new(2.2, 0.6, 0.0), Vec3::new(0.0, 0.38, 0.0)),
                ("front".into(), Vec3::new(0.0, 0.7, -2.05), Vec3::new(0.0, 0.40, -0.1)),
                ("rear".into(), Vec3::new(-1.5, 0.8, 1.7), Vec3::new(0.0, 0.38, 0.0)),
                ("street_distance".into(), Vec3::new(2.5, 1.6, -4.0), Vec3::new(0.0, 0.38, 0.0)),
            ],
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (feed_motion, drive_observer, dogs::sync_dogs, dogs::drive_dog_bodies,
            dogs::animate_dog_gait.run_if(animation_enabled), track_camera, capture).chain());
    {
        let mut capture = app.world_mut().resource_mut::<Capture>();
        if rat {
            for (name, eye, target) in &mut capture.views {
                *eye *= if name == "side" { 0.38 } else { 0.32 };
                *target *= 0.25;
                eye.y += 0.012;
                target.y += 0.012;
                // Include the naked tail in the side silhouette without
                // making the face views smaller.
                if name == "side" {
                    eye.z += 0.065;
                    target.z += 0.065;
                }
                if name == "street_distance" {
                    *eye = Vec3::new(1.5, 1.6, -2.2);
                    *target = Vec3::new(0.0, 0.07, 0.0);
                }
            }
        }
        if animation {
            let name = std::env::var("ANIMAL_VIEW").unwrap_or_else(|_| "three_quarter".into());
            let (_, eye, target) = capture.views.iter().find(|v| v.0 == name).unwrap().clone();
            let seconds = std::env::var("ANIMAL_SECONDS").unwrap_or_else(|_| "4".into()).parse::<f32>().unwrap();
            let frames = (seconds * (60 / capture.steps_per_frame) as f32).round() as usize;
            capture.views = (0..frames).map(|frame| (format!("frame_{frame:03}"), eye, target)).collect();
        }
    }
    app.run();
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * fraction).round() as usize]
}

fn benchmark(out: PathBuf) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>().init_asset::<Image>().init_asset::<StandardMaterial>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_resource::<SkinCpuSample>()
        .init_resource::<dogs::DogInbox>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
        .add_systems(Update, (dogs::sync_dogs, dogs::drive_dog_bodies, dogs::animate_dog_gait).chain())
        .add_systems(PostUpdate, benchmark_skin_work.after(bevy::transform::TransformSystems::Propagate));
    app.world_mut().spawn((controller::PlayerCamera, Camera3d::default(), Transform::from_xyz(0.0, 1.6, -3.0)));
    use cathedral_sim::DogCoat;
    let forms = [(DogCoat::Brindle, 1.0), (DogCoat::Black, 1.12),
        (DogCoat::Grey, 1.15), (DogCoat::Fawn, 1.2),
        (DogCoat::White, 0.72), (DogCoat::Pied, 0.9)];
    for index in 0..10 {
        let (coat, build) = forms[index % forms.len()];
        app.world_mut().resource_mut::<dogs::DogInbox>().0.insert(format!("dog_bench_{index}"), dogs::DogSample {
            name: format!("Bench {index}"), coat, build,
            position: Vec3::new(index as f32 * 1.2, 0.91, 0.0), facing_yaw: 0.0,
            speed: 1.7, gait_phase: 0.0, seq: 0,
        });
    }
    let mut dog_ms = Vec::new();
    let mut update_ms = Vec::new();
    let mut skin_ms = Vec::new();
    let mut dog_allocations = 0;
    let mut max_allocations = 0;
    for frame in 0..3200 {
        if frame % 3 == 0 {
            for sample in app.world_mut().resource_mut::<dogs::DogInbox>().0.values_mut() {
                sample.speed = if frame < 1700 { 1.7 } else { 0.0 };
                sample.position.z -= sample.speed * 0.05;
                sample.gait_phase += sample.speed * 0.05 * cathedral_sim::dogs::DOG_GAIT_CADENCE as f32;
                sample.seq += 1;
            }
        }
        perf::take();
        perf::take_allocations();
        let start = std::time::Instant::now();
        app.update();
        let update = start.elapsed().as_secs_f64() * 1000.0;
        let nanos = perf::take();
        let allocations = perf::take_allocations();
        if frame >= 200 {
            dog_ms.push(nanos as f64 / 1.0e6);
            update_ms.push(update);
            skin_ms.push(app.world().resource::<SkinCpuSample>().nanos as f64 / 1.0e6);
            dog_allocations += allocations;
            max_allocations = max_allocations.max(allocations);
        }
    }
    dog_ms.sort_by(f64::total_cmp);
    update_ms.sort_by(f64::total_cmp);
    skin_ms.sort_by(f64::total_cmp);
    let skin_work = app.world().resource::<SkinCpuSample>();
    let (skin_meshes, skin_joints) = (skin_work.meshes, skin_work.joints);
    let world = app.world_mut();
    let handles: Vec<_> = world.query::<&Mesh3d>().iter(world).map(|m| m.0.clone()).collect();
    let meshes = world.resource::<Assets<Mesh>>();
    let vertices: usize = handles.iter().map(|h| meshes.get(h).unwrap().count_vertices()).sum();
    let triangles: usize = handles.iter().map(|h| meshes.get(h).unwrap().indices().map_or(0, |i| i.len() / 3)).sum();
    let rats = rats::studio_benchmark();
    let json = format!(concat!(
        "{{\n  \"method\":\"CPU only, fixture v4: ten travelling dogs across six coat/build forms; fifty distinct seeded rats travelling then stopping; production dog spans and rat pose plus geometry with reused buffers; no renderer\",\n",
        "  \"dogs\":{{\"count\":10,\"samples\":{},\"p50_ms\":{},\"p95_ms\":{},\"p99_ms\":{},\"mesh_entities\":{},\"unique_mesh_assets\":{},\"vertices_drawn\":{},\"triangles_drawn\":{},\"system_allocations_total\":{},\"system_allocations_max_per_frame\":{},\"app_update_with_transforms_p50_ms\":{},\"app_update_with_transforms_p95_ms\":{}}},\n",
        "  \"skin_cpu\":{{\"method\":\"Actual Bevy dynamic skin bounds plus joint-matrix arithmetic; all joints, no render-world bookkeeping or GPU upload\",\"meshes\":{},\"joint_matrices\":{},\"p50_ms\":{},\"p95_ms\":{}}},\n",
        "  \"rats\":{}\n}}\n"), dog_ms.len(), percentile(&dog_ms, 0.5), percentile(&dog_ms, 0.95),
        percentile(&dog_ms, 0.99), handles.len(), meshes.len(), vertices, triangles, dog_allocations,
        max_allocations, percentile(&update_ms, 0.5), percentile(&update_ms, 0.95),
        skin_meshes, skin_joints, percentile(&skin_ms, 0.5), percentile(&skin_ms, 0.95), rats);
    std::fs::write(out.join("performance.json"), &json).unwrap();
    println!("{json}");
}

fn animation_enabled(capture: Res<Capture>) -> bool { capture.animation }

fn feed_motion(mut time: ResMut<Time>, mut virtual_time: ResMut<Time<Virtual>>,
    capture: Res<Capture>, mut inbox: ResMut<dogs::DogInbox>, mut driver: ResMut<MotionDriver>,
    mut rats: Query<(&Mesh3d, &mut Transform), With<StudioRat>>, mut rat_state: ResMut<rats::StudioState>,
    mut meshes: ResMut<Assets<Mesh>>) {
    if !capture.animation { return; }
    // Screenshot readback can take variable render frames. Pause the animal
    // clock during it: output samples retain their requested 60 Hz step count.
    let advancing = !capture.awaiting && capture.frames < if capture.view == 0 { capture.initial_steps } else { capture.steps_per_frame };
    virtual_time.advance_by(if advancing { Duration::from_secs_f64(1.0 / 60.0) } else { Duration::ZERO });
    *time = virtual_time.as_generic();
    let seconds = time.elapsed_secs();
    if capture.rat && capture.motion == "production" {
        if !advancing { return; }
        let (position, heading, pause_remaining) = driver.rat_trace.as_ref().unwrap().sample(seconds);
        let speed = position.distance(driver.position) / time.delta_secs().max(1e-6);
        driver.position = position;
        driver.speed = speed;
        driver.yaw = (-heading.x).atan2(-heading.y);
        rat_state.update(position.xz(), heading, speed, seconds, pause_remaining);
        for (mesh, _) in &mut rats { *meshes.get_mut(&mesh.0).unwrap() = rat_state.mesh(); }
        return;
    }
    if capture.motion == "production" {
        driver.since_sample += time.delta_secs();
        if driver.since_sample >= 0.049 {
            let (position, yaw, speed, phase) = driver.production.as_mut().unwrap().step();
            driver.since_sample = 0.0;
            driver.position = position;
            driver.yaw = yaw;
            driver.speed = speed;
            driver.phase = phase;
            for sample in inbox.0.values_mut() {
                sample.position = position + Vec3::Y * 0.91;
                sample.facing_yaw = yaw;
                sample.speed = speed;
                sample.gait_phase = phase;
                sample.seq += 1;
            }
        }
        return;
    }
    let requested_speed = if capture.motion == "sequence" {
        match seconds {
            t if t < 6.0 => 0.0,
            t if t < 8.0 => 0.65,
            t if t < 12.0 => capture.speed,
            t if t < 18.0 => 0.0,
            t if t < 22.0 => 0.9,
            _ => 0.0,
        }
    } else if capture.motion == "sit_departure" {
        if seconds < 12.0 { 0.0 } else { capture.speed }
    } else if capture.moving { capture.speed } else { 0.0 };
    driver.yaw = if capture.motion == "sequence" {
        ((seconds - 18.0) / 3.0).clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2
    } else { 0.0 };
    driver.since_sample += time.delta_secs();
    let publish = driver.since_sample >= 0.049;
    let speed = if capture.ramp {
        if publish {
            let rate = if requested_speed > driver.speed {
                cathedral_sim::dogs::DOG_ACCEL_MPS2 as f32
            } else {
                cathedral_sim::dogs::DOG_BRAKE_MPS2 as f32
            };
            driver.speed + (requested_speed - driver.speed).clamp(-rate * 0.05, rate * 0.05)
        } else { driver.speed }
    } else { requested_speed };
    // Ramped motion advances exactly once per published 20 Hz sample, matching
    // the production acceleration limits and distance clock. The fixture's
    // scheduled stop is not a navigation destination or stopping-distance test.
    let motion_dt = if capture.ramp { if publish { 0.05 } else { 0.0 } } else { time.delta_secs() };
    let velocity = Quat::from_rotation_y(driver.yaw) * Vec3::NEG_Z * speed;
    driver.speed = speed;
    driver.position += velocity * motion_dt;
    driver.phase += speed * motion_dt * cathedral_sim::dogs::DOG_GAIT_CADENCE as f32;
    if publish {
        driver.since_sample = 0.0;
        for sample in inbox.0.values_mut() {
            sample.speed = speed;
            sample.gait_phase = driver.phase;
            sample.facing_yaw = driver.yaw;
            if capture.travel { sample.position = Vec3::new(0.0, 0.91, 0.0) + driver.position; }
            sample.seq += 1;
        }
    }
    if capture.rat && advancing {
        let heading = (Quat::from_rotation_y(driver.yaw) * Vec3::NEG_Z).xz();
        rat_state.update(driver.position.xz(), heading, speed, seconds,
            if speed > 0.01 { 0.0 } else { f32::INFINITY });
        for (mesh, mut transform) in &mut rats {
            *meshes.get_mut(&mesh.0).unwrap() = rat_state.mesh();
            transform.translation = if capture.travel { Vec3::ZERO } else { -driver.position };
        }
    }
}

fn track_camera(state: Res<Capture>, driver: Res<MotionDriver>,
    dogs: Query<&Transform, (With<dogs::StreetDog>, Without<Camera3d>)>,
    mut camera: Single<&mut Transform, (With<Camera3d>, Without<dogs::StreetDog>, Without<StudioRat>)>) {
    if !state.travel || state.view >= state.views.len() { return; }
    let offset = if state.rat { Some(driver.position) }
        else { dogs.iter().next().map(|root| Vec3::new(root.translation.x, 0.0, root.translation.z)) };
    if let Some(offset) = offset {
        **camera = Transform::from_translation(state.views[state.view].1 + offset)
            .looking_at(state.views[state.view].2 + offset, Vec3::Y);
    }
}

fn drive_observer(capture: Res<Capture>, time: Res<Time>, driver: Res<MotionDriver>,
    mut observer: Query<&mut Transform, With<StudioObserver>>) {
    for mut transform in &mut observer {
        let offset = if capture.travel { driver.position } else { Vec3::ZERO };
        let (position, target) = if capture.observer == "approach" {
            let t = ((time.elapsed_secs() - 4.0) / 6.0).clamp(0.0, 1.0);
            let t = t * t * (3.0 - 2.0 * t);
            (Vec3::new(4.3, 1.6, -7.5).lerp(Vec3::new(1.8, 1.6, -2.8), t), Vec3::new(0.0, 0.61, -0.40))
        } else {
            let eye = capture.views[0].1;
            (eye, eye + (eye - capture.views[0].2))
        };
        *transform = Transform::from_translation(position + offset).looking_at(target + offset, Vec3::Y);
    }
}

fn setup(mut commands: Commands, mut inbox: ResMut<dogs::DogInbox>,
    mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>,
    capture: Res<Capture>, mut driver: ResMut<MotionDriver>, mut rat_state: ResMut<rats::StudioState>) {
    let (name, coat, build) = match std::env::var("ANIMAL_DOG").as_deref().unwrap_or("brindle") {
        "black" => ("Marrow", cathedral_sim::DogCoat::Black, 1.12),
        "grey" => ("Sedge", cathedral_sim::DogCoat::Grey, 1.15),
        "white" => ("Pip", cathedral_sim::DogCoat::White, 0.72),
        "fawn" => ("Warden", cathedral_sim::DogCoat::Fawn, 1.2),
        "pied" => ("Eel", cathedral_sim::DogCoat::Pied, 0.9),
        _ => ("Bracken", cathedral_sim::DogCoat::Brindle, 1.0),
    };
    if !capture.rat {
        let mut id = format!("dog_{}", name.to_lowercase());
        let mut yaw = 0.0;
        if capture.motion == "production" {
            let nav = cathedral_sim::NavData::from_parts(
                &std::fs::read_to_string("assets/world/navigation.json").unwrap(),
                &std::fs::read("assets/world/navigation.bin").unwrap(),
            ).unwrap();
            let mut pack = cathedral_sim::dogs::seed_pack(&nav);
            pack.retain(|dog| dog.name == name);
            assert_eq!(pack.len(), 1, "one authored dog selected for its real route");
            let dog = &pack[0];
            id = dog.id.as_str().to_owned();
            yaw = dog.facing_yaw as f32;
            let origin = dog.position_m;
            driver.production = Some(ProductionMotion { nav, pack, origin });
        }
        inbox.0.insert(id, dogs::DogSample {
        name: name.into(), coat, build, position: Vec3::new(0.0, 0.91, 0.0),
        facing_yaw: yaw, speed: 0.0, gait_phase: 0.0, seq: 1,
        });
    } else {
        if let Ok(path) = std::env::var("ANIMAL_RAT_TRACE") {
            let trace = RatTrace::load(&path);
            let (position, heading, pause_remaining) = trace.sample(0.0);
            driver.position = position;
            driver.yaw = (-heading.x).atan2(-heading.y);
            rat_state.reset_at(position.xz(), heading, pause_remaining);
            driver.rat_trace = Some(trace);
        }
        commands.spawn((StudioRat, bevy::light::NotShadowCaster, Mesh3d(meshes.add(rat_state.mesh())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE, perceptual_roughness: 1.0, reflectance: 0.02,
                cull_mode: None, double_sided: true, ..default()
            }))));
    }
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.31, 0.33, 0.34), perceptual_roughness: 0.95, ..default()
        })), Transform::from_xyz(0.0, if capture.rat { 0.012 } else { 0.0 }, 0.0)));
    if capture.travel {
        use bevy::{asset::RenderAssetUsages, mesh::{Indices, PrimitiveTopology}};
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for station in -40..=40 {
            for cross in [false, true] {
                let s = station as f32 * 0.5;
                let first = positions.len() as u32;
                for (x, z) in [(s-0.0015,-20.0),(s-0.0015,20.0),(s+0.0015,20.0),(s+0.0015,-20.0)] {
                    positions.push(if cross { [z,0.0004,x] } else { [x,0.0004,z] });
                }
                if cross { indices.extend([first,first+2,first+1,first,first+3,first+2]); }
                else { indices.extend([first,first+1,first+2,first,first+2,first+3]); }
            }
        }
        let count = positions.len();
        let grid = Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION,positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL,vec![[0.0,1.0,0.0];count])
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0,vec![[0.0,0.0];count])
            .with_inserted_indices(Indices::U32(indices));
        commands.spawn((Mesh3d(meshes.add(grid)),
            MeshMaterial3d(materials.add(StandardMaterial { base_color:Color::srgb(0.22,0.24,0.25),perceptual_roughness:1.0,..default() })),
            Transform::from_xyz(0.0,if capture.rat{0.012}else{0.0},0.0),bevy::light::NotShadowCaster));
    }
    commands.spawn((DirectionalLight { illuminance: 8500.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(-3.0, 6.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y)));
    let camera = commands.spawn((Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: 36.0_f32.to_radians(), ..default() }),
        Transform::from_translation(capture.views[0].1).looking_at(capture.views[0].2, Vec3::Y))).id();
    if capture.observer == "camera" {
        commands.entity(camera).insert(controller::PlayerCamera);
    } else {
        commands.spawn((StudioObserver, controller::PlayerCamera, Transform::IDENTITY));
    }
}

fn capture(mut commands: Commands, mut state: ResMut<Capture>, time: Res<Time>, driver: Res<MotionDriver>, rat_state: Res<rats::StudioState>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
    hierarchy: Query<(Entity, &Transform, Option<&ChildOf>, Option<&Name>), Without<Camera3d>>,
    mut exit: MessageWriter<AppExit>) {
    state.frames += 1;
    if state.awaiting {
        if !state.saved.load(Ordering::Acquire) { return; }
        state.view += 1;
        if state.view == state.views.len() {
            exit.write(AppExit::Success);
            return;
        }
        state.awaiting = false;
        state.frames = 0;
        **camera = Transform::from_translation(state.views[state.view].1)
            .looking_at(state.views[state.view].2, Vec3::Y);
    }
    let warmup = if state.view == 0 { 120.max(state.initial_steps) }
        else if state.animation { state.steps_per_frame } else { 24 };
    if state.frames < warmup { return; }
    let path = state.out.join(format!("{}.png", state.views[state.view].0));
    // Compose current local transforms here: GlobalTransform propagation runs
    // after Update, so reading it would record the previous simulation step.
    // Named contact points are evidence only; no capture code enters the game.
    let mut rig_points = Vec::new();
    for (entity, _, _, name) in &hierarchy {
        let Some(name) = name else { continue; };
        if !(name.as_str().starts_with("Dog paw ") || name.as_str().starts_with("Dog sole ")) { continue; }
        let mut matrix = Mat4::IDENTITY;
        let mut next = Some(entity);
        for _ in 0..24 {
            let Some(entity) = next else { break; };
            let Ok((_, transform, parent, _)) = hierarchy.get(entity) else { break; };
            matrix = transform.to_matrix() * matrix;
            next = parent.map(ChildOf::parent);
        }
        rig_points.push(format!("{:?}:{:?}", name.as_str(), matrix.transform_point3(Vec3::ZERO).to_array()));
        if let Some(leg) = name.as_str().strip_prefix("Dog paw ") {
            // Authored pad point documented by the rig: 29 mm below ankle,
            // one millimetre above the broad contact plane in the rest pose.
            let sole = matrix.transform_point3(Vec3::new(0.0, -0.029, 0.0));
            rig_points.push(format!("{:?}:{:?}", format!("Dog sole {leg}"), sole.to_array()));
        }
    }
    rig_points.sort();
    std::fs::write(path.with_extension("json"), format!(
        "{{\"time_seconds\":{},\"species\":\"{}\",\"moving\":{},\"travel\":{},\"input_speed_mps\":{},\"input_heading_yaw\":{},\"input_gait_phase\":{},\"camera\":{:?},\"target\":{:?},\"rig_points\":{{{}}},\"rat_pose\":{}}}\n",
        time.elapsed_secs_f64(), if state.rat { "rat" } else { "dog" }, driver.speed > 0.01,
        state.travel, driver.speed, driver.yaw, driver.phase, camera.translation.to_array(),
        (camera.translation + state.views[state.view].2 - state.views[state.view].1).to_array(),
        rig_points.join(","), if state.rat { rat_state.metadata() } else { "null".into() },
    )).unwrap();
    state.saved.store(false, Ordering::Release);
    let saved = state.saved.clone();
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path))
        .observe(move |_: On<ScreenshotCaptured>| { saved.store(true, Ordering::Release); });
    state.awaiting = true;
}
