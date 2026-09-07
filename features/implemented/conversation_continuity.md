# Conversation continuity with voluntary interruptions

Status: implemented and evaluated with live LLM simulations (2026-09-06).

## Problem and evidence

Session 814 (`logs/session_814_2026-09-06_21_02_22`) ends with the player
speaking to Sibbe Skell. Sibbe answers at 21:07:05 and 21:07:10. The player
asks "How are you going to trade?"; Rohese Nett answers at 21:07:19 about
her own spices and oil jar. Rohese's archived prompt contains Sibbe's replies,
so the history was available. Rohese had approached to 2.3 m from the player.

Player speech is broadcast and the nearest listener receives the protected
reaction turn. Any NPC addressing the player also replaces the engine's
conversation partner. Prompt courtesy alone has not prevented a nearby
person from treating another person's follow-up as their own.

## Intended experience

An exchange has continuity. Ordinary follow-ups favour the person the player
has been engaging. Passing closer, answering unsolicited, or crossing the
crosshair does not acquire that exchange. Other people can still hear,
remember, interrupt, object, warn, and join when the player engages them.
An NPC who waits must not prevent a different intended listener from answering.

The player can switch by deliberately attending to another person or naming
them, and can invite a group. Briefly looking away does not end an exchange.
Leaving and extended silence eventually release it. Nobody gets private
speech or knowledge of names they have not learned as a side effect.

## Design

### Conversation evidence and selection

Track a probable conversational addressee independently from the most recent
NPC to address the player. Prefer established reciprocal engagement to raw
distance. An unsolicited NPC line is an invitation or an interjection; it
does not overwrite an established partner. A player response can accept an
invitation, change partner, or continue the original exchange.

Use available gaze as supporting evidence, with sustained focus rather than
a single crosshair hit. Capture attention and conversation evidence with the
utterance, before transcription completes. Typed speech uses the same
selection policy. Validate candidates against presence and hearing; expired,
absent and out-of-range actors cannot monopolize player reactions. Use nearest
listener only when stronger evidence is absent.

Treat inferred attention as evidence, not an unconditional claim about the
meaning of the words. Explicit addressing and open group language must remain
possible even when gaze or continuity suggests someone else. Prefer a small,
explainable implementation; do not introduce a second LLM call per utterance.
If conservative language cues are needed for timely named/group reactions,
measure them and document their limits; merely mentioning someone's name
must not redirect speech.

### Hearing and prompts

Retain the existing physical hearing set, including bystanders. Add concise,
perspective-safe context identifying the apparent exchange and whether the
actor is its probable addressee or is overhearing. Preserve this interpretation
with the utterance so an old inbox cannot be relabelled by a later exchange.
Do not expose names, earlier dialogue or other facts the observer did not hear.

The model decides whether it is addressed, included in a group, or has a
specific reason to interject. Qualify the final instruction to answer the
latest question: first establish that it is addressed to this actor. Another
person's personal question is not an invitation to describe your own life.

Allow justified interruptions: a witnessed danger, a relevant correction,
a personal stake, an objection, or a character-specific intrusive motive.
An interjection should acknowledge its connection to the exchange. Having
a similar job by itself is insufficient. Do not require an extra action verb
or a compulsory spoken apology, and do not mechanically suppress all
bystander speech.

### Scheduling and ownership

Give the probable addressee the protected player reaction slot. Preserve the
existing floor, inbox, failure retry and player-composing guarantees. Other
listeners remain eligible for ordinary turns and relevant priority events.
Changing the recorded partner requires player engagement; NPC speech alone
must not silently transfer ownership of an established exchange.

Use bounded, expiring state and retain movement/stage protections where they
support an exchange. Validate delayed and stale utterances and actor removal.
Record enough diagnostic evidence to explain why a listener was preferred.

## Implementation and chosen parameters

The pure simulation now owns `Conversation` state, separately tracking player
engagement, an NPC invitation, attention samples and utterance ordering. The
selection order is a conservative explicit name or group cue, sustained gaze,
the captured warm partner, a recent invitation, then the nearest listener.

- Gaze must remain on the same actor for **0.65 seconds**; samples older than
  **1 second** do not count. A brief glance preserves the current exchange.
- Engagement lasts **30 seconds** after player engagement or that partner's
  response; an unsolicited invitation lasts **10 seconds**. Candidates must
  still be present and within hearing distance when the words arrive.
- Voice captures attention at recording onset, before local or streamed STT;
  typed speech captures when submitted. At most **8** captures survive for
  **120 seconds**. Abort, silence and failure release capture state. Sequence
  numbers prevent a delayed transcription from replacing a newer choice.
- Initial unambiguous vocatives such as `Sibbe, ...` and common English
  constructions such as `Sibbe can you ...` select a named listener. Leading
  cues such as `does anyone ...` and `both of you, ...` invite the group.
  Quoted, reported and appositive mentions and ambiguous first names do not
  redirect an exchange. This is a deliberately limited English recognizer;
  harder syntax still reaches all hearers for the LLM to interpret.
- A group question preserves the prior partner. The nearest listener receives
  the protected reaction; other hearers receive ordinary handoffs so they can
  answer even outside the current conversation stage. A new player follow-up
  takes precedence over those remaining handoffs.
- Speech remains a physical broadcast. Each hearer receives an immutable
  apparent-addressee note on that utterance, using names they know. A newcomer
  gets apparent-attention evidence rather than an assertion that they witnessed
  an earlier exchange; an unseen target's identity is not disclosed. Exchange
  witness tracking is bounded to **128 actors**.

