use super::*;
use crate::traits::AcceptedOutputBudget::{Accepted, MissingLegacy};

#[test]
fn checkpoint_cognition_inputs_scheduler_all_lanes_hold_errors_stale_cleanup_and_legacy() {
    for lane in [TurnLane::PlayerReaction, TurnLane::Handoff, TurnLane::Idle] {
        for failed in [false, true] {
            let (mut w, mut n, mut s) = flight(lane);
            n.close();
            let f = n.in_flight.as_ref().unwrap().clone();
            assert_eq!(f.output_token_budget, Accepted(Some(2400)));
            w.characters.get_mut(&f.actor_id).unwrap().sheet.lore = None;
            w.characters
                .get_mut(&f.actor_id)
                .unwrap()
                .state
                .presence_epoch += 1;
            n.actors_departed(std::slice::from_ref(&f.actor_id));
            assert_eq!(
                n.in_flight.as_ref().unwrap().output_token_budget,
                f.output_token_budget
            );
            let done = Completion {
                request_id: f.request_id,
                result: if failed {
                    Err(crate::CognitionError::detailed("Kind", "Detail"))
                } else {
                    Ok("wait {}".into())
                },
                duration_seconds: 0.5,
            };
            poll(
                &mut n,
                &mut w,
                0.1,
                vec![done.clone()],
                true,
                IdleGate::Suppressed,
                &mut s,
            );
            assert_eq!(n.held_result, Some(done));
            assert_eq!(
                n.in_flight.as_ref().unwrap().output_token_budget,
                f.output_token_budget
            );
            let raw = bytes(&n, &w, 0.1);
            let copied = copy(&n);
            let decoded = decode(&raw, &w, 0.1).unwrap();
            for legacy in [&copied, &decoded.value().scheduler] {
                assert_eq!(
                    legacy.in_flight.as_ref().unwrap().output_token_budget,
                    MissingLegacy
                );
                assert_eq!(bytes(legacy, &w, 0.1), raw);
            }
            let mut bad: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            bad["scheduler"]["in_flight"]["output_token_budget"] = json!(null);
            assert!(decode(&serde_json::to_vec(&bad).unwrap(), &w, 0.1).is_err());
            poll(
                &mut n,
                &mut w,
                0.2,
                vec![],
                false,
                IdleGate::Suppressed,
                &mut s,
            );
            assert!(n.in_flight.is_none() && n.held_result.is_none());
            assert!(cognition_input(&n).is_none());
        }
    }
}
#[derive(Default)]
struct Budgets {
    busy: bool,
    calls: Vec<(String, Option<u32>)>,
}
impl Cognition for Budgets {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, CognitionBusy> {
        panic!("wrong method")
    }
    fn request_with_budget(
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
fn checkpoint_cognition_inputs_scheduler_busy_then_failure_retry_resolves_new_argument() {
    let mut w = world();
    let a = actor("mjr01");
    let mut n = NpcScheduler::new(vec![a.clone()], 0.0, 60.0, 0.0);
    n.start(0.0);
    n.prioritize_player_reaction(&w, &a, 0.0);
    let mut s = Budgets {
        busy: true,
        ..Default::default()
    };
    let run =
        |n: &mut NpcScheduler, w: &mut World, s: &mut Budgets, t, mut done: Vec<Completion>| {
            n.poll(
                t,
                w,
                &mut vec![],
                &mut done,
                false,
                IdleGate::All,
                s,
                &env(),
            )
        };
    run(&mut n, &mut w, &mut s, 0.0, vec![]);
    assert!(cognition_input(&n).is_none());
    let semantic = n.retry_work[&a].semantic;
    assert_eq!(s.calls[0].1, Some(2400));
    w.characters.get_mut(&a).unwrap().sheet.lore = None;
    s.busy = false;
    run(&mut n, &mut w, &mut s, 1.0, vec![]);
    let f = n.in_flight.as_ref().unwrap().clone();
    assert_eq!(f.semantic, semantic);
    assert_eq!(f.output_token_budget, Accepted(None));
    assert_eq!(f.prompt, s.calls[1].0);
    run(
        &mut n,
        &mut w,
        &mut s,
        1.1,
        vec![Completion {
            request_id: f.request_id,
            result: Err(crate::CognitionError::new("retry")),
            duration_seconds: 0.1,
        }],
    );
    assert!(cognition_input(&n).is_none());
    w.characters.get_mut(&a).unwrap().sheet.lore =
        world().characters[&actor("mnr01")].sheet.lore.clone();
    run(&mut n, &mut w, &mut s, 10.0, vec![]);
    let f = n.in_flight.as_ref().unwrap();
    assert_eq!(f.semantic, semantic);
    assert_eq!(f.output_token_budget, Accepted(Some(1400)));
    assert_eq!(f.prompt, s.calls.last().unwrap().0);
    assert_eq!(s.calls.last().unwrap().1, Some(1400));
}
