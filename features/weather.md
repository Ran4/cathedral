# Weather: rain, changing skies, and a city that reacts

> A specification for making weather one coherent world state rather than a
> collection of unrelated effects. The first release is a temperate-summer
> system: cloud, fog, wind, drizzle, rain, downpour, and thunderstorms.

## Outcome

Weather should change how Ombreval looks, sounds, moves, and speaks.

A rainstorm is successful when the player can watch the sky close over, hear
the first drops on slate, see exposed cobbles darken, step beneath a passage
and become dry while rain remains visible beyond the opening, watch nearby
people make for cover, and later emerge into a wet city after the rain has
stopped. Fog and overcast must feel like different weather, not rain with its
particles turned off.

The system must also remain cheap enough for the kilometre-wide procedural
city, deterministic in the headless simulation, and incapable of waking 500
LLM actors merely because a scalar changed.

---

## What exists now

The game already has useful pieces, but no weather authority:

- `crates/cathedral-sim` owns a deterministic world clock and publishes it on
  the hot engine channel.
- `src/smart_actors/clock.rs` rotates the sun and writes ambient brightness.
- `src/scene.rs` creates a physical Earth atmosphere and fixed 300 m distance
  fog.
- `src/city/smoke.rs` has a hard-coded prevailing wind.
- `src/soundscape.rs` contains a deterministic late-day “summer storm” window,
  a lightning flash, distance-delayed thunder, and storm-sensitive animals.
  It has no cloud, rain, wetness, or shared storm state.
- City geometry already includes roofs, bridges, passages, awnings, arcades,
  stalls, and well shelters, but there is no semantic query for “is this point
  open to the sky?”

The existing lightning/thunder work is not thrown away. It moves under the new
weather authority so there is exactly one answer to whether a storm is active.

## Design principles

1. **One cause, many consequences.** The same authoritative sample drives the
   prompt, deterministic NPC behavior, sun, sky, fog, rain, wet surfaces,
   smoke, animals, footsteps, roof patter, and thunder.
2. **The sim owns weather.** Weather affects world behavior and actor context,
   so its authority belongs in pure, IO-free `cathedral-sim`. Bevy only projects
   it. There must not be a second random weather clock in the renderer.
3. **Weather is more than precipitation.** Cloud cover, wind, visibility,
   surface wetness, and standing water are independent values. “Overcast” is
   useful even when no drop falls.
4. **Shelter must be convincing.** Rain may be visible outside a doorway, but
   drops and splashes must not appear below the cathedral roof, an overhead
   bridge, a market awning, or a covered passage.
5. **Macro time and micro time are different.** Fronts, wetting, and drying
   advance in game time and follow the debug time scale. Falling drops, audio
   fades, cloud texture motion, and lightning flashes animate in real time so
   `T` at 60× does not turn rain into white noise.
6. **Routine reactions are not LLM decisions.** Seeking cover and closing an
   exposed stall belong in the deterministic behavior layer. The LLM may
   comment, remember, or change its mind, but it is not asked to simulate being
   rained on.
7. **Stable, not repetitive.** A configured seed makes the same day reproduce
   the same fronts, including headlessly. Within that constraint, onset times,
   wind, duration, and intensity vary.
8. **Respect the setting.** The current clock, wildlife, and lighting describe
   a warm temperate summer. Snow is not smuggled into this feature without a
   season and temperature model. The Great Rains and the Hammering remain
   historical catastrophes, not ordinary random rolls.

---

## Release scope

### Weather the first release supports

| State | What distinguishes it | Precipitation | Typical visibility |
|---|---|---:|---:|
| **Clear** | Open physical sky, hard sun, light breeze | none | 300–360 m |
| **Broken cloud** | Moving cloud fields and intermittent softening of the key light | none | 260–330 m |
| **Overcast** | Continuous grey sky, weak shadows, cool flat fill | none | 220–320 m |
| **Mist / dawn fog** | Low, quiet layer pooled in streets and the old river ground | none | 45–120 m at street level |
| **Drizzle** | Fine short streaks, little splash, slow wetting | 0.10–0.30 | 180–280 m |
| **Steady rain** | Legible drops, roof patter, runoff, wet footsteps | 0.30–0.70 | 130–220 m |
| **Downpour** | Dense wind-driven rain, strong splash and runoff | 0.70–1.00 | 70–150 m |
| **Thunderstorm** | Dark storm cloud, gusts, downpour, lightning then thunder | 0.55–1.00 | 70–160 m |

