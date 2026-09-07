# Conversation continuity — live evidence, 2026-09-06

Implementation and deterministic verification: see the
[conversation continuity specification](../conversation_continuity.md). All calls below used the configured
OpenAI-compatible backend with **gpt-5.6-luna**, the model recorded in the
incident. No Bevy window, renderer, STT service or voice worker was started.

## What was measured

There are **505 live provider trials**: 425 prompt comparisons (including one
explicit retry) and 80 turns in six complete Engine runs. The latter are three
scene scripts run before and after refinement. They use `Engine::poll`, real
prompt rendering, real scheduling and action application, with a live
`LlmClient` behind a recording `Cognition` adapter. They do not substitute a
fake answer for the model's social decision.

Provider-reported usage across the runs: 2,452,850 input tokens, of which
1,639,126 were cached, and 47,736 completion tokens. Failed responses may not
report usage. The repository has no pricing entry for this model, so no USD
estimate is inferred here.

Prompt comparisons preserve the full archived character sheets and change only
the scenario's stated percepts, attention hints and social instructions. They
use the incident's 350-token ambient completion cap. The Engine runs use the
scheduler's actual caps and a compact three-NPC seed. These are complementary:
archive trials exercise production-sized prompts; Engine runs exercise the
real integration and propagation of replies through the next person's history.

`fixtures/` preserves the original Sibbe/Rohese, Petronel, Aldith and Lark
prompts from session 814, plus the missed acceptance found during the first
Engine run. The original unwanted replies remain in those fixtures. Result
JSON files preserve each answer, model, cap, duration and error. For prompt
comparisons, `prompt_path` points to the exact full prompt in the same run's
`prompts/` directory; repeated prompts are stored once. Engine turn JSON files
carry their complete prompts inline. Nothing failed was deleted or overwritten
by a retry.

## Selection of the prompt

| Run | Calls | Finding |
| --- | ---: | --- |
| `round1/` | 117 | Previous prompt, context alone, and longer role instructions; three samples each across 13 scenes. The exact archived Sibbe question happened to get quiet bystanders even with the old prompt, so it alone could not establish improvement. Longer instructions missed one explicitly named override. |
| `round2/` | 72 | Added three other archived interruptions and three ordinary follow-up phrasings. Old prompt produced unwanted personal answers in **14/18** trials. Context alone and shorter courtesy wording both produced **0/18**, but context alone missed all three named overrides in this batch. Shorter wording answered all three. |
| `final_prompt/` | 95 | Five samples over 19 scenes with shorter instructions. All 40 intended-quiet trials stayed quiet. Direct and group responses and warnings remained possible. A debt scenario exposed an empty output and a false inference about repayment. |
| Initial `engine_*` runs | 40 | Routing worked across proximity, groups, switches and departure. One acceptance after two NPCs warned the player got a `wait` despite sustained gaze at the first warner. |
| `attention_refinement/` | 40 | Compared before/after a short sustained-attention clarification. The previously missed acceptance answered **4/5 → 5/5**; named overrides, direct gaze switches and quiet Sibbe bystanders also behaved as intended. |
| `acceptance/` | 100 | Five samples across all 20 scenes using the selected prompt, including the attention clarification. Detailed results below. |
| `acceptance_retry/` | 1 | Retried the one output-budget error separately, using the same prompt and 350-token cap; received a correct debt objection. |
| `engine_final_*` runs | 40 | All three complete scene scripts with the final prompt. Partner decisions and the resulting dialogue matched the intended cases. |

The final policy keeps the existing permission to interject. It adds specific
context, explains that another person's personal question does not become yours
because you have a similar trade, qualifies the final answer instruction, and
lets explicit words override the tentative hint. Sustained gaze can identify
the recipient of a plain pronoun question even when another NPC spoke last.

## Final 100-trial batch

