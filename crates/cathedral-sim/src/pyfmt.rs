//! CPython text semantics the port depends on: `str.strip()`, `float.__repr__`
//! and `repr()`.
//!
//! The places that need these must agree:
//!
//! * `actions::parse_text` strips exactly like `str.strip()` — Python's
//!   whitespace set is *wider* than Rust's, so a value Python accepts must not
//!   become an error here (`sim.py:502`);
//! * `actions` and `scheduler` interpolate `{value!r}` / `str(dict)` into
//!   messages that become model-visible `system:` inbox lines, i.e. prompt
//!   bytes — and `repr()` of a float argument is `float.__repr__`, which
//!   disagrees with Rust/ryu at both ends of the magnitude range.

use serde_json::{Map, Value};

/// Python's `str.isspace()`: Unicode whitespace *plus* the C0 separators
/// `\x1c..\x1f`, which Rust's `char::is_whitespace` excludes. Both `\s` in the
/// action pattern and `str.strip()` use this set.
pub(crate) fn is_py_space(character: char) -> bool {
    character.is_whitespace() || matches!(character, '\u{1c}'..='\u{1f}')
}

/// Python's `str.strip()`.
pub(crate) fn py_strip(text: &str) -> &str {
    text.trim_matches(is_py_space)
}

/// CPython's `repr(float)`.
///
/// Rust and CPython both produce the shortest round-tripping digits, but they
/// lay them out differently: CPython switches to scientific notation when the
/// decimal point sits at or below `1e-5` or above `1e16`, and pads the exponent
/// to two digits (`1e-05`, not ryu's `1e-5`; `0.00001` in Rust's `Display`).
/// A float argument in an action error message reaches this via [`py_repr`].
///
/// Non-finite input yields Python's `repr` spelling (`nan` / `inf` / `-inf`).
/// The JSON path cannot reach it: serde_json writes `null` for non-finite floats
/// without consulting the formatter, exactly where CPython would write `NaN`.
pub(crate) fn py_float_repr(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-inf"
        } else {
            "inf"
        }
        .to_string();
    }

    let (digits, exponent) = shortest_digits(value);
    let sign = if value.is_sign_negative() { "-" } else { "" };
    // `value == 0.<digits> * 10^decpt`, which is the quantity CPython branches
    // on (`format_float_short`, mode 0: scientific iff `decpt <= -4 || decpt > 16`).
    let digit_count = digits.len() as i32;
    let decpt = exponent + 1;

    let mut out = String::with_capacity(digits.len() + 8);
    out.push_str(sign);
    if decpt <= -4 || decpt > 16 {
        out.push_str(&digits[..1]);
        if digit_count > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let exponent = decpt - 1;
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        // Always at least two digits: CPython writes `1e-05`, never `1e-5`.
        out.push_str(&format!("{:02}", exponent.abs()));
    } else if decpt <= 0 {
        out.push_str("0.");
        for _ in 0..-decpt {
            out.push('0');
        }
        out.push_str(&digits);
    } else if decpt >= digit_count {
        out.push_str(&digits);
        for _ in 0..decpt - digit_count {
            out.push('0');
        }
        // An integral value keeps its `.0` — `114.0`, never `114`.
        out.push_str(".0");
    } else {
        let split = decpt as usize;
        out.push_str(&digits[..split]);
        out.push('.');
        out.push_str(&digits[split..]);
    }
    out
}

/// The shortest round-tripping digits of `value.abs()`, and `k` such that
/// `value == 0.<digits> * 10^(k+1)` — i.e. CPython's `_Py_dg_dtoa(v, 0, …)`.
///
/// Rust's `{:e}` gives the same *number* of digits, but breaks an exact decimal
/// tie the other way: `f64::from_bits(0x431f003bd0f70bad)` is exactly
/// `2181495296738027.25`, and Rust rounds away from zero (`…27.3`) where CPython
/// rounds half to **even** (`…27.2`). Both round-trip, so neither is "wrong" —
/// but the prompt is golden-diffed, so we need CPython's.
///
/// The fix is to take only the *length* from `{:e}` and re-render at that
/// precision: Rust's fixed-precision formatter rounds the exact binary value
/// half-to-even (the same property [`py_round`](crate::py_round) relies on).
///
/// A tie can also carry (`9.95` → `10.`), so the exponent is re-read from the
/// rounded text and any trailing zeros are stripped, exactly as `dtoa` mode 0
/// does.
fn shortest_digits(value: f64) -> (String, i32) {
    let count = format!("{value:e}")
        .split_once('e')
        .expect("`{:e}` always emits an exponent")
        .0
        .chars()
        .filter(char::is_ascii_digit)
        .count();

    let rounded = format!("{value:.*e}", count - 1);
    let (mantissa, exponent) = rounded
        .split_once('e')
        .expect("`{:e}` always emits an exponent");
    let exponent: i32 = exponent.parse().expect("the exponent is an integer");
    let mut digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();

    let kept = digits.trim_end_matches('0').len().max(1);
    digits.truncate(kept);
    (digits, exponent)
}

