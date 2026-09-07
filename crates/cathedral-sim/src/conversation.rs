//! Player conversation evidence. Hearing stays physical; this only selects a
//! first reaction and describes the apparent addressee in that utterance's percept.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::{
    ActorId, Control, HEARING_RADIUS_M, STAGE_PARTNER_MEMORY_SECONDS, World,
    actions::apply_player_speech_at, error::ActionError, math::Vec3, perception::identify_ids,
    scheduler::NpcScheduler,
};

pub const FOCUS_DWELL_SECONDS: f64 = 0.65;
const FOCUS_STALE_SECONDS: f64 = 1.0;
const INVITATION_SECONDS: f64 = 10.0;
pub const CAPTURE_LIFETIME_SECONDS: f64 = 120.0;
const MAX_EXCHANGE_WITNESSES: usize = 128;

#[derive(Debug, Clone, PartialEq)]
struct Engagement {
    actor: ActorId,
    at: f64,
    reciprocal: bool,
    witnesses: BTreeSet<ActorId>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Conversation {
    engagement: Option<Engagement>,
    invitation: Option<(ActorId, f64)>,
    focus: Option<(ActorId, f64, f64)>,
    next_utterance: u64,
    latest_applied_utterance: u64,
}

/// A value captured before transcription. Later gaze, invitations and partner
/// changes cannot reinterpret it. Candidates are checked against hearing again
/// when the words become available.
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedAttention {
    pub captured_at: f64,
    sequence: u64,
    focus: Option<ActorId>,
    engagement: Option<Engagement>,
    invitation: Option<ActorId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressEvidence {
    Named,
    Group,
    Focus,
    Reciprocal,
    Continuing,
    Invitation,
    Nearest,
    Nobody,
}

impl AddressEvidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Named => "explicit name",
            Self::Group => "open group language",
            Self::Focus => "sustained attention",
            Self::Reciprocal | Self::Continuing => "ongoing exchange",
            Self::Invitation => "recent invitation",
            Self::Nearest => "proximity only",
            Self::Nobody => "no plausible listener",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechSelection {
    pub addressee: Option<ActorId>,
    pub evidence: AddressEvidence,
    witnesses: BTreeSet<ActorId>,
}

/// Supplied by the host with the other prompt strings; the simulation does
/// not read assets, and social wording stays editable without rebuilding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationStrings {
    pub addressed: String,
    pub overhearing: String,
    pub group: String,
    pub unseen_target: String,
    pub apparent_attention: String,
    pub named: String,
    pub focus: String,
    pub continuing: String,
    pub invitation: String,
    pub nearest: String,
}

impl SpeechSelection {
    /// Render at delivery, once, inside the speech percept. Unknown names stay
    /// unknown, and a distant target's identity is not disclosed to a hearer.
    pub fn percept_suffix(
        &self,
        world: &World,
        observer: &ActorId,
        prose: &ConversationStrings,
    ) -> String {
        if self.evidence == AddressEvidence::Group {
            return prose.group.clone();
        }
        let Some(target) = &self.addressee else {
            return String::new();
        };
        let evidence = if matches!(
            self.evidence,
            AddressEvidence::Reciprocal | AddressEvidence::Continuing | AddressEvidence::Invitation
        ) && !self.witnesses.contains(observer)
        {
            &prose.apparent_attention
        } else {
            match self.evidence {
                AddressEvidence::Named => &prose.named,
                AddressEvidence::Focus => &prose.focus,
                AddressEvidence::Reciprocal | AddressEvidence::Continuing => &prose.continuing,
                AddressEvidence::Invitation => &prose.invitation,
                _ => &prose.nearest,
            }
        };
        if target == observer {
            return prose.addressed.replace("{evidence}", evidence);
        }
        let target_label = match (world.characters.get(observer), world.characters.get(target)) {
            (Some(a), Some(b)) if a.position_m().distance(b.position_m()) <= HEARING_RADIUS_M => {
                identify_ids(world, observer, target)
            }
            _ => prose.unseen_target.clone(),
        };
        prose
            .overhearing
            .replace("{target}", &target_label)
            .replace("{evidence}", evidence)
    }
}

impl Conversation {
    pub fn observe_focus(&mut self, now: f64, actor: Option<ActorId>) {
        self.focus = actor.map(|actor| {
            let since = self
                .focus
                .as_ref()
                .filter(|(id, _, last)| id == &actor && now - last <= FOCUS_STALE_SECONDS)
                .map_or(now, |(_, since, _)| *since);
            (actor, since, now)
        });
    }

