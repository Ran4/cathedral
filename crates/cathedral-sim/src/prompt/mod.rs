//! The LLM text format (`prompt.py`): the character sheet, the turn prompt, and
//! the reply parser.
//!
//! The split between code and data is deliberate (prompt.md §6.2): **all**
//! LLM-visible prose lives in `assets/prompts/` — the prose body in `turn.j2`,
//! the sheet's micro-strings in `strings.toml` — while every decision (who is
//! visible, which offers show, how far away someone is, how a section is laid
//! out) stays here. The template receives fully-computed values and does no
//! filtering, ordering or formatting of its own. The sheet's **section labels**
//! (`**you_hold**`, `**since_your_last_turn**`, …) are the one exception: they
//! mirror the old JSON keys, `turn.j2`'s prose names them, and they are
//! structure, not prose — so they live in [`sheet_markdown`], not in
//! `strings.toml`.
//!
//! The sheet reaches the model as *markdown*, not JSON — the same data at
//! roughly half the tokens, without the quoting, bracing and repeated keys
//! (measured 1456 → 788 o200k tokens on a live turn). [`Sheet`] is still built
//! as a struct: the markdown is one rendering of it ([`sheet_markdown`]), and
//! [`render_sheet_value`] exposes the same data as JSON for tests and tooling.
//!
//! The rendered prompt is byte-diffed against blessed fixtures
//! (`tests/golden_prompts.rs`), so the layout in this module is behavior.

pub mod parse;

use minijinja::{AutoEscape, Environment, context};
use serde::{Deserialize, Serialize};

use crate::{
    HEARING_RADIUS_M, ITEM_INTERACTION_RADIUS_M,
    character::Character,
    error::PromptError,
    ids::{ActorId, ItemId, PlaceId},
    offer::Offer,
    world::World,
};

pub use parse::{ParsedAction, REPLY_MUST_BE_TEXT, parse_reply, parse_reply_value};

/// The one template `assets/prompts/turn.j2` is registered under.
const TURN_TEMPLATE: &str = "turn.j2";

/// The LLM-visible strings the sheet embeds (`assets/prompts/strings.toml`).
///
/// They are not in `turn.j2` because the sheet is serialized before templating;
/// they are not in Rust because they are prose the model reads (D3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptStrings {
    /// Rendered instead of the real name of anyone outside `knows`.
    pub unknown_person_name: String,
    /// `you_see.description`.
    pub you_see_description: String,
    /// `since_your_last_turn` when nothing happened.
    pub nothing: String,
    /// `recent_history` when nothing has happened yet.
    pub nothing_yet: String,
    /// `you_offer` for a broadcast offer.
    pub offer_to_anyone: String,
    /// `the_only_languages_you_know`.
    pub languages: String,
    /// `offered_to_you`'s accept line, with `%s` standing for the item id.
    pub accept_with: String,
    /// `you_see` when nobody is within earshot.
    pub nobody: String,
    /// `stored_memories` when there are none.
    pub no_memories: String,
    /// `places_you_know` when the actor holds no place handles.
    pub no_places: String,
    /// `you_hold` when the hands are empty.
    pub holding_nothing: String,
    /// The parenthesis after `**places_you_know**`.
    pub places_note: String,
    /// Introduces `you_are`'s clock phrase.
    pub the_hour_label: String,
    /// Introduces the lore profile's `illegal_activity` on the `**you**` line.
    pub illegal_activity_label: String,
}

impl PromptStrings {
    fn accept_with(&self, item_id: &ItemId) -> String {
        self.accept_with.replacen("%s", item_id.as_str(), 1)
    }
}

/// The prompt renderer: the compiled template plus the sheet's strings.
///
/// Build it once at startup from the two `assets/prompts/` files. There is no
/// embedded fallback — a missing or broken file is a hard error, because a
/// silently baked-in prompt is exactly what the data files exist to prevent
/// (prompt.md §6.3, R11).
pub struct PromptEnv {
    environment: Environment<'static>,
    strings: PromptStrings,
}

impl std::fmt::Debug for PromptEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptEnv")
            .field("strings", &self.strings)
            .finish_non_exhaustive()
    }
}

