# Sounds (non-speech percepts)

Status: **implemented** (2026-07-13). The catalog lives in
`assets/sounds/catalog.toml` (it was `prompt_playgound/sounds.py` until the
sidecar was ported into `crates/cathedral-sim`), assets in `assets/sounds/`,
generation via `scripts/generate_sounds.py`.

Generalises the one perception rule the sim has today — *"everyone within 20 m
hears you"* — into a catalog of non-speech sounds with per-sound radii, and adds
the sim's first **sight** test: whether a hearer also *saw who did it*.

The first entry in the catalog is a fart on the `F` key, because it is the
cheapest possible way to play-test the whole loop. The point is not the fart.
The point is that a broken beer glass, a rung town bell, and a lamplighter's
whistle are then all one row of data each.

## Summary

A sound is emitted at a position with an audible radius. Everyone inside that
radius **hears** it. Those among them whose **view cone contains the actor**
also **see** who did it, and get an attributed percept instead of an anonymous
one:

| | percept |
| --- | --- |
| heard **and** saw you | `Sven farted.` |
| heard, but you were behind them | `[You heard a big fart!]` |
| out of range | nothing |

That single split is the whole feature. It gives us, for the price of one event
type: sneaking up behind people, anonymous acts in a crowd, bells that carry
across the city, and — because attribution runs through the existing
`identify()` — **"A stranger (id p0) farted."** for a witness who doesn't know
you yet.

This is the design/06 `sound` event (`lore/second_sun/design/06_the_sound_of_the_city.md` §5),
built early and small. Bells, cries and the name-knell are later rows in the same
table, not a second mechanism.

## Naming

The event type is `sound`, not `fart` and not `noise`.

`fart` as a first-class verb was rejected: the verb name is most of the semantics
the model sees, and a `fart` verb teaches the LLM nothing reusable. `make_sound
{"sound": "fart"}` and `make_sound {"sound": "glass_break"}` are the same motor
action with different data, which is exactly what they are.

`sound` (not `noise`) because a sound has a source and a meaning; noise is what
you call a sound you don't want. The bell is not noise.

## The catalog

One table, `assets/sounds/catalog.toml`, and it is the **single source of truth
for three consumers**: the sim (`cathedral_sim::SoundCatalog`, which emits and
renders percepts), the generator script (which makes the wav/mp3), and Bevy
(which resolves the asset by convention from the id alone).

```toml
[[sounds]]
sound_id = "fart"            # [a-z_]+ — also the asset basename
sound_class = "body"         # body | impact | bell   (design/06's `class`)
audible_distance = 20.0
heard = "[You heard a big fart!]"   # unattributed percept — everyone in radius
seen = "{actor} farted."            # attributed percept; omitted => not attributable
sfx_prompt = "..."                  # ElevenLabs generation prompt
duration_seconds = 2.0
actor_emittable = true              # may an LLM choose this via make_sound?

# ... glass_break (impact, 25 m, emittable, seen = "{actor} broke a beer glass.")
# ... town_bell   (bell, 600 m, not emittable, no `seen`)
```

A missing `seen` means **not attributable**: there is no witness split and
everyone in range gets `heard`. Nobody wonders *who* rang the town bell, which is
also what makes a 600 m radius sensible.

`{actor}` is rendered per-recipient through `perception::identify`, so strangers
stay strangers — see Events.

## Perception

Two independent tests. A recipient must pass the first; passing the second only
upgrades their percept.

**1. Audible — 3D radius, no occlusion.** `world.characters_within(source, audible_distance)`,
the same call speech already uses at 20 m. Walls do not block sound, because
**the sidecar has no geometry** — it only ever receives positions
(`spatial_update`), so it cannot raycast. This is not a shortcut we're taking; it
is the existing model (speech already carries 20 m through solid stone) and it is
what design/06 §2 specifies: *"3D radii, no occlusion."*

**2. Seen — a horizontal view cone.** The recipient sees the actor when the angle
between the recipient's facing and the direction to the actor is within
`view_cone_degrees / 2`.

- **135° total FOV** (67.5° half-angle) — configurable. You are anonymous
  anywhere outside a fairly narrow forward wedge, which makes attribution the
  interesting case rather than the default.
- **The cone is horizontal (XZ plane only).** Facing is a single yaw, so there is
  no vertical component to test against. The *radius* is full 3D; the *cone* is
  a compass bearing. An NPC on a balcony directly above you is judged by their
  yaw alone.
