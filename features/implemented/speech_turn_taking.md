# Social turn-taking for overheard speech

Status: implemented on 2026-07-11.

## Problem

Speech is correctly audible to every actor within 20 metres, but nearby NPCs
often treat hearing a line as an obligation to answer it. In a group, saying
"Sven, do you have any fish?" can therefore make Ilse or Conny reply "I'm not
Sven" instead of leaving the answer to Sven.

That response is evidence that the model understood the name mismatch. The
missing piece is the social rule that an overheard utterance may be relevant
context without creating a conversational turn for the listener.

## Why it happens

Player microphone speech is intentionally submitted as an open `say` action:

- Rust sends `target_id: null` for `player_recording`.
- After transcription, Python calls `apply_action(..., "say", {"text": text})`.
- `sim.py` consequently gives every nearby NPC the same general form,
  `A stranger said: "..."`, in both its inbox and recent conversation.

This open delivery is desirable: addressing somebody is not privacy, and
bystanders should still hear the words.

The prompt, however, starts by asking the NPC to "Take one or more actions" and
describes `wait` mainly as the response when nobody has said anything new. A
newly heard line therefore looks like a reason to act even when its wording
clearly addresses somebody else. Since each NPC gets an ordinary round-robin
turn, all of them independently get that nudge.

The simulation currently distinguishes explicit NPC `say {"target": ...}`
actions for event rendering, but natural player speech should not need a name
parser or other hard-coded targeting rule. Understanding who a sentence is
for is part of the NPC's language and social reasoning.

## Implemented solution

Teach the cognition prompt that hearing, being addressed, and having a useful
reason to interject are three different things. Before speaking in response to
new dialogue, the actor should infer the intended audience from:

- the words themselves, including names and phrases such as "anyone";
- its own name and identity;
- the visible people around it;
- recent conversational context; and
- whether it has a character-specific reason to interject.

Add guidance along these lines near the general action/`wait` instructions in
`prompt_playgound/prompt.py`:

> Speech in your history is what you could hear, not necessarily speech
> addressed to you. Decide from the wording, names, nearby people, and recent
> conversation whether the speaker is talking to you, to the group, or to
> somebody else. If a line is clearly for somebody else, normally use `wait
> {}` alone. Do not answer merely to announce that you are not the named
> person. Interject only when your character has a concrete reason to do so.
> If you are the only plausible listener and the speaker asks for somebody who
> is not there, asking for clarification can be natural. Questions to
> "anyone", "everyone", or the group are open to relevant answers.

The existing final instruction should also stop implying that `wait` is only
for turns with no new speech. Its meaning should instead be: use `wait {}`
whenever there is nothing useful and socially appropriate for this actor to do,
even if the actor just overheard something new.

A few compact contrastive examples in the prompt may make the distinction more
reliable:

- In a group, Ilse hears "Sven, do you have fish?" -> normally `wait {}`.
- Sven hears the same question -> answer if he can.
- Ilse is alone with the speaker and hears them ask for Sven -> clarification
  or confusion is reasonable.
- An actor hears "Does anyone have fish?" -> answer only if it has a relevant
  answer; otherwise `wait {}`.

No recipient filtering, vocative-name parser, extra LLM classification call,
or change to hearing range is required. Every nearby actor continues to hear
and remember the utterance; each actor's existing cognition call decides
whether to participate.

## Validation

Prompt rendering tests should verify that the revised audience and `wait`
guidance is present. Because the behavior itself is model judgment, exercise a
small repeated evaluation matrix with the configured cognition model:

1. Player, Sven, Conny, and Ilse nearby; "Sven, do you have any fish?" — Sven
   may answer, while Conny and Ilse normally wait.
2. Player and Ilse alone; the same line — Ilse may reasonably express
   confusion.
3. The full group; "Does anyone have any fish?" — an actor with relevant
   knowledge or inventory may answer.
4. The full group; speech addressed to one actor where a bystander has a strong
   in-character reason to interject — the prompt must allow that judgment
   rather than imposing silence mechanically.

Existing simulation tests for open delivery, explicit targeted delivery,
bystander hearing, recent-conversation retention, and the 20 metre boundary
should remain unchanged.
