//! Developer visualization of the baked navigation graph (F7).
//!
//! Draws the street graph the sim walks on — every welded road edge, the leaf
//! nodes for the 69 named places and ~2,560 building doors — straight from the
//! committed `navigation.json` / `navigation.bin`. Like the area overlay it is a
//! view of authoritative data, not a second coordinate set: the same [`NavData`]
//! the movement layer will route on.
//!
//! Verify it with the drive harness:
//! ```sh
//! CATHEDRAL_DRIVE='sleep 2; key F7; shot navgraph; quit' cargo run
//! ```

use bevy::prelude::*;
use cathedral_sim::{NavData, WALK_Y};

const NAV_JSON: &str = include_str!("../assets/world/navigation.json");
const NAV_BIN: &[u8] = include_bytes!("../assets/world/navigation.bin");

/// The loaded navigation graph, available to the whole game (the overlay now,
/// the mover in M2). Absent if the committed artifact failed to parse.
#[derive(Resource)]
pub struct Navigation(pub NavData);

#[derive(Resource, Default)]
struct NavOverlay {
    enabled: bool,
}

/// Loads the navigation graph and draws it on F7.
pub struct NavDebugPlugin;

impl Plugin for NavDebugPlugin {
    fn build(&self, app: &mut App) {
        match NavData::from_parts(NAV_JSON, NAV_BIN) {
            Ok(nav) => {
                app.insert_resource(Navigation(nav));
            }
            Err(error) => {
                error!("navigation graph did not load, F7 overlay disabled: {error}");
                return;
            }
        }
        app.init_resource::<NavOverlay>()
            .add_systems(Update, (toggle_overlay, draw_navigation_graph).chain());
    }
}

fn toggle_overlay(keyboard: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<NavOverlay>) {
    if keyboard.just_pressed(KeyCode::F7) {
        overlay.enabled = !overlay.enabled;
        info!("navigation overlay {}", if overlay.enabled { "on" } else { "off" });
    }
}

fn draw_navigation_graph(
    overlay: Res<NavOverlay>,
    navigation: Option<Res<Navigation>>,
    mut gizmos: Gizmos,
) {
    if !overlay.enabled {
        return;
    }
    let Some(navigation) = navigation else {
        return;
    };
    let nav = &navigation.0;
    // Just above the walk plane so the lines read against the cobbles.
    let y = WALK_Y as f32 + 0.05;
    let edge_color = Color::srgb(0.25, 0.75, 1.0);
    let place_color = Color::srgb(1.0, 0.85, 0.25);
    let door_color = Color::srgb(1.0, 0.45, 0.30);

    let world = |xz: [f64; 2], height: f32| Vec3::new(xz[0] as f32, height, xz[1] as f32);

    for (a, edges) in nav.adjacency().iter().enumerate() {
        let pa = nav.node_xz(a);
        for edge in edges {
            if edge.to <= a {
                continue; // each undirected edge once
            }
            gizmos.line(world(pa, y), world(nav.node_xz(edge.to), y), edge_color);
        }
    }
    // A door is a short red tick; a named place a taller amber post.
    for door in nav.doors() {
        let p = nav.node_xz(door.node);
        gizmos.line(world(p, y), world(p, y + 0.8), door_color);
    }
    for place in nav.places() {
        let p = nav.node_xz(place.node);
        gizmos.line(world(p, y), world(p, y + 4.0), place_color);
    }
}
