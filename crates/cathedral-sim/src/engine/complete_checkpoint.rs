//! Same-boundary complete composition. Contexts use saved mutable owners and
//! explicitly retained installed definition roles; no World or Engine is seeded.
use super::*;
use crate::checkpoint::{
    self, BoundedText, Reservation, Result,
    complete::{
        self, CheckpointCategory as Category, CompleteManifestV1, InstalledCheckpointDefinitions,
        check, hash, meter::DecodeMeter, wire::Envelope,
    },
    host::{self},
};
use crate::timeline::LogicalTime;

struct InstalledWorldRoles<'a> {
    items: &'a crate::ItemCatalog,
    nav: Option<&'a NavData>,
    shelters: &'a ShelterMap,
    areas: &'a AreaMap,
    sounds: &'a SoundCatalog,
    marks: &'a crate::marks::MarkCatalog,
    facts: &'a crate::knowledge::FactCatalog,
    salience: &'a crate::knowledge::SalienceTable,
    area_adjacency: &'a crate::knowledge::AreaAdjacency,
}
impl<'a> InstalledCheckpointDefinitions<'a> {
    /// Bind actual parsed assets without constructing or seeding a World/Engine.
    /// Caller retains the factory's admitted asset/scratch ownership throughout.
    pub fn from_assets(
        assets: &'a complete::HydrationAssets,
        host: host::DefinitionsV1,
        now: LogicalTime,
    ) -> Result<Self> {
        let w = &assets.world;
        Self::from_roles(
            &assets.config,
            assets.seed_identity,
            assets.env.checkpoint_identity(),
            InstalledWorldRoles {
                items: &w.items,
                nav: w.nav.as_deref(),
                shelters: &w.shelters,
                areas: &w.areas,
                sounds: &w.sounds,
                marks: &w.marks,
                facts: &w.facts,
                salience: &w.salience,
                area_adjacency: &w.area_adjacency,
            },
            host,
            now,
        )
    }

