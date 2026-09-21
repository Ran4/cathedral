use super::*;
use crate::installed_recipe::{InstalledRecipe, StagedStartup};
use bevy::{
    asset::RenderAssetUsages,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

fn pixels() -> Image {
    Image::new_fill(
        Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[128, 64, 32, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    )
}

#[test]
fn timeout_never_releases_a_live_capture_or_turns_late_write_into_success() {
    let installed = InstalledRecipe::new().unwrap();
    let budget = installed.budget().clone();
    let baseline = budget.retained_bytes();
    let requests = Requests::admitted(&budget).unwrap();
    let start = Instant::now();
    let flight = requests.begin(start).unwrap();
    let receipt = flight.receipt.clone();
    assert!(matches!(requests.begin(start), Err(Refusal::Busy)));
    assert_eq!(receipt.at(start + TIMEOUT), Outcome::TimedOut);
    assert!(matches!(
        requests.begin(start + TIMEOUT),
        Err(Refusal::Busy)
    ));
    receipt.finish(Outcome::Saved);
    assert_eq!(receipt.outcome(), Outcome::TimedOut);
    drop(flight);
    let next = requests.begin(Instant::now()).unwrap();
    assert_eq!(next.receipt.outcome(), Outcome::Pending);
    assert_eq!(receipt.outcome(), Outcome::TimedOut);
    drop(next);
    drop(requests);
    assert_eq!(
        budget.retained_bytes(),
        baseline + REQUEST_BYTES,
        "receipt still owns its metadata allowance"
    );
    drop(receipt);
    assert_eq!(budget.retained_bytes(), baseline);
    drop(installed);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn dimensions_and_cpu_image_layout_are_checked_before_clone() {
    for dims in [
        (0, 1),
        (1, 0),
        (4097, 1),
        (4096, 4096),
        (u32::MAX, u32::MAX),
    ] {
        assert_eq!(check_extent(dims.0, dims.1), Err(Refusal::Extent));
    }
    let mut image = pixels();
    assert!(valid_image(&image));
    image.texture_descriptor.size.width = u32::MAX;
    let (valid, allocated) =
        crate::host_checkpoint::measure_installed_allocations(|| valid_image(&image));
    assert!(!valid);
    assert_eq!(allocated, 0);
    image = pixels();
    image.data.as_mut().unwrap().pop();
    assert!(!valid_image(&image));
    image = pixels();
    image.texture_descriptor.format = TextureFormat::Rgba32Float;
    assert!(!valid_image(&image));
}

#[test]
fn pixel_handoff_does_not_clone_owned_render_metadata() {
    let mut source = pixels();
    source.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        label: Some("render-only".repeat(100_000)),
        ..Default::default()
    });
    let (copy, allocated) =
        crate::host_checkpoint::measure_installed_allocations(|| copy_pixels(&source).unwrap());
    assert_eq!(copy.data, source.data);
    assert!(matches!(copy.sampler, bevy::image::ImageSampler::Default));
    assert!(
        allocated <= 64,
        "only the 16 pixel bytes are copied: {allocated}"
    );
    source.texture_descriptor.size.width = u32::MAX;
    let (rejected, allocated) =
        crate::host_checkpoint::measure_installed_allocations(|| copy_pixels(&source));
    assert!(rejected.is_none());
    assert_eq!(allocated, 0);
}

#[test]
fn actual_observer_reports_real_png_completion_and_write_failure() {
    AsyncComputeTaskPool::get_or_init(|| {
        bevy::tasks::TaskPoolBuilder::new().num_threads(1).build()
    });
    let installed = InstalledRecipe::new().unwrap();
    let requests = Requests::admitted(installed.budget()).unwrap();
    let directory = std::env::temp_dir().join(format!("alibi-m3d-images-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    for (name, expected) in [
        ("saved.png", Outcome::Saved),
        ("absent/fail.png", Outcome::WriteFailed),
    ] {
        let path = directory.join(name);
        let flight = requests.begin(Instant::now()).unwrap();
        let receipt = flight.receipt.clone();
        let mut app = App::new();
        let entity = app
            .world_mut()
            .spawn_empty()
            .observe(observer(path.clone(), flight))
            .id();
        assert_eq!(receipt.outcome(), Outcome::Pending);
        assert!(!path.exists());
        app.world_mut().trigger(ScreenshotCaptured {
            entity,
            image: pixels(),
        });
        let stop = Instant::now() + Duration::from_secs(3);
        while matches!(receipt.outcome(), Outcome::Pending | Outcome::Encoding)
            && Instant::now() < stop
        {
            std::thread::yield_now();
        }
        assert_eq!(receipt.outcome(), expected);
        if expected == Outcome::Saved {
            assert_eq!(&fs::read(&path).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
        } else {
            assert!(!path.exists());
        }
        // The task may publish immediately before its final Flight drop.
        while requests.slot.busy.load(Ordering::Acquire) && Instant::now() < stop {
            std::thread::yield_now();
        }
        assert!(!requests.slot.busy.load(Ordering::Acquire));
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cancelled_or_late_observer_never_copies_an_image_or_claims_success() {
    let installed = InstalledRecipe::new().unwrap();
    let requests = Requests::admitted(installed.budget()).unwrap();
    let now = Instant::now();
    let flight = requests.begin(now).unwrap();
    let receipt = flight.receipt.clone();
    assert_eq!(receipt.at(now + TIMEOUT), Outcome::TimedOut);
    let mut app = App::new();
    let entity = app
        .world_mut()
        .spawn_empty()
        .observe(observer("/tmp/must-not-be-written-m3d.png".into(), flight))
        .id();
    app.world_mut().trigger(ScreenshotCaptured {
        entity,
        image: pixels(),
    });
    assert_eq!(receipt.outcome(), Outcome::TimedOut);
    assert!(!requests.slot.busy.load(Ordering::Acquire));
}

#[test]
fn installed_context_without_renderer_refuses_before_allocating_capture() {
    let installed = InstalledRecipe::new().unwrap();
    let mut config = installed
        .load_config_from_paths("/nonexistent/alibi-m3d-config", "default_config.ron")
        .unwrap();
    config.smart_actors.enabled = false;
    config.smart_actors.fake_backend = true;
    config.smart_actors.tts_backend = "off".into();
    let staged = StagedStartup::prepare_with(installed, config, |_| {
        cathedral_backends::BackendsConfig::resolve(
            &cathedral_backends::Environment::from_map(Default::default()),
            &cathedral_backends::BackendsOptions {
                fake_mode: true,
                dotenv_path: None,
                workers_dir: "/nonexistent/workers".into(),
                ..Default::default()
            },
        )
    })
    .unwrap();
    let budget = staged.recipe().budget().clone();
    let mut app = App::new();
    assert!(staged.install(&mut app).is_ok());
    app.insert_resource(FrameEnvironment {
        headless: true,
        audio_disabled: true,
    });
    app.world_mut().spawn((
        PrimaryWindow,
        Window {
            visible: false,
            ..Default::default()
        },
    ));
    let retained = budget.retained_bytes();
    app.add_systems(Update, |context: CaptureContext, mut commands: Commands| {
        assert_eq!(context.checkpoint_acceptance(), Err(Refusal::Renderer));
        assert!(matches!(
            context.submit(&mut commands, "/tmp/never-m3d.png".into()),
            Err(Refusal::Renderer)
        ));
    });
    app.update();
    assert_eq!(budget.retained_bytes(), retained);
    assert!(
        !app.world()
            .resource::<Requests>()
            .slot
            .busy
            .load(Ordering::Acquire)
    );
}
