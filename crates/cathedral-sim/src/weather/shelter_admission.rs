//! Admission for the embedded installed shelter definition only. No caller
//! supplied source or arbitrary Deserialize implementation is trusted here.
use super::{Office, Shelter, ShelterDocument, ShelterMap, ShelterStorage};
use crate::checkpoint::{CheckpointBudget, Reservation};
use std::{fmt, mem::size_of, sync::Arc};

const SOURCE: &str = include_str!("../../../../assets/world/shelters.json");

/// Temporary parser/validation diagnostics are disposed under the lease.
/// The error crossing this boundary has no owned heap allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShelterAdmissionError {
    Admission,
    InvalidDefinition,
}
impl fmt::Display for ShelterAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Admission => "installed shelter admission refused",
            Self::InvalidDefinition => "invalid installed shelter definition",
        })
    }
}
impl std::error::Error for ShelterAdmissionError {}
type Result<T> = std::result::Result<T, ShelterAdmissionError>;

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(ShelterAdmissionError::Admission)
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or(ShelterAdmissionError::Admission)
}

#[derive(Debug)]
struct Shape {
    bytes: usize,
    objects: usize,
    arrays: usize,
    strings: usize,
    longest: usize,
}
impl Shape {
    // This is a storage upper-bound scan, not a JSON validator. Delimiters in
    // quoted text do not count; malformed unfinished quoted tails still do.
    // Stack-only scan and checked arithmetic run before any parser allocation.
    fn scan(source: &str) -> Result<Self> {
        let mut shape = Self {
            bytes: source.len(),
            objects: 0,
            arrays: 0,
            strings: 0,
            longest: 0,
        };
        let mut start = None;
        let mut escaped = false;
        for (index, byte) in source.bytes().enumerate() {
            if let Some(begin) = start {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    shape.longest = shape.longest.max(index - begin);
                    start = None;
                }
            } else {
                match byte {
                    b'"' => {
                        shape.strings = add(shape.strings, 1)?;
                        start = Some(add(index, 1)?);
                    }
                    b'{' => shape.objects = add(shape.objects, 1)?,
                    b'[' => shape.arrays = add(shape.arrays, 1)?,
                    _ => {}
                }
            }
        }
        if let Some(begin) = start {
            shape.longest = shape.longest.max(source.len() - begin);
        }
        Ok(shape)
    }

    fn peak(&self) -> Result<usize> {
        // A shelter accepts either an object or serde's struct-as-sequence.
        // A vertex requires an array, and every valid Office carries a string
        // tag (also for externally tagged object notation). These counts include
        // partially built rows and overcount document/field/vertex syntax.
        let rows = add(self.objects, self.arrays)?;
        // serde_core 1.0.228 Vec visitor starts with JSON's zero size hint and
        // pushes. Rust 1.96 Vec doubles; old+new capacity <=3*n+minimum, where
        // minimum is 4 for Shelter/[f64;2], 8 for the one-byte Office enum.
        // This bounds simultaneous storage, not cumulative allocator requests.
        let row_slots = add(mul(3, rows)?, 4)?;
        let vertex_slots = add(mul(3, self.arrays)?, mul(4, rows)?)?;
        let office_slots = add(mul(3, self.strings)?, mul(8, rows)?)?;
        let mut bytes = mul(3, self.bytes)?; // JSON escape/ignore scratch overlap
        bytes = add(bytes, add(self.bytes, mul(64, rows)?)?)?; // id/label text + headers
        bytes = add(bytes, mul(row_slots, size_of::<Shelter>())?)?;
        bytes = add(bytes, mul(vertex_slots, size_of::<[f64; 2]>())?)?;
        bytes = add(bytes, mul(office_slots, size_of::<Office>())?)?;
        bytes = add(bytes, mul(32, add(mul(4, rows)?, 2)?)?)?; // old/new Vec blocks
        // Audited x86_64 Rust 1.96 BTree node: 16-byte leaf prefix, 11 borrowed
        // &str keys, at most 12 pointer edges, plus allocator/control slack.
        // One full internal node per row overcounts the sparse first node too.
        let node = add(
            add(16, mul(11, size_of::<&str>())?)?,
            add(mul(12, size_of::<usize>())?, 32)?,
        )?;
        bytes = add(bytes, mul(rows, node)?)?;
        // Serde Debug escaping <=6*token bytes, formatter growth and overlapping
        // serde/validation errors; include unterminated tokens and duplicate ids.
        bytes = add(bytes, add(mul(32, self.longest)?, 4096)?)?;
        bytes = add(bytes, add(size_of::<ShelterDocument>(), 256)?)?;
        add(bytes, shared_roots()?)
    }
}

fn arc_block<T>() -> Result<usize> {
    add(size_of::<T>(), add(mul(2, size_of::<usize>())?, 32)?)
}
fn shared_roots() -> Result<usize> {
    add(arc_block::<ShelterStorage>()?, arc_block::<ShelterMap>()?)
}
fn retained(rows: &Vec<Shelter>) -> Result<usize> {
    let mut bytes = add(
        shared_roots()?,
        add(mul(rows.capacity(), size_of::<Shelter>())?, 32)?,
    )?;
    for row in rows {
        for capacity in [row.id.capacity(), row.label.capacity()] {
            bytes = add(bytes, add(capacity, 32)?)?;
        }
        bytes = add(
            bytes,
            add(mul(row.polygon_xz.capacity(), size_of::<[f64; 2]>())?, 32)?,
        )?;
        bytes = add(
            bytes,
            add(mul(row.offices.capacity(), size_of::<Office>())?, 32)?,
        )?;
    }
    Ok(bytes)
}

impl ShelterMap {
    /// Parse the same build-identified shelter artifact used by game startup.
    /// The returned outer Arc and all ShelterMap clones retain the rows' lease.
    /// Extra caller-created Arc wrappers need their own caller admission.
    /// Only this immutable definition is admitted, not Engine/World construction.
    pub fn installed_admitted(budget: &CheckpointBudget) -> Result<Arc<Self>> {
        Self::admit_source(budget, SOURCE)
    }

    // Private source seam permits malformed/pressure witnesses without exposing
    // a generic caller-source admitted API or changing the legacy JSON loader.
    fn admit_source(budget: &CheckpointBudget, source: &str) -> Result<Arc<Self>> {
        let peak = Shape::scan(source)?.peak()?;
        let mut lease: Reservation = budget
            .reserve_running_overhead(peak)
            .map_err(|_| ShelterAdmissionError::Admission)?;
        // lease is declared before rows, so all temporary/error owners disappear
        // first on every failure. parse_rows disposes serde and validation scratch.
        let rows =
            Self::parse_rows(source).map_err(|_| ShelterAdmissionError::InvalidDefinition)?;
        let retained = retained(&rows)?;
        if retained > lease.bytes() {
            return Err(ShelterAdmissionError::Admission);
        }
        lease
            .resize(retained)
            .map_err(|_| ShelterAdmissionError::Admission)?;
        // Both Arc blocks are included before either allocation occurs.
        Ok(Arc::new(Self {
            storage: Some(Arc::new(ShelterStorage {
                shelters: rows,
                lease: Some(lease),
            })),
        }))
    }

    /// Definition storage only, shared through every clone; zero for legacy maps.
    pub fn admitted_storage_bytes(&self) -> usize {
        self.storage
            .as_ref()
            .and_then(|storage| storage.lease.as_ref())
            .map_or(0, Reservation::bytes)
    }
}

#[cfg(test)]
mod tests;