impl PromptEnv {
    /// `turn_template` is the text of `assets/prompts/turn.j2`, `strings_toml`
    /// that of `assets/prompts/strings.toml`. The host reads the files; the sim
    /// crate never touches the filesystem.
    ///
    /// The minijinja settings are all `[BYTE]`-load-bearing: without
    /// `keep_trailing_newline` the prompt loses its final `\n`, and autoescape
    /// would HTML-escape the quotes in the sheet JSON.
    pub fn new(turn_template: &str, strings_toml: &str) -> Result<Self, PromptError> {
        let strings: PromptStrings = toml::from_str(strings_toml)
            .map_err(|error| PromptError::new(format!("invalid prompt strings: {error}")))?;
        if !strings.accept_with.contains("%s") {
            return Err(PromptError::new(
                "prompt strings: accept_with must contain the '%s' item-id placeholder",
            ));
        }

        let mut environment = Environment::new();
        environment.set_keep_trailing_newline(true);
        environment.set_auto_escape_callback(|_| AutoEscape::None);
        environment
            .add_template_owned(TURN_TEMPLATE, turn_template.to_string())
            .map_err(|error| PromptError::new(format!("invalid prompt template: {error}")))?;

        Ok(Self {
            environment,
            strings,
        })
    }

    pub fn strings(&self) -> &PromptStrings {
        &self.strings
    }
}

// ---------------------------------------------------------------------------
// The character sheet. Declaration order is normative: [`sheet_markdown`]
// renders its sections in this order, and [`render_sheet_value`] serializes
// the same structs for tests and tooling.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Sheet<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    lore_profile: Option<PromptLoreProfile<'a>>,
    back_story: &'a str,
    you_are: YouAre,
    /// The wayfinding whitelist (M5): the places this character can `go_to`,
    /// each an opaque handle plus the name people speak of it by. Rendered even
    /// when empty — an empty list honestly says "you know no ways".
    places_you_know: Vec<PlaceRef<'a>>,
    you_hold: Vec<ItemRef<'a>>,
    /// Omitted entirely when empty — not rendered as `[]` (`prompt.py:227`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    you_offer: Vec<YouOffer<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    offered_to_you: Vec<OfferedToYou<'a>>,
    you_see: YouSee<'a>,
    since_your_last_turn: Vec<&'a str>,
    recent_history: Vec<&'a str>,
    stored_memories: &'a [String],
    the_only_languages_you_know: &'a str,
    /// Always a string: a cleared goal is the literal `"None"` (D15).
    current_goal: &'a str,
}

/// The compact, always-relevant portion of a lore record. The potentially
/// long extended description stays authoritative in the sim but is reserved
/// for a future introspection mechanism rather than paid for on every turn.
#[derive(Serialize)]
struct PromptLoreProfile<'a> {
    age: u16,
    gender: &'a str,
    occupation: Option<&'a str>,
    title: Option<&'a str>,
    rank: Option<&'a str>,
    faction_role: Option<&'a str>,
    illegal_activity: Option<&'a str>,
    district: &'a str,
    father: Option<LoreRelation<'a>>,
    mother: Option<LoreRelation<'a>>,
    children: Vec<LoreRelation<'a>>,
    circumstances: &'a [String],
    conditions: &'a [String],
}

#[derive(Serialize)]
struct LoreRelation<'a> {
    id: &'a ActorId,
    name: String,
}

#[derive(Serialize)]
struct YouAre {
    location_description: String,
    /// The office, as a short phrase — the clock reaches the model here, not as
    /// a percept (`features/movement/01_the_clock.md` §7). Omitted when the host
    /// carries no clock, which keeps the frozen golden fixtures byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    the_hour: Option<String>,
    position_m: Position,
}

/// `Vec3.to_json()`. The fields are `f64` even when integral, so `114.0` renders
/// as `114.0` and not `114`.
#[derive(Serialize)]
struct Position {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Serialize)]
struct ItemRef<'a> {
    id: &'a ItemId,
    name: &'a str,
}

/// One `places_you_know` entry. The key is `place_id`, not `id`, so a place
/// handle can never be conflated with a person handle even out of context
/// (`features/movement/05_the_llm_seam.md` §3).
#[derive(Serialize)]
struct PlaceRef<'a> {
    place_id: &'a PlaceId,
    name: &'a str,
}

