//! The LLM text format (`prompt.py`): the character sheet, the turn prompt, and
//! the reply parser.
//!
//! The split between code and data is deliberate (prompt.md §6.2): **all**
//! LLM-visible prose lives in `assets/prompts/` — the prose body in `turn.j2`,
//! the sheet's micro-strings in `strings.toml` — while every decision (who is
//! visible, which offers show, how far away someone is, how a section is laid
//! out) stays here. The template receives fully-computed values and does no
//! filtering, ordering or formatting of its own. Two exceptions: the sheet's
//! **section labels** (`**you_hold**`, `**since_your_last_turn**`, …) mirror
//! the old JSON keys, `turn.j2`'s prose names them, and they are structure,
//! not prose — so they live in [`sheet_markdown`], not in `strings.toml`. And
//! [`you_line`]'s connective grammar — the `Family:`/`Circumstances:`/
//! `Conditions:` sentence glue, the `father`/`mother`/`children` and `(id …)`
//! fragments, the gender words — is inseparable from the joining logic that
//! assembles the sentence, so it stays inline there too.
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
    HEARING_RADIUS_M, ITEM_INTERACTION_RADIUS_M, PLACE_ARRIVE_RADIUS_M, WALK_DESTINATION_SNAP_M,
    character::{Character, IntentTarget},
    error::PromptError,
    ids::{ActorId, ItemId, PlaceId},
    lore::PlanningWard,
    offer::Offer,
    world::World,
};

pub use parse::{ParsedAction, REPLY_MUST_BE_TEXT, parse_reply, parse_reply_value};

/// The one template `assets/prompts/turn.j2` is registered under.
const TURN_TEMPLATE: &str = "turn.j2";

