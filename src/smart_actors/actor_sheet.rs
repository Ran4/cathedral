//! Developer character-sheet overlay for the last NPC the player looked at.
//!
//! This is part of the same `B` debug mode as [`super::area_debug`] and shares
//! its one [`AreaDebugState`] toggle. While the layer is on, the NPC currently —
//! or most recently — under the crosshair is *inspected*: a right-hand panel
//! (about a fifth of the viewport wide) shows the authoritative simulation state
//! for that character, its otherwise-private goal and dynamic need statuses
//! included. Like the area layer it never reads a projected snapshot; it borrows
//! the live [`cathedral_sim::World`] out of [`LocalEngine`], so the sheet is a
//! view of the sim rather than a second copy of it.
//!
//! The subject is *sticky*: looking away leaves the last person on screen, and
//! looking at someone new replaces them. Turning the layer off (pressing `B`
//! again) clears the subject, so re-opening starts blank.

use bevy::prelude::*;
use cathedral_sim::{
    Character, LoreProfile, Significance, THIRST_MAX, THIRST_PARCHED, THIRST_THIRSTY,
    Vec3 as SimVec3, World,
};

use crate::fonts::CathedralFonts;

use super::{area_debug::AreaDebugState, hud, local_engine::LocalEngine, model::ActorId,
    targeting::ActorFocus};

/// Fraction of the viewport width the sheet occupies — "about 1/5".
const SHEET_WIDTH_PERCENT: f32 = 20.0;
/// Cleared below the top-right actor-status panel so the two never overlap.
const SHEET_TOP_PX: f32 = 88.0;

// Font sizes, 50% larger than the first pass for legibility across the room.
const LABEL_FONT_PX: f32 = 17.25;
const NAME_FONT_PX: f32 = 28.5;
const BODY_FONT_PX: f32 = 18.75;

/// Width of the thirst (and future gauge) bars, in cells.
const GAUGE_CELLS: usize = 16;
/// Newest history lines shown, and the per-line character cap that keeps the
/// narrow column from wrapping a percept into a wall of text.
const RECENT_LINES: usize = 3;
const RECENT_LINE_CHARS: usize = 56;

/// The NPC whose sheet is pinned on screen. Sticky: set whenever the debug layer
/// is on and an actor is under the crosshair, cleared only when the layer turns
/// off. See [`next_subject`] for the exact transition.
#[derive(Resource, Debug, Default)]
pub(super) struct InspectedActor {
    actor_id: Option<ActorId>,
}

#[derive(Component)]
pub(super) struct ActorSheetPanel;
#[derive(Component)]
pub(super) struct ActorSheetName;
#[derive(Component)]
pub(super) struct ActorSheetBody;

/// A character's live movement, reduced to the three states the sheet names.
#[derive(Debug, Clone, PartialEq)]
enum MoveState {
    /// No [`cathedral_sim::Movement`] at all — the character never walks.
    Still,
    /// Has a mover but the path emptied: standing where it last arrived.
    Arrived,
    /// Actively walking, with waypoints still to reach.
    Walking { speed: f64, waypoints: usize },
}

/// The authored biography behind the identity header. Lore-less fixtures (the
/// player, test doubles) have none, and the header falls back to the bare name.
#[derive(Debug, Clone, PartialEq)]
struct Bio {
    title: Option<String>,
    /// Humanized from `occupation_id` (`"scribe_and_clerk"` → `"Scribe and
    /// clerk"`), per the requested `{occupation_id.replace("_"," ").title()}`.
    occupation: Option<String>,
    age: u16,
    /// Single letter in the lore data (`"f"` / `"m"`); shown upper-cased.
    gender: String,
    district: String,
}

/// An owned, render-ready projection of one character's authoritative state:
/// everything the sheet shows, with item names and location already resolved out
/// of the live world so the formatters stay pure and unit-testable.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CharacterDebug {
    name: String,
    id: String,
    control_is_llm: bool,
    significance: Significance,
    bio: Option<Bio>,
    location: Option<String>,
    goal: String,
    thirst: f64,
    movement: MoveState,
    position: SimVec3,
    facing_yaw: f64,
    holds: Vec<String>,
    memory_count: usize,
    recent: Vec<String>,
}

