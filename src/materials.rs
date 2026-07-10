//! Shared material helpers for world-scale, repeating surfaces.

use bevy::{
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
};

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