    pub fn from_engine(e: &'a Engine, host: host::DefinitionsV1, now: LogicalTime) -> Result<Self> {
        e.complete_field_inventory();
        e.world.complete_field_inventory();
        Self::from_roles(
            &e.config,
            e.checkpoint_seed_identity,
            e.env.checkpoint_identity(),
            InstalledWorldRoles {
                items: &e.world.item_catalog,
                nav: e.world.nav.as_deref(),
                shelters: &e.world.shelters,
                areas: &e.world.area_map,
                sounds: &e.world.sound_catalog,
                marks: &e.world.mark_catalog,
                facts: &e.world.fact_catalog,
                salience: &e.world.salience,
                area_adjacency: &e.world.area_adjacency,
            },
            host,
            now,
        )
    }
    fn from_roles(
        c: &'a EngineConfig,
        seed_identity: [u8; 32],
        prompt_identity: [u8; 32],
        roles: InstalledWorldRoles<'a>,
        host: host::DefinitionsV1,
        now: LogicalTime,
    ) -> Result<Self> {
        let configuration = hash(&(
            (
                &c.player_id,
                &c.operations,
                c.fake_mode,
                c.sounds_enabled,
                c.view_cone_degrees.to_bits(),
                c.sound_cooldown_seconds.to_bits(),
            ),
            (
                c.turn_delay_seconds.to_bits(),
                c.maximum_backoff_seconds.to_bits(),
                c.tts_selected,
                &c.tts_startup_message,
                c.stt_stream_grace_seconds.to_bits(),
            ),
            (
                match c.idle_mode {
                    IdleCognitionMode::All => 0u8,
                    IdleCognitionMode::Stage => 1,
                },
                c.stage.radius_m.to_bits(),
                c.stage.max_actors,
                c.idle_requires_news,
                c.idle_curiosity.enabled,
                c.idle_curiosity.scale.to_bits(),
            ),
            (
                c.clock.checkpoint_provenance_v1(now)?,
                c.ring_the_offices,
                c.weather.enabled,
                c.weather.seed,
                format!("{:?}", c.weather.mode),
                c.weather.frequency.to_bits(),
            ),
            (
                c.night_office.enabled,
                c.night_office.majors,
                c.night_office.wards,
                c.night_office.ambients,
            ),
            (
                c.marks_enabled,
                c.mark_kinds.cross,
                c.mark_kinds.tally,
                c.mark_kinds.ward_sign,
                c.marks_decay_scale.to_bits(),
            ),
            (
                c.knowledge_enabled,
                &c.fact_packs,
                c.pollen_flat,
                c.pollen_no_salience,
            ),
        ))?;
        let manifest = CompleteManifestV1 {
            version: 1,
            source_sha256: [0; 32],
            implementation_sha256: [0; 32],
            host_image: c
                .checkpoint_host_image
                .ok_or_else(|| complete::error("actual host image identity is unavailable"))?,
            toolchain: BoundedText::new("").map_err(complete::error)?,
            target: BoundedText::new("").map_err(complete::error)?,
            procedural_hasher: BoundedText::new("").map_err(complete::error)?,
            default_hasher_witness: [0; 4],
            ordered_seed: seed_identity,
            prompts: prompt_identity,
            world_items: roles.items.checkpoint_fingerprint(),
            world_climate_definitions: climate_checkpoint::installed_definition_identity(
                roles.nav,
                roles.shelters,
                roles.areas,
                roles.sounds,
            )?,
            world_area_adjacency:
                crate::knowledge::pollen::checkpoint::complete_adjacency_identity(
                    roles.area_adjacency,
                )?,
            world_marks: crate::marks::checkpoint::complete_catalog_identity(roles.marks)?,
            world_facts: crate::knowledge::catalog::checkpoint::fingerprint(roles.facts)?,
            world_salience: crate::knowledge::salience::checkpoint::fingerprint(roles.salience)?,
            engine_nav: host::Nullable(c.nav.as_deref().map(NavData::checkpoint_fingerprint)),
            engine_shelters: hash(&c.shelters.shelters())?,
            engine_configuration: configuration,
            host,
        }
        .build()?;
        Ok(Self {
            manifest,
            player: &c.player_id,
            items: roles.items,
            world_nav: roles.nav,
            world_shelters: roles.shelters,
            areas: roles.areas,
            sounds: roles.sounds,
            marks: roles.marks,
            facts: roles.facts,
            salience: roles.salience,
            engine_nav: c.nav.as_deref(),
            operations: &c.operations,
            engine_config: c,
        })
    }
}
impl Engine {
    pub(crate) fn complete_write_category<W: std::io::Write>(
        &self,
        category: Category,
        now: LogicalTime,
        writer: &mut W,
        reservation: &mut Reservation,
    ) -> Result<()> {
        match category {
            Category::Ledger => {
                let dto = self.world.command_ledger.checkpoint_v1(
                    now,
                    reservation.sublease(crate::receipts::CommandLedgerDtoV1::WORKING_BYTES)?,
                )?;
                complete::write_json(writer, dto.value())
            }
            Category::Operations => {
                let dto = self.world.operations.checkpoint_v1(
                    &self.config.operations,
                    &self.world,
                    now,
                    reservation.sublease(crate::operations::OperationKernelDtoV1::WORKING_BYTES)?,
                )?;
                complete::write_json(writer, dto.value())
            }
            Category::Backbone => self.world.complete_write_backbone(writer),
            Category::Round => self.round.complete_write(
                crate::round::checkpoint::RoundCheckpointContext::from_world(
                    &self.world,
                    self.config.nav.as_deref(),
                ),
                writer,
            ),
            Category::Climate => self.complete_write_climate(now, writer, reservation),
            Category::Knowledge => self.complete_write_knowledge(now, writer, reservation),
            Category::Law => self.complete_write_law(now, writer, reservation),
            Category::Marks => self.complete_write_marks(now, writer, reservation),
            Category::Animals => self.complete_write_animals(now, writer, reservation),
            Category::Social => self.complete_write_social(now, writer, reservation),
            Category::Continuity => self.complete_write_continuity(now, writer, reservation),
            Category::Scheduler => self.complete_write_scheduler(now, writer, reservation),
            Category::Night => self.complete_write_night(now, writer, reservation),
            Category::Speech => self.complete_write_speech(now, writer, reservation),
            Category::CognitionInputs => {
                self.complete_write_cognition_inputs(now, writer, reservation)
            }
            Category::Host => Err(complete::error(
                "host category requires actual host adapter",
            )),
        }
    }
}

