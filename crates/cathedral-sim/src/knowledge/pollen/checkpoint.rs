//! Exact area accelerator wire. The global ward grid is immutable and derived
//! solely from compiled homes + constants; its lazy cache warmth is not state.
use super::*;
use serde::{Deserialize, Serialize};
pub(crate) struct AdjacencyV1;
impl AdjacencyV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &AreaAdjacency,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        struct Row<'a>(&'a [AreaKey]);
        impl Serialize for Row<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_seq(self.0.iter().map(|k| k.0))
            }
        }
        s.collect_seq(v.neighbours.iter().map(|r| Row(r)))
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<AreaAdjacency, D::Error> {
        let rows = Vec::<Vec<u16>>::deserialize(d)?;
        Ok(AreaAdjacency {
            neighbours: rows
                .into_iter()
                .map(|r| r.into_iter().map(AreaKey).collect())
                .collect(),
        })
    }
}
pub(crate) fn validate(
    v: &AreaAdjacency,
    map: &AreaMap,
    initialized: bool,
) -> crate::checkpoint::Result<()> {
    use crate::knowledge::checkpoint::check;
    // Default is a source-valid uninitialized accelerator in a bare World.
    // Preserve it exactly; only Engine requires its constructor-built shape.
    if v.neighbours.is_empty() && !initialized {
        return Ok(());
    }
    check(
        v.neighbours.len() == map.areas.len(),
        "area adjacency shape disagreement",
    )?;
    for (index, row) in v.neighbours.iter().enumerate() {
        check(row.len() <= 5, "area adjacency row count")?;
        let mut seen = BTreeSet::new();
        for key in row {
            check(
                usize::from(key.0) < map.areas.len()
                    && usize::from(key.0) != index
                    && seen.insert(*key),
                "invalid area adjacency key",
            )?;
        }
    }
    check(
        *v == AreaAdjacency::build(map),
        "area adjacency derivation disagreement",
    )
}
pub(crate) fn rows(v: &AreaAdjacency) -> usize {
    v.neighbours.len()
}
/// Re-derive only the eight immutable marker coordinates. The caller retains
/// the separate definition allowance; this never initializes either singleton.
pub(crate) fn centroids() -> crate::checkpoint::Result<BTreeMap<PlanningWard, Vec3>> {
    use crate::{
        checkpoint::aggregate,
        knowledge::checkpoint::{DEFINITION_WORKING_BYTES, check},
    };
    let cost = aggregate::inspect(crate::homes::HOMES_JSON.as_bytes())?;
    check(
        128 * 1024 + 2 * cost.expanded_upper_bytes + 3 * cost.encoded_bytes
            <= DEFINITION_WORKING_BYTES,
        "installed homes scratch exceeds admitted policy",
    )?;
    let mut sums: BTreeMap<PlanningWard, (Vec3, usize)> = BTreeMap::new();
    // ward_map keeps homes::ward_marks's district/x/z order, which is also the
    // accumulation order in the ordinary ward_centroids initializer.
    for ([x, z], ward) in crate::crowd::ward_map() {
        let (sum, count) = sums.entry(ward).or_insert((Vec3::ZERO, 0));
        *sum += Vec3::new(x, 0.0, z);
        *count += 1;
    }
    Ok(sums
        .into_iter()
        .map(|(ward, (sum, count))| (ward, sum / count as f64))
        .collect())
}

#[cfg(test)]
pub(crate) fn layout_maximum_adjacency() {
    let map = AreaMap {
        areas: (0..512)
            .map(|i| crate::areas::Area {
                id: format!("area_{i}"),
                label: format!("Area {i}"),
                boxes: vec![crate::areas::AreaBox {
                    min_m: Vec3::new((i % 32) as f64 * 0.2, 0.0, (i / 32) as f64 * 0.2),
                    max_m: Vec3::new(
                        (i % 32) as f64 * 0.2 + 0.1,
                        1.0,
                        (i / 32) as f64 * 0.2 + 0.1,
                    ),
                }],
            })
            .collect(),
        ..Default::default()
    };
    map.validate().unwrap();
    let adjacency = AreaAdjacency::build(&map);
    let capacity: usize = adjacency.neighbours.iter().map(Vec::capacity).sum();
    let maximum = adjacency
        .neighbours
        .iter()
        .map(Vec::capacity)
        .max()
        .unwrap();
    let retained = adjacency.neighbours.capacity() * std::mem::size_of::<Vec<AreaKey>>()
        + capacity * std::mem::size_of::<AreaKey>();
    println!(
        "maximum_adjacency_rows={} row_max_capacity={} retained_bytes={}",
        adjacency.neighbours.len(),
        maximum,
        retained
    );
    // Even retaining all pre-truncation nearest records through in-place Vec
    // collection fits the combined, sequential definition/geometry allowance.
    assert!(
        retained + 2 * 512 * 16 + 4096 <= crate::knowledge::checkpoint::VALIDATION_WORKING_BYTES
    );
}
pub(crate) fn fingerprint() -> crate::checkpoint::Result<[u8; 32]> {
    // Do not call ward_grid()/ward_centroids(): a cold singleton would parse
    // homes and bake an accelerator during validation. Exact compiled input and
    // formula constants are bound here; M2a1's behavior/build manifest binds code.
    crate::knowledge::checkpoint::context::digest(&(
        include_str!("../../../../../assets/world/homes.json"),
        CITY_MIN_X,
        CITY_MIN_Z,
        CITY_SPAN_X,
        CITY_SPAN_Z,
        WARD_CELL_M,
        "ward-marks-sort-nearest-corners-v1",
    ))
}