impl CharacterDebug {
    /// Read one character out of the live world, resolving the ids it references
    /// (held items, its named location) against the same world.
    fn from_world(world: &World, character: &Character) -> Self {
        let holds = character
            .holds()
            .iter()
            .map(|item_id| {
                world
                    .items
                    .get(item_id)
                    .map_or_else(|| item_id.as_str().to_string(), |item| item.name.clone())
            })
            .collect();
        let movement = match character.state.movement.as_ref() {
            None => MoveState::Still,
            Some(movement) if movement.path.is_empty() => MoveState::Arrived,
            Some(movement) => MoveState::Walking {
                speed: movement.speed,
                waypoints: movement.path.len(),
            },
        };
        let recent = character
            .recent_history()
            .iter()
            .rev()
            .take(RECENT_LINES)
            .rev()
            .cloned()
            .collect();
        Self {
            name: character.name().to_string(),
            id: character.id().as_str().to_string(),
            control_is_llm: character.control().is_llm(),
            significance: character.significance(),
            bio: character.lore().map(bio_from_lore),
            location: world.area_map.location_description(character.position_m()),
            goal: character.goal().to_string(),
            thirst: character.needs().thirst,
            movement,
            position: character.position_m(),
            facing_yaw: character.facing_yaw(),
            holds,
            memory_count: character.memories().len(),
            recent,
        }
    }
}

fn bio_from_lore(lore: &LoreProfile) -> Bio {
    Bio {
        title: lore.title.clone(),
        occupation: lore.occupation_id.as_deref().map(humanize_occupation),
        age: lore.age,
        gender: lore.gender.clone(),
        district: lore.district.clone(),
    }
}

/// `"scribe_and_clerk"` → `"Scribe and clerk"`: underscores to spaces, first
/// letter capitalized (ids are already lower-case, so the rest is left as-is).
fn humanize_occupation(id: &str) -> String {
    let spaced = id.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// The role qualifier after the name: `"Verger (Church attendant)"`, or just the
/// title, or just the occupation, or nothing when neither is authored.
fn role_phrase(title: &Option<String>, occupation: &Option<String>) -> Option<String> {
    match (title, occupation) {
        (Some(title), Some(occupation)) => Some(format!("{title} ({occupation})")),
        (Some(one), None) | (None, Some(one)) => Some(one.clone()),
        (None, None) => None,
    }
}

/// The big header line: `"Dunstan Pike, Verger (Church attendant)"`, or the bare
/// name when there is no authored role.
fn identity_line(sheet: &CharacterDebug) -> String {
    match sheet
        .bio
        .as_ref()
        .and_then(|bio| role_phrase(&bio.title, &bio.occupation))
    {
        Some(role) => format!("{}, {role}", sheet.name),
        None => sheet.name.clone(),
    }
}

fn significance_label(significance: Significance) -> &'static str {
    match significance {
        Significance::Major => "MAJOR",
        Significance::Minor => "MINOR",
        Significance::Ambient => "AMBIENT",
    }
}

/// A coarse, human-readable name for the thirst gauge, matching the thresholds
/// the behaviour ladder itself reads.
fn thirst_label(thirst: f64) -> &'static str {
    if thirst < THIRST_PARCHED {
        "PARCHED"
    } else if thirst < THIRST_THIRSTY {
        "THIRSTY"
    } else {
        "WATERED"
    }
}

/// A `████░░░░░░` fill bar for a `0..=1` fraction.
fn bar(fraction: f64, cells: usize) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * cells as f64).round() as usize;
    let filled = filled.min(cells);
    (0..cells)
        .map(|cell| if cell < filled { '█' } else { '░' })
        .collect()
}

fn move_summary(movement: &MoveState) -> String {
    match movement {
        MoveState::Still => "Stationary (never walks)".to_string(),
        MoveState::Arrived => "Arrived · idle".to_string(),
        MoveState::Walking { speed, waypoints } => {
            format!("Walking · {speed:.1} m/s · {waypoints} waypoint(s) left")
        }
    }
}

