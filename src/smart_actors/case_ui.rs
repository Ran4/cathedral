//! The journal's Case tab: every matter before the player, where its review
//! stands, the accounts people have given of it, the controls that ask for an
//! account and lodge the picked ones, and one list that tells what was only
//! said from what was asked, taken up, done or broken off.

use std::collections::BTreeSet;

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use cathedral_sim::{CaseAccount, CaseInquiry, FactId};

use crate::fonts::CathedralFonts;

use super::SmartActorRuntime;
use super::bridge::{BridgeCommand, BridgeHandle};
use super::hud::{self, SmartActorHudState};
use super::interaction::InteractionState;
use super::journal_ui::{
    BUTTON_BG, BUTTON_HOVER, CaseEntriesRoot, JournalTab, JournalUiState, locative, spawn_text,
};
use super::model::{self, WorldMirror};
use super::targeting::ActorFocus;

const PICKED_BG: Color = Color::srgba(0.15, 0.26, 0.19, 1.0);

/// How many request rows are kept. Past this the oldest finished one makes
/// room for the next.
const REQUESTS_MAX: usize = 12;

/// One account in the player's notes: the sim's handles for its fact and its
/// speaker, kept as they came and only ever handed back.
pub(super) type Pick = (FactId, cathedral_sim::ActorId);

/// The player's record of each inquiry, as the sim last published it, and what
/// the player is doing with it. The sim has already resolved every name
/// through the player, so the rows are kept as they came and only joined into
/// lines when drawn.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub(super) struct PlayerCase {
    pub inquiries: Vec<CaseInquiry>,
    /// The accounts picked for lodging. A draft: it outlives the overlay
    /// closing and a refused lodging, and an account leaves it only by being
    /// unpicked or taken by a reviewer.
    pub picked: BTreeSet<Pick>,
    /// What the player has asked of the city from this tab and what people
    /// have said they will do, oldest first.
    pub requests: Vec<RequestRow>,
    /// Whether this city's record has arrived once. What the first message
    /// after a start or a load lists was already so, and is no news.
    pub synced: bool,
}

/// One ask or lodging put to the engine and what came of it, or something
/// somebody said they would do.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RequestRow {
    /// The engine's handle for an ask or a lodging. Empty for words heard,
    /// which the engine owes no answer.
    pub request_id: String,
    /// The picks a lodging handed over. Empty for anything else.
    pub sent: Vec<Pick>,
    pub state: RequestState,
    /// What was asked for while it is pending, then the engine's own sentence.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestState {
    /// Somebody said they would. Nothing was asked of the city and nothing
    /// has been done.
    Said,
    Pending,
    /// A reviewer has taken a bundle and its review is dated.
    Accepted,
    Done,
    BrokenOff,
}

impl RequestState {
    fn label(self) -> &'static str {
        match self {
            Self::Said => "SAID — words only",
            Self::Pending => "PENDING",
            Self::Accepted => "ACCEPTED",
            Self::Done => "DONE",
            Self::BrokenOff => "BROKEN OFF",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Said | Self::Pending => hud::MUTED,
            Self::Accepted => hud::TEXT,
            Self::Done => hud::ONLINE,
            Self::BrokenOff => hud::DEGRADED,
        }
    }
}

/// Whether a spoken line reads as an undertaking: "I will fetch the reeve".
pub(super) fn sounds_like_a_promise(text: &str) -> bool {
    let text = text.to_lowercase();
    ["i will ", "i'll ", "i shall "]
        .iter()
        .any(|promise| text.contains(promise))
}

/// What a newly published record adds to the one on show, a line each: the
/// accounts first and the reviews after them, so that where only the last line
/// fits it is a review's.
pub(super) fn notices(before: &[CaseInquiry], after: &[CaseInquiry]) -> Vec<String> {
    let earlier = |inquiry: &CaseInquiry| before.iter().find(|known| known.id == inquiry.id);
    let mut lines = Vec::new();
    for inquiry in after {
        let known = earlier(inquiry);
        for account in &inquiry.accounts {
            // A re-ask replaces a row in place and is not a new account.
            let had = known.is_some_and(|known| {
                known
                    .accounts
                    .iter()
                    .any(|old| old.fact == account.fact && old.speaker == account.speaker)
            });
            if !had {
                lines.push(format!(
                    "New account: {} on {}",
                    account.speaker_name, inquiry.title
                ));
            }
        }
    }
    for inquiry in after {
        if inquiry.finding.is_some()
            && earlier(inquiry).is_none_or(|known| known.finding != inquiry.finding)
        {
            lines.push(format!("The review of {} was held", inquiry.title));
        }
    }
    lines
}

