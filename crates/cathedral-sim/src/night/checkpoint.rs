//! Exact existing Night authority; candidates cannot install a partial Engine.
use super::*;
use crate::{
    checkpoint::{self, Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
mod context;
pub(crate) mod records;
pub use context::NightCheckpointContext;
const OWNER: &str = "night";
pub(crate) fn cognition_input(
    n: &NightOffice,
) -> Option<crate::engine::cognition_inputs_checkpoint::records::NightInputRef<'_>> {
    use crate::engine::cognition_inputs_checkpoint::{
        CognitionRequestMethod,
        records::{NightInputRef, NightSubjectRef},
    };
    n.in_flight.as_ref().map(|f| NightInputRef {
        method: CognitionRequestMethod::RequestNight,
        subject: match &f.subject {
            Subject::Person(a) => NightSubjectRef::Person(a),
            Subject::Ward(w) => NightSubjectRef::Ward(*w),
        },
        presence_epoch: f.presence_epoch,
        request_id: f.request_id,
        semantic: f.semantic,
        owed_day: f.owed_day,
        prompt: &f.prompt,
        output_token_budget: f.output_token_budget,
    })
}
pub(crate) fn validate_cognition_inputs(
    n: &NightOffice,
    c: NightCheckpointContext<'_>,
) -> Result<()> {
    context::BindingV1::new(c)?;
    validate(n, c)
}
pub const MAX_PERSONS: usize = 25_000;
pub const MAX_SUBJECTS: usize = MAX_PERSONS + 8;
pub const VALIDATION_WORKING_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_PROMPT_BYTES: usize = 65_536;
const COUNTER_DROP_HEADROOM: u64 = (2 * MAX_SUBJECTS + 1) as u64;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NightCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for NightCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            validation_working_bytes: VALIDATION_WORKING_BYTES,
            peak_bytes: c.peak_bytes + VALIDATION_WORKING_BYTES,
        }
    }
}
pub(crate) fn check(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(CheckpointError::new(OWNER, reason))
    }
}
fn id(s: &str) -> Result<()> {
    check(
        s.len() <= 4 * crate::MAX_ID_CHARS && crate::ids::is_valid_id(s),
        "invalid Night identity",
    )
}
fn day(d: i64) -> Result<()> {
    checkpoint::calendar(OWNER, d as f64)
}
fn subject(s: &Subject) -> Result<()> {
    if let Subject::Person(a) = s {
        id(a.as_str())?;
    }
    Ok(())
}
pub(crate) fn prepare<T: Serialize>(v: &T, r: &mut Reservation) -> Result<NightCost> {
    let cost = NightCost::from(aggregate::prepare_export(v, OWNER, r)?);
    if r.bytes() < cost.peak_bytes {
        r.resize(cost.peak_bytes)?;
    }
    Ok(cost)
}
pub(crate) fn binding(
    c: NightCheckpointContext<'_>,
    r: &mut Reservation,
) -> Result<context::BindingV1> {
    r.require(checkpoint::Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
    if r.bytes() < aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES {
        r.resize(aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES)?;
    }
    context::BindingV1::new(c)
}
fn validate(n: &NightOffice, c: NightCheckpointContext<'_>) -> Result<()> {
    check(
        n.queue.len() <= MAX_SUBJECTS
            && n.last_reflected.len() <= MAX_SUBJECTS
            && n.bedtimes.len() <= MAX_PERSONS,
        "Night count limit",
    )?;
    checkpoint::calendar(OWNER, n.last_office_days)?;
    if let Some(d) = n.last_ambient_reroll_day {
        day(d)?;
    }
    check(
        n.next_attempt_at >= 0.0
            && (n.next_attempt_at.is_finite() || n.next_attempt_at == f64::INFINITY),
        "invalid Night pacing",
    )?;
    checkpoint::logical(OWNER, n.next_yield_report)?;
    // Ring may add the entire borrowed bounded cast/wards before one poll
    // drops the old+new queues and one harvested flight.
    // This is local headroom, not an all-future/calendar-consumer proof.
    check(
        n.reflected < u64::MAX && n.dropped <= u64::MAX - COUNTER_DROP_HEADROOM,
        "Night counter headroom",
    )?;
    for (s, d) in &n.last_reflected {
        subject(s)?;
        day(*d)?;
    }
    for a in n.bedtimes.keys() {
        id(a.as_str())?;
    }
    for (index, due) in n.queue.iter().enumerate() {
        subject(&due.subject)?;
        day(due.day)?;
        check(
            n.last_reflected.get(&due.subject) == Some(&due.day),
            "queued Night stamp disagreement",
        )?;
        match due.semantic {
            None => check(due.presence_epoch.is_none(), "unadmitted Night incarnation")?,
            Some(root) => {
                check(
                    index == 0 && n.in_flight.is_none(),
                    "admitted Night duty must be the sole front admission without a flight",
                )?;
                check(c.root(root), "Night queue semantic root disagreement")?;
                incarnation(&due.subject, due.presence_epoch)?;
            }
        }
    }
    // Bounded borrowed scratch, sorted without altering queue order.
    let mut subjects: Vec<_> = n.queue.iter().map(|d| &d.subject).collect();
    subjects.sort_unstable();
    check(
        subjects.windows(2).all(|x| x[0] != x[1]),
        "duplicate Night queue subject",
    )?;
    let mut roots: Vec<_> = n.queue.iter().filter_map(|d| d.semantic).collect();
    if let Some(f) = &n.in_flight {
        subject(&f.subject)?;
        day(f.owed_day)?;
        check(
            c.root(f.semantic),
            "Night flight semantic root disagreement",
        )?;
        incarnation(&f.subject, f.presence_epoch)?;
        let stamp = n.last_reflected.get(&f.subject);
        let queued = n.queue.iter().find(|d| d.subject == f.subject);
        check(
            stamp.is_some()
                && (stamp == Some(&f.owed_day) || queued.is_some_and(|d| Some(&d.day) == stamp)),
            "Night flight queue-time stamp disagreement",
        )?;
        check(
            queued.is_none_or(|d| d.day != f.owed_day),
            "duplicate queued and flying Night duty day",
        )?;
        check(
            f.prompt.len() <= MAX_PROMPT_BYTES,
            "Night prompt byte limit",
        )?;
        roots.push(f.semantic);
    }
    roots.sort_unstable();
    check(
        roots.windows(2).all(|x| x[0] != x[1]),
        "duplicate Night semantic root",
    )?;
    if let Some(done) = &n.held_result {
        check(
            n.in_flight
                .as_ref()
                .is_some_and(|f| f.request_id == done.request_id),
            "held Night request disagreement",
        )?;
        checkpoint::logical(OWNER, done.duration_seconds)?;
        match &done.result {
            Ok(reply) => check(
                receipts::provider_reply_within_limit(reply),
                "held Night reply limit",
            )?,
            Err(error) => check(
                error.kind().len() <= checkpoint::records::MAX_TEXT_BYTES
                    && error.detail().len() <= checkpoint::records::MAX_TEXT_BYTES,
                "held Night error limit",
            )?,
        }
    }
    Ok(())
}
fn incarnation(s: &Subject, e: Option<u64>) -> Result<()> {
    check(
        matches!(
            (s, e),
            (Subject::Person(_), Some(_)) | (Subject::Ward(_), None)
        ),
        "Night subject/incarnation disagreement",
    )
}
pub(crate) fn copy(n: &NightOffice) -> NightOffice {
    NightOffice {
        config: n.config,
        queue: n.queue.clone(),
        in_flight: n.in_flight.clone().map(|mut f| {
            f.output_token_budget = crate::traits::AcceptedOutputBudget::MissingLegacy;
            f
        }),
        held_result: n.held_result.clone(),
        last_reflected: n.last_reflected.clone(),
        bedtimes: n.bedtimes.clone(),
        last_office_days: n.last_office_days,
        last_ambient_reroll_day: n.last_ambient_reroll_day,
        next_attempt_at: n.next_attempt_at,
        next_yield_report: n.next_yield_report,
        seeded: n.seeded,
        reflected: n.reflected,
        dropped: n.dropped,
    }
}
#[derive(Debug, Serialize)]
pub struct NightOfficeDtoV1 {
    version: u16,
    pub(crate) boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "records::NightV1")]
    pub(crate) night: NightOffice,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "records::NightV1")]
    night: NightOffice,
}
impl From<Wire> for NightOfficeDtoV1 {
    fn from(w: Wire) -> Self {
        Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            night: w.night,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    context: &'a context::BindingV1,
    #[serde(with = "records::NightV1")]
    night: &'a NightOffice,
}
impl<'a> View<'a> {
    pub(crate) fn new(
        n: &'a NightOffice,
        now: LogicalTime,
        context: &'a context::BindingV1,
    ) -> Self {
        Self {
            version: 1,
            boundary: now,
            context,
            night: n,
        }
    }
}
#[derive(Debug)]
pub struct NightOfficeCandidate {
    pub(crate) data: NightOfficeDtoV1,
}
impl NightOffice {
    pub fn export_checkpoint(
        &self,
        c: NightCheckpointContext<'_>,
        mut r: Reservation,
    ) -> Result<Admitted<NightOfficeDtoV1>> {
        let context = binding(c, &mut r)?;
        prepare(&View::new(self, c.now, &context), &mut r)?;
        validate(self, c)?;
        Ok(Admitted::new(
            NightOfficeDtoV1 {
                version: 1,
                boundary: c.now,
                context,
                night: copy(self),
            },
            r,
        ))
    }
}
impl NightOfficeDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: NightCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let wire: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self::from(wire);
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub(crate) fn validate(&self, c: NightCheckpointContext<'_>) -> Result<()> {
        check(self.version == 1, "unsupported Night version")?;
        check(self.boundary == c.now, "Night boundary disagreement")?;
        check(
            self.context == context::BindingV1::new(c)?,
            "Night clock context disagreement",
        )?;
        validate(&self.night, c)
    }
    pub fn cost(&self) -> Result<NightCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: NightCheckpointContext<'_>) -> NightCounts {
        counts(&self.night, c)
    }
}
impl Admitted<NightOfficeDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: NightCheckpointContext<'_>,
    ) -> Result<Admitted<NightOfficeCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(NightOfficeCandidate { data: d })
        })
    }
}
impl NightOfficeCandidate {
    pub fn night(&self) -> &NightOffice {
        &self.data.night
    }
    pub fn counts(&self, c: NightCheckpointContext<'_>) -> NightCounts {
        self.data.counts(c)
    }
}
pub(crate) struct NightOfficeV1;
impl NightOfficeV1 {
    pub fn serialize<S: serde::Serializer>(
        v: &NightOfficeDtoV1,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<NightOfficeDtoV1, D::Error> {
        Wire::deserialize(d).map(Into::into)
    }
}
#[derive(Debug, Serialize)]
pub struct WorldNightDtoV1 {
    version: u16,
    pub(crate) boundary: LogicalTime,
    #[serde(with = "checkpoint::records::text::map")]
    pub(crate) ward_moods: BTreeMap<PlanningWard, String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldWire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "checkpoint::records::text::map")]
    ward_moods: BTreeMap<PlanningWard, String>,
}
impl From<WorldWire> for WorldNightDtoV1 {
    fn from(w: WorldWire) -> Self {
        Self {
            version: w.version,
            boundary: w.boundary,
            ward_moods: w.ward_moods,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct WorldView<'a> {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "checkpoint::records::text::map")]
    ward_moods: &'a BTreeMap<PlanningWard, String>,
}
impl<'a> WorldView<'a> {
    pub(crate) fn new(w: &'a World, now: LogicalTime) -> Self {
        Self {
            version: 1,
            boundary: now,
            ward_moods: &w.ward_moods,
        }
    }
}
#[derive(Debug)]
pub struct WorldNightCandidate {
    pub(crate) data: WorldNightDtoV1,
}
impl World {
    pub fn export_night_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<WorldNightDtoV1>> {
        checkpoint::logical(OWNER, now.seconds())?;
        prepare(&WorldView::new(self, now), &mut r)?;
        Ok(Admitted::new(
            WorldNightDtoV1 {
                version: 1,
                boundary: now,
                ward_moods: self.ward_moods.clone(),
            },
            r,
        ))
    }
}
impl WorldNightDtoV1 {
    pub(crate) fn cognition_inputs_boundary(&self) -> LogicalTime {
        self.boundary
    }
    pub(crate) fn from_world_for_engine(w: &World, now: LogicalTime) -> Self {
        Self {
            version: 1,
            boundary: now,
            ward_moods: w.ward_moods.clone(),
        }
    }
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: NightCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let wire: WorldWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self::from(wire);
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub(crate) fn validate(&self, c: NightCheckpointContext<'_>) -> Result<()> {
        checkpoint::logical(OWNER, c.now.seconds())?;
        check(self.version == 1, "unsupported World Night version")?;
        check(self.boundary == c.now, "World Night boundary disagreement")?;
        check(
            self.ward_moods.len() <= 8
                && self
                    .ward_moods
                    .values()
                    .all(|s| s.len() <= checkpoint::records::MAX_TEXT_BYTES),
            "ward mood text limit",
        )
    }
    pub fn cost(&self) -> Result<NightCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn ward_moods(&self) -> &BTreeMap<PlanningWard, String> {
        &self.ward_moods
    }
}
impl Admitted<WorldNightDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: NightCheckpointContext<'_>,
    ) -> Result<Admitted<WorldNightCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(WorldNightCandidate { data: d })
        })
    }
}
impl WorldNightCandidate {
    pub fn ward_moods(&self) -> &BTreeMap<PlanningWard, String> {
        &self.data.ward_moods
    }
}
pub(crate) struct WorldNightV1;
impl WorldNightV1 {
    pub fn serialize<S: serde::Serializer>(
        v: &WorldNightDtoV1,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<WorldNightDtoV1, D::Error> {
        WorldWire::deserialize(d).map(Into::into)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NightCounts {
    pub characters: usize,
    pub queued: usize,
    pub queued_admitted: usize,
    pub queued_people: usize,
    pub queued_wards: usize,
    pub in_flight: bool,
    pub held_success: bool,
    pub held_error: bool,
    pub prompt_bytes: usize,
    pub held_bytes: usize,
    pub stamps: usize,
    pub bedtimes: usize,
    pub seeded: bool,
    pub enabled: bool,
    pub reflected: u64,
    pub dropped: u64,
}
fn counts(n: &NightOffice, c: NightCheckpointContext<'_>) -> NightCounts {
    NightCounts {
        characters: c.backbone.characters.len(),
        queued: n.queue.len(),
        queued_admitted: n.queue.iter().filter(|d| d.semantic.is_some()).count(),
        queued_people: n
            .queue
            .iter()
            .filter(|d| matches!(d.subject, Subject::Person(_)))
            .count(),
        queued_wards: n
            .queue
            .iter()
            .filter(|d| matches!(d.subject, Subject::Ward(_)))
            .count(),
        in_flight: n.in_flight.is_some(),
        held_success: n.held_result.as_ref().is_some_and(|x| x.result.is_ok()),
        held_error: n.held_result.as_ref().is_some_and(|x| x.result.is_err()),
        prompt_bytes: n.in_flight.as_ref().map_or(0, |x| x.prompt.len()),
        held_bytes: n.held_result.as_ref().map_or(0, |x| match &x.result {
            Ok(t) => t.len(),
            Err(e) => e.kind().len() + e.detail().len(),
        }),
        stamps: n.last_reflected.len(),
        bedtimes: n.bedtimes.len(),
        seeded: n.seeded,
        enabled: n.enabled(),
        reflected: n.reflected,
        dropped: n.dropped,
    }
}
#[cfg(test)]
pub(crate) mod tests;
