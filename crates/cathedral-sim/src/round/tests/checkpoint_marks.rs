//! The existing Round writer/reader continues from the restored marks owner.
use super::*;
use crate::{
    checkpoint::{CheckpointBudget, Cohort},
    marks::{
        self, MarkAnchor, MarkKind,
        checkpoint::{MarksCheckpointContext, WorldMarksDtoV1},
    },
    timeline::LogicalTime,
};
fn pair() -> (World, Round) {
    let mut world = World::new();
    let mut round = Round::new();
    for (i, (name, x)) in [("Chain Well", 10.0), ("Ford Well", 25.0)]
        .into_iter()
        .enumerate()
    {
        register_place(
            &mut world,
            &format!("pl_w{i}"),
            name,
            Vec3::new(x, WALK_Y, 0.0),
        );
        round.sources.push(WaterSource {
            name: name.into(),
            draw_point: Vec3::new(x, WALK_Y, 0.0),
            draw_sound: "draw_water",
            keeper: Some(ActorId::from_raw(format!("keeper{i}"))),
            queue: Vec::new(),
            serving: None,
            keeper_next_sound: 0.0,
        });
    }
    (world, round)
}
#[test]
fn checkpoint_zero_and_saturated_tally_continue_the_ordinary_writer_and_reader() {
    for start in [0, 12] {
        let (mut control, round) = pair();
        let (mut resumed, _) = pair();
        let anchor = MarkAnchor::Place("Chain Well".into());
        let id =
            marks::draw_or_refresh(&mut control, MarkKind::WellTally, anchor.clone(), None, 0.0)
                .unwrap()
                .id;
        control.marks.get_mut(id).unwrap().strokes = start;
        let budget = CheckpointBudget::default();
        let now = LogicalTime::new(0.0).unwrap();
        let bytes = control
            .export_marks_checkpoint(now, budget.reserve(Cohort::SavePayload, 4096).unwrap())
            .unwrap()
            .encode()
            .unwrap();
        let c = MarksCheckpointContext::from_world(&resumed, now);
        let candidate = WorldMarksDtoV1::decode(
            bytes.value(),
            budget
                .reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
                .unwrap(),
            c,
        )
        .unwrap()
        .into_candidate(c)
        .unwrap();
        resumed.marks = candidate.value().marks().clone();
        resumed.world_revision = control.world_revision;
        let here = Vec3::new(0.0, WALK_Y, 0.0);
        assert_eq!(
            tallied_source(&round, &resumed, here),
            Some(if start == 0 { 0 } else { 1 })
        );
        for day in [0.1, 0.2, 0.3] {
            notch_the_tally(&round, &mut control, 0, day);
            notch_the_tally(&round, &mut resumed, 0, day);
            assert_eq!(control.marks, resumed.marks);
            assert_eq!(control.world_revision, resumed.world_revision);
        }
        assert_eq!(
            resumed.marks.get(id).unwrap().strokes,
            if start == 0 { 3 } else { 12 }
        );
        assert_eq!(tallied_source(&round, &resumed, here), Some(1));
        marks::scrub(&mut resumed, id);
        assert_eq!(tallied_source(&round, &resumed, here), Some(0));
    }
}
