//! CPython-compatible JSON text for the character sheet (`prompt.py:240`).
//!
//! Python renders the sheet with `json.dumps(sheet, indent=4)` — i.e. with
//! `ensure_ascii=True`, which serde_json's formatters do not do. The prompt is
//! golden-diffed byte for byte, so the escaping rules here are behavior:
//!
//! * 4-space indent, `": "` after keys, empty containers inline (`[]`, `{}`);
//! * every character outside `0x20..=0x7E` becomes `\uXXXX` with **lowercase**
//!   hex — including `0x7F`, which is ASCII but not printable;
//! * astral characters become UTF-16 surrogate pairs;
//! * the short forms `\b \t \n \f \r \" \\` are kept (serde_json's default
//!   `write_char_escape` already emits exactly those, plus lowercase `\u00xx`
//!   for the remaining C0 controls — the same set Python uses);
//! * floats are written by CPython's `float.__repr__` ([`py_float_repr`]), which
//!   ryu does not reproduce: `1e-05` where serde_json says `0.00001`, `1e-07`
//!   where it says `1e-7`. Sheet positions are raw `f64`, so this is reachable.

use std::io;

use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter, Serializer};

use crate::pyfmt::py_float_repr;

/// `json.dumps(value, indent=4)` — including `ensure_ascii=True`.
///
/// Container layout is delegated to serde_json's `PrettyFormatter`; only string
/// escaping is ours.
pub struct PyAsciiFormatter {
    pretty: PrettyFormatter<'static>,
}

impl Default for PyAsciiFormatter {
    fn default() -> Self {
        Self {
            pretty: PrettyFormatter::with_indent(b"    "),
        }
    }
}

const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

fn write_escaped_unit<W: ?Sized + io::Write>(writer: &mut W, unit: u16) -> io::Result<()> {
    writer.write_all(&[
        b'\\',
        b'u',
        HEX_DIGITS[((unit >> 12) & 0xF) as usize],
        HEX_DIGITS[((unit >> 8) & 0xF) as usize],
        HEX_DIGITS[((unit >> 4) & 0xF) as usize],
        HEX_DIGITS[(unit & 0xF) as usize],
    ])
}

/// `\uXXXX`, or a surrogate pair for anything above the BMP.
fn write_escaped_char<W: ?Sized + io::Write>(writer: &mut W, character: char) -> io::Result<()> {
    let code = character as u32;
    if let Ok(unit) = u16::try_from(code) {
        return write_escaped_unit(writer, unit);
    }
    let offset = code - 0x1_0000;
    write_escaped_unit(writer, 0xD800 + ((offset >> 10) as u16))?;
    write_escaped_unit(writer, 0xDC00 + ((offset & 0x3FF) as u16))
}

impl Formatter for PyAsciiFormatter {
    /// serde_json hands us the runs of a string it considers escape-free: `"`,
    /// `\` and the C0 controls have already been split out into
    /// `write_char_escape`. Everything left that is not printable ASCII is what
    /// `ensure_ascii=True` escapes and serde_json would have passed through.
    fn write_string_fragment<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let bytes = fragment.as_bytes();
        let mut plain_from = 0usize;
        for (index, character) in fragment.char_indices() {
            if matches!(character, ' '..='~') {
                continue;
            }
            if plain_from < index {
                writer.write_all(&bytes[plain_from..index])?;
            }
            write_escaped_char(writer, character)?;
            plain_from = index + character.len_utf8();
        }
        if plain_from < bytes.len() {
            writer.write_all(&bytes[plain_from..])?;
        }
        Ok(())
    }

    /// `json.dumps` renders a float with `float.__repr__`, not with ryu.
    ///
    /// serde_json writes `null` for a non-finite float without ever calling us,
    /// so only finite values arrive here (CPython would write `NaN`/`Infinity` —
    /// a divergence no sheet can reach, since `math` admits finite coordinates
    /// only).
    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        debug_assert!(value.is_finite(), "serde_json filters non-finite floats");
        writer.write_all(py_float_repr(value).as_bytes())
    }

    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_array(writer)
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_array(writer)
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pretty.begin_array_value(writer, first)
    }

    fn end_array_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_array_value(writer)
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_object(writer)
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_object(writer)
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pretty.begin_object_key(writer, first)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_object_value(writer)
    }

    fn end_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_object_value(writer)
    }
}

