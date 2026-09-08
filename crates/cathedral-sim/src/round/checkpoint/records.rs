//! Explicit strict v1 records, separate from authoring defaults.
#![allow(dead_code, private_interfaces)] // Internal remote adapters retain runtime owner privacy.
use super::*;
use crate::checkpoint::{
    records as common,
    serde_support::{remote_adapters, required_option, unique_map, unique_set},
};
use serde::{Deserialize, Serialize};

pub(crate) struct WaterSourceV1;
impl WaterSourceV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &WaterSource,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            name: &'a str,
            #[serde(with = "crate::math::vec3_serde")]
            draw_point: Vec3,
            draw_sound: &'a str,
            keeper: &'a Option<ActorId>,
            queue: &'a [ActorId],
            serving: &'a Option<(ActorId, f64)>,
            keeper_next_sound: f64,
        }
        Ref {
            name: &v.name,
            draw_point: v.draw_point,
            draw_sound: v.draw_sound,
            keeper: &v.keeper,
            queue: &v.queue,
            serving: &v.serving,
            keeper_next_sound: v.keeper_next_sound,
        }
        .serialize(s)
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<WaterSource, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Data {
            #[serde(with = "common::TextV1")]
            name: String,
            #[serde(with = "crate::math::vec3_serde")]
            draw_point: Vec3,
            #[serde(with = "common::TextV1")]
            draw_sound: String,
            #[serde(deserialize_with = "required_option")]
            keeper: Option<ActorId>,
            queue: Vec<ActorId>,
            #[serde(deserialize_with = "required_option")]
            serving: Option<(ActorId, f64)>,
            keeper_next_sound: f64,
        }
        let v = Data::deserialize(d)?;
        let draw_sound = SOURCES
            .iter()
            .find_map(|(_, sound)| (*sound == v.draw_sound).then_some(*sound))
            .ok_or_else(|| serde::de::Error::custom("unknown water source sound"))?;
        Ok(WaterSource {
            name: v.name,
            draw_point: v.draw_point,
            draw_sound,
            keeper: v.keeper,
            queue: v.queue,
            serving: v.serving,
            keeper_next_sound: v.keeper_next_sound,
        })
    }
}
remote_adapters!(water_source, WaterSource, WaterSourceV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "FoodStall", deny_unknown_fields)]
pub(crate) struct FoodStallV1 {
    #[serde(with = "common::TextV1")]
    name: String,
    #[serde(with = "common::TextV1")]
    site: String,
    #[serde(with = "crate::math::vec3_serde")]
    pitch: Vec3,
    #[serde(with = "common::TextV1")]
    trade: String,
    #[serde(deserialize_with = "required_option")]
    vendor: Option<ActorId>,
    queue: Vec<ActorId>,
    #[serde(deserialize_with = "required_option")]
    serving: Option<(ActorId, f64)>,
    #[serde(deserialize_with = "required_option")]
    preferred: Option<ActorId>,
    #[serde(with = "crate::round::checkpoint::records::OpenSpecV1")]
    open: OpenSpec,
    cry_next: f64,
}
remote_adapters!(food_stall, FoodStall, FoodStallV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "RoadParty", deny_unknown_fields)]
pub(crate) struct RoadPartyV1 {
    id: PartyId,
    leader: ActorId,
    members: Vec<ActorId>,
    #[serde(with = "common::TextV1")]
    gate: String,
    #[serde(with = "crate::math::vec3_serde")]
    gate_point: Vec3,
    only_on: Vec<Weekday>,
    stage_at: Office,
    enter_at: Office,
    return_at: Office,
    #[serde(with = "unique_map")]
    wallet_floats: BTreeMap<ActorId, u32>,
    #[serde(with = "crate::round::checkpoint::records::matcher::vec")]
    commercial_cargo: Vec<ItemMatcher>,
    #[serde(with = "crate::checkpoint::records::stock::vec")]
    manifest: Vec<StockSpec>,
    #[serde(with = "crate::round::checkpoint::records::round_leg::vec")]
    legs: Vec<RoundLeg>,
    #[serde(with = "crate::round::checkpoint::records::PartyStateV1")]
    state: PartyState,
    #[serde(deserialize_with = "required_option")]
    last_trigger_day: Option<i64>,
    #[serde(with = "unique_map")]
    departure_excuses: BTreeMap<ActorId, f64>,
}
remote_adapters!(road_party, RoadParty, RoadPartyV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Counter", deny_unknown_fields)]
pub(crate) struct CounterV1 {
    #[serde(with = "common::TextV1")]
    id: String,
    #[serde(with = "common::TextV1")]
    trade: String,
    #[serde(with = "common::TextV1")]
    site: String,
    #[serde(with = "crate::math::vec3_serde")]
    pitch: Vec3,
    seller: ActorId,
    offices: Vec<Office>,
    #[serde(with = "crate::round::checkpoint::records::ArrivalV1")]
    required_doing: Arrival,
    #[serde(deserialize_with = "required_option")]
    road_party: Option<PartyId>,
    worksite_only: bool,
    radius_m: f64,
}
remote_adapters!(counter, Counter, CounterV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "CounterBindingKey", deny_unknown_fields)]
pub(crate) struct CounterBindingKeyV1 {
    #[serde(with = "common::TextV1")]
    counter_id: String,
    seller: ActorId,
    #[serde(with = "crate::round::checkpoint::records::CounterSessionV1")]
    session: CounterSession,
}
remote_adapters!(counter_binding_key, CounterBindingKey, CounterBindingKeyV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "MarketErrand", deny_unknown_fields)]
pub(crate) struct MarketErrandV1 {
    #[serde(with = "common::TextV1")]
    plan_id: String,
    #[serde(with = "crate::round::checkpoint::records::counter_binding_key::option")]
    selected: Option<CounterBindingKey>,
    #[serde(with = "crate::round::checkpoint::records::counter_binding_key::vec")]
    bindings_seen: Vec<CounterBindingKey>,
    #[serde(with = "crate::round::checkpoint::records::MarketErrandPhaseV1")]
    phase: MarketErrandPhase,
    spent_sparks: u32,
    #[serde(with = "common::text::option")]
    last_failed_fingerprint: Option<String>,
    #[serde(deserialize_with = "required_option")]
    travel_deadline_real: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    deadline_hold_began_real: Option<f64>,
}
remote_adapters!(market_errand, MarketErrand, MarketErrandV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "ClosedMarketVisit", deny_unknown_fields)]
pub(crate) struct ClosedMarketVisitV1 {
    #[serde(with = "common::TextV1")]
    plan_id: String,
    #[serde(with = "crate::round::checkpoint::records::counter_binding_key::vec")]
    bindings_seen: Vec<CounterBindingKey>,
    #[serde(with = "crate::round::checkpoint::records::MarketVisitEndV1")]
    end_reason: MarketVisitEnd,
}
remote_adapters!(closed_market_visit, ClosedMarketVisit, ClosedMarketVisitV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "ResolvedProductionPlan", deny_unknown_fields)]
pub(crate) struct ResolvedProductionPlanV1 {
    producer: ActorId,
    max_jobs_per_day: u32,
    #[serde(with = "crate::round::checkpoint::records::resolved_transform_spec::vec")]
    transforms: Vec<ResolvedTransformSpec>,
}
remote_adapters!(
    resolved_production_plan,
    ResolvedProductionPlan,
    ResolvedProductionPlanV1
);

