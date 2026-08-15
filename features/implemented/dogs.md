# Street dogs

**Status:** implemented (2026-07-31). Written as a record — there was no prior
spec; the request was "add dogs: no interaction, they just walk around, and
NPCs should always see them as nearby".

Ten hand-authored street dogs wander the city on the walkable surface and
appear on **every nearby character's sheet** — that last part is the point,
and it is what separates them from the rats design (`features/rats.md` §2.3,
"an ordinary rat never reaches the sim"): a dog *does* reach the sim, because
the ask was that the LLM always knows one is there.

## Shape

**Sim-owned, non-cognitive** — the `RoadCart`/`LampView` seam, not a
character and not render-only:

- `crates/cathedral-sim/src/dogs.rs` — `Dog`, `DogCoat`, the authored pack
  (`seed_pack`), and `step_dogs`, called per movement slice from
  `Engine::tick_movement` beside `step_movement`. Wander decisions are pure
  `hash01`-style rolls (no RNG): drift to a hashed walkable point within the
  leash, routed over the street graph with the exact target appended
  (`route_path_to_point`'s idea, no lane offset), then rest a hashed 1.5–10 s.
  Trot 1.7 m/s; no Needle claim, no separation steering.
- **Never a character.** No sheet, no inbox, no `knows`, no scheduler lane,
  not in `characters_within`, not in `attention.rs::context_hash` — a dog
  wandering past cannot re-arm the novelty gate, so the pack costs **zero
  tokens**. It also never rides the cold snapshot (`world.dogs` is skipped by
  `public_snapshot`); poses go out on their own hot channel,
  `EngineMessage::Dogs` — the `Lamps` shape, republished whole when any dog
  moves, once at startup so the host can spawn resting bodies, never bumping
  `world_revision`.
- **The prompt**: `build_sheet` lists dogs within the same 20 m
  `HEARING_RADIUS_M` as `you_see`, nearest first, as a `**dogs_nearby**`
  bullet section directly under `you_see` — description prose only
  (`a rangy brindle dog, 6.3 m, moving`), no id and no name, because no verb
  takes a dog. No `knows` gating: nobody needs an introduction to see a dog.
  The section and its `turn.j2` explainer paragraph render **only when a dog
  is within radius**, so every dog-less sheet — including all golden
  fixtures — is byte-identical.
- **The pack is authored, never spread** (the no-procedural-characters rule):
  each dog has a name (logs only), a coat, a build and a home leash, anchored
  where the city already asserts dogs — the six `MARKET_DOG_ANCHORS` the
  soundscape barks from, Jonet Sparr's furnace yard
  (`lore/families/family_sparr.md`), the Shambles, Eelback, the forecourt.
  Bracken, Marrow, Sedge, Pip, Warden, Eel, Cinder, Gnaw, Smoke, Alms.

**Render** (`src/smart_actors/dogs.rs`): a lofted-primitive quadruped in the
puppet style — barrel and muzzle via the turnshoe lay-down trick, joint-origin
pivots, one flat coat material per `DogCoat`, `build` scaling a ground frame
hung at −0.91 under the sim root. Root interpolation clones
`drive_npc_bodies` (20 Hz sweep + Cut margin lift); `animate_dog_gait` swings
diagonal leg pairs off the sim's own `gait_phase`, folds the lower leg through
the back of its arc, bobs the barrel twice a stride, wags the tail faster with
speed, and sways the head while resting. No `ActorView`, no `ActorTarget`, no
name label, no collider (`features/rats.md` §2.1 —
`collision_footprints.json` is byte-identical), visibility fade 90–110 m.

## Levers

None. The pack is always on (2026-08-15: `config.ron:
smart_actors.dogs_enabled`, `EngineConfig::dogs_enabled` and
`CATHEDRAL_NO_DOGS=1` were all removed — dogs cost no tokens and no snapshot
traffic, so there was nothing for a switch to protect). Seeding still requires
a nav graph, which is why every nav-less test and fixture is untouched by the
feature.

## Verifying

`cargo test -p cathedral-sim --test dogs_tests` pins the wander (walkable,
deterministic, revision-free), the sheet (nearest first, moving flag, absent
past 20 m), the markdown + explainer gating, the hot channel and the nav-less
empty kennel. Live:

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 120 -v 2>&1 | grep -A 2 dogs_nearby
CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE='wait-online; sleep 3; \
  tp 0 3 90 180 -10; sleep 1; shot dogs_forecourt; quit' cargo run
```

## Not done (deliberately)

No interaction of any kind: no verb, no percept when one barks (the
soundscape's render-side barks are unchanged and unconnected), no reaction to
the player, no hunger, no cats-with-quarry. The sim halves of `features/rats.md`
and the stigmergy future (`features/adhd-new-cool-features/04_stigmergy_fields.md`)
stay open and are the places to grow this.