/// The Night Office's template (`assets/prompts/night.j2`, movement M6). One
/// file with two branches — the person reflecting at their own bedtime and the
/// ward speaking for its Minors after the curfew — because they share every
/// rule that matters: nobody can hear you, nothing you do is seen, and the only
/// verbs are the ones that change what tomorrow looks like.
const NIGHT_TEMPLATE: &str = "night.j2";

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
    /// The parenthesis after `**you_sell**` — a bound vendor's price list.
    pub sell_note: String,
    /// Introduces `you_are`'s clock phrase.
    pub the_hour_label: String,
    /// Introduces `you_are`'s weekday phrase.
    pub the_day_label: String,
    /// The parenthesis after `**your_round**`.
    pub round_note: String,
    /// The parenthesis after `**word_in_the_ward**` — the ward's live notices.
    pub notices_note: String,
    /// The parenthesis after `**the_ward_says**` — the Night Office's ward
    /// mood, on every Minor's sheet (movement M6).
    pub ward_says_note: String,
    /// The parenthesis after `**your_people**` in the ward digest.
    pub ward_people_note: String,
    /// The parenthesis after `**their_places**` in the ward digest.
    pub ward_places_note: String,
    /// `you_are`'s walk sentence, with `%s` standing for the destination name.
    pub walking_to: String,
    /// `you_are`'s follow sentence, with `%s` standing for the person.
    pub following: String,
    /// Introduces the lore profile's `faction_role` on the `**you**` line.
    pub faction_role_label: String,
    /// Introduces the lore profile's `illegal_activity` on the `**you**` line.
    pub illegal_activity_label: String,
    /// Introduces the lore profile's `home` on the `**you**` line.
    pub home_label: String,
    /// The parenthesis tying `home` to its wayfinding handle — `(go_to
    /// pl_x9k2)` — so walking home never needs a scan of `places_you_know`.
    pub home_place_label: String,
    /// The parenthesis after `**dogs_nearby**` — the street dogs
    /// ([`crate::dogs`]), rendered unconditionally: no `knows` gating, because
    /// nobody needs an introduction to see a dog.
    pub dogs_note: String,
    /// The parenthesis after `**marks_here**` — the chalk on the walls, listed
    /// for everyone with no gating at all: there is no literacy state, and a
    /// mark's meaning is plain to anyone over seven who looks at it.
    pub marks_note: String,
    /// The `you_hold` suffix for a unit riding the mouth
    /// (`features/extra_pockets.md`): `- k3f9x wet spark (in your mouth)`.
    pub pocket_mouth_note: String,
    /// The same for the butt slot — period-euphemistic on purpose.
    pub pocket_butt_note: String,
    /// The same for the frontbutt slot.
    pub pocket_frontbutt_note: String,
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
    /// `turn_template` is the text of `assets/prompts/turn.j2`,
    /// `night_template` that of `assets/prompts/night.j2`, `strings_toml` that
    /// of `assets/prompts/strings.toml`. The host reads the files; the sim
    /// crate never touches the filesystem.
    ///
    /// A missing or broken night template is a hard error exactly like a
    /// missing turn template, and for the same reason (R11): a silently
    /// baked-in prompt is what the data files exist to prevent, and a Night
    /// Office that quietly did not exist would be indistinguishable from one
    /// that ran and changed nothing.
    ///
    /// The minijinja settings are all `[BYTE]`-load-bearing: without
    /// `keep_trailing_newline` the prompt loses its final `\n`, and autoescape
    /// would HTML-escape the quotes in the sheet JSON.
    pub fn new(
        turn_template: &str,
        night_template: &str,
        strings_toml: &str,
    ) -> Result<Self, PromptError> {
        let strings: PromptStrings = toml::from_str(strings_toml)
            .map_err(|error| PromptError::new(format!("invalid prompt strings: {error}")))?;
        if !strings.accept_with.contains("%s") {
            return Err(PromptError::new(
                "prompt strings: accept_with must contain the '%s' item-id placeholder",
            ));
        }
        if !strings.walking_to.contains("%s") {
            return Err(PromptError::new(
                "prompt strings: walking_to must contain the '%s' destination placeholder",
            ));
        }
        if !strings.following.contains("%s") {
            return Err(PromptError::new(
                "prompt strings: following must contain the '%s' person placeholder",
            ));
        }

        let mut environment = Environment::new();
        environment.set_keep_trailing_newline(true);
        environment.set_auto_escape_callback(|_| AutoEscape::None);
        environment
            .add_template_owned(TURN_TEMPLATE, turn_template.to_string())
            .map_err(|error| PromptError::new(format!("invalid prompt template: {error}")))?;
        environment
            .add_template_owned(NIGHT_TEMPLATE, night_template.to_string())
            .map_err(|error| PromptError::new(format!("invalid night template: {error}")))?;

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
    /// Being in the law's hands (`law_and_order.md` M4b′): who has you, where
    /// you are being taken, and what would end it. Gated on the state exactly as
    /// `has_notices`, `has_pockets` and `has_frontbutt` gate theirs — the sheet's
    /// whole idiom is that *a prompt renders nothing for a situation you are not
    /// in*, so somebody not in custody pays zero bytes and the frozen fixtures
    /// do not move. A person being marched across the city by a sergeant can
    /// afford four lines about it.
    #[serde(skip_serializing_if = "Option::is_none")]
    you_are_held: Option<String>,
    /// The other side of the same fact: whom this officer has taken, where they
    /// are walking them, and whether a hand is on the arm. Without it `release`
    /// and `grab` have no id to name — the verbs would be listed and unusable —
    /// and an officer could not answer "who is that you have there?".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    you_have_in_charge: Vec<String>,
    /// The actor's own daily timetable, one line per leg, as the round seeded
    /// it ([`crate::character::CharacterState::daily_round`]) — so "where will
    /// you be tomorrow?" is answered from the sheet, not improvised. Omitted
    /// entirely when the round never enrolled this actor.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    your_round: Vec<&'a str>,
    /// What this actor's ward is saying tonight — the mood the Night Office's
    /// ward batch returned (movement M6, `05_the_llm_seam.md` §4). Carried by
    /// the [`Minor`](crate::lore::Significance::Minor) cast only: it is *their*
    /// reflection, bought one prompt for a hundred and twenty people, where a
    /// Major reflects for themselves. `None` — every sheet until a ward has
    /// spoken — omits the section and keeps the frozen fixtures byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    the_ward_says: Option<&'a str>,
    /// The wayfinding whitelist (M5): the places this character can `go_to`,
    /// each an opaque handle plus the name people speak of it by. Rendered even
    /// when empty — an empty list honestly says "you know no ways".
    places_you_know: Vec<PlaceRef<'a>>,
    you_hold: Vec<ItemRef<'a>>,
    /// A bound vendor's price list, rendered between `you_hold` and `you_see`
    /// so a vendor quotes its stall's prices off the sheet instead of inventing
    /// them (`05_the_llm_seam.md` §3). Priced from the catalog's stock template,
    /// not from current stock, so a sold-out baker still knows what they charge.
    /// Omitted entirely when empty — an unbound actor never sees the section,
    /// the `you_offer` pattern — which keeps the frozen golden fixtures stable.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    you_sell: Vec<SellLine<'a>>,
    /// Omitted entirely when empty — not rendered as `[]` (`prompt.py:227`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    you_offer: Vec<YouOffer<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    offered_to_you: Vec<OfferedToYou<'a>>,
    you_see: YouSee<'a>,
    /// The street dogs within earshot ([`crate::dogs`]), nearest first —
    /// rendered for *everyone* near one, with no `knows` gating, because a dog
    /// in the lane is a plain fact about the street. Omitted entirely when no
    /// dog is near (and in every dog-less world), which keeps the frozen
    /// fixtures byte-identical.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dogs_nearby: Vec<DogLine<'a>>,
    /// The chalk within sight (`features/chalking_the_walls.md`). A standing
    /// fact about the street like the dogs, so it sits beside them; and like
    /// them it is omitted entirely when there is none, which is the universal
    /// case and every frozen fixture's case.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    marks_here: Vec<MarkLine>,
    /// The ward's live notices this actor carries (`law_and_order.md` M3) —
    /// what the ward is saying right now, newest first, capped at
    /// [`crate::notices::NOTICES_SHEET_MAX`] by [`ward_notice_lines`], which
    /// spends the last seat on their own wrong before a newer word. A standing
    /// truth, so it sits with the world sections above the time axis; omitted
    /// entirely for the (usual) carrier-less case, which keeps the frozen
    /// fixtures stable.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    word_in_the_ward: Vec<NoticeLine>,
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
    /// The baked spoken home ("a house in the Cinder Ward, near the Shambles
    /// well" — or the bedless framing), so "Where do you live?" is answered
    /// from the sheet, not improvised (see `homes.rs`).
    home: Option<&'a str>,
    /// The same home as a wayfinding handle — the registry entry
    /// [`Round::seed`](crate::round::Round::seed) minted for this actor's own
    /// door — rendered as a `go_to` hint after the description. `None` for
    /// the bedless (no door) and in nav-less worlds (no registry), where the
    /// prose stands alone.
    home_place_id: Option<&'a PlaceId>,
    father: Option<LoreRelation<'a>>,
    mother: Option<LoreRelation<'a>>,
    children: Vec<LoreRelation<'a>>,
    circumstances: &'a [String],
    /// Authored bodily conditions plus the one *computed* drive condition the
    /// sim writes each prompt — `hungry`/`famished` off the hunger gauge
    /// (`features/food_and_items/03_hunger.md` §5). Owned, not borrowed, because
    /// the computed word is appended per render; an un-enrolled actor's hunger
    /// stays full, so nothing is appended and the frozen fixtures are unchanged.
    conditions: Vec<String>,
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
    /// a percept (`features/implemented/movement/01_the_clock.md` §7). Omitted when the host
    /// carries no clock, which keeps the frozen golden fixtures byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    the_hour: Option<String>,
    /// The weekday, likewise — without it a day worker cannot know why the
    /// round has them praying instead of working. Omitted without a clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    the_day: Option<String>,
    /// Actor-readable weather, including whether this particular body is under
    /// cover. Kept beside the clock instead of in history so current weather
    /// cannot evict dialogue.
    #[serde(skip_serializing_if = "Option::is_none")]
    weather: Option<String>,
    /// Where the current walk ends, as a full sentence — the actor's own view
    /// of what the sheet overlay calls the heading. Without it, "where are you
    /// going?" during a round-laid walk can only be answered by confabulation.
    /// Omitted while standing, which keeps the frozen fixtures byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    on_your_way: Option<String>,
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
    /// The catalog-derived display name — one source of truth, so a vendor
    /// cannot render a "loaf" that disagrees with its kind.
    name: String,
    /// The stack (or offered) quantity; rendered as `×N` when above 1.
    quantity: u32,
    /// Where units of this stack ride, when any of them do
    /// (`features/extra_pockets.md`): the `you_hold` suffix `(in your mouth)`.
    /// Only the owner's own sheet ever carries it — that is the entire point of
    /// a body pocket — so every other construction site passes `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pocketed: Option<String>,
}

/// One `you_sell` line: a kind's display name and its catalog price in sparks.
/// Borrowed from the vendor's [`crate::character::VendorListing`], which the
/// round computed at bind time.
#[derive(Serialize)]
struct SellLine<'a> {
    name: &'a str,
    price_sparks: u32,
}

/// One `places_you_know` entry. The key is `place_id`, not `id`, so a place
/// handle can never be conflated with a person handle even out of context
/// (`features/implemented/movement/05_the_llm_seam.md` §3).
#[derive(Serialize)]
struct PlaceRef<'a> {
    place_id: &'a PlaceId,
    name: &'a str,
}