/// The rows the matters themselves add to the request list: each review that
/// was held, and each bundle a reviewer has taken that waits for its review.
fn review_rows(inquiries: &[CaseInquiry]) -> Vec<(RequestState, String)> {
    let mut rows = Vec::new();
    for inquiry in inquiries {
        if let Some(finding) = &inquiry.finding {
            rows.push((
                RequestState::Done,
                format!("{}: review held at {}", inquiry.title, finding.held),
            ));
        }
        if let Some(at) = &inquiry.review_at {
            rows.push((
                RequestState::Accepted,
                format!(
                    "{}: lodged with {}, review at {at}",
                    inquiry.title, inquiry.reviewer
                ),
            ));
        }
    }
    rows
}

impl PlayerCase {
    /// The picked accounts of one matter, in the order its rows show them.
    fn picks_for(&self, inquiry: &CaseInquiry) -> Vec<Pick> {
        inquiry
            .accounts
            .iter()
            .map(|account| (account.fact.clone(), account.speaker.clone()))
            .filter(|pick| self.picked.contains(pick))
            .collect()
    }

    /// Put a request to the engine and list it as pending. One the queue will
    /// not take is reported and leaves no row.
    fn send(
        &mut self,
        handle: &BridgeHandle,
        hud: &mut SmartActorHudState,
        command: BridgeCommand,
        row: RequestRow,
    ) {
        if let Err(error) = handle.try_send(command) {
            hud.toast(error);
            return;
        }
        self.list(row);
    }

    /// List what somebody said they would do, as words and nothing more.
    pub fn note_speech(&mut self, speaker: &str, text: &str) {
        self.list(RequestRow {
            request_id: String::new(),
            sent: Vec::new(),
            state: RequestState::Said,
            text: format!("{speaker}: “{text}”"),
        });
    }

    fn list(&mut self, row: RequestRow) {
        if self.requests.len() >= REQUESTS_MAX {
            let oldest_finished = self
                .requests
                .iter()
                .position(|old| old.state != RequestState::Pending)
                .unwrap_or(0);
            self.requests.remove(oldest_finished);
        }
        self.requests.push(row);
    }

    /// Whether the engine still owes this tab an answer to `request_id`.
    pub fn awaits(&self, request_id: &str) -> bool {
        self.requests
            .iter()
            .any(|row| row.request_id == request_id && row.state == RequestState::Pending)
    }

    /// The engine's answer to a request this tab [`awaits`](Self::awaits).
    ///
    /// An ask keeps its row, now carrying what was said. A lodging that was
    /// taken leaves the list, since its matter's dated review is listed in its
    /// place, and what it handed over leaves the picks. A refusal keeps the
    /// row with the reason, and the picks stay for another try.
    pub fn settle(&mut self, request_id: &str, success: bool, message: String) {
        let Some(index) = self
            .requests
            .iter()
            .position(|row| row.request_id == request_id)
        else {
            return;
        };
        if success && !self.requests[index].sent.is_empty() {
            for pick in self.requests.remove(index).sent {
                self.picked.remove(&pick);
            }
            return;
        }
        let row = &mut self.requests[index];
        row.state = if success {
            RequestState::Done
        } else {
            RequestState::BrokenOff
        };
        row.text = message;
    }
}

/// What a press on one of the page's buttons does.
#[derive(Component, Debug, Clone)]
pub(super) enum CaseButton {
    /// Pick an account for lodging, or put it back.
    Account(Pick),
    /// Ask whoever the player is facing about the matter with this id.
    Ask(String),
    /// Lodge the matter's picked accounts with its reviewer.
    Lodge(String),
}

