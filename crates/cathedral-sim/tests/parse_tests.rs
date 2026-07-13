//! Reply parsing (`prompt.py:264-320`;
//! `tests/test_prompt_scheduler.py::PromptTests` 9-10).
//!
//! The six error strings are protocol: they become `system:` inbox lines that
//! the model reads on its next turn, so they are pinned here. Only the
//! parenthesized suffix of `bad JSON in:` diverges from CPython (serde_json's
//! wording instead of `JSONDecodeError`'s) — a documented divergence
//! (prompt.md R7); the goldens only cover clean runs.

use cathedral_sim::{
    parse_reply, parse_reply_value,
    prompt::parse::{ParsedAction, REPLY_MUST_BE_TEXT},
};
use serde_json::{Value, json};

fn verbs(actions: &[ParsedAction]) -> Vec<&str> {
    actions.iter().map(|(verb, _)| verb.as_str()).collect()
}

/// 9. `test_parser_rejects_non_object_and_trailing_garbage`
#[test]
fn parser_rejects_non_object_and_trailing_garbage() {
    let (actions, errors) =
        parse_reply("say [1]\nsay {\"text\":\"ok\"} garbage\nsay {\"text\":\"# safe\"} # comment");

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].0, "say");
    assert_eq!(actions[0].1["text"], "# safe");
    assert_eq!(errors.len(), 2);
    // NOT "args must be a JSON object": the pattern already demands `{`, so a
    // `[` never gets as far as the type check. Verified against CPython — that
    // branch is unreachable in both implementations, and is kept only because
    // removing it would be a silent behavior claim.
    assert_eq!(errors[0], "not understood: say [1]");
    assert_eq!(
        errors[1],
        "unexpected text after JSON in: say {\"text\":\"ok\"} garbage"
    );

    // A reply that is not text at all.
    assert_eq!(
        parse_reply_value(&Value::Null),
        (Vec::new(), vec![REPLY_MUST_BE_TEXT.to_string()])
    );
    assert_eq!(parse_reply_value(&json!(7)).1, [REPLY_MUST_BE_TEXT]);
    assert_eq!(parse_reply_value(&json!("wait {}")).0.len(), 1);
}

/// 10. `test_pathologically_nested_reply_becomes_a_parse_error`
#[test]
fn pathologically_nested_reply_becomes_a_parse_error() {
    let nested = format!(
        "remember {{\"memory\":{}\"x\"{}}}",
        "[".repeat(2_000),
        "]".repeat(2_000)
    );
    let (actions, errors) = parse_reply(&nested);
    assert!(actions.is_empty());
    assert!(!errors.is_empty());
}

/// The shape guard's own band: serde_json parses depth 65..=127 happily, and the
/// guard is what rejects it — with the same message Python produces.
#[test]
fn the_shape_guard_rejects_depth_past_sixty_four() {
    let ok = format!(
        "remember {{\"memory\":{}\"x\"{}}}",
        "[".repeat(63),
        "]".repeat(63)
    );
    let (actions, errors) = parse_reply(&ok);
    assert_eq!(actions.len(), 1, "{errors:?}");

    // memory is at depth 1, so 64 brackets put the innermost scalar at depth 65.
    let too_deep = format!(
        "remember {{\"memory\":{}\"x\"{}}}",
        "[".repeat(64),
        "]".repeat(64)
    );
    let (actions, errors) = parse_reply(&too_deep);
    assert!(actions.is_empty());
    assert_eq!(
        errors,
        ["JSON structure is too deeply nested or large in: remember"]
    );
}

#[test]
fn the_shape_guard_rejects_more_than_ten_thousand_nodes() {
    let wide = format!("remember {{\"memory\":[{}]}}", vec!["1"; 10_000].join(","));
    let (actions, errors) = parse_reply(&wide);
    assert!(actions.is_empty());
    assert_eq!(
        errors,
        ["JSON structure is too deeply nested or large in: remember"]
    );

    // 9_997 scalars + the array + `memory`'s object = 9_999 nodes: accepted.
    let ok = format!("remember {{\"memory\":[{}]}}", vec!["1"; 9_997].join(","));
    assert_eq!(parse_reply(&ok).0.len(), 1);
}

#[test]
fn blank_comment_and_fence_lines_are_skipped() {
    let (actions, errors) = parse_reply("```json\n\n   \n# a comment\nwait {}\n```\n");
    assert_eq!(verbs(&actions), ["wait"]);
    assert!(errors.is_empty());
}

#[test]
fn a_line_without_braces_is_not_understood() {
    let (actions, errors) = parse_reply("wait\nsay hello\n42 {}\n{\"text\": \"x\"}");
    assert!(actions.is_empty());
    assert_eq!(
        errors,
        [
            "not understood: wait",
            "not understood: say hello",
            "not understood: 42 {}",
            "not understood: {\"text\": \"x\"}",
        ]
    );
}

#[test]
fn bad_json_keeps_the_offending_line_in_the_message() {
    let (actions, errors) = parse_reply("say {\"text\": }");
    assert!(actions.is_empty());
    assert_eq!(errors.len(), 1);
    // The wording of the suffix is Rust's; the prefix is protocol.
    assert!(
        errors[0].starts_with("bad JSON in: say {\"text\": } ("),
        "{}",
        errors[0]
    );
    assert!(errors[0].ends_with(')'), "{}", errors[0]);
}

/// Python's `str.splitlines()` splits on far more than `\n` (D21) — an LLM that
/// emits `\u{2028}` must not smuggle two actions past the parser as one line.
#[test]
fn replies_split_on_pythons_full_line_boundary_set() {
    let reply = "wait {}\u{b}wait {}\u{c}wait {}\u{1c}wait {}\u{1d}wait {}\u{1e}\
                 wait {}\u{85}wait {}\u{2028}wait {}\u{2029}wait {}\r\nwait {}\rwait {}";
    let (actions, errors) = parse_reply(reply);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(actions.len(), 11);
    assert!(
        actions
            .iter()
            .all(|(verb, args)| verb == "wait" && args.is_empty())
    );
}

#[test]
fn errors_and_actions_accumulate_independently() {
    let (actions, errors) = parse_reply(
        "say {\"text\": \"one\"}\nnonsense\nset_goal {\"goal\": \"Eat fish\"} # why\nsay [1]",
    );
    assert_eq!(verbs(&actions), ["say", "set_goal"]);
    assert_eq!(errors.len(), 2);
}
