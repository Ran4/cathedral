//! Snapshot-driven inventory selection and player interaction intents.

use std::collections::HashMap;

use bevy::{
    input::mouse::AccumulatedMouseScroll,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::controller::PlayerController;

use super::{
    ActorFocus, ITEM_INTERACTION_RADIUS_M, POSITION_UPDATE_HZ, SmartActorRuntime,
    SmartActorsConfig,
    bridge::{BridgeCommand, BridgeHandle, TranscriptionBackend},
    hud::SmartActorHudState,
    microphone::{MicrophoneCommand, MicrophoneEvent, MicrophonePoll, MicrophoneService},
    model::{ActorId, ItemId, WorldMirror},
    speech::StopNpcSpeech,
};

#[derive(Message, Debug, Clone, PartialEq)]
pub enum PlayerIntent {
    SpatialUpdate {
        spatial_seq: u64,
        position: Vec3,
        facing_yaw: f32,
    },
    /// Fire-and-forget deliberate noise (the F key); no request_id because
    /// there is no failure the player can act on.
    Sound {
        sound_id: String,
    },
    Recording {
        request_id: String,
        wav_basename: String,
        stt_backend: TranscriptionBackend,
        spatial_seq: u64,
        position: Vec3,
    },
    Offer {
        request_id: String,
        target_id: ActorId,
        item_id: ItemId,
        /// Units of a stack to offer — `None` offers the whole stack. Only the
        /// coin purse sets it, via the count picker (05_the_llm_seam.md §7).
        quantity: Option<u32>,
        spatial_seq: u64,
        position: Vec3,
    },
    Accept {
        request_id: String,
        item_id: ItemId,
        spatial_seq: u64,
        position: Vec3,
    },
    Decline {
        request_id: String,
        item_id: ItemId,
        spatial_seq: u64,
        position: Vec3,
    },
    Retract {
        request_id: String,
        item_id: ItemId,
    },
    // ------------------------------------------------ the body pockets
    // (`features/extra_pockets.md`). Everything you do to your own body needs
    // nobody else nearby, so only `Spit` carries a position.
    /// Tuck one unit of a held stack into a body cavity.
    Pocket {
        request_id: String,
        item_id: ItemId,
        slot: cathedral_sim::BodySlot,
    },
    /// Take a pocketed unit back into the open; the slot is derivable sim-side.
    Retrieve {
        request_id: String,
        item_id: ItemId,
    },
    Swallow {
        request_id: String,
        item_id: ItemId,
    },
    /// Spitting is aimed at somebody within `ITEM_INTERACTION_RADIUS_M`, so it
    /// carries a position exactly like an offer does.
    Spit {
        request_id: String,
        item_id: ItemId,
        target_id: ActorId,
        spatial_seq: u64,
        position: Vec3,
    },
    Gargle {
        request_id: String,
        item_id: ItemId,
    },
    /// Void the lower slots where you stand; names no item — the gut decides.
    Expel {
        request_id: String,
    },
    Eat {
        request_id: String,
        item_id: ItemId,
    },
    DebugSay {
        request_id: String,
        text: String,
        target_id: Option<ActorId>,
        spatial_seq: u64,
        position: Vec3,
    },
    /// Typed chat (the Enter box): a real `say` in any mode, unlike `DebugSay`.
    Say {
        request_id: String,
        text: String,
        spatial_seq: u64,
        position: Vec3,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PendingKind {
    Recording,
    Offer { item_id: ItemId, target_id: ActorId, quantity: Option<u32> },
    Accept { item_id: ItemId },
    Decline { item_id: ItemId },
    Retract { item_id: ItemId },
    /// Every body-pocket verb that names an item (`features/extra_pockets.md`).
    /// They all move the same thing — a stack-unit between the open and a
    /// cavity — so one kind covers the lot; the label is only for the toast.
    BodySlot { item_id: ItemId },
    /// `expel` names no item: whatever is down there comes out.
    Expel,
    DebugSay,
    Say,
}

impl PendingKind {
    fn item_id(&self) -> Option<&ItemId> {
        match self {
            Self::Offer { item_id, .. }
            | Self::Accept { item_id }
            | Self::Decline { item_id }
            | Self::Retract { item_id }
            | Self::BodySlot { item_id } => Some(item_id),
            Self::Recording | Self::Expel | Self::DebugSay | Self::Say => None,
        }
    }

    fn expects_snapshot(&self) -> bool {
        !matches!(self, Self::Recording | Self::DebugSay | Self::Say)
    }
}

#[derive(Debug, Clone)]
struct PendingCommand {
    kind: PendingKind,
    sent_revision: u64,
    succeeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveOfferCard {
    pub item_id: ItemId,
    pub giver_id: ActorId,
    pub created_seq: u64,
    pub broadcast: bool,
    pub text: String,
    pub additional_count: usize,
}

#[derive(Resource, Debug, Default)]
pub struct InteractionState {
    pub selected_item: Option<ItemId>,
    selected_index: usize,
    pub active_offer: Option<ActiveOfferCard>,
    /// How many coins the purse's offer picker hands over
    /// (`features/food_and_items/05_the_llm_seam.md` §7). Meaningful only while
    /// the coin stack is selected; clamped to the live stack, and a raw 0 reads
    /// as 1 so a fresh state offers a single coin, never the whole purse.
    coin_offer_count: u32,
    dismissed_broadcasts: HashMap<ItemId, u64>,
    pending: HashMap<String, PendingCommand>,
    next_request: u64,
}

impl InteractionState {
    /// The effective coin-offer count for a stack of `stack` units: at least
    /// one, never more than the stack.
    fn coin_count(&self, stack: u32) -> u32 {
        self.coin_offer_count.clamp(1, stack.max(1))
    }

    pub(super) fn request_id(&mut self) -> String {
        self.next_request = self.next_request.wrapping_add(1).max(1);
        format!("rust-{}", self.next_request)
    }

    pub(super) fn insert_pending(&mut self, request_id: String, kind: PendingKind, revision: u64) {
        self.pending.insert(
            request_id,
            PendingCommand {
                kind,
                sent_revision: revision,
                succeeded: false,
            },
        );
    }

    pub fn resolve_command(
        &mut self,
        request_id: &str,
        success: bool,
        current_revision: Option<u64>,
    ) -> bool {
        let Some(pending) = self.pending.get_mut(request_id) else {
            return false;
        };
        if !success {
            self.pending.remove(request_id);
            return true;
        }
        pending.succeeded = true;
        if !pending.kind.expects_snapshot()
            || current_revision.is_some_and(|revision| revision > pending.sent_revision)
        {
            self.pending.remove(request_id);
        }
        true
    }

    pub fn reconcile_revision(&mut self, revision: u64) {
        self.pending.retain(|_, pending| {
            !(pending.succeeded
                && pending.kind.expects_snapshot()
                && revision > pending.sent_revision)
        });
    }

    pub fn clear_pending(&mut self) {
        self.pending.clear();
        self.active_offer = None;
    }

    fn item_is_pending(&self, item_id: &ItemId) -> bool {
        self.pending
            .values()
            .any(|pending| pending.kind.item_id() == Some(item_id))
    }

    /// Whether an identical offer — same item, same target, **same quantity** —
    /// is already in flight, so a repeated right-click is idempotent. Quantity is
    /// part of the identity: re-offering the coin purse at a *different* count is
    /// a genuinely new offer the sim will replace, so it must not be deduped
    /// (05 §7 — otherwise the player would have to press R before changing it).
    fn identical_offer_pending(
        &self,
        item_id: &ItemId,
        target_id: &ActorId,
        quantity: Option<u32>,
    ) -> bool {
        self.pending.values().any(|pending| {
            matches!(
                &pending.kind,
                PendingKind::Offer {
                    item_id: pending_item,
                    target_id: pending_target,
                    quantity: pending_quantity,
                } if pending_item == item_id
                    && pending_target == target_id
                    && *pending_quantity == quantity
            )
        })
    }
}

#[derive(Resource, Debug)]
pub struct PlayerSpatialState {
    pub sequence: u64,
    last_position: Option<Vec3>,
    last_yaw: Option<f32>,
    last_background_send: f64,
}

impl Default for PlayerSpatialState {
    fn default() -> Self {
        Self {
            sequence: 0,
            last_position: None,
            last_yaw: None,
            last_background_send: f64::NEG_INFINITY,
        }
    }
}

impl PlayerSpatialState {
    pub fn position_for_action(&mut self, position: Vec3) -> u64 {
        if self.last_position != Some(position) {
            self.sequence = self.sequence.saturating_add(1);
            self.last_position = Some(position);
        }
        self.sequence
    }

    /// Sequence for a background `spatial_update`, which carries facing too.
    /// Turning in place must take a fresh sequence: the engine treats an
    /// equal sequence as an idempotent repeat and would drop the new yaw.
    pub fn spatial_update_needed(&self, position: Vec3, facing_yaw: f32) -> bool {
        self.last_position != Some(position) || self.last_yaw != Some(facing_yaw)
    }

    pub fn mark_spatial_update(&mut self, position: Vec3, facing_yaw: f32) -> u64 {
        if self.spatial_update_needed(position, facing_yaw) {
            self.sequence = self.sequence.saturating_add(1);
            self.last_position = Some(position);
            self.last_yaw = Some(facing_yaw);
        }
        self.sequence
    }

    pub fn mark_hello_position(&mut self, position: Vec3) -> u64 {
        self.last_position = Some(position);
        self.sequence
    }

    pub fn retry_latest_position(&mut self) {
        self.last_position = None;
        self.last_yaw = None;
    }
}

#[derive(Debug, Clone)]
struct RecordingContext {
    wav_basename: String,
    stt_backend: TranscriptionBackend,
}

/// Persistent, voice-activated microphone state.
///
/// `enabled` is the player's preference and defaults on; `worker_enabled`
/// tracks the command sent to the capture worker. Transcription never gates
/// rearming.
#[derive(Resource, Debug)]
pub struct MicrophoneInputState {
    pub enabled: bool,
    pub stt_backend: TranscriptionBackend,
    worker_enabled: bool,
    recording: Option<RecordingContext>,
    /// Last streaming gate acknowledged by the capture worker. `None` forces
    /// a resend, covering worker (re)spawn after connect/disconnect.
    streaming_sync: Option<bool>,
}

impl Default for MicrophoneInputState {
    fn default() -> Self {
        Self {
            enabled: true,
            stt_backend: TranscriptionBackend::Cloud,
            worker_enabled: false,
            recording: None,
            streaming_sync: None,
        }
    }
}

impl MicrophoneInputState {
    pub fn with_backend(stt_backend: TranscriptionBackend) -> Self {
        Self {
            stt_backend,
            ..Self::default()
        }
    }

    /// Whether the capture worker is recording an utterance right now.
    ///
    /// This is intentionally narrower than `enabled`: voice activation can be
    /// armed indefinitely without constituting dialogue audio activity.
    pub(crate) fn recording_active(&self) -> bool {
        self.recording.is_some()
    }

    pub fn clear_on_disconnect(&mut self) {
        self.recording = None;
        self.streaming_sync = None;
    }

    #[cfg(test)]
    fn is_enabled(&self) -> bool {
        self.enabled && self.worker_enabled
    }
}

/// Cloud streaming starts only when every layer agrees: config.ron has not
/// disabled it, the player has the cloud backend selected, and the engine
/// reported cloud transcription available.
pub(crate) fn effective_streaming(
    config: &SmartActorsConfig,
    state: &MicrophoneInputState,
    runtime: &SmartActorRuntime,
) -> bool {
    config.stt_streaming
        && state.stt_backend == TranscriptionBackend::Cloud
        && runtime.stt_cloud_available
}

pub fn sync_player_position(
    time: Res<Time>,
    runtime: Res<SmartActorRuntime>,
    players: Query<(&GlobalTransform, &PlayerController)>,
    mut spatial: ResMut<PlayerSpatialState>,
    mut intents: MessageWriter<PlayerIntent>,
) {
    if !runtime.interactions_enabled() {
        return;
    }
    let Ok((player, controller)) = players.single() else {
        return;
    };
    let position = player.translation();
    let facing_yaw = controller.yaw();
    let now = time.elapsed_secs_f64();
    if !spatial.spatial_update_needed(position, facing_yaw)
        || now - spatial.last_background_send < f64::from(POSITION_UPDATE_HZ.recip())
    {
        return;
    }
    let spatial_seq = spatial.mark_spatial_update(position, facing_yaw);
    spatial.last_background_send = now;
    intents.write(PlayerIntent::SpatialUpdate {
        spatial_seq,
        position,
        facing_yaw,
    });
}

pub fn reconcile_interaction_state(
    mirror: Res<WorldMirror>,
    runtime: Res<SmartActorRuntime>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    focus: Res<ActorFocus>,
    mut state: ResMut<InteractionState>,
    mut hud: ResMut<SmartActorHudState>,
) {
    if let Some(revision) = runtime.mirror_revision {
        state.reconcile_revision(revision);
    }
    let player_id = ActorId("player".into());
    let holds = mirror
        .actor(&player_id)
        .map_or(&[][..], |actor| actor.holds.as_slice());
    normalize_selection(&mut state, holds);
    state.dismissed_broadcasts.retain(|item_id, sequence| {
        mirror
            .offers()
            .any(|offer| &offer.item_id == item_id && offer.created_seq == *sequence)
    });

    // Keep the coin-offer count within the live purse; `Some` only while the
    // coin stack is the current selection, which is where the HUD shows it.
    let coin_stack = selected_coin_stack(&mirror, state.selected_item.as_ref());
    if let Some(stack) = coin_stack {
        state.coin_offer_count = state.coin_count(stack);
    }
    let coin_count = coin_stack.map(|stack| state.coin_count(stack));

    hud.inventory = inventory_text(&mirror, holds, state.selected_item.as_ref(), coin_count);

    let player_position = players
        .single()
        .map(GlobalTransform::translation)
        .unwrap_or(Vec3::ZERO);
    let cards = if runtime.interactions_enabled() {
        actionable_offer_cards(
            &mirror,
            player_position,
            &player_id,
            &state.dismissed_broadcasts,
        )
    } else {
        Vec::new()
    };
    state.active_offer = cards.first().cloned().map(|mut card| {
        card.additional_count = cards.len().saturating_sub(1);
        card
    });
    hud.offer_card = state
        .active_offer
        .as_ref()
        .map_or_else(String::new, |card| {
            let more = if card.additional_count == 0 {
                String::new()
            } else {
                format!("    +{} more", card.additional_count)
            };
            let controls = if card.broadcast {
                "[Y] Accept    [N] Dismiss"
            } else {
                "[Y] Accept    [N] Decline"
            };
            let waiting = if state.item_is_pending(&card.item_id) {
                "\nWaiting for the actor engine…"
            } else {
                ""
            };
            format!("{}{}\n{}{}", card.text, more, controls, waiting)
        });

    hud.focus_hint = if runtime.interactions_enabled() {
        focus_hint(&mirror, &focus, state.selected_item.as_ref(), coin_count)
    } else {
        String::new()
    };
}

pub fn select_inventory_item(
    keyboard: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    mirror: Res<WorldMirror>,
    inventory: Option<Res<super::inventory_ui::InventoryUiState>>,
    mut state: ResMut<InteractionState>,
) {
    // The `I` screen owns the pointer while it is open: its wheel scrolls the
    // panel and its digits are not quickbar keys.
    if inventory.is_some_and(|inventory| inventory.open) {
        return;
    }
    let player_id = ActorId("player".into());
    let Some(player) = mirror.actor(&player_id) else {
        return;
    };
    let holds = &player.holds;
    if holds.is_empty() {
        state.selected_item = None;
        state.selected_index = 0;
        return;
    }

    let number_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    if let Some(index) = number_keys
        .iter()
        .position(|key| keyboard.just_pressed(*key))
        .filter(|index| *index < holds.len())
    {
        state.selected_index = index;
    } else if scroll.delta.y.abs() > f32::EPSILON {
        if scroll.delta.y > 0.0 {
            state.selected_index = state
                .selected_index
                .checked_sub(1)
                .unwrap_or(holds.len() - 1);
        } else {
            state.selected_index = (state.selected_index + 1) % holds.len();
        }
    }
    state.selected_item = holds.get(state.selected_index).cloned();
}

#[allow(clippy::too_many_arguments)]
pub fn collect_item_interaction_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    runtime: Res<SmartActorRuntime>,
    mirror: Res<WorldMirror>,
    focus: Res<ActorFocus>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    mut spatial: ResMut<PlayerSpatialState>,
    mut state: ResMut<InteractionState>,
    mut hud: ResMut<SmartActorHudState>,
    mut intents: MessageWriter<PlayerIntent>,
) {
    if !runtime.interactions_enabled()
        || cursor
            .single()
            .map_or(true, |cursor| cursor.grab_mode == CursorGrabMode::None)
    {
        return;
    }
    let Ok(player) = players.single() else { return };
    let position = player.translation();
    let revision = runtime.mirror_revision.unwrap_or(0);

    // The coin purse's count picker (05 §7): while the spark stack is selected,
    // `[` and `]` dial how many coins a right-click will offer, within the stack.
    // (`Equal` is the screenshot key; the brackets stay clear of every existing
    // binding.) No other item has a picker — they offer whole-stack in v1.
    if let Some(stack) = selected_coin_stack(&mirror, state.selected_item.as_ref()) {
        let down = keyboard.just_pressed(KeyCode::BracketLeft)
            || keyboard.just_pressed(KeyCode::NumpadSubtract);
        let up = keyboard.just_pressed(KeyCode::BracketRight)
            || keyboard.just_pressed(KeyCode::NumpadAdd);
        if down || up {
            let current = state.coin_count(stack);
            state.coin_offer_count = if up {
                (current + 1).min(stack)
            } else {
                current.saturating_sub(1).max(1)
            };
        }
    }

    if mouse.just_pressed(MouseButton::Right) {
        let (Some(item_id), Some(target)) = (state.selected_item.clone(), focus.item.as_ref())
        else {
            return;
        };
        let player_id = ActorId("player".into());
        let player_holds_item = mirror
            .actor(&player_id)
            .is_some_and(|actor| actor.holds.contains(&item_id));
        // The coin purse hands over the picked count; every other item is whole.
        // Quantity is decided before the dedup guard so re-offering the purse at a
        // different count is not mistaken for a repeat of the one in flight.
        let quantity = selected_coin_stack(&mirror, Some(&item_id)).map(|stack| state.coin_count(stack));
        if !player_holds_item
            || target.actor_id == player_id
            || target.body_distance_m > ITEM_INTERACTION_RADIUS_M
            || state.identical_offer_pending(&item_id, &target.actor_id, quantity)
        {
            return;
        }
        let request_id = state.request_id();
        let target_name = mirror
            .actor(&target.actor_id)
            .map_or(target.actor_id.0.as_str(), |actor| {
                actor.name_for_player.as_str()
            });
        let offered = mirror.item(&item_id).map_or_else(
            || item_id.0.clone(),
            |item| match quantity {
                Some(count) if count > 1 => format!("{count} {}", item.display_plural),
                Some(count) => format!("{count} {}", item.name),
                None => item.name.clone(),
            },
        );
        hud.toast(format!("Offering {offered} to {target_name}…"));
        state.insert_pending(
            request_id.clone(),
            PendingKind::Offer {
                item_id: item_id.clone(),
                target_id: target.actor_id.clone(),
                quantity,
            },
            revision,
        );
        let spatial_seq = spatial.position_for_action(position);
        intents.write(PlayerIntent::Offer {
            request_id,
            target_id: target.actor_id.clone(),
            item_id,
            quantity,
            spatial_seq,
            position,
        });
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        let Some(item_id) = state.selected_item.clone() else {
            return;
        };
        let is_players_offer = mirror
            .offers()
            .any(|offer| offer.item_id == item_id && offer.giver_id.0 == "player");
        if is_players_offer && !state.item_is_pending(&item_id) {
            let request_id = state.request_id();
            state.insert_pending(
                request_id.clone(),
                PendingKind::Retract {
                    item_id: item_id.clone(),
                },
                revision,
            );
            intents.write(PlayerIntent::Retract {
                request_id,
                item_id,
            });
        }
    }

    if keyboard.just_pressed(KeyCode::KeyY) || keyboard.just_pressed(KeyCode::KeyN) {
        let Some(card) = state.active_offer.clone() else {
            return;
        };
        if state.item_is_pending(&card.item_id) {
            return;
        }
        if keyboard.just_pressed(KeyCode::KeyN) && card.broadcast {
            state
                .dismissed_broadcasts
                .insert(card.item_id, card.created_seq);
            return;
        }
        let request_id = state.request_id();
        let spatial_seq = spatial.position_for_action(position);
        if keyboard.just_pressed(KeyCode::KeyY) {
            state.insert_pending(
                request_id.clone(),
                PendingKind::Accept {
                    item_id: card.item_id.clone(),
                },
                revision,
            );
            intents.write(PlayerIntent::Accept {
                request_id,
                item_id: card.item_id,
                spatial_seq,
                position,
            });
        } else {
            state.insert_pending(
                request_id.clone(),
                PendingKind::Decline {
                    item_id: card.item_id.clone(),
                },
                revision,
            );
            intents.write(PlayerIntent::Decline {
                request_id,
                item_id: card.item_id,
                spatial_seq,
                position,
            });
        }
    }
}

/// F emits the catalog fart at the player's position. Deliberately the whole
/// non-speech sound loop in one key: perception, attribution, playback, and
/// the rate limit are all exercised by mashing it (features/sounds.md).
pub fn collect_sound_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    config: Res<SmartActorsConfig>,
    runtime: Res<SmartActorRuntime>,
    mut intents: MessageWriter<PlayerIntent>,
) {
    if !config.sounds.enabled
        || !keyboard.just_pressed(KeyCode::KeyF)
        || !runtime.interactions_enabled()
        || cursor
            .single()
            .map_or(true, |cursor| cursor.grab_mode == CursorGrabMode::None)
    {
        return;
    }
    intents.write(PlayerIntent::Sound {
        sound_id: "fart".into(),
    });
}

#[allow(clippy::too_many_arguments)]
pub fn update_microphone_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<SmartActorsConfig>,
    runtime: Res<SmartActorRuntime>,
    microphone: Option<Res<MicrophoneService>>,
    mut state: ResMut<MicrophoneInputState>,
    mut hud: ResMut<SmartActorHudState>,
) {
    if keyboard.just_pressed(KeyCode::KeyZ) {
        state.stt_backend = match state.stt_backend {
            TranscriptionBackend::Cloud => TranscriptionBackend::Local,
            TranscriptionBackend::Local => TranscriptionBackend::Cloud,
        };
        hud.toast(match state.stt_backend {
            TranscriptionBackend::Cloud => "Transcription: cloud model",
            TranscriptionBackend::Local => "Transcription: local Canary-Qwen FP16",
        });
    }
    hud.transcription_backend = match state.stt_backend {
        TranscriptionBackend::Cloud => "CLOUD".into(),
        TranscriptionBackend::Local => "CANARY-QWEN FP16".into(),
    };

    if keyboard.just_pressed(KeyCode::KeyV) {
        state.enabled = !state.enabled;
        if state.enabled {
            if microphone.is_none() && !hud.microphone_unavailable {
                hud.microphone_available = false;
            }
        } else if !hud.microphone_unavailable {
            // Keep an explicit MIC OFF visible even if V was pressed before
            // capability probing or the engine handshake completed.
            hud.microphone_available = true;
        }
        hud.toast(if state.enabled {
            "Microphone on"
        } else {
            "Microphone off"
        });
    }
    hud.microphone_enabled = state.enabled;

    let should_enable = state.enabled && runtime.interactions_enabled() && runtime.stt_available;
    let Some(microphone) = microphone else {
        state.worker_enabled = false;
        state.streaming_sync = None;
        hud.listening = false;
        return;
    };
    // Sync the streaming gate ahead of Enable so a freshly armed worker
    // already knows the mode before its first utterance can trigger.
    let streaming = effective_streaming(&config, &state, &runtime);
    if state.streaming_sync != Some(streaming) {
        if state.streaming_sync.is_none() && !streaming {
            // A fresh worker already starts with streaming off.
            state.streaming_sync = Some(false);
        } else if microphone
            .try_send(MicrophoneCommand::SetStreaming { enabled: streaming })
            .is_ok()
        {
            state.streaming_sync = Some(streaming);
        }
    }
    if should_enable != state.worker_enabled {
        let command = if should_enable {
            MicrophoneCommand::Enable
        } else {
            MicrophoneCommand::Disable
        };
        match microphone.try_send(command) {
            Ok(()) => state.worker_enabled = should_enable,
            Err(error) => hud.toast(error),
        }
    }
    hud.listening = state.worker_enabled && should_enable;
}

#[allow(clippy::too_many_arguments)]
pub fn poll_microphone(
    microphone: Option<Res<MicrophoneService>>,
    handle: Res<BridgeHandle>,
    runtime: Res<SmartActorRuntime>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    mut spatial: ResMut<PlayerSpatialState>,
    mut microphone_input: ResMut<MicrophoneInputState>,
    mut interaction: ResMut<InteractionState>,
    mut hud: ResMut<SmartActorHudState>,
    mut intents: MessageWriter<PlayerIntent>,
    mut stop_speech: MessageWriter<StopNpcSpeech>,
) {
    let Some(microphone) = microphone else { return };
    let mut recording_started_this_poll = false;
    loop {
        let event = match microphone.poll() {
            MicrophonePoll::Event(event) => event,
            MicrophonePoll::Empty => break,
            MicrophonePoll::Disconnected => {
                microphone_input.enabled = false;
                microphone_input.worker_enabled = false;
                microphone_input.recording = None;
                hud.microphone_available = false;
                hud.listening = false;
                if !hud.microphone_unavailable {
                    hud.microphone_unavailable = true;
                    hud.connection_detail = "Microphone worker is unavailable".into();
                    hud.toast("Microphone worker stopped");
                }
                break;
            }
        };
        match event {
            MicrophoneEvent::Available => {
                hud.microphone_available = true;
                hud.microphone_unavailable = false;
                if hud.connection_detail.starts_with("Microphone unavailable:") {
                    hud.connection_detail = "Microphone ready; speak openly".into();
                }
            }
            MicrophoneEvent::Unavailable(message) => {
                microphone_input.enabled = false;
                microphone_input.worker_enabled = false;
                microphone_input.recording = None;
                hud.connection_detail = format!(
                    "Microphone unavailable: {} · [V] retry",
                    message.chars().take(220).collect::<String>()
                );
                hud.toast(message);
                hud.microphone_available = false;
                hud.microphone_unavailable = true;
                hud.listening = false;
            }
            MicrophoneEvent::RecordingStarted { wav_basename } => {
                microphone_input.recording = Some(RecordingContext {
                    wav_basename,
                    stt_backend: microphone_input.stt_backend,
                });
                recording_started_this_poll = true;
            }
            MicrophoneEvent::RecordingFinished {
                wav_basename,
                silent,
                ..
            } => {
                let context_matches = microphone_input
                    .recording
                    .as_ref()
                    .is_some_and(|context| context.wav_basename == wav_basename);
                let stt_backend = microphone_input
                    .recording
                    .as_ref()
                    .filter(|context| context.wav_basename == wav_basename)
                    .map_or(microphone_input.stt_backend, |context| context.stt_backend);
                if context_matches {
                    microphone_input.recording = None;
                }
                if silent {
                    continue;
                }
                if !context_matches {
                    // Discarded after finishing: also release any streamed
                    // copy so the engine never holds an ownerless transcript.
                    let _ = handle.try_send(BridgeCommand::PlayerAudioAbort {
                        wav_basename: wav_basename.clone(),
                    });
                    if let Err(error) = microphone.discard_recording(wav_basename) {
                        hud.toast(error);
                    }
                    continue;
                }
                if !runtime.interactions_enabled() {
                    // An offline engine is a hard command barrier: there is
                    // nobody to hear this utterance and nowhere to queue it.
                    let _ = handle.try_send(BridgeCommand::PlayerAudioAbort {
                        wav_basename: wav_basename.clone(),
                    });
                    if let Err(error) = microphone.discard_recording(wav_basename) {
                        hud.toast(error);
                    }
                    continue;
                }
                let Ok(player) = players.single() else {
                    let _ = handle.try_send(BridgeCommand::PlayerAudioAbort {
                        wav_basename: wav_basename.clone(),
                    });
                    if let Err(error) = microphone.discard_recording(wav_basename) {
                        hud.toast(error);
                    }
                    continue;
                };
                let position = player.translation();
                let spatial_seq = spatial.position_for_action(position);
                let request_id = interaction.request_id();
                interaction.insert_pending(
                    request_id.clone(),
                    PendingKind::Recording,
                    runtime.mirror_revision.unwrap_or(0),
                );
                intents.write(PlayerIntent::Recording {
                    request_id,
                    wav_basename,
                    stt_backend,
                    spatial_seq,
                    position,
                });
            }
            MicrophoneEvent::RecordingCancelled { wav_basename } => {
                if microphone_input
                    .recording
                    .as_ref()
                    .is_some_and(|context| context.wav_basename == wav_basename)
                {
                    microphone_input.recording = None;
                }
            }
            MicrophoneEvent::RecordingFailed(message) => {
                microphone_input.recording = None;
                microphone_input.enabled = false;
                microphone_input.worker_enabled = false;
                hud.microphone_available = false;
                hud.microphone_unavailable = true;
                hud.listening = false;
                hud.toast(message);
            }
        }
    }
    // The worker can report RecordingStarted and RecordingCancelled together
    // when an NPC stream suspends a fresh VAD candidate. Only interrupt NPC
    // audio if a recording is still genuinely active after draining the
    // complete event batch.
    if should_interrupt_npc_speech(
        recording_started_this_poll,
        microphone_input.recording.is_some(),
    ) {
        stop_speech.write(StopNpcSpeech);
    }
}

fn should_interrupt_npc_speech(started_this_poll: bool, recording_still_active: bool) -> bool {
    started_this_poll && recording_still_active
}

pub fn inject_debug_say(
    text: String,
    target_id: Option<ActorId>,
    position: Vec3,
    runtime: &SmartActorRuntime,
    spatial: &mut PlayerSpatialState,
    interaction: &mut InteractionState,
) -> Option<PlayerIntent> {
    if !runtime.interactions_enabled()
        || !runtime.fake_backend
        || text.trim().is_empty()
        || text.chars().count() > super::PLAYER_SPEECH_MAX_CHARS
    {
        return None;
    }
    let request_id = interaction.request_id();
    interaction.insert_pending(
        request_id.clone(),
        PendingKind::DebugSay,
        runtime.mirror_revision.unwrap_or(0),
    );
    Some(PlayerIntent::DebugSay {
        request_id,
        text: text.trim().to_string(),
        target_id,
        spatial_seq: spatial.position_for_action(position),
        position,
    })
}

/// Typed chat (the Enter box). Unlike [`inject_debug_say`] this is a real
/// player action in any mode; only the engine being offline refuses it.
pub fn prepare_player_say(
    text: &str,
    position: Vec3,
    runtime: &SmartActorRuntime,
    spatial: &mut PlayerSpatialState,
    interaction: &mut InteractionState,
) -> Option<PlayerIntent> {
    let text = text.trim();
    if !runtime.interactions_enabled()
        || text.is_empty()
        || text.chars().count() > super::PLAYER_SPEECH_MAX_CHARS
    {
        return None;
    }
    let request_id = interaction.request_id();
    interaction.insert_pending(
        request_id.clone(),
        PendingKind::Say,
        runtime.mirror_revision.unwrap_or(0),
    );
    Some(PlayerIntent::Say {
        request_id,
        text: text.to_string(),
        spatial_seq: spatial.position_for_action(position),
        position,
    })
}

fn normalize_selection(state: &mut InteractionState, holds: &[ItemId]) {
    if holds.is_empty() {
        state.selected_item = None;
        state.selected_index = 0;
        return;
    }
    if let Some(index) = state
        .selected_item
        .as_ref()
        .and_then(|selected| holds.iter().position(|item| item == selected))
    {
        state.selected_index = index;
    } else {
        state.selected_index = state.selected_index.min(holds.len() - 1);
        state.selected_item = Some(holds[state.selected_index].clone());
    }
}

/// The selected item's stack size when it is the coin purse (kind `spark`), else
/// `None` — the one stack the offer flow prompts a count for (05 §7).
fn selected_coin_stack(mirror: &WorldMirror, selected: Option<&ItemId>) -> Option<u32> {
    let item = mirror.item(selected?)?;
    (item.kind == "spark").then_some(item.quantity)
}

/// The coin purse's phrase for `count` units — `1 spark` / `3 sparks`, straight
/// off the catalog display so the plural matches the sheet.
fn coin_phrase(mirror: &WorldMirror, item_id: &ItemId, count: u32) -> String {
    match mirror.item(item_id) {
        Some(item) if count == 1 => format!("{count} {}", item.name),
        Some(item) => format!("{count} {}", item.display_plural),
        None => format!("{count}"),
    }
}

fn inventory_text(
    mirror: &WorldMirror,
    holds: &[ItemId],
    selected: Option<&ItemId>,
    coin_count: Option<u32>,
) -> String {
    if holds.is_empty() {
        return "INVENTORY  empty".into();
    }
    let slots = holds
        .iter()
        .enumerate()
        .map(|(index, item_id)| {
            let label = mirror.item(item_id).map_or_else(
                || item_id.0.clone(),
                |item| {
                    if item.quantity > 1 {
                        format!("{} ×{}", item.name, item.quantity)
                    } else {
                        item.name.clone()
                    }
                },
            );
            if selected == Some(item_id) {
                // The coin purse shows the picked offer count inline, so the
                // player can dial it before ever aiming at a vendor (05 §7).
                let picker = coin_count
                    .map(|count| format!("  ·  offering {count} ([ / ])"))
                    .unwrap_or_default();
                format!("▶ [{}] {}{}", index + 1, label, picker)
            } else {
                format!("[{}] {}", index + 1, label)
            }
        })
        .collect::<Vec<_>>()
        .join("    ");
    format!("INVENTORY    {slots}")
}

fn focus_hint(
    mirror: &WorldMirror,
    focus: &ActorFocus,
    selected: Option<&ItemId>,
    coin_count: Option<u32>,
) -> String {
    let Some(target) = focus.actor.as_ref() else {
        return String::new();
    };
    let name = mirror
        .actor(&target.actor_id)
        .map_or(target.actor_id.0.as_str(), |actor| {
            actor.name_for_player.as_str()
        });
    if focus.item.is_some()
        && let Some(item_id) = selected
    {
        // The coin purse offers a chosen count, adjustable in place; every other
        // item offers whole-stack in v1.
        if let Some(count) = coin_count {
            return format!(
                "{name}  ·  Right click to offer {}  ·  [ / ] to change",
                coin_phrase(mirror, item_id, count)
            );
        }
        let item = mirror
            .item(item_id)
            .map_or(item_id.0.as_str(), |item| item.name.as_str());
        return format!("{name}  ·  Right click to offer {item}");
    }
    name.to_string()
}

fn actionable_offer_cards(
    mirror: &WorldMirror,
    player_position: Vec3,
    player_id: &ActorId,
    dismissed: &HashMap<ItemId, u64>,
) -> Vec<ActiveOfferCard> {
    let mut cards = mirror
        .offers()
        .filter_map(|offer| {
            if offer.giver_id == *player_id
                || offer
                    .target_id
                    .as_ref()
                    .is_some_and(|target| target != player_id)
            {
                return None;
            }
            let broadcast = offer.target_id.is_none();
            if broadcast && dismissed.get(&offer.item_id) == Some(&offer.created_seq) {
                return None;
            }
            let giver = mirror.actor(&offer.giver_id)?;
            let giver_position: Vec3 = giver.position_m.into();
            if player_position.distance_squared(giver_position)
                > ITEM_INTERACTION_RADIUS_M * ITEM_INTERACTION_RADIUS_M
            {
                return None;
            }
            let item = mirror.item(&offer.item_id)?;
            let text = if broadcast {
                format!(
                    "{} offers {} to anyone",
                    giver.name_for_player,
                    indefinite(&item.name)
                )
            } else {
                format!(
                    "{} offers you {}",
                    giver.name_for_player,
                    indefinite(&item.name)
                )
            };
            Some(ActiveOfferCard {
                item_id: offer.item_id.clone(),
                giver_id: offer.giver_id.clone(),
                created_seq: offer.created_seq,
                broadcast,
                text,
                additional_count: 0,
            })
        })
        .collect::<Vec<_>>();
    cards.sort_by(|left, right| {
        left.broadcast
            .cmp(&right.broadcast)
            .then_with(|| left.created_seq.cmp(&right.created_seq))
            .then_with(|| left.item_id.0.cmp(&right.item_id.0))
    });
    cards
}

fn indefinite(name: &str) -> String {
    let article = if name
        .chars()
        .next()
        .is_some_and(|first| matches!(first.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u'))
    {
        "an"
    } else {
        "a"
    };
    format!("{article} {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_actors::model::{
        ActorControl, ActorSnapshot, ItemSnapshot, OfferSnapshot, Position, WorldSnapshot,
    };

    #[test]
    fn effective_streaming_requires_cloud_selected_available_and_configured() {
        let mut config = SmartActorsConfig::default();
        let mut state = MicrophoneInputState::default();
        let mut runtime = SmartActorRuntime::starting(false);

        runtime.stt_cloud_available = true;
        assert!(effective_streaming(&config, &state, &runtime));

        config.stt_streaming = false;
        assert!(!effective_streaming(&config, &state, &runtime));
        config.stt_streaming = true;

        state.stt_backend = TranscriptionBackend::Local;
        assert!(!effective_streaming(&config, &state, &runtime));
        state.stt_backend = TranscriptionBackend::Cloud;

        runtime.stt_cloud_available = false;
        assert!(!effective_streaming(&config, &state, &runtime));
    }

    #[test]
    fn recording_activity_tracks_live_capture_not_microphone_preference() {
        let mut state = MicrophoneInputState::default();
        assert!(state.enabled);
        assert!(!state.recording_active());

        state.recording = Some(RecordingContext {
            wav_basename: "utterance.wav".into(),
            stt_backend: TranscriptionBackend::Cloud,
        });
        assert!(state.recording_active());

        state.clear_on_disconnect();
        assert!(!state.recording_active());
    }

    fn offer_mirror(giver_x: f32) -> WorldMirror {
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    ActorSnapshot {
                        id: ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: ActorControl::Player,
                        position_m: Position::new(0.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                    ActorSnapshot {
                        id: ActorId("ilse".into()),
                        name_for_player: "Ilse".into(),
                        control: ActorControl::Llm,
                        position_m: Position::new(giver_x, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![ItemId("coin".into()), ItemId("fish".into())],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                ],
                items: vec![
                    ItemSnapshot {
                        id: ItemId("coin".into()),
                        kind: "spark".into(),
                        name: "copper coin".into(),
                        display_plural: "copper coins".into(),
                        visual_key: "copper_coin".into(),
                        quantity: 1,
                        metadata: Default::default(),
                    },
                    ItemSnapshot {
                        id: ItemId("fish".into()),
                        kind: "herring".into(),
                        name: "fish".into(),
                        display_plural: "fish".into(),
                        visual_key: "fish".into(),
                        quantity: 1,
                        metadata: Default::default(),
                    },
                ],
                offers: vec![
                    OfferSnapshot {
                        item_id: ItemId("fish".into()),
                        giver_id: ActorId("ilse".into()),
                        target_id: None,
                        created_seq: 1,
                    },
                    OfferSnapshot {
                        item_id: ItemId("coin".into()),
                        giver_id: ActorId("ilse".into()),
                        target_id: Some(ActorId("player".into())),
                        created_seq: 5,
                    },
                ],
                road_carts: vec![],
            })
            .unwrap();
        mirror
    }

    #[test]
    fn one_item_inventory_selects_automatically_and_selection_survives_reorder() {
        let coin = ItemId("coin".into());
        let fish = ItemId("fish".into());
        let mut state = InteractionState::default();
        normalize_selection(&mut state, std::slice::from_ref(&coin));
        assert_eq!(state.selected_item, Some(coin.clone()));

        normalize_selection(&mut state, &[fish, coin.clone()]);
        assert_eq!(state.selected_item, Some(coin));
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn the_inventory_renders_a_stack_count() {
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![ActorSnapshot {
                    id: ActorId("player".into()),
                    name_for_player: "You".into(),
                    control: ActorControl::Player,
                    position_m: Position::new(0.0, 0.0, 0.0).unwrap(),
                    facing_yaw: 0.0,
                    appearance: Default::default(),
                    holds: vec![ItemId("spk".into()), ItemId("one".into())],
                    active_gesture: None,
                    statuses: Vec::new(),
                    pockets: Vec::new(),
                }],
                items: vec![
                    ItemSnapshot {
                        id: ItemId("spk".into()),
                        kind: "spark".into(),
                        name: "spark".into(),
                        display_plural: "sparks".into(),
                        visual_key: "copper_coin".into(),
                        quantity: 3,
                        metadata: Default::default(),
                    },
                    ItemSnapshot {
                        id: ItemId("one".into()),
                        kind: "herring".into(),
                        name: "herring".into(),
                        display_plural: "herrings".into(),
                        visual_key: "fish".into(),
                        quantity: 1,
                        metadata: Default::default(),
                    },
                ],
                offers: vec![],
                road_carts: vec![],
            })
            .unwrap();
        let holds = [ItemId("spk".into()), ItemId("one".into())];
        let text = inventory_text(&mirror, &holds, None, None);
        // A stack above 1 shows a ×N count; a single item shows none.
        assert!(text.contains("spark ×3"), "{text}");
        assert!(text.contains("herring") && !text.contains("herring ×"), "{text}");

        // Selecting the coin stack surfaces the offer count picker inline.
        let picker = inventory_text(&mirror, &holds, Some(&ItemId("spk".into())), Some(2));
        assert!(picker.contains("spark ×3"), "{picker}");
        assert!(picker.contains("offering 2 ([ / ])"), "{picker}");
    }

    #[test]
    fn command_waits_for_both_result_and_newer_snapshot() {
        let mut state = InteractionState::default();
        state.insert_pending(
            "r1".into(),
            PendingKind::Accept {
                item_id: ItemId("coin".into()),
            },
            7,
        );
        assert!(state.resolve_command("r1", true, Some(7)));
        assert!(state.pending.contains_key("r1"));
        state.reconcile_revision(8);
        assert!(!state.pending.contains_key("r1"));
    }

    #[test]
    fn failed_command_unlocks_without_waiting_for_snapshot() {
        let mut state = InteractionState::default();
        state.insert_pending("r1".into(), PendingKind::Recording, 1);
        assert!(state.resolve_command("r1", false, None));
        assert!(state.pending.is_empty());
    }

    /// A command whose result never arrives (the engine died mid-flight) must
    /// not lock its controls forever: the disconnect clears the pending slot.
    #[test]
    fn clearing_pending_unlocks_a_command_whose_result_was_lost() {
        let mut state = InteractionState::default();
        let item_id = ItemId("coin".into());
        let target_id = ActorId("ilse".into());
        state.insert_pending(
            "lost-result".into(),
            PendingKind::Offer {
                item_id: item_id.clone(),
                target_id: target_id.clone(),
                quantity: Some(2),
            },
            3,
        );
        assert!(state.identical_offer_pending(&item_id, &target_id, Some(2)));
        // A different coin count to the same vendor is a genuinely new offer, not
        // a repeat — so it is not deduped and needs no prior retract (05 §7).
        assert!(!state.identical_offer_pending(&item_id, &target_id, Some(3)));

        state.clear_pending();
        assert!(!state.identical_offer_pending(&item_id, &target_id, Some(2)));
    }

    #[test]
    fn microphone_is_user_enabled_by_default_but_not_live_before_availability() {
        let state = MicrophoneInputState::default();
        assert!(state.enabled);
        assert!(!state.is_enabled());
        assert_eq!(state.stt_backend, TranscriptionBackend::Cloud);
    }

    #[test]
    fn z_toggles_between_cloud_and_local_transcription() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyZ);
        let runtime = SmartActorRuntime {
            connected: false,
            ready: false,
            stt_available: false,
            tts_available: false,
            fake_backend: false,
            mirror_revision: None,
            ..SmartActorRuntime::starting(false)
        };
        let mut app = App::new();
        app.insert_resource(keyboard)
            .insert_resource(SmartActorsConfig::default())
            .insert_resource(runtime)
            .insert_resource(MicrophoneInputState::default())
            .insert_resource(SmartActorHudState::default())
            .add_systems(Update, update_microphone_toggle);

        app.update();

        let state = app.world().resource::<MicrophoneInputState>();
        assert_eq!(state.stt_backend, TranscriptionBackend::Local);
        let hud = app.world().resource::<SmartActorHudState>();
        assert_eq!(hud.transcription_backend, "CANARY-QWEN FP16");
    }

    #[test]
    fn v_before_handshake_keeps_the_explicit_mic_off_state_visible() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyV);
        let runtime = SmartActorRuntime {
            connected: false,
            ready: false,
            stt_available: false,
            tts_available: false,
            fake_backend: false,
            mirror_revision: None,
            ..SmartActorRuntime::starting(false)
        };
        let mut app = App::new();
        app.insert_resource(keyboard)
            .insert_resource(SmartActorsConfig::default())
            .insert_resource(runtime)
            .insert_resource(MicrophoneInputState::default())
            .insert_resource(SmartActorHudState::default())
            .add_systems(Update, update_microphone_toggle);

        app.update();
        assert!(!app.world().resource::<MicrophoneInputState>().enabled);
        let hud = app.world().resource::<SmartActorHudState>();
        assert!(hud.microphone_available);
        assert!(!hud.microphone_unavailable);
        assert!(!hud.microphone_enabled);
        assert!(!hud.listening);
    }

    #[test]
    fn available_microphone_arms_by_default_and_v_toggles_both_directions() {
        let (microphone, commands) = MicrophoneService::command_harness_for_tests();
        let state = MicrophoneInputState::default();
        let runtime = SmartActorRuntime {
            connected: true,
            ready: true,
            stt_available: true,
            tts_available: false,
            fake_backend: true,
            mirror_revision: Some(1),
            ..SmartActorRuntime::starting(true)
        };
        let mut hud = SmartActorHudState::default();
        hud.microphone_available = true;
        hud.microphone_unavailable = false;

        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(SmartActorsConfig::default())
            .insert_resource(runtime)
            .insert_resource(microphone)
            .insert_resource(state)
            .insert_resource(hud)
            .add_systems(Update, update_microphone_toggle);

        app.update();
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Enable)));
        assert!(app.world().resource::<MicrophoneInputState>().is_enabled());
        assert!(app.world().resource::<SmartActorHudState>().listening);
        assert!(
            app.world()
                .resource::<SmartActorHudState>()
                .microphone_enabled
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyV);
        app.update();
        assert!(matches!(
            commands.try_recv(),
            Ok(MicrophoneCommand::Disable)
        ));
        assert!(!app.world().resource::<MicrophoneInputState>().enabled);
        assert!(!app.world().resource::<SmartActorHudState>().listening);
        assert!(
            !app.world()
                .resource::<SmartActorHudState>()
                .microphone_enabled
        );

        {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.clear();
            keyboard.release(KeyCode::KeyV);
            keyboard.clear();
            keyboard.press(KeyCode::KeyV);
        }
        app.update();
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Enable)));
        assert!(app.world().resource::<MicrophoneInputState>().enabled);
        assert!(app.world().resource::<SmartActorHudState>().listening);

        {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.clear();
            keyboard.release(KeyCode::KeyV);
            keyboard.clear();
        }
        // The engine going away disarms the capture without forgetting that the
        // player wants his microphone on.
        app.world_mut().resource_mut::<SmartActorRuntime>().ready = false;
        app.update();
        assert!(matches!(
            commands.try_recv(),
            Ok(MicrophoneCommand::Disable)
        ));
        assert!(app.world().resource::<MicrophoneInputState>().enabled);
        let hud = app.world().resource::<SmartActorHudState>();
        assert!(hud.microphone_enabled);
        assert!(!hud.listening);

        app.world_mut().resource_mut::<SmartActorRuntime>().ready = true;
        app.update();
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Enable)));
        assert!(app.world().resource::<MicrophoneInputState>().enabled);
        assert!(app.world().resource::<SmartActorHudState>().listening);

        // Keep the receiving end alive while the resource shuts the worker down.
        drop(app);
    }

    #[test]
    fn actor_focus_hint_does_not_imply_that_gaze_targets_microphone_speech() {
        let mirror = offer_mirror(4.0);
        let focus = ActorFocus {
            actor: Some(super::super::targeting::FocusedActor {
                actor_id: ActorId("ilse".into()),
                entity: Entity::PLACEHOLDER,
                ray_distance_m: 2.0,
                body_distance_m: 2.0,
            }),
            item: None,
        };
        let hint = focus_hint(&mirror, &focus, None, None);
        assert!(!hint.contains("mic"));
        assert!(!hint.contains("talk"));
    }

    /// The coin purse's count picker (05 §7): only the spark stack is a purse,
    /// the count clamps into `[1, stack]`, and both the inventory line and the
    /// focus hint surface it — pluralized off the catalog — so the player is
    /// never forced to offer their whole purse.
    #[test]
    fn the_coin_purse_prompts_for_an_offer_count() {
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    ActorSnapshot {
                        id: ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: ActorControl::Player,
                        position_m: Position::new(0.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![ItemId("purse".into()), ItemId("loaf".into())],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                    ActorSnapshot {
                        id: ActorId("wyn".into()),
                        name_for_player: "Wyn".into(),
                        control: ActorControl::Llm,
                        position_m: Position::new(2.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                ],
                items: vec![
                    ItemSnapshot {
                        id: ItemId("purse".into()),
                        kind: "spark".into(),
                        name: "spark".into(),
                        display_plural: "sparks".into(),
                        visual_key: "copper_coin".into(),
                        quantity: 3,
                        metadata: Default::default(),
                    },
                    ItemSnapshot {
                        id: ItemId("loaf".into()),
                        kind: "loaf".into(),
                        name: "loaf".into(),
                        display_plural: "loaves".into(),
                        visual_key: "loaf".into(),
                        quantity: 1,
                        metadata: Default::default(),
                    },
                ],
                offers: vec![],
                road_carts: vec![],
            })
            .unwrap();

        // Only the spark stack is a purse; the loaf never gets a picker.
        assert_eq!(selected_coin_stack(&mirror, Some(&ItemId("purse".into()))), Some(3));
        assert_eq!(selected_coin_stack(&mirror, Some(&ItemId("loaf".into()))), None);

        // The count clamps into [1, stack]: a fresh 0 reads as one coin, and it
        // never exceeds the purse.
        let mut state = InteractionState::default();
        assert_eq!(state.coin_count(3), 1, "a fresh purse offers a single coin");
        state.coin_offer_count = 5;
        assert_eq!(state.coin_count(3), 3, "never more than the purse holds");

        // A focused vendor plus the purse selected: the hint prompts for a count,
        // pluralized off the catalog.
        let facing_wyn = ActorFocus {
            actor: Some(super::super::targeting::FocusedActor {
                actor_id: ActorId("wyn".into()),
                entity: Entity::PLACEHOLDER,
                ray_distance_m: 2.0,
                body_distance_m: 2.0,
            }),
            item: Some(super::super::targeting::FocusedActor {
                actor_id: ActorId("wyn".into()),
                entity: Entity::PLACEHOLDER,
                ray_distance_m: 2.0,
                body_distance_m: 2.0,
            }),
        };
        let purse = ItemId("purse".into());
        let many = focus_hint(&mirror, &facing_wyn, Some(&purse), Some(3));
        assert!(many.contains("offer 3 sparks"), "{many}");
        assert!(many.contains("[ / ] to change"), "{many}");
        let one = focus_hint(&mirror, &facing_wyn, Some(&purse), Some(1));
        assert!(one.contains("offer 1 spark"), "singular at one: {one}");
    }

    #[test]
    fn right_click_intent_does_not_mutate_inventory_projection() {
        let holds = vec![ItemId("coin".into())];
        let before = holds.clone();
        let _intent = PlayerIntent::Offer {
            request_id: "r".into(),
            target_id: ActorId("conny".into()),
            item_id: holds[0].clone(),
            quantity: None,
            spatial_seq: 3,
            position: Vec3::ZERO,
        };
        assert_eq!(holds, before);
    }

    #[test]
    fn broadcast_dismissal_is_local_and_never_a_decline_intent() {
        let card = ActiveOfferCard {
            item_id: ItemId("fish".into()),
            giver_id: ActorId("sven".into()),
            created_seq: 2,
            broadcast: true,
            text: String::new(),
            additional_count: 0,
        };
        let mut dismissed = HashMap::new();
        dismissed.insert(card.item_id.clone(), card.created_seq);
        assert_eq!(dismissed.get(&card.item_id), Some(&2));
    }

    #[test]
    fn spatial_sequence_advances_when_an_action_has_a_new_position() {
        let mut state = PlayerSpatialState::default();
        assert_eq!(state.mark_hello_position(Vec3::ZERO), 0);
        assert_eq!(state.position_for_action(Vec3::ZERO), 0);
        assert_eq!(state.position_for_action(Vec3::X), 1);
        assert_eq!(state.position_for_action(Vec3::X), 1);
    }

    #[test]
    fn articles_make_offer_cards_read_naturally() {
        assert_eq!(indefinite("copper coin"), "a copper coin");
        assert_eq!(indefinite("apple"), "an apple");
    }

    #[test]
    fn cancelled_microphone_start_does_not_interrupt_npc_audio() {
        assert!(should_interrupt_npc_speech(true, true));
        assert!(!should_interrupt_npc_speech(true, false));
        assert!(!should_interrupt_npc_speech(false, true));
    }

    #[test]
    fn targeted_offer_cards_precede_broadcasts_and_keep_exact_item_ids() {
        let mirror = offer_mirror(4.0);
        let cards = actionable_offer_cards(
            &mirror,
            Vec3::ZERO,
            &ActorId("player".into()),
            &HashMap::new(),
        );

        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].item_id, ItemId("coin".into()));
        assert!(!cards[0].broadcast);
        assert_eq!(cards[1].item_id, ItemId("fish".into()));
        assert!(cards[1].broadcast);
    }

    #[test]
    fn offer_range_is_inclusive_and_broadcast_dismissal_is_revision_scoped() {
        let mirror = offer_mirror(ITEM_INTERACTION_RADIUS_M);
        let player = ActorId("player".into());
        let mut dismissed = HashMap::new();
        dismissed.insert(ItemId("fish".into()), 1);
        let cards = actionable_offer_cards(&mirror, Vec3::ZERO, &player, &dismissed);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].item_id, ItemId("coin".into()));

        let outside = offer_mirror(ITEM_INTERACTION_RADIUS_M + 0.001);
        assert!(actionable_offer_cards(&outside, Vec3::ZERO, &player, &HashMap::new()).is_empty());
    }

    #[test]
    fn disconnect_hides_actionable_cards_while_retaining_the_mirror() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(offer_mirror(3.0))
            .insert_resource(SmartActorRuntime {
                connected: true,
                ready: true,
                stt_available: false,
                tts_available: false,
                fake_backend: true,
                mirror_revision: Some(1),
                ..SmartActorRuntime::starting(true)
            })
            .init_resource::<ActorFocus>()
            .init_resource::<InteractionState>()
            .init_resource::<SmartActorHudState>()
            .add_systems(Update, reconcile_interaction_state);
        app.world_mut().spawn((
            PlayerController::default(),
            GlobalTransform::from_translation(Vec3::ZERO),
        ));

        app.update();
        assert!(
            app.world()
                .resource::<InteractionState>()
                .active_offer
                .is_some()
        );
        assert!(
            !app.world()
                .resource::<SmartActorHudState>()
                .offer_card
                .is_empty()
        );

        app.world_mut()
            .resource_mut::<SmartActorRuntime>()
            .connected = false;
        app.update();
        assert!(
            app.world()
                .resource::<InteractionState>()
                .active_offer
                .is_none()
        );
        assert!(
            app.world()
                .resource::<SmartActorHudState>()
                .offer_card
                .is_empty()
        );
        assert_eq!(app.world().resource::<WorldMirror>().offers().len(), 2);
    }
}