impl CaseButton {
    /// The `Name` the drive's `click` finds the button by.
    fn name(&self) -> String {
        match self {
            Self::Account((fact, speaker)) => format!("Case account: {fact} {speaker}"),
            Self::Ask(inquiry_id) => format!("Case ask: {inquiry_id}"),
            Self::Lodge(inquiry_id) => format!("Case lodge: {inquiry_id}"),
        }
    }
}

/// A button's fill: lifted under the pointer, tinted while its account is
/// picked.
fn fill(interaction: Interaction, picked: bool) -> Color {
    match (interaction, picked) {
        (Interaction::None, false) => BUTTON_BG,
        (Interaction::None, true) => PICKED_BG,
        _ => BUTTON_HOVER,
    }
}

/// Whoever the crosshair rests on, and the name the player has for them.
fn faced(
    focus: Option<&ActorFocus>,
    mirror: Option<&WorldMirror>,
) -> Option<(model::ActorId, String)> {
    let id = &focus?.actor.as_ref()?.actor_id;
    let name = mirror
        .and_then(|mirror| mirror.actor(id))
        .map_or_else(|| id.0.clone(), |actor| actor.name_for_player.clone());
    Some((id.clone(), name))
}

/// Who takes the accounts, and whether a bundle is waiting on them.
fn review_line(inquiry: &CaseInquiry) -> String {
    match (&inquiry.review_at, &inquiry.finding) {
        (Some(at), _) => format!("Reviewer: {} — review at {at}", inquiry.reviewer),
        (None, Some(_)) => format!("Reviewer: {}", inquiry.reviewer),
        (None, None) => format!("Reviewer: {} — nothing lodged yet", inquiry.reviewer),
    }
}

/// Who gave the account, when and where, what it rests on, and where it
/// stands with the player and the reviewer.
fn account_line(account: &CaseAccount, picked: bool) -> String {
    let mut line = format!("{} — {}", account.speaker_name, account.when);
    if let Some(place) = &account.place {
        line.push_str(&format!(", {}", locative(place)));
    }
    line.push_str(&format!(" — {}", account.register));
    if account.lodged {
        line.push_str(" — LODGED");
    }
    if picked {
        line.push_str(" — PICKED");
    }
    line
}

/// One of a matter's two controls.
fn spawn_control(
    parent: &mut ChildSpawnerCommands,
    button: CaseButton,
    label: &str,
    font: &FontSource,
) {
    parent
        .spawn((
            Name::new(button.name()),
            Button,
            button,
            Node {
                flex_shrink: 0.0,
                padding: UiRect::axes(px(14), px(6)),
                border_radius: BorderRadius::all(px(999)),
                ..default()
            },
            BackgroundColor(BUTTON_BG),
        ))
        .with_children(|pill| spawn_text(pill, label, font, 14.0, hud::TEXT));
}

/// A press on the Case page: pick an account or put it back, ask whoever is
/// faced, or lodge the picks. An ask and a lodging go straight to the engine
/// and stay listed as pending until it answers (`CommandResult`, in the drain).
#[allow(clippy::too_many_arguments)]
pub(super) fn case_buttons(
    presses: Query<(&Interaction, &CaseButton), Changed<Interaction>>,
    journal: Res<JournalUiState>,
    runtime: Res<SmartActorRuntime>,
    focus: Res<ActorFocus>,
    mirror: Res<WorldMirror>,
    handle: Res<BridgeHandle>,
    mut interaction: ResMut<InteractionState>,
    mut case: ResMut<PlayerCase>,
    mut hud: ResMut<SmartActorHudState>,
) {
    // The page is built while the Words tab is on show too, where a drive
    // `click` could still reach its buttons.
    if !journal.open || journal.tab != JournalTab::Case || !runtime.interactions_enabled() {
        return;
    }
    for (pressed, button) in &presses {
        if *pressed != Interaction::Pressed {
            continue;
        }
        match button {
            CaseButton::Account(pick) => {
                if !case.picked.remove(pick) {
                    case.picked.insert(pick.clone());
                }
            }
            CaseButton::Ask(inquiry_id) => {
                let Some(inquiry) = case.inquiries.iter().find(|i| i.id == *inquiry_id) else {
                    continue;
                };
                let Some((target_id, name)) = faced(Some(&*focus), Some(&*mirror)) else {
                    hud.toast("Face someone to ask them");
                    continue;
                };
                let text = format!("Asking {name} about “{}”", inquiry.title);
                let request_id = interaction.request_id();
                let command = BridgeCommand::PlayerAsk {
                    request_id: request_id.clone(),
                    target_id,
                    inquiry_id: inquiry_id.clone(),
                };
                let row = RequestRow {
                    request_id,
                    sent: Vec::new(),
                    state: RequestState::Pending,
                    text,
                };
                case.send(&handle, &mut hud, command, row);
            }
            CaseButton::Lodge(inquiry_id) => {
                let Some(inquiry) = case.inquiries.iter().find(|i| i.id == *inquiry_id) else {
                    continue;
                };
                let accounts = case.picks_for(inquiry);
                if accounts.is_empty() {
                    hud.toast("Pick the accounts to lodge first");
                    continue;
                }
                let text = format!(
                    "Lodging {} with {} about “{}”",
                    accounts.len(),
                    inquiry.reviewer,
                    inquiry.title
                );
                let request_id = interaction.request_id();
                let command = BridgeCommand::PlayerLodge {
                    request_id: request_id.clone(),
                    inquiry_id: inquiry_id.clone(),
                    accounts: accounts.clone(),
                };
                let row = RequestRow {
                    request_id,
                    sent: accounts,
                    state: RequestState::Pending,
                    text,
                };
                case.send(&handle, &mut hud, command, row);
            }
        }
    }
}

