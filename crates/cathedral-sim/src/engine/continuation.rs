//! Owner-side joint pending-work transformation. No normal poll or IO.
use super::*;
use crate::checkpoint::{
    Result,
    complete::{ContinuationReport, ContinuationServices, ContinuationStage},
};
#[cfg(test)]
mod complete_tests;
#[cfg(test)]
pub(crate) mod future_harness;
#[cfg(test)]
mod future_tests;
impl Engine {
    pub(crate) fn preflight_continuation(
        &mut self,
        inputs: &cognition_inputs_checkpoint::EngineCognitionInputsCandidate,
        speech: &speech_checkpoint::EngineSpeechCandidate,
    ) -> Result<()> {
        self.scheduler.bind_input(inputs.scheduler());
        self.night.bind_input(inputs.night());
        self.scheduler.continuation_preflight()?;
        self.night.continuation_preflight(&self.world)?;
        speech.continuation_preflight()
    }
    #[cfg(test)]
    pub(crate) fn prepare_pending_work(
        &mut self,
        speech: speech_checkpoint::EngineSpeechCandidate,
        readable: &[crate::checkpoint::host::RecordV1<String>],
        now: crate::timeline::LogicalTime,
        typed: usize,
        observer: &mut impl FnMut(ContinuationStage),
    ) -> Result<ContinuationReport> {
        self.prepare_pending_work_retained(speech, readable, now, typed, observer)
            .map_err(|(error, _)| error)
    }
    pub(crate) fn prepare_pending_work_retained(
        &mut self,
        speech: speech_checkpoint::EngineSpeechCandidate,
        readable: &[crate::checkpoint::host::RecordV1<String>],
        now: crate::timeline::LogicalTime,
        typed: usize,
        observer: &mut impl FnMut(ContinuationStage),
    ) -> std::result::Result<
        ContinuationReport,
        (
            crate::checkpoint::CheckpointError,
            speech_checkpoint::EngineSpeechCandidate,
        ),
    > {
        self.scheduler.prepare_continuation(&mut self.world);
        self.night.prepare_continuation(&mut self.world);
        observer(ContinuationStage::Inputs);
        speech.prepare_continuation_retained(&mut self.world, &mut self.speech_router, now)?;
        observer(ContinuationStage::Speech);
        let released_audio_waits = self.floor.prepare_continuation(now.seconds(), readable);
        observer(ContinuationStage::Floor);
        Ok(ContinuationReport {
            scheduler_retries: self.scheduler.load_retry_count(),
            scheduler_held: self.scheduler.has_held_result(),
            scheduler_resumed: self.scheduler.resumed(),
            night_retry: self.night.load_retry_pending(),
            night_held: self.night.has_held_completion(),
            interrupted_inputs: self
                .speech_router
                .interrupted
                .iter()
                .map(|g| g.count())
                .sum(),
            interruption_receipts: self
                .speech_router
                .interrupted
                .iter()
                .map(|g| g.receipts().len())
                .sum(),
            released_audio_waits,
            typed_upper_bytes: typed,
            services_bound: false,
        })
    }
    pub(crate) fn bind_continuation_services(&mut self, s: ContinuationServices) {
        self.cognition = s.cognition;
        self.transcription = s.transcription;
        self.tts = s.tts;
        self.sight = s.sight;
        self.capabilities = s.capabilities;
        self.config.runtime_dir = s.runtime_dir;
    }
    pub(crate) fn interrupted_speech(
        &self,
    ) -> &[crate::speech_router::checkpoint::InterruptedSpeech] {
        &self.speech_router.interrupted
    }
}

/// Test-only application-adoption stand-in. It roundtrips the real complete
/// pending-owner codecs, uses the same production preparation, and leaves
/// unrelated owners on the supplied fixture Engine. Production cannot extract
/// an Engine from PreparedContinuation; full host tests exercise hydration.
#[cfg(test)]
pub(crate) fn adopt_for_test(
    mut engine: Engine,
    now: crate::timeline::LogicalTime,
    generation: crate::RuntimeGeneration,
    readable: &[crate::checkpoint::host::RecordV1<String>],
) -> Engine {
    use crate::checkpoint::{
        CheckpointBudget, Cohort,
        complete::{CheckpointCategory as C, meter::DecodeMeter},
    };
    let budget = CheckpointBudget::default();
    let _running = budget
        .reserve(
            Cohort::Running,
            crate::checkpoint::complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES,
        )
        .unwrap();
    let mut save = budget
        .reserve(
            Cohort::SavePayload,
            crate::checkpoint::complete::meter::VALIDATION_SCRATCH,
        )
        .unwrap();
    let mut raws = Vec::new();
    for c in [C::Scheduler, C::Night, C::Speech, C::CognitionInputs] {
        let mut bytes = Vec::new();
        engine
            .complete_write_category(c, now, &mut bytes, &mut save)
            .unwrap();
        raws.push(bytes);
    }
    let mut load = budget.reserve(Cohort::LoadCandidate, 4096).unwrap();
    let meter = DecodeMeter::new(&mut load, raws.iter().map(Vec::capacity).sum()).unwrap();
    let scheduler = scheduler_checkpoint::EngineSchedulerDtoV1::complete_decode(
        &raws[0],
        &meter,
        engine.scheduler_checkpoint_context(now),
    )
    .unwrap();
    let night = night_checkpoint::EngineNightDtoV1::complete_decode(
        &raws[1],
        &meter,
        engine.night_checkpoint_context(now),
    )
    .unwrap();
    let speech = speech_checkpoint::EngineSpeechDtoV1::complete_decode(
        &raws[2],
        &meter,
        engine.speech_checkpoint_context(now),
    )
    .unwrap();
    let inputs = cognition_inputs_checkpoint::EngineCognitionInputsDtoV1::complete_decode(
        &raws[3],
        &meter,
        engine.cognition_inputs_checkpoint_context(now),
    )
    .unwrap();
    engine.scheduler = scheduler.data.scheduler;
    engine.night = night.data.night.night;
    engine.speech_router = SpeechRouter::new(speech.stt_stream_grace_seconds());
    engine.preflight_continuation(&inputs, &speech).unwrap();
    engine
        .prepare_pending_work(speech, readable, now, meter.expanded(), &mut |_| {})
        .unwrap();
    engine.config.runtime_generation = generation;
    engine
}

#[cfg(test)]
pub(crate) fn demo_engine(mut config: EngineConfig, cognition: Box<dyn Cognition>) -> Engine {
    use crate::{NullSight, NullTranscription, NullTts};
    config.nav = Some(crate::dogs::checkpoint::tests::nav());
    config.clock = WorldClock::new(3600.0, Office::Waning, 0, 0.05);
    Engine::new(
        config,
        &crate::WorldSeed::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/demo_seed.json"
        )))
        .unwrap(),
        AreaMap::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/world/areas.json"
        )))
        .unwrap(),
        SoundCatalog::from_toml_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sounds/catalog.toml"
        )))
        .unwrap(),
        PromptEnv::new(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/turn.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/night.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/strings.toml"
            )),
        )
        .unwrap(),
        cognition,
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(0.0, 0.91, 111.0), 0.0),
        0,
        0.0,
    )
    .unwrap()
}
