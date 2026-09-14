use super::*;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};
fn rank<T>(r: &RecordV1<T>) -> u8 {
    use RecordV1::*;
    match r {
        Draft { .. } => 0,
        SelectedItem { .. } => 1,
        Custody { .. } => 2,
        Holder { .. } => 3,
        Notice { .. } => 4,
        Journal { .. } => 5,
        JournalStanding { .. } => 6,
        Hud { .. } => 7,
        ActiveOffer { .. } => 8,
        DismissedBroadcast { .. } => 9,
        PendingCommand { .. } => 10,
        InventoryContext { .. } => 11,
        ChalkHold { .. } => 12,
        ChalkPen { .. } => 13,
        ChalkAnchor { .. } => 14,
        Cooldown { .. } => 15,
        WellDraw { .. } => 16,
        Work { .. } => 17,
        UnreadBell { .. } => 18,
        UnreadIntent { .. } => 19,
        UnreadSpeech { .. } => 20,
        Subtitle { .. } => 21,
        Bubble { .. } => 22,
        PlayerReceipt { .. } => 23,
    }
}
pub(super) fn compare(a: &RecordV1<String>, b: &RecordV1<String>) -> Ordering {
    use RecordV1::*;
    rank(a).cmp(&rank(b)).then_with(|| match (a, b) {
        (Hud { slot: a, .. }, Hud { slot: b, .. }) => (*a as u8).cmp(&(*b as u8)),
        (DismissedBroadcast { item: a, .. }, DismissedBroadcast { item: b, .. }) => a.cmp(b),
        (PendingCommand { request: a, .. }, PendingCommand { request: b, .. }) => a.cmp(b),
        (Cooldown { key: a, .. }, Cooldown { key: b, .. }) => a.cmp(b),
        (WellDraw { source: a, .. }, WellDraw { source: b, .. }) => (*a as u8).cmp(&(*b as u8)),
        (Work { kind: a, .. }, Work { kind: b, .. }) => (*a as u8).cmp(&(*b as u8)),
        (Bubble { speech: a, .. }, Bubble { speech: b, .. }) => a.sequence.cmp(&b.sequence),
        // Every other repeated family retains actual consumer/publication order.
        _ => Ordering::Equal,
    })
}
pub(super) fn validate(rows: &[RecordV1<String>], s: &ScalarsV1) -> Result<()> {
    use RecordV1::*;
    check(
        rows.windows(2)
            .all(|p| compare(&p[0], &p[1]) != Ordering::Greater),
        "host family/key order",
    )?;
    let mut seen = BTreeSet::new();
    let mut originals: BTreeMap<&str, &SpeechV1<String>> = BTreeMap::new();
    let mut sequence_events = BTreeMap::new();
    let mut previous = [None; 3];
    let mut subtitle_sequence = None;
    let mut subtitle_count = 0;
    let mut player_receipt = false;
    let mut hud = [false; 9];
    for row in rows {
        references(row)?;
        let speech = match row {
            UnreadSpeech { speech, .. }
            | Subtitle { speech, .. }
            | Bubble { speech, .. }
            | PlayerReceipt { speech } => Some(speech),
            _ => None,
        };
        if let Some(v) = speech {
            check(
                seen.insert((rank(row), v.event.as_str())),
                "duplicate speech within presentation owner",
            )?;
            if let Some(previous) = originals.insert(&v.event, v) {
                check(
                    previous == v
                        && previous.position.map(f32::to_bits) == v.position.map(f32::to_bits),
                    "original speech receipt disagreement",
                )?;
            }
            if let Some(previous) = sequence_events.insert(v.sequence, v.event.as_str()) {
                check(
                    previous == v.event,
                    "speech sequence reused for another event",
                )?;
            }
        }
        let unread = match row {
            UnreadBell { message_id, .. } => Some((0, *message_id)),
            UnreadIntent { message_id, intent } => {
                validate_intent(intent)?;
                Some((1, *message_id))
            }
            UnreadSpeech { message_id, .. } => Some((2, *message_id)),
            _ => None,
        };
        if let Some((family, id)) = unread {
            check(id < usize::MAX as u64, "unread message identity exhausted")?;
            check(
                previous[family].is_none_or(|p| p < id),
                "unread message order/identity",
            )?;
            previous[family] = Some(id);
        }
        match row {
            Subtitle {
                speech,
                visible_since,
                formatted_text,
                minimum_seconds,
                ..
            } => {
                check(
                    speech.speaker != "player"
                        && s.boundary
                            .last_speech_sequence
                            .0
                            .is_some_and(|last| speech.sequence <= last),
                    "subtitle consumption boundary",
                )?;
                check(
                    subtitle_sequence.is_none_or(|last| last < speech.sequence),
                    "subtitle sequence order",
                )?;
                subtitle_sequence = Some(speech.sequence);
                check(
                    subtitle_count == 0 || visible_since.0.is_none(),
                    "only the front subtitle can have begun",
                )?;
                check(
                    formatted_text == &format!("{}: {}", speech.label, speech.text.trim()),
                    "subtitle readable receipt disagreement",
                )?;
                check(
                    minimum_seconds.to_bits()
                        == f64::from(speech_reading_seconds(speech.text.trim())).to_bits(),
                    "original readable minimum was shortened or changed",
                )?;
                subtitle_count += 1;
            }
            Bubble { speech, text, .. } => check(
                speech.speaker != "player"
                    && text == speech.text.trim()
                    && s.boundary
                        .last_speech_sequence
                        .0
                        .is_some_and(|last| speech.sequence <= last),
                "bubble readable receipt disagreement",
            )?,
            PlayerReceipt { speech } => {
                check(
                    speech.speaker == "player"
                        && s.boundary
                            .last_speech_sequence
                            .0
                            .is_some_and(|last| speech.sequence <= last),
                    "player receipt consumer disagreement",
                )?;
                let expected = player_delivery_caption(&speech.text, speech.recipient_count);
                check(rows.iter().any(|r|matches!(r,Hud{slot:HudSlot::PlayerTranscript,text,..} if *text==expected)),"player caption receipt disagreement")?;
                player_receipt = true;
            }
            Hud {
                slot, remaining, ..
            } => {
                hud[*slot as usize] = true;
                let timed = matches!(
                    slot,
                    HudSlot::PlayerTranscript | HudSlot::OfferOutcome | HudSlot::Transient
                );
                check(remaining.0.is_some() == timed, "HUD lifetime owner")?;
            }
            ChalkAnchor { kinds, .. } => {
                let mut end = false;
                let mut bits = 0u8;
                for k in kinds {
                    match k.0 {
                        None => end = true,
                        Some(k) => {
                            let bit = 1 << (k as u8);
                            check(!end && bits & bit == 0, "chalk kind order/duplication")?;
                            bits |= bit;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    check(
        [
            HudSlot::Subtitle,
            HudSlot::Inventory,
            HudSlot::OfferCard,
            HudSlot::LawStanding,
            HudSlot::JournalStanding,
            HudSlot::FocusHint,
        ]
        .iter()
        .all(|k| hud[*k as usize]),
        "missing singular HUD slot",
    )?;
    check(
        !player_receipt || hud[HudSlot::PlayerTranscript as usize],
        "player receipt without readable caption",
    )
}
fn validate_intent(intent: &IntentV1<String>) -> Result<()> {
    use IntentV1::*;
    let spatial = match intent {
        SpatialUpdate {
            sequence,
            position,
            yaw,
        } => {
            check(yaw.is_finite(), "intent yaw")?;
            Some((*sequence, *position))
        }
        RecordingInterrupted {
            sequence, position, ..
        }
        | Offer {
            sequence, position, ..
        }
        | Accept {
            sequence, position, ..
        }
        | Decline {
            sequence, position, ..
        }
        | Spit {
            sequence, position, ..
        }
        | DebugSay {
            sequence, position, ..
        }
        | Say {
            sequence, position, ..
        } => Some((*sequence, *position)),
        _ => None,
    };
    if let Some((sequence, position)) = spatial {
        check(
            sequence < i64::MAX as u64 && finite(position.map(f64::from)),
            "unsubmitted intent spatial range",
        )?;
    }
    if let Say { text, .. } | DebugSay { text, .. } = intent {
        check(
            !text.trim().is_empty() && text.trim().chars().count() <= 500,
            "unsubmitted text intent range",
        )?;
    }
    match intent {
        SpatialUpdate { .. } => {}
        Sound { sound } => id(sound)?,
        RecordingInterrupted {
            request,
            basename,
            backend,
            ..
        } => {
            id(request)?;
            id(basename)?;
            check(
                matches!(backend.as_str(), "cloud" | "local"),
                "recording interruption backend",
            )?;
        }
        Offer {
            request,
            target,
            item,
            quantity,
            ..
        } => {
            id(request)?;
            id(target)?;
            id(item)?;
            check(quantity.0.is_none_or(|q| q > 0), "offer quantity")?;
        }
        Accept { request, item, .. }
        | Decline { request, item, .. }
        | Retract { request, item }
        | Pocket { request, item, .. }
        | Retrieve { request, item }
        | Swallow { request, item }
        | Gargle { request, item }
        | Eat { request, item } => {
            id(request)?;
            id(item)?;
        }
        Spit {
            request,
            item,
            target,
            ..
        } => {
            id(request)?;
            id(item)?;
            id(target)?;
        }
        Expel { request } | Say { request, .. } => id(request)?,
        DebugSay {
            request, target, ..
        } => {
            id(request)?;
            optional_id(target)?;
        }
    }
    Ok(())
}
fn id(v: &str) -> Result<()> {
    check(
        crate::ids::is_valid_id(v),
        "malformed historical host reference",
    )
}
fn optional_id(v: &Nullable<String>) -> Result<()> {
    if let Some(v) = &v.0 {
        id(v)?;
    }
    Ok(())
}
fn references(row: &RecordV1<String>) -> Result<()> {
    use RecordV1::*;
    match row {
        SelectedItem { item } => optional_id(item)?,
        Custody { officer, .. } => optional_id(officer)?,
        Holder { actor } => id(actor)?,
        ActiveOffer { item, giver, .. } => {
            id(item)?;
            id(giver)?;
        }
        DismissedBroadcast { item, .. } => id(item)?,
        InventoryContext {
            item, spit_target, ..
        } => {
            id(item)?;
            if let Some((target, _)) = &spit_target.0 {
                id(target)?;
            }
        }
        PendingCommand { request, kind, .. } => {
            id(request)?;
            match kind {
                PendingKindV1::Offer {
                    item,
                    target,
                    quantity,
                } => {
                    id(item)?;
                    id(target)?;
                    check(quantity.0.is_none_or(|q| q > 0), "pending offer quantity")?;
                }
                PendingKindV1::Accept { item }
                | PendingKindV1::Decline { item }
                | PendingKindV1::Retract { item }
                | PendingKindV1::BodySlot { item } => id(item)?,
                _ => {}
            }
        }
        ChalkAnchor { handle, .. } => id(handle)?,
        ChalkHold {
            intent: Nullable(Some(ChalkIntentV1::Draw { handle, .. })),
        } => id(handle)?,
        UnreadSpeech { speech, .. }
        | Subtitle { speech, .. }
        | Bubble { speech, .. }
        | PlayerReceipt { speech } => {
            id(&speech.event)?;
            id(&speech.speaker)?;
            optional_id(&speech.target)?;
        }
        UnreadBell {
            pattern: BellV1::NameKnell { years },
            ..
        } => check(*years <= 120, "bell stroke count")?,
        _ => {}
    }
    Ok(())
}
