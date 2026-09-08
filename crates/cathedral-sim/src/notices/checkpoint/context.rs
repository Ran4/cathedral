//! Law reads accepted backbone references; frozen custody destinations are not
//! re-resolved against the current registry.
use super::*;
use crate::{
    clock::WorldTime,
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
#[derive(Clone, Copy)]
pub struct LawCheckpointContext<'a> {
    pub(crate) now: LogicalTime,
    pub(crate) backbone: BackboneRefs<'a>,
}
impl<'a> LawCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
        }
    }
    pub fn from_backbone(candidate: &'a BackboneCandidate, now: LogicalTime) -> Self {
        Self {
            now,
            backbone: candidate.references(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingV1 {
    places: [u8; 32],
    law_definition: [u8; 32],
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
                .filter(|n| *n <= 4 * 1024 * 1024)
                .ok_or_else(|| std::io::Error::other("law context byte limit"))?;
            self.hash.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink {
        hash: Sha256::new(),
        bytes: 0,
    };
    sink.hash.update(b"cathedral-law-context-v1\0");
    serde_json::to_writer(&mut sink, value)
        .map_err(|_| error("invalid or oversized law context"))?;
    Ok(sink.hash.finalize().into())
}
impl BindingV1 {
    pub(crate) fn new(c: LawCheckpointContext<'_>) -> Result<Self> {
        checkpoint::logical(OWNER, c.now.seconds())?;
        check(
            c.backbone.characters.len() <= MAX_NAMES
                && c.backbone.places.checkpoint_entry_count() <= MAX_NAMES,
            "law backbone context count",
        )?;
        #[derive(Serialize)]
        struct Places<'a>(
            #[serde(with = "crate::places::checkpoint::PlaceRegistryV1")]
            &'a crate::places::PlaceRegistry,
        );
        // Stream a bounded representation before the index-validation BTree
        // scratch; no loaded definition is parsed or owned here. Validation
        // still refuses every invalid finite/identity/index value before use.
        let places = digest(&Places(c.backbone.places))?;
        crate::places::checkpoint::validate(c.backbone.places, c.backbone.characters)?;
        use crate::custody as cu;
        let law_definition = digest(&(
            LAW_OCCUPATIONS,
            NOTICE_LIFE_GAME_DAYS,
            NOTICES_MAX_LIVE,
            WITNESSED_BREACH_GAME_DAYS,
            NOTICES_SHEET_MAX,
            cu::STATION_PLACE_NAMES,
            cu::STONE_HOUSE_PLACE_NAME,
            [
                cu::CUSTODY_REACH_M,
                cu::CUSTODY_LEASH_M,
                cu::COMMITTED_ROAM_M,
                cu::CUSTODY_ESCORT_CONTACT_M,
                cu::CUSTODY_TETHER_M,
                cu::STATION_ARRIVE_RADIUS_M,
                cu::STATION_HOLD_SECONDS,
                cu::STONE_HOUSE_HOLD_SECONDS,
                cu::CUSTODY_DEAD_MAN_SECONDS,
                cu::STRAIN_BASE_SECONDS,
            ],
            cu::CUSTODY_MAX_ARRESTS,
            cu::GAOL_FEE_SPARKS,
            "law-carrier-and-struggle-hash-v1",
        ))?;
        Ok(Self {
            places,
            law_definition,
        })
    }
}
pub(crate) fn boundary(
    version: u16,
    now: LogicalTime,
    binding: &BindingV1,
    time: Option<WorldTime>,
    c: LawCheckpointContext<'_>,
) -> Result<()> {
    check(version == 1, "unsupported law component version")?;
    check(now == c.now, "law boundary disagreement")?;
    checkpoint::logical(OWNER, now.seconds())?;
    check(*binding == BindingV1::new(c)?, "law context disagreement")?;
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
    check(same, "law sampled time disagrees with backbone")?;
    if let Some(t) = time {
        checkpoint::calendar(OWNER, t.game_days())?;
        check(
            t.fraction.is_finite()
                && (0.0..1.0).contains(&t.fraction)
                && t.weekday == crate::clock::Weekday::of_day(t.day)
                && t.office == WorldTime::from_game_days(t.fraction).office,
            "invalid law sampled time",
        )?;
    }
    Ok(())
}
