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
    Character, ErrandDebug, HUNGER_FAMISHED, HUNGER_HUNGRY, HUNGER_MAX, IntentTarget, LoreProfile,
    RoundPhase, Significance, THIRST_MAX, THIRST_PARCHED, THIRST_THIRSTY, Vec3 as SimVec3,
    WALK_DESTINATION_SNAP_M, World,
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

/// Radius within which a walk endpoint counts as *at* a registered place. The
/// routed path ends at a snapped nav node rather than the place's own point,
/// so this is deliberately wider than the arrival radii. Shared with the
/// prompt's `you_are` walk line, so the actor and this overlay always name
/// the same destination.
const DESTINATION_SNAP_M: f64 = WALK_DESTINATION_SNAP_M;

/// The live walk, resolved against the world: where it ends by name, how much
/// polyline is left, and the real-time ETA at the current speed.
#[derive(Debug, Clone, PartialEq)]
struct Heading {
    /// Best name first: the `go_to` target, the assigned well, the nearest
    /// registered place, the area label, or bare coordinates.
    destination: String,
    /// Which layer aims the feet, when the round knows.
    reason: Option<&'static str>,
    /// Metres left along the waypoints, in the walk (XZ) plane.
    distance_m: f64,
    /// `distance_m / speed` in real seconds; `None` if the speed is zero.
    eta_seconds: Option<f64>,
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
    hunger: f64,
    /// The same actor-perspective prose used by the prompt, plus the coarse
    /// authoritative wetness band for developer diagnosis.
    weather: Option<String>,
    movement: MoveState,
    /// The current walk's destination and ETA; `None` while standing.
    heading: Option<Heading>,
    /// The round activity while standing at a well (queued / drawing), which
    /// says more than "Arrived · idle".
    well_activity: Option<String>,
    /// A live `go_to` the feet are *not* currently walking (held by a
    /// conversation, or waiting for the next ladder decision), by name.
    pending_intent: Option<String>,
    position: SimVec3,
    facing_yaw: f64,
    holds: Vec<String>,
    memory_count: usize,
    recent: Vec<String>,
}

