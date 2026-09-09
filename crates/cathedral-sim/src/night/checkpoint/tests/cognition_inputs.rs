use super::*;
use crate::traits::AcceptedOutputBudget::{Accepted, MissingLegacy};
#[derive(Default)]
struct Budgets {
    busy: bool,
    calls: Vec<(String, Option<u32>)>,
}
impl Cognition for Budgets {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, CognitionBusy> {
        panic!("wrong method")
    }
    fn request_night(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> std::result::Result<RequestId, CognitionBusy> {
        self.calls.push((prompt, budget));
        if self.busy {
            Err(CognitionBusy)
        } else {
            Ok(RequestId(self.calls.len() as u64))
        }
    }
}
#[test]
fn checkpoint_cognition_inputs_night_busy_person_and_ward_keep_only_accepted_argument() {
    for subject in [
        Subject::Person(ActorId::from_raw("mjr01")),
        Subject::Ward(PlanningWard::Weigh),
    ] {
        let (mut n, mut w, c, _) = staged(subject.clone());
        let mut s = Budgets {
            busy: true,
            ..Default::default()
        };
        n.poll(1.0, &mut w, &c, &mut vec![], open(), &mut s, &env());
        assert!(cognition_input(&n).is_none());
        let semantic = n.queue[0].semantic.unwrap();
        if let Subject::Person(a) = &subject {
            w.characters
                .get_mut(a)
                .unwrap()
                .sheet
                .lore
                .as_mut()
                .unwrap()
                .significance = Significance::Minor;
        }
        s.busy = false;
        n.poll(6.0, &mut w, &c, &mut vec![], open(), &mut s, &env());
        let f = n.in_flight.as_ref().unwrap().clone();
        assert_eq!(f.semantic, semantic);
        assert_eq!(f.output_token_budget, Accepted(Some(1400)));
        assert_eq!(s.calls.last().unwrap().1, Some(1400));
        assert_eq!(f.prompt, s.calls.last().unwrap().0);
        if let Subject::Person(a) = &subject {
            w.characters.remove(a);
        }
        assert_eq!(
            n.in_flight.as_ref().unwrap().output_token_budget,
            f.output_token_budget
        );
        let raw = bytes(&n, &w, &c, 6.0);
        let copied = copy(&n);
        let decoded = NightOfficeDtoV1::decode(
            &raw,
            CheckpointBudget::default()
                .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                .unwrap(),
            NightCheckpointContext::from_world(&w, time(6.0), &c),
        )
        .unwrap();
        for legacy in [&copied, &decoded.value().night] {
            assert_eq!(
                legacy.in_flight.as_ref().unwrap().output_token_budget,
                MissingLegacy
            );
            assert_eq!(bytes(legacy, &w, &c, 6.0), raw);
        }
        let mut bad: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        bad["night"]["in_flight"]["output_token_budget"] = json!(1400);
        let bad = serde_json::to_vec(&bad).unwrap();
        assert!(
            NightOfficeDtoV1::decode(
                &bad,
                CheckpointBudget::default()
                    .reserve(Cohort::LoadCandidate, bad.len() + 4096)
                    .unwrap(),
                NightCheckpointContext::from_world(&w, time(6.0), &c)
            )
            .is_err()
        );
        n.held_result = Some(Completion {
            request_id: f.request_id,
            result: Err(CognitionError::detailed("NightKind", "NightDetail")),
            duration_seconds: 0.25,
        });
        n.config.enabled = false;
        n.poll(6.1, &mut w, &c, &mut vec![], open(), &mut s, &env());
        assert_eq!(
            n.in_flight.as_ref().unwrap().output_token_budget,
            f.output_token_budget
        );
        assert!(n.held_result.is_some());
        n.config.enabled = true;
        n.poll(6.2, &mut w, &c, &mut vec![], open(), &mut s, &env());
        assert!(n.in_flight.is_none() && n.held_result.is_none());
        assert!(cognition_input(&n).is_none());
    }
}
#[test]
fn checkpoint_cognition_inputs_night_held_success_retains_budget_and_prompt_until_application() {
    let (mut n, mut w, c, mut service) = staged(Subject::Ward(PlanningWard::Weigh));
    n.poll(1.0, &mut w, &c, &mut vec![], open(), &mut service, &env());
    let f = n.in_flight.as_ref().unwrap().clone();
    let protected = saturate(&mut w, 1.0);
    let done = Completion {
        request_id: f.request_id,
        result: Ok("ward_mood {\"mood\":\"Exact accepted input\"}".into()),
        duration_seconds: 0.25,
    };
    poll(&mut n, &mut w, &c, 1.1, &mut service, Some(done.clone()));
    assert_eq!(n.held_result, Some(done));
    assert_eq!(
        n.in_flight.as_ref().unwrap().output_token_budget,
        Accepted(Some(1400))
    );
    assert_eq!(n.in_flight.as_ref().unwrap().prompt, f.prompt);
    w.command_ledger.unprotect(protected);
    poll(&mut n, &mut w, &c, 1.2, &mut service, None);
    assert!(cognition_input(&n).is_none());
    assert_eq!(w.ward_moods[&PlanningWard::Weigh], "Exact accepted input");
    // The new enum is retained in old V1 owner DTOs too. Their unchanged
    // object/key charges cover the grown inline owners, before values/strings.
    assert!(std::mem::size_of::<Flight>() < 512 + 6 * 64);
    assert!(std::mem::size_of::<NightOffice>() < 512 + 13 * 64);
    assert!(std::mem::size_of::<NightOfficeDtoV1>() < 2 * 512 + 17 * 64);
    println!(
        "cognition_inputs_night_layout flight={} owner={} accepted_budget={}",
        std::mem::size_of::<Flight>(),
        std::mem::size_of::<NightOffice>(),
        std::mem::size_of::<crate::traits::AcceptedOutputBudget>()
    );
}
