//! Complete private custody records, with specific live/historical bindings.
use super::*;
use crate::{
    checkpoint::{self, Result},
    notices::checkpoint::{HANDLE_HEADROOM, LawCheckpointContext, MAX_NAMES, id, text},
};
pub(crate) mod records;
use crate::{
    checkpoint::{Admitted, Reservation, aggregate},
    clock::WorldTime,
    notices::checkpoint::{
        LawCost, VALIDATION_WORKING_BYTES, context, context_for_export, prepare,
    },
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
const OWNER: &str = "custody";
fn check(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(checkpoint::CheckpointError::new("custody", reason))
    }
}
pub(crate) fn validate(custody: &Custody, c: LawCheckpointContext<'_>) -> Result<()> {
    check(custody.held.len() <= MAX_NAMES, "custody record count")?;
    check(
        custody.arrest_count() <= CUSTODY_MAX_ARRESTS,
        "active arrest count",
    )?;
    for (prisoner, r) in &custody.held {
        id(prisoner.as_str())?;
        check(
            c.backbone.characters.contains_key(prisoner),
            "custody prisoner binding missing",
        )?;
        id(r.station.place_id.as_str())?;
        text(&r.station.name)?;
        crate::character::checkpoint::point(r.station.point)?;
        if let Some(n) = r.notice_id {
            check(n > 0, "invalid historical custody notice identity")?;
        }
        if let Some(officer) = &r.officer {
            id(officer.as_str())?;
            if r.state == Confinement::InCharge {
                check(
                    c.backbone.characters.contains_key(officer),
                    "active escort binding missing",
                )?;
            }
        }
        check(r.holders.len() <= MAX_NAMES, "custody holder count")?;
        let mut seen = BTreeSet::new();
        for h in &r.holders {
            id(h.as_str())?;
            check(seen.insert(h), "duplicate custody holder")?;
            check(
                c.backbone.characters.contains_key(h),
                "active holder binding missing",
            )?;
        }
        check(
            current_strain_seconds(c, prisoner, &r.holders).is_finite(),
            "custody grip arithmetic overflows",
        )?;
        checkpoint::logical("custody", r.seized_at)?;
        check(
            r.seized_at <= c.now.seconds(),
            "custody seized anchor in future",
        )?;
        for t in [r.committed_at, r.officer_last_turn].into_iter().flatten() {
            checkpoint::logical("custody", t)?;
            check(
                t >= r.seized_at && t <= c.now.seconds(),
                "custody logical anchor disagreement",
            )?;
        }
        check(
            r.struggles <= u64::MAX - HANDLE_HEADROOM,
            "struggle allocator lacks supported headroom",
        )?;
        check(
            r.sentence_office.is_some() == r.sentence_due_game_days.is_some(),
            "sentence office/deadline disagreement",
        )?;
        if let Some(d) = r.sentence_due_game_days {
            checkpoint::calendar("custody", d)?;
            check(
                r.state == Confinement::Committed && r.station.stone_house,
                "sentence without gaol commitment",
            )?;
        }
        match r.state {
            Confinement::InCharge => check(
                r.officer.is_some() && r.committed_at.is_none() && !r.authored,
                "in-charge ownership/commitment disagreement",
            )?,
            Confinement::Committed => {
                check(r.committed_at.is_some(), "committed custody lacks anchor")?
            }
        }
        // A committed officer and notice/Station links may outlive departure,
        // settlement/expiry and registry reload. Empty hands do not end custody;
        // closing can remain latched across commitment until its ordinary pass.
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct CustodyDtoV1 {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::CustodyV1")]
    pub(crate) custody: Custody,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustodyWire {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::CustodyV1")]
    pub(crate) custody: Custody,
}
#[derive(Serialize)]
struct CustodyView<'a> {
    version: u16,
    boundary: LogicalTime,
    context: &'a context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::CustodyV1")]
    custody: &'a Custody,
}
#[derive(Debug)]
pub struct CustodyCandidate {
    pub(crate) data: CustodyDtoV1,
}
impl Custody {
    pub fn export_checkpoint(
        &self,
        c: LawCheckpointContext<'_>,
        mut r: Reservation,
    ) -> Result<Admitted<CustodyDtoV1>> {
        let context = context_for_export(c, &mut r)?;
        prepare(
            &CustodyView {
                version: 1,
                boundary: c.now,
                context: &context,
                sampled_time: c.backbone.current_time,
                custody: self,
            },
            &mut r,
        )?;
        context::boundary(1, c.now, &context, c.backbone.current_time, c)?;
        validate(self, c)?;
        Ok(Admitted::new(
            CustodyDtoV1 {
                version: 1,
                boundary: c.now,
                context,
                sampled_time: c.backbone.current_time,
                custody: self.clone(),
            },
            r,
        ))
    }
}
impl CustodyDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: LawCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: CustodyWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            sampled_time: w.sampled_time,
            custody: w.custody,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub(crate) fn validate(&self, c: LawCheckpointContext<'_>) -> Result<()> {
        context::boundary(
            self.version,
            self.boundary,
            &self.context,
            self.sampled_time,
            c,
        )?;
        validate(&self.custody, c)?;
        Ok(())
    }
    pub fn cost(&self) -> Result<LawCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
}
impl Admitted<CustodyDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(self, c: LawCheckpointContext<'_>) -> Result<Admitted<CustodyCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(CustodyCandidate { data: d })
        })
    }
}