/// One `word_in_the_ward` entry. The number is carried, not implied by
/// position: `settle_notice` names a notice by it, the list is filtered and
/// capped per reader, and two carriers must be able to talk about the same
/// number (`law_and_order.md` M3.5).
#[derive(Serialize)]
struct NoticeLine {
    notice_id: u64,
    word: String,
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

/// One street dog on the sheet (`features/implemented/dogs.md`): the authored
/// noun phrase, never a name or an id — no verb takes a dog, so there is
/// nothing to reference one by.
#[derive(Serialize)]
struct DogLine<'a> {
    description: &'a str,
    distance_m: f64,
    moving: bool,
}

/// A `marks_here` bullet's parts. Owned, not borrowed: the label is composed
/// (a tally's stroke count goes into it) and the site is spelled through the
/// same unknown-people rule the rest of the sheet uses, so neither outlives
/// the world borrow the way a dog's authored description does.
#[derive(Serialize)]
struct MarkLine {
    id: String,
    label: String,
    /// `on Ede Clove's door`, `at Chain Well` — already carrying its
    /// preposition, because a household reads differently from a place.
    site: String,
    distance_m: f64,
    meaning: String,
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
    let actor = llm_actor(world, actor_id)?;
    let sheet = build_sheet(world, actor, since, &env.strings);
    // The your_round explainer paragraph renders only when the sheet carries a
    // round — round-less worlds (the golden fixtures) keep their exact bytes.
    let has_round = !sheet.your_round.is_empty();
    // Likewise the ward-notice explainer tracks its section, and the
    // `raise_notice` verb (with its law paragraph) is listed only for the law
    // cast (`law_and_order.md` M3) — every other sheet keeps its exact bytes.
    let has_notices = !sheet.word_in_the_ward.is_empty();
    // …and the ward-mood explainer tracks its section, so a Major, an ambient,
    // and every sheet before the first Night Office keep their exact bytes.
    let has_ward_mood = sheet.the_ward_says.is_some();
    // The dogs explainer tracks its section too: only a sheet with a dog on it
    // pays the paragraph, and every dog-less world keeps its exact bytes.
    let has_dogs = !sheet.dogs_nearby.is_empty();
    let has_law_verbs = crate::notices::is_law(actor);
    // `settle_notice` reaches one person outside the law cast: whoever a live
    // notice names as wronged, who may forgive their own spark (M3.5). They
    // always carry that notice (`notices::carries`) and it always keeps its seat
    // past the sheet cap (`ward_notice_lines`), so the number the verb takes is
    // on their sheet.
    let has_settle_verb = has_law_verbs
        || world
            .notices
            .live()
            .iter()
            .any(|notice| notice.wronged.as_ref() == Some(actor_id));
    // The body-pocket verbs are documented only to someone who could use them
    // today — anything already pocketed, or anything palmable in hand
    // (`features/extra_pockets.md`: "an actor with empty pockets and nothing
    // palmable in hand gets zero extra tokens"). The frontbutt is named only to
    // a body that has one.
    let has_pockets = !actor.pockets().is_empty()
        || actor.holds().iter().any(|item_id| {
            world
                .items
                .get(item_id)
                .is_some_and(|item| world.item_catalog.size(item) == crate::item::ItemSize::Palmable)
        });
    let has_frontbutt = actor.has_body_slot(crate::character::BodySlot::Frontbutt);
    // `grab` is rendered only to somebody who actually has a person in charge —
    // it is the officer's deliberate counterpart to the host-side reflex, not a
    // verb the whole law cast carries around. `struggle` is rendered only to
    // somebody a hand is actually on (M4d), and `you_are_held` explains it.
    let is_held = world.custody.is_held(actor_id);
    // Keeping is wider than having *taken* (M4e/M5b), and it is one concept, not
    // two: the Stone House's keeper is nobody's officer of record — the eight
    // the city was already holding have none — so a flag built on
    // `Custody::prisoners_of` misses the one person whose whole job this is,
    // while the roster, the paragraph, `grab` and the posted fee all want the
    // same answer. `custody::kept_by` gives it once, and `in_charge_lines` reads
    // the very same list, so the sheet cannot offer a verb it has no id for.
    let has_custody = crate::custody::keeps_anyone(world, actor_id);
    let sheet_md = sheet_markdown(&sheet, &env.strings);
    let emittable_sounds = world.sound_catalog.emittable_sound_ids().join(", ");

    env.environment
        .get_template(TURN_TEMPLATE)
        .and_then(|template| {
            template.render(context! {
                sheet_md,
                sounds_enabled => world.sounds_enabled,
                emittable_sounds,
                has_round,
                has_ward_mood,
                has_dogs,
                has_notices,
                has_law_verbs,
                has_settle_verb,
                has_custody,
                is_held,
                gaol_fee => gaol_fee_phrase(),
                has_pockets,
                has_frontbutt,
            })
        })
        .map_err(|error| PromptError::new(format!("the turn template did not render: {error}")))
}

/// Render one Major's **Night Office** prompt (movement M6): the same
/// character sheet they read all day, wrapped in the bedtime instructions
/// rather than the turn ones.
///
/// It is the same sheet on purpose — `your_round` is what `set_round` edits by
/// number, `places_you_know` is what it may name, `recent_history` *is* the day
/// being reflected on, and a second sheet format would be a second thing to
/// keep true. What it does **not** do is drain: `since` stays `None`, so the
/// inbox is read and left where it is, and whatever reached this person while
/// they walked home is still news to them in the morning.
pub fn render_night_prompt(
    world: &World,
    actor_id: &ActorId,
    env: &PromptEnv,
) -> Result<String, PromptError> {
    let actor = llm_actor(world, actor_id)?;
    let sheet = build_sheet(world, actor, None, &env.strings);
    let sheet_md = sheet_markdown(&sheet, &env.strings);
    env.environment
        .get_template(NIGHT_TEMPLATE)
        .and_then(|template| {
            template.render(context! {
                sheet_md,
                is_ward => false,
                legs => sheet.your_round.len(),
            })
        })
        .map_err(|error| PromptError::new(format!("the night template did not render: {error}")))
}

/// Render one ward's **Night Office** prompt (movement M6): a hundred and
/// twenty Minors' share of reflection, bought eight prompts a night instead of
/// a hundred and twenty.
///
/// The digest is deliberately not a character sheet. A ward is not a person: it
/// has no hands, no position and no history of its own, and what it knows is
/// exactly who lives in it, what each of them is set on, what the ward is
/// saying, and where its feet can be pointed.
pub fn render_ward_prompt(
    world: &World,
    ward: PlanningWard,
    env: &PromptEnv,
) -> Result<String, PromptError> {
    let sheet_md = ward_markdown(world, ward, &env.strings);
    env.environment
        .get_template(NIGHT_TEMPLATE)
        .and_then(|template| {
            template.render(context! {
                sheet_md,
                is_ward => true,
                legs => 0,
            })
        })
        .map_err(|error| PromptError::new(format!("the night template did not render: {error}")))
}

