use super::*;
use crate::checkpoint::{CheckpointBudget, Cohort};
#[test]
fn round_preflight_rejects_malicious_shape_before_record_decoding() {
    let world = World::new();
    let context = RoundCheckpointContext::from_world(&world, None);
    for (bytes, reason) in [
        (
            format!("{{\"unknown\":{}0{}}}", "[".repeat(70), "]".repeat(70)).into_bytes(),
            "nesting",
        ),
        (
            format!(
                "{{\"unknown\":[{}]}}",
                "[],".repeat(270_000).trim_end_matches(',')
            )
            .into_bytes(),
            "aggregate expanded",
        ),
    ] {
        let b = CheckpointBudget::default();
        let error = RoundDtoV1::decode(
            &bytes,
            b.reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            context,
        )
        .unwrap_err();
        assert!(error.reason.contains(reason), "{error}");
        assert_eq!(b.retained_bytes(), 0);
    }
}
#[test]
fn round_declared_scratch_and_closed_record_layouts_fit_the_admission_model() {
    let mut expanded = 0;
    let mut encoded = 0;
    for input in [FOOD_JSON, ROUNDS_JSON, HOMES_JSON] {
        let c = aggregate::inspect(input.as_bytes()).unwrap();
        expanded += c.expanded_upper_bytes;
        encoded += c.encoded_bytes;
    }
    assert_eq!((expanded, encoded), (1_730_786, 179_470));
    assert!(2 * expanded + 3 * encoded + 128 * 1024 <= DEFINITION_WORKING_BYTES);
    macro_rules! size {
        ($t:ty,$max:expr) => {{
            let n = std::mem::size_of::<$t>();
            println!("{}={n}", stringify!($t));
            assert!(n <= $max);
        }};
    }
    size!(Round, 2048);
    size!(Townsperson, 512);
    size!(RoadParty, 512);
    size!(Counter, 256);
    size!(ResolvedProductionPlan, 64);
    size!(ResolvedTransformSpec, 192);
    size!(ResolvedTrade, 128);
    size!(RoundLeg, 128);
    size!(WaterSource, 192);
    size!(FoodStall, 256);
    size!(MarketErrand, 256);
    size!(CounterBindingKey, 96);
    size!(CounterSession, 48);
    size!(StockSource, 32);
    size!(StockPlanSpec, 128);
    size!(StockTargetSpec, 64);
    size!(FoodPhase, 48);
    size!(WeatherShelterIntent, 80);
    size!(TransformStallWatch, 48);
    crate::round::residents::checkpoint::check_layout();
    // Two aggregate copies cover sparse BTree roots/Vec capacity for the largest
    // closed record value; the other two cover validation/serde transients.
    let empty = Round::new();
    let world = World::new();
    let b = CheckpointBudget::default();
    let cost = empty
        .checkpoint_round_cost(
            RoundCheckpointContext::from_world(&world, None),
            b.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
    println!("empty_cost={:?}", cost.value());
    assert!(cost.value().expanded_upper_bytes >= 8 * std::mem::size_of::<Round>());
}