/// Rebuild only when what the page shows changes, as the Words tab does, so
/// it keeps its place while an account or a finding arrives.
#[allow(clippy::too_many_arguments)]
pub(super) fn refresh_case_tab(
    mut commands: Commands,
    state: Res<JournalUiState>,
    case: Res<PlayerCase>,
    focus: Option<Res<ActorFocus>>,
    mirror: Option<Res<WorldMirror>>,
    fonts: Option<Res<CathedralFonts>>,
    mut roots: Query<(Entity, Option<&Children>, &mut ScrollPosition), With<CaseEntriesRoot>>,
    mut drawn: Local<Option<(PlayerCase, Option<String>)>>,
) {
    let Ok((entity, children, mut scroll)) = roots.single_mut() else {
        return;
    };
    let asking = faced(focus.as_deref(), mirror.as_deref()).map(|(_, name)| name);
    let next = state.open.then_some((&*case, &asking));
    if (*drawn).as_ref().map(|(case, asking)| (case, asking)) == next {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    if let Some((case, asking)) = next {
        let font = fonts
            .as_deref()
            .map(CathedralFonts::body)
            .unwrap_or_default();
        let ask_label = match asking {
            Some(name) => format!("Ask {name} about this"),
            None => "Ask about this".to_string(),
        };
        commands.entity(entity).with_children(|root| {
            if case.inquiries.is_empty() {
                spawn_text(root, "No matters are before you.", &font, 16.0, hud::MUTED);
            }
            for inquiry in &case.inquiries {
                root.spawn(Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    row_gap: px(6),
                    ..default()
                })
                .with_children(|matter| {
                    spawn_text(matter, &inquiry.title, &font, 18.0, hud::TEXT);
                    spawn_text(matter, &review_line(inquiry), &font, 14.0, hud::MUTED);
                    if let Some(finding) = &inquiry.finding {
                        spawn_text(
                            matter,
                            &format!("Reviewed at {}: {}", finding.held, finding.text),
                            &font,
                            16.0,
                            if finding.full {
                                hud::ONLINE
                            } else {
                                hud::DEGRADED
                            },
                        );
                    }
                    for account in &inquiry.accounts {
                        let pick = (account.fact.clone(), account.speaker.clone());
                        let picked = case.picked.contains(&pick);
                        let button = CaseButton::Account(pick);
                        matter
                            .spawn((
                                Name::new(button.name()),
                                Button,
                                button,
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    flex_shrink: 0.0,
                                    row_gap: px(4),
                                    padding: UiRect::axes(px(12), px(8)),
                                    margin: UiRect {
                                        left: px(18),
                                        top: px(2),
                                        ..default()
                                    },
                                    border_radius: BorderRadius::all(px(6)),
                                    ..default()
                                },
                                BackgroundColor(fill(Interaction::None, picked)),
                            ))
                            .with_children(|row| {
                                spawn_text(
                                    row,
                                    &account_line(account, picked),
                                    &font,
                                    14.0,
                                    hud::MUTED,
                                );
                                spawn_text(
                                    row,
                                    &format!("“{}”", account.word),
                                    &font,
                                    16.0,
                                    hud::TEXT,
                                );
                            });
                    }
                    matter
                        .spawn(Node {
                            flex_shrink: 0.0,
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: px(8),
                            row_gap: px(6),
                            margin: UiRect {
                                top: px(4),
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|controls| {
                            spawn_control(
                                controls,
                                CaseButton::Ask(inquiry.id.clone()),
                                &ask_label,
                                &font,
                            );
                            spawn_control(
                                controls,
                                CaseButton::Lodge(inquiry.id.clone()),
                                &format!(
                                    "Lodge {} with {}",
                                    case.picks_for(inquiry).len(),
                                    inquiry.reviewer
                                ),
                                &font,
                            );
                        });
                });
            }
            let rows: Vec<(RequestState, String)> = case
                .requests
                .iter()
                .map(|row| (row.state, row.text.clone()))
                .chain(review_rows(&case.inquiries))
                .collect();
            if !rows.is_empty() {
                root.spawn(Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    row_gap: px(6),
                    ..default()
                })
                .with_children(|list| {
                    spawn_text(list, "REQUESTS", &font, 14.0, hud::MUTED);
                    for (state, text) in &rows {
                        spawn_text(
                            list,
                            &format!("{} — {text}", state.label()),
                            &font,
                            15.0,
                            state.color(),
                        );
                    }
                });
            }
        });
    } else if scroll.0 != Vec2::ZERO {
        scroll.0 = Vec2::ZERO;
    }
    *drawn = next.map(|(case, asking)| (case.clone(), asking.clone()));
}

