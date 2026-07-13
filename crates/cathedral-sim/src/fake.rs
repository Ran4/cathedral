//! Deterministic offline cognition (`server.py:393-412` `fake_llm_complete`).
//!
//! It parses the *rendered* prompt — pulling the character sheet out of its
//! ```` ```json ```` fence exactly as Python does (D25, option (a)) — instead of
//! being handed structured data. That is the point: the fake doubles as a
//! regression test that the minijinja templates still render a machine-readable
//! sheet carrying `name` and `since_your_last_turn`. If the template drifts, the
//! scripted conversation stops working and a test says so.
//!
//! There are deliberately no phrase hooks anywhere in production code paths;
//! this is the only place the words "Ilse" and "coin" mean anything.

use serde_json::Value;

use crate::{
    ids::RequestId,
    traits::{Cognition, CognitionBusy, Completion},
};

/// The no-op turn: harmless, and what every unrecognized prompt gets.
const REPLY_NOOP: &str = r#"set_goal {"goal": null}"#;
const REPLY_INTRODUCE: &str =
    r#"say {"target": "player", "text": "My name is Ilse. I am a pilgrim."}"#;
const REPLY_OFFER_COIN: &str = concat!(
    r#"say {"target": "player", "text": "You may have my copper coin."}"#,
    "\n",
    r#"offer_item {"item_id": "c0prs", "target": "player"}"#,
);

/// The scripted reply for one rendered prompt.
///
/// Rules, in order (any parse failure falls through to the no-op):
/// 1. Ilse, asked her name → she introduces herself.
/// 2. Ilse, asked to offer her coin → she says so and offers `c0prs`.
/// 3. anyone else → a no-op turn.
pub fn fake_reply(prompt: &str) -> String {
    let Some(sheet) = sheet_from_prompt(prompt) else {
        return REPLY_NOOP.to_string();
    };
    let name = sheet
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let history = history_text(&sheet).to_lowercase();

    if name == "Ilse" && history.contains("what's your name") {
        return REPLY_INTRODUCE.to_string();
    }
    if name == "Ilse" && history.contains("offer") && history.contains("coin") {
        return REPLY_OFFER_COIN.to_string();
    }
    REPLY_NOOP.to_string()
}

/// `prompt.split("```json\n", 1)[1].split("\n```", 1)[0]`, then `json.loads`.
fn sheet_from_prompt(prompt: &str) -> Option<Value> {
    let (_, after) = prompt.split_once("```json\n")?;
    let (block, _) = after.split_once("\n```")?;
    serde_json::from_str(block).ok()
}

/// The sheet's `since_your_last_turn` entries, newline-joined.
fn history_text(sheet: &Value) -> String {
    let Some(events) = sheet.get("since_your_last_turn").and_then(Value::as_array) else {
        return String::new();
    };
    events
        .iter()
        .map(|event| match event {
            // Python's `str(event)`: a string stays itself, anything else is
            // repr'd. Only strings ever occur.
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`Cognition`] with the script above and no provider at all.
///
/// Completions are computed synchronously at submit time and *staged*: the host
/// (or a test) drains them with [`FakeCognition::drain_completions`] and feeds
/// them back into the next poll, so the fake exercises the very same
/// submit-now/complete-later path a real backend takes — and lets a test decide
/// exactly which poll a reply lands on.
#[derive(Debug, Default)]
pub struct FakeCognition {
    next_request_id: u64,
    staged: Vec<Completion>,
    prompts: Vec<String>,
}

impl FakeCognition {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hand over every completion staged since the last drain.
    pub fn drain_completions(&mut self) -> Vec<Completion> {
        std::mem::take(&mut self.staged)
    }

    /// Every prompt this backend has been asked to complete, in order.
    pub fn prompts(&self) -> &[String] {
        &self.prompts
    }
}

impl Cognition for FakeCognition {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        self.staged.push(Completion {
            request_id,
            result: Ok(fake_reply(&prompt)),
            duration_seconds: 0.0,
        });
        self.prompts.push(prompt);
        Ok(request_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt_with(sheet: &str) -> String {
        format!("HEADER\n```json\n{sheet}\n```\nFOOTER\n")
    }

    #[test]
    fn ilse_introduces_herself_when_asked_her_name() {
        let prompt = prompt_with(
            r#"{"name": "Ilse", "since_your_last_turn": ["Someone said: \"What's your name?\""]}"#,
        );
        assert_eq!(fake_reply(&prompt), REPLY_INTRODUCE);
    }

    #[test]
    fn ilse_offers_the_coin_when_asked_to() {
        let prompt = prompt_with(
            r#"{"name": "Ilse", "since_your_last_turn": ["Please OFFER me your Coin"]}"#,
        );
        let reply = fake_reply(&prompt);
        assert!(reply.contains("You may have my copper coin."), "{reply}");
        assert!(reply.contains(r#"offer_item {"item_id": "c0prs", "target": "player"}"#));
    }

    #[test]
    fn everyone_else_and_every_malformed_sheet_takes_a_no_op_turn() {
        let prompt =
            prompt_with(r#"{"name": "Sven", "since_your_last_turn": ["what's your name"]}"#);
        assert_eq!(fake_reply(&prompt), REPLY_NOOP);
        assert_eq!(fake_reply("no fence at all"), REPLY_NOOP);
        assert_eq!(fake_reply(&prompt_with("{not json")), REPLY_NOOP);
        // A sheet that is valid JSON but not an object must not panic either.
        assert_eq!(fake_reply(&prompt_with("5")), REPLY_NOOP);
    }

    #[test]
    fn requests_stage_one_completion_each_and_record_the_prompt() {
        let mut cognition = FakeCognition::new();
        let first = cognition.request("a".to_string()).unwrap();
        let second = cognition.request("b".to_string()).unwrap();
        assert_ne!(first, second);
        assert_eq!(cognition.prompts(), ["a", "b"]);

        let completions = cognition.drain_completions();
        assert_eq!(completions.len(), 2);
        assert_eq!(completions[0].request_id, first);
        assert_eq!(completions[0].result, Ok(REPLY_NOOP.to_string()));
        assert!(cognition.drain_completions().is_empty());
    }
}