- No occlusion here either, for the same reason. You can be "seen" through a wall.
  Accepted, and flagged in Known gaps.

### Facing: the blocker this feature has to solve

Nothing in the stack has an orientation today. `Character` (`sim.py:141`) has
`position_m` and nothing else spatial; `public_snapshot` (`sim.py:277`) ships no
orientation; and Bevy slams `transform.rotation = Quat::IDENTITY` on every actor
(`actors.rs:165`), so they all face the same arbitrary world direction.

**NPCs — a new seeded, static `facing_yaw` on `Character`.** NPCs never move
(there is no `move_to` verb), so a facing seeded at world creation is enough: you
walk around *them*. Sidecar stays authoritative; no protocol rule is relaxed.

**This must be rendered.** `actors.rs:165` becomes
`Quat::from_rotation_y(actor.facing_yaw)`, fed from the snapshot. If the sim
thinks an NPC faces away and the render draws them facing you, the rule is
unlearnable and the whole feature is noise. **The render is the only place the
player can read the rule from.**

**The player — a new `facing_yaw` on the player's `spatial_update`.** The player
is subject to the identical test: an NPC who farts behind your back gets
`[You heard a big fart!]` in your HUD, not their name. This keeps the sidecar
authoritative for *all* perception (Bevy never decides what the player knows) and
keeps the rule symmetric — if you can sneak up on them, they can sneak up on you.

Requires widening the strict payload check at `server.py:874`
(`_exact_payload(raw_update, required={"actor_id", "position_m"})`).

## Verbs

### `make_sound {"sound": "<sound_id>"}`

Make a sound at your own position.

- Validation: `sound_id` exists in the catalog **and** is `actor_emittable`.
  Otherwise → `system:` event in the actor's own inbox (`"there is no sound
  'burp'"`). Ids only; no name-fallback, per the house rule.
- `town_bell` is not `actor_emittable` — an NPC cannot ring the city bell by
  saying so. World sounds have world causes (see Known gaps).
- The prompt renders the emittable ids inline, so the model sees its options.

The player uses the same code path via a `player_sound` bridge message (below),
not this verb — the player is never scheduled and never has a turn.

## Wire

A third `event_type` beside `speech` and `world_event` (`sim.py:125`), and a
third server message type. It needs its own type rather than a `world_event`
kind because `WorldEventWire` (`mod.rs:352`) carries `(kind, actor_id, target_id,
item_id, recipients)` — no position, no radius, no asset. A sound needs all three.

```json
{"event_id": "sound-12", "sound_id": "fart", "class": "body",
 "actor_id": "p0",
 "position_m": {"x": 1.0, "y": 0.0, "z": 2.0},
 "audible_distance": 20.0,
 "recipient_ids": ["c1", "c2", "c3"],
 "witness_ids":   ["c1"]}
```

`witness_ids` ⊆ `recipient_ids`. `actor_id` is nullable (a world sound has no
actor). Bevy uses `position_m` + `sound_id` for positional playback and
`witness_ids` to caption the player's HUD; it changes no state, exactly like
`world_event` — offers and ownership still reconcile only from snapshots.

**Player input:** `player_sound {"sound_id": "fart"}`, fire-and-forget with no
`command_result` (there is no failure the player can act on), following the
`player_offer` / `player_accept` / `player_decline` / `player_retract` pattern at
`bridge.rs:276-317`.

## Events

All percepts render per-recipient through `perception::identify`, so the existing
`knows` machinery does the attribution work for free. Sven farts, in range of
three people:

| recipient | knows Sven? | facing Sven? | inbox |
| --- | --- | --- | --- |
| Conny | yes | yes | `Sven farted.` |
| Ilse | no | yes | `A stranger (id p0) farted.` |
| Mott | yes | no | `[You heard a big fart!]` |
| Pike (40 m away) | — | — | *nothing* |

Note Mott gets no id at all. **A percept you didn't see must not leak who it
was** — including through the id.

Percepts are past-tense history with no action hint, like offer events: by the
time a character reads one, the farter may have walked away.

**The player has no inbox** (never scheduled, so it would never be drained). The
player's percept is a HUD toast, carrying the same two-tier text.

**Scheduler nudge.** A percept sitting in an inbox does nothing until that actor's
next turn, which round-robin may not reach for a while — and "fart near people and
watch them react" is dead if the reaction lands 30 seconds later. So the sim
`prioritize()`s the **nearest witness** (`scheduler.py:153`, already exists, already
supports `immediate=`). One nudge per sound: the turn stream is global and single,
so there is nothing to be gained by prioritising all of them.

