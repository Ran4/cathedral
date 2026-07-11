# Smart actors

Status: implemented on 2026-07-10.

This feature brings the simulation in `prompt_playgound/` into the Bevy game.
It also supersedes the "Do NOT implement in the bevy application yet" note in
`features/giving_things.md`: item offers are part of this vertical slice.

The repository directory is currently spelled `prompt_playgound/`; all paths in
this document use that real spelling.

## Outcome

The game starts Sven, Conny, and Ilse as stationary people near one another.
They retain the Python prototype's identities, inventories, memories, goals,
knowledge, LLM prompts, and consent-based item-transfer rules. They talk to one
another autonomously, react to the human player, and can exchange singular item
entities.

The player is represented in the same Python world as a character, but is
human-controlled and is never sent to an LLM. With a microphone, voice input is
enabled by default and the player can simply speak naturally. Voice activity is
split into completed utterances, and each accepted transcription is applied as
an open/broadcast player `say` action. For example:

1. Say aloud, "What's your name?"
2. Say aloud, "Ilse, please offer me a coin."
3. If she chooses `offer_item`, see the coin held above her head and an offer
   card in the HUD.
4. Press `Y` to accept it or `N` to decline it.
5. Select a held item in the player's inventory, look at an actor, and
   right-click to offer it back.

All speech is local. An utterance can affect only characters within 20 metres
of the speaker at the moment it is spoken. Addressing one person is not a
private channel: other characters inside that radius hear it as bystanders.
The player sees the text and hears synthesized speech only when the player was
one of those recipients.

## Scope

The first complete slice includes:

- the seeded Sven, Conny, and Ilse scenario from `prompt_playgound/main.py`;
- stationary, targetable actor bodies and name labels;
- a persistent non-blocking Rust/Python bridge;
- Python-owned NPC cognition, world rules, inventories, and offers;
- metre-based proximity in place of textual "same location" checks;
- world-anchored speech text, subtitles, and NPC text-to-speech;
- default-on, voice-activated microphone capture and speech-to-text for the player;
- a player inventory quickbar and gaze-based item offering;
- Zelda-style offered-item visuals above the giver;
- the `Y`/`N` incoming-offer HUD; and
- offline automated tests with fake LLM, transcription, and speech backends.

The following are explicitly out of scope:

- actor walking, pathfinding, schedules, or a `move_to` action;
- actor combat, animation graphs, lip sync, or production character art;
- physical collision with actors;
- hearing occlusion, reverberation zones, or language translation;
- wake-word detection, typed chat, retaining ambient audio, or streaming raw
  microphone audio to a provider;
- item stacks, currency quantities, atomic barter, theft, or dropped items;
- save-game persistence, multiplayer, and simulating a remote server;
- moving LLM calls, prompt rendering, reply parsing, or action execution to
  Rust; and
- automatically inferring learned names from arbitrary dialogue. The existing
  explicit `knows` data remains the source of NPC name knowledge for this slice.

## Settled constants and controls

Bevy units are already metres, so no coordinate conversion is needed.

| Name | Value | Meaning |
| --- | ---: | --- |
| `HEARING_RADIUS_M` | `20.0` | Maximum distance for speech recipients and nearby-person prompt context. |
| `ITEM_INTERACTION_RADIUS_M` | `4.0` | Maximum distance for offer, accept, and decline actions. |
| `PLAYER_SPEECH_MAX_SECONDS` | `15` | Hard cap for one voice-detected utterance. |
| `PLAYER_SPEECH_MAX_CHARS` | `500` | Maximum accepted transcription or `say` text after trimming. |
| `POSITION_UPDATE_HZ` | `10` | Maximum player-position update rate sent to Python while moving. |

Distances use full three-dimensional Euclidean distance between consistently
defined actor body-centre positions. The boundary is inclusive:
`distance_squared <= radius * radius`. There is no line-of-sight or wall
occlusion in the hearing rule.

| Input | Action |
| --- | --- |
| `V` | Toggle microphone listening on/off. It is on by default. |
| Mouse wheel / `1`-`9` | Select a held item in the inventory quickbar. |
| Right click | Offer the selected item to the actor under the crosshair, if within 4 m. |
| `Y` | Accept the active incoming offer. |
| `N` | Decline the active targeted offer, or locally dismiss an open/broadcast offer. |
| `R` | Retract the player's pending offer of the selected item. |

World/item actions require the game to own the cursor and the relevant HUD
control not to be waiting for a Python result. `V` remains a direct persistent
microphone toggle whenever the app receives keyboard input. Existing movement
bindings keep their current meaning.

## Authority boundary