/// The ward digest the ward branch of `night.j2` embeds — the twin of
/// [`sheet_markdown`], and, like it, all layout and no prose.
///
/// `their_places` is the ward's own registered places plus everybody's coarse
/// handles: a `set_round` edit the ward makes is the ward's decision, so the
/// night lane teaches the handle to whoever it names rather than requiring them
/// to have already known it ([`crate::night`]).
fn ward_markdown(world: &World, ward: PlanningWard, strings: &PromptStrings) -> String {
    let mut sections: Vec<String> = Vec::with_capacity(6);

    sections.push(format!("**the_ward** — {}", ward_label(ward)));
    if let Some(time) = world.current_time {
        let mut line = format!(
            "**tonight** — {} of {}",
            time.office.label(),
            time.weekday.label()
        );
        if let Some(weather) = world.current_weather {
            line.push_str(&format!("; {}", weather.prompt_phrase(None)));
        }
        sections.push(line);
    }

    // Everybody the batch speaks for, in roster order, with the one fact that
    // says where a night might move them: what they are set on.
    sections.push(bullet_section(
        &format!("**your_people** ({})", strings.ward_people_note),
        ward_minors(world, ward).map(|actor| {
            let trade = actor
                .lore()
                .and_then(|profile| {
                    profile
                        .title
                        .as_deref()
                        .or(profile.occupation_display.as_deref())
                })
                .unwrap_or("no fixed trade");
            format!(
                "{} — {}, {trade} — set on: {}",
                actor.id(),
                actor.name(),
                actor.goal()
            )
        }),
        &strings.nobody,
    ));

    sections.push(bullet_section(
        &format!("**the_word** ({})", strings.notices_note),
        world
            .notices
            .live()
            .iter()
            .rev()
            .map(|notice| notice.line()),
        &strings.nothing,
    ));

    // The ward's own places first, then the coarse handles everyone holds. A
    // major square is both, so the pass is de-duplicated by id — `Vec::dedup`
    // would not do it, the two runs are not adjacent.
    let mut seen: std::collections::BTreeSet<&PlaceId> = std::collections::BTreeSet::new();
    let places: Vec<String> = world
        .places
        .ward_places(ward.as_str())
        .chain(world.places.coarse())
        .filter(|entry| seen.insert(&entry.id))
        .map(|entry| format!("{} {}", entry.id, entry.name))
        .collect();
    sections.push(bullet_section(
        &format!("**their_places** ({})", strings.ward_places_note),
        places.into_iter(),
        &strings.no_places,
    ));

    sections.push(format!(
        "**last_night** — {}",
        world
            .ward_moods
            .get(&ward)
            .map(String::as_str)
            .unwrap_or(&strings.nothing_yet)
    ));

    sections.join("\n\n")
}

/// The ward's [`Minor`](crate::lore::Significance::Minor) cast, in roster
/// order — exactly the people a ward batch reflects *for*, and the only people
/// its `set_round` edits may name.
pub(crate) fn ward_minors(
    world: &World,
    ward: PlanningWard,
) -> impl Iterator<Item = &Character> + '_ {
    world.roster.iter().filter_map(move |actor_id| {
        let actor = world.characters.get(actor_id)?;
        let profile = actor.lore()?;
        (profile.significance == crate::lore::Significance::Minor
            && profile.planning_ward == ward
            && actor.control().is_llm()
            && world.is_present(actor_id))
        .then_some(actor)
    })
}

