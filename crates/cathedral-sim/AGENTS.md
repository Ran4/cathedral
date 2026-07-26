# cathedral-sim — the authoritative character simulation

The world, the prompt format, the action parser and the NPC turn scheduler.
Pure: no clock, no threads, no network, no filesystem, no Bevy. It is driven by
one call — `Engine::poll(now, commands) -> Vec<EngineMessage>` — and it reaches
the outside world only through the `Cognition` / `Transcription` / `Tts` traits,
whose results come back as plain values. That is what lets the same engine run
inside the game (`src/smart_actors/local_engine.rs`) and inside the headless
runner (`cargo run -p cathedral-backends --bin cathedral-headless -- --fake`),
and what makes every test here deterministic and offline.

This crate is the port of the old Python sidecar (`prompt_playgound/server.py`,
`sim.py`, `prompt.py`, `scheduler.py`), which is deleted. Where a behaviour
looks arbitrary, it is usually Python's, preserved on purpose — the comments say
which line it came from.

## How a turn works

NPCs near the player take autonomous turns in a significance-weighted stream;
anyone the player cannot see, hear or talk to enters it only when speech or a
nearby event wakes them (see *Who gets an idle turn* below). Each turn:

1. `prompt::render_prompt_and_drain` renders the character's full sheet
   (backstory, location, visible people, held items, memories, goal) plus a
   `since_your_last_turn` section drained from that character's **inbox**. The
   sheet is markdown (`**you_hold**:` over `- fzbn9 fish` bullets — about half
   the tokens of the JSON it replaced); the template is
   `assets/prompts/turn.j2`; the strings are `assets/prompts/strings.toml`.
   Nothing about the LLM text format lives anywhere else.
2. The prompt goes to the provider as one stateless call — no provider-side chat
   history. Each character carries a bounded `recent_history`
   (`RECENT_HISTORY_MAX_ENTRIES` = 32 entries) of received speech, heard sound
   percepts, and its own lines; a line repeating the newest entry coalesces
   into it with a running count (`… (3 times now)`), so a percept barrage
   cannot flush real dialogue out of the window. The two fields never
   overlap: a percept is *pending* while unread, appears once as
   `since_your_last_turn`, and graduates into `recent_history` when that turn
   **succeeds** (a failed turn re-queues it as new). `memories` and `goal` are
   the only durable state, so the prompt tells the model to use
   `remember`/`forget` deliberately.
3. The reply is parsed as `VERB {json}` lines (`prompt::parse_reply`) and each
   action is applied to the world (`apply_action`).

Invalid reply lines and failed actions become `system:` lines in the actor's own
inbox, so the model can self-correct next turn (and are echoed as
`EngineMessage::Diagnostic`).

**Hearing** has one authoritative recipient calculation within 20 m (full 3D
distance, inclusive boundary, stable `(distance, id)` ordering). Every recipient
id is retained in the structured event the game consumes; LLM-controlled
recipients additionally get a prose inbox line. The player is never scheduled, so
his inbox never accumulates. Fresh player speech queues the nearest LLM listener
in a protected FIFO reaction lane: NPC-to-NPC handoffs cannot overwrite it, and
background speech outside the player's earshot cannot hold its completed reply.

**Unknown people.** Each character has a `knows` set of character ids. People
outside it render as `(unknown - you don't know the name of this person)` in
`you_see` and as `a stranger (id <id>)` in heard events. `perception::identify`
is the single place this perspective rendering happens. There is no introduction
verb: characters introduce themselves in speech. A human observer who hears a
speaker say their own full name learns that identity in `knows`; NPC listeners
keep introduced names as memories (the prompt tells them to).

For the shipped cast, the player begins knowing all major/minor figures by
reputation and no ambient names. `PlayerKnowledge::Everyone` is retained for
headless/developer use (`cathedral-headless --know-everybody`). Provider requests
cap output at 1,200 / 700 / 350 tokens for major / minor / ambient turns.
Significance itself never enters the prompt.

## Who gets an idle turn (`attention.rs`)

Three lanes select the next actor, and only one of them is a clock:

| Lane | Fires because | Gated? |
|---|---|---|
| player reaction | the player spoke | never |
| priority slot | an addressed `say` or an audible sound reached them | never |
| round robin | time passed | **yes** |

The round robin is restricted to the **stage**: the LLM actors within
`radius_m` (32 m, wider than the 20 m hearing radius on purpose) of the player,
nearest first, capped at `max_actors` (6), plus whoever the player is currently
in an exchange with — a partner keeps a reserved seat for 30 s after the last
targeted line either of them addressed to the other, so backing out of the radius
mid-sentence does not cut them off. The engine recomputes it once per poll, next
to `floor_busy` and for the same reason (D20).

