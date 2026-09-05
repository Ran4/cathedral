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
/// The scripted `gesture` emission (`features/npc_bodies.md` §7): anyone asked
/// to wave greets the player and waves at them, so the offline e2e, the
/// headless runner and the live drive run all exercise the full gesture path —
/// the percept to any bystander, the `EngineMessage::Gesture`, and the host
/// pose — deterministically.
const REPLY_WAVE: &str = concat!(
    r#"say {"target": "player", "text": "Of course, hello there!"}"#,
    "\n",
    r#"gesture {"kind": "wave", "to": "player"}"#,
);
/// The scripted looping gesture: anyone asked to dance says so and starts the
/// `dance` loop, which the snapshot then carries until they next act.
const REPLY_DANCE: &str = concat!(
    r#"say {"target": "player", "text": "Watch me, then!"}"#,
    "\n",
    r#"gesture {"kind": "dance"}"#,
);

/// The scripted reply for one rendered prompt.
///
/// Rules, in order (any parse failure falls through to the no-op):
/// 1. Ilse, asked her name → she introduces herself.
/// 2. Ilse, asked to offer her coin → she says so and offers `c0prs`.
/// 3. anyone **asked a question** who holds something → they say their first
///    `what_you_know` bullet back, verbatim (`features/knowledge_and_rumor/`).
/// 4. anyone asked to dance or wave → they say so and do it.
/// 5. anyone else → a no-op turn.
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
    // A holder who is asked something says the thing they hold, in the exact
    // words the sheet gave them. Read off the *rendered* block, which is the
    // only reason a phrase hook is allowed to exist in this file: a renderer
    // that stops emitting the block breaks the offline knowledge run, and a
    // test says so. Cannot disturb any existing transcript — before M1 no
    // prompt carried the block, and after M1 only a holder's does.
    if history.contains('?')
        && let Some(word) = known_bullets(prompt).next()
    {
        return format!(
            r#"say {{"target": "player", "text": {}}}"#,
            serde_json::to_string(word).unwrap_or_else(|_| "\"\"".to_string())
        );
    }
    if history.contains("dance") {
        return REPLY_DANCE.to_string();
    }
    if history.contains("wave") {
        return REPLY_WAVE.to_string();
    }
    REPLY_NOOP.to_string()
}

/// The scripted **Night Office** reflection (movement M6). Like every other
/// rule here it is read off the *rendered* prompt, which is the point: the fake
/// can only emit a working `set_round` if the night sheet really carries
/// numbered round legs and place handles the model could have named. If either
/// stops rendering, the offline night run stops moving anybody's day and a test
/// says so.
///
/// A person with no round, or no places, settles their memory and nothing else
/// — which is also the honest answer for the anchoress bricked into her wall.
pub fn fake_night_reply(prompt: &str) -> String {
    if let Some(ward) = ward_name(prompt) {
        let mut reply = format!(
            r#"ward_mood {{"mood": "It has been an ordinary night in {ward}, and nobody is saying much."}}"#
        );
        // One edit, if the digest offered both a person and a place — the ward
        // branch's own end-to-end proof.
        if let (Some(person), Some(place)) = (first_ward_person(prompt), first_place_id(prompt)) {
            reply.push('\n');
            reply.push_str(&format!(
                r#"set_round {{"person": "{person}", "leg": 1, "place_id": "{place}"}}"#
            ));
        }
        return reply;
    }
    let name = sheet_name(prompt).unwrap_or("Someone");
    let mut reply =
        format!(r#"remember {{"memory": "I, {name}, lay down at the end of an ordinary day."}}"#);
    if let (Some(place), true) = (first_place_id(prompt), has_round_legs(prompt)) {
        reply.push('\n');
        reply.push_str(&format!(
            r#"set_round {{"leg": 1, "place_id": "{place}"}}"#
        ));
    }
    reply
}

/// The ward the digest names, or `None` for a person's night sheet — how the
/// fake tells the two branches of `night.j2` apart without a phrase hook.
fn ward_name(prompt: &str) -> Option<&str> {
    prompt
        .lines()
        .find_map(|line| line.strip_prefix("**the_ward** — "))
        .map(str::trim)
}

/// The first id in the ward digest's `your_people` — `- k0fb1 — Tam Rud, …`.
fn first_ward_person(prompt: &str) -> Option<&str> {
    section_bullets(prompt, "**your_people**")
        .next()?
        .split_whitespace()
        .next()
}

/// The first handle in `places_you_know` / `their_places` — `- pl_x2vw The
/// Tallage`.
fn first_place_id(prompt: &str) -> Option<&str> {
    section_bullets(prompt, "**places_you_know**")
        .chain(section_bullets(prompt, "**their_places**"))
        .next()?
        .split_whitespace()
        .next()
        .filter(|id| id.starts_with("pl_"))
}

/// Whether the sheet numbered any round legs — `- leg 1 — at Dayspring: …`.
fn has_round_legs(prompt: &str) -> bool {
    section_bullets(prompt, "**your_round**").any(|line| line.starts_with("leg "))
}

/// The `- ` bullets under `**what_you_know**` (`features/knowledge_and_rumor/`).
///
/// Unlike every other section it puts its instruction paragraph between the
/// header and the bullets, so [`section_bullets`]' immediate `take_while` sees
/// the blank line and stops. Bounded by the next section header, so it cannot
/// run on into the ward's word.
fn known_bullets(prompt: &str) -> impl Iterator<Item = &str> {
    let mut lines = prompt.lines();
    let found = lines
        .find(|line| line.starts_with("**what_you_know**"))
        .is_some_and(|line| line.ends_with(':'));
    lines
        .take_while(move |line| found && !line.starts_with("**"))
        .filter_map(|line| line.strip_prefix("- "))
        .map(str::trim)
}

/// The `- ` bullets under one markdown section header, empty when the section
/// is absent or took its inline empty form.
fn section_bullets<'a>(prompt: &'a str, header: &str) -> impl Iterator<Item = &'a str> {
    let mut lines = prompt.lines();
    let found = lines
        .find(|line| line.starts_with(header))
        .is_some_and(|line| line.ends_with(':'));
    lines
        .take_while(move |line| found && line.starts_with("- "))
        .map(|line| line["- ".len()..].trim())
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
        self.stage(fake_reply(&prompt), prompt)
    }

    /// The fake has no second slot to build: it completes synchronously at
    /// submit time, so the two lanes can never contend for it. What it does
    /// need is the *other script* — the night prompt is a different prompt and
    /// gets a different reply.
    fn request_night(
        &mut self,
        prompt: String,
        _max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.stage(fake_night_reply(&prompt), prompt)
    }
}

impl FakeCognition {
    fn stage(&mut self, reply: String, prompt: String) -> Result<RequestId, CognitionBusy> {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        self.staged.push(Completion {
            request_id,
            result: Ok(reply),
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
    fn anyone_asked_to_wave_greets_and_waves_at_the_player() {
        let reply = fake_reply(&prompt_with("Sven", &["Player said: \"Please wave at me\""]));
        assert!(reply.contains("hello there"), "{reply}");
        assert!(reply.contains(r#"gesture {"kind": "wave", "to": "player"}"#), "{reply}");
    }

    #[test]
    fn anyone_asked_to_dance_starts_the_dance_loop() {
        let reply = fake_reply(&prompt_with("Conny", &["Player said: \"Would you dance?\""]));
        assert!(reply.contains(r#"gesture {"kind": "dance"}"#), "{reply}");
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