NPC speech can confirm the player's existing choice or offer an invitation;
it cannot take the exchange from another partner. All bystanders keep their
ordinary turns. The social prompt asks them to respect personal follow-ups,
but permits warnings, objections and other specific reasons to interject.
Explicit words can override the tentative attention hint. A sustained-attention
clarification, selected after live trials, supports accepting one speaker's
interjection even if a third person has spoken since.

Production integration lives in `cathedral-sim/src/conversation.rs`, the shared
speech transaction/router and Engine, with host gaze/onset forwarding in
`src/smart_actors/`. Prompt wording remains editable in `assets/prompts/`.
No extra LLM call is made to classify the intended audience.

### Completed work

1. Implement conversation evidence, speech/typed-input integration, scheduling
   and prompt context; add deterministic behavioural regression coverage.
2. Compare live LLM variants with archived and constructed conversation scenes.
   Use the shipped model (`gpt-5.6-luna` in the incident); use existing provider
   configuration without printing secrets. Keep prompts, raw replies, timings,
   model identity, failures and assessment available for review.
3. Exercise complete exchanges through the pure Rust Engine with live cognition;
   then select the measured prompt, finish checks and record results here.

One fresh implementation agent handled production changes while the parent
prepared the evaluation harness and reviewed results. Verification used pure
Engine simulations and a windowless host systems test; the 3D game was not run.

## Evaluation matrix

Compare at least the previous prompt, a continuity-context version, and a
version with explicit addressee/interjection guidance. Repeat critical cases;
record failures as well as successes. Use live model output for the social
judgment, not a fake model or a string-presence test standing in for behaviour.

- Sibbe's personal follow-up with a nearer, otherwise uninvolved trader.
- Same scene from Sibbe's perspective: a useful answer about Sibbe's trade.
- A bystander speaking before the follow-up must not acquire the partner.
- A clearly named new addressee; a name merely mentioned in a sentence.
- An explicit open/group question relevant to a bystander.
- A justified interruption based on a witnessed event or personal stake.
- A nosy/interested character entering as an interjection, rather than
  impersonating the addressee.
- Player accepts an interjection and asks its speaker a follow-up.
- Brief glance away or a passerby crossing the crosshair.
- Sustained attention to a new person; returning to the former partner.
- Silence, departure, absent actor, and no plausible listener.
- Transcription completing after attention/positions change; typed/voice parity.
- Newcomer who did not hear the initial exchange; preserve perspective limits.

Deterministic tests cover routing, hearing, ownership, lifecycle and capture.
Live trials cover whether personal follow-ups stay with the proper person and
whether group invitations and justified interruptions still produce speech.
Acceptance requires inspecting actual content, not merely counting `say`.
State the sample sizes and residual ambiguity; a finite live evaluation is
not a guarantee of perfect social inference.

## Results

The [evidence report](conversation_continuity_evidence/README.md) contains the
method, every variant's results, full prompts, raw replies, errors, timings and
complete Engine transcripts. **505 live provider trials** used `gpt-5.6-luna`:
425 prompt comparisons and 80 Engine turns across three scenes before and
after refinement.

On six challenging archived/paraphrased personal follow-ups, the previous
prompt produced unwanted answers in **14/18** trials; both context alone and
the selected shorter courtesy wording produced **0/18**. Context alone also
silenced **3/3** explicitly named switches, while the shorter wording answered
all three. Longer role instructions also occasionally suppressed a named
recipient, so they were not selected. The exact original Sibbe question alone
did not reliably reproduce the failure on new calls; that limitation is
recorded rather than using it as proof of improvement.

In the final 100-trial batch, **40/40** intended-quiet personal-follow-up cases
stayed quiet, **45/45** direct/named/group/gaze/accepted-interjection cases
spoke from the intended perspective, and **5/5** falling-tile cases warned the
player. All three final Engine scenes preserved and switched partners as
intended, including acceptance after another NPC repeated a warning.

Specific debt objections were correct in **3/5** final trials: one confused
coins currently held with repayment of a remembered loan; one hit the
provider's 350-token output limit. A separately archived retry at the same cap
produced a correct objection. Generic nosiness stayed quiet **5/5** times, so
these trials demonstrate room for concrete interruptions, not reliable
interruption from personality alone. Neither the economy nor token cap was
changed to hide these results.

### Verification

- `cargo test -p cathedral-sim`: **1,062 tests passed**,
  including **15 new conversation regressions**, speech routing and prompt
  goldens; one fixture-regeneration test is normally ignored.
- `cathedral-backends`: **174 tests passed** in the combined crate run; four existing
  opt-in probes remained ignored. That run exposed an obsolete hand-written
  prompt-strings test fixture in the simulation crate, which was corrected
  before the full successful simulation rerun.
- `cargo check -p cathedralbevy --tests --quiet` passed. The actual host systems
  test `conversation_attention_precedes_voice_capture_and_typed_speech` passed
  without a window, renderer or microphone worker.
- `cargo clippy -p cathedral-sim -p cathedral-backends --all-targets` completed
  with existing warnings. `-D warnings` remains blocked by pre-existing lints
  in unrelated code; there were no new conversation/harness lints.
- Scoped Rust formatting and `git diff --check` passed. Workspace-wide format
  checking encounters existing unrelated formatting drift.

The deterministic cases cover bystander speech, capture-time gaze, typed/voice
parity, delayed transcription ordering, group follow-up preemption, departing
actors, expired state, failure cleanup and perspective-safe history. The live
runner invokes the real Engine scheduler, prompt renderer and action handling;
it does not supply canned LLM decisions.

### Limits

There is one primary partner, not a full multiparty conversation graph. An
unnamed acceptance without a gaze change can remain ambiguous. The language
cues are conservative English patterns, and attention notes remain fallible
evidence for the model. Existing audio-floor timing still applies to warnings;
this feature does not add emergency audio preemption. Finite trials on one
model do not guarantee perfect conversation judgment.