/// Hover feedback for the page's buttons, over a picked account's own tint.
pub(super) fn tint_case_buttons(
    case: Res<PlayerCase>,
    mut buttons: Query<(&Interaction, &CaseButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, button, mut background) in &mut buttons {
        let picked = matches!(button, CaseButton::Account(pick) if case.picked.contains(pick));
        background.0 = fill(*interaction, picked);
    }
}

#[cfg(test)]
mod tests {
    use cathedral_sim::{ActorId, CaseFinding};

    use super::super::journal_ui::spawn_journal_ui;
    use super::super::targeting::FocusedActor;
    use super::*;

    fn account(speaker: &str, register: &str, lodged: bool) -> CaseAccount {
        CaseAccount {
            fact: FactId::from_raw("dev.barrow"),
            speaker: ActorId::from_raw(speaker),
            speaker_name: speaker.to_string(),
            word: "The barrow was fetched.".into(),
            when: "the Waning, day 2".into(),
            place: Some("The masons' lodge".into()),
            register: register.to_string(),
            lodged,
        }
    }

    fn pick(speaker: &str) -> Pick {
        (FactId::from_raw("dev.barrow"), ActorId::from_raw(speaker))
    }

    fn pending(request_id: &str, sent: Vec<Pick>) -> RequestRow {
        RequestRow {
            request_id: request_id.into(),
            sent,
            state: RequestState::Pending,
            text: format!("Request {request_id}"),
        }
    }

    /// The page's lines, top to bottom.
    fn lines(app: &mut App) -> Vec<String> {
        fn collect(world: &World, entity: Entity, out: &mut Vec<String>) {
            if let Some(text) = world.get::<Text>(entity) {
                out.push(text.0.clone());
            }
            for child in world.get::<Children>(entity).into_iter().flatten() {
                collect(world, *child, out);
            }
        }
        let world = app.world_mut();
        let root = world
            .query_filtered::<Entity, With<CaseEntriesRoot>>()
            .single(world)
            .unwrap();
        let mut out = Vec::new();
        collect(world, root, &mut out);
        out
    }