Alone in a field this costs **nothing**; before the gate the rotation spent ~1,100
provider calls an hour regardless of where the player stood. Ambient NPCs remain
reachable by speech and by sound anywhere in the city — those lanes are ungated,
and they are the only way an ambient NPC ever thought in the first place.

Two further gates ride on the round robin, both in `attention.rs` and both
`config.ron`-revertible under `idle_cognition`. **`Novelty`** (`require_news`)
demands that something have *happened* to an on-stage actor since their last turn
— a non-empty inbox, or a changed set of ids within their 20 m — because a turn
that ends in `wait {}` changes nothing and would only buy another one.
**`CuriosityConfig`** (`curiosity`, `curiosity_scale`) then asks whether they are
someone who would *say* anything about it: derived from the lore sheet's age,
trade and standing, overridable by an authored `curiosity` in the character's own
JSON, and calibrated so ~20% of the people you walk past think about you at all.
Curiosity applies **only** to the changed-context branch — a non-empty inbox is
never rolled — so an aloof NPC never opens, but always answers. The roll is a pure
hash of `(actor, context, meeting)` and never a fresh draw: the engine polls at
60 Hz, and a re-drawn 20% is a certainty within a frame.

The weights change meaning with the gate, so there are two orders.
`background_turn_order` (Major ×4 / Minor ×1 / **Ambient ×0**) answers "who, out
of 500 people, deserves scarce global compute?". `stage_turn_order` (Major 3 /
Minor 2 / **Ambient 1**) answers "who, out of the six people in front of the
player, thinks next?" — where the ambient market crowd *is* the scene, and the
completion caps above keep their turns cheap instead. `EngineConfig::idle_mode`
picks one at construction and defaults to `All`, so the tests and the headless
runner keep exercising the full cast without faking proximity;
`config.ron: idle_cognition.mode` puts the game on `"stage"`, and
`cathedral-headless --stage` opts in when the gate itself is under test.

No idle turn starts while the player is composing an utterance — microphone hot,
STT in flight, or inside the router's grace window (`SpeechRouter::player_composing`).
The scheduler cannot preempt a provider call once it is out, so the only way to
keep the player's words from queueing behind two seconds of some irrelevant NPC's
thinking is to not start that turn. The protected reaction lane is the one
exception and still fires immediately.

**Known consequence.** With the gate in and nothing behind it, the city outside
the stage stops moving: no errands, no autonomous movement, no gossip. That is
the accepted trade, and it promotes the non-LLM behavior layer from a
nice-to-have to a dependency.

## Actions

One action per line, `VERB {json args}`, optional `# comment` after. Omitting a
key and passing `null` mean the same thing. `item_id` takes ids only — a name
like `"fish"` is an error, never a fallback.

- `say {"target": "<id>", "text": "..."}` — an LLM target's inbox gets "said to
  you", LLM bystanders get "said to X". Omitted/null `target` broadcasts within
  20 m. An invalid or out-of-range explicit target is an error and never falls
  back to broadcast.
- `offer_item {"item_id": "<id>", "target": "<id>"}` — offer a held item; it
  stays in the giver's `holds` until accepted. Omitted/null `target` = broadcast
  (anyone within 4 m may accept, first wins). Re-offering replaces the pending
  offer, and a nearby jilted target gets a `retract_offer` event.
- `accept_offered_item {"item_id": "<id>"}` — take an item offered to you (or
  broadcast) by someone still within 4 m; moves it giver → you, clears the offer.
- `decline_offer {"item_id": "<id>"}` — turn down an offer targeted at you; the
  giver keeps the item. Broadcast offers can only be ignored.
- `retract_offer {"item_id": "<id>"}` — withdraw your own pending offer.
- `eat {"item_id": "<id>"}` — consume one uncommitted unit of a held edible
  stack. Offers and transform reservations remain promises: if no unit is free,
  eating fails with `item_committed` until the offer is retracted or replaced.
- `make_sound {"sound_id": "<id>"}` — emit a catalog sound
  (`assets/sounds/catalog.toml`); only rows with `actor_emittable = true`.
- `go_to {"place_id": "<pl_…>"}` / `go_to {"person": "<id>"}` — set a travel
  *intent*; it moves nobody (M5, `features/implemented/movement/05_the_llm_seam.md`). The
  behaviour ladder (`round.rs`) walks it as a rung between thirsty and the
  round; the intent expires on a route-derived real-seconds budget and the
  pressing needs preempt it — arrival and every lapse are percepts, and both
  grant the priority-lane nudge an addressed `say` gets. `place_id` takes an
  opaque handle from the sheet's `places_you_know` (the per-actor wayfinding
  whitelist in `CharacterState.places_known`, resolved against
  `World.places` — see `places.rs`); a person target must currently be in
  `you_see`, and losing sight degrades the follow to their last-seen spot.
  Errors: `unknown_place`, `no_route` (`too_far` is reserved until the hunger
  need exists).