pub(crate) struct ValidatedOwners {
    pub ledger: crate::receipts::CommandLedger,
    pub operations: crate::operations::OperationKernel,
    pub backbone: crate::world::checkpoint::BackboneCandidate,
    pub round: crate::round::checkpoint::RoundCandidate,
    pub climate: climate_checkpoint::EngineClimateCandidate,
    pub knowledge: knowledge_checkpoint::EngineKnowledgeCandidate,
    pub law: law_checkpoint::EngineLawCandidate,
    pub marks: marks_checkpoint::EngineMarksCandidate,
    pub animals: animals_checkpoint::EngineAnimalsCandidate,
    pub social: social_checkpoint::EngineSocialCandidate,
    pub continuity: continuity_checkpoint::EngineContinuityCandidate,
    pub scheduler: scheduler_checkpoint::EngineSchedulerCandidate,
    pub night: night_checkpoint::EngineNightCandidate,
    pub speech: speech_checkpoint::EngineSpeechCandidate,
    pub cognition: cognition_inputs_checkpoint::EngineCognitionInputsCandidate,
    pub host: host::HostCandidate,
}

pub(crate) fn validate_components(
    w: &Envelope<'_>,
    d: &InstalledCheckpointDefinitions<'_>,
    m: &DecodeMeter<'_>,
    observer: &mut impl FnMut(complete::CompleteCheckpointStage),
) -> Result<usize> {
    let owners = decode_components(w, d, m, observer)?;
    Ok(owners.backbone.references().characters.len())
}