/// Serialize exactly like `json.dumps(value, indent=4)`.
pub fn to_py_json<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    let mut buffer = Vec::with_capacity(1024);
    let mut serializer = Serializer::with_formatter(&mut buffer, PyAsciiFormatter::default());
    value.serialize(&mut serializer)?;
    // The formatter only ever emits ASCII plus fragments of the (valid UTF-8)
    // input, so this cannot fail.
    Ok(String::from_utf8(buffer).expect("serde_json emits UTF-8"))
}

/// CPython's `round(value, digits)`: correctly-rounded decimal, ties to even.
///
/// `{:.N}` formats the *exact* binary value and rounds half-to-even, which is
/// what `float.__round__` does; parsing the result back reproduces the same
/// double. The naive `(x * 10^n).round() / 10^n` does not (`0.25` → `0.3`
/// instead of `0.2`). Non-finite input is returned unchanged — Python would
/// raise, but no such value can reach a sheet.
pub fn py_round(value: f64, digits: usize) -> f64 {
    if !value.is_finite() {
        return value;
    }
    format!("{value:.digits$}")
        .parse()
        .expect("a fixed-precision float always parses back")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strings_are_ascii_escaped_like_ensure_ascii() {
        // `I’ll` (U+2019), an astral emoji, DEL, and a C0 control.
        let rendered = to_py_json(&json!({"t": "I\u{2019}ll \u{1F600}\u{7f}\u{1}\"\\\n"})).unwrap();
        assert_eq!(
            rendered,
            "{\n    \"t\": \"I\\u2019ll \\ud83d\\ude00\\u007f\\u0001\\\"\\\\\\n\"\n}"
        );
    }

    #[test]
    fn layout_matches_indent_four_with_inline_empty_containers() {
        let rendered = to_py_json(&json!({"a": [], "b": {}, "c": [1, {"d": 2.5}]})).unwrap();
        assert_eq!(
            rendered,
            "{\n    \"a\": [],\n    \"b\": {},\n    \"c\": [\n        1,\n        \
             {\n            \"d\": 2.5\n        }\n    ]\n}"
        );
    }

    #[test]
    fn integral_floats_keep_their_point_zero() {
        // Python's Vec3 coerces to float, so `114.0` must not print as `114`.
        let rendered = to_py_json(&json!({"z": 114.0})).unwrap();
        assert_eq!(rendered, "{\n    \"z\": 114.0\n}");
    }

    /// A sheet position is a raw `f64`. serde_json's own float writer would say
    /// `0.00001` and `1e-7` here; CPython's `json.dumps` says `1e-05` and
    /// `1e-07`, and the prompt is golden-diffed byte for byte.
    #[test]
    fn small_and_large_floats_use_cpython_exponent_notation() {
        let rendered = to_py_json(&json!({"x": 1e-5, "y": 1e-7, "z": 1e16})).unwrap();
        assert_eq!(
            rendered,
            "{\n    \"x\": 1e-05,\n    \"y\": 1e-07,\n    \"z\": 1e+16\n}"
        );
        // …and the values a sheet actually carries are untouched.
        let rendered =
            to_py_json(&json!({"a": 0.0001, "b": -1.8, "c": 1000000000000000.0})).unwrap();
        assert_eq!(
            rendered,
            "{\n    \"a\": 0.0001,\n    \"b\": -1.8,\n    \"c\": 1000000000000000.0\n}"
        );
    }

    #[test]
    fn rounding_is_ties_to_even_on_the_exact_binary_value() {
        assert_eq!(py_round(0.25, 1), 0.2);
        assert_eq!(py_round(0.35, 1), 0.3); // 0.35 is really 0.34999999999999997...
        assert_eq!(py_round(2.6907248094147422, 1), 2.7);
        assert_eq!(py_round(1.2345, 3), 1.234); // 1.2344999999999999...
        assert_eq!(py_round(-0.25, 1), -0.2);
        assert_eq!(py_round(2.0, 1), 2.0);
    }
}