#[derive(Serialize, Deserialize)]
#[serde(remote = "ResolvedTransformSpec", deny_unknown_fields)]
pub(crate) struct ResolvedTransformSpecV1 {
    #[serde(with = "common::TextV1")]
    id: String,
    #[serde(with = "common::TextV1")]
    site: String,
    #[serde(with = "crate::math::vec3_serde")]
    point: Vec3,
    #[serde(with = "crate::checkpoint::records::stock::vec")]
    consumes: Vec<StockSpec>,
    #[serde(with = "crate::checkpoint::records::stock::vec")]
    produces: Vec<StockSpec>,
    allowed_offices: Vec<Office>,
    work_minutes: u32,
    desired_output_quantity: u32,
}
remote_adapters!(
    resolved_transform_spec,
    ResolvedTransformSpec,
    ResolvedTransformSpecV1
);

#[derive(Serialize, Deserialize)]
#[serde(remote = "ResolvedTrade", deny_unknown_fields)]
pub(crate) struct ResolvedTradeV1 {
    #[serde(with = "common::text::vec")]
    occupations: Vec<String>,
    #[serde(with = "crate::round::checkpoint::records::matcher::vec")]
    listings: Vec<ItemMatcher>,
    #[serde(with = "crate::checkpoint::records::stock::vec")]
    restock: Vec<StockSpec>,
    #[serde(deserialize_with = "required_option")]
    per_serving: Option<crate::item::ItemKind>,
}
remote_adapters!(resolved_trade, ResolvedTrade, ResolvedTradeV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "FoodErrand", deny_unknown_fields)]
pub(crate) struct FoodErrandV1 {
    stall: usize,
    #[serde(with = "crate::round::checkpoint::records::FoodPhaseV1")]
    phase: FoodPhase,
}
remote_adapters!(food_errand, FoodErrand, FoodErrandV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Lamp", deny_unknown_fields)]
pub(crate) struct LampV1 {
    #[serde(with = "common::TextV1")]
    square: String,
    #[serde(with = "crate::math::vec3_serde")]
    position: Vec3,
    lit: bool,
    #[serde(deserialize_with = "required_option")]
    keeper: Option<ActorId>,
}
remote_adapters!(lamp, Lamp, LampV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "RoundLeg", deny_unknown_fields)]
pub(crate) struct RoundLegV1 {
    from: Office,
    #[serde(with = "crate::math::vec3_serde")]
    at: Vec3,
    #[serde(with = "common::TextV1")]
    label: String,
    #[serde(with = "crate::round::checkpoint::records::ArrivalV1")]
    doing: Arrival,
    #[serde(deserialize_with = "required_option")]
    only_on: Option<Vec<Weekday>>,
    is_home: bool,
}
remote_adapters!(round_leg, RoundLeg, RoundLegV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Townsperson", deny_unknown_fields)]
pub(crate) struct TownspersonV1 {
    #[serde(with = "crate::checkpoint::records::point::option")]
    home: Option<Vec3>,
    #[serde(with = "crate::math::vec3_serde")]
    base: Vec3,
    #[serde(with = "crate::round::checkpoint::records::round_leg::vec")]
    legs: Vec<RoundLeg>,
    leash_m: f64,
    curfew_exempt: bool,
    #[serde(deserialize_with = "required_option")]
    source: Option<usize>,
    is_household: bool,
    #[serde(with = "crate::round::checkpoint::records::food_errand::option")]
    food: Option<FoodErrand>,
    #[serde(with = "crate::round::checkpoint::records::PhaseV1")]
    phase: Phase,
    #[serde(with = "crate::checkpoint::records::point::option")]
    travel_target: Option<Vec3>,
    travel_for_intent: bool,
    motion_cause: motion::MotionCause,
    next_decision: f64,
    epoch: u64,
    #[serde(with = "super::evening")]
    evening_seed: Option<(usize, RoundLeg)>,
    leg_lag_share: f64,
    excused: bool,
}
remote_adapters!(townsperson, Townsperson, TownspersonV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "WeatherShelterIntent", deny_unknown_fields)]
pub(crate) struct WeatherShelterIntentV1 {
    shelter: usize,
    #[serde(with = "crate::math::vec3_serde")]
    target: Vec3,
    release_threshold: f64,
    #[serde(deserialize_with = "required_option")]
    below_since_days: Option<f64>,
    release_after_days: f64,
}
remote_adapters!(
    weather_shelter_intent,
    WeatherShelterIntent,
    WeatherShelterIntentV1
);

