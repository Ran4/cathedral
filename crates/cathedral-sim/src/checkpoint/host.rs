//! Closed, read-only host continuation records. No Bevy, IO or adoption.
//! The host supplies borrowed observations; measurement precedes collection.
use super::{Admitted, CheckpointError, Cohort, ComponentCost, Reservation, Result, aggregate};
mod ordering;
mod publication;
mod wire;
pub use publication::resolved_journal_row;
use publication::validate_publications;
use serde::{Deserialize, Serialize, ser::SerializeSeq};
use std::io::Write;
use std::time::Duration;
pub use wire::{BodySlotV1, HudSlot, MarkKindV1, OfficeV1, RungV1, WeekdayV1, WellKind, WorkKind};

const OWNER: &str = "host";
pub(crate) mod context;
pub use context::HostCheckpointContext;
pub const MAX_PRESENTATIONS: usize = 512;
pub const MAX_UNREAD_PUBLICATIONS: usize = 8192;
pub const MAX_TEXT_BYTES: usize = 4 * super::records::MAX_TEXT_BYTES + 1024;
/// Largest enum rows and sorting/index overhead are charged independently of
/// the lexical bound: an empty short variant still occupies the full stride.
pub const ROW_WORKING_BYTES: usize = 4096;
pub const VALIDATION_WORKING_BYTES: usize = 4 * 1024 * 1024;
/// Shared with the actual controller setters/solver, so admission uses the
/// same supported motion vocabulary without depending on Bevy.
pub mod controller_limits {
    pub const RUN_SPEED: f32 = 12.0;
    pub const MAX_FLY_SPEED: f32 = 11.0;
    pub const GRAVITY: f32 = 22.0;
    pub const COYOTE_SECONDS: f32 = 0.10;
    pub const JUMP_BUFFER_SECONDS: f32 = 0.12;
    pub const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;
}
pub fn player_delivery_caption(text: &str, recipients: usize) -> String {
    let delivery = match recipients {
        0 => "nobody nearby".to_owned(),
        1 => "heard by 1 nearby person".to_owned(),
        n => format!("heard by {n} nearby people"),
    };
    format!("You: {}  ·  {delivery}", text.trim())
}
pub fn speech_reading_seconds(text: &str) -> f32 {
    (2.0 + text.chars().count() as f32 / 15.0).clamp(3.0, 10.0)
}
pub struct DefinitionHasher(sha2::Sha256);
impl DefinitionHasher {
    pub fn new(domain: &[u8]) -> Self {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(domain);
        Self(h)
    }
    pub fn bytes(&mut self, value: &[u8]) {
        use sha2::Digest;
        self.0.update((value.len() as u64).to_le_bytes());
        self.0.update(value);
    }
    pub fn finish(self) -> [u8; 32] {
        use sha2::Digest;
        self.0.finalize().into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Nullable<T>(pub Option<T>);
impl<T> Default for Nullable<T> {
    fn default() -> Self {
        Self(None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerV1 {
    pub flying: bool,
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
    pub coyote_remaining: f32,
    pub jump_buffer_remaining: f32,
    pub previous: [f32; 3],
    pub current: [f32; 3],
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameV1 {
    #[serde(with = "wire::duration")]
    pub wall_delta: Duration,
    #[serde(with = "wire::duration")]
    pub accepted_delta: Duration,
    #[serde(with = "wire::duration")]
    pub elapsed: Duration,
    #[serde(with = "wire::duration")]
    pub debt: Duration,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeV1 {
    pub accepted: super::HostTimeV1,
    #[serde(with = "wire::duration")]
    pub virtual_elapsed: Duration,
    #[serde(with = "wire::duration")]
    pub virtual_delta: Duration,
    #[serde(with = "wire::duration")]
    pub fixed_elapsed: Duration,
    #[serde(with = "wire::duration")]
    pub fixed_delta: Duration,
    pub last_frame: Nullable<FrameV1>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryV1 {
    pub generation: u64,
    pub input_watermark: u64,
    pub physical_sequence: u64,
    pub issued: u64,
    pub message_sequence: u64,
    pub speech_read: u64,
    pub cue_read: u64,
    pub intent_read: u64,
    pub last_speech_sequence: Nullable<u64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialV1 {
    pub sequence: u64,
    pub last_position: Nullable<[f32; 3]>,
    pub last_yaw: Nullable<f32>,
    #[serde(with = "wire::logical_anchor")]
    pub last_background_send: super::LogicalAnchorV1,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockV1 {
    pub present: bool,
    pub day: i64,
    pub fraction: f64,
    pub office: OfficeV1,
    pub weekday: WeekdayV1,
    pub brightness: f64,
    pub scale: f64,
    pub seconds_per_day: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionV1 {
    pub value: f32,
    pub start: f32,
    pub target: f32,
    pub elapsed: f32,
    pub duration: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatesV1 {
    pub initialized: bool,
    pub closed: bool,
    pub previous: Nullable<(f64, bool)>,
    pub stone_leaves: MotionV1,
    pub river_leaves: MotionV1,
    pub river_bar: MotionV1,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerminV1 {
    pub seed: u64,
    pub swarm_percepts: bool,
    pub density: f32,
    pub announced_boil_night: Nullable<i64>,
    pub last_percept_minutes: Nullable<f64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundscapeV1 {
    pub last_pruned_at: f64,
    pub observed_office: Nullable<OfficeV1>,
    pub curfew_day: Nullable<i64>,
    pub flour_day: Nullable<i64>,
    pub ford_until: f64,
    pub chain_until: f64,
    pub three_curb_until: f64,
    pub three_curb_paused_from: f64,
    pub three_curb_paused_until: f64,
    pub crossed_bucket_day: Nullable<i64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiV1 {
    pub chat_open: bool,
    pub chat_cursor: usize,
    pub journal_open: bool,
    pub journal_scroll: [f32; 2],
    pub inventory_open: bool,
    pub map_open: bool,
    pub selected_index: usize,
    pub coin_offer_count: u32,
    pub next_request: u64,
    pub chalk_progress: f32,
    pub chalk_choice: usize,
    pub strain: f32,
    pub struggling_reported: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerminDefinitionV1 {
    pub seed: u64,
    pub swarm_percepts: bool,
    pub density_bits: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionsV1 {
    pub collision: [u8; 32],
    pub barriers: [u8; 32],
    pub vermin: [u8; 32],
    pub installed_catalogs: [u8; 32],
    pub gates_present: bool,
    pub vermin_installed: Nullable<VerminDefinitionV1>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarsV1 {
    pub controller: ControllerV1,
    pub time: TimeV1,
    pub boundary: BoundaryV1,
    pub spatial: SpatialV1,
    pub clock: ClockV1,
    pub gates: Nullable<GatesV1>,
    pub vermin: Nullable<VerminV1>,
    pub soundscape: SoundscapeV1,
    pub ui: UiV1,
    pub definitions: DefinitionsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum BellV1 {
    ScoldCurfew,
    ScoldSummons,
    NameKnell { years: u16 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemSourceV1 {
    Carried,
    Pocketed(BodySlotV1),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PendingKindV1<T> {
    Recording,
    Offer {
        item: T,
        target: T,
        quantity: Nullable<u32>,
    },
    Accept {
        item: T,
    },
    Decline {
        item: T,
    },
    Retract {
        item: T,
    },
    BodySlot {
        item: T,
    },
    Expel,
    DebugSay,
    Say,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ChalkIntentV1<T> {
    Scrub(u64),
    Draw { kind: MarkKindV1, handle: T },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentV1<T> {
    SpatialUpdate {
        sequence: u64,
        position: [f32; 3],
        yaw: f32,
    },
    Sound {
        sound: T,
    },
    RecordingInterrupted {
        request: T,
        basename: T,
        backend: T,
        sequence: u64,
        position: [f32; 3],
    },
    Offer {
        request: T,
        target: T,
        item: T,
        quantity: Nullable<u32>,
        sequence: u64,
        position: [f32; 3],
    },
    Accept {
        request: T,
        item: T,
        sequence: u64,
        position: [f32; 3],
    },
    Decline {
        request: T,
        item: T,
        sequence: u64,
        position: [f32; 3],
    },
    Retract {
        request: T,
        item: T,
    },
    Pocket {
        request: T,
        item: T,
        slot: BodySlotV1,
    },
    Retrieve {
        request: T,
        item: T,
    },
    Swallow {
        request: T,
        item: T,
    },
    Spit {
        request: T,
        item: T,
        target: T,
        sequence: u64,
        position: [f32; 3],
    },
    Gargle {
        request: T,
        item: T,
    },
    Expel {
        request: T,
    },
    Eat {
        request: T,
        item: T,
    },
    DebugSay {
        request: T,
        text: T,
        target: Nullable<T>,
        sequence: u64,
        position: [f32; 3],
    },
    Say {
        request: T,
        text: T,
        sequence: u64,
        position: [f32; 3],
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechV1<T> {
    pub sequence: u64,
    pub event: T,
    pub speaker: T,
    pub label: T,
    pub target: Nullable<T>,
    pub text: T,
    pub position: [f32; 3],
    pub recipient_count: usize,
    pub expect_audio: bool,
}

/// Finite field vocabulary. Ordered rows keep their owner order; keyed rows
/// are canonicalized only after the full extraction/index allowance exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum RecordV1<T> {
    Draft {
        text: T,
    },
    SelectedItem {
        item: Nullable<T>,
    },
    Custody {
        officer: Nullable<T>,
        officer_name: T,
        station_name: T,
        anchor: [f32; 3],
        closing: bool,
        strain_seconds: f32,
        held: bool,
        committed: bool,
        fee_sparks: u32,
        release_office: Nullable<T>,
        booked_as: Nullable<T>,
    },
    Holder {
        actor: T,
    },
    Notice {
        id: u64,
        line: T,
        rung: RungV1,
        clears_when: T,
    },
    Journal {
        attribution: T,
        word: T,
    },
    JournalStanding {
        text: T,
    },
    Hud {
        slot: HudSlot,
        text: T,
        #[serde(with = "wire::nullable_duration")]
        remaining: Nullable<Duration>,
    },
    ActiveOffer {
        item: T,
        giver: T,
        created_seq: u64,
        broadcast: bool,
        text: T,
        additional_count: usize,
    },
    DismissedBroadcast {
        item: T,
        created_seq: u64,
    },
    PendingCommand {
        request: T,
        kind: PendingKindV1<T>,
        sent_revision: u64,
        succeeded: bool,
    },
    InventoryContext {
        item: T,
        source: ItemSourceV1,
        screen_position: [f32; 2],
        spit_target: Nullable<(T, T)>,
    },
    ChalkHold {
        intent: Nullable<ChalkIntentV1<T>>,
    },
    ChalkPen {
        present: bool,
    },
    ChalkAnchor {
        handle: T,
        label: T,
        kinds: [Nullable<MarkKindV1>; 3],
    },
    Cooldown {
        key: u64,
        free_at: f64,
    },
    WellDraw {
        source: WellKind,
        at: f64,
    },
    Work {
        kind: WorkKind,
        position: [f32; 3],
        active: bool,
    },
    UnreadBell {
        message_id: u64,
        pattern: BellV1,
    },
    UnreadIntent {
        message_id: u64,
        intent: IntentV1<T>,
    },
    UnreadSpeech {
        message_id: u64,
        speech: SpeechV1<T>,
    },
    Subtitle {
        speech: SpeechV1<T>,
        formatted_text: T,
        minimum_seconds: f64,
        visible_since: Nullable<f64>,
        audio_playing: bool,
    },
    Bubble {
        speech: SpeechV1<T>,
        text: T,
        world_anchor: [f32; 3],
        expires_at: f64,
        audio_extended: bool,
    },
    PlayerReceipt {
        speech: SpeechV1<T>,
    },
}

pub type RecordRef<'a> = RecordV1<&'a str>;
pub trait HostCheckpointSource {
    fn scalars(&self) -> Result<ScalarsV1>;
    /// Visit without collecting, cloning strings or building indexes.
    fn records(&self, visitor: &mut dyn FnMut(RecordRef<'_>) -> Result<()>) -> Result<()>;
    /// Immutable borrowed rows for complete canonical encoding. The default
    /// leaves existing streaming-only component sources unchanged.
    fn complete_records<'a>(
        &'a self,
        _visitor: &mut dyn FnMut(RecordRef<'a>) -> Result<()>,
    ) -> Result<()> {
        Err(error(
            "complete capture requires immutable borrowed host records",
        ))
    }
    /// Actual host/Engine publication agreement. Called after admission.
    fn validate_boundary(&self) -> Result<()>;
}

/// Closed expected context supplied by the actual owner adapter. Full shared
/// root/category equality remains the later component assembly obligation.
#[derive(Debug, Serialize)]
pub struct HostDtoV1 {
    version: u16,
    scalars: ScalarsV1,
    records: Vec<RecordV1<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostWire {
    version: u16,
    scalars: ScalarsV1,
    records: Vec<RecordV1<String>>,
}
impl From<HostWire> for HostDtoV1 {
    fn from(w: HostWire) -> Self {
        Self {
            version: w.version,
            scalars: w.scalars,
            records: w.records,
        }
    }
}
#[derive(Debug)]
pub struct HostCandidate {
    dto: HostDtoV1,
}
impl HostCandidate {
    pub fn scalars(&self) -> &ScalarsV1 {
        &self.dto.scalars
    }
    pub fn records(&self) -> &[RecordV1<String>] {
        &self.dto.records
    }
}
struct Rows<'a, S>(&'a S);
impl<S: HostCheckpointSource> Serialize for Rows<'_, S> {
    fn serialize<T: serde::Serializer>(
        &self,
        serializer: T,
    ) -> std::result::Result<T::Ok, T::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        self.0
            .records(&mut |row| {
                seq.serialize_element(&row)
                    .map_err(|e| error(e.to_string()))
            })
            .map_err(serde::ser::Error::custom)?;
        seq.end()
    }
}
#[derive(Serialize)]
#[serde(bound(serialize = "S: HostCheckpointSource"))]
struct View<'a, S> {
    version: u16,
    scalars: ScalarsV1,
    records: Rows<'a, S>,
}
pub fn error(reason: impl Into<String>) -> CheckpointError {
    CheckpointError {
        owner: OWNER,
        reason: reason.into(),
    }
}
fn check(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(error(reason)) }
}

pub fn checkpoint_host_cost(source: &impl HostCheckpointSource) -> Result<ComponentCost> {
    let view = View {
        version: 1,
        scalars: source.scalars()?,
        records: Rows(source),
    };
    let mut counter = ContainerCounter::default();
    serde_json::to_writer(&mut counter, &view).map_err(|e| error(e.to_string()))?;
    let mut cost = aggregate::measure(&view, OWNER)?;
    cost.peak_bytes = cost
        .peak_bytes
        .checked_add(
            counter
                .containers
                .checked_mul(ROW_WORKING_BYTES)
                .ok_or_else(|| error("row stride overflow"))?,
        )
        .ok_or_else(|| error("host cost overflow"))?;
    cost.peak_bytes = cost
        .peak_bytes
        .checked_add(VALIDATION_WORKING_BYTES)
        .ok_or_else(|| error("validation cost overflow"))?;
    Ok(cost)
}
pub fn export_host_checkpoint(
    source: &impl HostCheckpointSource,
    context: HostCheckpointContext,
    mut reservation: Reservation,
) -> Result<Admitted<HostDtoV1>> {
    reservation.require(Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
    let cost = checkpoint_host_cost(source)?;
    if reservation.bytes() < cost.peak_bytes {
        reservation.resize(cost.peak_bytes)?;
    }
    source.validate_boundary()?;
    let view = View {
        version: 1,
        scalars: source.scalars()?,
        records: Rows(source),
    };
    let mut writer = BoundedWriter {
        bytes: Vec::with_capacity(cost.encoded_bytes),
        limit: cost.encoded_bytes,
    };
    serde_json::to_writer(&mut writer, &view).map_err(|e| error(e.to_string()))?;
    let bytes = writer.bytes;
    // Public borrowed sources may use interior mutability. Never trust that
    // the measured pass is the later pass: check exact raw/stride cost before
    // any typed row allocation, and never grow the bounded writer.
    let actual = raw_cost(&bytes)?;
    reservation.require(Cohort::SavePayload, actual.peak_bytes)?;
    let mut dto: HostDtoV1 = serde_json::from_slice::<HostWire>(&bytes)
        .map_err(|e| error(e.to_string()))?
        .into();
    dto.records.sort_by(ordering::compare);
    dto.validate(context)?;
    Ok(Admitted::new(dto, reservation))
}
impl HostDtoV1 {
    pub fn decode(
        bytes: &[u8],
        context: HostCheckpointContext,
        mut reservation: Reservation,
    ) -> Result<Admitted<Self>> {
        reservation.require(
            Cohort::LoadCandidate,
            bytes.len().saturating_add(aggregate::INITIAL_BYTES),
        )?;
        check(
            bytes.len() <= super::POPULATED_PAYLOAD_BYTES,
            "encoded host component exceeds supported bound",
        )?;
        // Input + lexer expansion are charged before deserialization. The row
        // stride is bounded by the number of lexical object/array openers,
        // including harmless whitespace/padding in the original input charge.
        let mut counter = ContainerCounter::default();
        counter.write_all(bytes).map_err(|e| error(e.to_string()))?;
        let dto: Self = aggregate::decode_with_working::<HostWire>(
            bytes,
            OWNER,
            &mut reservation,
            counter
                .containers
                .checked_mul(ROW_WORKING_BYTES)
                .and_then(|v| v.checked_add(VALIDATION_WORKING_BYTES))
                .ok_or_else(|| error("row stride overflow"))?,
        )?
        .into();
        dto.validate(context)?;
        Ok(Admitted::new(dto, reservation))
    }
    pub fn scalars(&self) -> &ScalarsV1 {
        &self.scalars
    }
    pub fn records(&self) -> &[RecordV1<String>] {
        &self.records
    }
    pub fn cost(&self) -> Result<ComponentCost> {
        let mut c = aggregate::measure(self, OWNER)?;
        let mut counter = ContainerCounter::default();
        serde_json::to_writer(&mut counter, self).map_err(|e| error(e.to_string()))?;
        c.peak_bytes = c
            .peak_bytes
            .checked_add(
                counter
                    .containers
                    .checked_mul(ROW_WORKING_BYTES)
                    .ok_or_else(|| error("row stride overflow"))?,
            )
            .and_then(|v| v.checked_add(VALIDATION_WORKING_BYTES))
            .ok_or_else(|| error("host cost overflow"))?;
        Ok(c)
    }
    fn validate(&self, c: HostCheckpointContext) -> Result<()> {
        c.validate()?;
        check(self.version == 1, "unsupported host version")?;
        validate_scalars(&self.scalars, c)?;
        validate_records(&self.records, &self.scalars)?;
        validate_publications(&self.records, &self.scalars, c)
    }
}
#[derive(Default)]
struct ContainerCounter {
    containers: usize,
    quoted: bool,
    escape: bool,
}
impl Write for ContainerCounter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for b in bytes {
            if self.quoted {
                if self.escape {
                    self.escape = false;
                } else if *b == b'\\' {
                    self.escape = true;
                } else if *b == b'"' {
                    self.quoted = false;
                }
            } else {
                match b {
                    b'"' => self.quoted = true,
                    b'{' | b'[' => {
                        self.containers = self
                            .containers
                            .checked_add(1)
                            .ok_or_else(|| std::io::Error::other("container count overflow"))?
                    }
                    _ => {}
                }
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other(
                "borrowed host source changed after preflight",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn raw_cost(bytes: &[u8]) -> Result<ComponentCost> {
    let mut c = aggregate::inspect(bytes)?;
    let mut count = ContainerCounter::default();
    count.write_all(bytes).map_err(|e| error(e.to_string()))?;
    c.peak_bytes = c
        .peak_bytes
        .checked_add(
            count
                .containers
                .checked_mul(ROW_WORKING_BYTES)
                .ok_or_else(|| error("row cost overflow"))?,
        )
        .ok_or_else(|| error("host cost overflow"))?;
    c.peak_bytes = c
        .peak_bytes
        .checked_add(VALIDATION_WORKING_BYTES)
        .ok_or_else(|| error("validation cost overflow"))?;
    Ok(c)
}
impl Admitted<HostDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|v, r| aggregate::encode(&v, OWNER, r))
    }
    pub fn into_candidate(self, context: HostCheckpointContext) -> Result<Admitted<HostCandidate>> {
        self.try_map(|dto, _| {
            dto.validate(context)?;
            Ok(HostCandidate { dto })
        })
    }
}

fn finite(values: impl IntoIterator<Item = f64>) -> bool {
    values.into_iter().all(f64::is_finite)
}
fn bits3(a: [f32; 3], b: [f64; 3]) -> bool {
    a.into_iter()
        .zip(b)
        .all(|(a, b)| f64::from(a).to_bits() == b.to_bits())
}
fn validate_scalars(s: &ScalarsV1, c: HostCheckpointContext) -> Result<()> {
    let position = c.position()?;
    let yaw = c.yaw()?;
    s.time.accepted.validate()?;
    let t = s.time.accepted.accepted()?;
    check(
        s.definitions == c.definitions,
        "installed host definitions disagree",
    )?;
    check(
        s.gates.0.is_some() == c.definitions.gates_present,
        "installed gate continuation presence",
    )?;
    check(
        s.vermin.0.map(|v| VerminDefinitionV1 {
            seed: v.seed,
            swarm_percepts: v.swarm_percepts,
            density_bits: v.density.to_bits(),
        }) == c.definitions.vermin_installed.0,
        "installed vermin continuation presence/configuration",
    )?;
    check(
        t.elapsed == c.elapsed
            && s.time.virtual_elapsed == t.elapsed
            && s.time
                .fixed_elapsed
                .checked_add(s.time.accepted.fixed_residual()?)
                == Some(t.elapsed),
        "actual accepted host clocks disagree",
    )?;
    check(
        s.time.virtual_delta <= crate::timeline::MAX_ACCEPTED_FRAME
            && s.time.fixed_delta <= s.time.accepted.fixed_step()?,
        "host clock delta outside range",
    )?;
    if let Some(f) = s.time.last_frame.0 {
        check(
            f.elapsed == t.elapsed
                && f.debt == t.debt
                && f.wall_delta <= t.wall
                && f.accepted_delta == s.time.virtual_delta,
            "last accepted frame disagreement",
        )?;
    }
    let b = &s.boundary;
    check(
        b.generation > 0
            && b.physical_sequence == c.sim.spatial_sequence as u64
            && b.physical_sequence < i64::MAX as u64
            && b.input_watermark == c.input_watermark
            && b.issued == c.issued
            && b.issued >= c.sim.host_high_water
            && b.message_sequence < u64::MAX
            && b.last_speech_sequence
                .0
                .is_none_or(|last| last <= b.message_sequence)
            && [b.speech_read, b.cue_read, b.intent_read]
                .iter()
                .all(|cursor| *cursor < usize::MAX as u64),
        "host command/physical boundary disagreement",
    )?;
    check(
        bits3(s.controller.current, position)
            && f64::from(s.controller.yaw).to_bits() == yaw.to_bits()
            && s.time.accepted.movement_residual()?.to_bits() == c.movement_residual().to_bits(),
        "host/sim pose or movement residual disagreement",
    )?;
    check(
        finite(s.controller.velocity.map(f64::from))
            && finite(s.controller.previous.map(f64::from))
            && finite([
                f64::from(s.controller.pitch),
                f64::from(s.controller.coyote_remaining),
                f64::from(s.controller.jump_buffer_remaining),
            ])
            && s.controller.coyote_remaining >= 0.0
            && s.controller.jump_buffer_remaining >= 0.0,
        "controller numeric range",
    )?;
    let cbody = &s.controller;
    // Walking has a horizontal clamp; flight has a whole-vector clamp. The
    // only unbounded ordinary axis is gravity during a fall. Its integral is
    // bounded by accepted lifetime; factor two conservatively covers f32 step
    // rounding. This also keeps squared lengths/displacements far from f32
    // overflow throughout the supported logical horizon.
    let vertical_limit = f64::from(controller_limits::MAX_FLY_SPEED)
        + 2.0 * f64::from(controller_limits::GRAVITY) * t.elapsed.as_secs_f64();
    check(
        cbody.velocity[0].abs() <= controller_limits::RUN_SPEED + 0.01
            && cbody.velocity[2].abs() <= controller_limits::RUN_SPEED + 0.01
            && f64::from(cbody.velocity[1].abs()) <= vertical_limit
            && cbody.pitch.abs() <= controller_limits::PITCH_LIMIT
            && cbody.coyote_remaining <= controller_limits::COYOTE_SECONDS
            && cbody.jump_buffer_remaining <= controller_limits::JUMP_BUFFER_SECONDS,
        "controller setter/accepted-lifetime bounds",
    )?;
    check(
        s.spatial.sequence == b.physical_sequence
            && s.spatial
                .last_position
                .0
                .is_some_and(|p| bits3(p, position))
            && s.spatial
                .last_yaw
                .0
                .is_some_and(|y| f64::from(y).to_bits() == yaw.to_bits()),
        "spatial owner disagreement",
    )?;
    s.spatial.last_background_send.validate()?;
    let k = s.clock;
    check(
        k.present
            && finite([k.fraction, k.brightness, k.scale, k.seconds_per_day])
            && (0.0..1.0).contains(&k.fraction)
            && k.day.unsigned_abs() <= super::MAX_CALENDAR_DAYS as u64
            && k.scale > 0.0
            && k.seconds_per_day > 0.0,
        "sampled clock numeric range",
    )?;
    check(
        (0.0..=1.0).contains(&s.ui.strain)
            && (0.0..=1.0).contains(&s.ui.chalk_progress)
            && finite(s.ui.journal_scroll.map(f64::from)),
        "UI numeric range",
    )?;
    check(
        s.ui.selected_index.checked_add(1).is_some(),
        "quickbar next-index arithmetic bound",
    )?;
    if let Some(g) = s.gates.0 {
        for m in [g.stone_leaves, g.river_leaves, g.river_bar] {
            check(
                finite([m.value, m.start, m.target, m.elapsed, m.duration].map(f64::from))
                    && [m.value, m.start, m.target]
                        .iter()
                        .all(|v| (0.0..=1.0).contains(v))
                    && m.elapsed >= 0.0
                    && m.duration >= m.elapsed,
                "gate motion range",
            )?;
        }
        if let Some((day, _)) = g.previous.0 {
            super::calendar(OWNER, day)?;
        }
    }
    if let Some(v) = s.vermin.0 {
        check(
            v.density.is_finite() && v.density >= 0.0,
            "vermin density range",
        )?;
        if let Some(m) = v.last_percept_minutes.0 {
            check(
                m.is_finite() && m.abs() <= super::MAX_CALENDAR_DAYS * 1440.0,
                "vermin calendar range",
            )?;
        }
    }
    let a = s.soundscape;
    for t in [
        a.last_pruned_at,
        a.ford_until,
        a.chain_until,
        a.three_curb_until,
        a.three_curb_paused_from,
        a.three_curb_paused_until,
    ] {
        super::logical(OWNER, t)?;
    }
    Ok(())
}

fn validate_records(rows: &[RecordV1<String>], s: &ScalarsV1) -> Result<()> {
    ordering::validate(rows, s)?;
    // Typed validation, exact singular families, and source-backed row limits.
    // Index allocation follows the attached aggregate + inline stride charge.
    let mut singular = std::collections::BTreeSet::new();
    let mut keys = std::collections::BTreeSet::new();
    let mut subtitles = 0usize;
    let mut bubbles = 0usize;
    let mut unread = 0usize;
    for r in rows {
        // All textual leaves are bounded, including ID and historical metadata;
        // later family checks impose the smaller ordinary speech/editor limits.
        let bytes = serde_json::to_vec(r).map_err(|e| error(e.to_string()))?;
        check(
            bytes.len() <= MAX_TEXT_BYTES * 4 + ROW_WORKING_BYTES,
            "host record text limit",
        )?;
        let unique = match r {
            RecordV1::Draft { text } => {
                check(
                    text.chars().count() <= 500 && s.ui.chat_cursor <= text.chars().count(),
                    "draft cursor/text limit",
                )?;
                Some("draft")
            }
            RecordV1::SelectedItem { .. } => Some("selected_item"),
            RecordV1::Custody {
                strain_seconds,
                anchor,
                ..
            } => {
                check(
                    strain_seconds.is_finite()
                        && *strain_seconds > 0.0
                        && finite(anchor.map(f64::from)),
                    "custody numeric range",
                )?;
                Some("custody")
            }
            RecordV1::ActiveOffer { .. } => Some("active_offer"),
            RecordV1::InventoryContext {
                screen_position, ..
            } => {
                check(finite(screen_position.map(f64::from)), "inventory position")?;
                Some("inventory_context")
            }
            RecordV1::ChalkHold { .. } => Some("chalk_hold"),
            RecordV1::ChalkPen { .. } => Some("chalk_pen"),
            RecordV1::PlayerReceipt { speech } => {
                validate_speech(speech, s)?;
                Some("player_receipt")
            }
            RecordV1::Subtitle {
                speech,
                minimum_seconds,
                visible_since,
                ..
            } => {
                validate_speech(speech, s)?;
                check((3.0..=10.0).contains(minimum_seconds), "subtitle duration")?;
                if let Some(t) = visible_since.0 {
                    super::logical(OWNER, t)?;
                    check(
                        t <= s.time.virtual_elapsed.as_secs_f64(),
                        "future subtitle visible time",
                    )?;
                }
                subtitles += 1;
                None
            }
            RecordV1::Bubble {
                speech,
                world_anchor,
                expires_at,
                ..
            } => {
                validate_speech(speech, s)?;
                check(finite(world_anchor.map(f64::from)), "bubble anchor")?;
                super::logical(OWNER, *expires_at)?;
                bubbles += 1;
                None
            }
            RecordV1::UnreadSpeech { message_id, speech } => {
                validate_speech(speech, s)?;
                check(
                    *message_id >= s.boundary.speech_read,
                    "speech cursor regression",
                )?;
                unread += 1;
                None
            }
            RecordV1::UnreadBell { message_id, .. } => {
                check(*message_id >= s.boundary.cue_read, "cue cursor regression")?;
                unread += 1;
                None
            }
            RecordV1::UnreadIntent { message_id, .. } => {
                check(
                    *message_id >= s.boundary.intent_read,
                    "intent cursor regression",
                )?;
                unread += 1;
                None
            }
            RecordV1::Cooldown { key, free_at } => {
                super::logical(OWNER, *free_at)?;
                check(keys.insert(format!("cooldown:{key}")), "duplicate cooldown")?;
                None
            }
            RecordV1::WellDraw { source, at } => {
                super::logical(OWNER, *at)?;
                check(keys.insert(format!("well:{source:?}")), "duplicate well")?;
                None
            }
            RecordV1::Work { kind, position, .. } => {
                check(finite(position.map(f64::from)), "work position")?;
                check(keys.insert(format!("work:{kind:?}")), "duplicate work")?;
                None
            }
            RecordV1::Hud {
                slot, remaining, ..
            } => {
                check(keys.insert(format!("hud:{slot:?}")), "duplicate HUD slot")?;
                if let Some(t) = remaining.0 {
                    check(t <= Duration::from_secs(8), "HUD duration range")?;
                }
                None
            }
            RecordV1::PendingCommand { request, .. } => {
                check(
                    keys.insert(format!("pending:{request}")),
                    "duplicate pending command",
                )?;
                None
            }
            RecordV1::DismissedBroadcast { item, .. } => {
                check(
                    keys.insert(format!("dismissed:{item}")),
                    "duplicate dismissed offer",
                )?;
                None
            }
            RecordV1::Holder { actor } => {
                check(
                    keys.insert(format!("holder:{actor}")),
                    "duplicate custody holder",
                )?;
                None
            }
            RecordV1::Notice { id, .. } => {
                check(
                    keys.insert(format!("notice:{id}")),
                    "duplicate custody notice",
                )?;
                None
            }
            RecordV1::ChalkAnchor { handle, .. } => {
                check(
                    keys.insert(format!("chalk:{handle}")),
                    "duplicate chalk anchor",
                )?;
                None
            }
            RecordV1::Journal { .. } | RecordV1::JournalStanding { .. } => None,
        };
        if let Some(k) = unique {
            check(singular.insert(k), "duplicate singular host record")?;
        }
    }
    check(
        ["draft", "selected_item", "chalk_hold", "chalk_pen"]
            .iter()
            .all(|key| singular.contains(key)),
        "missing singular host record",
    )?;
    check(
        subtitles <= MAX_PRESENTATIONS
            && bubbles <= MAX_PRESENTATIONS
            && unread <= MAX_UNREAD_PUBLICATIONS,
        "host presentation row limit",
    )?;
    Ok(())
}
fn validate_speech(v: &SpeechV1<String>, s: &ScalarsV1) -> Result<()> {
    check(
        !v.event.is_empty()
            && !v.speaker.is_empty()
            && !v.text.trim().is_empty()
            && v.text.trim().chars().count() <= 500
            && v.event
                .len()
                .saturating_add(v.speaker.len())
                .saturating_add(v.label.len())
                .saturating_add(v.text.len())
                .saturating_add(v.target.0.as_ref().map_or(0, String::len))
                <= 16 * 1024
            && finite(v.position.map(f64::from))
            && v.sequence <= s.boundary.message_sequence,
        "speech identity/text/boundary range",
    )
}

pub(crate) fn complete_write<W: std::io::Write>(
    source: &impl HostCheckpointSource,
    writer: &mut W,
) -> Result<()> {
    source.validate_boundary()?;
    // The complete caller has already reserved 64 MiB scratch. Allocate this
    // fixed bounded index before visiting, so even an interior-mutating source
    // cannot cause unadmitted growth. Stable sorting retains consumer order.
    const INDEX_BYTES: usize = 8 * 1024 * 1024;
    let limit = INDEX_BYTES / std::mem::size_of::<RecordRef<'_>>();
    let mut records = Vec::with_capacity(limit);
    source.complete_records(&mut |row| {
        check(records.len() < limit, "complete host record index limit")?;
        records.push(row);
        Ok(())
    })?;
    records.sort_by(ordering::compare);
    #[derive(Serialize)]
    struct Canonical<'a> {
        version: u16,
        scalars: ScalarsV1,
        records: Vec<RecordRef<'a>>,
    }
    super::complete::write_json(
        writer,
        &Canonical {
            version: 1,
            scalars: source.scalars()?,
            records,
        },
    )
}

impl HostDtoV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn complete_decode_with_components(
        bytes: &[u8],
        meter: &super::complete::meter::DecodeMeter<'_>,
        backbone: &crate::world::checkpoint::BackboneCandidate,
        law: &crate::engine::law_checkpoint::EngineLawCandidate,
        knowledge: &crate::engine::knowledge_checkpoint::EngineKnowledgeCandidate,
        marks: &crate::engine::marks_checkpoint::EngineMarksCandidate,
        climate: &crate::engine::climate_checkpoint::EngineClimateCandidate,
        animals: &crate::engine::animals_checkpoint::EngineAnimalsCandidate,
        ledger: &crate::receipts::CommandLedgerDtoV1,
        definitions: DefinitionsV1,
        now: crate::timeline::LogicalTime,
    ) -> Result<HostCandidate> {
        let dto: Self = meter.decode::<HostWire>(bytes)?.into();
        let elapsed = dto.scalars.time.virtual_elapsed;
        check(
            elapsed.as_secs_f64().to_bits() == now.seconds().to_bits(),
            "complete host elapsed disagreement",
        )?;
        let context = HostCheckpointContext::from_components(
            backbone,
            law,
            knowledge,
            marks,
            climate,
            animals,
            ledger,
            elapsed,
            definitions,
            dto.scalars.boundary.input_watermark,
            dto.scalars.boundary.issued,
        );
        dto.validate(context)?;
        Ok(HostCandidate { dto })
    }
}