Python remains the single authority for character simulation. Rust owns the
physical game client and keeps a read-only projection of the semantic state.

| Concern | Authority | Mirrored/consumed by the other side |
| --- | --- | --- |
| NPC prompts, memories, goals, inboxes, knowledge, and turn scheduling | Python | Rust receives only status needed by the HUD. |
| Reply parsing and validation of every action | Python | Rust may pre-check an interaction for UX, but a pre-check never commits state. |
| Item ownership and pending offers | Python | Rust mirrors IDs and current records for visuals, quickbar, and offer cards. |
| Hearing-recipient selection and NPC inbox delivery | Python | Rust receives the frozen recipient list on each speech event; the player does not accumulate an unread Python inbox. |
| Actor and item stable IDs/profile data | Python | Rust uses typed ID wrappers and snapshot data. |
| NPC spawn hints | Python | Rust creates the entity at the hint and then owns its actual transform. |
| Player/NPC transforms, camera, gaze target, and input | Rust | Python receives position snapshots and action intents. |
| Actor meshes, text, offered-item props, HUD, microphone capture, and playback | Rust | Python supplies structured events, transcripts, and generated WAV files. |
| STT and TTS provider calls and API credentials | Python | Rust supplies/consumes temporary WAV files only. |

"Mirrored in Rust" deliberately does not mean a second simulation. Rust has
`ActorId`/`ItemId` types, a `WorldMirror`, range/focus pre-checks, and all view
state, but it never moves an item or resolves an offer optimistically. It sends
an intent and waits for the next authoritative Python snapshot.

## Python domain model

### Characters

Extend `Character` rather than creating a separate player type:

```python
@dataclass
class Vec3:
    x: float
    y: float
    z: float

@dataclass
class Character:
    id: CharIdStr
    name: str
    control: Literal["llm", "player"]
    back_story: str
    location_description: str
    position_m: Vec3
    appearance_key: str
    voice_key: str | None
    holds: list[ItemIdStr] = field(default_factory=list)
    goal: str = "None"
    memories: list[str] = field(default_factory=list)
    inbox: list[str] = field(default_factory=list)
    knows: set[CharIdStr] = field(default_factory=set)
```

`control` is the important distinction. Only `"llm"` characters enter the NPC
turn scheduler. The player can still use the same `say`, `offer_item`,
`accept_offered_item`, `decline_offer`, and `retract_offer` code paths.

All received coordinates must be finite. Protocol input containing NaN,
infinity, an unknown actor ID, or an older spatial sequence is rejected rather
than corrupting world state.

### Items and offers

Keep singular world items and add a renderer-facing key:

```python
@dataclass
class Item:
    id: ItemIdStr
    name: str
    visual_key: str

@dataclass
class Offer:
    item_id: ItemIdStr
    giver_id: CharIdStr
    target_id: CharIdStr | None
    created_seq: int
```

`World.offers` remains keyed by item ID, so an item has at most one pending
offer. More than one item may be pending from the same giver. The item remains
in `giver.holds` until acceptance.

The transfer semantics in `features/giving_things.md` remain normative, with
these spatial replacements:

- `offer_item` requires a non-self target, when present, within 4 m;
- `accept_offered_item` revalidates that giver and receiver are within 4 m;
- `decline_offer` is valid only for an offer targeted at the actor and while the
  giver is within 4 m;
- a broadcast offer can be accepted by any non-giver currently within 4 m, and
  cannot be declined globally;
- `retract_offer` needs no proximity check; and
- pending offers do not expire merely because participants separate. They stop
  appearing as actionable until the participants are close again.

Acceptance is the only transfer operation. Re-offering an item replaces its
old offer exactly as in the prototype, including notifying a displaced target.
Eating an offered item implicitly retracts it.

Offer, accept, decline, retract, and eat history is delivered to an
LLM-controlled observer's inbox only when that observer is within 20 m as the
action happens. The same frozen nearby set is carried in structured events for
the human-facing client, without accumulating prose in the unscheduled
player's inbox. The acting character knows its own result directly. A distant
former target receives no magical notification; current `you_offer`,
`offered_to_you`, and HUD state still reconcile from the authoritative offer
table rather than relying on that historical event.

### Seed world

Preserve the current IDs, biographies, memories, knowledge, and inventory:

- Sven (`sv3n1`) is the blacksmith apprentice, knows Conny, and holds fish
  `fzbn9`;
- Conny (`cb947`) is the fishmonger, knows Sven, and remembers that Sven owes
  two coppers; and
- Ilse (`k0fb1`) is the hungry pilgrim, knows nobody, and holds copper coin
  `c0prs`.

