//! Remaining Engine cadence/publication/configuration authority. Read-only
//! candidate composition; no Engine::new, polling, service work or adoption.
//! Full loading must transform old floor holds with speech obligations and
//! readable presentation, and resolve the separate player transcript policy.
use super::*;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate, records::text},
    floor::checkpoint::{self as owner, records::future},
    scheduler::checkpoint::records::raw_float,
    timeline::LogicalTime,
    world::checkpoint::BackboneCandidate,
};
pub use owner::FloorCost as ContinuityCost;
use serde::{Deserialize, Serialize};
pub const MAX_STARTUP_DIAGNOSTICS: usize = 25_000;
pub const MAX_TEXT_BYTES: usize = 65_536;
#[derive(Clone, Copy)]
pub struct EngineContinuityCheckpointContext<'a> {
    now: LogicalTime,
    player: &'a ActorId,
    characters: &'a BTreeMap<ActorId, Character>,
}
impl<'a> EngineContinuityCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime, player: &'a ActorId) -> Self {
        Self {
            now,
            player,
            characters: &w.characters,
        }
    }
    pub fn from_backbone(b: &'a BackboneCandidate, now: LogicalTime, player: &'a ActorId) -> Self {
        Self {
            now,
            player,
            characters: b.references().characters,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    fake_mode: bool,
    sounds_enabled: bool,
    #[serde(with = "raw_float")]
    view_cone_degrees: f64,
    #[serde(with = "raw_float")]
    sound_cooldown_seconds: f64,
    tts_selected: TtsBackendKind,
    #[serde(with = "text::option")]
    tts_startup_message: Option<String>,
    #[serde(with = "raw_float")]
    stt_stream_grace_seconds: f64,
}
#[derive(Serialize)]
struct ConfigView<'a> {
    fake_mode: bool,
    sounds_enabled: bool,
    #[serde(with = "raw_float")]
    view_cone_degrees: f64,
    #[serde(with = "raw_float")]
    sound_cooldown_seconds: f64,
    tts_selected: TtsBackendKind,
    tts_startup_message: &'a Option<String>,
    #[serde(with = "raw_float")]
    stt_stream_grace_seconds: f64,
}
impl<'a> ConfigView<'a> {
    fn new(c: &'a EngineConfig) -> Self {
        Self {
            fake_mode: c.fake_mode,
            sounds_enabled: c.sounds_enabled,
            view_cone_degrees: c.view_cone_degrees,
            sound_cooldown_seconds: c.sound_cooldown_seconds,
            tts_selected: c.tts_selected,
            tts_startup_message: &c.tts_startup_message,
            stt_stream_grace_seconds: c.stt_stream_grace_seconds,
        }
    }
    fn copy(&self) -> Config {
        Config {
            fake_mode: self.fake_mode,
            sounds_enabled: self.sounds_enabled,
            view_cone_degrees: self.view_cone_degrees,
            sound_cooldown_seconds: self.sound_cooldown_seconds,
            tts_selected: self.tts_selected,
            tts_startup_message: self.tts_startup_message.clone(),
            stt_stream_grace_seconds: self.stt_stream_grace_seconds,
        }
    }
}
mod last_sound {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", deny_unknown_fields)]
    enum Past {
        Never,
        At(f64),
    }
    pub fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        if *v == f64::NEG_INFINITY {
            Past::Never.serialize(s)
        } else if v.is_finite() && *v >= 0.0 {
            Past::At(*v).serialize(s)
        } else {
            Err(serde::ser::Error::custom(
                "invalid last player sound anchor",
            ))
        }
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<f64, D::Error> {
        match Past::deserialize(d)? {
            Past::Never => Ok(f64::NEG_INFINITY),
            Past::At(x) if x.is_finite() && x >= 0.0 => Ok(x),
            _ => Err(serde::de::Error::custom("invalid last player sound anchor")),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct EngineContinuityDtoV1 {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    #[serde(with = "owner::records::FloorV1")]
    floor: ConversationFloor,
    last_snapshot_revision: i64,
    #[serde(with = "last_sound")]
    last_player_sound_at: f64,
    #[serde(with = "future")]
    next_round_tick_at: f64,
    lamp_revision_sent: u64,
    #[serde(with = "text::vec")]
    startup_diagnostics: Vec<String>,
    ready_emitted: bool,
    tts_selected: TtsBackendKind,
    config: Config,
}
#[derive(Deserialize)]
#[serde(remote = "EngineContinuityDtoV1", deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    #[serde(with = "owner::records::FloorV1")]
    floor: ConversationFloor,
    last_snapshot_revision: i64,
    #[serde(with = "last_sound")]
    last_player_sound_at: f64,
    #[serde(with = "future")]
    next_round_tick_at: f64,
    lamp_revision_sent: u64,
    #[serde(with = "text::vec")]
    startup_diagnostics: Vec<String>,
    ready_emitted: bool,
    tts_selected: TtsBackendKind,
    config: Config,
}
#[derive(Deserialize)]
struct Decoded(#[serde(with = "Wire")] EngineContinuityDtoV1);
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    player_id: &'a ActorId,
    #[serde(with = "owner::records::FloorV1")]
    floor: &'a ConversationFloor,
    last_snapshot_revision: i64,
    #[serde(with = "last_sound")]
    last_player_sound_at: f64,
    #[serde(with = "future")]
    next_round_tick_at: f64,
    lamp_revision_sent: u64,
    startup_diagnostics: &'a [String],
    ready_emitted: bool,
    tts_selected: TtsBackendKind,
    config: ConfigView<'a>,
}
impl<'a> View<'a> {
    fn new(e: &'a Engine, now: LogicalTime) -> Self {
        Self {
            version: 1,
            boundary: now,
            player_id: &e.config.player_id,
            floor: &e.floor,
            last_snapshot_revision: e.last_snapshot_revision,
            last_player_sound_at: e.last_player_sound_at,
            next_round_tick_at: e.next_round_tick_at,
            lamp_revision_sent: e.lamp_revision_sent,
            startup_diagnostics: &e.startup_diagnostics,
            ready_emitted: e.ready_emitted,
            tts_selected: e.tts_selected,
            config: ConfigView::new(&e.config),
        }
    }
    fn validate(&self, c: EngineContinuityCheckpointContext<'_>) -> Result<()> {
        validate_binding(self.boundary, self.player_id, c)?;
        owner::validate(self.floor)?;
        validate_config(
            self.config.sound_cooldown_seconds,
            self.config.stt_stream_grace_seconds,
            self.config.tts_startup_message,
            self.startup_diagnostics,
        )?;
        owner::future(self.next_round_tick_at)?;
        owner::check(
            self.last_player_sound_at == f64::NEG_INFINITY
                || (self.last_player_sound_at.is_finite() && self.last_player_sound_at >= 0.0),
            "invalid last player sound anchor",
        )
    }
}
fn validate_binding(
    boundary: LogicalTime,
    player: &ActorId,
    c: EngineContinuityCheckpointContext<'_>,
) -> Result<()> {
    owner::binding(boundary, c.now)?;
    owner::check(
        player.as_str().len() <= 4 * crate::MAX_ID_CHARS && player.is_valid(),
        "invalid continuity player id",
    )?;
    owner::check(
        player == c.player && c.characters.contains_key(player),
        "continuity player binding disagreement",
    )
}
fn validate_config(
    cooldown: f64,
    grace: f64,
    message: &Option<String>,
    diagnostics: &[String],
) -> Result<()> {
    // These are stored constructor outputs. clamp preserves NaN and signed
    // zero; max floors NaN grace but permits +infinity. Do not normalize again.
    owner::check(
        cooldown.is_nan() || (0.0..=MAX_SOUND_COOLDOWN_SECONDS).contains(&cooldown),
        "stored sound cooldown outside constructor range",
    )?;
    owner::check(
        grace >= MIN_STT_STREAM_GRACE_SECONDS,
        "stored STT grace outside constructor range",
    )?;
    owner::check(
        diagnostics.len() <= MAX_STARTUP_DIAGNOSTICS,
        "startup diagnostic count limit",
    )?;
    owner::check(
        diagnostics
            .iter()
            .chain(message.iter())
            .all(|s| s.len() <= MAX_TEXT_BYTES),
        "continuity text byte limit",
    )
}
#[derive(Debug)]
pub struct EngineContinuityCandidate {
    data: EngineContinuityDtoV1,
}
impl Engine {
    pub fn continuity_checkpoint_context(
        &self,
        now: LogicalTime,
    ) -> EngineContinuityCheckpointContext<'_> {
        EngineContinuityCheckpointContext::from_world(&self.world, now, &self.config.player_id)
    }
    pub fn checkpoint_continuity_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<ContinuityCost>> {
        let cost = owner::prepare(&View::new(self, now), &mut r)?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_continuity_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineContinuityDtoV1>> {
        let v = View::new(self, now);
        owner::prepare(&v, &mut r)?;
        v.validate(self.continuity_checkpoint_context(now))?;
        Ok(Admitted::new(
            EngineContinuityDtoV1 {
                version: 1,
                boundary: now,
                player_id: self.config.player_id.clone(),
                floor: owner::copy(&self.floor),
                last_snapshot_revision: self.last_snapshot_revision,
                last_player_sound_at: self.last_player_sound_at,
                next_round_tick_at: self.next_round_tick_at,
                lamp_revision_sent: self.lamp_revision_sent,
                startup_diagnostics: self.startup_diagnostics.clone(),
                ready_emitted: self.ready_emitted,
                tts_selected: self.tts_selected,
                config: v.config.copy(),
            },
            r,
        ))
    }
}
impl EngineContinuityDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: EngineContinuityCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let Decoded(d) = aggregate::decode_with_working(
            bytes,
            owner::OWNER,
            &mut r,
            owner::VALIDATION_WORKING_BYTES,
        )?;
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: EngineContinuityCheckpointContext<'_>) -> Result<()> {
        owner::check(self.version == 1, "unsupported Engine continuity version")?;
        validate_binding(self.boundary, &self.player_id, c)?;
        owner::validate(&self.floor)?;
        validate_config(
            self.config.sound_cooldown_seconds,
            self.config.stt_stream_grace_seconds,
            &self.config.tts_startup_message,
            &self.startup_diagnostics,
        )?;
        owner::future(self.next_round_tick_at)?;
        owner::check(
            self.last_player_sound_at == f64::NEG_INFINITY
                || (self.last_player_sound_at.is_finite() && self.last_player_sound_at >= 0.0),
            "invalid last player sound anchor",
        )
    }
    pub fn cost(&self) -> Result<ContinuityCost> {
        Ok(aggregate::measure(self, owner::OWNER)?.into())
    }
    pub fn counts(&self, c: EngineContinuityCheckpointContext<'_>) -> ContinuityCounts {
        ContinuityCounts {
            characters: c.characters.len(),
            floor: owner::counts(&self.floor),
            startup_diagnostics: self.startup_diagnostics.len(),
            startup_diagnostic_bytes: self.startup_diagnostics.iter().map(String::len).sum(),
            startup_message_bytes: self
                .config
                .tts_startup_message
                .as_ref()
                .map_or(0, String::len),
            ready_emitted: self.ready_emitted,
            sound_ever_emitted: self.last_player_sound_at != f64::NEG_INFINITY,
            lamp_revision_sent: self.lamp_revision_sent,
            last_snapshot_revision: self.last_snapshot_revision,
            tts_selected: self.tts_selected,
            configured_tts_selected: self.config.tts_selected,
        }
    }
}
impl Admitted<EngineContinuityDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, owner::OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: EngineContinuityCheckpointContext<'_>,
    ) -> Result<Admitted<EngineContinuityCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineContinuityCandidate { data: d })
        })
    }
}
impl EngineContinuityCandidate {
    pub fn floor(&self) -> &ConversationFloor {
        &self.data.floor
    }
    pub fn player_id(&self) -> &ActorId {
        &self.data.player_id
    }
    pub fn last_snapshot_revision(&self) -> i64 {
        self.data.last_snapshot_revision
    }
    pub fn last_player_sound_at(&self) -> f64 {
        self.data.last_player_sound_at
    }
    pub fn next_round_tick_at(&self) -> f64 {
        self.data.next_round_tick_at
    }
    pub fn lamp_revision_sent(&self) -> u64 {
        self.data.lamp_revision_sent
    }
    pub fn startup_diagnostics(&self) -> &[String] {
        &self.data.startup_diagnostics
    }
    pub fn ready_emitted(&self) -> bool {
        self.data.ready_emitted
    }
    pub fn tts_selected(&self) -> TtsBackendKind {
        self.data.tts_selected
    }
    pub fn configured_tts_selected(&self) -> TtsBackendKind {
        self.data.config.tts_selected
    }
    pub fn fake_mode(&self) -> bool {
        self.data.config.fake_mode
    }
    pub fn sounds_enabled(&self) -> bool {
        self.data.config.sounds_enabled
    }
    pub fn view_cone_degrees(&self) -> f64 {
        self.data.config.view_cone_degrees
    }
    pub fn sound_cooldown_seconds(&self) -> f64 {
        self.data.config.sound_cooldown_seconds
    }
    pub fn stt_stream_grace_seconds(&self) -> f64 {
        self.data.config.stt_stream_grace_seconds
    }
    pub fn tts_startup_message(&self) -> Option<&str> {
        self.data.config.tts_startup_message.as_deref()
    }
    pub fn counts(&self, c: EngineContinuityCheckpointContext<'_>) -> ContinuityCounts {
        self.data.counts(c)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ContinuityCounts {
    pub characters: usize,
    pub floor: owner::FloorCounts,
    pub startup_diagnostics: usize,
    pub startup_diagnostic_bytes: usize,
    pub startup_message_bytes: usize,
    pub ready_emitted: bool,
    pub sound_ever_emitted: bool,
    pub lamp_revision_sent: u64,
    pub last_snapshot_revision: i64,
    pub tts_selected: TtsBackendKind,
    pub configured_tts_selected: TtsBackendKind,
}
#[cfg(test)]
mod tests;