/// `_person` (`prompt.py:144-155`): `distance_m` and `moving` only appear in
/// `you_see`.
#[derive(Serialize)]
struct Person<'a> {
    id: &'a ActorId,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance_m: Option<f64>,
    /// Whether they are walking past rather than standing with you — the
    /// difference between greeting everyone who passes and greeting people who
    /// stop. One definition of moving everywhere: `!is_settled()`, the novelty
    /// gate's own threshold (05_the_llm_seam.md §3).
    #[serde(skip_serializing_if = "Option::is_none")]
    moving: Option<bool>,
}

#[derive(Serialize)]
struct YouOffer<'a> {
    item: ItemRef<'a>,
    to: OfferTo<'a>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum OfferTo<'a> {
    /// A broadcast offer — or, quirk intact, one whose target has left the
    /// world (`prompt.py:186-196`; R9/R21: replicate, do not "fix").
    Anyone(&'a str),
    Person(Person<'a>),
}

#[derive(Serialize)]
struct OfferedToYou<'a> {
    item: ItemRef<'a>,
    from: Person<'a>,
    accept_with: String,
}

#[derive(Serialize)]
struct YouSee<'a> {
    description: &'a str,
    people: Vec<Person<'a>>,
}

fn person<'a>(
    actor: &Character,
    other: &'a Character,
    strings: &'a PromptStrings,
    distance_m: Option<f64>,
    moving: Option<bool>,
) -> Person<'a> {
    Person {
        id: other.id(),
        name: if actor.knows().contains(other.id()) {
            other.name()
        } else {
            &strings.unknown_person_name
        },
        distance_m: distance_m.map(|distance| py_round(distance, 1)),
        moving,
    }
}

/// The global offer order both sheet sections follow (`prompt.py:162-163`).
/// `ItemId`'s `Ord` is UTF-8 byte order, which equals Python's `str` code-point
/// order.
fn offer_sort_key(offer: &Offer) -> (i64, &ItemId) {
    (offer.created_seq, &offer.item_id)
}

/// Render `actor_id`'s turn prompt.
///
/// `since` overrides `since_your_last_turn`; passing `None` reads the actor's
/// live inbox and mutates nothing (`prompt.py:214`) — the preview path used by
/// tests and tooling.
pub fn render_prompt(
    world: &World,
    actor_id: &ActorId,
    since: Option<&[String]>,
    env: &PromptEnv,
) -> Result<String, PromptError> {
    let sheet = build_sheet(world, llm_actor(world, actor_id)?, since, &env.strings);
    let sheet_md = sheet_markdown(&sheet, &env.strings);
    let emittable_sounds = world.sound_catalog.emittable_sound_ids().join(", ");

    env.environment
        .get_template(TURN_TEMPLATE)
        .and_then(|template| {
            template.render(context! {
                sheet_md,
                sounds_enabled => world.sounds_enabled,
                emittable_sounds,
            })
        })
        .map_err(|error| PromptError::new(format!("the turn template did not render: {error}")))
}

/// The sheet as structured JSON — the markdown's source data, exposed for
/// tests and tooling. The prompt itself renders the same [`Sheet`] as
/// markdown, so a value observed here is the value the model reads (modulo
/// layout). Key order is NOT meaningful here — the markdown's section order is
/// pinned by the golden fixtures instead.
pub fn render_sheet_value(
    world: &World,
    actor_id: &ActorId,
    since: Option<&[String]>,
    env: &PromptEnv,
) -> Result<serde_json::Value, PromptError> {
    let sheet = build_sheet(world, llm_actor(world, actor_id)?, since, &env.strings);
    serde_json::to_value(&sheet).map_err(|error| {
        PromptError::new(format!("the character sheet did not serialize: {error}"))
    })
}

fn llm_actor<'a>(world: &'a World, actor_id: &ActorId) -> Result<&'a Character, PromptError> {
    let Some(actor) = world.characters.get(actor_id) else {
        return Err(PromptError::new(format!("unknown actor '{actor_id}'")));
    };
    if !actor.control().is_llm() {
        return Err(PromptError::new(
            "the human-controlled player must never receive an LLM prompt",
        ));
    }
    Ok(actor)
}

