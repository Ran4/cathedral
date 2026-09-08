//! Complete Round component authority, not a World/Engine checkpoint. Candidate
//! references resolve against the covered, unadopted backbone and exact geometry.
use super::*;
use crate::{
    checkpoint::{
        Admitted, CalendarAnchorV1, CheckpointError, ComponentCost, Reservation, Result, aggregate,
        serde_support::remote_adapters,
    },
    item::ItemCatalog,
    weather::ShelterMap,
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
use serde::{Deserialize, Serialize};
mod definitions;
pub(crate) mod records;
#[cfg(test)]
mod tests;
pub(super) mod validate;
const OWNER: &str = "round";
pub const MAX_PEOPLE: usize = 25_000;
pub const MAX_PLANS: usize = 25_000;
pub const MAX_LEGS: usize = 256;
pub const MAX_CATALOG_ROWS: usize = 25_000;
pub const DEFINITION_WORKING_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RoundCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub definition_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for RoundCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            definition_working_bytes: DEFINITION_WORKING_BYTES,
            peak_bytes: c.peak_bytes + DEFINITION_WORKING_BYTES,
        }
    }
}
fn admit_definitions(cost: ComponentCost, r: &mut Reservation) -> Result<RoundCost> {
    let cost = RoundCost::from(cost);
    if r.bytes() < cost.peak_bytes {
        r.resize(cost.peak_bytes)?;
    }
    Ok(cost)
}