    #[test]
    fn each_matter_shows_its_review_its_finding_and_who_said_what() {
        let mut app = App::new();
        app.insert_resource(JournalUiState {
            open: true,
            tab: JournalTab::Case,
        })
        .init_resource::<PlayerCase>()
        .add_systems(Startup, spawn_journal_ui)
        .add_systems(Update, refresh_case_tab);
        app.update();
        assert_eq!(lines(&mut app), ["No matters are before you."]);

        let mut noll = account("Noll", "saw it themselves", true);
        noll.place = None;
        app.world_mut().resource_mut::<PlayerCase>().inquiries = vec![
            CaseInquiry {
                id: "dev_scaffold".into(),
                title: "The scaffold pole".into(),
                reviewer: "Ewart Toll".into(),
                review_at: None,
                finding: None,
                accounts: Vec::new(),
            },
            CaseInquiry {
                id: "dev_barrow".into(),
                title: "The lime barrow".into(),
                reviewer: "Ewart Toll".into(),
                review_at: Some("High Wick, day 3".into()),
                finding: Some(CaseFinding {
                    full: false,
                    text: "Not established.".into(),
                    held: "High Wick, day 2".into(),
                }),
                accounts: vec![noll, account("Clemence", "had it from Noll", false)],
            },
        ];
        app.update();
        assert_eq!(
            lines(&mut app),
            [
                "The scaffold pole",
                "Reviewer: Ewart Toll — nothing lodged yet",
                "Ask about this",
                "Lodge 0 with Ewart Toll",
                "The lime barrow",
                "Reviewer: Ewart Toll — review at High Wick, day 3",
                "Reviewed at High Wick, day 2: Not established.",
                "Noll — the Waning, day 2 — saw it themselves — LODGED",
                "“The barrow was fetched.”",
                "Clemence — the Waning, day 2, at The masons' lodge — had it from Noll",
                "“The barrow was fetched.”",
                "Ask about this",
                "Lodge 0 with Ewart Toll",
                "REQUESTS",
                "DONE — The lime barrow: review held at High Wick, day 2",
                "ACCEPTED — The lime barrow: lodged with Ewart Toll, review at High Wick, day 3",
            ]
        );

        // The review has been held and nothing more is lodged.
        app.world_mut().resource_mut::<PlayerCase>().inquiries[1].review_at = None;
        app.update();
        assert_eq!(lines(&mut app)[5], "Reviewer: Ewart Toll");

        // A pick is marked on its row and counted on its matter's lodge
        // control alone; a request and a promise are listed under the matters,
        // ahead of the reviews; and the ask control names whoever the
        // crosshair rests on.
        {
            let mut case = app.world_mut().resource_mut::<PlayerCase>();
            case.picked.insert(pick("Clemence"));
            case.requests.push(pending("rust-1", Vec::new()));
            case.note_speech("Clemence", "I will fetch the reeve.");
        }
        app.insert_resource(ActorFocus {
            actor: Some(FocusedActor {
                actor_id: model::ActorId("b6clm".into()),
                entity: Entity::PLACEHOLDER,
                ray_distance_m: 2.0,
                body_distance_m: 2.0,
            }),
            item: None,
        });
        app.update();
        let shown = lines(&mut app);
        assert_eq!(
            shown[9],
            "Clemence — the Waning, day 2, at The masons' lodge — had it from Noll — PICKED"
        );
        assert_eq!(
            shown
                .iter()
                .filter(|line| line.starts_with("Lodge "))
                .collect::<Vec<_>>(),
            ["Lodge 0 with Ewart Toll", "Lodge 1 with Ewart Toll"]
        );
        assert_eq!(
            shown
                .iter()
                .filter(|line| *line == "Ask b6clm about this")
                .count(),
            2
        );
        assert_eq!(
            shown[shown.len() - 4..],
            [
                "REQUESTS",
                "PENDING — Request rust-1",
                "SAID — words only — Clemence: “I will fetch the reeve.”",
                "DONE — The lime barrow: review held at High Wick, day 2",
            ]
        );

        app.world_mut().resource_mut::<JournalUiState>().open = false;
        app.update();
        assert!(lines(&mut app).is_empty());
    }

    #[test]
    fn a_promise_is_heard_in_i_will_ill_and_i_shall() {
        for line in [
            "I will fetch the reeve.",
            "Wait here, I'll see to it.",
            "That I SHALL not do.",
        ] {
            assert!(sounds_like_a_promise(line), "{line}");
        }
        for line in ["The reeve will hear of it.", "Will you fetch the reeve?"] {
            assert!(!sounds_like_a_promise(line), "{line}");
        }
    }

