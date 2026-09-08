//! Borrowed installed context, available before any World adoption.
use super::*;
use crate::{
    areas::AreaMap,
    timeline::LogicalTime,
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
use serde::{Deserialize, Serialize};
pub const MAX_AREAS: usize = 512;
pub const MAX_AREA_BOXES: usize = 4096;
const MAX_CONTEXT_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, Copy)]
pub struct KnowledgeCheckpointContext<'a> {
    pub(crate) now: LogicalTime,
    pub(crate) backbone: BackboneRefs<'a>,
    pub(crate) areas: &'a AreaMap,
    pub(crate) catalog: &'a FactCatalog,
    pub(crate) salience: &'a SalienceTable,
}
impl<'a> KnowledgeCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
            areas: &w.area_map,
            catalog: &w.fact_catalog,
            salience: &w.salience,
        }
    }
    pub fn from_backbone(
        candidate: &'a BackboneCandidate,
        now: LogicalTime,
        areas: &'a AreaMap,
        catalog: &'a FactCatalog,
        salience: &'a SalienceTable,
    ) -> Self {
        Self {
            now,
            backbone: candidate.references(),
            areas,
            catalog,
            salience,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingV1 {
    areas: [u8; 32],
    catalog: [u8; 32],
    salience: [u8; 32],
    ward_definition: [u8; 32],
    household_doors: [u8; 32],
}
pub(crate) fn digest<T: Serialize>(value: &T) -> Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    struct Sink {
        hash: Sha256,
        bytes: usize,
    }
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_CONTEXT_BYTES)
                .ok_or_else(|| std::io::Error::other("knowledge context byte limit"))?;
            self.hash.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut s = Sink {
        hash: Sha256::new(),
        bytes: 0,
    };
    s.hash.update(b"cathedral-knowledge-context-v1\0");
    serde_json::to_writer(&mut s, value)
        .map_err(|_| error("invalid or oversized knowledge context"))?;
    Ok(s.hash.finalize().into())
}
impl BindingV1 {
    /// Caller has already admitted fixed validation scratch. No context parser
    /// or global ward-grid bake is run here.
    pub(crate) fn new(c: KnowledgeCheckpointContext<'_>) -> Result<Self> {
        checkpoint::logical(OWNER, c.now.seconds())?;
        check(
            c.backbone.characters.len() <= MAX_NAMES,
            "backbone character count",
        )?;
        check(c.areas.areas.len() <= MAX_AREAS, "area context count")?;
        let mut boxes = 0usize;
        for a in &c.areas.areas {
            id(&a.id)?;
            text(&a.label)?;
            boxes = boxes
                .checked_add(a.boxes.len())
                .ok_or_else(|| error("area box count overflow"))?;
            check(boxes <= MAX_AREA_BOXES, "area context box count")?;
            for b in &a.boxes {
                crate::character::checkpoint::point(b.min_m)?;
                crate::character::checkpoint::point(b.max_m)?;
            }
        }
        c.areas
            .validate()
            .map_err(|_| error("invalid area context geometry or identity"))?;
        check(
            c.backbone.household_doors.len() <= MAX_NAMES,
            "household door context count",
        )?;
        for (who, point) in c.backbone.household_doors {
            id(who.as_str())?;
            check(
                point.is_finite() && point.abs().max_element() <= 1_000_000.0,
                "invalid household door context point",
            )?;
        }
        struct Doors<'a>(&'a BTreeMap<ActorId, crate::math::Vec3>);
        impl Serialize for Doors<'_> {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                s.collect_seq(self.0.iter().map(|(id, p)| (id, p.to_array())))
            }
        }
        Ok(Self {
            areas: digest(c.areas)?,
            catalog: super::super::catalog::checkpoint::fingerprint(c.catalog)?,
            salience: super::super::salience::checkpoint::fingerprint(c.salience)?,
            ward_definition: super::super::pollen::checkpoint::fingerprint()?,
            household_doors: digest(&Doors(c.backbone.household_doors))?,
        })
    }
}
