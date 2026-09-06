Status: implemented and verified (2026-09-06).

# NPCs know how far their workplace is

An NPC who walks to work should be able to give a rough distance or walking
time when asked. The answer should come from the streets the simulation makes
them walk, and should refer to the place the player was asking about.

## Evidence from the conversation

Session: `logs/session_799_2026-09-06_11_49_52/`.
The NPC was Colm Threefinger, generated citizen `x00334`, a stable hand.

The [11:53:10 prompt and answer](../../logs/session_799_2026-09-06_11_49_52/prompts/2026-09-06_11_53_10__00__x00334__Colm-Threefinger_prompt.md)
shows both failures:

- Colm had twice told the player that he worked at the Harne Gate.
- His `your_round` explicitly placed his work there, and `places_you_know`
  included `pl_yg7v The Harne Gate`. His current position was near the Stone
  Gate, so this was a question about travelling between two distinct gates.
- The player asked, as transcribed: “Okay how many meters do you need to how
  long does it take to walk to there from here?”
- After that question, Hamel Ashe mentioned going to Coswald’s Yard. Colm
  answered: “Hamel Ashe. He’s a tile maker. I don’t know the distance or walking
  time to Coswald’s Yard from here.”

In the [next answer](../../logs/session_799_2026-09-06_11_49_52/prompts/2026-09-06_11_53_20__00__x00334__Colm-Threefinger_prompt.md)
Colm doubled down: “Nor do I. Ask a gate man or a carter.”

The missing travel data is directly observable: each known place carries only
an ID and name. The switch to Coswald’s Yard is consistent with the model
letting the intervening speaker replace the topic of its exchange with the
player. The transcript also contains rough speech transcription, which should
be preserved in the reproduction rather than silently cleaned up.

## Behavior

When asked how far his workplace is, Colm names the Harne Gate and gives an
approximate walk from where he is standing. This applies to everyone's known
places: workplaces, homes, routine destinations, and places learned through
`tell_way`.

The existing `places_you_know` section will use the user's compact column
format, with the explanation shared once in the header:

```text
**places_you_know** (place_id — name — distance — time to walk on foot from here; approximate street estimates; go_to takes these place_ids):
- pl_8sky — Bell and Sluice Wards — 500 m — 4 min
- pl_yg7v — The Harne Gate — 500 m — 4 min
```

Those numbers illustrate the format; they are not a measurement of this
session. Colm can turn them into ordinary speech: “The Harne Gate? About four
minutes on foot.” He need not recite both numbers unless asked.

## Implementation

### 1. Derive walking estimates from the existing street graph

Extend the known-place data assembled in
[`crates/cathedral-sim/src/prompt/mod.rs`](../../crates/cathedral-sim/src/prompt/mod.rs).
Keep this in the pure simulation; the Bevy host and backend need no new data
source or provider call.

- Resolve only the actor's existing `places_known` handles against
  `World.places`. Preserve the current ordering and dangling-handle handling.
  `Round::seed` already adds workplace and daily-round destinations, including
  Colm's gate; there is no need to grant extra geographical knowledge.
- Cache distances to each destination node in `NavData`, shared by all NPCs.
  The first query for a destination calls `distances_from(destination)` once
  and stores an `f32` distance from every possible starting node. The street
  graph is undirected, so these are also the distances to that destination.
  Later queries look up the actor's nearest node in that table. Use the actual
  walkable nodes rather than a regular grid that can fall inside buildings.
  Tables are lazy: unused destinations require no distance array. A new graph
  owns a fresh cache; cache warmth must not change graph equality or behavior.
  The current graph has 3,972 nodes. Tables for 78 named places/ward anchors
  and 1,101 doors would use about 18 MiB with four-byte entries; even tables
  for every graph node are bounded at about 60 MiB. Homes sharing a destination
  node share its table. No per-frame or per-NPC table recomputation is needed.
- Include the short horizontal approach from the actor's position to the
  graph, and the destination's offset from its node where applicable. Follow
  the existing movement endpoint rules when deciding whether an off-graph
  endpoint is usable. Use the street route rather than the direct distance
  through buildings. These remain estimates: individual walking lanes,
  avoidance and interruptions can change the actual trip.
- Derive walking minutes from route length divided by the existing
  `WALK_SPEED_MPS` (currently 2.1), then by 60. This describes elapsed walking
  time at the game's normal movement speed. Do not multiply by the accelerated
  day/night clock or the developer time control: the player needs an estimate
  of how long the walk takes to play.
- Round metres to roughly the nearest ten and minutes to one decimal in the
  sheet. Use a minimum positive estimate for short walks. For a reachable
  destination already within the existing arrival radius, render “right here”
  instead of “0 minutes”.
