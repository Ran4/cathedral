#![allow(dead_code)]
#[path = "/home/ran/src/rust/cathedralbevy/src/fonts.rs"]
mod fonts;
mod map {
    use bevy::prelude::*;
    #[derive(Resource, Default)]
    pub struct MapState {
        pub fullscreen_open: bool,
    }
}
mod smart_actors {
    pub mod chat {
        use bevy::prelude::*;
        #[derive(Resource, Default)]
        pub struct ChatInputState {
            pub open: bool,
        }
    }
    pub mod config_menu {
        use bevy::prelude::*;
        #[derive(Resource, Default)]
        pub struct ConfigMenuState {
            pub open: bool,
        }
    }
    pub mod inventory_ui {
        use bevy::prelude::*;
        #[derive(Resource, Default)]
        pub struct InventoryUiState {
            pub open: bool,
        }
    }
    pub mod hud {
        use bevy::prelude::*;
        pub const TEXT: Color = Color::WHITE;
        pub const MUTED: Color = Color::WHITE;
        pub const OFFLINE: Color = Color::WHITE;
        #[derive(Resource, Default)]
        pub struct SmartActorHudState;
        impl SmartActorHudState {
            pub fn set_journal_standing(&mut self, _: String) {}
        }
    }
    #[path = "/home/ran/src/rust/cathedralbevy/src/smart_actors/journal_ui.rs"]
    mod journal_ui;

    pub fn run(rows: usize, small: bool) {
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
        use bevy::input::mouse::AccumulatedMouseScroll;
        use bevy::prelude::*;
        use bevy::window::WindowPlugin;
        let scale = if small { 1.1666666 } else { 1.0 };
        let physical_size = if small {
            UVec2::new(1120, 630)
        } else {
            UVec2::new(1280, 720)
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin { file_path: "/home/ran/src/rust/cathedralbevy/assets".into(), ..default() },
            TransformPlugin,
            bevy::input::InputPlugin,
            bevy::input_focus::InputFocusPlugin,
            bevy::picking::DefaultPickingPlugins,
            WindowPlugin { primary_window: None, ..default() },
        ))
        .init_asset::<Image>()
        .init_asset::<TextureAtlasLayout>()
        .add_plugins((bevy::text::TextPlugin, bevy::ui::UiPlugin, crate::fonts::CathedralFontsPlugin))
        .insert_resource(journal_ui::JournalUiState { open: false })
        .insert_resource(journal_ui::PlayerJournal {
            entries: (0..rows).map(|i| journal_ui::JournalRow {
                attribution: format!("A stranger, at the Salt Cellars, today — at one remove — and {} others since, in 2 wards:", i + 1),
                word: "“A salt trader of the Weigh Ward (you don't know their name) gave short measure at the cellars yesterday.”".into(),
            }).collect(),
            standing: Vec::new(),
        })
        .init_resource::<AccumulatedMouseScroll>()
        .add_systems(Startup, journal_ui::spawn_journal_ui)
        .add_systems(Update, (
            journal_ui::refresh_journal_ui,
            journal_ui::scroll_journal,
            journal_ui::update_journal_ui,
        ).chain());
        app.world_mut().spawn((
            Camera2d,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size,
                        scale_factor: scale,
                    }),
                    ..default()
                },
                viewport: Some(Viewport {
                    physical_size,
                    ..default()
                }),
                ..default()
            },
        ));
        app.finish();
        app.cleanup();
        // Mimic opening after assets and the UI have settled, as in the drive.
        for _ in 0..60 {
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let font_assets = app.world().resource::<Assets<Font>>();
        let project_fonts = app.world().resource::<crate::fonts::CathedralFonts>();
        for source in [project_fonts.body(), project_fonts.display()] {
            let FontSource::Handle(handle) = source else {
                panic!("bundled font handle")
            };
            let font = font_assets
                .get(&handle)
                .expect("the bundled font really loaded");
            println!("font alias={} bytes={}", font.alias, font.data.len());
        }
        app.world_mut()
            .resource_mut::<journal_ui::JournalUiState>()
            .open = true;
        for frame in 0..120 {
            eprintln!("rows={rows} frame={frame} begin");
            let start = std::time::Instant::now();
            app.update();
            eprintln!("rows={rows} frame={frame} done {:?}", start.elapsed());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let world = app.world_mut();
        let mut names = world.query::<(&Name, &ComputedNode)>();
        for (name, node) in names.iter(world) {
            if name.as_str().starts_with("Journal") {
                println!(
                    "rows={rows} {name}: size={:?}, content={:?}, scroll={:?}",
                    node.size, node.content_size, node.scroll_position
                );
            }
        }
        let mut texts = world.query::<(&Text, &ComputedNode)>();
        assert!(
            texts.iter(world).all(|(_, node)| node.size.y > 0.0),
            "fonts must be measured, not silently missing"
        );
        let mut entries =
            world.query_filtered::<&ComputedNode, With<journal_ui::JournalEntriesRoot>>();
        let entries = entries.single(world).unwrap();
        assert!(entries.size.y > 0.0 && entries.size.y < 720.0);
        let overflow =
            (entries.content_size.y - entries.size.y).max(0.0) * entries.inverse_scale_factor;
        if rows == 24 {
            assert!(overflow > 0.0);
        }
        // Wheel input through InputPlugin, then scroll against actual layout.
        app.world_mut()
            .write_message(bevy::input::mouse::MouseWheel {
                unit: bevy::input::mouse::MouseScrollUnit::Pixel,
                x: 0.0,
                y: -100_000.0,
                window: Entity::PLACEHOLDER,
                phase: bevy::input::touch::TouchPhase::Moved,
            });
        app.update();
        let world = app.world_mut();
        let mut positions = world.query_filtered::<(&ComputedNode, &ScrollPosition), With<journal_ui::JournalEntriesRoot>>();
        let (node, position) = positions.single(world).unwrap();
        println!(
            "rows={rows} max scroll={overflow}, position={:?}, computed={:?}",
            position.0, node.scroll_position
        );
        assert!(
            (position.y - overflow).abs() < 1.0,
            "actual scroll reaches bottom"
        );
        for _ in 0..10 {
            app.world_mut()
                .resource_mut::<journal_ui::JournalUiState>()
                .open = false;
            app.update();
            app.world_mut()
                .resource_mut::<journal_ui::JournalUiState>()
                .open = true;
            app.update();
        }
        let images = app.world().resource::<Assets<Image>>();
        for (_, image) in images.iter() {
            println!("atlas dimensions={:?}", image.texture_descriptor.size);
            assert!(image.texture_descriptor.size.width <= 8192);
            assert!(image.texture_descriptor.size.height <= 8192);
        }
        println!("PASS rows={rows} small={small}");
    }
}
fn main() {
    let rows = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "24".into())
        .parse()
        .unwrap();
    smart_actors::run(rows, std::env::args().nth(2).as_deref() == Some("small"));
}