/// The ward as its people name it — the `snake_case` planning key turned back
/// into the words on the wayfinding registry's ward anchor ("Cinder Ward"), or
/// the key itself if the registry has no anchor for it (a nav-less world).
fn ward_label(ward: PlanningWard) -> String {
    match ward {
        PlanningWard::Fabric => "the Fabric Ward".to_string(),
        PlanningWard::Wick => "the Wick Ward".to_string(),
        PlanningWard::Cloth => "the Cloth Ward".to_string(),
        PlanningWard::Wallwright => "the Wallwright Ward".to_string(),
        PlanningWard::Cinder => "the Cinder Ward".to_string(),
        PlanningWard::Weigh => "the Weigh Ward".to_string(),
        PlanningWard::Reed => "the Reed Ward".to_string(),
        PlanningWard::BellAndSluice => "the Bell and Sluice Ward".to_string(),
    }
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
    if !world.is_present(actor_id) {
        return Err(PromptError::new(format!(
            "actor '{actor_id}' is beyond the walls"
        )));
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
            person(
                actor,
                other,
                strings,
                Some(distance),
                Some(!other.is_settled()),
            )
        })
        .collect();

    // The street dogs within the same 20 m the people section uses — one
    // radius for "nearby", not two. Ordered by (distance², id) exactly like
    // `characters_within`, so the list reads stably turn after turn.
    let mut dog_matches: Vec<(f64, &crate::dogs::Dog)> = world
        .dogs
        .iter()
        .filter_map(|dog| {
            let distance_squared = actor.position_m().distance_squared(dog.position_m);
            (distance_squared <= HEARING_RADIUS_M * HEARING_RADIUS_M)
                .then_some((distance_squared, dog))
        })
        .collect();
    dog_matches.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .expect("positions are finite")
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    let dogs_nearby: Vec<DogLine<'_>> = dog_matches
        .into_iter()
        .map(|(distance_squared, dog)| DogLine {
            description: &dog.description,
            distance_m: py_round(distance_squared.sqrt(), 1),
            moving: dog.is_moving(),
        })
        .collect();

    // The chalk within sight. `marks_within` already sorts nearest-first with
    // an id tie-break, so a replay renders the same bullets in the same order.
    // The occupant of a chalked household is spelled through the sheet's own
    // unknown-people rule — a cross on a stranger's door names the stranger
    // exactly as `you_see` would, and never leaks a name the actor has not
    // been told.
    let marks_here: Vec<MarkLine> = crate::marks::marks_within(
        world,
        actor.position_m(),
        crate::marks::MARK_NOTICE_RADIUS_M,
    )
    .into_iter()
    .map(|near| {
        let site = match &near.occupant {
            // Your own door first. Nobody's `knows` set contains their own id,
            // so without this clause the debtor standing at their own chalked
            // door would be told it belongs to "a stranger (you don't know
            // their name)" — absurd, and since M1's cross is drawn on exactly
            // that door, the most likely line this section will ever render.
            Some(owner) if owner == actor.id() => "on your own door".to_string(),
            Some(owner) => {
                let whose = world
                    .characters
                    .get(owner)
                    .filter(|_| actor.knows().contains(owner))
                    .map_or(strings.unknown_person_name.as_str(), |other| other.name());
                format!("on {whose}'s door")
            }
            None => format!("at {}", near.site_label),
        };
        MarkLine {
            id: near.id.to_string(),
            label: near.label,
            site,
            distance_m: py_round(near.distance_m, 1),
            meaning: near.meaning,
        }
    })
    .collect();

    let mut sorted_offers: Vec<&Offer> = world.offers.values().collect();
    sorted_offers.sort_by_key(|offer| offer_sort_key(offer));

    let mut you_offer: Vec<YouOffer<'_>> = Vec::new();
    let mut offered_to_you: Vec<OfferedToYou<'_>> = Vec::new();
    for offer in sorted_offers {
        if !world.is_present(&offer.giver_id)
            || offer
                .target_id
                .as_ref()
                .and_then(|target| world.characters.get(target))
                .is_some_and(|target| target.state.presence != crate::Presence::InCity)
        {
            continue;
        }
        // An offer of an item that has left the world shows in neither section.
        let Some(entity) = world.items.get(&offer.item_id) else {
            continue;
        };
        let item = ItemRef {
            id: &offer.item_id,
            name: world.item_catalog.display_name(entity),
            // The offer's own quantity, which may be a slice of a larger stack.
            quantity: offer.quantity,
            // A pocketed unit cannot be offered at all, so an offer line never
            // carries the suffix.
            pocketed: None,
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
        home: profile.home.as_deref(),
        home_place_id: world.places.home_of(actor.id()).map(|entry| &entry.id),
        father: profile.father.as_ref().map(&relation),
        mother: profile.mother.as_ref().map(&relation),
        children: profile.children.iter().map(relation).collect(),
        circumstances: &profile.circumstances,
        conditions: {
            // Authored conditions first, then the computed drive word (if any),
            // so the sheet reads "…, travel-worn, famished." — recomputed each
            // prompt, which is how eating clears the line with no memory hygiene.
            let mut conditions = profile.conditions.clone();
            if let Some(word) = hunger_condition(actor.needs().hunger) {
                conditions.push(word.to_string());
            }
            conditions
        },
    });
    Sheet {
        name: actor.name(),
        lore_profile,
        back_story: actor.back_story(),
        you_are: YouAre {
            location_description,
            the_hour: world
                .current_time
                .map(|time| format!("{} — {}", time.office.label(), time.office.prompt_phrase())),
            the_day: world.current_time.map(|time| {
                format!(
                    "{} — {}",
                    time.weekday.label(),
                    time.weekday.prompt_phrase()
                )
            }),
            weather: world.current_time.and_then(|_| {
                world
                    .current_weather
                    .map(|weather| weather.prompt_phrase(world.shelters.label_at(position)))
            }),
            on_your_way: on_your_way(world, actor, strings),
            position_m: Position {
                x: position.x,
                y: position.y,
                z: position.z,
            },
        },
        you_are_held: held_line(world, actor.id()),
        you_have_in_charge: in_charge_lines(world, actor),
        your_round: actor.state.daily_round.iter().map(String::as_str).collect(),
        the_ward_says: actor
            .lore()
            .filter(|profile| profile.significance == crate::lore::Significance::Minor)
            .and_then(|profile| world.ward_moods.get(&profile.planning_ward))
            .map(String::as_str),
        places_you_know,
        you_hold: actor
            .holds()
            .iter()
            // A dangling id is silently skipped (`prompt.py:222-226`).
            .filter_map(|item_id| {
                world.items.get(item_id).map(|item| ItemRef {
                    id: item_id,
                    name: world.item_catalog.display_name(item),
                    quantity: item.quantity,
                    pocketed: pocket_note(actor, item_id, item.quantity, strings),
                })
            })
            .collect(),
        you_sell: actor
            .state
            .you_sell
            .iter()
            .map(|listing| SellLine {
                name: &listing.name,
                price_sparks: listing.price_sparks,
            })
            .collect(),
        you_offer,
        offered_to_you,
        you_see: YouSee {
            description: &strings.you_see_description,
            people,
        },
        dogs_nearby,
        marks_here,
        word_in_the_ward: ward_notice_lines(world, actor.id()),
        since_your_last_turn,
        recent_history,
        stored_memories: actor.memories(),
        the_only_languages_you_know: &strings.languages,
        current_goal: actor.goal(),
    }
}

/// The posted gaol fee, as a keeper would say it aloud. One function, so the
/// keeper's paragraph and the prisoner's `you_are_held` line cannot come to
/// disagree about the number or about how to count it — *"gaol fees are fixed
/// publicly; inventing a fee is extortion"*, and two spellings of one fee is a
/// small way of inventing one.
fn gaol_fee_phrase() -> String {
    let sparks = crate::custody::GAOL_FEE_SPARKS;
    if sparks == 1 {
        "one spark".to_string()
    } else {
        format!("{sparks} sparks")
    }
}

/// The `you_are_held` section (`law_and_order.md` M4b′), or `None` — which is
/// everybody, nearly always. Three facts and no more: who has you, where you
/// are being taken, and what would end it. The last one is not decoration: a
/// brand with a visible door is a story and a brand with no door is a bug, and
/// the same promise the player's HUD line makes is owed to the cast.
fn held_line(world: &World, actor_id: &ActorId) -> Option<String> {
    let record = world.custody.get(actor_id)?;
    let who = record
        .officer
        .as_ref()
        .and_then(|officer| world.characters.get(officer))
        .map(|officer| {
            if world.characters[actor_id].knows().contains(officer.id()) {
                officer.name().to_string()
            } else {
                format!("someone who serves the law (id {})", officer.id())
            }
        });
    let mut line = match (&who, record.state) {
        (Some(who), crate::custody::Confinement::InCharge) => format!(
            "{who} has you in charge and is walking you to {}",
            record.station.name
        ),
        (Some(who), crate::custody::Confinement::Committed) => format!(
            "{who} brought you to {}, and here you are kept",
            record.station.name
        ),
        (None, _) => format!("you are held at {}", record.station.name),
    };
    if record.is_held() {
        line.push_str("; a hand is on your arm, and struggle is the only way out of that");
    }
    match record
        .notice_id
        .and_then(|notice_id| world.notices.get(notice_id))
    {
        // What would free you, always named. Restitution settles the word and
        // the word is what holds you; an officer may also simply let you go.
        Some(notice) => line.push_str(&format!(
            ". The word against you is notice {} — {}. Settling that word, or the law choosing to let you go, is what ends this",
            notice.id,
            notice.line()
        )),
        None => line.push_str(
            ". Only the law choosing to let you go ends this; ask them what it would take",
        ),
    }
    // The fee is posted on the wall, so the prisoner can read it as plainly as
    // the keeper can quote it — that is the whole point of a public fee, and it
    // is what keeps "what would end this" from being a mystery box. It buys
    // nothing by itself: the keeper still chooses (M3.5's bribery-by-omission).
    line.push_str(&format!(
        ". The posted gaol fee is {}; offering it to whoever keeps you is the plain way to ask, though taking it and freeing you is still their choice, and somebody who knows you and will come and speak for you does as well",
        gaol_fee_phrase()
    ));
    // The sentence, stated in the clock everything else in this world is stated
    // in (M5c) — and in the Stone House the bell that ends it rings directly
    // overhead, which is what makes it a clock you are serving time against
    // rather than a number on a screen.
    if let Some(office) = record.sentence_office {
        line.push_str(&format!(
            ". You were told you go at {}; the bell rings over this very roof",
            office.label()
        ));
    }
    Some(line)
}

