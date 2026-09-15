//! Exhaustive ownership split for disposal. Neither returned owner grants
//! access to a World or a pollable Engine. No unsafe Send assertion is used.
use super::*;

/// Opaque Send authority, permitted only to be destroyed after retirement.
#[allow(dead_code)]
pub struct RetiredEngineState {
    checkpoint_seed_identity: [u8; 32],
    world: World,
    transcript: Vec<String>,
    scheduler: NpcScheduler,
    floor: ConversationFloor,
    speech_router: SpeechRouter,
    env: PromptEnv,
    config: EngineConfig,
    capabilities: Capabilities,
    tts_selected: TtsBackendKind,
    last_snapshot_revision: i64,
    last_player_sound_at: f64,
    conversation: crate::conversation::Conversation,
    npc_exchanges: WarmExchanges,
    novelty: Novelty,
    clock: WorldClock,
    weather: WeatherTimeline,
    last_weather_days: f64,
    last_weather_sample: WeatherSample,
    last_clock_days: f64,
    bell_strokes: VecDeque<f64>,
    bell_seq: u64,
    movement_now: f64,
    round: Round,
    night: NightOffice,
    next_round_tick_at: f64,
    next_stage_hop_at: f64,
    next_player_pollen_game_days: f64,
    lamp_revision_sent: u64,
    dogs_published: bool,
    startup_diagnostics: Vec<String>,
    ready_emitted: bool,
    last_law_standing: Option<EngineMessage>,
    last_chalk_standing: Option<EngineMessage>,
    last_ward_heat: Option<EngineMessage>,
    door_shut_until: BTreeMap<(ActorId, ActorId), f64>,
    last_journal: Option<EngineMessage>,
    last_journal_receipts: u64,
    last_journal_at: f64,
}
/// Non-Send service adapters remain on their host thread. Production hosts take
/// the real Send services out of their forwarding owners before disposing this.
pub struct RetiredEngineAdapters {
    cognition: Box<dyn Cognition>,
    transcription: Box<dyn Transcription>,
    tts: Box<dyn Tts>,
    sight: Box<dyn Sight>,
}
impl RetiredEngineAdapters {
    /// Drops only adapters for a production host that already detached its real
    /// service owners. Arbitrary user-provided service Drops are not claimed to
    /// be bounded; they are a trusted host responsibility.
    pub fn dispose(self) {
        let Self {
            cognition,
            transcription,
            tts,
            sight,
        } = self;
        drop((cognition, transcription, tts, sight));
    }
}
impl Engine {
    /// Irreversibly consume an already fenced Engine for disposal. The host
    /// must reserve its retiring slot and actual lifetime charge BEFORE this
    /// call; this performs only exhaustive moves, never seeding or polling.
    pub fn into_retirement(self) -> (RetiredEngineState, RetiredEngineAdapters) {
        let Self {
            checkpoint_seed_identity,
            world,
            transcript,
            scheduler,
            floor,
            speech_router,
            env,
            cognition,
            transcription,
            tts,
            sight,
            config,
            capabilities,
            tts_selected,
            last_snapshot_revision,
            last_player_sound_at,
            conversation,
            npc_exchanges,
            novelty,
            clock,
            weather,
            last_weather_days,
            last_weather_sample,
            last_clock_days,
            bell_strokes,
            bell_seq,
            movement_now,
            round,
            night,
            next_round_tick_at,
            next_stage_hop_at,
            next_player_pollen_game_days,
            lamp_revision_sent,
            dogs_published,
            startup_diagnostics,
            ready_emitted,
            last_law_standing,
            last_chalk_standing,
            last_ward_heat,
            door_shut_until,
            last_journal,
            last_journal_receipts,
            last_journal_at,
        } = self;
        (
            RetiredEngineState {
                checkpoint_seed_identity,
                world,
                transcript,
                scheduler,
                floor,
                speech_router,
                env,
                config,
                capabilities,
                tts_selected,
                last_snapshot_revision,
                last_player_sound_at,
                conversation,
                npc_exchanges,
                novelty,
                clock,
                weather,
                last_weather_days,
                last_weather_sample,
                last_clock_days,
                bell_strokes,
                bell_seq,
                movement_now,
                round,
                night,
                next_round_tick_at,
                next_stage_hop_at,
                next_player_pollen_game_days,
                lamp_revision_sent,
                dogs_published,
                startup_diagnostics,
                ready_emitted,
                last_law_standing,
                last_chalk_standing,
                last_ward_heat,
                door_shut_until,
                last_journal,
                last_journal_receipts,
                last_journal_at,
            },
            RetiredEngineAdapters {
                cognition,
                transcription,
                tts,
                sight,
            },
        )
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn retired_domain_is_send_without_service_erasure() {
        fn send<T: Send>() {}
        send::<super::RetiredEngineState>();
    }
}