    pub fn partner<'a>(&'a self, now: f64, world: &World) -> Option<&'a ActorId> {
        self.engagement
            .as_ref()
            .filter(|e| now - e.at < STAGE_PARTNER_MEMORY_SECONDS && world.is_present(&e.actor))
            .map(|e| &e.actor)
            .or_else(|| {
                self.invitation
                    .as_ref()
                    .filter(|(id, at)| now - at < INVITATION_SECONDS && world.is_present(id))
                    .map(|(id, _)| id)
            })
    }

    pub fn capture(&mut self, now: f64, world: &World) -> CapturedAttention {
        self.next_utterance = self.next_utterance.saturating_add(1);
        CapturedAttention {
            captured_at: now,
            sequence: self.next_utterance,
            focus: self
                .focus
                .as_ref()
                .filter(|(_, since, last)| {
                    now - since >= FOCUS_DWELL_SECONDS && now - last <= FOCUS_STALE_SECONDS
                })
                .map(|(id, _, _)| id.clone()),
            engagement: self
                .engagement
                .as_ref()
                .filter(|e| now - e.at < STAGE_PARTNER_MEMORY_SECONDS && world.is_present(&e.actor))
                .cloned(),
            invitation: self
                .invitation
                .as_ref()
                .filter(|(id, at)| now - at < INVITATION_SECONDS && world.is_present(id))
                .map(|(id, _)| id.clone()),
        }
    }

    /// NPC speech can invite engagement or confirm the player's choice. It
    /// cannot acquire an exchange from another actor, nor resurrect an expired one.
    pub fn npc_addressed_player(&mut self, now: f64, actor: &ActorId, recipients: &[ActorId]) {
        if let Some(e) = self.engagement.as_mut()
            && e.actor == *actor
            && now - e.at < STAGE_PARTNER_MEMORY_SECONDS
        {
            e.reciprocal = true;
            e.at = now;
            e.witnesses.extend(recipients.iter().cloned());
            e.witnesses.insert(actor.clone());
            while e.witnesses.len() > MAX_EXCHANGE_WITNESSES {
                e.witnesses.pop_last();
            }
        }
        self.invitation = Some((actor.clone(), now));
    }

    pub fn player_addressed(&mut self, now: f64, actor: &ActorId, recipients: &[ActorId]) {
        self.next_utterance = self.next_utterance.saturating_add(1);
        self.latest_applied_utterance = self.next_utterance;
        self.note_choice(now, actor, recipients);
    }

    fn note_choice(&mut self, now: f64, actor: &ActorId, recipients: &[ActorId]) {
        let previous = self
            .engagement
            .take()
            .filter(|e| e.actor == *actor && now - e.at < STAGE_PARTNER_MEMORY_SECONDS);
        let reciprocal = previous.as_ref().is_some_and(|e| e.reciprocal)
            || self
                .invitation
                .as_ref()
                .is_some_and(|(id, at)| id == actor && now - at < INVITATION_SECONDS);
        let mut witnesses = previous.map_or_else(BTreeSet::new, |e| e.witnesses);
        witnesses.extend(recipients.iter().cloned());
        while witnesses.len() > MAX_EXCHANGE_WITNESSES {
            witnesses.pop_last();
        }
        self.engagement = Some(Engagement {
            actor: actor.clone(),
            at: now,
            reciprocal,
            witnesses,
        });
    }

    pub fn forget(&mut self, actors: &[ActorId]) {
        if self
            .engagement
            .as_ref()
            .is_some_and(|e| actors.contains(&e.actor))
        {
            self.engagement = None;
        }
        if self
            .invitation
            .as_ref()
            .is_some_and(|(id, _)| actors.contains(id))
        {
            self.invitation = None;
        }
        if self
            .focus
            .as_ref()
            .is_some_and(|(id, _, _)| actors.contains(id))
        {
            self.focus = None;
        }
    }
}

