//! Whole-engine checkpoints (D06): the value a save slot carries, and the two
//! serde adapters the real runtime types need in order to carry it.
//!
//! There are no save DTOs. The saved state is the engine's own types with
//! `Serialize`/`Deserialize` derived on them; [`crate::Engine::checkpoint`]
//! captures it and [`crate::Engine::restore`] installs it, and both are
//! exhaustive over the engine's fields, so a new field does not compile until
//! somebody decides whether it is saved. Immutable assets and services are
//! `#[serde(skip)]` and rebound by the restore.

use std::{collections::BTreeMap, fmt, sync::Arc};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{areas::AreaMap, clock::Office, engine::EngineState, nav::NavData, weather::ShelterMap};

/// The save format's version. Bumped whenever a saved type changes shape;
/// there are no migrations, so a save of any other version is refused.
pub const CHECKPOINT_VERSION: u32 = 1;

/// One captured simulation: everything a restored engine needs to continue
/// exactly where this one was.
#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: u32,
    /// [`geometry_fingerprint`] of the map the state's indices point into
    /// (nav nodes, resident spots, shelters, areas). A save made against other
    /// geometry is refused rather than replayed onto the wrong streets.
    pub geometry: u64,
    /// The simulation `now` of the capture. The host continues its clock from
    /// here; time with the game closed does not pass (D06).
    pub now: f64,
    pub label: CheckpointLabel,
    pub(crate) state: EngineState,
}

/// What a slot list shows for a save, without decoding the state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointLabel {
    pub day: i64,
    pub office: Office,
    /// The named area the player stood in, if any.
    pub area: Option<String>,
}

impl fmt::Display for CheckpointLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "day {}, {}", self.day, self.office.label())?;
        if let Some(area) = &self.area {
            write!(f, ", {area}")?;
        }
        Ok(())
    }
}

/// Why a checkpoint was not installed. The caller keeps whatever engine it
/// already had.
#[derive(Debug, Clone, PartialEq)]
pub enum RestoreError {
    Version { found: u32 },
    Geometry { saved: u64, current: u64 },
    Invalid(String),
}

impl fmt::Display for RestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version { found } => write!(
                f,
                "save format {found} is not this build's {CHECKPOINT_VERSION}"
            ),
            Self::Geometry { saved, current } => write!(
                f,
                "the save was made on other city geometry ({saved:016x}, now {current:016x})"
            ),
            Self::Invalid(reason) => write!(f, "the saved world is not valid: {reason}"),
        }
    }
}

impl std::error::Error for RestoreError {}

/// A cheap, build-independent hash of the geometry a saved world indexes into:
/// the nav nodes, places, doors and resident spots, the named areas and the
/// shelters. FNV-1a rather than `DefaultHasher`, whose algorithm may change
/// between compilers — a rebuild must not invalidate every save.
pub fn geometry_fingerprint(nav: Option<&NavData>, areas: &AreaMap, shelters: &ShelterMap) -> u64 {
    let mut hash = Fnv::new();
    if let Some(nav) = nav {
        hash.write(&(nav.node_count() as u64).to_le_bytes());
        for node in 0..nav.node_count() {
            for value in nav.node_xz(node) {
                hash.write(&value.to_bits().to_le_bytes());
            }
        }
        for place in nav.places() {
            hash.write(place.name.as_bytes());
            hash.write(&(place.node as u64).to_le_bytes());
        }
        for door in nav.doors() {
            hash.write(door.building.as_bytes());
            hash.write(&(door.node as u64).to_le_bytes());
        }
        let resident = nav.resident_places();
        for patch in &resident.patches {
            hash.write(patch.id.as_bytes());
            hash.write(&(patch.spots.len() as u64).to_le_bytes());
        }
        hash.write(&(resident.shelter_spots.len() as u64).to_le_bytes());
    }
    hash.write(&serde_json::to_vec(areas).unwrap_or_default());
    for shelter in shelters.shelters() {
        hash.write(&serde_json::to_vec(shelter).unwrap_or_default());
    }
    hash.0
}

/// FNV-1a's construction: the one hash whose values a save may carry. Fed
/// bytes, never `Hash` impls, so nothing in std can change what it returns.
/// Its multiplier is not FNV's published prime (`0x100_0000_01B3`); saves
/// already carry fingerprints made with this one, so it stays as it is.
pub(crate) struct Fnv(u64);

impl Fnv {
    pub(crate) fn new() -> Self {
        Self(0xCBF2_9CE4_8422_2325)
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x1_0000_0000_01B3);
        }
    }

    /// A string and a terminator, so `("ab", "c")` and `("a", "bc")` differ.
    pub(crate) fn write_str(&mut self, text: &str) {
        self.write(text.as_bytes());
        self.write(&[0xFF]);
    }

    pub(crate) fn finish(&self) -> u64 {
        self.0
    }
}

/// `#[serde(with)]` for an `f64` whose `NEG_INFINITY` means "never happened":
/// JSON has no infinities, so that one value travels as `null`.
pub(crate) mod never {
    use super::*;

    pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if *value == f64::NEG_INFINITY {
            serializer.serialize_none()
        } else {
            serializer.serialize_some(value)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(f64::NEG_INFINITY))
    }
}

/// `#[serde(with)]` for a map whose keys are not strings (tuples, enums with
/// data): JSON object keys must be, so it travels as `[[key, value], …]`.
pub(crate) mod pairs {
    use super::*;

    pub fn serialize<K: Serialize, V: Serialize, S: Serializer>(
        map: &BTreeMap<K, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(map)
    }

    pub fn deserialize<'de, K, V, D>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Ok(Vec::<(K, V)>::deserialize(deserializer)?
            .into_iter()
            .collect())
    }
}

/// [`pairs`] for a map shared copy-on-write behind an [`Arc`].
pub(crate) mod shared_pairs {
    use super::*;

    pub fn serialize<K: Serialize, V: Serialize, S: Serializer>(
        map: &Arc<BTreeMap<K, V>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(map.as_ref())
    }

    pub fn deserialize<'de, K, V, D>(deserializer: D) -> Result<Arc<BTreeMap<K, V>>, D::Error>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        pairs::deserialize(deserializer).map(Arc::new)
    }
}
