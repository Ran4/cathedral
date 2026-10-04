//! The quicksave key and the drive `save` action (D06). Both only ask: the
//! engine pump takes the checkpoint after its next poll and a worker thread
//! writes it (`local_engine.rs`, `cathedral_backends::save`).

use bevy::prelude::*;
use cathedral_backends::save::HostSave;

use super::{
    SmartActorRuntime,
    bridge::{BridgeCommand, BridgeHandle},
    hud::SmartActorHudState,
};
use crate::controller::PlayerController;

/// The slot F6 writes.
pub const QUICKSAVE_SLOT: &str = "quick";

/// Save the city into a named slot — the drive `save <slot>` action.
#[derive(Message, Debug, Clone)]
pub struct SaveRequest {
    pub slot: String,
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
