//! Full resolved table binding, including the supported flat constructor.
use super::*;
use serde::Serialize;
struct TableRef<'a>(&'a SalienceTable);
impl Serialize for TableRef<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            bands: Bands<'a>,
            ears: Ears<'a>,
            craft_own: f64,
            craft_other: f64,
            no_trade: f64,
            household: f64,
        }
        struct Bands<'a>(&'a BTreeMap<Topic, Band>);
        impl Serialize for Bands<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_seq(self.0.iter().map(|(k, v)| (*k, v.base, v.hedge_band)))
            }
        }
        struct Ears<'a>(&'a BTreeMap<Topic, Ear>);
        impl Serialize for Ears<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_seq(
                    self.0
                        .iter()
                        .map(|(k, v)| (*k, &v.occupations, v.multiplier)),
                )
            }
        }
        let v = self.0;
        Ref {
            bands: Bands(&v.bands),
            ears: Ears(&v.ears),
            craft_own: v.craft_own,
            craft_other: v.craft_other,
            no_trade: v.no_trade,
            household: v.household,
        }
        .serialize(s)
    }
}
pub(crate) fn fingerprint(v: &SalienceTable) -> crate::checkpoint::Result<[u8; 32]> {
    let valid = v.bands.values().all(|b| b.base.is_finite())
        && v.ears.values().all(|e| e.multiplier.is_finite())
        && [v.craft_own, v.craft_other, v.no_trade, v.household]
            .iter()
            .all(|v| v.is_finite());
    crate::knowledge::checkpoint::check(valid, "nonfinite salience context")?;
    crate::knowledge::checkpoint::context::digest(&TableRef(v))
}