#[derive(Serialize, Deserialize)]
#[serde(remote = "TransformStallWatch", deny_unknown_fields)]
pub(crate) struct TransformStallWatchV1 {
    #[serde(with = "common::TextV1")]
    job_id: String,
    progress_work_minutes: f64,
    unchanged_since_game_days: f64,
}
remote_adapters!(
    transform_stall_watch,
    TransformStallWatch,
    TransformStallWatchV1
);

#[derive(Serialize, Deserialize)]
#[serde(remote = "StockPlanSpec", deny_unknown_fields)]
pub(crate) struct StockPlanSpecV1 {
    #[serde(with = "common::TextV1")]
    id: String,
    buyer: ActorId,
    #[serde(with = "crate::round::checkpoint::records::StockSourceV1")]
    source: StockSource,
    #[serde(with = "crate::round::checkpoint::records::stock_target_spec::vec")]
    targets: Vec<StockTargetSpec>,
    max_spend_sparks: u32,
}
remote_adapters!(stock_plan_spec, StockPlanSpec, StockPlanSpecV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "StockTargetSpec", deny_unknown_fields)]
pub(crate) struct StockTargetSpecV1 {
    kind: crate::item::ItemKind,
    #[serde(with = "unique_map")]
    metadata: BTreeMap<String, String>,
    desired_quantity: u32,
}
remote_adapters!(stock_target_spec, StockTargetSpec, StockTargetSpecV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "OpenSpec", deny_unknown_fields)]
pub(crate) struct OpenSpecV1 {
    offices: Vec<Office>,
    #[serde(deserialize_with = "required_option")]
    weekdays: Option<Vec<Weekday>>,
}
remote_adapters!(open_spec, OpenSpec, OpenSpecV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "PartyState", deny_unknown_fields)]
pub(crate) struct PartyStateV1 {
    phase: PartyPhase,
    trip_number: u64,
}
remote_adapters!(party_state, PartyState, PartyStateV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Round", deny_unknown_fields)]
pub(crate) struct RoundV1 {
    #[serde(with = "crate::round::checkpoint::records::water_source::vec")]
    sources: Vec<WaterSource>,
    #[serde(with = "crate::checkpoint::records::point::vec")]
    taverns: Vec<Vec3>,
    #[serde(with = "crate::round::checkpoint::records::food_stall::vec")]
    stalls: Vec<FoodStall>,
    #[serde(with = "crate::round::checkpoint::records::resolved_trade::map")]
    food_trades: BTreeMap<String, ResolvedTrade>,
    #[serde(with = "common::text::vec")]
    food_log: Vec<String>,
    #[serde(deserialize_with = "required_option")]
    last_office_days: Option<f64>,
    #[serde(with = "crate::round::checkpoint::records::townsperson::map")]
    people: BTreeMap<ActorId, Townsperson>,
    #[serde(with = "crate::round::residents::checkpoint::ResidentsV1")]
    residents: residents::Residents,
    #[serde(with = "crate::round::checkpoint::records::weather_shelter_intent::map")]
    weather_shelter_intents: BTreeMap<ActorId, WeatherShelterIntent>,
    #[serde(with = "unique_map")]
    lightning_reflex_until: BTreeMap<ActorId, f64>,
    #[serde(with = "unique_map")]
    chalk_refused_until: BTreeMap<ActorId, f64>,
    #[serde(with = "unique_map")]
    knowledge_refused_until: BTreeMap<ActorId, f64>,
    #[serde(with = "unique_set")]
    knowledge_refused_buyers: BTreeSet<ActorId>,
    #[serde(with = "super::pollen::map")]
    next_pollen: BTreeMap<ActorId, f64>,
    #[serde(with = "unique_set")]
    pollen_due: BTreeSet<(u64, ActorId)>,
    last_game_days: f64,
    seeded: bool,
    #[serde(with = "crate::round::checkpoint::records::lamp::vec")]
    lamps: Vec<Lamp>,
    lamp_revision: u64,
    #[serde(deserialize_with = "required_option")]
    lamp_night_day: Option<i64>,
    #[serde(with = "unique_map")]
    lamp_targets: BTreeMap<ActorId, usize>,
    #[serde(with = "unique_map")]
    belwyn: BTreeMap<String, usize>,
    #[serde(with = "unique_map")]
    household_reserves: BTreeMap<ActorId, u32>,
    #[serde(deserialize_with = "required_option")]
    last_household_watch_day: Option<i64>,
    #[serde(deserialize_with = "required_option")]
    last_household_settlement_day: Option<i64>,
    #[serde(with = "unique_map")]
    unrelieved_zero_streak: BTreeMap<ActorId, u32>,
    institutional_payroll_sparks: u64,
    household_redistributed_sparks: u64,
    road_cash_in_sparks: u64,
    road_cash_out_sparks: u64,
    #[serde(with = "crate::round::checkpoint::records::road_party::map")]
    road_parties: BTreeMap<PartyId, RoadParty>,
    #[serde(with = "unique_map")]
    observed_cart_loads: BTreeMap<PartyId, Vec<CartLoadKind>>,
    departed_this_tick: Vec<ActorId>,
    #[serde(with = "crate::checkpoint::records::point::map")]
    worksites: BTreeMap<String, Vec3>,
    #[serde(with = "crate::round::checkpoint::records::counter::map")]
    counters: BTreeMap<String, Counter>,
    #[serde(with = "unique_map")]
    counter_groups: BTreeMap<String, Vec<String>>,
    #[serde(with = "crate::round::checkpoint::records::stock_plan_spec::vec")]
    stock_plans: Vec<StockPlanSpec>,
    #[serde(with = "crate::round::checkpoint::records::market_errand::map")]
    market_errands: BTreeMap<ActorId, MarketErrand>,
    #[serde(with = "crate::round::checkpoint::records::closed_market_visit::map")]
    closed_market_visits: BTreeMap<String, ClosedMarketVisit>,
    #[serde(with = "crate::round::checkpoint::records::resolved_production_plan::vec")]
    production_plans: Vec<ResolvedProductionPlan>,
    #[serde(with = "super::starts")]
    production_starts: BTreeMap<(ActorId, i64), u32>,
    production_last_game_days: f64,
    #[serde(with = "unique_map")]
    production_was_eligible: BTreeMap<ActorId, bool>,
    #[serde(with = "crate::round::checkpoint::records::transform_stall_watch::map")]
    transform_stall_watch: BTreeMap<ActorId, TransformStallWatch>,
    #[serde(skip)]
    ladder_scratch: Vec<ActorId>,
}
remote_adapters!(round, Round, RoundV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Arrival", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ArrivalV1 {
    Work,
    Trade,
    Sleep,
    Pray,
    Idle,
    DrawWater,
    Stand,
}
remote_adapters!(arrival, Arrival, ArrivalV1);

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "CounterSession",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum CounterSessionV1 {
    Daily { absolute_day: i64 },
    RoadTrip { party_id: PartyId, trip_number: u64 },
}
remote_adapters!(counter_session, CounterSession, CounterSessionV1);

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MarketErrandPhase",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum MarketErrandPhaseV1 {
    Approaching,
    WaitingForOpen,
    AtCounter,
}
remote_adapters!(market_errand_phase, MarketErrandPhase, MarketErrandPhaseV1);

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MarketVisitEnd",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum MarketVisitEndV1 {
    TargetsSatisfied,
    BudgetExhausted,
    SourceIneligible,
    LastOfficePassed,
    NoRoute,
    TravelExpired,
    ReplacedByGoTo,
    Returning,
    UnpricedStock,
}
remote_adapters!(market_visit_end, MarketVisitEnd, MarketVisitEndV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "FoodPhase", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum FoodPhaseV1 {
    Approaching,
    Queued,

    Eating { item: ItemId, until: f64 },
}
remote_adapters!(food_phase, FoodPhase, FoodPhaseV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Phase", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum PhaseV1 {
    Idle,
    Approaching,
    Queued,
    Drawing,

    Returning,
    Travelling,
}
remote_adapters!(phase, Phase, PhaseV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "StockSource", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum StockSourceV1 {
    Counter(#[serde(with = "common::TextV1")] String),
    CounterGroup(#[serde(with = "common::TextV1")] String),
}
remote_adapters!(stock_source, StockSource, StockSourceV1);

#[derive(Serialize, Deserialize)]
#[serde(remote = "ItemMatcher", deny_unknown_fields)]
pub(crate) struct ItemMatcherV1 {
    kind: crate::item::ItemKind,
    #[serde(with = "unique_map")]
    metadata: BTreeMap<String, String>,
}
remote_adapters!(matcher, ItemMatcher, ItemMatcherV1);