- `stop {}` — abandon the current `go_to`; self-initiated, so no percept.
- `tell_way {"person": "<id>", "place_id": "<pl_…>"}` — teach someone within
  earshot a way you hold: the id is written into *their* `places_known` (the
  sheet is the model's memory) and one inbox line tells them. Targeted, never
  broadcast; eavesdroppers learn nothing.
- `remember` / `forget` / `set_goal` — the durable state. `set_goal
  {"goal": null}` clears it back to the `"None"` sentinel.
- `wait {}` — deliberately take no world action when speaking would only repeat
  the recent history. Scheduler `wait`s are not appended to the transcript.

Pending offers live in `World.offers` (item id → giver/target) and are rendered
on the sheet every turn as `you_offer` / `offered_to_you`, because inbox events
alone would be forgotten. Offer inbox events are past-tense history with no
accept hint — they can be stale by the time they are read (someone earlier in the
round may have taken a broadcast offer); the accept syntax appears only in
`offered_to_you`, which is always current. Full design:
`features/implemented/giving_things.md`.

A targeted offer also ends **by itself** when the two drift more than
`OFFER_LAPSE_RADIUS_M` (20 m) apart: `actions::lapse_distant_offers`, swept once
per `Engine::poll`, emits a `lapse_offer` world event and tells both parties why
(`features/implemented/offer_lapse.md`). It is not a verb — no reply can ask for
it — because past 20 m neither party *can* answer an offer that accept and
decline both need 4 m for, and the units stay committed against eating or
selling until something resolves it. Broadcast offers never lapse: they name
nobody to drift from.

## Data, not code

The data files below are the single source of truth for what would otherwise be
strings baked into Rust. The host reads them and passes strings to this crate:

| File | Owns |
|---|---|
| `assets/prompts/turn.j2` | the turn prompt (minijinja) |
| `assets/prompts/strings.toml` | the sheet's micro-strings |
| `assets/sounds/catalog.toml` | the sound catalog: percepts, radii, and the `sfx_prompt` `scripts/generate_sounds.py` synthesizes each asset from |
| `assets/world/items.json` | item kinds, metadata domains, display names, prices, and satiety |
| `assets/world/food.json` | market listings/restock, named counters, stock plans, transforms, working capital, and historical stock |
| `assets/world/rounds.json` | ordinary daily rounds plus fixed road-party schedules, manifests, wallet floats, and routes |
| `assets/world/seed.json` | Shared items and the player record. |
| `assets/world/areas.json` | named world geography: coordinate axes, stable IDs, prompt labels, and non-overlapping box unions used for containment and nearest-area descriptions |
| `lore/characters/**/*.json` | The authored cast (some 500 NPCs; one JSON per character), significance/status metadata, relationships, memories, items and canonical spawn transforms. Sorted relative paths seed the significance-aware turn order. |
| `lore/core_lore/occupations.json` | Occupation display names, locations and valid character titles. |

The loaders take `&str`, never a path: the host reads the file.

## Tests

`cargo test -p cathedral-sim`. All offline, all deterministic — `FakeCognition`
parses the *rendered* prompt's markdown sheet (the `**you**` line and the
`**since_your_last_turn**` section), so a renderer that drifts breaks the
end-to-end test instead of silently passing.

`tests/golden_prompts.rs` is the byte-diff that pins the prompt. Its scenario
worlds were generated from Python HEAD; the prompt bytes are **blessed** — the
Rust renderer is the truth, and the sheet has rendered as markdown (not JSON)
since the token-cost change of 2026-07. They remain the witness that the prompt
still says what it said — change one only when you have decided to change the
prompt, via the ignored `regenerate_golden_fixtures` test.

## Known gaps (intentional, for now)

- Some verbs from the original prompt sketch are not implemented (`eat` and,
  since M5, `go_to` are) — characters may narrate world changes the sim does
  not model.
- Memory/goal hygiene is prompt-enforced only: the prompt tells characters to
  record outcomes the turn they happen, forget superseded memories, and
  clear/replace achieved goals. Nothing in the sim guarantees it.
- Recent history is a bounded short-term aid, not durable memory.
- `Sight` (occlusion, NPC-eye screenshots) is plumbed as a trait but stubbed:
  `line_of_sight` always returns true and `npc_pov_frame` always returns `None`.
