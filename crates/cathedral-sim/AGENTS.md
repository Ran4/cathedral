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

Major and minor NPCs take autonomous turns in a significance-weighted stream;
ambient NPCs enter it only when speech or a nearby event wakes them. Each turn:

1. `prompt::render_prompt_and_drain` renders the character's full sheet
   (backstory, location, visible people, held items, memories, goal) plus a
   `since_your_last_turn` field drained from that character's **inbox**. The
   template is `assets/prompts/turn.j2`; the strings are
   `assets/prompts/strings.toml`. Nothing about the LLM text format lives
   anywhere else.
2. The prompt goes to the provider as one stateless call — no provider-side chat
   history. Each character carries a bounded `recent_history` (16 entries) of
   received speech, heard sound percepts, and its own lines. The two fields never
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
headless/developer use (`cathedral-headless --know-everybody`). Major lore NPCs
receive four autonomous-order slots for each minor slot; ambient NPCs receive
none, but player speech and event priority can schedule them even when the idle
order is empty. Provider requests cap output at 1,200 / 700 / 350 tokens for
major / minor / ambient turns. Significance itself never enters the prompt.

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
- `eat {"item_id": "<id>"}` — consume a held item; it leaves the world (a pending
  offer of it is implicitly retracted, with notification).
- `make_sound {"sound_id": "<id>"}` — emit a catalog sound
  (`assets/sounds/catalog.toml`); only rows with `actor_emittable = true`.
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

## Data, not code

The data files below are the single source of truth for what would otherwise be
strings baked into Rust. The host reads them and passes strings to this crate:

| File | Owns |
|---|---|
| `assets/prompts/turn.j2` | the turn prompt (minijinja) |
| `assets/prompts/strings.toml` | the sheet's micro-strings |
| `assets/sounds/catalog.toml` | the sound catalog: percepts, radii, and the `sfx_prompt` `scripts/generate_sounds.py` synthesizes each asset from |
| `assets/world/seed.json` | Shared items and the player record. |
| `assets/world/areas.json` | named world geography: coordinate axes, stable IDs, prompt labels, and non-overlapping box unions used for containment and nearest-area descriptions |
| `lore/characters/**/*.json` | The 500-NPC authored cast, significance/status metadata, relationships, memories, items and canonical spawn transforms. Sorted relative paths seed the significance-aware turn order. |
| `lore/core_lore/occupations.json` | Occupation display names, locations and valid character titles. |

The loaders take `&str`, never a path: the host reads the file.

## Tests

`cargo test -p cathedral-sim`. All offline, all deterministic — `FakeCognition`
parses the *rendered* prompt's ```json fence, so a template that drifts breaks
the end-to-end test instead of silently passing.

`tests/golden_prompts.rs` is the byte-diff against Python. Its fixtures were
generated from Python HEAD and are now **frozen**: the generator is deleted and
the Rust renderer is the truth. They remain the last independent witness that the
prompt still says what it said — change one only when you have decided to change
the prompt.

## Known gaps (intentional, for now)

- `move_to` and other verbs from the original prompt sketch are not implemented
  (`eat` is) — characters may narrate world changes the sim does not model.
- Memory/goal hygiene is prompt-enforced only: the prompt tells characters to
  record outcomes the turn they happen, forget superseded memories, and
  clear/replace achieved goals. Nothing in the sim guarantees it.
- Recent history is a bounded short-term aid, not durable memory. Repeated
  identical percepts are not yet coalesced
  (`features/small_thing_deduplicate_repeat_recent_history.md`).
- `Sight` (occlusion, NPC-eye screenshots) is plumbed as a trait but stubbed:
  `line_of_sight` always returns true and `npc_pov_frame` always returns `None`.