## Assets

**One-shots are mp3, the ambient loop is wav.** Requires adding `"mp3"` to the
Bevy features (`Cargo.toml:10`) — it resolves to `rodio/mp3` → symphonia, a
pure-Rust decoder, no C toolchain, no patents (expired 2017).

Why not mp3 for everything: mp3 carries encoder delay and end padding in the
format, and rodio does not honour LAME gapless tags, so a **looped** mp3 clicks
at the wrap point. The fireplace is the only looping asset, so it is the only wav.

Why not wav for everything: ElevenLabs returns mp3 on every tier, while its PCM
output formats are tier-gated. mp3 removes both a conversion step and a possible
tier failure. It is also ~5× smaller in a git-tracked `assets/` — a 4 s bell is
~350 KB as wav, ~64 KB as mp3, and design/06 eventually wants eight bell strokes
plus cries, songs and a choral verse.

**Resolution is by convention, and no filename ever crosses the wire:**

| Sound | Resolved by | Format |
| --- | --- | --- |
| catalog sounds | `assets/sounds/{sound_id}.mp3`, id validated `^[a-z_]+$` | mp3, one-shot |
| ambient loops | named directly in Bevy scene code | wav, gapless |

Unknown or unplayable id → **skip playback, still toast the percept**. The sim is
authoritative; a missing asset must never silence a percept.

**Playback** reuses the NPC-voice path verbatim (`speech.rs:745`):
`PlaybackSettings::DESPAWN.with_spatial(true).with_spatial_scale(...)`, so
positional audio and inverse-square attenuation come for free.

### Ambient sound is not an event

A fireplace is a looping `AudioPlayer` in the Bevy scene. It never enters the
event stream, and the sidecar does not know it exists. An inbox line per crackle
is token suicide — design/06 §5 dedupes ambient hard for exactly this reason —
and NPC awareness of the fire is already covered by `location_description`, which
ships today.

Ambient assets still need generating, so they live in a second table in
`catalog.toml` (`[[ambients]]`) which the generator reads and the sim never does.

### Generation

`scripts/generate_sounds.py` — a uv inline script, **idempotent by design**:

1. read the catalog (`[[sounds]]` + `[[ambients]]` from `catalog.toml`),
2. diff against `assets/sounds/`,
3. generate **only what is missing**, via ElevenLabs sound-generation using each
   row's `sfx_prompt` and `duration_seconds`.

So: **don't like a sound? delete the file and re-run.** Nothing else regenerates,
nothing else costs credits.

`ELEVENLABS_API_KEY` lives in the repo-root `.env` (gitignored), alongside
`OPENAI_API_KEY`.

## Keybindings

| key | before | after |
| --- | --- | --- |
| `F` | toggle fly | **fart** (`player_sound {"sound_id": "fart"}`) |
| `'` | — | **toggle fly** (`controller.rs:261`) |

**`'` means `KeyCode::Quote`, which is a *physical* key code** (Bevy/winit use
US-layout positions). On the sv-SE layout that same physical key is **ä**. This is
chosen deliberately, but it is worth writing down so nobody "fixes" it later.

## Config

Additive under `smart_actors` in `config.ron`:

```ron
sounds: (
    enabled: true,
    view_cone_degrees: 135.0,                 // total FOV for the seen-test
    min_seconds_between_player_sounds: 2.0,   // see Edge cases
),
```

`view_cone_degrees` is exposed precisely because 135° is a guess. It is the one number
in this feature that only play-testing can settle.

## Edge cases

- **Key-mashing `F` floods inboxes.** Every fart is a percept in up to N inboxes,
  and inboxes are prompt tokens. `min_seconds_between_player_sounds` (default 2) rate-limits
  `player_sound`; sounds inside the cooldown are dropped silently at the sidecar,
  not queued. Without this, holding `F` is a denial-of-service on your own LLM bill.
- **Nobody in range.** No event recipients, no percepts. The player still gets a
  HUD confirmation, or the key feels broken.
- **The actor never hears themselves.** `characters_within` already excludes the
  origin actor. The farter's own knowledge of the fart is not a percept.
- **`audible_distance` > 20 m is new ground.** `town_bell` at 600 m is the first thing in
  the codebase to exceed the 20 m speech constant. It is the proof that per-sound
  radii work, and the reason the radius is a catalog column instead of a constant.