Add one human character with stable ID `player`, empty inventory, no voice, and
`control="player"`. Seed the player's `knows` set with the three demo actors so
the first-slice HUD can label them by name. NPCs do not automatically know the
player.

Spawn the trio on clear ground just outside the cathedral's west entrance,
near the near edge of the grand forecourt:

| Actor | Body-centre position in metres |
| --- | --- |
| Conny | `(0.0, 0.91, 112.0)` |
| Sven | `(-1.8, 0.91, 114.0)` |
| Ilse | `(1.8, 0.91, 114.0)` |

All three are within both defined radii of one another. The exact cluster may
be translated slightly if visual inspection finds it intersects architecture,
but its spacing, easy access from player spawn, and forecourt setting must be
preserved.

## Spatial perception and speech

Replace `World.at_location()` with explicit queries. Keep the human-readable
`location_description` only for prose in the prompt.

`characters_within(origin, radius, exclude)` must be the one Python helper used
for hearing, prompt visibility, and bystander event delivery. It returns a
stable order (distance, then actor ID) so prompts and tests are deterministic.

### `say`

The Python action remains:

```text
say {"target": "<character id>", "text": "..."}
say {"text": "..."}
```

Rules:

1. `text` must be a string, is trimmed, must not be empty, and is limited to
   500 characters.
2. Omitted or JSON `null` `target` means broadcast to every other character
   within 20 m.
3. A present target must exist, must not be the speaker, and must be within
   20 m. Failure is an action error delivered back to the acting character.
4. Unlike the prototype's current `say` implementation, a bad, self, or distant
   explicit target must never fall back to broadcast.
5. An LLM-controlled target receives the perspective-specific "said to you"
   inbox event. Other LLM recipients in range receive the bystander form. The
   player is represented in the same structured recipient set but does not
   accumulate an unread Python inbox. Addressing is not privacy.
6. Recipient membership is frozen when the action is applied. Moving into range
   later does not reveal old text or start delayed audio.
7. Emit a structured `speech` event even when the player is not a recipient.
   The event contains the exact recipient IDs; Rust uses that list rather than
   recomputing who heard the historical utterance.

The speaker does not receive their own utterance in their inbox.

### Prompt changes

Keep `prompt.py` as the only definition of the LLM text format. `think.md` is a
stale sketch and is not an implementation contract.

For an NPC turn:

- `you_see.people` is the set of other characters within 20 m, including the
  human player;
- each person still has a stable ID and perspective-sensitive name;
- add `distance_m`, rounded to one decimal, to each visible person;
- change `you_are` to include `location_description` and `position_m`, while
  retaining natural-language location context;
- `offered_to_you` contains only currently valid offers whose giver is within
  4 m;
- `you_offer` continues to show all of the actor's pending offers, including a
  target who has walked away; and
- `since_your_last_turn`, current inventory, current offers, memories, and goal
  retain their present meanings.

There is no prompt for the player. A player transcription goes directly into
`apply_action(world, player, "say", ...)` as untrusted text data; it is never
parsed as an LLM action reply.

### Text presentation

When the player is in `recipient_ids`, Rust immediately:

- shows a padded UI speech bubble projected from the speaking NPC's world
  position, so it renders correctly through the existing 3D camera;
- adds a bottom-centre subtitle in the form `Ilse: ...`; and
- queues TTS for that utterance.

If the speaker is the player, show `You: ...` as confirmation but do not
synthesize or play the player's own voice. If the player is absent from the
recipient list, show no bubble, subtitle, notification, or audio; this prevents
text from leaking conversations on the other side of the map.

Speech bubbles wrap text and remain for `clamp(2 + characters / 15, 3, 10)`
seconds. If audio begins later, the corresponding subtitle remains until audio
finishes. Multiple utterances are queued by server event sequence rather than
overwriting one another.

## NPC scheduling and concurrency

Retain a single global, round-robin NPC turn stream. Do not run multiple LLM
turns concurrently in the first slice: sequential action application is part
of the prototype's behavior and makes item offers deterministic.

The service must nevertheless remain responsive while a blocking provider call
is in flight:

1. On turn start, render a prompt from the current world and atomically move the
   actor's existing inbox events into that prompt.
2. Run `llm_client.complete()` on a worker thread, never the protocol/state
   thread and never the Bevy render thread.
3. Events arriving while the request is in flight remain in the actor's now-new
   inbox for its next turn.
4. On completion, parse the existing `VERB {json object}` format in Python and
   apply actions sequentially against the latest world state. Revalidate every
   ID, type, ownership rule, and distance; the prompt-time snapshot grants no
   authority.
5. Parsing or action failures become `system:` inbox events as today. Malformed
   types must be handled too; arbitrary LLM output must never terminate the
   service.

