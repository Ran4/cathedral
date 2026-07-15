//! Project-owned typography with deterministic cross-platform glyph coverage.

use bevy::prelude::*;

const BODY_FONT_PATH: &str = "fonts/EBGaramond12-Regular.otf";
const DISPLAY_FONT_PATH: &str = "fonts/EBGaramondSC12-Regular.otf";
const MONO_FONT_PATH: &str = "fonts/DejaVuSansMono.ttf";

/// The book face handles used throughout the HUD and smart-actor UI.
#[derive(Resource, Clone)]
pub struct CathedralFonts {
    body: Handle<Font>,
    display: Handle<Font>,
    mono: Handle<Font>,
}

impl CathedralFonts {
    pub fn body(&self) -> FontSource {
        self.body.clone().into()
    }

    pub fn display(&self) -> FontSource {
        self.display.clone().into()
    }

    /// A fixed-width face, for developer readouts whose columns and fill bars
    /// only line up when every glyph is the same width (the character debug
    /// sheet).
    pub fn mono(&self) -> FontSource {
        self.mono.clone().into()
    }
}

/// Loads bundled fonts instead of relying on Bevy's ASCII-only default face.
pub struct CathedralFontsPlugin;

impl Plugin for CathedralFontsPlugin {
    fn build(&self, app: &mut App) {
        let asset_server = app.world().resource::<AssetServer>();
        app.insert_resource(CathedralFonts {
            body: asset_server.load(BODY_FONT_PATH),
            display: asset_server.load(DISPLAY_FONT_PATH),
            mono: asset_server.load(MONO_FONT_PATH),
        });
    }
}
