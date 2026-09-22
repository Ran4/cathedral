//! Captured sound TOML admission, pinned to the parser/container inventory in
//! plan/evidence/m3_sound_admission. No arbitrary Deserialize is admitted here.
use super::{AmbientRow, AmbientSound, CatalogFile, Sound, SoundCatalog, SoundRow, SoundStorage};
use crate::checkpoint::CheckpointBudget;
use serde_spanned::Spanned;
use std::{borrow::Cow, fmt, mem::size_of};
use toml::de::DeValue;
use toml_parser::{
    Source,
    lexer::Token,
    parser::{Event, EventKind, RecursionGuard, ValidateWhitespace},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundAdmissionError {
    Admission,
    InvalidDefinition,
}
impl fmt::Display for SoundAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Admission => "sound catalog admission refused",
            Self::InvalidDefinition => "invalid sound catalog definition",
        })
    }
}
impl std::error::Error for SoundAdmissionError {}
type Result<T> = std::result::Result<T, SoundAdmissionError>;
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(SoundAdmissionError::Admission)
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or(SoundAdmissionError::Admission)
}
fn token_peak(bytes: usize) -> Result<usize> {
    // Lexer::into_vec initially reserves source bytes, not token count. EOF
    // can force growth. 3N+4 includes old/new doubling and Vec's minimum4.
    add(mul(add(mul(3, bytes)?, 4)?, size_of::<Token>())?, 64)
}
fn shared_root() -> Result<usize> {
    add(
        size_of::<SoundStorage>(),
        add(mul(2, size_of::<usize>())?, 32)?,
    )
}

#[derive(Default, Debug)]
struct Shape {
    bytes: usize,
    tokens: usize,
    events: usize,
    keys: usize,
    scalars: usize,
    arrays: usize,
    containers: usize,
    text: usize,
}
impl Shape {
    fn event(&mut self, event: Event) -> Result<()> {
        self.events = add(self.events, 1)?;
        match event.kind() {
            EventKind::SimpleKey => self.keys = add(self.keys, 1)?,
            EventKind::Scalar => self.scalars = add(self.scalars, 1)?,
            EventKind::ArrayOpen | EventKind::ArrayTableOpen => self.arrays = add(self.arrays, 1)?,
            _ => {}
        }
        if matches!(
            event.kind(),
            EventKind::ArrayOpen | EventKind::ArrayTableOpen | EventKind::InlineTableOpen
        ) {
            self.containers = add(self.containers, 1)?;
        }
        if matches!(event.kind(), EventKind::SimpleKey | EventKind::Scalar) {
            let span = event.span();
            self.text = add(
                self.text,
                span.end()
                    .checked_sub(span.start())
                    .ok_or(SoundAdmissionError::Admission)?,
            )?;
        }
        Ok(())
    }

    fn preflight(source: &str) -> Result<Self> {
        // Caller has already admitted the complete token allocation. The
        // counting receiver and error flag allocate nothing. These wrappers and
        // depth match toml0.9.12's full parser; no accepted syntax is removed.
        let source = Source::new(source);
        let tokens = source.lex().into_vec();
        let mut shape = Self {
            bytes: source.input().len(),
            tokens: tokens.len(),
            ..Self::default()
        };
        let mut overflow = false;
        let mut count = |event| {
            overflow |= shape.event(event).is_err();
        };
        let mut whitespace = ValidateWhitespace::new(&mut count, source);
        let mut depth = RecursionGuard::new(&mut whitespace, 80);
        let mut invalid = false;
        toml_parser::parser::parse_document(&tokens, &mut depth, &mut |_| {
            invalid = true;
        });
        if overflow {
            return Err(SoundAdmissionError::Admission);
        }
        if invalid {
            return Err(SoundAdmissionError::InvalidDefinition);
        }
        Ok(shape)
    }