| Behaviour | Result |
| --- | --- |
| Eight personal-follow-up/overhearing cases | **40/40 stayed quiet**; no personal answer takeover. Some chose an unrelated movement action rather than `wait`, which is allowed. |
| Nine direct, named, group, gaze and accepted-interjection cases | **45/45 spoke**; inspected replies answered from the intended character's perspective. |
| Witnessed falling tile | **5/5 warned the player**, without replacing the question with their own trade plans. |
| Specific debt/personal stake | **3/5 correct objections**, one false debt-memory inference, one provider output-budget error. Separate retry of the provider error produced a correct objection. |
| General intrusive personality, with no urgent obligation to speak | **5/5 quiet**. This is allowed, but does not demonstrate that generic nosiness reliably produces interruptions. |

The debt fixture deliberately retains the archived four sparks in Rohese's
hand while also giving her an unpaid four-spark loan. One answer inferred that
the held coins disproved the loan, despite the supplied memory. This is a
grounding/memory limitation, not successful debt handling, and it is not
counted as a correct objection merely because a `say` was emitted. The failed
provider request explicitly reported its output limit; the retry is separately
visible. These findings were not used to widen the production token cap or
change the unrelated economy.

## Complete Engine exchanges

The exact messages and routing diagnostics are in each `transcript.json`.
Each step also records the selected partner, making ownership independently
checkable from what the model chose to say.

- [`engine_final_continuity/transcript.json`](engine_final_continuity/transcript.json):
  Sibbe answers while Rohese stands closer. A group invitation lets Rohese
  advertise spices while Sibbe remains the prior partner. Naming Rohese makes
  her the partner for the next pronoun follow-up; naming Sibbe switches back.
- [`engine_final_interruption/transcript.json`](engine_final_interruption/transcript.json):
  Rohese's unsolicited greeting does not acquire the exchange. She subsequently
  warns about a tile, still without acquiring it. The player looks at her and
  asks her name; she answers, becomes the partner, and answers the next question
  about her living.
- [`engine_final_attention/transcript.json`](engine_final_attention/transcript.json):
  a 0.2-second glance preserves Sibbe; 0.7-second attention selects Rohese.
  Departure and 35 seconds of silence allow subsequent exchanges normally.

The first three runs use the default compact-fixture calendar (Bellday);
the final three explicitly use Highmarket, matching the incident and their
trade backstories. No navigation graph is loaded: movements are scripted
position changes, while conversation selection and replies use the real Engine.
NPC lines used to establish the unsolicited-greeting challenge are scripted
setup, and the witnessed tile is an injected percept. All cognition decisions
and resulting responses are live. In one group trial the quiet resident also
referred the player to the spicer after her answer; that redundant but relevant
contribution remains possible.

## Reproduction

From the repository root:

```sh
uv run scripts/evaluate_conversation_continuity.py prepare \
  --output /tmp/conversation-trials.json --variants shipped --repeats 5
cargo run -p cathedral-backends --example conversation_eval -- \
  --requests /tmp/conversation-trials.json --output /tmp/conversation-trials
uv run scripts/evaluate_conversation_continuity.py summarize /tmp/conversation-trials --replies

uv run scripts/evaluate_conversation_continuity.py scenes --output /tmp/conversation-scenes
cargo run -p cathedral-backends --example conversation_sim -- \
  --scene /tmp/conversation-scenes/interruption_and_acceptance.json \
  --output /tmp/conversation-engine
```

Configuration is loaded through the same `BackendsConfig`/dotenv handling as
the game. Credentials are not copied into evidence. The summary script only
counts speaking/quiet and invalid output; acceptance also requires reading
the actual content. Subsequent provider runs need not reproduce these exact
counts or sentences.

## Limits

This is one model and a finite evaluation. The probable addressee remains a
hint, and a model may still misread it or choose an unwanted interruption.
Urgent warnings are permitted but retain the existing presentation floor and
turn scheduling; this does not introduce emergency preemption of another
speaker's audio. The engine keeps one primary partner, rather than building
a complete social graph of a three-person conversation. An unnamed semantic
reply to an interjection without a gaze change can still be ambiguous;
explicit address or sustained attention supplies reliable evidence of a switch.

The deterministic suite covers actual routing/capture rather than relying on
these prompt heuristics to prove engine behaviour. Final command results are
recorded in the feature specification.