/// Shorten a percept to keep one line one line in the narrow column.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}

/// Render the body block. The identity line is drawn in its own larger node, so
/// this starts at the demographic line beneath it.
fn format_body(sheet: &CharacterDebug) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();

    // "43M, The Lanthorn" — age, gender initial, home district.
    if let Some(bio) = &sheet.bio {
        let _ = writeln!(
            out,
            "{}{}, {}",
            bio.age,
            bio.gender.to_uppercase(),
            bio.district
        );
    }
    // The developer identity line: importance, who decides its actions, its id.
    let _ = write!(
        out,
        "{} · {} · {}",
        significance_label(sheet.significance),
        if sheet.control_is_llm { "LLM" } else { "PLAYER" },
        sheet.id,
    );
    if let Some(location) = &sheet.location {
        let _ = write!(out, "\n@ {location}");
    }

    let _ = write!(out, "\n\nGOAL\n  {}", sheet.goal);

    // Each status is a labelled value with its own fill bar on the next line, so
    // the level reads at a glance.
    let _ = write!(
        out,
        "\n\nSTATUSES\n  Thirst  {:.0}/{:.0} · {}\n  {}",
        sheet.thirst,
        THIRST_MAX,
        thirst_label(sheet.thirst),
        bar(sheet.thirst / THIRST_MAX, GAUGE_CELLS),
    );

    let _ = write!(out, "\n\nMOVEMENT\n  {}", move_summary(&sheet.movement));

    let _ = write!(
        out,
        "\n\nPOSE\n  x {:.1}  y {:.1}  z {:.1}\n  yaw {:.2} rad",
        sheet.position.x, sheet.position.y, sheet.position.z, sheet.facing_yaw,
    );

    let holds = if sheet.holds.is_empty() {
        "(empty-handed)".to_string()
    } else {
        sheet.holds.join(", ")
    };
    let _ = write!(out, "\n\nHOLDS\n  {holds}");

    let _ = write!(out, "\n\nMEMORY {} stored", sheet.memory_count);
    if !sheet.recent.is_empty() {
        let _ = write!(out, "\nRECENT");
        for line in &sheet.recent {
            let _ = write!(out, "\n  · {}", truncate(line, RECENT_LINE_CHARS));
        }
    }

    out
}

/// Which subject the sheet pins this frame: the crosshair actor if there is one,
/// otherwise whoever was pinned before (sticky). The caller clears `current` to
/// `None` before this is ever reached while the layer is off.
fn next_subject(current: Option<ActorId>, focused: Option<&ActorId>) -> Option<ActorId> {
    focused.cloned().or(current)
}

/// Shown while the layer is on but no NPC has been looked at yet.
const EMPTY_HINT: &str = "Aim the crosshair at an NPC.\n\nTheir sheet stays until you look at\nsomeone else, or press B to close.";

pub(super) fn spawn_actor_sheet(mut commands: Commands, fonts: Option<Res<CathedralFonts>>) {
    let body_font = fonts
        .as_deref()
        .map(CathedralFonts::body)
        .unwrap_or_default();
    let display_font = fonts
        .as_deref()
        .map(CathedralFonts::display)
        .unwrap_or_default();

    commands
        .spawn((
            Name::new("Actor debug sheet"),
            ActorSheetPanel,
            Node {
                position_type: PositionType::Absolute,
                top: px(SHEET_TOP_PX),
                right: px(0),
                width: percent(SHEET_WIDTH_PERCENT),
                max_height: percent(86),
                padding: UiRect::axes(px(14), px(12)),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                overflow: Overflow::clip(),
                // Rounded on the inner (left) edge only; the panel sits flush to
                // the right screen edge.
                border_radius: BorderRadius {
                    top_left: px(9),
                    bottom_left: px(9),
                    top_right: px(0),
                    bottom_right: px(0),
                },
                ..default()
            },
            BackgroundColor(hud::PANEL),
            ZIndex(21),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                Name::new("Actor sheet mode label"),
                Text::new("CHARACTER DEBUG"),
                TextFont {
                    font: display_font.clone(),
                    font_size: FontSize::Px(LABEL_FONT_PX),
                    ..default()
                },
                TextColor(hud::MUTED),
            ));
            panel.spawn((
                Name::new("Actor sheet name"),
                ActorSheetName,
                Text::new("CHARACTER DEBUG"),
                TextFont {
                    font: display_font,
                    font_size: FontSize::Px(NAME_FONT_PX),
                    ..default()
                },
                TextColor(hud::TEXT),
                TextShadow::default(),
            ));
            panel.spawn((
                Name::new("Actor sheet body"),
                ActorSheetBody,
                Text::new(EMPTY_HINT),
                TextFont {
                    font: body_font,
                    font_size: FontSize::Px(BODY_FONT_PX),
                    ..default()
                },
                TextColor(hud::TEXT),
                TextLayout::justify(Justify::Left),
            ));
        });
}