/// CPython's `repr(str)`.
///
/// Single-quoted, unless the value contains a `'` and no `"` — then Python
/// switches to double quotes rather than escaping (`repr("it's") == "\"it's\""`).
///
/// Escapes: `\\`, the active quote, `\t`, `\n`, `\r`, and the C0/C1 controls as
/// `\xNN`. CPython also escapes the other non-printable categories (Cf, Zs, Zl,
/// Zp, Co, Cn); Rust has no `str.isprintable()` and this crate takes no unicode
/// dependency (D22), so a format-effector or an exotic space is passed through
/// literally. A documented divergence no realistic action argument reaches — the
/// reachable case is a control character in a `say` text, which is Cc.
pub(crate) fn py_repr_str(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(value.len() + 2);
    out.push(quote);
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            character if character == quote => {
                out.push('\\');
                out.push(character);
            }
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            // `char::is_control` is exactly Cc: `\x00..\x1f` and `\x7f..\x9f`.
            character if character.is_control() => {
                out.push_str(&format!("\\x{:02x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push(quote);
    out
}

/// CPython's `repr()` of a JSON-decoded value: `None` / `True` / `{'a': 1}`.
///
/// Integers and floats are distinguished the way `json.loads` does — `9` is an
/// `int` and prints as `9`, `9.0` is a `float` and prints as `9.0` — which
/// serde_json's `Number` preserves. An integer too large for `i64`/`u64` becomes
/// a float here where CPython keeps arbitrary precision; nothing that reaches an
/// error message goes near that range.
pub(crate) fn py_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(number) => match number.as_f64() {
            Some(float) if number.is_f64() => py_float_repr(float),
            _ => number.to_string(),
        },
        Value::String(text) => py_repr_str(text),
        Value::Array(items) => {
            let rendered: Vec<String> = items.iter().map(py_repr).collect();
            format!("[{}]", rendered.join(", "))
        }
        Value::Object(map) => py_repr_map(map),
    }
}

/// CPython's `str(dict)` — which is `repr(dict)`: `{'text': 9, 'target': None}`.
pub(crate) fn py_repr_map(map: &Map<String, Value>) -> String {
    let rendered: Vec<String> = map
        .iter()
        .map(|(key, value)| format!("{}: {}", py_repr_str(key), py_repr(value)))
        .collect();
    format!("{{{}}}", rendered.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strip_covers_the_c0_separators_python_calls_whitespace() {
        // U+001C..U+001F are `str.isspace()` in CPython but not
        // `char::is_whitespace` in Rust — the whole reason this module exists.
        assert_eq!(py_strip(" \t\u{1f}say {}\u{1c}\u{a0} "), "say {}");
        assert_eq!(py_strip("\u{1c}hello\u{1e}"), "hello");
        assert!(!'\u{1f}'.is_whitespace() && is_py_space('\u{1f}'));
    }

    /// Every expected string here was printed by CPython 3 (`repr(x)`, which is
    /// what `json.dumps` uses for floats).
    #[test]
    fn float_repr_matches_cpython_at_both_ends() {
        for (value, expected) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (114.0, "114.0"),
            (0.91, "0.91"),
            (-1.5, "-1.5"),
            (2.5, "2.5"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1.0 / 3.0, "0.3333333333333333"),
            // The decimal/scientific switch: ryu would say `0.00001` / `1e-5`.
            (1e-4, "0.0001"),
            (1e-5, "1e-05"),
            (2.5e-5, "2.5e-05"),
            (1e-6, "1e-06"),
            (1e-7, "1e-07"),
            (1.5e-7, "1.5e-07"),
            (5e-324, "5e-324"),
            // …and the upper one: 1e15 stays decimal, 1e16 does not.
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1e17, "1e+17"),
            (1e21, "1e+21"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (f64::MAX, "1.7976931348623157e+308"),
        ] {
            assert_eq!(py_float_repr(value), expected, "value {value:?}");
        }
        assert_eq!(py_float_repr(f64::NAN), "nan");
        assert_eq!(py_float_repr(f64::INFINITY), "inf");
        assert_eq!(py_float_repr(f64::NEG_INFINITY), "-inf");
    }

    /// An exact decimal tie is where the two shortest-repr algorithms part ways:
    /// Rust rounds away from zero, CPython's `dtoa` rounds half to **even**.
    ///
    /// These bit patterns are exactly halfway between two 17-digit decimals — the
    /// value below is precisely `2181495296738027.25` — and Rust's `{:e}` says
    /// `…27.3` where CPython says `…27.2`. Both round-trip; only one is what
    /// CPython prints. (Verified against CPython over 206k values: a
    /// random bit-pattern sweep plus every decade from 1e-320 to 1e308.)
    #[test]
    fn an_exact_decimal_tie_rounds_to_even_like_cpython() {
        for (bits, expected) in [
            (0x431f_003b_d0f7_0badu64, "2181495296738027.2"),
            (0xc30a_a61f_a224_75ca, "-937625523621561.2"),
            (0x42d1_1c37_8bee_3b08, "75251554695404.12"),
            (0x42ed_1ac1_5f8d_d0d4, "256006004960902.62"),
        ] {
            let value = f64::from_bits(bits);
            assert_eq!(py_float_repr(value), expected, "bits {bits:#018x}");
            // Both spellings parse back to the same double — the tie is real, and
            // this is a choice of representation, not of value.
            assert_eq!(expected.parse::<f64>().unwrap().to_bits(), bits);
        }
    }

    #[test]
    fn string_repr_switches_quotes_like_cpython() {
        assert_eq!(py_repr_str("plain"), "'plain'");
        // repr("it's") == '"it\'s"' — double quotes, no escape.
        assert_eq!(py_repr_str("it's"), "\"it's\"");
        // Both quote kinds present: back to single quotes, with escapes.
        assert_eq!(py_repr_str("it's \"x\""), "'it\\'s \"x\"'");
        assert_eq!(py_repr_str("say \"hi\""), "'say \"hi\"'");
        assert_eq!(py_repr_str("a\\b"), "'a\\\\b'");
        assert_eq!(py_repr_str("a\nb\tc\r"), "'a\\nb\\tc\\r'");
        assert_eq!(py_repr_str("\u{1}\u{7f}\u{9f}"), "'\\x01\\x7f\\x9f'");
        // Printable non-ASCII stays literal.
        assert_eq!(py_repr_str("héllo — ‘x’"), "'héllo — ‘x’'");
    }

    /// Python literal syntax, not JSON: `'` quotes, `': '` / `', '` separators,
    /// `None` / `True` / `False`, and `9` vs `9.0` kept apart the way `json.loads`
    /// keeps `int` and `float` apart.
    ///
    /// Key *order* is the one thing this cannot reproduce: `serde_json::Map` is a
    /// `BTreeMap` (pinned by ARCHITECTURE §2.4), so it sorts where Python's dict
    /// preserved the document order.
    #[test]
    fn value_repr_renders_python_literals_not_json() {
        assert_eq!(
            py_repr(&json!({"text": 9, "target": null, "a": true, "b": 1.0})),
            "{'a': True, 'b': 1.0, 'target': None, 'text': 9}"
        );
        assert_eq!(py_repr(&json!(["fart"])), "['fart']");
        assert_eq!(
            py_repr(&json!({"a": [1, {"b": false}]})),
            "{'a': [1, {'b': False}]}"
        );
        assert_eq!(py_repr(&json!({})), "{}");
        assert_eq!(py_repr(&json!([])), "[]");
        assert_eq!(py_repr(&json!(-3)), "-3");
        assert_eq!(py_repr(&json!(1e-5)), "1e-05");
        // A key needing CPython's quote flip is repr'd like any other string.
        assert_eq!(py_repr(&json!({"it's": 1})), "{\"it's\": 1}");
    }
}
