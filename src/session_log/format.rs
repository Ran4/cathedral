//! Bounded tracing capture: no String/Value/Map or default fmt-layer scratch.
use super::bounded::{Draft, Refusal, SinkSender, SliceWriter, Ticket};
use bevy::log::tracing;
use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Serialize, Serializer};
use std::{fmt, io::Write, time::Instant};

const FIELD_COUNT: usize = 32;
const FIELD_NAME_BYTES: usize = 128;
#[derive(Clone, Copy)]
enum FieldValue {
    Text(usize, usize),
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
}
#[derive(Clone, Copy)]
struct Field {
    name: &'static str,
    value: FieldValue,
}
pub(crate) struct Collector<'a> {
    arena: &'a mut [u8],
    used: usize,
    message: (usize, usize),
    fields: [Option<Field>; FIELD_COUNT],
    count: usize,
    failed: bool,
}
impl<'a> Collector<'a> {
    pub(crate) fn new(arena: &'a mut [u8]) -> Self {
        Self {
            arena,
            used: 0,
            message: (0, 0),
            fields: [None; FIELD_COUNT],
            count: 0,
            failed: false,
        }
    }
    fn allowed(&mut self, name: &str) -> bool {
        if self.failed
            || name.len() > FIELD_NAME_BYTES
            || name != "message" && self.count == FIELD_COUNT
        {
            self.failed = true;
            false
        } else {
            true
        }
    }
    fn text(&mut self, field: &tracing::field::Field, args: fmt::Arguments<'_>) {
        let name = field.name();
        if !self.allowed(name) {
            return;
        }
        let start = self.used;
        let mut out = Arena {
            bytes: &mut self.arena[self.used..],
            len: 0,
        };
        if fmt::write(&mut out, args).is_err() {
            self.failed = true;
            return;
        }
        self.used += out.len;
        if name == "message" {
            self.message = (start, self.used);
        } else {
            self.insert(name, FieldValue::Text(start, self.used));
        }
    }
    fn insert(&mut self, name: &'static str, value: FieldValue) {
        if !self.allowed(name) {
            return;
        }
        self.fields[self.count] = Some(Field { name, value });
        self.count += 1;
    }
    pub(crate) fn finish(self) -> Result<Captured, Refusal> {
        if self.failed {
            Err(Refusal::Oversized)
        } else {
            Ok(Captured {
                message: self.message,
                fields: self.fields,
                count: self.count,
            })
        }
    }
}
struct Arena<'a> {
    bytes: &'a mut [u8],
    len: usize,
}
impl fmt::Write for Arena<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if text.len() > self.bytes.len() - self.len {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
        Ok(())
    }
}
impl tracing::field::Visit for Collector<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.text(field, format_args!("{value:?}"));
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.text(field, format_args!("{value}"));
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == "message" {
            self.text(field, format_args!("{value}"));
        } else {
            self.insert(field.name(), FieldValue::I64(value));
        }
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "message" {
            self.text(field, format_args!("{value}"));
        } else {
            self.insert(field.name(), FieldValue::U64(value));
        }
    }
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if field.name() == "message" {
            self.text(field, format_args!("{value}"));
        } else {
            self.insert(field.name(), FieldValue::F64(value));
        }
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if field.name() == "message" {
            self.text(field, format_args!("{value}"));
        } else {
            self.insert(field.name(), FieldValue::Bool(value));
        }
    }
}
pub(crate) struct Captured {
    message: (usize, usize),
    fields: [Option<Field>; FIELD_COUNT],
    count: usize,
}
struct Fields<'a> {
    captured: &'a Captured,
    arena: &'a [u8],
}
impl Serialize for Fields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.captured.count))?;
        for field in self.captured.fields[..self.captured.count].iter().flatten() {
            match field.value {
                FieldValue::Text(start, end) => map.serialize_entry(
                    field.name,
                    std::str::from_utf8(&self.arena[start..end]).unwrap(),
                )?,
                FieldValue::I64(v) => map.serialize_entry(field.name, &v)?,
                FieldValue::U64(v) => map.serialize_entry(field.name, &v)?,
                FieldValue::F64(v) => map.serialize_entry(field.name, &v)?,
                FieldValue::Bool(v) => map.serialize_entry(field.name, &v)?,
            }
        }
        map.end()
    }
}
struct Body<'a> {
    source: &'a str,
    level: &'a str,
    target: Option<&'a str>,
    message: &'a str,
    fields: Option<Fields<'a>>,
}
impl Serialize for Body<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct(
            "LogRecord",
            3 + usize::from(self.target.is_some()) + usize::from(self.fields.is_some()),
        )?;
        out.serialize_field("source", self.source)?;
        out.serialize_field("level", self.level)?;
        if let Some(target) = self.target {
            out.serialize_field("target", target)?;
        }
        out.serialize_field("message", self.message)?;
        if let Some(fields) = &self.fields {
            out.serialize_field("fields", fields)?;
        }
        out.end()
    }
}
pub(crate) fn line(
    sink: &SinkSender,
    source: &str,
    level: &str,
    message: &str,
    evidence: bool,
    deadline: Option<Instant>,
) -> Result<Ticket, Refusal> {
    if source.len() > 64 || level.len() > 16 || message.len() > super::bounded::RECORD_BYTES {
        sink.refuse(Refusal::Oversized);
        return Err(Refusal::Oversized);
    }
    let mut draft = sink.begin(evidence, deadline)?;
    let (_, output) = draft.parts();
    let mut out = SliceWriter::new(output);
    if serde_json::to_writer(
        &mut out,
        &Body {
            source,
            level,
            target: None,
            message,
            fields: None,
        },
    )
    .is_err()
    {
        sink.refuse(Refusal::Oversized);
        return Err(Refusal::Oversized);
    }
    let len = out.len;
    draft.set_body_len(len);
    sink.commit(draft, true)
}
pub(crate) fn event(
    sink: &SinkSender,
    event: &tracing::Event<'_>,
    json: bool,
) -> Result<Ticket, Refusal> {
    let mut draft = sink.begin(false, None)?;
    let mut collector = Collector::new(draft.scratch());
    event.record(&mut collector);
    let captured = match collector.finish() {
        Ok(c) => c,
        Err(e) => {
            sink.refuse(e);
            return Err(e);
        }
    };
    encode_event(sink, draft, event.metadata(), captured, json)
}
fn encode_event(
    sink: &SinkSender,
    mut draft: Draft,
    metadata: &tracing::Metadata<'_>,
    captured: Captured,
    json: bool,
) -> Result<Ticket, Refusal> {
    if metadata.target().len() > 256 {
        sink.refuse(Refusal::Oversized);
        return Err(Refusal::Oversized);
    }
    let (arena, output) = draft.parts();
    let message = std::str::from_utf8(&arena[captured.message.0..captured.message.1]).unwrap();
    let fields = Fields {
        captured: &captured,
        arena,
    };
    let mut out = SliceWriter::new(output);
    let result = if json {
        serde_json::to_writer(
            &mut out,
            &Body {
                source: "rust",
                level: metadata.level().as_str(),
                target: Some(metadata.target()),
                message,
                fields: (captured.count != 0).then_some(fields),
            },
        )
        .map_err(io_error)
    } else {
        write!(
            out,
            "{} {}: {}",
            metadata.level(),
            metadata.target(),
            message
        )
        .and_then(|_| {
            if captured.count == 0 {
                Ok(())
            } else {
                out.write_all(b" ")
                    .and_then(|_| serde_json::to_writer(&mut out, &fields).map_err(io_error))
            }
        })
    };
    if result.is_err() {
        sink.refuse(Refusal::Oversized);
        return Err(Refusal::Oversized);
    }
    let len = out.len;
    draft.set_body_len(len);
    if json {
        sink.commit(draft, true)
    } else {
        // Console serialization used the same reserved body area; move it into
        // a newline-terminated plain record without allocating another buffer.
        draft.console_body();
        sink.commit(draft, false)
    }
}
fn io_error(e: serde_json::Error) -> std::io::Error {
    std::io::Error::other(e)
}