/// Everything the sheet says, computed from the world — who is visible, which
/// offers show, the wayfinding whitelist, the drained inbox. Pure data; both
/// renderings above consume it.
fn build_sheet<'a>(
    world: &'a World,
    actor: &'a Character,
    since: Option<&'a [String]>,
    strings: &'a PromptStrings,
) -> Sheet<'a> {
    let people: Vec<Person<'_>> = world
        .characters_within(actor.position_m(), HEARING_RADIUS_M, Some(actor.id()))
        .iter()
        .map(|other_id| {
            let other = &world.characters[other_id];
            let distance = actor
                .position_m()
                .distance_squared(other.position_m())
                .sqrt();
            person(actor, other, strings, Some(distance), Some(!other.is_settled()))
        })
        .collect();

    let mut sorted_offers: Vec<&Offer> = world.offers.values().collect();
    sorted_offers.sort_by_key(|offer| offer_sort_key(offer));

    let mut you_offer: Vec<YouOffer<'_>> = Vec::new();
    let mut offered_to_you: Vec<OfferedToYou<'_>> = Vec::new();
    for offer in sorted_offers {
        // An offer of an item that has left the world shows in neither section.
        let Some(entity) = world.items.get(&offer.item_id) else {
            continue;
        };
        let item = ItemRef {
            id: &offer.item_id,
            name: &entity.name,
        };
        if offer.giver_id == *actor.id() {
            let target = offer
                .target_id
                .as_ref()
                .and_then(|target_id| world.characters.get(target_id));
            you_offer.push(YouOffer {
                item,
                to: match target {
                    None => OfferTo::Anyone(&strings.offer_to_anyone),
                    Some(target) => OfferTo::Person(person(actor, target, strings, None, None)),
                },
            });
        } else if offer.target_id.is_none() || offer.target_id.as_ref() == Some(actor.id()) {
            let Some(giver) = world.characters.get(&offer.giver_id) else {
                continue;
            };
            // Inclusive at exactly 4 m: only a strictly greater distance drops
            // the offer, so `offered_to_you` lists exactly what can be accepted.
            if actor.position_m().distance_squared(giver.position_m())
                > ITEM_INTERACTION_RADIUS_M * ITEM_INTERACTION_RADIUS_M
            {
                continue;
            }
            offered_to_you.push(OfferedToYou {
                item,
                from: person(actor, giver, strings, None, None),
                accept_with: strings.accept_with(&offer.item_id),
            });
        }
    }

    // The wayfinding whitelist, resolved against the world registry and sorted
    // by name (then id) — a stable, human order, so the list reads the same to
    // the model turn after turn. A held handle the registry no longer names is
    // silently skipped, like a dangling item id.
    let mut places_you_know: Vec<PlaceRef<'_>> = actor
        .state
        .places_known
        .iter()
        .filter_map(|place_id| world.places.get(place_id))
        .map(|entry| PlaceRef {
            place_id: &entry.id,
            name: &entry.name,
        })
        .collect();
    places_you_know.sort_by(|left, right| {
        left.name
            .cmp(right.name)
            .then_with(|| left.place_id.cmp(right.place_id))
    });

    let events: &[String] = since.unwrap_or(actor.inbox());
    let since_your_last_turn = fallback(events, &strings.nothing);
    let recent_history = fallback(actor.recent_history(), &strings.nothing_yet);

    let position = actor.position_m();
    let location_description = world
        .area_map
        .location_description(position)
        .unwrap_or_else(|| actor.location_description().to_string());
    let relation = |id| LoreRelation {
        id,
        name: world
            .characters
            .get(id)
            .map_or_else(|| id.to_string(), |character| character.name().to_string()),
    };
    let lore_profile = actor.lore().map(|profile| PromptLoreProfile {
        age: profile.age,
        gender: &profile.gender,
        occupation: profile.occupation_display.as_deref(),
        title: profile.title.as_deref(),
        rank: profile.rank.as_deref(),
        faction_role: profile.faction_role.as_deref(),
        illegal_activity: profile.illegal_activity.as_deref(),
        district: &profile.district,
        father: profile.father.as_ref().map(&relation),
        mother: profile.mother.as_ref().map(&relation),
        children: profile.children.iter().map(relation).collect(),
        circumstances: &profile.circumstances,
        conditions: &profile.conditions,
    });
    Sheet {
        name: actor.name(),
        lore_profile,
        back_story: actor.back_story(),
        you_are: YouAre {
            location_description,
            the_hour: world.current_time.map(|time| {
                format!("{} — {}", time.office.label(), time.office.prompt_phrase())
            }),
            position_m: Position {
                x: position.x,
                y: position.y,
                z: position.z,
            },
        },
        places_you_know,
        you_hold: actor
            .holds()
            .iter()
            // A dangling id is silently skipped (`prompt.py:222-226`).
            .filter_map(|item_id| {
                world.items.get(item_id).map(|item| ItemRef {
                    id: item_id,
                    name: &item.name,
                })
            })
            .collect(),
        you_offer,
        offered_to_you,
        you_see: YouSee {
            description: &strings.you_see_description,
            people,
        },
        since_your_last_turn,
        recent_history,
        stored_memories: actor.memories(),
        the_only_languages_you_know: &strings.languages,
        current_goal: actor.goal(),
    }
}