- With no graph or no route, retain the known place and render
  `place_id — name — walking estimate unavailable`.
  Never substitute a straight-line distance for an unreachable street route.
  Distances refresh on each rendered turn as the actor moves; they are not
  stored as memories or taught as permanent numbers by `tell_way`.

Add an optional structured estimate to `PlaceRef`, shared by the JSON sheet
view and Markdown renderer. Put the display wording in
[`assets/prompts/strings.toml`](../../assets/prompts/strings.toml), following the
existing division between computed data and prompt prose.

### 2. Explain the estimates and preserve the conversation's topic

Update [`assets/prompts/turn.j2`](../../assets/prompts/turn.j2), including a short
reminder after the sheet:

- Explain that estimates beside known places are the character's practical
  knowledge of the walk from here. Use them for distance/time questions and
  speak approximately. Do not invent measurements when none are supplied, or
  invent street-by-street instructions from a distance alone.
- Resolve follow-ups such as “there” from the exchange with the asker,
  considering both recent history and new speech in order. A bystander's
  later comment does not retroactively change the place the asker meant.
  Honor an explicit change of destination; ask a brief clarification when
  the exchange genuinely leaves two plausible places.

Make explicit that supplied travel estimates belong to ordinary geographical
knowledge. Preserve the existing restrictions against inventing civic events
and gossip in `what_you_know`; its absence must not suppress information that
is actually supplied elsewhere in the sheet.

This first change uses the existing speaker-labelled history. New conversation
state, speech routing changes, and transcription changes are outside this
proposal. If prompt guidance cannot pass the replay below, record the failure
and propose a separate change rather than expanding this one silently.

### 3. Verify the data and the actual response

Add meaningful coverage in
[`crates/cathedral-sim/tests/place_estimates.rs`](../../crates/cathedral-sim/tests/place_estimates.rs),
alongside the existing prompt tests:

- A bent street route gives its walking length, not the shorter direct
  distance; walking time follows the shared speed constant.
- Moving the actor changes the estimate, including a short off-node approach;
  an arrived destination renders naturally.
- Disconnected destinations and worlds without navigation invent no estimate.
- Unknown places stay absent; learning a place exposes an estimate from the
  learner's current position. Home/workplace handles remain usable.
- Both the structured sheet and the actual rendered prompt carry the estimate.

Regenerate the blessed golden prompts through the repository's documented
ignored test because the prompt instructions intentionally change. Review the
fixture diff, then run `cargo test -p cathedral-sim` and the relevant backend
prompt-consumer/fake-cognition checks.

Replay Colm's archived exchange with the current template and computed place
estimates using `cathedral-headless --one-shot`. Keep the original transcript,
including Hamel's interruption, and use the recorded model where available.
Record that the archived position is rounded if it is used to reconstruct the
sheet. Run the exact failure case three times and keep prompts and answers as
evidence. Also check an explicit switch to Coswald's Yard and a destination
the character does not know. A fake backend can verify plumbing, but cannot
prove that the model follows the conversation correctly.

Measure the added sheet size and rendering time on a representative populated
world, separating cold-cache initialization from warmed lookups. Verify cache
reuse, graph isolation and cached values against route lengths. The new route
calculation should occur once per queried destination node and add no
background crowd work. No visible game window is needed for these
checks; any later Bevy check must use `CATHEDRAL_HEADLESS=1`.

## Acceptance

- Colm answers the recorded workplace follow-up about the Harne Gate in all
  three replays, with an estimate supported by his sheet.
- He can answer a direct distance question and a walking-time question; he
  does not claim ignorance of an estimate he has been given.
- A later bystander's destination does not replace the player's topic, while
  an explicit player change of topic is respected.
- Known, reachable places get current route estimates. Unknown and unreachable
  places acquire no fabricated travel facts.
- Automated checks pass, and the implementation report includes replay
  evidence, prompt-size impact and measured rendering overhead.

The user approved implementation on 2026-09-06, with the compact header and
rows shown above. No further approval is needed for this scope.

## Completed verification

All seven final live checks passed: the original ambiguous follow-up three
times, explicit distance and walking-time questions, an explicit switch to
Coswald's Yard, and an unknown destination. The final prompt needed both the
chronological guidance and a reminder after the sheet to reliably keep this
exchange on its original topic in those samples.

The sim suite passed 1,025 tests; the backend library passed 148. The game
passed `cargo check` and was rebuilt with `cargo build -p cathedralbevy`.
Cached distance lookup measured
2.16 ns on average, with a 25.9 µs median full-prompt render after the initial
3.68 ms cold render. The known-place section grew by 629 UTF-8 bytes for Colm's
20 places. These measurements and the original/final prompts are preserved in
[the verification record](npcs_know_how_far_their_workplace_is_evidence/README.md).