    #[test]
    fn only_a_new_account_and_a_new_review_are_news_and_the_review_comes_last() {
        let matter = |id: &str, title: &str| CaseInquiry {
            id: id.into(),
            title: title.into(),
            reviewer: "Ewart Toll".into(),
            review_at: None,
            finding: None,
            accounts: vec![account("Noll", "saw it themselves", false)],
        };
        let before = vec![
            matter("dev_scaffold", "The scaffold pole"),
            matter("dev_barrow", "The lime barrow"),
        ];
        let mut after = before.clone();
        assert!(notices(&before, &after).is_empty());

        // Asked again, lodged and awaiting review: the same account still.
        after[0].accounts[0].when = "Lamplight, day 2".into();
        after[0].accounts[0].lodged = true;
        after[0].review_at = Some("High Wick, day 3".into());
        assert!(notices(&before, &after).is_empty());

        let held = |day: u32| CaseFinding {
            full: false,
            text: "Not established.".into(),
            held: format!("High Wick, day {day}"),
        };
        after[0].finding = Some(held(3));
        after[1]
            .accounts
            .push(account("Clemence", "had it from Noll", false));
        assert_eq!(
            notices(&before, &after),
            [
                "New account: Clemence on The lime barrow",
                "The review of The scaffold pole was held",
            ]
        );

        // A second review of the same matter is news again.
        let before = after.clone();
        after[0].finding = Some(held(4));
        assert_eq!(
            notices(&before, &after),
            ["The review of The scaffold pole was held"]
        );
        // And to a record that had nothing, everything is.
        assert_eq!(notices(&[], &after).len(), 4);
    }

    #[test]
    fn an_answer_settles_its_request_and_only_a_taken_lodging_spends_its_picks() {
        let mut case = PlayerCase::default();
        case.picked.extend([pick("Clemence"), pick("Noll")]);
        case.requests = vec![
            pending("ask", Vec::new()),
            pending("refused", vec![pick("Clemence")]),
            pending("taken", vec![pick("Clemence")]),
        ];
        assert!(case.awaits("ask") && !case.awaits("somebody else's"));

        case.settle("ask", true, "Noll tells you: \"It was fetched.\"".into());
        assert!(!case.awaits("ask"));
        assert_eq!(case.requests[0].state, RequestState::Done);
        assert_eq!(case.requests[0].text, "Noll tells you: \"It was fetched.\"");

        case.settle("refused", false, "Ewart Toll is not here to take it".into());
        assert_eq!(case.requests[1].state, RequestState::BrokenOff);
        assert_eq!(case.requests[1].text, "Ewart Toll is not here to take it");
        assert_eq!(case.picked.len(), 2, "a refusal keeps the draft");

        case.settle("taken", true, "Ewart Toll takes 1 account".into());
        assert_eq!(case.requests.len(), 2);
        assert_eq!(case.picked, BTreeSet::from([pick("Noll")]));
    }

    #[test]
    fn a_full_request_list_drops_its_oldest_finished_row_first() {
        let (sender, _commands) = crossbeam_channel::bounded(32);
        let handle = BridgeHandle::new(sender, "/tmp".into());
        let mut hud = SmartActorHudState::default();
        let mut case = PlayerCase::default();
        let mut send = |case: &mut PlayerCase, id: usize| {
            let request_id = format!("r{id}");
            let command = BridgeCommand::PlayerAsk {
                request_id: request_id.clone(),
                target_id: model::ActorId("b6clm".into()),
                inquiry_id: "dev_barrow".into(),
            };
            case.send(&handle, &mut hud, command, pending(&request_id, Vec::new()));
        };
        for id in 0..REQUESTS_MAX {
            send(&mut case, id);
        }
        case.settle("r3", false, "too far".into());
        send(&mut case, REQUESTS_MAX);
        let ids: Vec<&str> = case
            .requests
            .iter()
            .map(|row| row.request_id.as_str())
            .collect();
        assert_eq!(ids.len(), REQUESTS_MAX);
        assert!(!ids.contains(&"r3") && ids.contains(&"r0"), "{ids:?}");
        assert_eq!(ids.last(), Some(&"r12"));
    }
}