/// `events or ["nothing"]` — the empty list is falsy in Python.
fn fallback<'a>(lines: &'a [String], empty: &'a str) -> Vec<&'a str> {
    if lines.is_empty() {
        vec![empty]
    } else {
        lines.iter().map(String::as_str).collect()
    }
}

// ---------------------------------------------------------------------------
// The markdown rendering — what the model actually reads.
// ---------------------------------------------------------------------------

/// Render the sheet as the markdown block `turn.j2` embeds.
///
/// Layout decisions live here; every human-facing phrase comes from
/// [`PromptStrings`]. Empty sections render as words (`**you_hold** —
/// nothing`), never as `[]`, and `you_offer` / `offered_to_you` disappear
/// entirely when empty — exactly as the JSON sheet omitted them.
fn sheet_markdown(sheet: &Sheet<'_>, strings: &PromptStrings) -> String {
    let mut sections: Vec<String> = Vec::with_capacity(12);

    sections.push(you_line(sheet, strings));
    sections.push(format!("**back_story** — {}", sheet.back_story));
    sections.push(you_are_line(&sheet.you_are, strings));

    sections.push(bullet_section(
        &format!("**places_you_know** ({})", strings.places_note),
        sheet
            .places_you_know
            .iter()
            .map(|place| format!("{} {}", place.place_id, place.name)),
        &strings.no_places,
    ));
    sections.push(bullet_section(
        "**you_hold**",
        sheet.you_hold.iter().map(item_md),
        &strings.holding_nothing,
    ));

    if !sheet.you_offer.is_empty() {
        sections.push(bullet_section(
            "**you_offer**",
            sheet.you_offer.iter().map(|offer| {
                let to = match &offer.to {
                    OfferTo::Anyone(anyone) => (*anyone).to_string(),
                    OfferTo::Person(person) => person_md(person),
                };
                format!("{} — to {to}", item_md(&offer.item))
            }),
            "",
        ));
    }
    if !sheet.offered_to_you.is_empty() {
        sections.push(bullet_section(
            "**offered_to_you**",
            sheet.offered_to_you.iter().map(|offer| {
                format!(
                    "{} — from {} (accept with: {})",
                    item_md(&offer.item),
                    person_md(&offer.from),
                    offer.accept_with
                )
            }),
            "",
        ));
    }

    sections.push(bullet_section(
        &format!("**you_see** ({})", sheet.you_see.description),
        sheet.you_see.people.iter().map(person_bullet),
        &strings.nobody,
    ));

    sections.push(history_section(
        "**since_your_last_turn**",
        &sheet.since_your_last_turn,
        &strings.nothing,
    ));
    sections.push(history_section(
        "**recent_history**",
        &sheet.recent_history,
        &strings.nothing_yet,
    ));
    sections.push(bullet_section(
        "**stored_memories**",
        sheet.stored_memories.iter().map(String::clone),
        &strings.no_memories,
    ));

    sections.push(format!(
        "**the_only_languages_you_know** — {}\n**current_goal** — {}",
        sheet.the_only_languages_you_know, sheet.current_goal
    ));

    sections.join("\n\n")
}

