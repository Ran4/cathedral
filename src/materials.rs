//! Shared material helpers for world-scale, repeating surfaces.

use bevy::{
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

/// The city's window panes: a StandardMaterial glass whose alpha fades with
/// camera distance (see `assets/shaders/window_glass.wgsl`) — see-through up
/// close, the plain opaque pane at range. The batched window meshes span whole
/// map tiles, so the fade must be per-fragment; no per-entity swap could do it.
pub type WindowGlassMaterial = ExtendedMaterial<StandardMaterial, WindowGlassExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WindowGlassExtension {
    /// x: metres where the pane is at its clearest, y: metres where it is
    /// fully opaque again, z: the point-blank alpha, w: padding.
    #[uniform(100)]
    pub fade: Vec4,
}

impl MaterialExtension for WindowGlassExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/window_glass.wgsl".into()
    }
}

/// Physical width and depth represented by one full floor texture tile.
pub const FLOOR_TEXTURE_SPAN_METERS: f32 = 6.0;

/// Loads a color texture with smooth, anisotropic filtering and wrapping on
/// both axes. Large world surfaces provide UVs outside 0..1 so the artwork
/// repeats at a believable physical scale instead of stretching once.
pub fn load_repeating_texture(asset_server: &AssetServer, path: &'static str) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            let mut sampler = ImageSamplerDescriptor::linear();
            sampler
                .set_address_mode(ImageAddressMode::Repeat)
                .set_anisotropic_filter(8);
            settings.sampler = ImageSampler::Descriptor(sampler);
        })
        .load(path)
}
