use super::*;
use crate::{
    ActorId, Character, Engine, EngineMessage, WorldClock,
    engine::{
        animals_checkpoint::EngineAnimalsCandidate, climate_checkpoint::EngineClimateCandidate,
        knowledge_checkpoint::EngineKnowledgeCandidate, law_checkpoint::EngineLawCandidate,
        marks_checkpoint::EngineMarksCandidate,
    },
    receipts::CommandLedgerDtoV1,
    world::checkpoint::BackboneCandidate,
};

#[derive(Clone, Copy)]
pub(crate) struct SimRefs<'a> {
    pub characters: &'a std::collections::BTreeMap<ActorId, Character>,
    pub spatial_sequence: i64,
    pub clock: WorldClock,
    pub movement_now: f64,
    pub calendar: Option<crate::WorldTime>,
    pub law: Option<&'a EngineMessage>,
    pub journal: Option<&'a EngineMessage>,
    pub chalk: Option<&'a EngineMessage>,
    pub host_high_water: u64,
}
#[derive(Clone, Copy)]
pub struct HostCheckpointContext<'a> {
    pub(crate) sim: SimRefs<'a>,
    pub(crate) definitions: DefinitionsV1,
    pub(crate) elapsed: Duration,
    pub(crate) input_watermark: u64,
    pub(crate) issued: u64,
    saved: Option<(
        &'a EngineLawCandidate,
        &'a EngineKnowledgeCandidate,
        &'a EngineMarksCandidate,
        &'a EngineClimateCandidate,
        &'a EngineAnimalsCandidate,
    )>,
    ledger: Option<&'a CommandLedgerDtoV1>,
}
impl<'a> HostCheckpointContext<'a> {
    pub fn from_engine(
        engine: &'a Engine,
        elapsed: Duration,
        definitions: DefinitionsV1,
        input_watermark: u64,
        issued: u64,
    ) -> Self {
        Self {
            sim: engine.host_checkpoint_refs(),
            definitions,
            elapsed,
            input_watermark,
            issued,
            saved: None,
            ledger: None,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn from_components(
        backbone: &'a BackboneCandidate,
        law: &'a EngineLawCandidate,
        knowledge: &'a EngineKnowledgeCandidate,
        marks: &'a EngineMarksCandidate,
        climate: &'a EngineClimateCandidate,
        animals: &'a EngineAnimalsCandidate,
        ledger: &'a CommandLedgerDtoV1,
        elapsed: Duration,
        definitions: DefinitionsV1,
        input_watermark: u64,
        issued: u64,
    ) -> Self {
        Self {
            sim: SimRefs {
                characters: backbone.references().characters,
                calendar: backbone.references().current_time,
                spatial_sequence: backbone.host_spatial_sequence(),
                clock: climate.host_clock(),
                movement_now: animals.movement_now().seconds(),
                law: law.last_law_standing(),
                journal: knowledge.host_last_journal(),
                chalk: marks.last_chalk_standing(),
                host_high_water: ledger.host_high_water(),
            },
            definitions,
            elapsed,
            input_watermark,
            issued,
            saved: Some((law, knowledge, marks, climate, animals)),
            ledger: Some(ledger),
        }
    }
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
    pub fn movement_residual(&self) -> f64 {
        self.elapsed.as_secs_f64() - self.sim.movement_now
    }
    fn player(&self) -> Result<&Character> {
        self.sim
            .characters
            .iter()
            .find(|(id, _)| id.as_str() == "player")
            .map(|(_, p)| p)
            .ok_or_else(|| error("host player missing"))
    }
    pub fn position(&self) -> Result<[f64; 3]> {
        let p = self.player()?.position_m();
        Ok([p.x, p.y, p.z])
    }
    pub fn yaw(&self) -> Result<f64> {
        Ok(self.player()?.state.facing_yaw)
    }
    pub fn clock(&self) -> &WorldClock {
        &self.sim.clock
    }
    pub fn law(&self) -> Option<&EngineMessage> {
        self.sim.law
    }
    pub fn journal(&self) -> Option<&EngineMessage> {
        self.sim.journal
    }
    pub fn chalk(&self) -> Option<&EngineMessage> {
        self.sim.chalk
    }
    pub(crate) fn validate(&self) -> Result<()> {
        let expected = self.sim.clock.at(self.elapsed.as_secs_f64());
        check(
            self.sim.calendar.is_some_and(|actual| {
                actual.day == expected.day
                    && actual.fraction.to_bits() == expected.fraction.to_bits()
                    && actual.office == expected.office
                    && actual.weekday == expected.weekday
            }),
            "host context calendar/backbone disagreement",
        )?;
        if let Some(l) = self.ledger {
            l.validate(
                crate::timeline::LogicalTime::new(self.elapsed.as_secs_f64())
                    .ok_or_else(|| error("invalid host logical boundary"))?,
            )?;
        }
        if let Some((l, k, m, c, a)) = self.saved {
            for (time, player) in [
                l.host_boundary(),
                k.host_boundary(),
                m.host_boundary(),
                c.host_boundary(),
                a.host_boundary(),
            ] {
                check(
                    time.seconds().to_bits() == self.elapsed.as_secs_f64().to_bits()
                        && player == "player",
                    "host context mixes component boundaries/player",
                )?;
            }
        }
        check(
            self.issued >= self.sim.host_high_water,
            "host allocator regresses below sim high water",
        )
    }
}
