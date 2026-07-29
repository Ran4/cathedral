use std::fs;

use bevy::{
    input::keyboard::Key,
    log::{error, info, warn},
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};

use crate::session_log;

pub struct CathedralScreenshotPlugin;

/// Allocates the `__nn` filename suffix that keeps multiple captures within
/// the same wall-clock second from overwriting each other.
#[derive(Debug, Default, Resource)]
struct ScreenshotNamer {
    last_stamp: String,
    next_index: u32,
}

impl ScreenshotNamer {
    fn file_name(&mut self, stamp: String) -> String {
        if stamp != self.last_stamp {
            self.last_stamp = stamp;
            self.next_index = 0;
        }
        let index = self.next_index;
        self.next_index += 1;
        format!("cathedral_screenshot_{}__{index:02}.png", self.last_stamp)
    }
}

impl Plugin for CathedralScreenshotPlugin {
    fn build(&self, app: &mut App) {
        match session_log::paths() {
            Some(session) => info!(
                "Cathedral session {}: logging to {}",
                session.number,
                session.root.display()
            ),
            None => warn!("No session directory; screenshots are disabled"),
        }
        app.init_resource::<ScreenshotNamer>()
            .add_systems(Update, capture_screenshot_on_key);
    }
}

fn capture_screenshot_on_key(
    mut commands: Commands,
    physical_keys: Res<ButtonInput<KeyCode>>,
    logical_keys: Res<ButtonInput<Key>>,
    mut namer: ResMut<ScreenshotNamer>,
) {
    if !screenshot_key_just_pressed(&physical_keys, &logical_keys) {
        return;
    }

    let Some(session) = session_log::paths() else {
        warn!("No session directory; screenshot skipped");
        return;
    };
    if let Err(error) = fs::create_dir_all(&session.screenshots) {
        error!(
            "Could not create screenshot directory {}: {error}",
            session.screenshots.display()
        );
        return;
    }

    let path = session
        .screenshots
        .join(namer.file_name(session_log::current_timestamp().file_stamp()));
    info!("Capturing screenshot to {}", path.display());
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

/// Visible to the crate so the chat box can prove, against the real binding,
/// that a typed acute accent no longer reaches it.
pub(crate) fn screenshot_key_just_pressed(
    physical_keys: &ButtonInput<KeyCode>,
    logical_keys: &ButtonInput<Key>,
) -> bool {
    physical_keys.just_pressed(KeyCode::F5)
        // The acute-accent key is physically `Equal` on a Swedish keyboard.
        || physical_keys.just_pressed(KeyCode::Equal)
        // Accept the US grave-key position too, for layouts that report it there.
        || physical_keys.just_pressed(KeyCode::Backquote)
        || logical_keys.just_pressed(Key::Dead(Some('\u{b4}')))
        || logical_keys.just_pressed(Key::Dead(Some('\u{301}')))
        || logical_keys.just_pressed(Key::Character("\u{b4}".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f5_and_swedish_acute_key_request_a_capture() {
        let mut f5_keys = ButtonInput::<KeyCode>::default();
        f5_keys.press(KeyCode::F5);
        assert!(screenshot_key_just_pressed(
            &f5_keys,
            &ButtonInput::<Key>::default()
        ));

        let mut acute_keys = ButtonInput::<Key>::default();
        acute_keys.press(Key::Dead(Some('\u{b4}')));
        assert!(screenshot_key_just_pressed(
            &ButtonInput::<KeyCode>::default(),
            &acute_keys
        ));

        assert!(!screenshot_key_just_pressed(
            &ButtonInput::<KeyCode>::default(),
            &ButtonInput::<Key>::default()
        ));
    }

    #[test]
    fn same_second_captures_get_increasing_suffixes() {
        let mut namer = ScreenshotNamer::default();

        assert_eq!(
            namer.file_name("2026-07-13_09_54_31".into()),
            "cathedral_screenshot_2026-07-13_09_54_31__00.png"
        );
        assert_eq!(
            namer.file_name("2026-07-13_09_54_31".into()),
            "cathedral_screenshot_2026-07-13_09_54_31__01.png"
        );
        assert_eq!(
            namer.file_name("2026-07-13_09_54_32".into()),
            "cathedral_screenshot_2026-07-13_09_54_32__00.png"
        );
    }
}
