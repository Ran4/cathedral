//! Sole private construction path for a fully validated complete checkpoint.
//! No normal creation, tick, provider probe, retry or publication is invoked.
use super::*;
use crate::checkpoint::{
    complete::{HydrationAssets, WorldIdentity},
    host::HostCandidate,
};
use cognition_inputs_checkpoint::EngineCognitionInputsCandidate;
use complete_checkpoint::ValidatedOwners;
use speech_checkpoint::EngineSpeechCandidate;

struct QuarantinedCognition;
impl Cognition for QuarantinedCognition {
    fn request(
        &mut self,
        _: String,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        panic!("quarantined hydration cannot submit cognition")
    }
}

pub(crate) fn construct(
    owners: ValidatedOwners,
    assets: HydrationAssets,
    world_identity: WorldIdentity,
    generation: crate::RuntimeGeneration,
) -> (
    Engine,
    EngineSpeechCandidate,
    HostCandidate,
    EngineCognitionInputsCandidate,
) {
    let ValidatedOwners {
        ledger,
        operations,
        backbone,
        round,
        climate,
        knowledge,
        law,
        marks,
        animals,
        social,
        continuity,
        scheduler,
        night,
        speech,
        cognition,
        host,
    } = owners;
    let climate = climate.data;
    let knowledge = knowledge.data;
    let law = law.data;
    let marks = marks.data;
    let animals = animals.data;
    let social = social.data;
    let continuity = continuity.data;
    let scheduler = scheduler.data;
    let night = night.data;
    let HydrationAssets {
        seed_identity,
        mut config,
        env,
        world: world_assets,
    } = assets;
    config.runtime_generation = generation;
    config.checkpoint_world_identity = Some(world_identity);
    // Immutable configuration has already been rebound exactly. No fallback,
    // seed-time normalization or current service availability changes it.
    let world = backbone.hydrate(
        world_assets,
        crate::world::checkpoint::HydrationOwners {
            ledger,
            operations,
            // This is a derived owner index, NOT terminalization/interruption. Every
            // accepted receipt remains protected until M2c jointly restores speech.
            speech_actions: speech.semantic_ids().collect(),
            current_weather: climate.world.current_weather,
            notices: law.world.notices,
            custody: law.world.custody,
            dogs: animals.world.dogs,
            marks: marks.world.marks,
            marks_enabled: marks.world.marks_enabled,
            mark_kinds: marks.world.mark_kinds,
            ward_moods: night.world.ward_moods,
            knowledge: knowledge.world.knowledge,
            knowledge_enabled: knowledge.world.knowledge_enabled,
            area_adjacency: knowledge.world.area_adjacency,
            pollen_no_salience: knowledge.world.pollen_no_salience,
        },
    );
    let engine = Engine {
        checkpoint_seed_identity: seed_identity,
        world,
        // Omniscient transcript is a session-only presentation artifact. Saved
        // committed readable lines remain in HostCandidate, never replayed here.
        transcript: Vec::new(),
        scheduler: scheduler.scheduler,
        floor: continuity.floor,
        // Transport state cannot be resumed. Its COMPLETE interruption authority
        // remains separately owned in `speech`, and this Engine cannot be polled.
        speech_router: SpeechRouter::new(speech.stt_stream_grace_seconds()),
        env,
        cognition: Box::new(QuarantinedCognition),
        transcription: Box::new(crate::NullTranscription),
        tts: Box::new(crate::NullTts),
        sight: Box::new(crate::NullSight),
        config,
        // Availability is a future runtime service observation, not saved state.
        capabilities: Capabilities::default(),
        tts_selected: continuity.tts_selected,
        last_snapshot_revision: continuity.last_snapshot_revision,
        last_player_sound_at: continuity.last_player_sound_at,
        conversation: social.conversation,
        npc_exchanges: social.warm_exchanges,
        novelty: social.novelty,
        clock: climate.clock.clock(),
        weather: climate.weather,
        last_weather_days: climate.last_weather_days,
        last_weather_sample: climate.last_weather_sample,
        last_clock_days: climate.last_clock_days,
        bell_strokes: climate.bell_strokes,
        bell_seq: climate.bell_seq,
        movement_now: animals.movement_now.seconds(),
        round: round.into_hydration(),
        night: night.night.night,
        next_round_tick_at: continuity.next_round_tick_at,
        next_stage_hop_at: knowledge.next_stage_hop_at,
        next_player_pollen_game_days: knowledge.next_player_pollen_game_days.legacy(),
        lamp_revision_sent: continuity.lamp_revision_sent,
        dogs_published: animals.dogs_published,
        startup_diagnostics: continuity.startup_diagnostics,
        ready_emitted: continuity.ready_emitted,
        last_law_standing: law.last_law_standing,
        last_chalk_standing: marks.last_chalk_standing,
        last_ward_heat: knowledge.last_ward_heat,
        door_shut_until: knowledge.door_shut_until,
        last_journal: knowledge.last_journal,
        last_journal_receipts: knowledge.last_journal_receipts,
        last_journal_at: knowledge.last_journal_at.legacy(),
    };
    (engine, speech, host, cognition)
}