    fn peak(&self) -> Result<usize> {
        type Key = Spanned<Cow<'static, str>>;
        type Value = Spanned<DeValue<'static>>;
        let mut bytes = token_peak(self.bytes)?;
        // E is counted through the actual parser, including recovery events.
        // Initial event capacity is T even when E<T; growth overlap <=3E+4.
        bytes = add(
            bytes,
            add(
                mul(
                    add(self.tokens, add(mul(3, self.events)?, 4)?)?,
                    size_of::<Event>(),
                )?,
                64,
            )?,
        )?;
        // x86_64 Rust1.96 BTree: leaf prefix16, eleven key/value slots,
        // twelve edges. One full internal node per key overcounts sparse nodes.
        let node = add(
            add(16, mul(11, add(size_of::<Key>(), size_of::<Value>())?)?)?,
            add(mul(12, size_of::<usize>())?, 32)?,
        )?;
        bytes = add(bytes, mul(self.keys, node)?)?;
        // Every pushed AST value consumes a scalar/container open. Each array has
        // an array or array-table open. Include old/new Vec blocks/minima.
        bytes = add(
            bytes,
            mul(
                add(
                    mul(3, add(self.scalars, self.containers)?)?,
                    mul(4, self.arrays)?,
                )?,
                size_of::<Value>(),
            )?,
        )?;
        bytes = add(bytes, mul(64, self.arrays)?)?;
        // Nonempty active dotted-path vectors consume keys; at most K vectors.
        // Their allocation happens BEFORE the separate80-key path limit check.
        bytes = add(bytes, mul(mul(7, self.keys)?, size_of::<Key>())?)?;
        bytes = add(bytes, mul(64, self.keys)?)?;
        // Invalid short hex escapes can replace2encoded bytes by3UTF8 bytes.
        // Decoder output<=2S, hence6S growth overlap +8minimum per scalar/key.
        // Key clones:<=2S retained +2S transient. Valid typed strings:<=S.
        let strings = add(self.keys, self.scalars)?;
        bytes = add(bytes, add(mul(11, self.text)?, mul(8, strings)?)?)?;
        bytes = add(
            bytes,
            add(
                mul(64, strings)?,
                add(mul(64, self.keys)?, mul(32, self.scalars)?)?,
            )?,
        )?;
        // Closed schema: each row needs a table/sequence container. Charge wire/result
        // Vecs separately, even if collect can reuse the original allocation.
        let slots = add(mul(3, self.containers)?, 4)?;
        let row_sizes = add(
            add(size_of::<SoundRow>(), size_of::<AmbientRow>())?,
            add(size_of::<Sound>(), size_of::<AmbientSound>())?,
        )?;
        bytes = add(bytes, add(mul(slots, row_sizes)?, 256)?)?;
        // Diagnostics are dropped without Display. First-error Arc<str> copies
        // N bytes.32S covers escaped serde/custom double-string construction,
        // validation ids and context keys;8*1024 covers fixed descriptions,
        // expected-field lists, growth/copies, context Vec and block headers.
        bytes = add(bytes, add(self.bytes, add(mul(32, self.text)?, 8 * 1024)?)?)?;
        add(bytes, shared_root()?)
    }
}

fn string_cost(value: &str, capacity: usize) -> Result<usize> {
    debug_assert!(capacity >= value.len());
    add(capacity, 32)
}
fn retained(sounds: &Vec<Sound>, ambients: &Vec<AmbientSound>) -> Result<usize> {
    let mut bytes = add(
        shared_root()?,
        add(mul(sounds.capacity(), size_of::<Sound>())?, 32)?,
    )?;
    bytes = add(
        bytes,
        add(mul(ambients.capacity(), size_of::<AmbientSound>())?, 32)?,
    )?;
    for row in sounds {
        for value in [&row.sound_id, &row.sound_class, &row.heard, &row.sfx_prompt]
            .into_iter()
            .chain(row.seen.iter())
        {
            bytes = add(bytes, string_cost(value, value.capacity())?)?;
        }
    }
    for row in ambients {
        for value in [&row.sound_id, &row.sfx_prompt] {
            bytes = add(bytes, string_cost(value, value.capacity())?)?;
        }
    }
    Ok(bytes)
}

impl SoundCatalog {
    /// Admit the captured TOML's parser, closed wire shape and retained rows.
    /// Uses the complete existing TOML grammar; arbitrary Deserialize, world
    /// construction and later caller getter allocations are not admitted here.
    /// Borrowed source storage is the caller's responsibility (installed source
    /// capture admits it). Even a captured4MiB file can refuse under pressure.
    pub fn from_toml_str_admitted(source: &str, budget: &CheckpointBudget) -> Result<Self> {
        let mut lease = budget
            .reserve_running_overhead(token_peak(source.len())?)
            .map_err(|_| SoundAdmissionError::Admission)?;
        let shape = Shape::preflight(source)?; // temporary tokens drop on return
        lease
            .resize(shape.peak()?)
            .map_err(|_| SoundAdmissionError::Admission)?;
        let file: CatalogFile =
            toml::from_str(source).map_err(|_| SoundAdmissionError::InvalidDefinition)?;
        let (sounds, ambients) =
            Self::convert_rows(file).map_err(|_| SoundAdmissionError::InvalidDefinition)?;
        Self::validate_rows(&sounds, &ambients)
            .map_err(|_| SoundAdmissionError::InvalidDefinition)?;
        let retained = retained(&sounds, &ambients)?;
        if retained > lease.bytes() {
            return Err(SoundAdmissionError::Admission);
        }
        lease
            .resize(retained)
            .map_err(|_| SoundAdmissionError::Admission)?;
        // Arc allocation is pre-admitted; row ownership precedes lease at drop.
        Ok(Self::from_rows(sounds, ambients, Some(lease)))
    }

    pub fn admitted_storage_bytes(&self) -> usize {
        self.storage
            .as_ref()
            .and_then(|storage| storage.lease.as_ref())
            .map_or(0, |lease| lease.bytes())
    }
}

#[cfg(test)]
mod tests;