impl CustodyCandidate {
    pub fn custody(&self) -> &Custody {
        &self.data.custody
    }
}

/// Pure saved-reference equivalent of grip_strength/strain_seconds. This checks
/// the next existing grip calculation without constructing a temporary World.
/// It does not claim a lifetime bound after future occupation/status changes.
fn current_strain_seconds(
    c: LawCheckpointContext<'_>,
    prisoner: &ActorId,
    holders: &[ActorId],
) -> f64 {
    if holders.is_empty() {
        return 0.0;
    }
    let mut strength = match holders.len() {
        1 => 1.0,
        2 => 7.0,
        _ => 20.0,
    };
    for h in holders {
        strength *= c
            .backbone
            .characters
            .get(h)
            .and_then(|h| h.lore())
            .and_then(|p| p.occupation_id.as_deref())
            .map_or(1.0, |o| match o {
                "bailiff_and_gaoler" | "militia_and_soldier" => 1.7,
                "watchman_and_keeper" | "civic_officer" => 1.25,
                _ => 1.0,
            });
    }
    if let Some(p) = c.backbone.characters.get(prisoner) {
        let status = |k| {
            p.state
                .statuses
                .get(&k)
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0)
        };
        strength *= 1.0 + 1.5 * status(crate::character::StatusKind::Drunkenness);
        strength *= 1.0 + status(crate::character::StatusKind::Weariness);
    }
    STRAIN_BASE_SECONDS * strength
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notices::checkpoint::tests::{active, actor, person};
    #[test]
    fn saved_reference_grip_matches_runtime_and_rejects_overflow() {
        let mut w = active();
        let p = actor("player");
        let now = LogicalTime::new(10.0).unwrap();
        let r = w.custody.get(&p).unwrap();
        assert_eq!(
            current_strain_seconds(LawCheckpointContext::from_world(&w, now), &p, &r.holders)
                .to_bits(),
            super::super::strain_seconds(&w, &p, &r.holders).to_bits()
        );
        for n in 0..1500 {
            let id = format!("elite{n}");
            w.add_character(person(&id, 0.0, Some("bailiff_and_gaoler")));
            w.custody.grab(&p, actor(&id));
        }
        assert!(
            !super::super::strain_seconds(&w, &p, &w.custody.get(&p).unwrap().holders).is_finite()
        );
        let b = checkpoint::CheckpointBudget::default();
        let err = w
            .custody
            .export_checkpoint(
                LawCheckpointContext::from_world(&w, now),
                b.reserve(checkpoint::Cohort::SavePayload, 4096).unwrap(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("grip arithmetic"));
        assert_eq!(b.retained_bytes(), 0);
    }
}