/// The `**you**` line: the name, then the lore profile folded into one
/// sentence. Absent lore fields simply do not appear — the JSON's explicit
/// `"father": null` bought nothing.
fn you_line(sheet: &Sheet<'_>, strings: &PromptStrings) -> String {
    let mut line = format!("**you** — {}", sheet.name);
    let Some(lore) = &sheet.lore_profile else {
        return line;
    };
    line.push_str(&format!(", {}, {}", lore.age, gender_word(lore.gender)));

    // "— Scrivener (Scribe and clerk, journeyman) of The Tallage."
    let head = lore.title.or(lore.occupation);
    let mut parenthetical: Vec<&str> = Vec::new();
    if lore.title.is_some() && lore.occupation != lore.title {
        parenthetical.extend(lore.occupation);
    }
    parenthetical.extend(lore.rank);
    match head {
        Some(head) if parenthetical.is_empty() => {
            line.push_str(&format!(" — {head} of {}.", lore.district));
        }
        Some(head) => line.push_str(&format!(
            " — {head} ({}) of {}.",
            parenthetical.join(", "),
            lore.district
        )),
        None => line.push_str(&format!(", of {}.", lore.district)),
    }

    if let Some(role) = lore.faction_role {
        line.push_str(&format!(" Faction role: {role}."));
    }
    if let Some(illegal) = lore.illegal_activity {
        line.push_str(&format!(" {} {illegal}.", strings.illegal_activity_label));
    }
    let mut family: Vec<String> = Vec::new();
    if let Some(father) = &lore.father {
        family.push(format!("father {} (id {})", father.name, father.id));
    }
    if let Some(mother) = &lore.mother {
        family.push(format!("mother {} (id {})", mother.name, mother.id));
    }
    if !lore.children.is_empty() {
        let children: Vec<String> = lore
            .children
            .iter()
            .map(|child| format!("{} (id {})", child.name, child.id))
            .collect();
        family.push(format!("children: {}", children.join(", ")));
    }
    if !family.is_empty() {
        line.push_str(&format!(" Family: {}.", family.join("; ")));
    }
    if !lore.circumstances.is_empty() {
        line.push_str(&format!(" Circumstances: {}.", lore.circumstances.join(", ")));
    }
    if !lore.conditions.is_empty() {
        line.push_str(&format!(" Conditions: {}.", lore.conditions.join(", ")));
    }
    line
}

/// "m"/"f" are lore-data codes; the model reads words.
fn gender_word(gender: &str) -> &str {
    match gender {
        "m" => "male",
        "f" => "female",
        other => other,
    }
}

/// `**you_are** — The Tallage (x -306, z 90). The hour: Dayspring — sunrise; …`
///
/// Coordinates are whole metres — the JSON's 17-significant-digit `f64`s were
/// pure token noise — and `y` (elevation) never informed a decision.
fn you_are_line(you_are: &YouAre, strings: &PromptStrings) -> String {
    let position = &you_are.position_m;
    let mut line = format!(
        "**you_are** — {} (x {:.0}, z {:.0}).",
        you_are.location_description, position.x, position.z
    );
    if let Some(hour) = &you_are.the_hour {
        line.push_str(&format!(" {} {hour}.", strings.the_hour_label));
    }
    line
}

/// `fzbn9 smoked fish` — items are always id-first, like places.
fn item_md(item: &ItemRef<'_>) -> String {
    format!("{} {}", item.id, item.name)
}

/// `id cb947: Conny` — how a person is referenced outside `you_see`.
fn person_md(person: &Person<'_>) -> String {
    format!("id {}: {}", person.id, person.name)
}

/// The `you_see` bullet: `id cb947: Conny, 2.7 m, moving`. `moving` appears
/// only when true — standing still is the unmarked case.
fn person_bullet(person: &Person<'_>) -> String {
    let mut line = person_md(person);
    if let Some(distance) = person.distance_m {
        line.push_str(&format!(", {distance:.1} m"));
    }
    if person.moving == Some(true) {
        line.push_str(", moving");
    }
    line
}

