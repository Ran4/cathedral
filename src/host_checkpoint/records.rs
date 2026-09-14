use super::*;
use crate::smart_actors::{
    interaction::{PendingKind, PlayerIntent},
    speech::PresentSpeech,
};

pub(super) fn speech(v: &PresentSpeech) -> SpeechV1<&str> {
    SpeechV1 {
        sequence: v.event_seq,
        event: &v.event_id,
        speaker: &v.speaker_id.0,
        label: &v.speaker_label,
        target: Nullable(v.target_id.as_ref().map(|id| id.0.as_str())),
        text: &v.text,
        position: v.speaker_position.to_array(),
        recipient_count: v.recipient_count,
        expect_audio: v.expect_audio,
    }
}
pub(super) fn visit<'a>(
    o: &HostObservation<'a>,
    v: &mut dyn FnMut(RecordRef<'a>) -> Result<()>,
) -> Result<()> {
    use RecordV1 as R;
    let chat = o.resource::<smart_actors::ChatInputState>()?;
    v(R::Draft { text: &chat.buffer })?;
    let interaction = o.resource::<InteractionState>()?;
    v(R::SelectedItem {
        item: Nullable(interaction.selected_item.as_ref().map(|i| i.0.as_str())),
    })?;
    let law = o.resource::<smart_actors::custody::PlayerCustodyState>()?;
    if let Some(c) = &law.custody {
        v(R::Custody {
            officer: Nullable(c.officer_id.as_ref().map(|id| id.0.as_str())),
            officer_name: &c.officer_name,
            station_name: &c.station_name,
            anchor: c.anchor_m.to_array(),
            closing: c.closing,
            strain_seconds: c.strain_seconds,
            held: c.held,
            committed: c.committed,
            fee_sparks: c.fee_sparks,
            release_office: Nullable(c.release_office.as_deref()),
            booked_as: Nullable(c.booked_as.as_deref()),
        })?;
        for id in &c.holder_ids {
            v(R::Holder { actor: &id.0 })?;
        }
    }
    for n in &law.notices {
        v(R::Notice {
            id: n.notice_id,
            line: &n.line,
            rung: n.rung.into(),
            clears_when: &n.clears_when,
        })?;
    }
    let journal = o.resource::<smart_actors::journal_ui::PlayerJournal>()?;
    for row in &journal.entries {
        v(R::Journal {
            attribution: &row.attribution,
            word: &row.word,
        })?;
    }
    for s in &journal.standing {
        v(R::JournalStanding { text: s })?;
    }
    let hud = o.resource::<SmartActorHudState>()?;
    for (slot, text) in [
        (HudSlot::Subtitle, &hud.subtitle),
        (HudSlot::Inventory, &hud.inventory),
        (HudSlot::OfferCard, &hud.offer_card),
        (HudSlot::LawStanding, &hud.law_standing),
        (HudSlot::JournalStanding, &hud.journal_standing),
        (HudSlot::FocusHint, &hud.focus_hint),
    ] {
        v(R::Hud {
            slot,
            text,
            remaining: Nullable(None),
        })?;
    }
    for (slot, t) in [
        (HudSlot::PlayerTranscript, &hud.player_transcript),
        (HudSlot::OfferOutcome, &hud.offer_outcome),
        (HudSlot::Transient, &hud.transient),
    ] {
        if let Some(t) = t {
            v(R::Hud {
                slot,
                text: &t.text,
                remaining: Nullable(Some(t.remaining)),
            })?;
        }
    }
    if hud.player_receipt_unavailable {
        return Err(error("unavailable committed player caption receipt"));
    }
    if hud.player_transcript.is_some() {
        if let Some(r) = &hud.player_receipt {
            v(R::PlayerReceipt { speech: speech(r) })?;
        }
    }
    if let Some(c) = &interaction.active_offer {
        v(R::ActiveOffer {
            item: &c.item_id.0,
            giver: &c.giver_id.0,
            created_seq: c.created_seq,
            broadcast: c.broadcast,
            text: &c.text,
            additional_count: c.additional_count,
        })?;
    }
    for (i, seq) in &interaction.dismissed_broadcasts {
        v(R::DismissedBroadcast {
            item: &i.0,
            created_seq: *seq,
        })?;
    }
    for (request, p) in &interaction.pending {
        v(R::PendingCommand {
            request,
            kind: pending(&p.kind),
            sent_revision: p.sent_revision,
            succeeded: p.succeeded,
        })?;
    }
    if let Some(c) = &o.resource::<smart_actors::InventoryUiState>()?.context_menu {
        v(R::InventoryContext {
            item: &c.item_id.0,
            source: match c.source {
                smart_actors::inventory_ui::ItemSource::Carried => ItemSourceV1::Carried,
                smart_actors::inventory_ui::ItemSource::Pocketed(s) => {
                    ItemSourceV1::Pocketed(s.into())
                }
            },
            screen_position: c.screen_pos.to_array(),
            spit_target: Nullable(
                c.spit_target
                    .as_ref()
                    .map(|(id, n)| (id.0.as_str(), n.as_str())),
            ),
        })?;
    }
    let hold = o.resource::<crate::city::marks::ChalkHold>()?;
    v(R::ChalkHold {
        intent: Nullable(hold.intent.as_ref().map(|i| match i {
            crate::city::marks::ChalkIntent::Scrub(id) => ChalkIntentV1::Scrub(*id),
            crate::city::marks::ChalkIntent::Draw { kind, handle } => ChalkIntentV1::Draw {
                kind: (*kind).into(),
                handle: handle.as_str(),
            },
        })),
    })?;
    let chalk = o.resource::<crate::city::marks::ChalkStanding>()?;
    v(R::ChalkPen { present: chalk.pen })?;
    for a in &chalk.anchors {
        if a.kinds.len() > 3 {
            return Err(error("unsupported chalk kind count"));
        }
        let mut kinds = [Nullable(None); 3];
        for (slot, kind) in kinds.iter_mut().zip(&a.kinds) {
            slot.0 = Some((*kind).into());
        }
        v(R::ChalkAnchor {
            handle: &a.handle,
            label: &a.label,
            kinds,
        })?;
    }
    let cool = o.resource::<soundscape::CueCooldowns>()?;
    for (key, free_at) in &cool.free_at {
        v(R::Cooldown {
            key: *key,
            free_at: *free_at,
        })?;
    }
    for (source, at) in &o.resource::<soundscape::WellSoundState>()?.last_draw_at {
        v(R::WellDraw {
            source: well(*source),
            at: *at,
        })?;
    }
    for (kind, w) in &o.resource::<soundscape::WorkSoundState>()?.0 {
        v(R::Work {
            kind: work(*kind),
            position: w.position.to_array(),
            active: w.active,
        })?;
    }
    let generation = o.resource::<BridgeHandle>()?.generation();
    let state = o.resource::<SpeechPresentationState>()?;
    if state.generation != generation {
        return Err(error("speech generation disagreement"));
    }
    for line in &state.subtitles {
        let r = line
            .receipt
            .as_ref()
            .ok_or_else(|| error("subtitle lacks original receipt"))?;
        v(R::Subtitle {
            speech: speech(r),
            formatted_text: &line.text,
            minimum_seconds: line.minimum_seconds,
            visible_since: Nullable(line.visible_since),
            audio_playing: line.audio_playing,
        })?;
    }
    for e in o.world.iter_entities() {
        if let Some(b) = e.get::<SpeechBubble>() {
            let r = b
                .receipt
                .as_ref()
                .ok_or_else(|| error("bubble lacks original receipt"))?;
            let text = e
                .get::<Text>()
                .ok_or_else(|| error("bubble has no readable text"))?;
            let parent = e
                .get::<ChildOf>()
                .ok_or_else(|| error("bubble has no stack"))?
                .parent();
            let stack = o
                .world
                .get::<SpeechBubbleStack>(parent)
                .ok_or_else(|| error("bubble has no anchor"))?;
            if r.speaker_id != stack.speaker_id || r.event_id != b.event_id {
                return Err(error("bubble receipt disagreement"));
            }
            let audio_extended = state.audio_order.iter().any(|a| a.event_id == b.event_id)
                || state
                    .active_voice
                    .as_ref()
                    .is_some_and(|a| a.event_id == b.event_id);
            v(R::Bubble {
                speech: speech(r),
                text: &text.0,
                world_anchor: stack.world_position.to_array(),
                expires_at: b.expires_at,
                audio_extended,
            })?;
        }
    }
    unread(
        o.resource::<Messages<PresentSpeech>>()?,
        state.speech_read,
        |id, m| {
            if m.generation == generation {
                v(R::UnreadSpeech {
                    message_id: id as u64,
                    speech: speech(m),
                })?;
            }
            Ok(())
        },
    )?;
    unread(
        o.resource::<Messages<soundscape::SoundscapeCue>>()?,
        cool.cue_read,
        |id, m| {
            if let soundscape::SoundscapeCue::CivicBell(p) = m {
                v(R::UnreadBell {
                    message_id: id as u64,
                    pattern: match p {
                        soundscape::BellPattern::ScoldCurfew => BellV1::ScoldCurfew,
                        soundscape::BellPattern::ScoldSummons => BellV1::ScoldSummons,
                        soundscape::BellPattern::NameKnell { years } => {
                            BellV1::NameKnell { years: *years }
                        }
                    },
                })?;
            }
            Ok(())
        },
    )?;
    unread(
        o.resource::<Messages<PlayerIntent>>()?,
        interaction.intent_read,
        |id, m| {
            let m = match m {
                PlayerIntent::InGeneration {
                    generation: g,
                    intent,
                } if *g == generation => &**intent,
                PlayerIntent::InGeneration { .. } => return Ok(()),
                m if generation == cathedral_sim::RuntimeGeneration::INITIAL => m,
                _ => return Ok(()),
            };
            v(R::UnreadIntent {
                message_id: id as u64,
                intent: intent(m)?,
            })
        },
    )?;
    Ok(())
}
fn unread<'a, M: Message>(
    messages: &'a Messages<M>,
    start: usize,
    mut f: impl FnMut(usize, &'a M) -> Result<()>,
) -> Result<()> {
    if start < messages.oldest_message_count() {
        return Err(error("host consumer missed messages"));
    }
    let mut id = start;
    while let Some((message, _)) = messages.get_message(id) {
        f(id, message)?;
        id = id
            .checked_add(1)
            .ok_or_else(|| error("message identity exhausted"))?;
    }
    Ok(())
}
fn pending(p: &PendingKind) -> PendingKindV1<&str> {
    use PendingKind as P;
    match p {
        P::Recording => PendingKindV1::Recording,
        P::Offer {
            item_id,
            target_id,
            quantity,
        } => PendingKindV1::Offer {
            item: &item_id.0,
            target: &target_id.0,
            quantity: Nullable(*quantity),
        },
        P::Accept { item_id } => PendingKindV1::Accept { item: &item_id.0 },
        P::Decline { item_id } => PendingKindV1::Decline { item: &item_id.0 },
        P::Retract { item_id } => PendingKindV1::Retract { item: &item_id.0 },
        P::BodySlot { item_id } => PendingKindV1::BodySlot { item: &item_id.0 },
        P::Expel => PendingKindV1::Expel,
        P::DebugSay => PendingKindV1::DebugSay,
        P::Say => PendingKindV1::Say,
    }
}
fn well(v: soundscape::SpecialWell) -> WellKind {
    match v {
        soundscape::SpecialWell::Ford => WellKind::Ford,
        soundscape::SpecialWell::Chain => WellKind::Chain,
        soundscape::SpecialWell::ThreeCurb => WellKind::ThreeCurb,
    }
}
fn work(v: soundscape::WorkActivityKind) -> WorkKind {
    match v {
        soundscape::WorkActivityKind::Baking => WorkKind::Baking,
        soundscape::WorkActivityKind::EelSmoking => WorkKind::EelSmoking,
        soundscape::WorkActivityKind::GlassFurnace => WorkKind::GlassFurnace,
        soundscape::WorkActivityKind::CulletSorting => WorkKind::CulletSorting,
        soundscape::WorkActivityKind::Weaving => WorkKind::Weaving,
    }
}
fn intent(v: &PlayerIntent) -> Result<IntentV1<&str>> {
    use IntentV1 as I;
    use PlayerIntent as P;
    Ok(match v {
        P::InGeneration { .. } => return Err(error("nested player intent")),
        P::SpatialUpdate {
            spatial_seq,
            position,
            facing_yaw,
        } => I::SpatialUpdate {
            sequence: *spatial_seq,
            position: position.to_array(),
            yaw: *facing_yaw,
        },
        P::Sound { sound_id } => I::Sound { sound: sound_id },
        P::Recording {
            request_id,
            wav_basename,
            stt_backend,
            spatial_seq,
            position,
        } => I::RecordingInterrupted {
            request: request_id,
            basename: wav_basename,
            backend: stt_backend.name(),
            sequence: *spatial_seq,
            position: position.to_array(),
        },
        P::Offer {
            request_id,
            target_id,
            item_id,
            quantity,
            spatial_seq,
            position,
        } => I::Offer {
            request: request_id,
            target: &target_id.0,
            item: &item_id.0,
            quantity: Nullable(*quantity),
            sequence: *spatial_seq,
            position: position.to_array(),
        },
        P::Accept {
            request_id,
            item_id,
            spatial_seq,
            position,
        } => I::Accept {
            request: request_id,
            item: &item_id.0,
            sequence: *spatial_seq,
            position: position.to_array(),
        },
        P::Decline {
            request_id,
            item_id,
            spatial_seq,
            position,
        } => I::Decline {
            request: request_id,
            item: &item_id.0,
            sequence: *spatial_seq,
            position: position.to_array(),
        },
        P::Retract {
            request_id,
            item_id,
        } => I::Retract {
            request: request_id,
            item: &item_id.0,
        },
        P::Pocket {
            request_id,
            item_id,
            slot,
        } => I::Pocket {
            request: request_id,
            item: &item_id.0,
            slot: (*slot).into(),
        },
        P::Retrieve {
            request_id,
            item_id,
        } => I::Retrieve {
            request: request_id,
            item: &item_id.0,
        },
        P::Swallow {
            request_id,
            item_id,
        } => I::Swallow {
            request: request_id,
            item: &item_id.0,
        },
        P::Spit {
            request_id,
            item_id,
            target_id,
            spatial_seq,
            position,
        } => I::Spit {
            request: request_id,
            item: &item_id.0,
            target: &target_id.0,
            sequence: *spatial_seq,
            position: position.to_array(),
        },
        P::Gargle {
            request_id,
            item_id,
        } => I::Gargle {
            request: request_id,
            item: &item_id.0,
        },
        P::Expel { request_id } => I::Expel {
            request: request_id,
        },
        P::Eat {
            request_id,
            item_id,
        } => I::Eat {
            request: request_id,
            item: &item_id.0,
        },
        P::DebugSay {
            request_id,
            text,
            target_id,
            spatial_seq,
            position,
        } => I::DebugSay {
            request: request_id,
            text,
            target: Nullable(target_id.as_ref().map(|id| id.0.as_str())),
            sequence: *spatial_seq,
            position: position.to_array(),
        },
        P::Say {
            request_id,
            text,
            spatial_seq,
            position,
        } => I::Say {
            request: request_id,
            text,
            sequence: *spatial_seq,
            position: position.to_array(),
        },
    })
}
