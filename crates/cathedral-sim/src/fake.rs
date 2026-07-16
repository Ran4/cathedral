//! Deterministic offline cognition (`server.py:393-412` `fake_llm_complete`).
//!
//! It parses the *rendered* prompt — reading the name off the markdown sheet's
//! `**you**` line and the news out of its `**since_your_last_turn**` section —
//! instead of being handed structured data. That is the point: the fake doubles
//! as a regression test that the rendered sheet still carries a machine-readable
//! name and history. If the renderer drifts, the scripted conversation stops
//! working and a test says so.
//!
//! There are deliberately no phrase hooks anywhere in production code paths;
//! this is the only place the words "Ilse" and "coin" mean anything.

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
    let Some(name) = sheet_name(prompt) else {
        return REPLY_NOOP.to_string();
    };
    let history = history_text(prompt).to_lowercase();

    if name == "Ilse" && history.contains("what's your name") {
        return REPLY_INTRODUCE.to_string();
    }
    if name == "Ilse" && history.contains("offer") && history.contains("coin") {
        return REPLY_OFFER_COIN.to_string();
    }
    REPLY_NOOP.to_string()
}

/// The name on the sheet's `**you**` line.
///
/// Without lore the line is the bare name (`**you** — Sven`); with lore the
/// name ends at the first comma (`**you** — Corin Copp, 26, male — …`).
pub fn sheet_name(prompt: &str) -> Option<&str> {
    let rest = prompt
        .lines()
        .find_map(|line| line.strip_prefix("**you** — "))?;
    let head = rest.split(" — ").next().unwrap_or(rest);
    Some(head.split(',').next().unwrap_or(head).trim())
}

/// The sheet's `**since_your_last_turn**` bullets, newline-joined; the inline
/// empty form (`**since_your_last_turn** — nothing`) yields the empty string.
fn history_text(prompt: &str) -> String {
    let mut lines = prompt.lines();
    let Some(header) = lines.find(|line| line.starts_with("**since_your_last_turn**")) else {
        return String::new();
    };
    if !header.ends_with(':') {
        return String::new();
    }
    let mut collected: Vec<&str> = Vec::new();
    for line in lines {
        let Some(text) = line.strip_prefix("- ") else {
            break;
        };
        collected.push(text);
    }
    collected.join("\n")
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

    /// A minimal markdown sheet, shaped like [`super::super::prompt`] renders it.
    fn prompt_with(name: &str, since: &[&str]) -> String {
        let mut sheet = format!("**you** — {name}, 30, female — Pilgrim of The Gradine.\n\n");
        if since.is_empty() {
            sheet.push_str("**since_your_last_turn** — nothing\n");
        } else {
            sheet.push_str("**since_your_last_turn**:\n");
            for line in since {
                sheet.push_str(&format!("- {line}\n"));
            }
        }
        format!("HEADER\n\nYour sheet:\n\n{sheet}\nFOOTER\n")
    }

    #[test]
    fn ilse_introduces_herself_when_asked_her_name() {
        let prompt = prompt_with("Ilse", &[r#"Someone said: "What's your name?""#]);
        assert_eq!(fake_reply(&prompt), REPLY_INTRODUCE);
    }

    #[test]
    fn ilse_offers_the_coin_when_asked_to() {
        let prompt = prompt_with("Ilse", &["Please OFFER me your Coin"]);
        let reply = fake_reply(&prompt);
        assert!(reply.contains("You may have my copper coin."), "{reply}");
        assert!(reply.contains(r#"offer_item {"item_id": "c0prs", "target": "player"}"#));
    }

    #[test]
    fn everyone_else_and_every_malformed_sheet_takes_a_no_op_turn() {
        let prompt = prompt_with("Sven", &["what's your name"]);
        assert_eq!(fake_reply(&prompt), REPLY_NOOP);
        assert_eq!(fake_reply("no sheet at all"), REPLY_NOOP);
        // The inline empty-history form reads as no news.
        assert_eq!(fake_reply(&prompt_with("Ilse", &[])), REPLY_NOOP);
    }

    #[test]
    fn the_you_line_parses_with_and_without_lore() {
        assert_eq!(sheet_name("**you** — Ilse\nrest"), Some("Ilse"));
        assert_eq!(
            sheet_name("**you** — Corin Copp, 26, male — Scrivener of The Tallage.\n"),
            Some("Corin Copp")
        );
        assert_eq!(sheet_name("no you line"), None);
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