impl CharacterDebug {
    /// Read one character out of the live world, resolving the ids it references
    /// (held items, its named location, its walk destination) against the same
    /// world. `errand` is the round's view of the same person, when enrolled.
    fn from_world(world: &World, character: &Character, errand: Option<&ErrandDebug>) -> Self {
        let holds = character
            .holds()
            .iter()
            .map(|item_id| {
                world.items.get(item_id).map_or_else(
                    || item_id.as_str().to_string(),
                    |item| {
                        let name = world.item_catalog.display_name(item);
                        if item.quantity > 1 {
                            format!("{name} ×{}", item.quantity)
                        } else {
                            name
                        }
                    },
                )
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
        let heading = heading_of(world, character, errand);
        // An intent whose walk is already under way is named on the heading
        // line; anything else is still pending and gets its own line.
        let pending_intent = character.state.intent.as_ref().and_then(|intent| {
            let walking_it = heading
                .as_ref()
                .is_some_and(|heading| heading.reason == Some("go_to"));
            (!walking_it).then(|| intent_target_name(world, &intent.target))
        });
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
            hunger: character.needs().hunger,
            weather: world.current_weather.map(|weather| {
                format!(
                    "{} · wetness: {}",
                    weather.prompt_phrase(world.shelters.label_at(character.position_m())),
                    weather.wetness_band(),
                )
            }),
            movement,
            heading,
            well_activity: well_activity(errand),
            pending_intent,
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

/// A coarse name for the hunger gauge, matching the ladder's rungs 3 & 7 and the
/// sheet's computed `famished`/`hungry` condition (food & items M2).
fn hunger_label(hunger: f64) -> &'static str {
    if hunger < HUNGER_FAMISHED {
        "FAMISHED"
    } else if hunger < HUNGER_HUNGRY {
        "HUNGRY"
    } else {
        "FED"
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

/// Resolve the live walk into a [`Heading`]: name the destination, measure the
/// path left, and price it in real seconds. `None` when there is no walk.
fn heading_of(world: &World, character: &Character, errand: Option<&ErrandDebug>) -> Option<Heading> {
    let movement = character.state.movement.as_ref()?;
    let final_point = *movement.path.last()?;
    let distance_m = path_length_m(character.position_m(), &movement.path);
    let eta_seconds = (movement.speed > 0.0).then(|| distance_m / movement.speed);
    // The most specific name wins: the go_to target this walk serves, the
    // assigned well, then whatever the walk's endpoint resolves to.
    let (destination, reason) = match errand {
        Some(errand) if errand.for_intent && character.state.intent.is_some() => {
            let intent = character.state.intent.as_ref().expect("checked in the guard");
            (intent_target_name(world, &intent.target), Some("go_to"))
        }
        Some(errand) if errand.phase == RoundPhase::Approaching => (
            errand.well.clone().unwrap_or_else(|| "their well".to_string()),
            Some("water round"),
        ),
        Some(errand) if errand.phase == RoundPhase::Returning => {
            (name_point(world, final_point), Some("delivering water"))
        }
        Some(errand) => (
            name_point(world, errand.walk_target.unwrap_or(final_point)),
            Some("daily round"),
        ),
        None => (name_point(world, final_point), None),
    };
    Some(Heading {
        destination,
        reason,
        distance_m,
        eta_seconds,
    })
}

/// Metres left along the waypoint polyline, measured in the walk (XZ) plane
/// exactly like the mover itself.
fn path_length_m(from: SimVec3, path: &[SimVec3]) -> f64 {
    let mut total = 0.0;
    let mut previous = from;
    for point in path {
        total += f64::hypot(point.x - previous.x, point.z - previous.z);
        previous = *point;
    }
    total
}

/// The best available name for a walk target: the nearest registered place
/// (homes included) within the snap radius, else the area label, else bare
/// coordinates.
fn name_point(world: &World, point: SimVec3) -> String {
    if let Some(place) = world.places.nearest(point, DESTINATION_SNAP_M) {
        return place.name.clone();
    }
    world
        .area_map
        .location_description(point)
        .unwrap_or_else(|| format!("x {:.0}, z {:.0}", point.x, point.z))
}

/// The `go_to` target as the sheet names it — the true names, this being a
/// debug view: `"The Gradine"`, `"Tam Rud (in sight)"`, `"Tam Rud (last seen)"`.
fn intent_target_name(world: &World, target: &IntentTarget) -> String {
    match target {
        IntentTarget::Place { name, .. } => name.clone(),
        IntentTarget::Person {
            actor_id, visible, ..
        } => {
            let name = world.characters.get(actor_id).map_or_else(
                || actor_id.as_str().to_string(),
                |person| person.name().to_string(),
            );
            if *visible {
                format!("{name} (in sight)")
            } else {
                format!("{name} (last seen)")
            }
        }
    }
}

/// The standing well activity — queued (with standing) or drawing — or `None`
/// in every phase the mover state already narrates.
fn well_activity(errand: Option<&ErrandDebug>) -> Option<String> {
    let errand = errand?;
    let well = errand.well.as_deref().unwrap_or("the well");
    match errand.phase {
        RoundPhase::Queued => Some(match errand.ahead_in_queue {
            Some(0) => format!("Queued at {well} · next up"),
            Some(ahead) => format!("Queued at {well} · {ahead} ahead"),
            None => format!("Queued at {well}"),
        }),
        RoundPhase::Drawing => Some(format!("Drawing water at {well}")),
        _ => None,
    }
}

fn move_summary(movement: &MoveState, well_activity: Option<&str>) -> String {
    if let Some(activity) = well_activity
        && !matches!(movement, MoveState::Walking { .. })
    {
        return activity.to_string();
    }
    match movement {
        MoveState::Still => "Stationary (never walks)".to_string(),
        MoveState::Arrived => "Arrived · idle".to_string(),
        MoveState::Walking { speed, waypoints } => {
            format!("Walking · {speed:.1} m/s · {waypoints} waypoint(s) left")
        }
    }
}

/// `"→ The Gradine · 47 m · ETA 26 s (go_to)"` — destination, walk left,
/// real-time ETA at the current speed, and the layer aiming the feet.
fn heading_line(heading: &Heading) -> String {
    use std::fmt::Write as _;
    let mut line = format!("→ {} · {:.0} m", heading.destination, heading.distance_m);
    if let Some(eta) = heading.eta_seconds {
        let _ = write!(line, " · ETA {}", format_eta(eta));
    }
    if let Some(reason) = heading.reason {
        let _ = write!(line, " ({reason})");
    }
    line
}

/// Whole real-life seconds up to two minutes (`"94 s"`), then minutes and
/// seconds (`"3 m 12 s"`).
fn format_eta(seconds: f64) -> String {
    let seconds = seconds.round().max(0.0) as u64;
    if seconds < 120 {
        format!("{seconds} s")
    } else {
        format!("{} m {} s", seconds / 60, seconds % 60)
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

    if let Some(weather) = &sheet.weather {
        let _ = write!(out, "\n\nWEATHER\n  {weather}");
    }

    // Each status is a labelled value with its own fill bar on the next line, so
    // the level reads at a glance.
    let _ = write!(
        out,
        "\n\nSTATUSES\n  Thirst  {:.0}/{:.0} · {}\n  {}\n  Hunger  {:.0}/{:.0} · {}\n  {}",
        sheet.thirst,
        THIRST_MAX,
        thirst_label(sheet.thirst),
        bar(sheet.thirst / THIRST_MAX, GAUGE_CELLS),
        sheet.hunger,
        HUNGER_MAX,
        hunger_label(sheet.hunger),
        bar(sheet.hunger / HUNGER_MAX, GAUGE_CELLS),
    );

    let _ = write!(
        out,
        "\n\nMOVEMENT\n  {}",
        move_summary(&sheet.movement, sheet.well_activity.as_deref())
    );
    if let Some(heading) = &sheet.heading {
        let _ = write!(out, "\n  {}", heading_line(heading));
    }
    if let Some(intent) = &sheet.pending_intent {
        let _ = write!(out, "\n  go_to (pending) → {intent}");
    }

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
    // A fixed-width face throughout, so the fill bars and numeric columns line
    // up character-for-character.
    let mono_font = fonts
        .as_deref()
        .map(CathedralFonts::mono)
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
                    font: mono_font.clone(),
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
                    font: mono_font.clone(),
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
                    font: mono_font,
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
        // The layer is off almost always, so the clear and the hide are both
        // compare-guarded: an unconditional write re-flags the panel every
        // frame of every run and re-propagates its whole subtree.
        if inspected.actor_id.is_some() {
            inspected.actor_id = None;
        }
        panel_visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    panel_visibility.set_if_neq(Visibility::Inherited);

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
            world.characters.get(&sim_id).map(|character| {
                let errand = engine
                    .round()
                    .and_then(|round| round.errand_debug(&sim_id));
                CharacterDebug::from_world(world, character, errand.as_ref())
            })
        })
    });

    // The sheet is ~1 KB of monospace that changes about once a game minute,
    // so the writes compare first. Assigning `Text` unconditionally re-runs
    // the measure, re-shapes the whole block through cosmic-text and
    // re-extracts every glyph — every frame the layer is up, which is exactly
    // when somebody is watching the frame time.
    match sheet {
        Some(sheet) => {
            set_text(&mut name_text, &identity_line(&sheet));
            set_text(&mut body_text, &format_body(&sheet));
        }
        // A subject was pinned but no longer resolves — it left the world, or the
        // cast is not online yet. Keep the panel up with a clear reason.
        None if inspected.actor_id.is_some() => {
            set_text(&mut name_text, "(subject unavailable)");
            set_text(&mut body_text, "");
        }
        None => {
            // The "CHARACTER DEBUG" mode label above already titles the panel;
            // keep the big name line empty until there is a subject to name.
            set_text(&mut name_text, "");
            set_text(&mut body_text, EMPTY_HINT);
        }
    }
}

/// Writes a line only when it differs from the one already standing.
fn set_text(text: &mut Mut<Text>, value: &str) {
    if text.0 != value {
        text.0 = value.to_string();
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
            hunger: 120.0,
            weather: Some("weather: steady rain; the streets are wet · wetness: wet".into()),
            movement: MoveState::Walking {
                speed: 1.2,
                waypoints: 3,
            },
            heading: Some(Heading {
                destination: "The Gradine".into(),
                reason: Some("go_to"),
                distance_m: 47.3,
                eta_seconds: Some(26.2),
            }),
            well_activity: None,
            pending_intent: None,
            position: SimVec3::new(12.34, 0.0, -45.6),
            facing_yaw: std::f64::consts::FRAC_PI_2,
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
            "WEATHER\n  weather: steady rain; the streets are wet · wetness: wet",
            "STATUSES",
            "Thirst  40/255 · THIRSTY", // 40 is between PARCHED (38) and THIRSTY (178)
            "░", // the gauge bar is present
            "MOVEMENT\n  Walking · 1.2 m/s · 3 waypoint(s) left",
            "→ The Gradine · 47 m · ETA 26 s (go_to)",
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
        sheet.heading = None;
        let body = format_body(&sheet);
        assert!(body.contains("HOLDS\n  (empty-handed)"));
        assert!(body.contains("MOVEMENT\n  Stationary (never walks)"));
        assert!(!body.contains("ETA"));
    }

    #[test]
    fn eta_reads_in_seconds_then_minutes() {
        assert_eq!(format_eta(0.4), "0 s");
        assert_eq!(format_eta(93.6), "94 s");
        assert_eq!(format_eta(119.4), "119 s");
        assert_eq!(format_eta(192.0), "3 m 12 s");
        assert_eq!(format_eta(-5.0), "0 s");
    }

    #[test]
    fn heading_line_degrades_without_eta_or_reason() {
        let mut heading = Heading {
            destination: "Chain Well".into(),
            reason: Some("water round"),
            distance_m: 12.4,
            eta_seconds: Some(6.9),
        };
        assert_eq!(heading_line(&heading), "→ Chain Well · 12 m · ETA 7 s (water round)");
        heading.eta_seconds = None;
        heading.reason = None;
        assert_eq!(heading_line(&heading), "→ Chain Well · 12 m");
    }

    #[test]
    fn well_activity_outranks_the_arrived_summary_but_never_a_walk() {
        assert_eq!(
            move_summary(&MoveState::Arrived, Some("Queued at Chain Well · 2 ahead")),
            "Queued at Chain Well · 2 ahead"
        );
        assert_eq!(
            move_summary(
                &MoveState::Walking { speed: 1.8, waypoints: 2 },
                Some("Queued at Chain Well · 2 ahead"),
            ),
            "Walking · 1.8 m/s · 2 waypoint(s) left"
        );
        assert_eq!(move_summary(&MoveState::Arrived, None), "Arrived · idle");
    }

    #[test]
    fn a_pending_intent_gets_its_own_line() {
        let mut sheet = sample();
        sheet.heading = None;
        sheet.movement = MoveState::Arrived;
        sheet.pending_intent = Some("Tam Rud (last seen)".into());
        let body = format_body(&sheet);
        assert!(body.contains("go_to (pending) → Tam Rud (last seen)"));
    }

    #[test]
    fn walk_lengths_are_measured_in_the_walk_plane() {
        let path = [SimVec3::new(3.0, 9.0, 0.0), SimVec3::new(3.0, 9.0, 4.0)];
        // 3 m along x, then 4 m along z; the y jump must not count.
        assert!((path_length_m(SimVec3::new(0.0, 0.0, 0.0), &path) - 7.0).abs() < 1e-9);
        assert_eq!(path_length_m(SimVec3::new(1.0, 2.0, 3.0), &[]), 0.0);
    }
}
