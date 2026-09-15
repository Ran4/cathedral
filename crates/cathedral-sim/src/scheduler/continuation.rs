//! Load-specific accepted work. No prompt render and no restoration into inbox.
use super::*;
use crate::checkpoint::{CheckpointError, Result};

pub const MAX_LOAD_RETRIES: usize = 64;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PromptContext {
    #[serde(with = "crate::knowledge::checkpoint::records::fact_key::vec")]
    pub(crate) seated: Vec<crate::FactKey>,
    #[serde(with = "crate::knowledge::checkpoint::records::occasion::option")]
    pub(crate) occasion: Option<crate::knowledge::Occasion>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoadRetry {
    #[serde(with = "super::checkpoint::records::FlightV2")]
    pub(super) flight: InFlight,
    pub(crate) context: PromptContext,
}

impl NpcScheduler {
    pub(crate) fn held_archive_requirements(&self, world: &World) -> Option<(usize, usize)> {
        self.held_result.as_ref()?;
        let f = self.in_flight.as_ref()?;
        Some((
            f.prompt.capacity(),
            world
                .characters
                .get(&f.actor_id)
                .map_or(0, |a| a.name().len())
                .max(f.actor_id.as_str().len())
                .saturating_add(f.actor_id.as_str().len()),
        ))
    }
    pub(crate) fn bind_held_archive(
        &mut self,
        archive: Option<crate::prompt_archive::PromptArchivePermit>,
    ) {
        self.archive = archive;
    }

    pub(crate) fn validate_context_knowledge(
        &self,
        knowledge: &crate::knowledge::Knowledge,
    ) -> Result<()> {
        for c in self
            .resumed_context
            .iter()
            .chain(self.load_retries.iter().map(|r| &r.context))
        {
            if !knowledge.valid_resumed_keys(c) {
                return Err(CheckpointError::new(
                    "scheduler",
                    "resumed seated key allocation disagreement",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn load_retry_count(&self) -> usize {
        self.load_retries.len()
    }
    pub(crate) fn resumed(&self) -> bool {
        self.resumed_context.is_some()
    }
    pub(super) fn apply_load_failure(
        &mut self,
        now: f64,
        world: &mut World,
        flight: InFlight,
        actor_name: &str,
        error: &CognitionError,
        events: &mut Vec<SchedulerEvent>,
    ) {
        // This IS a new provider failure, unlike preparation/Busy. Its error
        // percept and backoff are real, but exact input remains on its own
        // obligation instead of overwriting an actor-keyed later failure.
        let backoff = self.backoff_after_failure();
        self.next_turn_at = now + backoff;
        if let Some(actor) = world.characters.get_mut(&flight.actor_id) {
            actor.state.inbox.push(SYSTEM_PROVIDER_FAILED.to_string());
            actor.rebound_percepts();
        }
        let context = world.knowledge.detach_prompt(&flight.actor_id);
        events.push(SchedulerEvent::Diagnostic(format!(
            "[smart actors] LLM request for {actor_name} failed: {error}"
        )));
        events.push(SchedulerEvent::Status(StatusEvent::llm(
            STATE_DEGRADED,
            Some(flight.actor_id.clone()),
            Some(format!(
                "provider request failed; retrying in {} seconds",
                format_g(backoff)
            )),
        )));
        self.load_retries.push_back(LoadRetry { flight, context });
        self.load_retries
            .make_contiguous()
            .sort_by_key(|r| r.flight.semantic);
    }

    pub(crate) fn has_continuation(&self) -> bool {
        !self.load_retries.is_empty() || self.resumed_context.is_some()
    }

    pub(crate) fn require_legacy(&self) -> Result<()> {
        if self.has_continuation() {
            Err(CheckpointError::new(
                "scheduler",
                "load continuation requires scheduler V2",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn continuation_preflight(&self) -> Result<()> {
        let pending = usize::from(self.in_flight.is_some() && self.held_result.is_none());
        if self.load_retries.len() + pending > MAX_LOAD_RETRIES {
            return Err(CheckpointError::new("scheduler", "load retry count limit"));
        }
        for f in self
            .in_flight
            .iter()
            .chain(self.load_retries.iter().map(|r| &r.flight))
        {
            if matches!(
                f.output_token_budget,
                crate::traits::AcceptedOutputBudget::MissingLegacy
            ) {
                return Err(CheckpointError::new(
                    "scheduler",
                    "missing accepted cognition input",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn prepare_continuation(&mut self, world: &mut World) {
        if self.held_result.is_none()
            && let Some(flight) = self.in_flight.take()
        {
            let context = self
                .resumed_context
                .take()
                .unwrap_or_else(|| world.knowledge.detach_prompt(&flight.actor_id));
            self.load_retries.push_back(LoadRetry { flight, context });
            // Stable semantic allocation order is the original turn order,
            // independent of how many replacement executions have been retired.
            self.load_retries
                .make_contiguous()
                .sort_by_key(|r| r.flight.semantic);
        }
    }

    pub(crate) fn bind_input(
        &mut self,
        input: Option<&crate::engine::cognition_inputs_checkpoint::SchedulerInput>,
    ) {
        if let (Some(f), Some(input)) = (&mut self.in_flight, input) {
            f.output_token_budget =
                crate::traits::AcceptedOutputBudget::Accepted(input.output_token_budget());
        }
    }

    pub(super) fn retire_obsolete_load_retries(&mut self, world: &mut World) {
        let mut i = 0;
        while i < self.load_retries.len() {
            let f = &self.load_retries[i].flight;
            let valid = world.is_present(&f.actor_id)
                && world.characters.get(&f.actor_id).is_some_and(|a| {
                    a.control() == Control::Llm && a.state.presence_epoch == f.presence_epoch
                });
            if valid {
                i += 1;
            } else {
                let r = self.load_retries.remove(i).expect("existing retry");
                receipts::release_finished_root(world, r.flight.semantic);
            }
        }
    }

    /// True means the load lane either submitted or retained its exact work on
    /// Busy; ordinary selection must not spend another obligation this poll.
    pub(super) fn submit_load_retry(
        &mut self,
        now: f64,
        world: &mut World,
        idle: IdleGate<'_>,
        cognition: &mut dyn Cognition,
        events: &mut Vec<SchedulerEvent>,
    ) -> bool {
        let protected = self
            .load_retries
            .iter()
            .position(|r| r.flight.lane == TurnLane::PlayerReaction);
        if protected.is_none()
            && self.load_retries.len() == MAX_LOAD_RETRIES
            && idle.is_suppressed()
        {
            return true;
        }
        let index = protected.or_else(|| {
            // Full exact-work capacity backpressures ordinary prompt creation.
            // Keep its intent queued and retire an existing obligation first,
            // so every reachable active+deferred state can be prepared again.
            (self.load_retries.len() == MAX_LOAD_RETRIES
                || (self.player_reactions.is_empty()
                    && !idle.is_suppressed()
                    && !self.load_retries.is_empty()))
            .then_some(0)
        });
        let Some(index) = index else {
            return false;
        };
        let retry = &self.load_retries[index];
        let crate::traits::AcceptedOutputBudget::Accepted(budget) =
            retry.flight.output_token_budget
        else {
            unreachable!("complete preparation checked exact input authority")
        };
        let label_bytes = world
            .characters
            .get(&retry.flight.actor_id)
            .map_or(0, |a| a.name().len())
            .max(retry.flight.actor_id.as_str().len())
            .saturating_add(retry.flight.actor_id.as_str().len());
        let submission = cognition
            .reserve_prompt_archive(retry.flight.prompt.capacity(), label_bytes)
            .and_then(|archive| {
                cognition
                    .request_with_budget(retry.flight.prompt.clone(), budget)
                    .map(|id| (id, archive))
            });
        match submission {
            Ok((request_id, archive)) => {
                self.archive = Some(archive);
                let mut retry = self.load_retries.remove(index).expect("selected retry");
                retry.flight.request_id = request_id;
                events.push(SchedulerEvent::Status(StatusEvent::llm(
                    STATE_THINKING,
                    Some(retry.flight.actor_id.clone()),
                    None,
                )));
                self.in_flight = Some(retry.flight);
                self.resumed_context = Some(retry.context);
                // Original submission already consumed/stamped its novelty.
            }
            Err(_) => self.next_turn_at = now + self.minimum_delay_seconds.max(1.0),
        }
        true
    }
}