type NameQuery<'w, 's> =
    Query<'w, 's, &'static mut Text, (With<ActorSheetName>, Without<ActorSheetBody>)>;
type BodyQuery<'w, 's> =
    Query<'w, 's, &'static mut Text, (With<ActorSheetBody>, Without<ActorSheetName>)>;

/// Pin the crosshair subject, resolve its live state, and paint the sheet. Runs
/// after [`super::area_debug::update_area_debug_ui`] toggles the shared layer, so
/// the enabled flag it reads is this frame's, and after focus is updated, so the
/// subject is current.
pub(super) fn update_actor_sheet(
    debug: Res<AreaDebugState>,
    focus: Res<ActorFocus>,
    engine: NonSend<LocalEngine>,
    mut inspected: ResMut<InspectedActor>,
    mut panel: Query<&mut Visibility, With<ActorSheetPanel>>,
    mut name: NameQuery,
    mut body: BodyQuery,
) {
    let Ok(mut panel_visibility) = panel.single_mut() else {
        return;
    };

    if !debug.is_enabled() {
        inspected.actor_id = None;
        *panel_visibility = Visibility::Hidden;
        return;
    }
    *panel_visibility = Visibility::Inherited;

    inspected.actor_id = next_subject(
        inspected.actor_id.take(),
        focus.actor.as_ref().map(|actor| &actor.actor_id),
    );

    let (Ok(mut name_text), Ok(mut body_text)) = (name.single_mut(), body.single_mut()) else {
        return;
    };

    let sheet = inspected.actor_id.as_ref().and_then(|id| {
        let sim_id = cathedral_sim::ActorId::from_raw(id.0.clone());
        engine.world().and_then(|world| {
            world
                .characters
                .get(&sim_id)
                .map(|character| CharacterDebug::from_world(world, character))
        })
    });

    match sheet {
        Some(sheet) => {
            name_text.0 = identity_line(&sheet);
            body_text.0 = format_body(&sheet);
        }
        // A subject was pinned but no longer resolves — it left the world, or the
        // cast is not online yet. Keep the panel up with a clear reason.
        None if inspected.actor_id.is_some() => {
            name_text.0 = "(subject unavailable)".to_string();
            body_text.0 = String::new();
        }
        None => {
            // The "CHARACTER DEBUG" mode label above already titles the panel;
            // keep the big name line empty until there is a subject to name.
            name_text.0 = String::new();
            body_text.0 = EMPTY_HINT.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CharacterDebug {
        CharacterDebug {
            name: "Dunstan Pike".into(),
            id: "cb947".into(),
            control_is_llm: true,
            significance: Significance::Major,
            bio: Some(Bio {
                title: Some("Verger".into()),
                occupation: Some(humanize_occupation("church_attendant")),
                age: 43,
                gender: "m".into(),
                district: "The Lanthorn".into(),
            }),
            location: Some("The Gradine".into()),
            goal: "Fetch water from the well".into(),
            thirst: 40.0,
            movement: MoveState::Walking {
                speed: 1.2,
                waypoints: 3,
            },
            position: SimVec3::new(12.34, 0.0, -45.6),
            facing_yaw: 1.5707,
            holds: vec!["a clay jug".into()],
            memory_count: 5,
            recent: vec!["The bell rang for Sext".into()],
        }
    }

    #[test]
    fn sticky_subject_prefers_crosshair_then_holds_then_clears() {
        let a = ActorId("a".into());
        let b = ActorId("b".into());
        // First look pins the crosshair actor.
        assert_eq!(next_subject(None, Some(&a)), Some(a.clone()));
        // Looking away keeps the last subject.
        assert_eq!(next_subject(Some(a.clone()), None), Some(a.clone()));
        // Looking at someone new replaces it.
        assert_eq!(next_subject(Some(a), Some(&b)), Some(b));
        // Nothing pinned, nothing focused stays empty.
        assert_eq!(next_subject(None, None), None);
    }

    #[test]
    fn occupation_is_humanized_sentence_case() {
        assert_eq!(humanize_occupation("church_attendant"), "Church attendant");
        assert_eq!(humanize_occupation("scribe_and_clerk"), "Scribe and clerk");
        assert_eq!(humanize_occupation("verger"), "Verger");
        assert_eq!(humanize_occupation(""), "");
    }

    #[test]
    fn identity_line_matches_the_requested_header() {
        assert_eq!(
            identity_line(&sample()),
            "Dunstan Pike, Verger (Church attendant)"
        );
        // Title alone, occupation alone, and neither, all degrade gracefully.
        let mut only_occupation = sample();
        if let Some(bio) = only_occupation.bio.as_mut() {
            bio.title = None;
        }
        assert_eq!(
            identity_line(&only_occupation),
            "Dunstan Pike, Church attendant"
        );
        let mut nameless_role = sample();
        nameless_role.bio = None;
        assert_eq!(identity_line(&nameless_role), "Dunstan Pike");
    }

    #[test]
    fn demographic_line_reads_age_gender_district() {
        assert!(format_body(&sample()).starts_with("43M, The Lanthorn\n"));
    }

    #[test]
    fn thirst_label_matches_ladder_thresholds() {
        assert_eq!(thirst_label(THIRST_PARCHED - 1.0), "PARCHED");
        assert_eq!(thirst_label(THIRST_PARCHED), "THIRSTY");
        assert_eq!(thirst_label(THIRST_THIRSTY - 1.0), "THIRSTY");
        assert_eq!(thirst_label(THIRST_THIRSTY), "WATERED");
        assert_eq!(thirst_label(THIRST_MAX), "WATERED");
    }

    #[test]
    fn bar_fills_and_clamps() {
        assert_eq!(bar(0.0, 4), "░░░░");
        assert_eq!(bar(0.5, 4), "██░░");
        assert_eq!(bar(1.0, 4), "████");
        assert_eq!(bar(2.0, 4), "████");
        assert_eq!(bar(-1.0, 4), "░░░░");
    }

    #[test]
    fn truncate_caps_long_lines_with_an_ellipsis() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }

    #[test]
    fn body_renders_every_section_including_a_status_bar() {
        let body = format_body(&sample());
        for expected in [
            "43M, The Lanthorn",
            "MAJOR · LLM · cb947",
            "@ The Gradine",
            "GOAL\n  Fetch water from the well",
            "STATUSES",
            "Thirst  40/255 · THIRSTY", // 40 is between PARCHED (38) and THIRSTY (178)
            "░", // the gauge bar is present
            "MOVEMENT\n  Walking · 1.2 m/s · 3 waypoint(s) left",
            "POSE",
            "x 12.3",
            "z -45.6",
            "yaw 1.57 rad",
            "HOLDS\n  a clay jug",
            "MEMORY 5 stored",
            "RECENT",
            "The bell rang for Sext",
        ] {
            assert!(
                body.contains(expected),
                "sheet body is missing {expected:?}\n---\n{body}"
            );
        }
    }

    #[test]
    fn empty_hands_and_still_mover_read_clearly() {
        let mut sheet = sample();
        sheet.holds.clear();
        sheet.movement = MoveState::Still;
        let body = format_body(&sheet);
        assert!(body.contains("HOLDS\n  (empty-handed)"));
        assert!(body.contains("MOVEMENT\n  Stationary (never walks)"));
    }
}