/// `<header>:` over `- ` bullets — or `<header> — <empty_word>` inline when
/// there are no entries.
fn bullet_section(
    header: &str,
    entries: impl Iterator<Item = String>,
    empty_word: &str,
) -> String {
    let mut section = header.to_string();
    let mut any = false;
    for entry in entries {
        section.push_str(if any { "\n- " } else { ":\n- " });
        section.push_str(&entry);
        any = true;
    }
    if !any {
        section.push_str(&format!(" — {empty_word}"));
    }
    section
}

/// A history section: bullets, or ` — <sentinel>` inline when the builder's
/// [`fallback`] left only the sentinel (the markdown twin of `["nothing"]`).
fn history_section(header: &str, lines: &[&str], sentinel: &str) -> String {
    if lines.len() == 1 && lines[0] == sentinel {
        return format!("{header} — {sentinel}");
    }
    bullet_section(header, lines.iter().map(|line| (*line).to_string()), sentinel)
}

/// CPython's `round(value, digits)`: correctly-rounded decimal, ties to even.
///
/// `{:.N}` formats the *exact* binary value and rounds half-to-even, which is
/// what `float.__round__` does; parsing the result back reproduces the same
/// double. The naive `(x * 10^n).round() / 10^n` does not (`0.25` → `0.3`
/// instead of `0.2`). Non-finite input is returned unchanged — Python would
/// raise, but no such value can reach a sheet.
pub fn py_round(value: f64, digits: usize) -> f64 {
    if !value.is_finite() {
        return value;
    }
    format!("{value:.digits$}")
        .parse()
        .expect("a fixed-precision float always parses back")
}

/// Move the actor's inbox into a prompt, leaving a fresh inbox behind
/// (`prompt.py:243-261`).
///
/// Returns the prompt together with the percepts it presented as
/// `since_your_last_turn`. The caller graduates those into `recent_history`
/// with [`Character::absorb_presented_history`] once the turn *completes*; a
/// turn that never happens pushes them back onto the FRONT of
/// `pending_history`, so a retried prompt presents them as new instead of
/// showing the same line in both fields at once.
///
/// On a render error both lists are restored by **prepending**, so anything
/// that arrived mid-render stays behind them in chronological order.
pub fn render_prompt_and_drain(
    world: &mut World,
    actor_id: &ActorId,
    env: &PromptEnv,
) -> Result<(String, Vec<String>), PromptError> {
    let Some(actor) = world.characters.get_mut(actor_id) else {
        return Err(PromptError::new(format!("unknown actor '{actor_id}'")));
    };
    let drained = std::mem::take(&mut actor.state.inbox);
    let presented = actor.take_pending_history();

    match render_prompt(world, actor_id, Some(&drained), env) {
        Ok(rendered) => Ok((rendered, presented)),
        Err(error) => {
            let actor = world
                .characters
                .get_mut(actor_id)
                .expect("the actor existed a moment ago");
            prepend(&mut actor.state.inbox, drained);
            prepend(&mut actor.state.pending_history, presented);
            Err(error)
        }
    }
}

