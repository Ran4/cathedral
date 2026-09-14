//! Read-only observation of completed ordinary host boundaries. No polling,
//! input consumption, installation, device work or publication occurs here.
#![allow(dead_code)] // M2 component seam; M3 installs the production coordinator.
use crate::{
    controller::{CollisionWorld, DynamicBarrier, PhysicalPosition, PlayerController},
    live_time::LiveTime,
    smart_actors::{
        self,
        bridge::{BridgeHandle, BridgeInbox},
        hud::SmartActorHudState,
        interaction::{InteractionState, PlayerSpatialState},
        local_engine::LocalEngine,
        speech::{SpeechBubble, SpeechBubbleStack, SpeechPresentationState},
    },
    soundscape,
};
use bevy::prelude::*;
use cathedral_sim::checkpoint::{Result, host::*};
mod records;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HostCaptureSet;

pub(crate) struct HostObservation<'a> {
    world: &'a World,
}
impl<'a> HostObservation<'a> {
    pub(crate) fn new(world: &'a World) -> Result<Self> {
        Ok(Self { world })
    }
    fn resource<T: Resource>(&self) -> Result<&'a T> {
        self.world.get_resource::<T>().ok_or_else(|| {
            error(format!(
                "missing host resource {}",
                std::any::type_name::<T>()
            ))
        })
    }
    fn component<T: Component>(&self) -> Result<&'a T> {
        let mut found = None;
        for entity in self.world.iter_entities() {
            if let Some(v) = entity.get::<T>() {
                if found.is_some() {
                    return Err(error("duplicate singleton host component"));
                }
                found = Some(v);
            }
        }
        found.ok_or_else(|| {
            error(format!(
                "missing host component {}",
                std::any::type_name::<T>()
            ))
        })
    }
    fn optional_component<T: Component>(&self) -> Result<Option<&'a T>> {
        let mut found = None;
        for entity in self.world.iter_entities() {
            if let Some(v) = entity.get::<T>() {
                if found.is_some() {
                    return Err(error("duplicate singleton host component"));
                }
                found = Some(v);
            }
        }
        Ok(found)
    }
    fn physical_owner(&self) -> Result<(&'a PlayerController, &'a PhysicalPosition)> {
        let mut found = None;
        for entity in self.world.iter_entities() {
            match (
                entity.get::<PlayerController>(),
                entity.get::<PhysicalPosition>(),
            ) {
                (None, None) => {}
                (Some(c), Some(p)) => {
                    if found.replace((c, p)).is_some() {
                        return Err(error("duplicate physical host owner"));
                    }
                }
                _ => return Err(error("controller/body belong to different owners")),
            }
        }
        found.ok_or_else(|| error("missing physical host owner"))
    }
    fn local(&self) -> Result<&'a LocalEngine> {
        self.world
            .get_non_send::<LocalEngine>()
            .ok_or_else(|| error("missing local engine"))
    }
    pub(crate) fn context(&self) -> Result<HostCheckpointContext<'a>> {
        let local = self.local()?;
        let b = local
            .accepted_boundary
            .ok_or_else(|| error("no completed physical boundary"))?;
        Ok(HostCheckpointContext::from_engine(
            local.checkpoint_engine()?,
            self.resource::<LiveTime>()?.continuation.elapsed,
            self.definitions()?,
            b.input_watermark,
            self.resource::<BridgeHandle>()?.checkpoint_issued()?,
        ))
    }
    fn definitions(&self) -> Result<DefinitionsV1> {
        let mut collision = DefinitionHasher::new(b"host-collision-v1");
        let c = self.resource::<CollisionWorld>()?;
        if let Some(p) = self.world.get_resource::<crate::city::CutMarginProfile>() {
            collision.bytes(&[1]);
            p.checkpoint_definition(&mut collision);
        } else {
            collision.bytes(&[0]);
        }
        collision.bytes(&(c.boxes.len() as u64).to_le_bytes());
        for b in &c.boxes {
            hash_vec(&mut collision, b.min);
            hash_vec(&mut collision, b.max);
        }
        collision.bytes(&(c.convex_prisms.len() as u64).to_le_bytes());
        for p in &c.convex_prisms {
            hash_f32(&mut collision, p.min_y);
            hash_f32(&mut collision, p.max_y);
            collision.bytes(&(p.planes.len() as u64).to_le_bytes());
            for plane in &p.planes {
                hash_f32(&mut collision, plane.normal.x);
                hash_f32(&mut collision, plane.normal.y);
                hash_f32(&mut collision, plane.offset);
            }
            collision.bytes(&(p.footprint.len() as u64).to_le_bytes());
            for p in &p.footprint {
                hash_f32(&mut collision, p.x);
                hash_f32(&mut collision, p.y);
            }
        }
        let mut barriers = DefinitionHasher::new(b"host-barriers-v1");
        for e in self.world.iter_entities() {
            if let Some(b) = e.get::<DynamicBarrier>() {
                let t = e
                    .get::<Transform>()
                    .ok_or_else(|| error("barrier lacks transform"))?;
                let gate = e
                    .get::<crate::city::gates::GateBarrier>()
                    .ok_or_else(|| error("unsupported unowned dynamic barrier"))?;
                let runtime = self.resource::<crate::city::gates::GateRuntime>()?;
                if b.active != runtime.blocks(gate.0) {
                    return Err(error("gate barrier publication disagreement"));
                }
                barriers.bytes(&[match gate.0 {
                    crate::city::gates::GateKind::Stone => 0,
                    crate::city::gates::GateKind::River => 1,
                }]);
                hash_vec(&mut barriers, b.half_size);
                hash_vec(&mut barriers, t.translation);
            }
        }
        let mut vermin = DefinitionHasher::new(b"host-vermin-installed-v1");
        if let Some(v) = self.optional_component::<crate::city::vermin::Vermin>()? {
            vermin.bytes(&v.nav.checkpoint_fingerprint());
            vermin.bytes(&v.seed.to_le_bytes());
            vermin.bytes(&[v.swarm_percepts as u8]);
            hash_f32(&mut vermin, v.density);
            vermin.bytes(&(v.colonies.len() as u64).to_le_bytes());
            for c in &v.colonies {
                vermin.bytes(c.name.as_bytes());
                hash_f32(&mut vermin, c.anchor.x);
                hash_f32(&mut vermin, c.anchor.y);
                hash_f32(&mut vermin, c.radius_m);
                vermin.bytes(&[c.all_offices as u8]);
                for rats in [&c.rats, &c.boil_rats] {
                    vermin.bytes(&(rats.len() as u64).to_le_bytes());
                    for r in rats {
                        vermin.bytes(&r.seed.to_le_bytes());
                        for f in [r.period, r.phase, r.length_m, r.tint] {
                            hash_f32(&mut vermin, f);
                        }
                        vermin.bytes(&(r.legs.len() as u64).to_le_bytes());
                        for l in &r.legs {
                            for f in [
                                l.depart,
                                l.arrive,
                                l.from.x,
                                l.from.y,
                                l.to.x,
                                l.to.y,
                                l.heading.x,
                                l.heading.y,
                            ] {
                                hash_f32(&mut vermin, f);
                            }
                        }
                    }
                }
            }
        }
        let mut catalogs = DefinitionHasher::new(b"host-installed-algorithms-v1");
        catalogs.bytes(include_bytes!("controller.rs"));
        // These unchanged solver constants are now shared with the pure
        // validator, so their installed values are outside controller.rs.
        for value in [
            controller_limits::RUN_SPEED,
            controller_limits::MAX_FLY_SPEED,
            controller_limits::GRAVITY,
            controller_limits::COYOTE_SECONDS,
            controller_limits::JUMP_BUFFER_SECONDS,
            controller_limits::PITCH_LIMIT,
        ] {
            hash_f32(&mut catalogs, value);
        }
        catalogs.bytes(include_bytes!("city/gates.rs"));
        catalogs.bytes(include_bytes!("city/vermin.rs"));
        catalogs.bytes(include_bytes!("soundscape.rs"));
        catalogs.bytes(include_bytes!("../assets/world/marks.json"));
        catalogs.bytes(include_bytes!("city/mod.rs"));
        if let Some(mark_catalog) = self
            .world
            .get_resource::<crate::city::marks::MarkCatalogRes>()
        {
            catalogs.bytes(&[1]);
            for (kind, spec) in mark_catalog.0.kinds() {
                catalogs.bytes(&[match kind {
                    cathedral_sim::marks::MarkKind::ChalkCross => 0,
                    cathedral_sim::marks::MarkKind::WellTally => 1,
                    cathedral_sim::marks::MarkKind::WardSign => 2,
                }]);
                for text in [&spec.label, &spec.meaning, &spec.faint_label] {
                    catalogs.bytes(text.as_bytes());
                }
                catalogs.bytes(&(spec.anchors.len() as u64).to_le_bytes());
                for slot in &spec.anchors {
                    catalogs.bytes(&[match slot {
                        cathedral_sim::marks::AnchorSlot::Household => 0,
                        cathedral_sim::marks::AnchorSlot::Place => 1,
                    }]);
                }
                for value in [
                    spec.half_life_days_dry,
                    spec.half_life_days_wet,
                    spec.sheltered_multiplier,
                    spec.faint_below,
                    spec.gone_below,
                ] {
                    catalogs.bytes(&value.to_bits().to_le_bytes());
                }
                catalogs.bytes(&[spec.drawable_by_hand as u8]);
                catalogs.bytes(&(spec.places.len() as u64).to_le_bytes());
                for (key, value) in &spec.places {
                    catalogs.bytes(key.as_bytes());
                    catalogs.bytes(value.as_bytes());
                }
            }
        } else {
            catalogs.bytes(&[0]);
        }
        Ok(DefinitionsV1 {
            collision: collision.finish(),
            barriers: barriers.finish(),
            vermin: vermin.finish(),
            installed_catalogs: catalogs.finish(),
            gates_present: self
                .world
                .contains_resource::<crate::city::gates::GateRuntime>(),
            vermin_installed: Nullable(
                self.optional_component::<crate::city::vermin::Vermin>()?
                    .map(|v| VerminDefinitionV1 {
                        seed: v.seed,
                        swarm_percepts: v.swarm_percepts,
                        density_bits: v.density.to_bits(),
                    }),
            ),
        })
    }
}
fn hash_f32(h: &mut DefinitionHasher, v: f32) {
    h.bytes(&v.to_bits().to_le_bytes());
}
fn hash_vec(h: &mut DefinitionHasher, v: Vec3) {
    for f in v.to_array() {
        hash_f32(h, f);
    }
}
impl HostCheckpointSource for HostObservation<'_> {
    fn scalars(&self) -> Result<ScalarsV1> {
        let (c, p) = self.physical_owner()?;
        let live = self.resource::<LiveTime>()?;
        let fixed = self.resource::<Time<Fixed>>()?;
        let local = self.local()?;
        let b = local
            .accepted_boundary
            .ok_or_else(|| error("no completed physical boundary"))?;
        let i = self.resource::<InteractionState>()?;
        let spatial = self.resource::<PlayerSpatialState>()?;
        let speech = self.resource::<SpeechPresentationState>()?;
        let clock = self.resource::<smart_actors::WorldClockState>()?;
        let chat = self.resource::<smart_actors::ChatInputState>()?;
        let journal = self.resource::<smart_actors::JournalUiState>()?;
        let inventory = self.resource::<smart_actors::InventoryUiState>()?;
        let law = self.resource::<smart_actors::custody::PlayerCustodyState>()?;
        let hold = self.resource::<crate::city::marks::ChalkHold>()?;
        let choice = self.resource::<crate::city::marks::ChalkChoice>()?;
        let cooldowns = self.resource::<soundscape::CueCooldowns>()?;
        let wells = self.resource::<soundscape::WellSoundState>()?;
        let civic = self.resource::<soundscape::CivicBellState>()?;
        let sounds_clock = self.resource::<soundscape::ClockSoundState>()?;
        let journal_scroll = if journal.open {
            self.world
                .iter_entities()
                .filter(|e| e.contains::<smart_actors::journal_ui::JournalEntriesRoot>())
                .filter_map(|e| e.get::<ScrollPosition>())
                .next()
                .map(|p| [p.x, p.y])
                .ok_or_else(|| error("open journal has no scroll owner"))?
        } else {
            [0.0; 2]
        };
        let gates = self
            .world
            .get_resource::<crate::city::gates::GateRuntime>()
            .map(|g| g.checkpoint_scalar());
        let vermin = self
            .optional_component::<crate::city::vermin::Vermin>()?
            .map(|v| v.checkpoint_scalar());
        Ok(ScalarsV1 {
            controller: ControllerV1 {
                flying: c.flying,
                velocity: c.velocity.to_array(),
                yaw: c.yaw,
                pitch: c.pitch,
                grounded: c.grounded,
                coyote_remaining: c.coyote_remaining,
                jump_buffer_remaining: c.jump_buffer_remaining,
                previous: p.previous.to_array(),
                current: p.current.to_array(),
            },
            time: TimeV1 {
                accepted: cathedral_sim::checkpoint::HostTimeV1::from_accepted(
                    live.continuation,
                    fixed.timestep(),
                    fixed.overstep(),
                    self.context()?.movement_residual(),
                )?,
                virtual_elapsed: live.accepted_virtual.elapsed(),
                virtual_delta: live.accepted_virtual.delta(),
                fixed_elapsed: fixed.elapsed(),
                fixed_delta: fixed.delta(),
                last_frame: Nullable(live.last_frame.map(|f| FrameV1 {
                    wall_delta: f.wall_delta,
                    accepted_delta: f.accepted_delta,
                    elapsed: f.elapsed,
                    debt: f.debt,
                })),
            },
            boundary: BoundaryV1 {
                generation: b.generation.0,
                input_watermark: b.input_watermark,
                physical_sequence: b.physical_sequence,
                issued: self.resource::<BridgeHandle>()?.checkpoint_issued()?,
                message_sequence: self
                    .resource::<smart_actors::SmartActorRuntime>()?
                    .message_sequence,
                speech_read: speech.speech_read as u64,
                cue_read: cooldowns.cue_read as u64,
                intent_read: i.intent_read as u64,
                last_speech_sequence: Nullable(speech.last_event_seq),
            },
            spatial: SpatialV1 {
                sequence: spatial.sequence,
                last_position: Nullable(spatial.last_position.map(|p| p.to_array())),
                last_yaw: Nullable(spatial.last_yaw),
                last_background_send: cathedral_sim::checkpoint::LogicalAnchorV1::from_legacy(
                    spatial.last_background_send,
                )?,
            },
            clock: ClockV1 {
                present: clock.present,
                day: clock.day,
                fraction: clock.fraction,
                office: clock.office.into(),
                weekday: clock.weekday.into(),
                brightness: clock.brightness,
                scale: clock.scale,
                seconds_per_day: clock.seconds_per_day,
            },
            gates: Nullable(gates),
            vermin: Nullable(vermin),
            soundscape: SoundscapeV1 {
                last_pruned_at: cooldowns.last_pruned_at,
                observed_office: Nullable(civic.observed_office.map(Into::into)),
                curfew_day: Nullable(civic.curfew_day),
                flour_day: Nullable(sounds_clock.flour_day),
                ford_until: wells.ford_until,
                chain_until: wells.chain_until,
                three_curb_until: wells.three_curb_until,
                three_curb_paused_from: wells.three_curb_paused_from,
                three_curb_paused_until: wells.three_curb_paused_until,
                crossed_bucket_day: Nullable(wells.crossed_bucket_day),
            },
            ui: UiV1 {
                chat_open: chat.open,
                chat_cursor: chat.cursor,
                journal_open: journal.open,
                journal_scroll,
                inventory_open: inventory.open,
                map_open: self.resource::<crate::map::MapState>()?.fullscreen_open,
                selected_index: i.selected_index,
                coin_offer_count: i.coin_offer_count,
                next_request: i.next_request,
                chalk_progress: hold.progress,
                chalk_choice: choice.step,
                strain: law.strain,
                struggling_reported: law.struggling_reported,
            },
            definitions: self.definitions()?,
        })
    }
    fn records(&self, visitor: &mut dyn FnMut(RecordRef<'_>) -> Result<()>) -> Result<()> {
        records::visit(self, visitor)
    }
    fn validate_boundary(&self) -> Result<()> {
        let local = self.local()?;
        let b = local
            .accepted_boundary
            .ok_or_else(|| error("no completed physical boundary"))?;
        let handle = self.resource::<BridgeHandle>()?;
        let runtime = self.resource::<smart_actors::SmartActorRuntime>()?;
        if !runtime.ready
            || !runtime.connected
            || self.resource::<BridgeInbox>()?.len() != 0
            || b.generation != handle.generation()
            || local.issued_at_boundary != Some(handle.checkpoint_issued()?)
        {
            return Err(error("incomplete host consumer boundary"));
        }
        let (c, p) = self.physical_owner()?;
        if p.current.to_array().map(f32::to_bits) != b.position.to_array().map(f32::to_bits)
            || c.yaw.to_bits() != b.yaw.to_bits()
            || self
                .resource::<LiveTime>()?
                .continuation
                .elapsed
                .as_secs_f64()
                .to_bits()
                != b.elapsed.seconds().to_bits()
        {
            return Err(error("stale accepted physical boundary"));
        }
        let accepted = self.resource::<LiveTime>()?.continuation.elapsed;
        if self.resource::<Time>()?.elapsed() != accepted
            || self.resource::<Time<Virtual>>()?.elapsed() != accepted
        {
            return Err(error("published Bevy clock disagreement"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_public;