/// The officer's side of custody: one bullet per person they have taken, each
/// carrying the id `release` and `grab` need. Empty — so the section vanishes —
/// for everyone who has taken nobody, which is the whole city.
fn in_charge_lines(world: &World, actor: &Character) -> Vec<String> {
    // `custody::kept_by`, not `Custody::prisoners_of`: the Stone House's keeper
    // took nobody and holds nobody — the eight the city was already holding have
    // no arresting officer — so the roster has to be "whom am I keeping", which
    // is the same predicate `is_keeper` gates the paragraph on.
    crate::custody::kept_by(world, actor.id())
        .into_iter()
        .filter_map(|prisoner_id| {
            let record = world.custody.get(&prisoner_id)?;
            let prisoner = world.characters.get(&prisoner_id)?;
            let who = if actor.knows().contains(prisoner.id()) {
                format!("{} (id {prisoner_id})", prisoner.name())
            } else {
                format!("a stranger (id {prisoner_id})")
            };
            let where_to = match record.state {
                crate::custody::Confinement::InCharge => {
                    format!("walking to {}", record.station.name)
                }
                crate::custody::Confinement::Committed => {
                    format!("kept at {}", record.station.name)
                }
            };
            let grip = if record.holders.contains(actor.id()) {
                " — you have hold of their arm"
            } else if record.is_held() {
                " — somebody else has hold of them"
            } else {
                " — nobody has hold of them; they walk of their own accord"
            };
            Some(format!("{who} — {where_to}{grip}"))
        })
        .collect()
}

