//! The action grammar (`prompt.py:264-320`): `VERB {json}` lines in, actions
//! and error lines out.
//!
//! Actions and errors accumulate independently — one bad line never aborts the
//! rest — and verb *validity* is not checked here: unknown verbs reach
//! [`apply_action`](crate::apply_action), which rejects them. Both kinds of
//! failure become `system:` inbox lines for the actor's next turn.
//!
//! No regex crate (D21): the pattern is `^([a-z_]\w*)\s*(\{.*)$` with
//! IGNORECASE, which is small enough to match by hand — and Python's
//! `str.splitlines()` splits on more than `\n`/`\r\n`, so `str::lines` would be
//! wrong anyway.

use serde_json::{Map, Value};

use crate::pyfmt::{is_py_space, py_strip};

/// One parsed line: the lowercased verb and its argument object.
pub type ParsedAction = (String, Map<String, Value>);

/// The reply was not text at all — only reachable from an untyped wire value.
pub const REPLY_MUST_BE_TEXT: &str = "reply must be text";

/// `_safe_json_shape` (`prompt.py:268-282`).
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 10_000;

/// Python's `str.splitlines()` (D21). Beyond `\n`, `\r` and `\r\n` it breaks on
/// `\v`, `\f`, the file/group/record separators `\x1c..\x1e`, NEL `\u{85}`, and
/// U+2028 / U+2029 — an LLM emitting any of those would otherwise smuggle two
/// actions onto one line. A trailing terminator does not produce an empty tail.
fn py_splitlines(text: &str) -> Vec<&str> {
    fn is_line_boundary(character: char) -> bool {
        matches!(
            character,
            '\n' | '\r'
                | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        )
    }

    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if !is_line_boundary(character) {
            continue;
        }
        lines.push(&text[start..index]);
        let mut next = index + character.len_utf8();
        if character == '\r' && characters.peek().is_some_and(|(_, next)| *next == '\n') {
            characters.next();
            next += 1;
        }
        start = next;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// `^([a-z_]\w*)\s*(\{.*)$` (IGNORECASE) on an already-stripped line.
///
/// Returns the lowercased verb and the argument text, which always starts at
/// `{`: a bare `wait` is not an action, `say{"text": "x"}` is. `\w*` is greedy
/// and `\s`/`{` are disjoint from it, so no backtracking is possible — matching
/// the longest word run and then skipping whitespace is exactly the regex.
fn match_action(stripped: &str) -> Option<(String, &str)> {
    let mut characters = stripped.char_indices();
    let (_, first) = characters.next()?;
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }
    let mut verb_end = first.len_utf8();
    for (index, character) in characters {
        // Python's `\w` is `str.isalnum() or '_'`.
        if !character.is_alphanumeric() && character != '_' {
            break;
        }
        verb_end = index + character.len_utf8();
    }
    let rest = &stripped[verb_end..];
    let args_start = rest
        .find(|character: char| !is_py_space(character))
        .unwrap_or(rest.len());
    let args = &rest[args_start..];
    if !args.starts_with('{') {
        return None;
    }
    Some((stripped[..verb_end].to_lowercase(), args))
}

/// Reject arguments no sane action carries. Iterative (a recursive walk is what
/// this is defending against), root at depth 0, and every scalar counts as a
/// node — a port of `_safe_json_shape`, kept even though serde_json's own
/// recursion limit (128) already refuses to build the deepest inputs.
fn safe_json_shape(root: &Map<String, Value>) -> bool {
    // The argument object is the root, at depth 0 and worth one node.
    let mut nodes = 1usize;
    let mut stack: Vec<(&Value, usize)> = root.values().map(|child| (child, 1usize)).collect();
    while let Some((current, depth)) = stack.pop() {
        nodes += 1;
        if depth > MAX_DEPTH || nodes > MAX_NODES {
            return false;
        }
        match current {
            Value::Object(map) => stack.extend(map.values().map(|child| (child, depth + 1))),
            Value::Array(items) => stack.extend(items.iter().map(|child| (child, depth + 1))),
            _ => {}
        }
    }
    true
}

/// Parse `VERB {json}` lines into `(actions, errors)`.
///
/// Trailing `# comments` work because the JSON value ends the parse: a `#`
/// inside a quoted string is never seen as one.
pub fn parse_reply(reply: &str) -> (Vec<ParsedAction>, Vec<String>) {
    let mut actions: Vec<ParsedAction> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for line in py_splitlines(reply) {
        let stripped = py_strip(line);
        // A model that wraps its reply in a fenced block still parses.
        if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with("```") {
            continue;
        }
        let Some((verb, args_text)) = match_action(stripped) else {
            errors.push(format!("not understood: {stripped}"));
            continue;
        };

        // `raw_decode`: parse ONE value and remember where it ended.
        let mut stream = serde_json::Deserializer::from_str(args_text).into_iter::<Value>();
        let value = match stream.next() {
            Some(Ok(value)) => value,
            // The error text is CPython's in Python and serde_json's here — a
            // documented divergence (prompt.md R7); it is frozen by a test.
            Some(Err(error)) => {
                errors.push(format!("bad JSON in: {stripped} ({error})"));
                continue;
            }
            None => {
                errors.push(format!("bad JSON in: {stripped} (no JSON value)"));
                continue;
            }
        };
        let end = stream.byte_offset();

        // Unreachable in practice — the pattern already demanded `{`, and no
        // other JSON value starts with one. Python carries the same dead branch
        // (`prompt.py:310-312`); dropping it would be a behavior claim, so it
        // stays, error string and all.
        let Value::Object(args) = value else {
            errors.push(format!("args must be a JSON object: {stripped}"));
            continue;
        };
        if !safe_json_shape(&args) {
            errors.push(format!(
                "JSON structure is too deeply nested or large in: {verb}"
            ));
            continue;
        }
        let trailing = py_strip(&args_text[end..]);
        if !trailing.is_empty() && !trailing.starts_with('#') {
            errors.push(format!("unexpected text after JSON in: {stripped}"));
            continue;
        }
        actions.push((verb, args));
    }

    (actions, errors)
}

/// [`parse_reply`] for a value that has not been proven to be text yet
/// (`prompt.py:291-292`). The typed backends make this unreachable, but the
/// protocol can still hand us a JSON `null`.
pub fn parse_reply_value(reply: &Value) -> (Vec<ParsedAction>, Vec<String>) {
    match reply {
        Value::String(text) => parse_reply(text),
        _ => (Vec::new(), vec![REPLY_MUST_BE_TEXT.to_string()]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitlines_matches_pythons_boundary_set() {
        assert_eq!(py_splitlines(""), Vec::<&str>::new());
        assert_eq!(py_splitlines("a\r\nb\n"), ["a", "b"]);
        assert_eq!(
            py_splitlines("a\u{b}b\u{c}c\u{1c}d\u{1d}e\u{1e}f\u{85}g\u{2028}h\u{2029}i"),
            ["a", "b", "c", "d", "e", "f", "g", "h", "i"]
        );
        // A lone \r ends a line; \r\n is one boundary, not two.
        assert_eq!(py_splitlines("a\rb"), ["a", "b"]);
        assert_eq!(py_splitlines("a\r\n\r\nb"), ["a", "", "b"]);
    }

    #[test]
    fn strip_covers_the_c0_separators_python_calls_whitespace() {
        assert_eq!(py_strip(" \t\u{1f}say {}\u{1f}\u{a0} "), "say {}");
    }
}
