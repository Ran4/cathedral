//! The quicksave and quickload keys, the drive `save` and `load` actions and
//! the Esc menu's slot list (D06). All of them only ask: the engine pump takes
//! a checkpoint after its next poll, or swaps in a slot the save thread has
//! decoded, and a worker thread does the disk work (`local_engine.rs`,
//! `cathedral_backends::save`).

use std::path::PathBuf;

use bevy::prelude::*;
use cathedral_backends::save::HostSave;

use super::{
    SmartActorRuntime,
    bridge::{BridgeCommand, BridgeHandle},
    hud::SmartActorHudState,
};
use crate::controller::PlayerController;

/// The slot F6 writes and F9 reads.
pub const QUICKSAVE_SLOT: &str = "quick";

/// Save the city into a named slot — the drive `save <slot>` action.
#[derive(Message, Debug, Clone)]
pub struct SaveRequest {
    pub slot: String,
}

/// Replace the running city with a named slot — the drive `load <slot>`
/// action and the Esc menu's load buttons.
#[derive(Message, Debug, Clone)]
pub struct LoadRequest {
    pub slot: String,
}

/// The world the game was projecting has just been replaced by a loaded one.
/// Written by the bridge drain, before the new world's first messages; every
/// presentation that remembers the previous frame (an interpolation sweep, a
/// bell's office edge, a queued stroke) forgets the old world on it.
#[derive(Message, Debug, Clone, Copy)]
pub struct WorldLoaded;

/// Where the slots live, for the Esc menu's list.
#[derive(Resource, Debug, Clone)]
pub struct SaveSlots {
    pub dir: PathBuf,
}

/// F6 (F5 is the screenshot key) and every [`SaveRequest`]. The player's
/// pitch and flight travel with the slot; everything else is the engine's.
pub fn request_saves(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut requests: MessageReader<SaveRequest>,
    runtime: Res<SmartActorRuntime>,
    handle: Res<BridgeHandle>,
    controller: Single<&PlayerController>,
    mut hud: ResMut<SmartActorHudState>,
) {
    let mut slots: Vec<String> = requests.read().map(|request| request.slot.clone()).collect();
    if keyboard.just_pressed(KeyCode::F6) {
        slots.push(QUICKSAVE_SLOT.to_string());
    }
    for slot in slots {
        if !runtime.interactions_enabled() {
            hud.toast("Nothing to save until the city is running");
            continue;
        }
        let host = HostSave {
            pitch: controller.pitch(),
            flying: controller.flying,
        };
        if let Err(error) = handle.try_send(BridgeCommand::Save { slot, host }) {
            hud.toast(error);
        }
    }
}

/// F9 and every [`LoadRequest`]. The city keeps running while the slot is
/// read; the swap, or the reason there was none, arrives through the drain.
pub fn request_loads(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut requests: MessageReader<LoadRequest>,
    runtime: Res<SmartActorRuntime>,
    handle: Res<BridgeHandle>,
    mut hud: ResMut<SmartActorHudState>,
) {
    let mut slots: Vec<String> = requests.read().map(|request| request.slot.clone()).collect();
    if keyboard.just_pressed(KeyCode::F9) {
        slots.push(QUICKSAVE_SLOT.to_string());
    }
    for slot in slots {
        if !runtime.interactions_enabled() {
            hud.toast("Nothing to load into until the city is running");
            continue;
        }
        if let Err(error) = handle.try_send(BridgeCommand::Load { slot }) {
            hud.toast(error);
        }
    }
}
