//! Shared six-phase measurement of real accepted inputs. Population and
//! ordinary request setup remain in the established caller examples.
use cathedral_sim::{
    Engine,
    checkpoint::{CheckpointBudget, Cohort},
    engine::cognition_inputs_checkpoint::EngineCognitionInputsDtoV1,
    timeline::LogicalTime,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Instant;
#[derive(Default, Serialize)]
struct Phases {
    preflight_us: Vec<f64>,
    export_us: Vec<f64>,
    encode_us: Vec<f64>,
    decode_validate_us: Vec<f64>,
    candidate_validate_us: Vec<f64>,
    drop_us: Vec<f64>,
}
fn time<T>(out: &mut Vec<f64>, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = f();
    out.push(start.elapsed().as_secs_f64() * 1e6);
    value
}
pub fn measure(
    engine: &Engine,
    boundary: LogicalTime,
    samples: usize,
    lane: &str,
    owner_counts: Value,
    witnesses: Value,
    submitted_requests: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut phases = Phases::default();
    let mut metadata = None;
    let c = engine.cognition_inputs_checkpoint_context(boundary);
    for _ in 0..samples {
        let b = CheckpointBudget::default();
        let initial = || b.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_cognition_inputs_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_cognition_inputs_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts();
        assert!(if lane == "scheduler" {
            counts.scheduler && !counts.night
        } else {
            !counts.scheduler && counts.night
        });
        let saved_inputs = json!({"scheduler":dto.value().scheduler(),"night":dto.value().night()});
        let saved = &saved_inputs[lane];
        let original = submitted_requests
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["request_id"] == saved["request_id"])
            .expect("saved input has a real accepted request");
        for key in ["method", "prompt", "output_token_budget", "request_id"] {
            assert_eq!(saved[key], original[key], "submitted {key} changed");
        }
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineCognitionInputsDtoV1::decode(
                bytes.value(),
                b.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
                    .unwrap(),
                c,
            )
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(c)
        })?;
        assert_eq!(candidate.value().counts(), counts);
        assert_eq!(
            json!({"scheduler":candidate.value().scheduler(),"night":candidate.value().night()}),
            saved_inputs
        );
        let current = json!({"scenario":format!("cognition-inputs-{lane}-v1"),"counts":counts,"owner_counts":owner_counts,"witnesses":witnesses,"submitted_requests":submitted_requests,"saved_inputs":saved_inputs,"cost":cost,"shared_reserved_peak_excluding_running_bytes":b.retained_bytes()});
        if let Some(prior) = &metadata {
            assert_eq!(prior, &current);
        } else {
            metadata = Some(current);
        }
        time(&mut phases.drop_us, || {
            drop(candidate);
            drop(bytes);
        });
        assert_eq!(b.retained_bytes(), 0);
    }
    let mut output = metadata.unwrap();
    for (key, value) in serde_json::to_value(phases)?.as_object().unwrap() {
        output[key] = value.clone();
    }
    Ok(output)
}
