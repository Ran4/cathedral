//! Exact private character record; seed values and live state remain distinct.
use super::*;
use crate::checkpoint::{
    self, Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate, records,
};
const OWNER: &str = "character";
#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterDtoV1 {
    version: u16,
    #[serde(with = "records::CharacterV1")]
    pub(crate) value: Character,
}
// Deserialization is private so external callers cannot bypass aggregate
// admission with serde_json::from_slice::<PublicDto>().
#[derive(Serialize, Deserialize)]
#[serde(remote = "CharacterDtoV1", deny_unknown_fields)]
pub(crate) struct CharacterWireV1 {
    version: u16,
    #[serde(with = "records::CharacterV1")]
    pub(crate) value: Character,
}
#[derive(Deserialize)]
struct Decoded(#[serde(with = "CharacterWireV1")] CharacterDtoV1);

#[derive(Serialize)]
struct View<'a> {
    version: u16,
    #[serde(with = "records::CharacterV1")]
    value: &'a Character,
}
impl Character {
    pub fn export_checkpoint(
        &self,
        mut reservation: Reservation,
    ) -> Result<Admitted<CharacterDtoV1>> {
        aggregate::prepare_export(
            &View {
                version: 1,
                value: self,
            },
            OWNER,
            &mut reservation,
        )?;
        validate(self)?;
        Ok(Admitted::new(
            CharacterDtoV1 {
                version: 1,
                value: self.clone(),
            },
            reservation,
        ))
    }
}
impl CharacterDtoV1 {
    pub fn decode(bytes: &[u8], mut reservation: Reservation) -> Result<Admitted<Self>> {
        let Decoded(dto) = aggregate::decode(bytes, OWNER, &mut reservation)?;
        dto.validate()?;
        Ok(Admitted::new(dto, reservation))
    }
    pub fn validate(&self) -> Result<()> {
        check(self.version == 1, "unsupported character version")?;
        validate(&self.value)
    }
    pub fn cost(&self) -> Result<ComponentCost> {
        aggregate::measure(self, OWNER)
    }
}
impl Admitted<CharacterDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|dto, r| aggregate::encode(&dto, OWNER, r))
    }
    #[allow(dead_code)] // Reserved for later complete owner composition.
    pub(crate) fn into_candidate(self) -> Result<Admitted<Character>> {
        self.try_map(|dto, _| {
            dto.validate()?;
            Ok(dto.value)
        })
    }
}
pub(crate) fn check(ok: bool, reason: &'static str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(CheckpointError::new(OWNER, reason))
    }
}
pub(crate) fn point(v: Vec3) -> Result<()> {
    check(
        v.is_finite() && v.abs().max_element() <= 1_000_000.0,
        "position outside supported city coordinate range",
    )
}
fn scalar(v: f64, max: f64) -> Result<()> {
    check(
        v.is_finite() && (0.0..=max).contains(&v),
        "invalid character numeric state",
    )
}
fn ids<'a, T: 'a>(
    values: impl IntoIterator<Item = &'a T>,
    valid: impl Fn(&T) -> bool,
) -> Result<()> {
    check(values.into_iter().all(valid), "invalid character reference")
}
pub(crate) fn metadata(values: &BTreeMap<String, String>) -> Result<()> {
    check(
        values
            .iter()
            .all(|(k, v)| k.len() <= records::MAX_TEXT_BYTES && v.len() <= records::MAX_TEXT_BYTES),
        "metadata text byte limit",
    )
}
pub(crate) fn validate(c: &Character) -> Result<()> {
    let s = &c.sheet;
    let a = &c.state;
    check(s.id.is_valid(), "invalid actor identity")?;
    point(s.position_m)?;
    point(a.position_m)?;
    check(
        s.facing_yaw.is_finite() && a.facing_yaw.is_finite(),
        "invalid facing",
    )?;
    ids(&s.holds, ItemId::is_valid)?;
    ids(&a.holds, ItemId::is_valid)?;
    // Sheet holdings are historical seed references: no live item lookup.
    for p in s.pockets.iter().chain(&a.pockets) {
        check(p.item_id.is_valid(), "invalid pocket reference")?;
    }
    ids(&s.knows, ActorId::is_valid)?;
    ids(&a.knows, ActorId::is_valid)?;
    ids(&a.places_known, PlaceId::is_valid)?;
    check(
        a.presence_epoch >= s.presence_epoch,
        "presence epoch precedes seed epoch",
    )?;
    scalar(a.needs.thirst, THIRST_MAX)?;
    scalar(a.needs.hunger, HUNGER_MAX)?;
    check(
        a.inbox.len() <= INBOX_MAX_ENTRIES
            && a.recent_history.len() <= RECENT_HISTORY_MAX_ENTRIES
            && a.pending_history.len() <= INBOX_MAX_ENTRIES,
        "percept window exceeds runtime limit",
    )?;
    // Repeated prose across ordered buffers is legal; do not deduplicate it.
    for g in &a.gut {
        check(g.kind.is_valid(), "invalid gut kind")?;
        metadata(&g.metadata)?;
        checkpoint::calendar(OWNER, g.due_game_days)?;
    }
    if let Some(t) = a.urgency_since_game_days {
        checkpoint::calendar(OWNER, t)?;
    }
    if let Some(v) = a.debug_urgency {
        scalar(v, 1.0)?;
    }
    for v in a.statuses.values() {
        scalar(*v, 1.0)?;
    }
    if let Some(m) = &a.movement {
        scalar(m.speed, 1000.0)?;
        scalar(m.gait_phase, 1.0e12)?;
        checkpoint::logical(OWNER, m.choke_wait)?;
        for p in &m.path {
            point(*p)?;
        }
    }
    if let Some(i) = &a.intent {
        checkpoint::logical(OWNER, i.budget_seconds)?;
        if let Some(t) = i.deadline {
            checkpoint::logical(OWNER, t)?;
        }
        // Stored deadline/target can await the next ordinary Round pass.
        match &i.target {
            IntentTarget::Place {
                place_id, point: p, ..
            } => {
                check(place_id.is_valid(), "invalid attempted place")?;
                point(*p)?;
            }
            IntentTarget::Person {
                actor_id,
                last_seen,
                ..
            } => {
                check(actor_id.is_valid(), "invalid attempted actor")?;
                point(*last_seen)?;
            }
        }
        if let Some(id) = i.receipt {
            command(id)?;
        }
    }
    if let Some(e) = &a.round_edit {
        check(e.place_id.is_valid(), "invalid attempted round place")?;
        check(e.leg < 65_536, "unsupported round edit index")?;
        if let Some(id) = e.receipt {
            command(id)?;
        }
        // An incarnation change can occur before the Round refuses the edit.
    }
    if let Some(g) = a.active_gesture {
        if let Some(t) = g.deadline {
            checkpoint::logical(OWNER, t)?;
        }
    }
    if let Some(r) = &a.resident {
        checkpoint::logical(OWNER, r.dwell_remaining_seconds)?;
    }
    if let Some(l) = &s.lore {
        ids(
            l.father.iter().chain(l.mother.iter()).chain(&l.children),
            ActorId::is_valid,
        )?;
        if let Some(p) = l.home_point_m {
            point(Vec3::new(p[0], 0.0, p[1]))?;
        }
        if let Some(v) = l.curiosity {
            scalar(v, 1.0)?;
        }
    }
    Ok(())
}
pub(crate) fn command(id: crate::receipts::CommandId) -> Result<()> {
    check(
        (id.operation.producer as usize) < crate::receipts::PRODUCER_CAPACITY
            && id.operation.sequence > 0
            && id.step <= crate::receipts::MAX_STEPS,
        "invalid obligation command identity",
    )
}