Wind is a vector and a gust value, not a mutually exclusive weather state. A
clear day can be windy and a steady rain can be nearly vertical.

### Explicit non-goals for the first release

- no snow, frost, ice, snow depth, footprints, or roof loading;
- no random re-enactment of the Great Rains or the Hammering;
- no structural damage, fire ignition, lightning damage, or hail injuries;
- no flooding, changing canal level, contaminated wells, harvest simulation,
  or long-term economy effects;
- no slippery player controls, stamina penalty, health damage, or wetness bar;
- no per-building indoor climate simulation;
- no real-world weather API and no dependence on the machine's local weather;
- no save-game format solely for weather. The configured clock day and weather
  seed are sufficient to reproduce the schedule.

An ordinary hail VFX/audio preset may be added later for an authored story
event. It must not enter the procedural table under the name “the Hammering.”

---

## 1. Authoritative weather model

Add `crates/cathedral-sim/src/weather.rs`. It reads no clock, random device,
file, or environment variable. Like `WorldClock`, it is handed time and a
seed.

The semantic kind is for prose and broad behavior. Continuous fields are what
presentation consumes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherKind {
    Clear,
    BrokenCloud,
    Overcast,
    Fog,
    Drizzle,
    Rain,
    Downpour,
    Thunderstorm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecipitationKind {
    None,
    Rain,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherSample {
    pub kind: WeatherKind,
    pub cloud_cover: f64,          // 0..=1
    pub precipitation_kind: PrecipitationKind,
    pub precipitation: f64,       // 0..=1
    pub wind_xz_mps: [f64; 2],    // world X/Z, finite
    pub gust: f64,                 // 0..=1
    pub fog: f64,                  // 0..=1
    pub visibility_m: f64,
    pub surface_wetness: f64,      // 0..=1, persists after rain
    pub standing_water: f64,       // 0..=1, rises and drains more slowly
    pub thunder: f64,              // 0..=1 storm-cell activity
    pub semantic_revision: u64,
}
```

The wind pair uses the same world X/Z convention the rest of the sim exports;
Bevy converts it to its own `Vec2` only after the bridge. All construction
clamps and validates values so NaN cannot cross the bridge.

`semantic_revision` changes only when something an actor would name changes:
rain begins or ends, fog forms or clears, or a storm reaches the city. It does
not change for every interpolated cloud or wetness value. The actor-specific
shelter bit is combined with this revision later when stage novelty is tested.

### Episodes and transitions

The timeline is a sequence of weather episodes with smooth lead-in, body, and
tail phases. Generation uses stable hashes of `(weather_seed, absolute_day,
slot)` and can be queried in tests without running Bevy.

Default warm-season behavior:

- clear and broken-cloud conditions occupy roughly 55–65% of long-run time;
- precipitation occupies roughly 20–25%; thunderstorms remain below 5%;
- fog is primarily a Watch/Kindling/Dayspring event, requires low wind, and
  normally burns off before High Wick;
- thunder cells prefer the Waning and Lamplight, matching the existing summer
  storm and the city's late-summer storm memory;
- an ordinary wet episode lasts about 1–6 game hours and never more than 8;
- a rain front normally passes through broken cloud and overcast before rain,
  then returns through trailing overcast. Clear-to-downpour is reserved for a
  forced developer preset or an authored exceptional event;
- wind direction changes gradually across a front. Gusts may vary within it;
- fronts cross midnight without resetting or popping at the day boundary.

The exact weights live in a small `WeatherClimate` value with shipped defaults,
not scattered `if day % N` checks. A later season system can provide another
climate without changing consumers.

### Wetting and drying

Wetness is authoritative because later systems may care whether it rained, not
whether the player happened to render the rain.

- Drizzle wets slowly; rain wets quickly; a downpour reaches saturation in
  roughly 15–25 game minutes.
- Standing water begins only after a wetness threshold and drains over 1–3
  game hours after precipitation ends.
- Surface wetness dries faster in direct sun and wind, slower under overcast,
  and barely at night.
- Advancing across a long frame splits integration at episode boundaries. The
  result must be the same within tolerance whether a six-hour span is sampled
  once or in 21,600 one-second steps.
- Engine startup warm-starts the previous game day. Beginning a session in the
  tail of a rain front therefore produces already-wet streets rather than dry
  ground that suddenly remembers the rain.

### Lightning is a crossed event

Lightning is transient and must not be inferred from “storm is active” in a
Bevy timer. The timeline deterministically schedules strikes inside thunder
episodes. The engine asks which strikes were crossed in `(previous, now]`, as
the clock already does for office bells, so acceleration and frame hitches do
not lose them.

Each strike carries a stable ID, game instant, world-space cloud origin, and
strength. A large debug jump coalesces an old backlog to the newest useful
strike rather than flashing ten times in one frame.

---

## 2. Ownership and data flow

```text
WorldClock + weather seed
          │
          ▼
cathedral-sim WeatherTimeline
          ├── current WeatherSample ──► behavior ladder / stalls / prompt
          ├── crossed lightning ──────► hot EngineMessage::Lightning
          └── hot EngineMessage::Weather
                                      │
                                      ▼
                            Bevy WorldWeatherState
       ┌─────────────┬───────────────┬──────────────┬──────────────┐
       ▼             ▼               ▼              ▼              ▼
 sky/light/fog   rain & cover   wet materials   soundscape   smoke/nature
```

### Engine integration

- `EngineConfig` gains a `WeatherConfig`/`WeatherTimeline`, constructed from
  committed defaults by the host.
- On each poll, the engine samples weather immediately after advancing the
  clock and before deterministic behavior runs.
- `World.current_weather` is available to prompt rendering and the round. It
  is not part of the cold public-snapshot revision loop.
- `EngineMessage::Weather` is a small hot message, analogous to `Clock`. It may
  be published every poll so transitions remain smooth without republishing
  500 actors.
- `EngineMessage::Lightning` is emitted only for a crossed strike. It is a
  player presentation event, not a broadcast sound percept.
- If smart actors/the engine are disabled, presentation fails to a stable clear
  late-afternoon state, just as the clock currently does. Bevy must not start a
  duplicate random timeline as a fallback.

### Bevy projection

`src/weather/` owns a `WorldWeatherState` resource refreshed by the bridge. It
contains the newest authoritative sample, the previous sample needed for
render smoothing, and receipt time. Consumers read it; only the bridge writes
it.

Weather joins the existing hot/cold discipline:

- it does not bump `world_revision`;
- it does not rebuild `WorldMirror`;
- it does not enqueue a line in every actor inbox;
- scalar interpolation never counts as novel actor context.

---

## 3. Sky, sun, fog, and exposure

### One environment compositor

Weather cannot safely be added as another system multiplying whatever value
`drive_sun` happened to write last frame. Refactor so one environment system is
the sole writer of:

- `DirectionalLight` illuminance and weather tint;
- `GlobalAmbientLight` color and brightness;
- `AtmosphereEnvironmentMapLight.intensity`;
- camera `DistanceFog` and `VolumetricFog` settings;
- weather sky/cloud material parameters;
- the fallback `ClearColor`.

The clock still computes solar direction and night brightness. The compositor
combines that absolute solar sample with weather every frame. Exposure remains
fixed initially; auto-exposure would conceal the intended difference between a
clear noon and a black storm and can cause visible pumping during lightning.

Suggested response curves:

| Input | Rendering consequence |
|---|---|
| cloud cover 0 → 1 | direct sun multiplier 1.0 → 0.12, softer/cooler ambient, dimmer atmosphere environment map |
| precipitation 0 → 1 | visibility closes toward 100 m, distance fog cools and darkens |
| fog 0 → 1 | street visibility closes toward 50 m and low volumetric density rises |
| thunder 0 → 1 | direct sun can fall to 0.02–0.08, cloud base darkens, ambient remains readable |
| wetness 0 → 1 | exposed ground/roof materials darken and become smoother |

Night remains night. Overcast must not raise the configured night floor, and a
storm at noon must remain navigable without looking like clear noon through a
grey filter.

### Cloud layer

Keep the physical atmosphere as the clear-sky base. Add a camera-centred cloud
shell above it using a dedicated unlit weather-sky material:

- two slowly scrolling noise layers break tiling and move with authoritative
  wind;
- cloud coverage, edge softness, base darkness, and sun-facing silvering are
  uniforms driven by the weather sample and solar direction;
- the shell follows XZ camera movement but not camera rotation;
- it never casts thousands of real cloud shadows. Direct-light modulation
  provides the large-scale shadow response;
- at broken cloud, light changes are slow and low-amplitude. Do not strobe the
  city every time a noise lobe crosses the sun;
- the cloud layer blends out cleanly at zero cover, leaving the existing
  atmosphere unchanged.

### Fog sea

Continue using `DistanceFog` for far visibility in every state. For actual fog,
also enable Bevy's `VolumetricFog` and one or more broad `FogVolume`s:

- strongest density follows the old Cut, canal/Serle edge, and low Reed Ward;
- a weaker city-wide layer occupies roughly the lowest 15–25 m;
- density falls rapidly above the layer so flying above it reveals towers and
  the Lanthorn over a fog sea;
- a small repeating 3D density texture scrolls slowly with the wind to avoid a
  perfectly level opaque slab;
- the sun is a `VolumetricLight` only while useful. Low sun through broken fog
  may make shafts; an overcast fog should remain diffuse;
- when volumetrics are disabled by quality settings, the same state still
  works through distance fog alone.

Near-camera fog is attenuated while the player is under solid cover so the
nave does not fill with the same ground mist as the street. Exterior fog
remains visible through doors and arches.

---

## 4. Rain rendering and shelter

### Precipitation mesh

Rain is one bounded, camera-local effect, not a city-wide entity field.

- Use a fixed-capacity pool in one shared dynamic mesh (or an equivalent single
  instanced draw): no entity per drop and no allocation proportional to city
  size.
- A high-quality target is roughly 1,500–2,500 live streaks within 28–35 m of
  the camera; lower qualities reduce capacity and splash count, not world
  behavior.
- Streak length, opacity, and population scale continuously from drizzle to
  downpour. Drizzle uses shorter/finer drops, not merely fewer downpour lines.
- Drop velocity is downward plus the authoritative wind vector. Gusts bend it
  smoothly.
- Positions are stable-hash seeded and world-anchored enough that turning the
  camera does not make rain rotate with the view.
- The depth buffer handles ordinary wall/roof occlusion. A weather cover query
  handles the harder case of particles being generated below a roof.
- Particle falling and recycling use real seconds. Intensity follows the
  authoritative game-time transition with a short real-time smoothing floor
  so debug acceleration does not pop the whole pool in one rendered frame.

### `PrecipitationOcclusionMap`

Do not cast a ray through every collider for every drop every frame. Build a
small top-down cover map once during procedural scene construction.

At approximately 2 m resolution over the 1.2 × 1.0 km city it is about 300,000
cells. Each cell stores:

- the highest precipitation impact height;
- ground impact height where distinct;
- a compact `CoverMaterial` (`Open`, `Slate`, `Tile`, `Thatch`, `Stone`,
  `Timber`, `Canvas`, `Glass`);
- whether the lower space should use a sheltered listener/particle mix.

Scene builders rasterize the same footprints they use to create:

- ordinary building roofs;
- the Lanthorn roof, dome, towers, and open west doorway boundary;
- overhead bridges, galleries, gate vaults, and Bellfoot Passage;
- market stalls and shopfront awnings;
- well/cistern roofs and other deliberate shelters.

The rain pool samples impact height for each XZ column. A streak ceases before
crossing that surface; a sampled fraction produces a roof or ground splash at
the impact point. This gives the right result both below and above a roof:

- standing under an awning: rain lands above the camera and none appears below;
- looking out from the nave: exterior cells continue raining;
- flying over a roof: drops remain visible above it and terminate on the roof;
- standing beneath a bridge: the bridge receives splashes while the passage
  remains dry.

The map is a presentation occlusion structure, not a new movement collider.
Its coarse roof heights may approximate slopes; normal depth testing hides the
approximation. Missing cover registrations are bugs and get regression tests
at named locations.

### Splashes, drips, and runoff

Use separate bounded batches:

- sparse crown/ripple splashes on exposed cobbles and roof surfaces;
- fine mist close to the ground only in a downpour;
- deterministic puddle ripples while standing water is nonzero;
- a small set of generated eave/gutter runoff anchors, activated by sustained
  rain rather than immediately on the first drop.

Runoff is deliberately sparse. A convincing stream from twenty nearby eaves is
better than a particle emitter on every one of 2,500 buildings.

---

## 5. Wet surfaces and the rain's aftermath

Rain stopping must not restore the dry noon materials in one frame.

Create a `WeatherReactiveMaterials` registry containing only exposed semantic
materials and their dry baselines. Update shared handles at a bounded rate
(for example 10 Hz or when wetness changes by at least 1/255); never clone a
material per building.

First-release response:

- cobbles, paving, yards, and the dry Cut darken modestly and reduce roughness;
- slate and terracotta darken and gain a restrained sheen;
- timber and canvas darken slightly but never become mirrors;
- cathedral interior floors and other explicitly indoor handles do not react;
- walls remain mostly unchanged in the first pass, avoiding the need for a
  spatial rain mask inside the existing batched façade meshes.

Generate one static batched puddle mesh from stable road/court positions. It
must reject covered cells, steep surfaces, door thresholds, navigation choke
points, and the dry Cut's non-road voids. Puddle alpha/roughness follows
`standing_water`; puddles do not collide and do not alter navigation.

Environment-map reflection is sufficient for the first pass. Screen-space
reflections are a later quality option, not a dependency for wet streets.

---

## 6. Soundscape

Weather audio remains presentation texture in `src/soundscape.rs`; it does not
become an NPC `make_sound` event or fill actor inboxes.

Add loop stems for:

- light exterior rain;
- heavy exterior rain;
- rain on hard roof (slate/tile/stone);
- rain on soft roof (thatch/canvas/timber);
- storm wind;
- sustained gutter/eave runoff;
- a sheltered/muffled exterior-rain version;
- wet cobble footsteps and a small splash variant.

The listener's cover-map sample crossfades the mix:

- exposed: exterior bed + nearby ground impacts;
- beneath hard cover: reduced/muffled exterior bed + hard roof patter;
- beneath canvas/thatch: reduced exterior bed + softer close patter;
- deep in the Lanthorn: distant exterior wash + broad roof resonance, no close
  cobble impacts.

Use recordings with baked muffling rather than requiring a runtime low-pass
effect. Weather gets a small reserved loop budget so a downpour cannot evict
all authored city loops, and all stems still obey `AudioActivity` ducking
during player speech, STT, and NPC voice.

Existing thunder audio and its speed-of-sound delay are preserved. On a
`Lightning` message Bevy flashes immediately, then schedules thunder at
`listener_distance / 343 m/s`. The delay uses real seconds and is not shortened
by the debug world-clock scale.

Weather also modifies existing soundscape policy:

- sparrows, swallows, flies, geese, and most gull/raven beds fall silent in
  sustained heavy rain or storm, replacing the private `SummerStormState`
  checks;
- exposed market ambience fades or closes with the actual stall/crowd state;
- dry cobble footsteps select wet clips above a wetness threshold;
- cart-rut and sack/crate sounds remain event-driven, with optional wet variants
  selected by surface state.

---

## 7. Lightning and the “Storm of White Glass”

Thunderstorms retain the existing sparse cadence and make the visual event
stronger:

1. A cold flash lights the skyline from a deterministic cloud origin.
2. For a fraction of a second, exterior direct/ambient light spikes without
   changing exposure.
3. The rose-window emissive and its interior colored-light contribution flare,
   briefly stamping the tracery into the dark nave.
4. The flash decays with a possible weaker second pulse.
5. Thunder arrives after the physical propagation delay.

Transient flash lights do not cast shadow maps; the sun's existing shadow map
is not rebuilt for a quarter-second event. The effect must be visible from both
street and nave without turning the whole scene flat white.

Lightning never selects an NPC, player, roof, or flammable prop as a gameplay
target in this release.

---

## 8. Wind as shared world state

Replace `src/city/smoke.rs`'s fixed heading/speed with projected weather wind.
Wind drives:

- rain slant and gust variation;
- cloud texture drift;
- chimney-smoke drift, bend, and faster dispersal;
- optional lightweight sway parameters for laundry, awnings, signs, and
  banners;
- storm-wind audio.

Hearth schedules remain clock-driven. Heavy rain may lower plume opacity and
make smoke ragged, but it does not extinguish every chimney. All consumers use
the same XZ convention so clouds, rain, and smoke cannot visibly travel in
three directions at once.

---

## 9. Smart actors and deterministic city behavior

### Prompt seam

Weather appears as one short perspective-aware line in `you_are`, beside the
clock, for example:

```text
weather: steady rain from the west; the streets are soaked; you are under the Bellfoot Passage roof
```

Do not expose numeric intensity, visibility, a forecast, or internal enum
names. An exposed actor and a sheltered actor may receive different final
clauses. Weather is current state, not `recent_history`, so it cannot crowd
dialogue out of the bounded history.

The on-stage novelty signature includes only `(semantic_revision,
sheltered_or_exposed)`. A meaningful change can therefore give nearby actors
one opportunity to comment, still subject to the existing curiosity and stage
caps. Interpolated wind and cloud values do not buy LLM calls.

### Shelter data

Presentation cover and navigable social shelter are related but not identical.
Add data-owned shelter destinations in `assets/world/shelters.json` with:

- stable ID and prompt label;
- position/covered polygon and route node;
- public/private access;
- capacity or stable spread radius;
- cover material/type;
- optional office/opening constraints.

Seed them from covered passages, arcades, gates, market roofs, church porches,
well roofs, and other places where a body can actually stand. Homes can count
as private shelter only if the behavior/projection has a credible way to place
or hide the resident; do not stack a household visibly on an unsheltered
doorstep and call it indoors.

### Behavior ladder

Weather reaction is deterministic and sits below active conversation and true
emergency behavior, but above an ordinary idle/routine leg when severity calls
for it.

| Weather | Default exposed-actor behavior |
|---|---|
| broken cloud / overcast / fog | no shelter diversion; fog may reduce long wandering later |
| drizzle | most continue; genteel/idle actors may choose nearby cover |
| steady rain | idle actors and exposed sellers seek a known nearby shelter; carriers and essential workers continue |
| downpour | almost every nonessential exposed actor seeks cover |
| thunderstorm | downpour behavior plus a short reflex pause at nearby lightning; no LLM required |

Requirements:

- choose among reachable shelters, never straight-line teleport;
- use stable offsets/capacity so everyone does not occupy one coordinate;
- add hysteresis: an actor does not leave shelter until rain has remained below
  its threshold for 10–20 game minutes;
- never abandon an active player conversation mid-sentence. After the exchange,
  shelter becomes the next deterministic intent;
- do not cancel a gate operation, well draw, cargo handoff, or other atomic
  action halfway through;
- actors already home/off-stage need no cognition to remain sheltered;
- behavior must work with fake cognition and with cognition unavailable.

### Markets and routes

In the first release:

- covered stalls stay open in ordinary rain;
- exposed stalls pause sales in downpour/storm and retain their stock;
- sellers use nearby shelter and resume if weather clears while their ordinary
  office remains open;
- road parties continue through drizzle/rain. Holding or cancelling trips in a
  thunderstorm is a later supply-chain extension;
- water levels, food stocks, prices, hunger, thirst, and crop yields do not
  change from one shower.

This is enough for weather to visibly change the city without quietly turning
the feature into an economy rewrite.

---

## 10. Configuration, forcing, and diagnostics

Add a documented top-level section to `default_config.ron`; the local
`config.ron` continues to inherit missing fields through serde defaults.

```ron
weather: (
    enabled: true,
    seed: 437,
    mode: "timeline",       // timeline, clear, overcast, fog, drizzle, rain, downpour, storm
    frequency: 1.0,         // scales episode frequency, not intensity
    quality: "high",        // low, medium, high
    volumetric_fog: true,
)
```

Invalid strings log once and fall back to `timeline`. `enabled: false` means a
stable clear sample, not “no weather resource.” `frequency: 0` is also a valid
always-clear timeline for comparisons.

Developer control must not consume another crowded gameplay key:

- add `SetWeatherOverride` / `ClearWeatherOverride` engine commands;
- add `--weather KIND` to `cathedral-headless`;
- add a drive action such as `weather rain`, `weather storm`, and `weather
  timeline`;
- forcing a kind uses a representative continuous sample and a short visual
  transition. Optional intensity may be accepted as `weather rain 0.5`;
- a forced state affects sim and Bevy through the normal bridge. It must never
  mutate only the renderer.

Log semantic transitions once:

```text
[weather] day 12 14:37 overcast -> rain, wind 4.2 m/s SW, visibility 170 m
```

Do not log per-frame samples. The debug actor sheet/headless final report shows
the current prose weather and wetness band so behavior can be diagnosed without
a GPU.

---

## 11. Performance budgets

At 1920×1080 on the existing high-quality path, weather should target:

- no more than 2 ms additional GPU time in steady rain and 3 ms with high
  volumetric fog;
- less than 1 ms steady-state CPU time;
- at most three precipitation-related draw calls (streaks, impacts/runoff,
  puddles), excluding the cloud/fog passes;
- zero entity per raindrop, puddle, or roof impact;
- a bounded particle pool independent of city size;
- no per-building material clones and no per-frame scan of all buildings;
- a cover map of only a few megabytes;
- no weather-driven cold snapshots, actor-wide inbox writes, or provider calls.

Quality scaling changes only presentation:

| Quality | Rain | Fog | Wet aftermath |
|---|---|---|---|
| low | ~600–900 streaks, few impacts | distance fog only | shared wet materials, no ripples |
| medium | ~1,200–1,600 streaks | 32-step volumetric fog | puddles + sparse impacts |
| high | ~1,500–2,500 streaks | 64-step volumetric fog | puddles, impacts, sparse runoff |

The authoritative schedule, NPC decisions, prompt, thunder timing, and wetness
are identical at every quality.

---

## 12. Test plan

### Pure sim tests

- same seed and game instant produce byte-for-byte equivalent semantic samples;
- different seeds vary schedules without producing invalid transitions;
- every scalar is finite and inside its documented range for a multi-year
  sweep;
- fronts remain continuous across midnight and thunder remains rare;
- fog respects its low-wind/morning constraints;
- wetness is invariant to polling step size and warm-starts correctly;
- crossed lightning is neither lost on a large step nor duplicated on a zero or
  rewound step;
- forcing and clearing each weather kind is deterministic;
- weather hot messages do not bump `world_revision`;
- weather changes do not append to all actor inboxes;
- prompt fixtures contain the short weather phrase only when a clock/weather
  context is present;
- stage novelty changes once at a semantic transition, not every poll;
- exposed actors choose reachable shelter with hysteresis while atomic actions
  and conversations retain precedence;
- exposed stalls pause/resume without losing or duplicating stock.

### Bevy/host tests

- bridge messages update `WorldWeatherState` without rebuilding `WorldMirror`;
- the environment compositor has one absolute writer and maps representative
  samples to expected sun, ambient, fog, and cloud values;
- rain mesh capacity is bounded and density is monotonic with intensity;
- named cover probes classify the Lanthorn nave, Bellfoot Passage, one bridge,
  one market awning, one well roof, and an open square correctly;
- no streak or ground splash descends below the cover-map impact height;
- a flying camera above that same roof receives rain above the impact surface;
- wet material values return exactly to their stored dry baselines;
- puddle candidates are deterministic and reject covered cells;
- shelter changes crossfade the intended audio stems;
- lightning flash is visible before its distance-delayed thunder and despawns;
- smoke, rain, and cloud drift agree on wind direction;
- no-audio and no-renderer test apps can still construct the plugin resources.

### Visual drive acceptance

Capture named screenshots in `logs/latest_session/screenshots/` for:

1. clear, broken cloud, overcast, fog, drizzle, rain, and storm from the same
   street position at High Wick;
2. steady rain in an open square;
3. the same rain viewed from beneath Bellfoot Passage and from inside the west
   doors, with a dry foreground and rainy exterior;
4. rain from a flying position above roofs;
5. wet streets immediately after rain and the same streets after drying;
6. street-level dawn fog and an above-fog skyline;
7. a lightning flash seen outside and its rose-window response in the nave.

During the rain drive, verify by ear that exposed rain, hard/soft roof patter,
deep-interior rain, wet footsteps, and delayed thunder are distinct and that
NPC voice remains intelligible.

---

## 13. Milestones

### W0 — Authority and debug seam

- pure weather timeline, wetness integration, overrides, tests;
- `EngineMessage::Weather` and `Lightning`;
- Bevy projection resource, headless flag/report, drive forcing;
- remove the independent `SummerStormState` authority while retaining its
  asset and flash/thunder behavior under the new messages.

### W1 — One complete rain slice

- overcast lighting/cloud response;
- forced steady rain with one bounded streak mesh;
- precipitation cover map for buildings, Lanthorn, passages, bridges, awnings,
  stalls, and well roofs;
- exposed/sheltered rain audio and wet footsteps;
- shared wet cobble/roof materials and drying.

This milestone is not complete if rain falls in the nave or beneath Bellfoot
Passage.

### W2 — Weather variety and aftermath

- clear/broken/overcast transitions;
- drizzle and downpour tuning;
- distance plus volumetric dawn fog;
- puddles, splashes, sparse runoff;
- wind shared by rain, cloud, and chimney smoke;
- soundscape/nature weather gating.

### W3 — Thunderstorm polish

- deterministic crossed lightning events;
- skyline and rose-window flash response;
- physical thunder delay and storm-wind mix;
- accelerated-time coalescing and visual tests.

### W4 — The city reacts

- prompt/context seam;
- data-owned shelter destinations;
- deterministic shelter rung and hysteresis;
- covered/exposed stall behavior;
- headless and fake-backend acceptance.

### W5 — Later extensions, each separately specified

- seasons, temperature, frost, snow accumulation, footprints, and thaw;
- authored ordinary hail and a distinct story treatment of Hammer-weather;
- well refill, drainage capacity, mud, flooding, crop/road consequences;
- clothing/hoods and individual tolerance;
- forecasts, sky-reading trades, and weather knowledge that can be wrong.

---

## Definition of done

The feature is done when weather is one deterministic state shared by the sim
and renderer; every first-release kind can occur naturally and be forced; rain
respects named cover; the city remains wet after rain; sky, light, fog, wind,
audio, smoke, nature, stalls, and actors agree about the current conditions;
lightning still flashes before thunder; headless tests prove the behavior
without Bevy; visual drive evidence covers outdoor, sheltered, interior,
flying, fog, and storm cases; and the measured performance stays within the
budgets above.