impl CapturedAttention {
    pub fn select(
        &self,
        now: f64,
        world: &World,
        player: &ActorId,
        at: Vec3,
        text: &str,
    ) -> SpeechSelection {
        let listeners: Vec<_> = world
            .characters_within(at, HEARING_RADIUS_M, Some(player))
            .into_iter()
            .filter(|id| world.characters[id].control() == Control::Llm)
            .collect();
        let selection = |addressee, evidence, witnesses| SpeechSelection {
            addressee,
            evidence,
            witnesses,
        };
        let no_witnesses = BTreeSet::new();
        // Explicit cues are intentionally small and anchored. A reported name
        // ("Sibbe said...") or a question about someone must not change lanes.
        if let Some(id) = named_addressee(world, &listeners, text) {
            return selection(Some(id), AddressEvidence::Named, no_witnesses);
        }
        if is_group_address(text) {
            return selection(None, AddressEvidence::Group, no_witnesses);
        }
        if now - self.captured_at < CAPTURE_LIFETIME_SECONDS {
            if let Some(id) = self.focus.as_ref().filter(|id| listeners.contains(id)) {
                return selection(Some(id.clone()), AddressEvidence::Focus, no_witnesses);
            }
            if let Some(e) = self
                .engagement
                .as_ref()
                .filter(|e| listeners.contains(&e.actor))
            {
                return selection(
                    Some(e.actor.clone()),
                    if e.reciprocal {
                        AddressEvidence::Reciprocal
                    } else {
                        AddressEvidence::Continuing
                    },
                    e.witnesses.clone(),
                );
            }
            if let Some(id) = self.invitation.as_ref().filter(|id| listeners.contains(id)) {
                return selection(Some(id.clone()), AddressEvidence::Invitation, no_witnesses);
            }
        }
        let nearest = listeners.first().cloned();
        let evidence = if nearest.is_some() {
            AddressEvidence::Nearest
        } else {
            AddressEvidence::Nobody
        };
        selection(nearest, evidence, no_witnesses)
    }
}

/// The typed and STT paths both use this transaction. Context is committed only
/// after a valid `say`; all bystanders retain the same inbox and ordinary lanes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn speak(
    now: f64,
    world: &mut World,
    scheduler: &mut NpcScheduler,
    conversation: &mut Conversation,
    player: &ActorId,
    captured: &CapturedAttention,
    at: Vec3,
    text: &str,
    prose: &ConversationStrings,
) -> Result<(String, SpeechSelection), ActionError> {
    let selection = captured.select(now, world, player, at, text);
    let line = apply_player_speech_at(world, player, text, at, &selection, prose)?;
    let hearers = world.characters_within(at, HEARING_RADIUS_M, Some(player));
    let newest = captured.sequence >= conversation.latest_applied_utterance;
    if newest {
        conversation.latest_applied_utterance = captured.sequence;
    }
    if let Some(id) = &selection.addressee {
        scheduler.prioritize_player_reaction(world, id, now);
        // A delayed old utterance must not overwrite a newer player choice.
        if newest {
            conversation.note_choice(now, id, &hearers);
        }
    } else if selection.evidence == AddressEvidence::Group {
        // Only the first group reaction is protected. The remaining listeners
        // get ordinary handoffs, so a fresh player follow-up wins immediately
        // while even an off-stage invited listener can still answer later.
        for (index, id) in hearers
            .iter()
            .filter(|id| world.characters[*id].control() == Control::Llm)
            .enumerate()
        {
            if index == 0 {
                scheduler.prioritize_player_reaction(world, id, now);
            } else {
                scheduler.prioritize(world, id, false, now);
            }
        }
    }
    Ok((line, selection))
}

fn named_addressee(world: &World, listeners: &[ActorId], text: &str) -> Option<ActorId> {
    let lower = text.trim().to_lowercase();
    let text = ["hey ", "hello ", "excuse me, "]
        .iter()
        .find_map(|prefix| lower.strip_prefix(prefix))
        .unwrap_or(&lower);
    let matches: Vec<_> = listeners
        .iter()
        .filter(|id| {
            let name = world.characters[*id].name().to_lowercase();
            std::iter::once(name.as_str())
                .chain(name.split_whitespace().next())
                .any(|name| {
                    text.strip_prefix(name).is_some_and(|rest| {
                        // "Conny, the fisherman, said..." is an appositive
                        // mention, not a vocative. Leave harder syntax to the
                        // model instead of treating every leading name as routing.
                        let after_comma = rest.strip_prefix(',').map(str::trim_start);
                        if after_comma.is_some_and(|tail| {
                            ["the ", "a ", "an ", "who ", "whose "]
                                .iter()
                                .any(|prefix| tail.starts_with(prefix))
                                && tail.contains(',')
                        }) {
                            return false;
                        }
                        rest.starts_with(',')
                            || rest.starts_with(':')
                            || [
                                " can you ",
                                " could you ",
                                " will you ",
                                " would you ",
                                " do you ",
                                " are you ",
                                " how are you ",
                                " what do you ",
                            ]
                            .iter()
                            .any(|cue| rest.starts_with(cue))
                    })
                })
        })
        .collect();
    (matches.len() == 1).then(|| (*matches[0]).clone())
}

fn is_group_address(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    [
        "anyone ",
        "anybody ",
        "everyone,",
        "everybody,",
        "all of you,",
        "both of you,",
        "you two,",
        "does anyone ",
        "does anybody ",
        "can anyone ",
        "can anybody ",
        "do any of you ",
        "do either of you ",
        "what do you all ",
        "what do you both ",
    ]
    .iter()
    .any(|cue| lower.starts_with(cue))
}