fn prepend(target: &mut Vec<String>, mut front: Vec<String>) {
    front.append(target);
    *target = front;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings() -> PromptStrings {
        PromptStrings {
            unknown_person_name: "a stranger (you don't know their name)".into(),
            you_see_description: "people within 20 metres, nearest first".into(),
            nothing: "nothing".into(),
            nothing_yet: "nothing yet".into(),
            offer_to_anyone: "anyone".into(),
            languages: "English".into(),
            accept_with: r#"accept_offered_item {"item_id": "%s"}"#.into(),
            nobody: "nobody".into(),
            no_memories: "none".into(),
            no_places: "none".into(),
            holding_nothing: "nothing".into(),
            places_note: "go_to takes these place_ids".into(),
            the_hour_label: "The hour:".into(),
            illegal_activity_label: "In secret:".into(),
        }
    }

    #[test]
    fn the_accept_with_pattern_takes_the_item_id() {
        assert_eq!(
            strings().accept_with(&ItemId::from_raw("fzbn9")),
            r#"accept_offered_item {"item_id": "fzbn9"}"#
        );
    }

    #[test]
    fn a_strings_file_without_the_placeholder_is_rejected() {
        let toml = "unknown_person_name = \"a\"\nyou_see_description = \"b\"\nnothing = \"c\"\nnothing_yet = \"d\"\noffer_to_anyone = \"e\"\nlanguages = \"f\"\naccept_with = \"no placeholder\"\nnobody = \"g\"\nno_memories = \"h\"\nno_places = \"i\"\nholding_nothing = \"j\"\nplaces_note = \"k\"\nthe_hour_label = \"l\"\nillegal_activity_label = \"m\"\n";
        let error = PromptEnv::new("x", toml).unwrap_err();
        assert!(error.message.contains("%s"), "{}", error.message);
    }

    #[test]
    fn bullet_sections_render_bullets_or_the_inline_empty_word() {
        assert_eq!(
            bullet_section("**you_hold**", ["a b", "c d"].iter().map(|s| (*s).to_string()), "nothing"),
            "**you_hold**:\n- a b\n- c d"
        );
        assert_eq!(
            bullet_section("**you_hold**", std::iter::empty(), "nothing"),
            "**you_hold** — nothing"
        );
    }

    #[test]
    fn history_sections_inline_only_the_fallback_sentinel() {
        assert_eq!(
            history_section("**recent_history**", &["nothing yet"], "nothing yet"),
            "**recent_history** — nothing yet"
        );
        assert_eq!(
            history_section("**recent_history**", &["he waved"], "nothing yet"),
            "**recent_history**:\n- he waved"
        );
    }

    #[test]
    fn the_you_line_folds_lore_and_omits_absent_fields() {
        let mother_id = ActorId::from_raw("br2sk");
        let sheet = Sheet {
            name: "Corin Copp",
            lore_profile: Some(PromptLoreProfile {
                age: 26,
                gender: "m",
                occupation: Some("Scribe and clerk"),
                title: Some("Scrivener"),
                rank: Some("journeyman"),
                faction_role: None,
                illegal_activity: Some("forger"),
                district: "The Tallage",
                father: None,
                mother: Some(LoreRelation {
                    id: &mother_id,
                    name: "Osanne Skell".into(),
                }),
                children: Vec::new(),
                circumstances: &[],
                conditions: &[],
            }),
            back_story: "",
            you_are: YouAre {
                location_description: String::new(),
                the_hour: None,
                position_m: Position { x: 0.0, y: 0.0, z: 0.0 },
            },
            places_you_know: Vec::new(),
            you_hold: Vec::new(),
            you_offer: Vec::new(),
            offered_to_you: Vec::new(),
            you_see: YouSee { description: "", people: Vec::new() },
            since_your_last_turn: Vec::new(),
            recent_history: Vec::new(),
            stored_memories: &[],
            the_only_languages_you_know: "English",
            current_goal: "None",
        };
        assert_eq!(
            you_line(&sheet, &strings()),
            "**you** — Corin Copp, 26, male — Scrivener (Scribe and clerk, journeyman) \
             of The Tallage. In secret: forger. Family: mother Osanne Skell (id br2sk)."
        );

        // A title equal to the occupation is not repeated — no "Anchoress
        // (Anchoress)".
        let mut same_title = sheet;
        {
            let lore = same_title.lore_profile.as_mut().unwrap();
            lore.title = Some("Scribe and clerk");
            lore.rank = None;
            lore.illegal_activity = None;
            lore.mother = None;
        }
        assert_eq!(
            you_line(&same_title, &strings()),
            "**you** — Corin Copp, 26, male — Scribe and clerk of The Tallage."
        );

        // No lore at all: the name stands alone.
        let plain = Sheet { lore_profile: None, ..same_title };
        assert_eq!(you_line(&plain, &strings()), "**you** — Corin Copp");
    }

    #[test]
    fn rounding_is_ties_to_even_on_the_exact_binary_value() {
        assert_eq!(py_round(0.25, 1), 0.2);
        assert_eq!(py_round(0.35, 1), 0.3); // 0.35 is really 0.34999999999999997...
        assert_eq!(py_round(2.6907248094147422, 1), 2.7);
        assert_eq!(py_round(1.2345, 3), 1.234); // 1.2344999999999999...
        assert_eq!(py_round(-0.25, 1), -0.2);
        assert_eq!(py_round(2.0, 1), 2.0);
    }
}