#[derive(Clone, Copy)]
pub struct RoundCheckpointContext<'a> {
    pub(crate) backbone: BackboneRefs<'a>,
    pub(crate) catalog: &'a ItemCatalog,
    pub(crate) nav: Option<&'a NavData>,
    pub(crate) shelters: &'a ShelterMap,
}
impl<'a> RoundCheckpointContext<'a> {
    pub fn from_world(world: &'a World, nav: Option<&'a NavData>) -> Self {
        Self {
            backbone: BackboneRefs::from_world(world),
            catalog: &world.item_catalog,
            nav,
            shelters: &world.shelters,
        }
    }
    pub fn from_backbone(
        candidate: &'a BackboneCandidate,
        catalog: &'a ItemCatalog,
        nav: Option<&'a NavData>,
        shelters: &'a ShelterMap,
    ) -> Self {
        Self {
            backbone: candidate.references(),
            catalog,
            nav,
            shelters,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextV1 {
    #[serde(deserialize_with = "crate::checkpoint::serde_support::required_option")]
    nav: Option<[u8; 32]>,
    shelters: [u8; 32],
    catalog: [u8; 32],
    embedded_planners: [u8; 32],
}
fn hash<T: Serialize>(value: &T) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    struct Sink(Sha256);
    impl std::io::Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.update(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut s = Sink(Sha256::new());
    s.0.update(b"cathedral-round-context-v1\0");
    serde_json::to_writer(&mut s, value).expect("validated immutable context serializes");
    s.0.finalize().into()
}
impl ContextV1 {
    fn new(c: RoundCheckpointContext<'_>) -> Self {
        Self {
            nav: c.nav.map(NavData::checkpoint_fingerprint),
            shelters: hash(&c.shelters.shelters()),
            catalog: c.catalog.checkpoint_fingerprint(),
            embedded_planners: hash(&(ROUNDS_JSON, FOOD_JSON, HOMES_JSON)),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct RoundDtoV1 {
    version: u16,
    context: ContextV1,
    #[serde(with = "records::RoundV1")]
    round: Round,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    context: ContextV1,
    #[serde(with = "records::RoundV1")]
    round: Round,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    context: ContextV1,
    #[serde(with = "records::RoundV1")]
    round: &'a Round,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RoundCounts {
    pub characters: usize,
    pub people: usize,
    pub residents: usize,
    pub sources: usize,
    pub stalls: usize,
    pub production_plans: usize,
    pub stock_plans: usize,
    pub road_parties: usize,
    pub occupied_reservations: usize,
    pub destination_reservations: usize,
}
impl Round {
    pub fn checkpoint_round_cost(
        &self,
        context: RoundCheckpointContext<'_>,
        mut reservation: Reservation,
    ) -> Result<Admitted<RoundCost>> {
        let view = View {
            version: 1,
            context: ContextV1::new(context),
            round: self,
        };
        let cost = aggregate::prepare_export(&view, OWNER, &mut reservation)?;
        let cost = admit_definitions(cost, &mut reservation)?;
        Ok(Admitted::new(cost, reservation))
    }
    pub fn export_checkpoint(
        &self,
        context: RoundCheckpointContext<'_>,
        mut reservation: Reservation,
    ) -> Result<Admitted<RoundDtoV1>> {
        check(self.ladder_scratch.is_empty(), "incomplete ladder pass")?;
        // These notifications are covered authority; they are preserved even if
        // an Engine has not yet drained them. Whole-envelope flush validation is later.
        let view = View {
            version: 1,
            context: ContextV1::new(context),
            round: self,
        };
        let cost = aggregate::prepare_export(&view, OWNER, &mut reservation)?;
        admit_definitions(cost, &mut reservation)?;
        let dto = RoundDtoV1 {
            version: 1,
            context: view.context,
            round: self.clone(),
        };
        dto.validate(context)?;
        Ok(Admitted::new(dto, reservation))
    }
}
impl RoundDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut reservation: Reservation,
        context: RoundCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let wire: Wire = aggregate::decode_with_working(
            bytes,
            OWNER,
            &mut reservation,
            DEFINITION_WORKING_BYTES,
        )?;
        let dto = Self {
            version: wire.version,
            context: wire.context,
            round: wire.round,
        };
        dto.validate(context)?;
        Ok(Admitted::new(dto, reservation))
    }
    pub fn validate(&self, context: RoundCheckpointContext<'_>) -> Result<()> {
        check(self.version == 1, "unsupported component version")?;
        check(
            self.context == ContextV1::new(context),
            "immutable Round context mismatch",
        )?;
        validate_context_shape(context)?;
        validate::validate(&self.round, context)?;
        definitions::validate(&self.round, context)
    }
    pub fn cost(&self) -> Result<RoundCost> {
        aggregate::measure(self, OWNER).map(RoundCost::from)
    }
    pub fn counts(&self, context: RoundCheckpointContext<'_>) -> RoundCounts {
        let r = &self.round;
        RoundCounts {
            characters: context.backbone.characters.len(),
            people: r.people.len(),
            residents: r.resident_count(),
            sources: r.sources.len(),
            stalls: r.stalls.len(),
            production_plans: r.production_plans.len(),
            stock_plans: r.stock_plans.len(),
            road_parties: r.road_parties.len(),
            occupied_reservations: r.residents.reservations.occupied_count(),
            destination_reservations: r.residents.reservations.destination_count(),
        }
    }
}
#[derive(Debug)]
pub struct RoundCandidate {
    data: RoundDtoV1,
}
impl Admitted<RoundDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        context: RoundCheckpointContext<'_>,
    ) -> Result<Admitted<RoundCandidate>> {
        self.try_map(|data, _| {
            data.validate(context)?;
            Ok(RoundCandidate { data })
        })
    }
}
impl RoundCandidate {
    pub fn counts(&self, context: RoundCheckpointContext<'_>) -> RoundCounts {
        self.data.counts(context)
    }
    #[cfg(test)]
    pub(crate) fn round(&self) -> &Round {
        &self.data.round
    }
}
fn err(reason: impl Into<String>) -> CheckpointError {
    CheckpointError::new(OWNER, reason)
}
fn check(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(err(reason)) }
}

// Only declared pollen initially-due sentinels accept Never. Arbitrary invalid
// calendar values cannot become sentinels through JSON's null conversion.
struct PollenV1;
impl PollenV1 {
    fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        CalendarAnchorV1::from_legacy(*v)
            .map_err(serde::ser::Error::custom)?
            .serialize(s)
    }
    fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<f64, D::Error> {
        let v = CalendarAnchorV1::deserialize(d)?;
        v.validate().map_err(serde::de::Error::custom)?;
        Ok(v.legacy())
    }
}
remote_adapters!(pollen, f64, PollenV1);
mod evening {
    use super::*;
    #[derive(Serialize, Deserialize)]
    struct Leg(#[serde(with = "records::RoundLegV1")] RoundLeg);
    pub fn serialize<S: serde::Serializer>(
        v: &Option<(usize, RoundLeg)>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a>(usize, #[serde(with = "records::RoundLegV1")] &'a RoundLeg);
        v.as_ref().map(|(i, leg)| Ref(*i, leg)).serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Option<(usize, RoundLeg)>, D::Error> {
        Option::<(usize, Leg)>::deserialize(d).map(|v| v.map(|(i, l)| (i, l.0)))
    }
}
mod starts {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &BTreeMap<(ActorId, i64), u32>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for (k, n) in v {
            seq.serialize_element(&(k, n))?;
        }
        seq.end()
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<BTreeMap<(ActorId, i64), u32>, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = BTreeMap<(ActorId, i64), u32>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("unique producer/day counts")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut out = BTreeMap::new();
                while let Some((k, v)) = a.next_element()? {
                    if out.insert(k, v).is_some() {
                        return Err(serde::de::Error::custom("duplicate production start key"));
                    }
                }
                Ok(out)
            }
        }
        d.deserialize_seq(V)
    }
}

fn validate_context_shape(c: RoundCheckpointContext<'_>) -> Result<()> {
    if let Some(nav) = c.nav {
        let p = nav.resident_places();
        check(
            p.patches.len() <= 4096 && p.capacity() <= 25_000 && p.shelter_spots.len() <= 256,
            "resident geometry context count exceeds v1 policy",
        )?;
    }
    check(
        c.shelters.shelters().len() <= 256
            && c.shelters
                .shelters()
                .iter()
                .all(|s| s.polygon_xz.len() <= 1024),
        "shelter geometry context count exceeds v1 policy",
    )
}
