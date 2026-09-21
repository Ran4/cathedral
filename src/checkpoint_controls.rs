//! Non-pausing checkpoint controls. F6 requests quick-save, F9 quick-load.
//! These are refusal-only until complete application admission/publication is
//! implemented. A key must never create a partial save or retire live control.
use bevy::prelude::*;
use cathedral_sim::{
    RuntimeGeneration,
    checkpoint::{self, CheckpointBudget, Reservation},
};
use std::sync::Arc;

use crate::{
    installed_recipe::CommittedStartup,
    smart_actors::{
        bridge::{BridgeHandle, ControlRoute},
        hud::SmartActorHudState,
        local_engine::LocalEngine,
    },
};

const CONTROL_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckpointAction {
    QuickSave,
    QuickLoad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlRefusal {
    NoCommittedStartup,
    NoActiveWorld,
    StaleGeneration,
    WorldOwnerMismatch,
    IncompleteAdmission,
    UnavailableApplicationPath,
    RequestIdsExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ControlReceipt {
    pub request_id: u64,
    pub action: CheckpointAction,
    pub generation: Option<RuntimeGeneration>,
    pub refusal: ControlRefusal,
}

/// One fixed terminal result, no retained requests, payloads, candidates or
/// deadline queue. This owner lives with the App rather than a world generation.
/// A future asynchronous path must add acknowledged completion/cancellation;
/// it cannot reinterpret these immediate refusals as submitted operations.
#[derive(Resource)]
pub(crate) struct CheckpointControls {
    next_request: Option<u64>,
    pub(crate) last: Option<ControlReceipt>,
    // Covers this fixed owner only, not the App or its pre-existing HUD. It
    // follows these fields through drop, independently of the mutable world.
    _lease: Reservation,
}

impl CheckpointControls {
    pub(crate) fn admitted(budget: &Arc<CheckpointBudget>) -> checkpoint::Result<Self> {
        let lease = budget.reserve_running_overhead(CONTROL_BYTES)?;
        Ok(Self {
            next_request: Some(1),
            last: None,
            _lease: lease,
        })
    }

    /// Refusals borrow every application owner. There is no rollback work:
    /// no input fence, world mutation, capture, storage submission or service
    /// activation occurs. Retired authority can never be reopened here.
    pub(crate) fn request(
        &mut self,
        action: CheckpointAction,
        generation: Option<RuntimeGeneration>,
        handle: Option<&BridgeHandle>,
        engine: Option<&LocalEngine>,
        installed: Option<&CommittedStartup>,
    ) -> Result<ControlReceipt, ControlRefusal> {
        let request_id = self
            .next_request
            .ok_or(ControlRefusal::RequestIdsExhausted)?;
        self.next_request = request_id.checked_add(1);
        let refusal = Self::gate(generation, handle, engine, installed);
        let receipt = ControlReceipt {
            request_id,
            action,
            generation,
            refusal,
        };
        self.last = Some(receipt);
        Ok(receipt)
    }

    fn gate(
        generation: Option<RuntimeGeneration>,
        handle: Option<&BridgeHandle>,
        engine: Option<&LocalEngine>,
        installed: Option<&CommittedStartup>,
    ) -> ControlRefusal {
        let Some(installed) = installed else {
            return ControlRefusal::NoCommittedStartup;
        };
        let Some(handle) = handle.filter(|h| h.control_route() == ControlRoute::Active) else {
            return ControlRefusal::NoActiveWorld;
        };
        if generation != Some(handle.generation()) {
            return ControlRefusal::StaleGeneration;
        }
        let Some(engine) = engine else {
            return ControlRefusal::NoActiveWorld;
        };
        if !engine.owns_control(handle, installed) {
            return ControlRefusal::WorldOwnerMismatch;
        }
        if installed.require_complete_admission().is_err() {
            return ControlRefusal::IncompleteAdmission;
        }
        // Keep publication explicitly closed even if admission is implemented
        // first. Complete atomic host adoption is a separate necessary proof.
        ControlRefusal::UnavailableApplicationPath
    }
}

pub(crate) fn keyboard_controls(
    keys: Res<ButtonInput<KeyCode>>,
    controls: Option<ResMut<CheckpointControls>>,
    handle: Option<Res<BridgeHandle>>,
    engine: Option<NonSend<LocalEngine>>,
    installed: Option<Res<CommittedStartup>>,
    mut hud: ResMut<SmartActorHudState>,
) {
    let mut controls = controls;
    // Fixed order when both keys arrive together, at most two constant-work
    // attempts per frame. Holding a key does not repeatedly submit requests.
    for (key, action) in [
        (KeyCode::F6, CheckpointAction::QuickSave),
        (KeyCode::F9, CheckpointAction::QuickLoad),
    ] {
        if !keys.just_pressed(key) {
            continue;
        }
        let result =
            controls
                .as_deref_mut()
                .map_or(Err(ControlRefusal::NoCommittedStartup), |controls| {
                    controls.request(
                        action,
                        handle.as_deref().map(BridgeHandle::generation),
                        handle.as_deref(),
                        engine.as_deref(),
                        installed.as_deref(),
                    )
                });
        let reason = result.map_or_else(|reason| reason, |receipt| receipt.refusal);
        let message = match (action, reason) {
            (_, ControlRefusal::RequestIdsExhausted) => {
                "Checkpoint requests exhausted; restart the game."
            }
            (CheckpointAction::QuickSave, _) => {
                "Save unavailable in this build. Your play continues."
            }
            (CheckpointAction::QuickLoad, _) => {
                "Load unavailable in this build. Your play continues."
            }
        };
        hud.toast(message);
    }
}

#[cfg(test)]
mod tests;
