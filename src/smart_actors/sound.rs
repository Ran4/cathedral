//! Positional one-shot playback for authoritative `sound` events.
//!
//! Assets resolve by convention from the sound id alone —
//! `assets/sounds/{sound_id}.mp3` — so no filename ever crosses the wire. A
//! missing or unplayable asset skips playback only; the percept toast (owned
//! by the bridge drain) is never silenced by it.

use bevy::{
    audio::{AudioPlayer, AudioSource, PlaybackSettings, SpatialScale},
    prelude::*,
};

/// A validated sound event the player heard (or made). Presentation only.
#[derive(Message, Debug, Clone)]
pub struct PlaySoundEffect {
    pub sound_id: String,
    pub position: Vec3,
    pub audible_distance: f32,
}

/// One live one-shot. `PlaybackSettings::DESPAWN` retires it after playback;
/// the timestamp is the fallback for assets that never start (missing file).
#[derive(Component, Debug)]
pub struct SoundEffect {
    spawned_at: f64,
}

/// A one-shot whose asset never produced a playing sink by this age is dead
/// (typically a missing mp3, which `DESPAWN` alone would leak forever).
const STALLED_EFFECT_TIMEOUT_SECONDS: f64 = 30.0;

/// Rodio applies inverse-square attenuation outside the scaled unit sphere.
/// Anchoring that sphere to a fixed fraction of each sound's audible radius
/// makes every catalog row fade over its own range: full volume close in,
/// roughly -18 dB at the audible edge, whether that edge is 20 m or 600 m.
const UNATTENUATED_FRACTION_OF_RADIUS: f32 = 1.0 / 8.0;

/// Catalog ids double as asset basenames; never build a path from anything
/// looser than this.
pub(super) fn valid_sound_id(sound_id: &str) -> bool {
    !sound_id.is_empty()
        && sound_id.len() <= 64
        && sound_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
}

pub(super) fn play_sound_effects(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
    mut effects: MessageReader<PlaySoundEffect>,
) {
    for effect in effects.read() {
        if !valid_sound_id(&effect.sound_id) {
            continue;
        }
        let radius = effect.audible_distance.clamp(1.0, 10_000.0);
        let source: Handle<AudioSource> =
            asset_server.load(format!("sounds/{}.mp3", effect.sound_id));
        commands.spawn((
            Name::new(format!("Sound effect: {}", effect.sound_id)),
            SoundEffect {
                spawned_at: time.elapsed_secs_f64(),
            },
            AudioPlayer::new(source),
            PlaybackSettings::DESPAWN
                .with_spatial(true)
                .with_spatial_scale(SpatialScale::new(
                    1.0 / (radius * UNATTENUATED_FRACTION_OF_RADIUS),
                )),
            Transform::from_translation(effect.position),
        ));
    }
}

pub(super) fn expire_stalled_sound_effects(
    mut commands: Commands,
    time: Res<Time>,
    effects: Query<(Entity, &SoundEffect)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, effect) in &effects {
        if now - effect.spawned_at > STALLED_EFFECT_TIMEOUT_SECONDS {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_ids_are_locked_to_asset_safe_names() {
        assert!(valid_sound_id("fart"));
        assert!(valid_sound_id("glass_break"));
        for bad in ["", "Fart", "../fart", "fart.mp3", "fart fart", "fart-1"] {
            assert!(!valid_sound_id(bad), "{bad}");
        }
    }

    #[test]
    fn every_catalog_radius_maps_to_a_positive_spatial_scale() {
        for radius in [20.0_f32, 25.0, 600.0] {
            let scale = 1.0 / (radius * UNATTENUATED_FRACTION_OF_RADIUS);
            assert!(scale.is_finite() && scale > 0.0);
        }
    }
}