- **Seen-through-walls.** A consequence of the sidecar having no geometry. An NPC
  facing your direction through a closed door will name you. Accepted for now.
- **Unknown `sound_id` on the wire** → Bevy skips playback but still toasts the
  percept.
- **Sound during speech.** No interaction with the conversation floor: a fart does
  not take the floor, interrupt TTS, or delay a turn. It is a percept, not an
  utterance.

## Decisions

1. **Generic `sound`, not a `fart` verb.** The catalog is data; the mechanism is
   one event. A beer glass, a bell and a whistle are rows, not features.
2. **New wire type `sound`, not a `world_event` kind.** `world_event` has nowhere
   to put position, radius, or asset.
3. **Two recipient sets in one event** (`recipient_ids` ⊇ `witness_ids`) rather
   than two events. This is exactly the shape design/03's `witnessed` predicate
   will need later.
4. **135° total FOV**, configurable. Anonymity is the default; being seen is the
   interesting case.
5. **The cone is horizontal.** Facing is a yaw; there is nothing honest to test
   vertically.
6. **Seeded static NPC facing**, sidecar-authoritative, rendered by Bevy. NPCs
   don't move, so this is sufficient — and it is the *only* option that keeps
   Bevy projection-only.
7. **The player is subject to the same cone**, which costs one new field on
   `spatial_update` and buys a symmetric, learnable rule.
8. **Not-seen percepts carry no id.** Fail dark: an unattributed sound must not
   leak its actor through the wire.
9. **mp3 one-shots, wav loops.** The only reason not to use mp3 everywhere is
   gapless looping, and the only loop is the fireplace.
10. **Ambient is a Bevy scene loop, outside the event system entirely.**
11. **NPCs may emit sounds** (`actor_emittable`), so a tavern can break its own
    glasses. World sounds (`town_bell`) are not emittable by an LLM saying so.

## Known gaps (intentional, for now)

- **`town_bell` has no simulation source.** There is no clock, no calendar, and no
  weather in the sim, so nothing *causes* a bell. It ships as a catalog row plus a
  `CATHEDRAL_DRIVE` trigger, honestly labelled — exactly as design/06 §7 ships the
  Ruin ("no fire or flood system exists").
- **No occlusion**, for hearing or for sight. Needs geometry the sidecar doesn't
  have; would require Bevy to compute visibility and push it, which inverts the
  authority model.
- **No dedupe.** design/06 §5 wants ambient percepts deduped per actor per day.
  Nothing here is ambient yet, so the cooldown is the only limiter.
- **Facing is static.** NPCs never turn to look at you. When they do (a `look_at`
  verb, or facing whoever last spoke), this feature gets much better for free.

## Tests

Offline, deterministic, in the existing Python suite
(`uv run --offline --no-project python -m unittest discover -s tests`):

- **S1** — a fart emits one `sound` event; recipients are exactly those within 20 m.
- **S2** — the witness split: an NPC facing the actor gets `Sven farted.`; an NPC
  facing away gets `[You heard a big fart!]`.
- **S3** — a witness who does not `know` the actor gets `A stranger (id p0) farted.`
- **S4** — a non-attributable sound (`town_bell`) produces no `witness_ids`;
  everyone in range gets the same percept.
- **S5** — `audible_distance` is honoured per sound: an actor 30 m away hears the bell and
  not the fart.
- **S6** — the cone is horizontal: an actor directly above the source is judged on
  yaw alone.
- **S7** — the player's own `facing_yaw` gates the player's HUD percept.
- **S8** — a non-emittable or unknown `sound_id` in `make_sound` → `system:` error
  in the actor's own inbox, no event.
- **S9** — a second `player_sound` inside `min_seconds_between_player_sounds` is dropped.

## Demo scenario

The whole feature, playable in thirty seconds:

Walk up to Conny face-to-face and press `F`. She sees you: *"Ilse farted."* — and
says something about it. Walk **behind** her and press `F` again. Same sound, same
radius, but now: *"[You heard a big fart!]"* — she knows someone did it, turns
nothing up, and has no one to blame.

Then do it in front of a stranger who doesn't know your name yet, and listen to
them try to describe you.

```sh
# D1 — seen, then unseen, from the same distance
CATHEDRAL_DRIVE='wait-online; key KeyF; sleep 6; shot farted_in_her_face; quit' cargo run
# assert: one `sound` event, sound_id "fart"; Conny in witness_ids;
#         her inbox line names the actor; her next turn reacts to it
```
