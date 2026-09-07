//! Two tiny shared pigment maps: short hair and broken brindle streaks.
//! No fur geometry or custom shader; these are ordinary mipmapped albedo.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

fn noise(x: i32, y: i32) -> f32 {
    let mut n = (x.rem_euclid(128) as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y.rem_euclid(128) as u32).wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    ((n ^ (n >> 16)) & 65535) as f32 / 65535.0
}

// Tileable value noise, with independently hashed cells. Unlike a bent sine
// wave, this has no repeated sequence of equal-width painted stripes.
fn field(u: f32, v: f32, nx: i32, ny: i32) -> f32 {
    let x = u * nx as f32;
    let y = v * ny as f32;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let fx = fx * fx * (3.0 - 2.0 * fx);
    let fy = fy * fy * (3.0 - 2.0 * fy);
    let sample = |dx: i32, dy: i32| noise((ix + dx).rem_euclid(nx), (iy + dy).rem_euclid(ny));
    let a = sample(0, 0) * (1.0 - fx) + sample(1, 0) * fx;
    let b = sample(0, 1) * (1.0 - fx) + sample(1, 1) * fx;
    a * (1.0 - fy) + b * fy
}

pub(super) fn image(brindle: bool) -> Image {
    const SIZE: usize = 128;
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / SIZE as f32;
            let v = y as f32 / SIZE as f32;
            let fraction = (y % 4) as f32 / 4.0;
            let fiber = noise(x as i32, (y / 4) as i32) * (1.0 - fraction)
                + noise(x as i32, (y / 4 + 1) as i32) * fraction;
            let fine = noise(x as i32, y as i32);
            let mut value = 0.84 + fiber * 0.13 + fine * 0.03;
            if brindle {
                let warp = field(u, v, 4, 5) - 0.5;
                let streak = field(u + warp * 0.075, v + warp * 0.055, 7, 19);
                let breakup = field(u, v, 17, 29);
                let dark = ((streak * 0.8 + breakup * 0.2 - 0.30) / 0.55).clamp(0.0, 1.0);
                value *= 1.0 - dark * 0.34;
            }
            let byte = (value * 255.0) as u8;
            rgba.extend([byte, byte, byte, 255]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut size = SIZE;
    let mut all_mips = rgba.clone();
    while size > 1 {
        let next = size / 2;
        let mut mip = Vec::with_capacity(next * next * 4);
        for y in 0..next {
            for x in 0..next {
                for channel in 0..4 {
                    let sum: u32 = [
                        (x * 2, y * 2),
                        (x * 2 + 1, y * 2),
                        (x * 2, y * 2 + 1),
                        (x * 2 + 1, y * 2 + 1),
                    ]
                    .iter()
                    .map(|(sx, sy)| rgba[(sy * size + sx) * 4 + channel] as u32)
                    .sum();
                    mip.push((sum / 4) as u8);
                }
            }
        }
        all_mips.extend_from_slice(&mip);
        rgba = mip;
        size = next;
    }
    image.data = Some(all_mips);
    image.texture_descriptor.mip_level_count = 8;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}