pub(crate) fn decode_components(
    w: &Envelope<'_>,
    d: &InstalledCheckpointDefinitions<'_>,
    m: &DecodeMeter<'_>,
    observer: &mut impl FnMut(complete::CompleteCheckpointStage),
) -> Result<ValidatedOwners> {
    use super::{
        animals_checkpoint::EngineAnimalsDtoV1,
        climate_checkpoint::{ClimateCheckpointContext, EngineClimateDtoV1},
        cognition_inputs_checkpoint::{
            CognitionInputsCheckpointContext, EngineCognitionInputsDtoV1,
        },
        continuity_checkpoint::{EngineContinuityCheckpointContext, EngineContinuityDtoV1},
        knowledge_checkpoint::EngineKnowledgeDtoV1,
        law_checkpoint::EngineLawDtoV1,
        marks_checkpoint::EngineMarksDtoV1,
        night_checkpoint::EngineNightDtoV1,
        scheduler_checkpoint::{EngineSchedulerCheckpointContext, EngineSchedulerDtoV1},
        social_checkpoint::{EngineSocialDtoV1, SocialCheckpointContext},
        speech_checkpoint::{EngineSpeechCheckpointContext, EngineSpeechDtoV1},
    };
    use crate::{
        dogs::checkpoint::AnimalsCheckpointContext,
        knowledge::checkpoint::KnowledgeCheckpointContext,
        marks::checkpoint::MarksCheckpointContext,
        night::checkpoint::NightCheckpointContext,
        notices::checkpoint::LawCheckpointContext,
        operations::OperationKernelDtoV1,
        receipts::CommandLedgerDtoV1,
        round::checkpoint::{RoundCheckpointContext, RoundDtoV1},
        world::checkpoint::WorldBackboneDtoV1,
    };
    let now = w.boundary;
    m.begin_category(Category::Ledger);
    let ledger: CommandLedgerDtoV1 = m.decode(w.ledger.get().as_bytes())?;
    ledger.validate(now)?;
    m.charge(ledger.complete_index_upper_bytes())?;
    let ledger_index = ledger.candidate(now)?;
    m.begin_category(Category::Backbone);
    let backbone = WorldBackboneDtoV1::complete_decode(
        w.backbone.get().as_bytes(),
        m,
        d.items,
        &ledger_index,
    )?;
    let refs = backbone.references();
    m.begin_category(Category::Operations);
    let operation_dto: OperationKernelDtoV1 = m.decode(w.operations.get().as_bytes())?;
    // The retained kernel rebuilds three indexes while its bounded DTO is alive.
    // This is complete representation expansion, not reusable validator scratch.
    m.charge(crate::operations::MAX_KERNEL_ALLOCATED_BYTES)?;
    let operations =
        operation_dto.candidate_refs(d.operations, refs.characters, &ledger_index, now)?;
    let round_context =
        RoundCheckpointContext::from_backbone(&backbone, d.items, d.engine_nav, d.world_shelters);
    m.begin_category(Category::Round);
    let round = RoundDtoV1::complete_decode(w.round.get().as_bytes(), m, round_context)?;
    m.begin_category(Category::Climate);
    let climate = EngineClimateDtoV1::complete_decode(
        w.climate.get().as_bytes(),
        m,
        ClimateCheckpointContext::from_backbone(
            &backbone,
            now,
            d.world_nav,
            d.world_shelters,
            d.areas,
            d.sounds,
        ),
    )?;
    let clock = climate.host_clock();
    m.begin_category(Category::Knowledge);
    let knowledge = EngineKnowledgeDtoV1::complete_decode(
        w.knowledge.get().as_bytes(),
        m,
        KnowledgeCheckpointContext::from_backbone(&backbone, now, d.areas, d.facts, d.salience),
    )?;
    m.begin_category(Category::Law);
    let law = EngineLawDtoV1::complete_decode(
        w.law.get().as_bytes(),
        m,
        LawCheckpointContext::from_backbone(&backbone, now),
    )?;
    m.begin_category(Category::Marks);
    let marks = EngineMarksDtoV1::complete_decode(
        w.marks.get().as_bytes(),
        m,
        MarksCheckpointContext::from_backbone(&backbone, now, d.marks, d.world_shelters),
    )?;
    marks.complete_config(d.engine_config)?;
    m.begin_category(Category::Animals);
    let animals = EngineAnimalsDtoV1::complete_decode(
        w.animals.get().as_bytes(),
        m,
        AnimalsCheckpointContext::from_backbone(&backbone, now, d.world_nav)
            .with_engine_nav(d.engine_nav),
    )?;
    m.begin_category(Category::Social);
    let social = EngineSocialDtoV1::complete_decode(
        w.social.get().as_bytes(),
        m,
        SocialCheckpointContext::from_backbone(&backbone, now, d.player),
    )?;
    m.begin_category(Category::Continuity);
    let continuity = EngineContinuityDtoV1::complete_decode(
        w.continuity.get().as_bytes(),
        m,
        EngineContinuityCheckpointContext::from_backbone(&backbone, now, d.player),
    )?;
    m.begin_category(Category::Scheduler);
    let scheduler = EngineSchedulerDtoV1::complete_decode(
        w.scheduler.get().as_bytes(),
        m,
        EngineSchedulerCheckpointContext::from_backbone(&backbone, now, &ledger, d.player),
    )?;
    m.begin_category(Category::Night);
    let night = EngineNightDtoV1::complete_decode(
        w.night.get().as_bytes(),
        m,
        NightCheckpointContext::from_backbone(&backbone, now, &clock, &ledger),
    )?;
    m.begin_category(Category::Speech);
    let speech = EngineSpeechDtoV1::complete_decode(
        w.speech.get().as_bytes(),
        m,
        EngineSpeechCheckpointContext::from_backbone(&backbone, now, &ledger, d.player),
    )?;
    m.begin_category(Category::CognitionInputs);
    let cognition = EngineCognitionInputsDtoV1::complete_decode(
        w.cognition_inputs.get().as_bytes(),
        m,
        CognitionInputsCheckpointContext::from_components(
            &scheduler, &night, &backbone, now, &ledger, &clock,
        ),
    )?;
    // This lightweight scalar parse is bounded by the same metered HostWire;
    // the final full host pass below validates all source-backed publication data.
    m.begin_category(Category::Host);
    let host = host::HostDtoV1::complete_decode_with_components(
        w.host.get().as_bytes(),
        m,
        &backbone,
        &law,
        &knowledge,
        &marks,
        &climate,
        &animals,
        &ledger,
        d.manifest.host,
        now,
    )?;
    let mut roots = BTreeMap::new();
    let mut add = |category: &'static str, ids: Vec<crate::receipts::OperationId>| -> Result<()> {
        for id in ids.into_iter().collect::<BTreeSet<_>>() {
            check(
                roots.insert(id, category).is_none(),
                "semantic root belongs to incompatible complete categories",
            )?;
        }
        Ok(())
    };
    add("operations", operations.complete_roots().collect())?;
    add("legacy", backbone.complete_roots().collect())?;
    add(
        "scheduler",
        scheduler.scheduler().complete_roots().collect(),
    )?;
    add("night", night.night().complete_roots().collect())?;
    add(
        "speech",
        speech.semantic_ids().map(|id| id.operation).collect(),
    )?;
    ledger.validate_owner_roots(&roots.keys().copied().collect())?;
    validate_agreement(
        &backbone,
        &round,
        &climate,
        &social,
        &continuity,
        &scheduler,
        &night,
        &speech,
        &host,
        d,
        now,
    )?;
    observer(complete::CompleteCheckpointStage::TypedValidation);
    Ok(ValidatedOwners {
        ledger: ledger_index,
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
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_agreement(
    backbone: &crate::world::checkpoint::BackboneCandidate,
    round: &crate::round::checkpoint::RoundCandidate,
    climate: &climate_checkpoint::EngineClimateCandidate,
    social: &social_checkpoint::EngineSocialCandidate,
    continuity: &continuity_checkpoint::EngineContinuityCandidate,
    scheduler: &scheduler_checkpoint::EngineSchedulerCandidate,
    night: &night_checkpoint::EngineNightCandidate,
    speech: &speech_checkpoint::EngineSpeechCandidate,
    host: &host::HostCandidate,
    definitions: &InstalledCheckpointDefinitions<'_>,
    now: LogicalTime,
) -> Result<()> {
    let config = definitions.engine_config;
    check(
        continuity.ready_emitted(),
        "complete checkpoint requires an ordinary Ready boundary",
    )?;
    check(
        continuity.last_snapshot_revision() == backbone.complete_revision(),
        "complete snapshot publication revision disagreement",
    )?;
    check(
        continuity.lamp_revision_sent() == round.complete_lamp_revision(),
        "complete lamp publication revision disagreement",
    )?;
    check(
        scheduler.scheduler().complete_submission_consumed(),
        "complete scheduler submission has not reached Novelty",
    )?;
    check(
        scheduler.turn_delay_seconds().to_bits() == config.turn_delay_seconds.to_bits()
            && scheduler.maximum_backoff_seconds().to_bits()
                == config.maximum_backoff_seconds.to_bits(),
        "complete scheduler configuration disagreement",
    )?;
    check(
        social.idle_mode() == config.idle_mode
            && social.stage().radius_m.to_bits() == config.stage.radius_m.to_bits()
            && social.stage().max_actors == config.stage.max_actors
            && social.idle_requires_news() == config.idle_requires_news
            && social.idle_curiosity().enabled == config.idle_curiosity.enabled
            && social.idle_curiosity().scale.to_bits() == config.idle_curiosity.scale.to_bits(),
        "complete social configuration disagreement",
    )?;
    check(
        continuity.fake_mode() == config.fake_mode
            && continuity.sounds_enabled() == config.sounds_enabled
            && continuity.view_cone_degrees().to_bits() == config.view_cone_degrees.to_bits()
            && continuity.sound_cooldown_seconds().to_bits()
                == config.sound_cooldown_seconds.to_bits()
            && continuity.stt_stream_grace_seconds().to_bits()
                == config.stt_stream_grace_seconds.to_bits()
            && continuity.configured_tts_selected() == config.tts_selected
            && continuity.tts_startup_message() == config.tts_startup_message.as_deref(),
        "complete continuity configuration disagreement",
    )?;
    check(
        speech.stt_stream_grace_seconds().to_bits()
            == continuity.stt_stream_grace_seconds().to_bits(),
        "complete speech/continuity grace disagreement",
    )?;
    check(
        night.config_night_office() == config.night_office,
        "complete Night configuration disagreement",
    )?;
    climate.complete_config(config, now)?;
    let clock = climate.host_clock();
    let next = now.seconds() + crate::timeline::MAX_ACCEPTED_FRAME.as_secs_f64();
    checkpoint::logical("complete", next)?;
    let days = clock.game_days(now.seconds());
    let next_days = clock.game_days(next);
    check(
        next_days >= days && next_days - days <= 1.0,
        "complete effective calendar rate exceeds next-frame horizon",
    )?;
    checkpoint::calendar("complete", next_days + 3.0)?;
    round.complete_clock_horizon(next_days, definitions.engine_nav.is_some())?;
    // A full supported transaction has bounded command/provider/actor domains.
    // Preserve a conservative 2^40 increment margin for unchecked public/event
    // counters, independently of wrapping/refusing semantic allocators.
    const WORLD_COUNTER_MARGIN: i64 = 1i64 << 40;
    check(
        backbone.complete_revision() <= i64::MAX - WORLD_COUNTER_MARGIN
            && backbone.complete_event_sequence() <= i64::MAX - WORLD_COUNTER_MARGIN,
        "complete World counter headroom",
    )?;
    check(
        host.scalars().boundary.physical_sequence < i64::MAX as u64 - 2,
        "complete spatial sample headroom",
    )?;
    Ok(())
}

impl Engine {
    // Intentionally exhaustive: adding an owner requires classification here.
    fn complete_field_inventory(&self) {
        let Engine {
            checkpoint_seed_identity: _,
            world: _,
            transcript: _,
            scheduler: _,
            floor: _,
            speech_router: _,
            env: _,
            cognition: _,
            transcription: _,
            tts: _,
            sight: _,
            config: _,
            capabilities: _,
            tts_selected: _,
            last_snapshot_revision: _,
            last_player_sound_at: _,
            conversation: _,
            npc_exchanges: _,
            novelty: _,
            clock: _,
            weather: _,
            last_weather_days: _,
            last_weather_sample: _,
            last_clock_days: _,
            bell_strokes: _,
            bell_seq: _,
            movement_now: _,
            round: _,
            night: _,
            next_round_tick_at: _,
            next_stage_hop_at: _,
            next_player_pollen_game_days: _,
            lamp_revision_sent: _,
            dogs_published: _,
            startup_diagnostics: _,
            ready_emitted: _,
            last_law_standing: _,
            last_chalk_standing: _,
            last_ward_heat: _,
            door_shut_until: _,
            last_journal: _,
            last_journal_receipts: _,
            last_journal_at: _,
        } = self;
        let EngineConfig {
            checkpoint_host_image: _,
            checkpoint_world_identity: _,
            runtime_generation: _,
            operations: _,
            player_id: _,
            fake_mode: _,
            sounds_enabled: _,
            view_cone_degrees: _,
            sound_cooldown_seconds: _,
            turn_delay_seconds: _,
            maximum_backoff_seconds: _,
            tts_selected: _,
            tts_startup_message: _,
            stt_stream_grace_seconds: _,
            runtime_dir: _,
            idle_mode: _,
            stage: _,
            idle_requires_news: _,
            idle_curiosity: _,
            clock: _,
            ring_the_offices: _,
            weather: _,
            shelters: _,
            nav: _,
            night_office: _,
            marks_enabled: _,
            mark_kinds: _,
            marks_decay_scale: _,
            knowledge_enabled: _,
            fact_packs: _,
            pollen_flat: _,
            pollen_no_salience: _,
        } = &self.config;
    }
    /// Actual session transcript capacity. This presentation artifact is outside
    /// candidate authority and must still be owned by the host Running scope.
    pub fn checkpoint_transcript_storage_bytes(&self) -> usize {
        self.transcript.capacity() * std::mem::size_of::<String>()
            + 32
            + self
                .transcript
                .iter()
                .map(|s| s.capacity() + 32)
                .sum::<usize>()
    }
}
