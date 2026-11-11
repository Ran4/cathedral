//! The journal's Case tab: every matter before the player, where its review
//! stands, and the accounts people have given of it.

use bevy::prelude::*;
use cathedral_sim::{CaseAccount, CaseInquiry};

use crate::fonts::CathedralFonts;

use super::hud;
use super::journal_ui::{CaseEntriesRoot, JournalUiState, locative, spawn_text};

/// The player's record of each inquiry, as the sim last published it. The sim
/// has already resolved every name through the player, so the rows are kept
/// as they came and only joined into lines when drawn.
#[derive(Resource, Debug, Default)]
pub(super) struct PlayerCase {
    pub inquiries: Vec<CaseInquiry>,
}

/// Who takes the accounts, and whether a bundle is waiting on them.
fn review_line(inquiry: &CaseInquiry) -> String {
    match (&inquiry.review_at, &inquiry.finding) {
        (Some(at), _) => format!("Reviewer: {} — review at {at}", inquiry.reviewer),
        (None, Some(_)) => format!("Reviewer: {}", inquiry.reviewer),
        (None, None) => format!("Reviewer: {} — nothing lodged yet", inquiry.reviewer),
    }
}

/// Who gave the account, when and where, and what it rests on.
fn account_line(account: &CaseAccount) -> String {
    let mut line = format!("{} — {}", account.speaker_name, account.when);
    if let Some(place) = &account.place {
        line.push_str(&format!(", {}", locative(place)));
    }
    line.push_str(&format!(" — {}", account.register));
    if account.lodged {
        line.push_str(" — LODGED");
    }
    line
}

/// Rebuild only when the record changes, as the Words tab does, so the page
/// keeps its place while an account or a finding arrives.
pub(super) fn refresh_case_tab(
    mut commands: Commands,
    state: Res<JournalUiState>,
    case: Res<PlayerCase>,
    fonts: Option<Res<CathedralFonts>>,
    mut roots: Query<(Entity, Option<&Children>, &mut ScrollPosition), With<CaseEntriesRoot>>,
    mut drawn: Local<Option<Vec<CaseInquiry>>>,
) {
    let Ok((entity, children, mut scroll)) = roots.single_mut() else {
        return;
    };
    let next = state.open.then_some(case.inquiries.as_slice());
    if drawn.as_deref() == next {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    if let Some(inquiries) = next {
        let font = fonts
            .as_deref()
            .map(CathedralFonts::body)
            .unwrap_or_default();
        commands.entity(entity).with_children(|root| {
            if inquiries.is_empty() {
                spawn_text(root, "No matters are before you.", &font, 16.0, hud::MUTED);
            }
            for inquiry in inquiries {
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
                        matter
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                flex_shrink: 0.0,
                                row_gap: px(4),
                                margin: UiRect {
                                    left: px(18),
                                    top: px(8),
                                    ..default()
                                },
                                ..default()
                            })
                            .with_children(|row| {
                                spawn_text(row, &account_line(account), &font, 14.0, hud::MUTED);
                                spawn_text(
                                    row,
                                    &format!("“{}”", account.word),
                                    &font,
                                    16.0,
                                    hud::TEXT,
                                );
                            });
                    }
                });
            }
        });
    } else if scroll.0 != Vec2::ZERO {
        scroll.0 = Vec2::ZERO;
    }
    *drawn = next.map(<[CaseInquiry]>::to_vec);
}

#[cfg(test)]
mod tests {
    use cathedral_sim::{ActorId, CaseFinding, FactId};

    use super::super::journal_ui::{JournalTab, spawn_journal_ui};
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
                "The lime barrow",
                "Reviewer: Ewart Toll — review at High Wick, day 3",
                "Reviewed at High Wick, day 2: Not established.",
                "Noll — the Waning, day 2 — saw it themselves — LODGED",
                "“The barrow was fetched.”",
                "Clemence — the Waning, day 2, at The masons' lodge — had it from Noll",
                "“The barrow was fetched.”",
            ]
        );

        // The review has been held and nothing more is lodged.
        app.world_mut().resource_mut::<PlayerCase>().inquiries[1].review_at = None;
        app.update();
        assert_eq!(lines(&mut app)[3], "Reviewer: Ewart Toll");

        app.world_mut().resource_mut::<JournalUiState>().open = false;
        app.update();
        assert!(lines(&mut app).is_empty());
    }
}
