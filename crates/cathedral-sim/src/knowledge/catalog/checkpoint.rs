//! Hash the full installed catalog without implementing runtime source serde.
use super::*;
use serde::Serialize;
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum SourceRef<'a> {
    Authored,
    Custody(&'a ActorId),
    Item(&'a ItemId),
    QuestPhase { quest: &'a str, phase: u8 },
}
struct CatalogRef<'a>(&'a FactCatalog);
impl Serialize for CatalogRef<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Row<'a> {
            id: &'a FactId,
            topic: Topic,
            said: &'a str,
            own: &'a BTreeMap<ActorId, String>,
            subject: &'a [ActorId],
            seeded: &'a BTreeSet<ActorId>,
            place: &'a Option<crate::ids::AreaId>,
            day: Option<i64>,
            decays: bool,
            #[serde(with = "crate::knowledge::checkpoint::records::GarbleV1")]
            garble: GarbleMask,
            source: SourceRef<'a>,
        }
        s.collect_seq(self.0.specs.iter().map(|v| Row {
            id: &v.id,
            topic: v.topic,
            said: &v.said,
            own: &v.own,
            subject: &v.subject,
            seeded: &v.seeded,
            place: &v.place,
            day: v.day,
            decays: v.decays,
            garble: v.garble,
            source: match &v.source {
                FactSourceSpec::Authored => SourceRef::Authored,
                FactSourceSpec::Custody(a) => SourceRef::Custody(a),
                FactSourceSpec::Item(i) => SourceRef::Item(i),
                FactSourceSpec::QuestPhase { quest, phase } => SourceRef::QuestPhase {
                    quest,
                    phase: *phase,
                },
            },
        }))
    }
}
pub(crate) fn fingerprint(c: &FactCatalog) -> crate::checkpoint::Result<[u8; 32]> {
    crate::knowledge::checkpoint::check(c.specs.len() <= 4096, "fact catalog count limit")?;
    // The installed catalog has already passed its private constructor. Full
    // resolved content, including failed-to-seed rows and sealed bindings, is
    // hashed; no catalog seeding or template re-resolution occurs here.
    crate::knowledge::checkpoint::context::digest(&CatalogRef(c))
}