After the current in-flight call, a valid player utterance addressed to an NPC
prioritizes that NPC for the next turn. Then normal round-robin order resumes.
This keeps spoken interactions responsive without allowing one actor to starve
the others. Use one configurable minimum delay between completed turns and
exponential backoff on provider failures.

The player is never inserted into the scheduler. No LLM call may have
`actor_id="player"`.

## Rust/Python protocol

### Process and transport

Add an uv inline-script entry point at `prompt_playgound/server.py`. Rust starts
it as an owned child process with argument-array execution equivalent to:

```text
uv run --script prompt_playgound/server.py --stdio --runtime-dir <session-dir>
```

Use newline-delimited UTF-8 JSON over the child's stdin/stdout. Stdout is
protocol-only: exactly one compact JSON object per line. Logs, prompts, raw LLM
replies, and tracebacks go to stderr. API keys remain in the Python environment
and never appear in a protocol message.

A dedicated Rust worker owns the child and both blocking pipes. It communicates
with Bevy through bounded channels that Bevy polls each frame. No Bevy system
may wait for process startup, JSON I/O, an LLM response, STT, TTS, or file I/O.

At startup Rust creates a private per-session directory under the OS temporary
directory and passes it to Python. Audio messages contain validated basenames,
not arbitrary paths. Both sides reject traversal or a path outside that
directory. The owner deletes each file after acknowledgement and removes the
directory on clean shutdown.

### Envelope

Every message has this envelope:

```json
{
  "protocol_version": 1,
  "session_id": "random-per-child-start",
  "message_id": "rust-42",
  "type": "player_offer",
  "payload": {}
}
```

Server events additionally carry a monotonically increasing `event_seq`.
Authoritative semantic snapshots carry a monotonically increasing
`world_revision`. Requests carry a unique `request_id`; exactly one
`command_result` eventually answers each accepted request.

Unknown protocol versions are fatal to the smart-actor connection, not to the
game. Unknown message types are reported and ignored. String IDs are opaque;
neither side derives behavior from their format or an item's display name.

### Rust to Python messages

| Type | Required payload | Meaning |
| --- | --- | --- |
| `hello` | supported version, player ID, current player position, spatial sequence | Begin handshake. |
| `spatial_update` | spatial sequence and changed `{actor_id, position_m}` records | Update Python's position mirror. In v1 only the moving player changes after initialization. |
| `player_recording` | request ID, WAV basename, null target, current position, spatial sequence | Ask Python to transcribe and apply one open player utterance. |
| `player_offer` | request ID, target actor ID, item ID, current position, spatial sequence | Invoke `offer_item` for the player. |
| `player_accept` | request ID, item ID, current position, spatial sequence | Invoke `accept_offered_item` for the player. |
| `player_decline` | request ID, item ID, current position, spatial sequence | Invoke `decline_offer` for the player. |
| `player_retract` | request ID and item ID | Invoke `retract_offer` for the player. |
| `audio_consumed` | speech event ID and WAV basename | Rust has copied the generated audio and Python may delete it. |
| `resync_request` | last accepted world revision | Request a complete current snapshot. |
| `shutdown` | empty | Stop scheduling, flush protocol output, and exit. |

Every player action message that depends on range includes the latest player
position. Python applies that position update before validating the action, so
a 10 Hz background position stream cannot create a stale boundary exploit.
For asynchronous transcription, the recording request's action position is
retained with the task: later movement cannot change who heard an utterance
that has already finished at the microphone.

### Python to Rust messages

| Type | Required payload | Meaning |
| --- | --- | --- |
| `ready` | capabilities and the full initial snapshot | Handshake succeeded; Rust may spawn actors and enable interactions. |
| `world_snapshot` | full public world projection and world revision | Atomically replace Rust's semantic mirror. |
| `speech` | event ID, speaker, optional target, text, speaker position, recipient IDs, player-perspective labels | One utterance occurred. |
| `world_event` | event ID, kind, actor/target/item IDs, recipient IDs | Structured offer/accept/decline/retract/eat feedback; never a machine-parsed transcript string. |
| `transcription_result` | request ID, text or error | Update the microphone HUD and show what the game understood. |
| `tts_ready` | speech event ID, WAV basename | Generated speech is ready for Rust to copy and play. |
| `command_result` | request ID, success flag, error code, player-safe message | Resolve pending input and show an error toast if needed. |
| `status` | subsystem (`llm`, `stt`, `tts`), state, optional actor ID/message | Drive small thinking/listening/degraded indicators. |

The full projection is intentionally small and simple for the first cast:

```json
{
  "world_revision": 7,
  "player_id": "player",
  "actors": [
    {
      "id": "k0fb1",
      "name_for_player": "Ilse",
      "control": "llm",
      "position_m": {"x": 1.8, "y": 0.91, "z": 114.0},
      "appearance_key": "ilse",
      "holds": ["c0prs"]
    }
  ],
  "items": [
    {"id": "c0prs", "name": "copper coin", "visual_key": "copper_coin"}
  ],
  "offers": [
    {
      "item_id": "c0prs",
      "giver_id": "k0fb1",
      "target_id": "player",
      "created_seq": 19
    }
  ]
}
```

Backstories, memories, goals, private knowledge, and NPC inboxes are not part of
the Rust projection.

### Ordering and resynchronization

- Python serializes all outgoing events through one writer, preserving
  `event_seq` order.
- Increment `world_revision` after every public semantic mutation. For the
  small initial world, send a complete snapshot after each offer/inventory
  mutation rather than designing a delta protocol.
- Rust accepts a snapshot only if its session matches and its revision is newer
  than the current mirror, except that an equal-revision full snapshot may
  complete an explicitly requested resync. It replaces the mirror atomically,
  then reconciles ECS visuals and HUD state.
- A sequence gap, malformed snapshot, or failed invariant triggers
  `resync_request`; it never causes Rust to guess the missing mutation.
- Resync is a hard command barrier: item input, transcript injection, position
  updates, and completed microphone recordings do not queue behind the request.
  Input resumes only after an authoritative replacement snapshot is accepted.
- If the initial `ready` snapshot is malformed but its capabilities are valid,
  Rust retains those capabilities and completes the handshake after the
  replacement snapshot arrives.
- A late LLM result from an old session is discarded. Within the same session,
  returned actions are revalidated against current state.

## Bevy implementation

Add `SmartActorsPlugin` after the controller and scene plugins. A suggested
layout is:

```text
src/smart_actors/
    mod.rs          plugin, system sets, shared constants
    model.rs        typed IDs, protocol DTOs, WorldMirror
    bridge.rs       child lifecycle, JSON Lines worker, channels
    actors.rs       actor/item meshes, spawn and snapshot reconciliation
    targeting.rs    camera ray, focus, interaction range
    interaction.rs  quickbar and player command intents
    speech.rs       bubbles, subtitles, dynamic audio, microphone capture
    hud.rs          status, inventory, offers, errors
```

Expose a unique `PlayerCamera` marker from `controller.rs` and attach the stable
player actor ID to the existing player root. Proximity uses the root/body-centre
transform; gaze and the audio listener use the child camera transform.

Use explicit ordered system sets:

1. drain bridge messages;
2. replace/reconcile `WorldMirror`;
3. update actor focus after transforms propagate;
4. collect smart-actor input and enqueue commands;
5. reconcile speech, offered-item visuals, and HUD; and
6. start ready audio without blocking.

### Actor visuals and targeting

The first slice may use original primitive placeholder people: a capsule/body,
sphere/head, distinct palette per character, and a billboarded name label.
Each NPC root has:

- `ActorId`;
- `ActorTarget` with a simple capsule or AABB hit volume;
- `SpeechAnchor` above the head;
- `OfferAnchor` above the speech anchor; and
- child render entities selected by `appearance_key`.

Do not register actor bodies in the append-only static `CollisionWorld`; actors
are non-solid in this slice so future movement does not inherit permanent
colliders.

For gaze targeting, cast the centre-camera ray against only `ActorTarget`
volumes, choose the nearest hit, and compare it with a lightweight raycast
against the controller's existing static AABBs so a wall blocks interaction.
Do not ray-test the thousands of rendered city meshes. Gaze focus is used only
for item interaction and right-click highlighting up to 4 m. Player microphone
speech is always open: every other character within 20 m hears it, independent
of where the player is looking.

Keep white world dialogue, subtitles, focus hints, microphone status, and toast
text readable against both the cathedral and sky by placing compact translucent
neutral-grey backdrops behind the laid-out text. Actor name labels and the
larger status/inventory/offer panels use the same dark translucent treatment.

### Offered-item presentation

An `Offer` is not ownership. Reconcile a separate visual copy of every pending
item beneath the giver's `OfferAnchor`; never reparent or remove the owned item
until Python's snapshot says ownership changed.

The item rises above the giver's head, bobs/turns gently, and remains there for
the full lifetime of the offer. Use `visual_key` to render a recognizable fish,
coin, or generic fallback prop. If one giver has several offered items, show all
of them in a centred horizontal fan; do not silently pick one. For a very large
future set, the renderer may show three props plus a `+N` label, but the three
demo items require no truncation.

