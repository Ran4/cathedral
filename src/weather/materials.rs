//! Shared wet-material response. The city contributes a small registry of
//! semantic handles once; rain never clones a material per building.

use bevy::prelude::*;

use super::SmoothedWeather;

#[derive(Debug, Clone, Copy)]
pub(crate) struct WetResponse {
    pub darken: f32,
    pub smooth_to: f32,
}

impl WetResponse {
    pub const GROUND: Self = Self {
        darken: 0.31,
        smooth_to: 0.34,
    };
    pub const PAVING: Self = Self {
        darken: 0.24,
        smooth_to: 0.28,
    };
    pub const ROOF: Self = Self {
        darken: 0.27,
        smooth_to: 0.25,
    };
    pub const TIMBER: Self = Self {
        darken: 0.16,
        smooth_to: 0.62,
    };
    pub const CANVAS: Self = Self {
        darken: 0.14,
        smooth_to: 0.72,
    };
}

#[derive(Debug, Clone)]
struct ReactiveMaterial {
    handle: Handle<StandardMaterial>,
    dry_color: Color,
    dry_roughness: f32,
    response: WetResponse,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct WeatherReactiveMaterials {
    entries: Vec<ReactiveMaterial>,
    last_wetness: Option<f32>,
    next_update_at: f64,
}

impl WeatherReactiveMaterials {
    pub(crate) fn capture(
        assets: &Assets<StandardMaterial>,
        handles: impl IntoIterator<Item = (Handle<StandardMaterial>, WetResponse)>,
    ) -> Self {
        let entries = handles
            .into_iter()
            .filter_map(|(handle, response)| {
                let material = assets.get(&handle)?;
                Some(ReactiveMaterial {
                    handle,
                    dry_color: material.base_color,
                    dry_roughness: material.perceptual_roughness,
                    response,
                })
            })
            .collect();
        Self {
            entries,
            ..default()
        }
    }
}

pub(super) fn update_wet_materials(
    time: Res<Time>,
    weather: Res<SmoothedWeather>,
    registry: Option<ResMut<WeatherReactiveMaterials>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut registry), Some(mut materials)) = (registry, materials) else {
        return;
    };
    // The visual smoothing approaches zero asymptotically. Snap the final
    // sub-byte step so a fully dry city restores the exact authored material
    // values instead of retaining an invisible floating-point residue forever.
    let wetness = displayed_wetness(weather.surface_wetness);
    let changed = registry
        .last_wetness
        .is_none_or(|last| (last - wetness).abs() >= 1.0 / 255.0);
    // Touch the shared city materials only when wetness truly moved, and at
    // most 10×/s: every `get_mut` marks the material Modified, which
    // re-prepares it and re-bins every batched mesh bound to it. (An earlier
    // `&&` here made the steady state fall through every 100 ms forever — the
    // single largest stutter source the 2026-07 profiling night found.)
    if !changed || time.elapsed_secs_f64() < registry.next_update_at {
        return;
    }
    registry.last_wetness = Some(wetness);
    registry.next_update_at = time.elapsed_secs_f64() + 0.1;

    for entry in &registry.entries {
        let Some(mut material) = materials.get_mut(&entry.handle) else {
            continue;
        };
        material.base_color = wet_color(entry.dry_color, entry.response.darken, wetness);
        material.perceptual_roughness =
            entry.dry_roughness + (entry.response.smooth_to - entry.dry_roughness) * wetness;
    }
}

fn displayed_wetness(wetness: f32) -> f32 {
    if wetness <= 1.0 / 255.0 {
        0.0
    } else {
        wetness.clamp(0.0, 1.0)
    }
}

fn wet_color(dry: Color, darken: f32, wetness: f32) -> Color {
    if wetness <= 0.0 {
        return dry;
    }
    let dry = dry.to_srgba();
    let multiplier = 1.0 - darken * wetness.clamp(0.0, 1.0);
    Color::srgba(
        dry.red * multiplier,
        dry.green * multiplier,
        dry.blue * multiplier,
        dry.alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_wetness_returns_the_exact_dry_color() {
        let dry = Color::srgb(0.72, 0.70, 0.66);
        assert_eq!(wet_color(dry, 0.31, 0.0), dry);
        assert_eq!(displayed_wetness(1.0 / 512.0), 0.0);
    }
}
