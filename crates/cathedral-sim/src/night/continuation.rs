//! Retry the exact saved Night duty only inside its original validity window.
use super::*;
use crate::checkpoint::{CheckpointError, Result};
#[cfg(test)]
mod tests;
impl NightOffice {
    #[cfg(test)]
    pub(crate) fn archive_test_hold_person(
        &mut self,
        actor: ActorId,
        world: &mut World,
        clock: &WorldClock,
        cognition: &mut dyn Cognition,
        env: &PromptEnv,
    ) {
        assert!(self.in_flight.is_none());
        self.enqueue(Subject::Person(actor), clock.at(0.0).day);
        self.poll(
            0.0,
            world,
            clock,
            &mut vec![],
            checkpoint::tests::open(),
            cognition,
            env,
        );
        let request_id = self.in_flight.as_ref().unwrap().request_id;
        self.held_result = Some(Completion {
            request_id,
            result: Err(crate::CognitionError::detailed(
                "saved",
                "held night archive",
            )),
            duration_seconds: 0.25,
        });
    }
    pub(crate) fn held_archive_requirements(&self, world: &World) -> Option<(usize, usize)> {
        self.held_result.as_ref()?;
        let f = self.in_flight.as_ref()?;
        Some((f.prompt.capacity(), f.subject.archive_label_bytes(world)))
    }
    pub(crate) fn bind_held_archive(
        &mut self,
        archive: Option<crate::prompt_archive::PromptArchivePermit>,
    ) {
        self.archive = archive;
    }

    pub(crate) fn has_held_completion(&self) -> bool {
        self.held_result.is_some()
    }
    pub(crate) fn load_retry_pending(&self) -> bool {
        self.load_retry_pending
    }
    pub(crate) fn has_continuation(&self) -> bool {
        self.load_retry_pending || self.queue.iter().any(|d| d.queued_presence_epoch.is_some())
    }
    pub(crate) fn queued_incarnations(&self) -> impl serde::Serialize + '_ {
        struct View<'a>(&'a VecDeque<Due>);
        impl serde::Serialize for View<'_> {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                use serde::ser::SerializeSeq;
                let mut seq = s.serialize_seq(Some(self.0.len()))?;
                for due in self.0 {
                    seq.serialize_element(&due.queued_presence_epoch)?;
                }
                seq.end()
            }
        }
        View(&self.queue)
    }
    pub(crate) fn require_legacy(&self) -> Result<()> {
        if self.has_continuation() {
            Err(CheckpointError::new(
                "night",
                "load continuation requires Night V2",
            ))
        } else {
            Ok(())
        }
    }
    pub(crate) fn bind_input(
        &mut self,
        input: Option<&crate::engine::cognition_inputs_checkpoint::NightInput>,
    ) {
        if let (Some(f), Some(input)) = (&mut self.in_flight, input) {
            f.output_token_budget =
                crate::traits::AcceptedOutputBudget::Accepted(input.output_token_budget());
        }
    }
    pub(crate) fn continuation_preflight(&self, world: &World) -> Result<()> {
        let migration_drops=self.queue.iter().filter(|d| d.queued_presence_epoch.is_none()
            && d.presence_epoch.is_none() && matches!(&d.subject, Subject::Person(id) if !world.characters.contains_key(id))).count() as u64;
        checkpoint::check(
            self.dropped
                .checked_add(migration_drops)
                .is_some_and(|n| n <= u64::MAX - checkpoint::COUNTER_DROP_HEADROOM),
            "Night migration counter headroom",
        )
    }
    pub(crate) fn prepare_continuation(&mut self, world: &mut World) {
        // V1 never captured the queue-time incarnation. Its explicit migration
        // anchors a still-unadmitted person to the saved World lifetime only.
        // Already admitted V1 rows retain their original admission epoch.
        self.queue.retain_mut(|due| {
            if let Subject::Person(id) = &due.subject {
                if due.queued_presence_epoch.is_none() {
                    due.queued_presence_epoch = due
                        .presence_epoch
                        .or_else(|| world.characters.get(id).map(|a| a.state.presence_epoch));
                }
                if due.queued_presence_epoch.is_none() {
                    self.dropped += 1;
                    if let Some(root) = due.semantic {
                        receipts::release_finished_root(world, root);
                    }
                    return false;
                }
            }
            true
        });
        self.load_retry_pending = self.in_flight.is_some() && self.held_result.is_none();
    }
    pub(crate) fn install_continuation_v2(
        &mut self,
        pending: bool,
        queued: Vec<Option<u64>>,
    ) -> Result<()> {
        checkpoint::check(
            !pending || (self.in_flight.is_some() && self.held_result.is_none()),
            "invalid pending Night retry",
        )?;
        checkpoint::check(
            queued.len() == self.queue.len(),
            "Night queued incarnation count disagreement",
        )?;
        for (due, epoch) in self.queue.iter_mut().zip(queued) {
            checkpoint::check(
                matches!(
                    (&due.subject, epoch),
                    (Subject::Person(_), Some(_)) | (Subject::Ward(_), None)
                ),
                "Night queued incarnation subject disagreement",
            )?;
            due.queued_presence_epoch = epoch;
        }
        self.load_retry_pending = pending;
        Ok(())
    }
    pub(super) fn submit_load_retry(
        &mut self,
        now: f64,
        world: &mut World,
        clock: &WorldClock,
        cognition: &mut dyn Cognition,
    ) {
        let f = self.in_flight.as_ref().expect("pending Night flight");
        let valid = f.owed_day == clock.at(now).day
            && match &f.subject {
                Subject::Ward(_) => true,
                Subject::Person(id) => {
                    world.is_present(id)
                        && world.characters.get(id).is_some_and(|a| {
                            a.control() == Control::Llm
                                && Some(a.state.presence_epoch) == f.presence_epoch
                        })
                }
            };
        if !valid {
            let f = self.in_flight.take().expect("pending Night flight");
            self.load_retry_pending = false;
            self.dropped += 1;
            receipts::release_finished_root(world, f.semantic);
            return;
        }
        let crate::traits::AcceptedOutputBudget::Accepted(budget) = f.output_token_budget else {
            unreachable!("complete accepted Night input")
        };
        let label_bytes = f.subject.archive_label_bytes(world);
        let submission = cognition
            .reserve_prompt_archive(f.prompt.capacity(), label_bytes)
            .and_then(|archive| {
                cognition
                    .request_night(f.prompt.clone(), budget)
                    .map(|id| (id, archive))
            });
        match submission {
            Ok((request_id, archive)) => {
                self.archive = Some(archive);
                self.in_flight.as_mut().unwrap().request_id = request_id;
                self.load_retry_pending = false;
                self.next_attempt_at = now + pace_seconds(clock);
            }
            Err(_) => self.next_attempt_at = now + RETRY_SECONDS,
        }
    }
}
