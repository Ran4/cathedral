//! Exact borrowed definition bindings before adoption; never reseed a catalog.
use super::*;
use crate::{
    clock::WorldTime,
    weather::ShelterMap,
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
#[derive(Clone, Copy)]
pub struct MarksCheckpointContext<'a> {
    pub(crate) now: LogicalTime,
    pub(crate) backbone: BackboneRefs<'a>,
    pub(crate) catalog: &'a MarkCatalog,
    pub(crate) shelters: &'a ShelterMap,
}
impl<'a> MarksCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
            catalog: &w.mark_catalog,
            shelters: &w.shelters,
        }
    }
    pub fn from_backbone(
        candidate: &'a BackboneCandidate,
        now: LogicalTime,
        catalog: &'a MarkCatalog,
        shelters: &'a ShelterMap,
    ) -> Self {
        Self {
            now,
            backbone: candidate.references(),
            catalog,
            shelters,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingV1 {
    places: [u8; 32],
    catalog: [u8; 32],
    shelters: [u8; 32],
    definition: [u8; 32],
}
fn digest<T: Serialize>(value: &T) -> Result<[u8; 32]> {
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
                .filter(|n| *n <= 4 * 1024 * 1024)
                .ok_or_else(|| std::io::Error::other("marks context byte limit"))?;
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
    s.hash.update(b"cathedral-marks-context-v1\0");
    serde_json::to_writer(&mut s, value)
        .map_err(|_| error("invalid or oversized marks context"))?;
    Ok(s.hash.finalize().into())
}
impl BindingV1 {
    pub(crate) fn new(c: MarksCheckpointContext<'_>) -> Result<Self> {
        checkpoint::logical(OWNER, c.now.seconds())?;
        check(
            c.backbone.characters.len() <= MAX_NAMES
                && c.backbone.places.checkpoint_entry_count() <= MAX_NAMES,
            "marks backbone context count",
        )?;
        #[derive(Serialize)]
        struct Places<'a>(
            #[serde(with = "crate::places::checkpoint::PlaceRegistryV1")]
            &'a crate::places::PlaceRegistry,
        );
        // Bounded streaming before any sparse registry validation scratch.
        let places = digest(&Places(c.backbone.places))?;
        crate::places::checkpoint::validate(c.backbone.places, c.backbone.characters)?;
        check(c.catalog.kinds.len() == 3, "marks catalog kind count")?;
        for s in c.catalog.kinds.values() {
            for t in [&s.label, &s.meaning, &s.faint_label] {
                text(t)?;
            }
            if let Some(t) = &s._places_doc {
                text(t)?;
            }
            check(
                !s.anchors.is_empty() && s.anchors.len() <= 1024 && s.places.len() <= MAX_NAMES,
                "marks catalog collection count",
            )?;
            for (k, v) in &s.places {
                text(k)?;
                text(v)?;
            }
            check(
                [
                    s.half_life_days_dry,
                    s.half_life_days_wet,
                    s.sheltered_multiplier,
                    s.faint_below,
                    s.gone_below,
                ]
                .iter()
                .all(|n| n.is_finite()),
                "nonfinite marks definition",
            )?;
            check(
                s.half_life_days_wet > 0.0
                    && s.half_life_days_wet <= s.half_life_days_dry
                    && s.sheltered_multiplier >= 1.0
                    && s.gone_below > 0.0
                    && s.gone_below < s.faint_below
                    && s.faint_below <= 1.0,
                "invalid marks definition",
            )?;
        }
        #[derive(Serialize)]
        struct Catalog<'a>(#[serde(with = "records::CatalogV1")] &'a MarkCatalog);
        let catalog = digest(&Catalog(c.catalog))?;
        let rows = c.shelters.shelters();
        check(rows.len() <= 256, "marks shelter count")?;
        for row in rows {
            id(&row.id)?;
            text(&row.label)?;
            check(
                row.polygon_xz.len() <= 1024 && row.offices.len() <= 1024,
                "marks shelter collection count",
            )?;
            check(
                row.spread_radius_m.is_finite(),
                "marks shelter nonfinite spread",
            )?;
            for p in &row.polygon_xz {
                crate::character::checkpoint::point(crate::math::Vec3::new(p[0], 0.0, p[1]))?;
            }
        }
        let shelters = digest(&rows)?;
        let definition = digest(&(
            MARKS_MAX,
            TALLY_STROKES_MAX,
            SWEEPS_PER_GAME_DAY,
            MARK_NOTICE_RADIUS_M,
            crate::actions::CHALK_REACH_M,
            crate::notices::CROSS_AFTER_GAME_DAYS,
            "marks-quantized-decay-and-id-eviction-v1",
        ))?;
        Ok(Self {
            places,
            catalog,
            shelters,
            definition,
        })
    }
}
pub(crate) fn boundary(
    version: u16,
    now: LogicalTime,
    binding: &BindingV1,
    time: Option<WorldTime>,
    c: MarksCheckpointContext<'_>,
) -> Result<()> {
    check(version == 1, "unsupported marks component version")?;
    check(now == c.now, "marks boundary disagreement")?;
    check(*binding == BindingV1::new(c)?, "marks context disagreement")?;
    let same = match (time, c.backbone.current_time) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.day == b.day
                && a.weekday == b.weekday
                && a.office == b.office
                && a.fraction.to_bits() == b.fraction.to_bits()
        }
        _ => false,
    };
    check(same, "marks sampled time disagrees with backbone")?;
    if let Some(t) = time {
        checkpoint::calendar(OWNER, t.game_days())?;
        check(
            t.fraction.is_finite()
                && (0.0..1.0).contains(&t.fraction)
                && t.weekday == crate::clock::Weekday::of_day(t.day)
                && t.office == WorldTime::from_game_days(t.fraction).office,
            "invalid marks sampled time",
        )?;
    }
    Ok(())
}