/// Where units of one held stack ride, as the `you_hold` suffix — `in your
/// mouth`, or `1 in your mouth; 1 carried privily, behind` when a stack is
/// split across cavities (`features/extra_pockets.md`). The count is dropped
/// when the whole stack rides one slot: "×1 spark (1 in your mouth)" would be
/// pure noise. `None` — the overwhelmingly common case — renders nothing at all.
fn pocket_note(
    actor: &Character,
    item_id: &ItemId,
    quantity: u32,
    strings: &PromptStrings,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for slot in crate::character::BodySlot::ALL {
        let count = actor
            .pockets()
            .iter()
            .filter(|unit| unit.slot == slot && &unit.item_id == item_id)
            .count();
        if count == 0 {
            continue;
        }
        let phrase = match slot {
            crate::character::BodySlot::Mouth => &strings.pocket_mouth_note,
            crate::character::BodySlot::Butt => &strings.pocket_butt_note,
            crate::character::BodySlot::Frontbutt => &strings.pocket_frontbutt_note,
        };
        parts.push(if count as u32 == quantity {
            phrase.clone()
        } else {
            format!("{count} {phrase}")
        });
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// The `word_in_the_ward` section: the live notices this actor carries, newest
/// first and capped at [`crate::notices::NOTICES_SHEET_MAX`] — except that a
/// notice naming *them* as wronged never loses its seat to a newer one.
///
/// That exception is what the verb stands on (`law_and_order.md` M3.5).
/// `settle_notice` takes a number and the wronged party reads that number off
/// this section, so a plain newest-first cap could bury their own word behind
/// four later ones their curiosity roll happened to make them a carrier of —
/// leaving the turn prompt telling the one person entitled to forgive the wrong
/// to settle a notice their sheet does not show, which they can only guess at
/// or let stand. The rendering order is still newest first: the exception picks
/// which notices get a seat, never where they sit.
fn ward_notice_lines(world: &World, actor_id: &ActorId) -> Vec<NoticeLine> {
    let carried: Vec<&crate::notices::WardNotice> = world
        .notices
        .live()
        .iter()
        .rev()
        .filter(|notice| crate::notices::carries(world, actor_id, notice.id))
        .collect();
    let wronged_here =
        |notice: &crate::notices::WardNotice| notice.wronged.as_ref() == Some(actor_id);
    // Their own words claim seats first, then the newest of the rest fill what
    // is left. Ids, not indices: the cap is chosen in priority order and spent
    // back in the ward's own order below.
    let seated: Vec<u64> = carried
        .iter()
        .filter(|notice| wronged_here(notice))
        .chain(carried.iter().filter(|notice| !wronged_here(notice)))
        .map(|notice| notice.id)
        .take(crate::notices::NOTICES_SHEET_MAX)
        .collect();
    carried
        .into_iter()
        .filter(|notice| seated.contains(&notice.id))
        .map(|notice| NoticeLine {
            notice_id: notice.id,
            word: notice.line(),
        })
        .collect()
}

/// The sheet's sentence for where the current walk ends, or `None` while
/// standing. The name is resolved exactly as the arrival will speak of it:
/// the actor's own `go_to` when the walk serves it (compared by endpoint —
/// an intent the round has not picked up yet must not mislabel the walk it
/// is still finishing), the patrol's far end, else the nearest registered
/// place, else the area. A destination nothing names renders no line at all
/// — bare coordinates would read as a map reference no medieval walker has.
fn on_your_way(world: &World, actor: &Character, strings: &PromptStrings) -> Option<String> {
    let movement = actor.state.movement.as_ref()?;
    // An empty path is an arrival, not a walk.
    let end = *movement.path.last()?;
    let planar = |target: crate::math::Vec3| f64::hypot(end.x - target.x, end.z - target.z);
    if let Some(patrol) = &movement.patrol {
        let name = if patrol.heading_to_b {
            &patrol.b
        } else {
            &patrol.a
        };
        return Some(strings.walking_to.replacen("%s", name, 1));
    }
    if let Some(intent) = &actor.state.intent {
        match &intent.target {
            IntentTarget::Place { name, point, .. } if planar(*point) <= PLACE_ARRIVE_RADIUS_M => {
                return Some(strings.walking_to.replacen("%s", name, 1));
            }
            IntentTarget::Person {
                actor_id,
                last_seen,
                ..
            } if planar(*last_seen) <= PLACE_ARRIVE_RADIUS_M => {
                if let Some(other) = world.characters.get(actor_id) {
                    let who = person_md(&person(actor, other, strings, None, None));
                    return Some(strings.following.replacen("%s", &who, 1));
                }
            }
            _ => {}
        }
    }
    let name = world
        .places
        .nearest(end, WALK_DESTINATION_SNAP_M)
        .map(|place| place.name.clone())
        .or_else(|| world.area_map.location_description(end))?;
    Some(strings.walking_to.replacen("%s", &name, 1))
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

    // Being in the law's hands sits directly under `you_are`, because it is the
    // most urgent fact about where this body is and what it may do. Omitted
    // entirely for everybody else, so no other sheet moves a byte.
    if let Some(held) = &sheet.you_are_held {
        sections.push(format!("**you_are_held** — {held}"));
    }
    if !sheet.you_have_in_charge.is_empty() {
        sections.push(bullet_section(
            "**you_have_in_charge**",
            sheet.you_have_in_charge.iter().map(String::clone),
            "",
        ));
    }

    if !sheet.your_round.is_empty() {
        // Numbered from 1, because `set_round` names a leg by its number and
        // the sheet is the only place the model can read one off (M6). The
        // number is structure, like the section labels, so it lives here.
        sections.push(bullet_section(
            &format!("**your_round** ({})", strings.round_note),
            sheet
                .your_round
                .iter()
                .enumerate()
                .map(|(index, leg)| format!("leg {} — {leg}", index + 1)),
            "",
        ));
    }

    // What the ward is saying tonight (M6): the Minors' whole share of the
    // Night Office, carried by everyone the batch spoke for. Omitted entirely
    // until a ward has reflected, which keeps the frozen fixtures stable.
    if let Some(mood) = sheet.the_ward_says {
        sections.push(format!(
            "**the_ward_says** ({}) — {mood}",
            strings.ward_says_note
        ));
    }

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

    // A bound vendor's price list — omitted entirely for everyone else, like
    // `you_offer`, so the section never appears on a non-vendor's sheet.
    if !sheet.you_sell.is_empty() {
        sections.push(bullet_section(
            &format!("**you_sell** ({})", strings.sell_note),
            sheet.you_sell.iter().map(sell_md),
            "",
        ));
    }

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

    // The street dogs — under `you_see`, where the eyes are. Omitted entirely
    // when none is near, like `you_sell`, so a dog-less sheet never moves a
    // byte.
    if !sheet.dogs_nearby.is_empty() {
        sections.push(bullet_section(
            &format!("**dogs_nearby** ({})", strings.dogs_note),
            sheet.dogs_nearby.iter().map(dog_bullet),
            "",
        ));
    }

    // The chalk — beside the dogs, where the eyes are, and omitted entirely
    // when the walls are bare so an unchalked sheet never moves a byte.
    if !sheet.marks_here.is_empty() {
        sections.push(bullet_section(
            &format!("**marks_here** ({})", strings.marks_note),
            sheet.marks_here.iter().map(mark_bullet),
            "",
        ));
    }

    // The ward's word — omitted entirely for the carrier-less majority, like
    // `you_sell`, so the section never appears on an untouched sheet.
    if !sheet.word_in_the_ward.is_empty() {
        // Numbered like `your_round`'s legs, and for the same reason: since
        // M3.5 `settle_notice` names a notice by its number, and the sheet is
        // the only place the model can read one off.
        sections.push(bullet_section(
            &format!("**word_in_the_ward** ({})", strings.notices_note),
            sheet
                .word_in_the_ward
                .iter()
                .map(|notice| format!("notice {} — {}", notice.notice_id, notice.word)),
            "",
        ));
    }

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

    if let Some(home) = lore.home {
        line.push_str(&format!(" {} {home}", strings.home_label));
        if let Some(place_id) = lore.home_place_id {
            line.push_str(&format!(" ({} {place_id})", strings.home_place_label));
        }
        line.push('.');
    }
    if let Some(role) = lore.faction_role {
        line.push_str(&format!(" {} {role}.", strings.faction_role_label));
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
        line.push_str(&format!(
            " Circumstances: {}.",
            lore.circumstances.join(", ")
        ));
    }
    if !lore.conditions.is_empty() {
        line.push_str(&format!(" Conditions: {}.", lore.conditions.join(", ")));
    }
    line
}

/// The computed drive condition the hunger gauge renders into the sheet's
/// `Conditions:` line — the one need surfaced as a word rather than left
/// un-narrated like thirst (`features/food_and_items/03_hunger.md` §5). `None`
/// above [`crate::HUNGER_HUNGRY`], which is where an un-enrolled (always full)
/// actor sits, so it never touches the frozen fixtures.
fn hunger_condition(hunger: f64) -> Option<&'static str> {
    if hunger < crate::HUNGER_FAMISHED {
        Some("famished")
    } else if hunger < crate::HUNGER_HUNGRY {
        Some("hungry")
    } else {
        None
    }
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
    if let Some(day) = &you_are.the_day {
        line.push_str(&format!(" {} {day}.", strings.the_day_label));
    }
    if let Some(weather) = &you_are.weather {
        line.push_str(&format!(" {weather}."));
    }
    if let Some(on_your_way) = &you_are.on_your_way {
        line.push_str(&format!(" {on_your_way}"));
    }
    line
}

/// `fzbn9 herring` — items are always id-first, like places — with a `×N` count
/// suffix when the stack (or offered) quantity is above 1: `c0prs spark ×7`.
/// Single-item traffic keeps its exact bytes.
fn item_md(item: &ItemRef<'_>) -> String {
    let mut line = if item.quantity > 1 {
        format!("{} {} ×{}", item.id, item.name, item.quantity)
    } else {
        format!("{} {}", item.id, item.name)
    };
    // The body-pocket suffix, on the owner's own `you_hold` line only.
    if let Some(pocketed) = &item.pocketed {
        line.push_str(&format!(" ({pocketed})"));
    }
    line
}

/// `loaf, 2 sparks` — one `you_sell` line: the kind's display name and its
/// price, singular `spark` only at exactly one (matching the purchase percept's
/// `for 1 spark`).
fn sell_md(listing: &SellLine<'_>) -> String {
    let unit = if listing.price_sparks == 1 {
        "spark"
    } else {
        "sparks"
    };
    format!("{}, {} {unit}", listing.name, listing.price_sparks)
}

/// `id cb947: Conny` — how a person is referenced outside `you_see`.
fn person_md(person: &Person<'_>) -> String {
    format!("id {}: {}", person.id, person.name)
}

/// The `dogs_nearby` bullet: `a rangy brindle dog, 6.3 m, moving` — the
/// person bullet's shape without an id, because no verb takes a dog.
fn dog_bullet(dog: &DogLine<'_>) -> String {
    let mut line = format!("{}, {:.1} m", dog.description, dog.distance_m);
    if dog.moving {
        line.push_str(", moving");
    }
    line
}

/// The `marks_here` bullet:
/// `mark 3: a chalk cross at knee height, on Ede Clove's door, 3.1 m — this
/// household owes and has not paid`.
///
/// The id leads, unlike a dog's line, because `scrub_mark` takes one and the
/// sheet is the only place the model can read it off — the same reason the
/// ward's notices are numbered.
fn mark_bullet(mark: &MarkLine) -> String {
    format!(
        "mark {}: {}, {}, {:.1} m — {}",
        mark.id, mark.label, mark.site, mark.distance_m, mark.meaning
    )
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
fn bullet_section(header: &str, entries: impl Iterator<Item = String>, empty_word: &str) -> String {
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
    bullet_section(
        header,
        lines.iter().map(|line| (*line).to_string()),
        sentinel,
    )
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
            sell_note: "your stall's prices".into(),
            the_hour_label: "The hour:".into(),
            the_day_label: "The day:".into(),
            round_note: "your standing day, leg by leg; each begins when its bell rings".into(),
            notices_note: "what the ward is saying; hearsay and descriptions, not proof".into(),
            ward_says_note: "how your ward feels this morning".into(),
            ward_people_note: "the ordinary householders and trades of this ward".into(),
            ward_places_note: "set_round may name any of these place_ids".into(),
            walking_to: "You are on your way to %s.".into(),
            following: "You are following %s.".into(),
            dogs_note: "street dogs within 20 metres, nearest first".into(),
            marks_note: "chalk on the walls within 8 metres, nearest first".into(),
            faction_role_label: "Faction role:".into(),
            illegal_activity_label: "In secret:".into(),
            home_label: "Home:".into(),
            home_place_label: "go_to".into(),
            pocket_mouth_note: "in your mouth".into(),
            pocket_butt_note: "carried privily, behind".into(),
            pocket_frontbutt_note: "carried privily, before".into(),
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
        let toml = "unknown_person_name = \"a\"\nyou_see_description = \"b\"\nnothing = \"c\"\nnothing_yet = \"d\"\noffer_to_anyone = \"e\"\nlanguages = \"f\"\naccept_with = \"no placeholder\"\nnobody = \"g\"\nno_memories = \"h\"\nno_places = \"i\"\nholding_nothing = \"j\"\nplaces_note = \"k\"\nsell_note = \"s\"\nthe_hour_label = \"l\"\nthe_day_label = \"p\"\nround_note = \"q\"\nnotices_note = \"t\"\nward_says_note = \"x\"\nward_people_note = \"y\"\nward_places_note = \"z\"\nwalking_to = \"to %s\"\nfollowing = \"after %s\"\ndogs_note = \"dd\"\nmarks_note = \"mm\"\nfaction_role_label = \"r\"\nillegal_activity_label = \"m\"\nhome_label = \"n\"\nhome_place_label = \"o\"\npocket_mouth_note = \"u\"\npocket_butt_note = \"v\"\npocket_frontbutt_note = \"w\"\n";
        let error = PromptEnv::new("x", "y", toml).unwrap_err();
        assert!(error.message.contains("%s"), "{}", error.message);
    }

    #[test]
    fn bullet_sections_render_bullets_or_the_inline_empty_word() {
        assert_eq!(
            bullet_section(
                "**you_hold**",
                ["a b", "c d"].iter().map(|s| (*s).to_string()),
                "nothing"
            ),
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
        let home_place = PlaceId::from_raw("pl_x9k2");
        let sheet = Sheet {
            name: "Corin Copp",
            lore_profile: Some(PromptLoreProfile {
                age: 26,
                gender: "m",
                occupation: Some("Scribe and clerk"),
                title: Some("Scrivener"),
                rank: Some("journeyman"),
                faction_role: Some("informant for the Watch"),
                illegal_activity: Some("forger"),
                district: "The Tallage",
                home: Some("a house in the Weigh Ward, near the Tallage"),
                home_place_id: None,
                father: None,
                mother: Some(LoreRelation {
                    id: &mother_id,
                    name: "Osanne Skell".into(),
                }),
                children: Vec::new(),
                circumstances: &[],
                conditions: Vec::new(),
            }),
            back_story: "",
            you_are: YouAre {
                location_description: String::new(),
                the_hour: None,
                the_day: None,
                weather: None,
                on_your_way: None,
                position_m: Position {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
            you_are_held: None,
            you_have_in_charge: Vec::new(),
            your_round: Vec::new(),
            the_ward_says: None,
            places_you_know: Vec::new(),
            you_hold: Vec::new(),
            you_sell: Vec::new(),
            you_offer: Vec::new(),
            offered_to_you: Vec::new(),
            you_see: YouSee {
                description: "",
                people: Vec::new(),
            },
            dogs_nearby: Vec::new(),
            marks_here: Vec::new(),
            word_in_the_ward: Vec::new(),
            since_your_last_turn: Vec::new(),
            recent_history: Vec::new(),
            stored_memories: &[],
            the_only_languages_you_know: "English",
            current_goal: "None",
        };
        assert_eq!(
            you_line(&sheet, &strings()),
            "**you** — Corin Copp, 26, male — Scrivener (Scribe and clerk, journeyman) \
             of The Tallage. Home: a house in the Weigh Ward, near the Tallage. \
             Faction role: informant for the Watch. In secret: forger. \
             Family: mother Osanne Skell (id br2sk)."
        );

        // With the registry handle the round's seed mints for the actor's own
        // door, the same sentence carries its go_to hint.
        let mut with_handle = sheet;
        with_handle.lore_profile.as_mut().unwrap().home_place_id = Some(&home_place);
        assert!(
            you_line(&with_handle, &strings())
                .contains("Home: a house in the Weigh Ward, near the Tallage (go_to pl_x9k2).")
        );

        // The bedless framing is just another home string — the bake wrote the
        // words, the line renders whatever it says. No door, no handle.
        let mut bedless = with_handle;
        {
            let lore = bedless.lore_profile.as_mut().unwrap();
            lore.home =
                Some("no fixed bed — you sleep rough in the Reed Ward, near the Alder Moorings");
            lore.home_place_id = None;
        }
        assert!(you_line(&bedless, &strings()).contains(
            "Home: no fixed bed — you sleep rough in the Reed Ward, near the Alder Moorings."
        ));

        // A title equal to the occupation is not repeated — no "Anchoress
        // (Anchoress)".
        let mut same_title = bedless;
        {
            let lore = same_title.lore_profile.as_mut().unwrap();
            lore.title = Some("Scribe and clerk");
            lore.rank = None;
            lore.faction_role = None;
            lore.illegal_activity = None;
            lore.mother = None;
            lore.home = None;
        }
        assert_eq!(
            you_line(&same_title, &strings()),
            "**you** — Corin Copp, 26, male — Scribe and clerk of The Tallage."
        );

        // No lore at all: the name stands alone.
        let plain = Sheet {
            lore_profile: None,
            ..same_title
        };
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
