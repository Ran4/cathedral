//! Explicit v1 private records. These definitions intentionally do not inherit
//! seed defaults or skip rules. A new runtime field makes the remote constructor
//! fail to compile until its checkpoint disposition is reviewed.
#![allow(dead_code)] // Several adapters are for the following composition cut.
use super::serde_support::{remote_adapters, required_option, unique_map, unique_set};
use crate::{
    appearance::{AppearanceSnapshot, Build, Headgear, OutfitClass},
    character::*,
    crowd::GeneratedRoutine,
    gesture::GestureKind,
    ids::{ActorId, ItemId, PlaceId},
    inventory::*,
    item::{Item, ItemKind},
    lore::{LoreProfile, PlanningWard, Significance},
    math::Vec3,
    places::PlaceEntry,
    receipts::{CommandId, OperationId},
    round::{
        motion,
        residents::{ResidentPhase, ResidentStatus},
    },
    world::NeedleClaim,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::Character", deny_unknown_fields)]
pub(crate) struct CharacterV1 {
    #[serde(with = "CharacterSheetV1")]
    sheet: CharacterSheet,
    #[serde(with = "CharacterStateV1")]
    state: CharacterState,
}
remote_adapters!(character, Character, CharacterV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::CharacterSheet", deny_unknown_fields)]
pub(crate) struct CharacterSheetV1 {
    id: ActorId,
    #[serde(with = "TextV1")]
    name: String,

    control: Control,
    #[serde(with = "TextV1")]
    back_story: String,
    #[serde(with = "TextV1")]
    location_description: String,
    #[serde(with = "AppearanceSnapshotV1")]
    appearance: AppearanceSnapshot,
    #[serde(with = "text::option")]
    voice_key: Option<String>,
    #[serde(with = "crate::math::vec3_serde")]
    position_m: Vec3,

    facing_yaw: f64,

    holds: Vec<ItemId>,
    #[serde(with = "pocket::vec")]
    pockets: Vec<PocketedUnit>,
    #[serde(deserialize_with = "required_option")]
    frontbutt: Option<bool>,
    #[serde(with = "TextV1")]
    goal: String,
    #[serde(with = "text::vec")]
    memories: Vec<String>,
    #[serde(with = "unique_set")]
    knows: BTreeSet<ActorId>,
    #[serde(with = "lore::option")]
    lore: Option<LoreProfile>,

    presence: Presence,

    presence_epoch: u64,

    economic_class: EconomicClass,
}
remote_adapters!(sheet, CharacterSheet, CharacterSheetV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::CharacterState", deny_unknown_fields)]
pub(crate) struct CharacterStateV1 {
    #[serde(with = "crate::math::vec3_serde")]
    position_m: Vec3,

    facing_yaw: f64,

    holds: Vec<ItemId>,
    #[serde(with = "pocket::vec")]
    pockets: Vec<PocketedUnit>,
    #[serde(with = "gut::vec")]
    gut: Vec<GutEntry>,
    #[serde(deserialize_with = "required_option")]
    urgency_since_game_days: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    debug_urgency: Option<f64>,
    #[serde(with = "TextV1")]
    goal: String,
    #[serde(with = "text::vec")]
    memories: Vec<String>,
    #[serde(with = "unique_set")]
    knows: BTreeSet<ActorId>,
    #[serde(with = "text::vec")]
    inbox: Vec<String>,
    #[serde(with = "text::vec")]
    recent_history: Vec<String>,
    #[serde(with = "text::vec")]
    pending_history: Vec<String>,
    #[serde(with = "movement::option")]
    movement: Option<Movement>,
    #[serde(with = "NeedsV1")]
    needs: Needs,
    #[serde(with = "unique_set")]
    places_known: BTreeSet<PlaceId>,
    #[serde(with = "travel::option")]
    intent: Option<TravelIntent>,
    #[serde(with = "text::vec")]
    daily_round: Vec<String>,
    #[serde(with = "resident::option")]
    resident: Option<crate::round::residents::ResidentStatus>,
    #[serde(with = "round_edit::option")]
    round_edit: Option<RoundEdit>,
    #[serde(with = "vendor::vec")]
    you_sell: Vec<VendorListing>,
    #[serde(with = "gesture::option")]
    active_gesture: Option<ActiveGesture>,

    leaving_city: bool,
    #[serde(with = "unique_map")]
    statuses: BTreeMap<StatusKind, f64>,

    presence: Presence,

    presence_epoch: u64,

    economic_class: EconomicClass,
}
remote_adapters!(state, CharacterState, CharacterStateV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::Patrol", deny_unknown_fields)]
pub(crate) struct PatrolV1 {
    #[serde(with = "TextV1")]
    a: String,
    #[serde(with = "TextV1")]
    b: String,

    heading_to_b: bool,
}
remote_adapters!(patrol, Patrol, PatrolV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::Movement", deny_unknown_fields)]
pub(crate) struct MovementV1 {
    exact_local: bool,
    #[serde(with = "point::vec")]
    path: Vec<Vec3>,

    speed: f64,

    gait_phase: f64,
    #[serde(with = "patrol::option")]
    patrol: Option<Patrol>,

    choke_wait: f64,
}
remote_adapters!(movement, Movement, MovementV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::PocketedUnit", deny_unknown_fields)]
pub(crate) struct PocketedUnitV1 {
    slot: BodySlot,

    item_id: ItemId,
}
remote_adapters!(pocket, PocketedUnit, PocketedUnitV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::GutEntry", deny_unknown_fields)]
pub(crate) struct GutEntryV1 {
    kind: crate::item::ItemKind,
    #[serde(with = "unique_map")]
    metadata: std::collections::BTreeMap<String, String>,

    due_game_days: f64,
}
remote_adapters!(gut, GutEntry, GutEntryV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::Needs", deny_unknown_fields)]
pub(crate) struct NeedsV1 {
    thirst: f64,

    hunger: f64,
}
remote_adapters!(needs, Needs, NeedsV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::VendorListing", deny_unknown_fields)]
pub(crate) struct VendorListingV1 {
    #[serde(with = "TextV1")]
    name: String,

    price_sparks: u32,
}
remote_adapters!(vendor, VendorListing, VendorListingV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::ActiveGesture", deny_unknown_fields)]
pub(crate) struct ActiveGestureV1 {
    kind: GestureKind,
    #[serde(deserialize_with = "required_option")]
    deadline: Option<f64>,
}
remote_adapters!(gesture, ActiveGesture, ActiveGestureV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::TravelIntent", deny_unknown_fields)]
pub(crate) struct TravelIntentV1 {
    #[serde(with = "command::option")]
    receipt: Option<crate::receipts::CommandId>,
    #[serde(with = "IntentTargetV1")]
    target: IntentTarget,

    budget_seconds: f64,
    #[serde(deserialize_with = "required_option")]
    deadline: Option<f64>,
}
remote_adapters!(travel, TravelIntent, TravelIntentV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::character::RoundEdit", deny_unknown_fields)]
pub(crate) struct RoundEditV1 {
    #[serde(with = "command::option")]
    receipt: Option<crate::receipts::CommandId>,
    #[serde(deserialize_with = "required_option")]
    presence_epoch: Option<u64>,

    teach_place_on_commit: bool,

    leg: usize,

    place_id: PlaceId,
}
remote_adapters!(round_edit, RoundEdit, RoundEditV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::lore::LoreProfile", deny_unknown_fields)]
pub(crate) struct LoreProfileV1 {
    significance: Significance,

    planning_ward: PlanningWard,

    age: u16,
    #[serde(with = "TextV1")]
    gender: String,
    #[serde(with = "text::option")]
    occupation_id: Option<String>,
    #[serde(with = "text::option")]
    occupation_display: Option<String>,
    #[serde(with = "text::option")]
    title: Option<String>,
    #[serde(with = "text::option")]
    rank: Option<String>,
    #[serde(with = "text::option")]
    faction_role: Option<String>,
    #[serde(with = "text::option")]
    illegal_activity: Option<String>,
    #[serde(with = "TextV1")]
    district: String,
    #[serde(deserialize_with = "required_option")]
    father: Option<ActorId>,
    #[serde(deserialize_with = "required_option")]
    mother: Option<ActorId>,

    children: Vec<ActorId>,
    #[serde(with = "text::vec")]
    circumstances: Vec<String>,
    #[serde(with = "text::vec")]
    conditions: Vec<String>,
    #[serde(with = "text::option")]
    home: Option<String>,
    #[serde(deserialize_with = "required_option")]
    home_point_m: Option<[f64; 2]>,
    #[serde(with = "TextV1")]
    core_character_description: String,
    #[serde(with = "TextV1")]
    extended_character_description: String,
    #[serde(deserialize_with = "required_option")]
    curiosity: Option<f64>,

    generated: bool,
    #[serde(with = "routine::option")]
    generated_routine: Option<crate::crowd::GeneratedRoutine>,
}
remote_adapters!(lore, LoreProfile, LoreProfileV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::appearance::AppearanceSnapshot", deny_unknown_fields)]
pub(crate) struct AppearanceSnapshotV1 {
    build: Build,

    outfit: OutfitClass,

    headgear: Headgear,

    palette_seed: u32,
    #[serde(with = "text::option")]
    bespoke: Option<String>,
}
remote_adapters!(appearance, AppearanceSnapshot, AppearanceSnapshotV1);

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "crate::round::residents::ResidentStatus",
    deny_unknown_fields
)]
pub(crate) struct ResidentStatusV1 {
    phase: ResidentPhase,
    #[serde(with = "TextV1")]
    patch: String,
    #[serde(with = "TextV1")]
    patch_description: String,
    #[serde(with = "text::option")]
    spot: Option<String>,
    #[serde(with = "text::option")]
    destination_spot: Option<String>,

    dwell_remaining_seconds: f64,

    resting_at_household_frontage: bool,

    resting_without_home: bool,

    optional_walk: bool,

    movement_cause: motion::MotionCause,

    sheltered: bool,
}
remote_adapters!(resident, ResidentStatus, ResidentStatusV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::inventory::StockSpec", deny_unknown_fields)]
pub(crate) struct StockSpecV1 {
    kind: ItemKind,
    #[serde(with = "unique_map")]
    metadata: BTreeMap<String, String>,

    quantity: u32,
}
remote_adapters!(stock, StockSpec, StockSpecV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::inventory::LegacyRestockShare", deny_unknown_fields)]
pub(crate) struct LegacyRestockShareV1 {
    original_vendor: ActorId,
    #[serde(with = "TextV1")]
    source_id: String,

    quantity: u32,
}
remote_adapters!(share, LegacyRestockShare, LegacyRestockShareV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::inventory::ReservedInput", deny_unknown_fields)]
pub(crate) struct ReservedInputV1 {
    item_id: ItemId,

    quantity: u32,
}
remote_adapters!(input, ReservedInput, ReservedInputV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::inventory::TransformJob", deny_unknown_fields)]
pub(crate) struct TransformJobV1 {
    #[serde(with = "TextV1")]
    job_id: String,
    #[serde(with = "TextV1")]
    spec_id: String,

    producer: ActorId,

    production_day: i64,

    start_slot: u32,
    #[serde(with = "input::vec")]
    inputs: Vec<ReservedInput>,
    #[serde(with = "stock::vec")]
    outputs: Vec<StockSpec>,

    progress_work_minutes: f64,
}
remote_adapters!(job, TransformJob, TransformJobV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::inventory::TransformReceiptLine", deny_unknown_fields)]
pub(crate) struct TransformReceiptLineV1 {
    item_id: ItemId,

    quantity: u32,
}
remote_adapters!(line, TransformReceiptLine, TransformReceiptLineV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::inventory::TransformReceipt", deny_unknown_fields)]
pub(crate) struct TransformReceiptV1 {
    #[serde(with = "TextV1")]
    job_id: String,

    producer: ActorId,
    #[serde(with = "line::vec")]
    consumed: Vec<TransformReceiptLine>,
    #[serde(with = "line::vec")]
    produced: Vec<TransformReceiptLine>,

    completed_on_day: i64,
}
remote_adapters!(receipt, TransformReceipt, TransformReceiptV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::inventory::CompletedTransform", deny_unknown_fields)]
pub(crate) struct CompletedTransformV1 {
    #[serde(with = "TransformReceiptV1")]
    receipt: TransformReceipt,

    completed_on_day: i64,
}
remote_adapters!(completed, CompletedTransform, CompletedTransformV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::item::Item", deny_unknown_fields)]
pub(crate) struct ItemV1 {
    id: ItemId,

    kind: ItemKind,

    quantity: u32,
    #[serde(with = "unique_map")]
    metadata: BTreeMap<String, String>,
}
remote_adapters!(item, Item, ItemV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::world::NeedleClaim", deny_unknown_fields)]
pub(crate) struct NeedleClaimV1 {
    holder: ActorId,
    #[serde(with = "crate::math::vec3_serde")]
    dir: Vec3,
}
remote_adapters!(needle, NeedleClaim, NeedleClaimV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::places::PlaceEntry", deny_unknown_fields)]
pub(crate) struct PlaceEntryV1 {
    id: PlaceId,
    #[serde(with = "TextV1")]
    name: String,
    #[serde(with = "crate::math::vec3_serde")]
    point: Vec3,
    #[serde(with = "text::option")]
    ward: Option<String>,

    coarse: bool,
}
remote_adapters!(place, PlaceEntry, PlaceEntryV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "IntentTarget", deny_unknown_fields)]
pub(crate) enum IntentTargetV1 {
    Place {
        place_id: PlaceId,
        #[serde(with = "TextV1")]
        name: String,
        #[serde(with = "crate::math::vec3_serde")]
        point: Vec3,
    },
    Person {
        actor_id: ActorId,
        #[serde(with = "crate::math::vec3_serde")]
        last_seen: Vec3,
        visible: bool,
    },
}
remote_adapters!(target, IntentTarget, IntentTargetV1);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "GeneratedRoutine",
    tag = "kind",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum GeneratedRoutineV1 {
    Resident {
        #[serde(with = "TextV1")]
        patch: String,
        #[serde(with = "TextV1")]
        spot: String,
    },
    Worker {
        #[serde(with = "TextV1")]
        occupation: String,
        #[serde(with = "TextV1")]
        workplace: String,
    },
}
remote_adapters!(routine, GeneratedRoutine, GeneratedRoutineV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "OperationId", deny_unknown_fields)]
pub(crate) struct OperationIdV1 {
    producer: u8,
    sequence: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "CommandId", deny_unknown_fields)]
pub(crate) struct CommandIdV1 {
    #[serde(with = "OperationIdV1")]
    operation: OperationId,
    step: u16,
}
remote_adapters!(command, CommandId, CommandIdV1);
// Vec3's existing adapter already requires precisely three finite coordinates.
struct PointV1;
impl PointV1 {
    fn serialize<S: serde::Serializer>(v: &Vec3, s: S) -> Result<S::Ok, S::Error> {
        crate::math::vec3_serde::serialize(v, s)
    }
    fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec3, D::Error> {
        crate::math::vec3_serde::deserialize(d)
    }
}
remote_adapters!(point, Vec3, PointV1);

struct TextV1;
impl TextV1 {
    fn serialize<S: serde::Serializer>(v: &str, s: S) -> Result<S::Ok, S::Error> {
        if v.len() > MAX_TEXT_BYTES {
            return Err(serde::ser::Error::custom("text exceeds v1 byte limit"));
        }
        v.serialize(s)
    }
    fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        let text = crate::checkpoint::BoundedText::<MAX_TEXT_BYTES>::deserialize(d)?;
        Ok(text.0)
    }
}
pub(crate) const MAX_TEXT_BYTES: usize = 65_536;
remote_adapters!(text, String, TextV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::offer::Offer", deny_unknown_fields)]
pub(crate) struct OfferV1 {
    item_id: ItemId,
    giver_id: ActorId,
    #[serde(deserialize_with = "required_option")]
    target_id: Option<ActorId>,
    created_seq: i64,
    quantity: u32,
}
remote_adapters!(offer, crate::offer::Offer, OfferV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::clock::WorldTime", deny_unknown_fields)]
pub(crate) struct WorldTimeV1 {
    day: i64,
    fraction: f64,
    office: crate::clock::Office,
    weekday: crate::clock::Weekday,
}
remote_adapters!(world_time, crate::clock::WorldTime, WorldTimeV1);