Remove/reconcile a prop only from an authoritative snapshot after acceptance,
decline, retraction, replacement, consumption, or resynchronization. A brief
lift animation may begin immediately, but the final visible state is snapshot
driven.

### Inventory and right-click offering

The bottom HUD shows the player's current `holds`, resolving item names through
the mirrored item table. One item is selected; a one-item inventory selects it
automatically. Mouse wheel and number keys change selection.

Right click sends `player_offer` only when all local preconditions pass:

- the cursor is captured;
- Python is connected and no identical request is pending;
- the player has a selected item in the latest snapshot;
- the centre ray has a non-player actor hit; and
- the mirrored current distance is at most 4 m.

Python repeats all checks. While waiting, show `Offering copper coin to
Ilse...`; do not remove the coin locally. If that item already has an offer,
right-click intentionally uses the prototype's re-offer/retarget behavior.
`R` retracts that selected item's offer.

### Incoming-offer HUD

Derive actionable cards from the current snapshot, never from historical
`world_event` text. A targeted offer addressed to the player is actionable only
while its giver is within 4 m.

If several targeted offers are actionable, order them by `created_seq`, show
the oldest card, and display `+N more`; after it resolves, show the next. The
card reads, for example:

```text
Ilse offers you a copper coin
[Y] Accept    [N] Decline
```

`Y` sends the exact active `item_id` to `player_accept`; `N` sends it to
`player_decline`. Disable both choices until `command_result` and a reconciled
snapshot arrive, preventing key repeat and stale double acceptance.

An in-range broadcast offer may use the same queue after targeted offers, but
its wording is `Ilse offers a copper coin to anyone`. `Y` accepts it. `N` only
dismisses that offer locally until its record changes; it must not send
`decline_offer`, because one character cannot decline on behalf of everyone.

Walking out of 4 m hides an actionable card without deleting the persistent
offer. Returning makes a still-current offer actionable again.

## Microphone and speech services

### Capture

Use Rust microphone capture (for example `cpal`) because input state and device
feedback belong to the game client. Probe the default input device off the
render thread. Absence, permission denial, disconnection, or an unsupported
format changes the HUD to a text-only actor experience and does not stop the
game.

Listening is enabled by default and `V` toggles it. The audio callback only
converts/copies samples into a bounded channel; it performs no disk, network,
JSON, or Bevy work. A worker uses local voice activity detection with bounded
pre-roll and trailing-silence windows, writes each detected utterance as a valid
WAV using the negotiated sample rate/channel count, and rearms immediately.
Each utterance stops at trailing silence or 15 seconds, whichever occurs first.

Suspend capture while synthesized NPC speech plays so it cannot feed directly
back into transcription, then automatically resume if the user still has the
microphone enabled. A player utterance already in progress finishes first, and
NPC playback waits for the capture worker's suspension acknowledgement. A dead
audio worker drops optional TTS after a bounded wait instead of freezing input.
Show a persistent `MIC ON`, `MIC OFF`, or `MIC UNAVAILABLE` indicator. During
the brief hard resync barrier, keep the user's on/off preference intact and
show `MIC PAUSED — SYNCING` rather than falsely presenting it as toggled off.
Empty/silent or failed transcription produces no `say` action and shows a short
status message.

The temporary utterance WAV is deleted after transcription finishes or the
request is rejected. Pending and completed duplicate requests also clean up
their unconsumed WAVs without deleting the active request's file. Ambient audio
that does not pass voice detection is never written, sent, or retained as game
state.

### Python STT/TTS adapter

Add `speech_client.py` with small provider interfaces:

```python
def transcribe(wav_path: Path) -> str: ...
def synthesize(text: str, voice_key: str, output_wav: Path) -> None: ...
```

The initial adapter uses the existing Python OpenAI SDK, separately from the
configured Moonshot/OpenAI text LLM. Use the Audio Transcriptions endpoint for
the completed utterance WAV and the Speech endpoint with WAV output for NPC
text. Default to a low-latency transcription model and `tts-1`, but make both
model IDs environment settings so replacing a model never changes the game
protocol. The current endpoint contracts accept WAV transcription input and
WAV speech output; see the official [OpenAI Audio API reference](https://platform.openai.com/docs/api-reference/audio).

Configuration belongs in `prompt_playgound/.example.env`:

```text
OPENAI_API_KEY=
STT_MODEL=gpt-4o-mini-transcribe
TTS_MODEL=tts-1
TTS_VOICE_SVEN=
TTS_VOICE_CONNY=
TTS_VOICE_ILSE=
NPC_TURN_DELAY_SECONDS=1.0
```

Blank voice settings use documented built-in defaults chosen once for distinct,
consistent characters. Never imitate a real person's voice. If the text LLM
uses Moonshot and no `OPENAI_API_KEY` is present, text cognition still works;
STT and TTS report unavailable independently.

This slice uses completed-file transcription, not the Realtime API. Local voice
activity detection still produces one authoritative text utterance per player
action without streaming ambient microphone input to a provider.

### Playback

Enable Bevy audio and WAV decoding. Insert generated WAV bytes directly into
`Assets<AudioSource>` and play them with an `AudioPlayer` entity at the
speaker's transform. Attach one `SpatialListener` to the player camera with a
realistic ear gap (approximately `0.18` m), not Bevy's intentionally large
default example gap.

Python starts TTS only for NPC speech whose frozen recipient list contains the
player. Text events never wait for synthesis. Rust queues eligible clips by
speech event sequence, plays at most one NPC voice at a time, and drops a clip
that belongs to an old session or is too stale to match its subtitle. TTS
failure leaves the complete text experience intact.

Use a hard 20 m eligibility gate plus smooth distance gain from normal volume
near the speaker to almost silent at the boundary. Spatial playback supplies
left/right panning; it is not the source of gameplay eligibility.

## Failure behavior and safety

- Missing `uv`, a missing LLM key, child startup failure, malformed protocol,
  or a Python crash must not terminate or freeze the Bevy game.
- On disconnect, keep stationary actor meshes and the last snapshot visible,
  clear transient speech and actionable offer cards, disable smart-actor
  inputs, and show `ACTORS OFFLINE`. Do not accept actions against stale state.
- The first implementation may offer one explicit restart action. A restarted
  sidecar creates a new session and resets the Python demo world; persistence is
  out of scope.
- Provider timeouts/errors become bounded status messages and actor `system:`
  events where appropriate. Apply exponential backoff; do not spin or flood an
  API.
- Treat LLM replies, transcriptions, dialogue, IDs, and protocol JSON as
  untrusted. Validate types and lengths before using them in state, UI, paths,
  or TTS.
- Escape dialogue as text. It must never be interpreted as markup, a file path,
  shell syntax, or a protocol message.
- Launch `uv` with an argument array, not through a shell. Do not log secrets or
  full microphone audio.
- Bound all bridge queues. When overloaded, preserve authoritative snapshots
  and command results; drop/coalesce redundant spatial updates and surface a
  degraded status rather than exhausting memory.

## Expected file and dependency changes

Python remains uv-managed. Expected Python work:

- refactor `sim.py` for control kind, positions, strict validation, structured
  events, and metric queries;
- update `prompt.py` for metric nearby people while preserving its action text
  format;
- keep `main.py` as a terminal prototype using the same simulation functions;
- add `server.py`, `protocol.py`, `scheduler.py`, and `speech_client.py`;
- update `.example.env` without ever committing real credentials; and
- add offline tests under `prompt_playgound/tests/`.

Expected Rust work:

- add `mod smart_actors` and register `SmartActorsPlugin` in `main.rs`;
- expose/mark the unique player camera and attach the player actor ID;
- extend `config.ron` with enable/disable and non-secret smart-actor settings;
- add the smart-actor modules listed above; and
- add direct dependencies for JSON, bounded channels, microphone capture, and
  WAV writing. On Bevy 0.19, enable the minimal `bevy_audio` and `wav` features
  rather than every audio codec.

The exact crate versions should be selected at implementation time and locked
in `Cargo.lock`; the intended families are `serde_json`, `crossbeam-channel`,
`cpal`, and `hound`.

## Implementation order

### 1. Make the Python core service-safe

- Add positions, control kinds, strict action schemas, structured events, and
  metric range tests.
- Preserve the CLI demo and offer invariants.
- Fix explicit invalid `say` targets so they cannot broadcast accidentally.
- Separate inbox draining from the blocking LLM call and prove events arriving
  during a call remain queued.

Exit criterion: an offline fake LLM can run the seeded world plus a player,
with no Rust process and no paid API calls.

### 2. Add protocol and stationary actors

- Implement the stdio server, versioned DTOs, worker thread, handshake, and
  snapshot resync.
- Add `SmartActorsPlugin`, spawn the three actor visuals, and update player
  position in Python.
- Keep all interaction disabled until `ready` validates.

Exit criterion: starting the game shows stationary, named Sven, Conny, and Ilse
at the specified cluster; killing Python leaves the game responsive.

### 3. Add local text conversation

- Schedule NPC turns, emit structured speech, and deliver recipient inboxes.
- Add gaze focus, bubbles, subtitles, player-recipient filtering, and a
  developer-only injected-transcript command for automated/manual testing.

Exit criterion: two groups more than 20 m apart cannot receive or display each
other's speech; addressed nearby bystanders still hear it.

### 4. Add item exchange

- Reconcile inventory/offers, overhead props, the inventory quickbar,
  right-click offer, retraction, and the incoming `Y`/`N` queue.
- Exercise replacement, rejection, stale command, and multiple-offer cases.

Exit criterion: no visual or HUD action can cause ownership to diverge from the
Python snapshot, and an item moves only after acceptance.

### 5. Add voice

- Add microphone capability detection and default-on voice-activity capture.
- Add Python transcription/synthesis adapters and temporary-file lifecycle.
- Add dynamic spatial WAV playback and degraded text-only behavior.

Exit criterion: with configured audio services the full spoken demo works; with
no microphone or audio key, all text and item interactions still work.

## Verification

No automated test may make a live or paid provider call.

### Python tests

Run Python tooling through `uv`. Cover at least:

- squared-distance behavior just inside, exactly at, and just outside 20 m and
  4 m;
- targeted speech, broadcast speech, nearby bystanders, and distant exclusion;
- structured player delivery without growth of the unscheduled player inbox;
- explicit bad/distant `say` target failure without broadcast fallback;
- player actions using the same validators while the scheduler skips player;
- prompt-time inbox draining with an event arriving during an in-flight fake
  completion;
- malformed verb arguments never escaping as an uncaught exception;
- all offer/accept/decline/retract/eat invariants, including multiple offers;
- monotonic revisions, request deduplication (including duplicate-WAV cleanup),
  session rejection, and full snapshot recovery;
- fake STT/TTS success, timeout, and failure; and
- a scripted exchange requiring no network or credentials.

### Rust tests

Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`. Cover at
least:

- protocol encode/decode fixtures shared with the Python tests;
- atomic mirror replacement and rejection of stale session/revision data;
- interaction gating during resync and initial-ready recovery from a
  replacement snapshot;
- actor-only ray targeting, nearest-hit choice, static-wall blocking, and both
  range cutoffs;
- offered-prop reconciliation for create, replace, accept, decline, retract,
  and multiple props from one giver;
- targeted-offer ordering and exact `Y`/`N` item IDs;
- right-click never mutating local inventory;
- microphone-absent and Python-disconnected states; and
- event-sequence speech/subtitle queue behavior.

### End-to-end fake-backend test

Provide a deterministic fake mode selectable only by configuration/test build:

1. Inject the player's open transcript "What's your name?" without a gaze target.
2. Assert every character within 20 m receives it and Ilse can answer naturally.
3. Return a scripted Ilse `say` reply and assert one bubble/subtitle event.
4. Inject "Please offer me your coin" and return Ilse's `offer_item` action.
5. Assert the coin remains in Ilse's inventory, appears above her, and produces
   the correct HUD card.
6. Press simulated `Y`; assert exactly one authoritative transfer to the
   player and removal of the offer prop/card.
7. Select that coin and simulate right-click on Conny; assert the coin remains
   with the player while the new offer is pending.

## Acceptance criteria

The feature is complete only when all of the following are true:

- Sven, Conny, and Ilse appear near one another and remain at their initial
  positions for at least a ten-minute run.
- The Python process owns all LLM/action/item logic; Rust contains no LLM prompt
  renderer or reply parser and commits no inventory mutation on its own.
- An utterance at `<= 20.0` m enters the listener's next-turn context; one
  outside 20 m does not. Targeted speech is still heard by in-range bystanders.
- Nearby NPC speech is readable as text over a translucent neutral-grey
  backdrop and, with TTS configured, audible from the speaker's position.
  Distant speech produces neither text nor sound for the player.
- With a microphone, voice input is on by default and `V` toggles it. Every
  accepted transcript becomes an open player `say` action heard by all actors
  within 20 m, independent of gaze, and never invokes an LLM for the player.
- Asking Ilse her name and then asking her to offer her coin can produce a
  normal LLM response and offer without bespoke phrase matching.
- Every pending NPC offer has a faithful item prop above the giver. Several
  pending items from one giver are all represented.
- A current targeted offer to the player shows the correct giver/item and maps
  `Y` to accept and `N` to decline exactly that item. Accept transfers once;
  decline transfers nothing.
- The player can select a held item and right-click a focused actor within 4 m
  to create a pending offer. The item remains owned by the player until the
  actor accepts it.
- Missing microphone, missing speech credentials, provider failure, malformed
  LLM output, or a dead Python child degrades visibly without freezing or
  crashing the game.
- All offline Python, Rust, and fake end-to-end tests pass, and
  `features/smart_actors.md` is moved to `features/implemented/` only after the
  implementation satisfies this list.
