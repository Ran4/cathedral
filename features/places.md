CURRENT STATUS: Steps 1 and 2 are implemented; awaiting user review before Step 3.

# Make the lore's places real

The core lore describes squares, streets, passages, courts, churches, and
landmarks, but most of those names do not currently map to the actual game
world. Character positions therefore do not tell the smart actors where they
are in the shared geography of Ombreval.

During step 1 and 2, only use `lore/core_lore/`, especially `lore/core_lore/places.md`, for this
Do not use `lore/second_sun/`  but do use it for step 3 onwards.
But NEVER use `lore/wip_lore_please_ignore_this_is_NOT_canon/` at any point ever!

## Spatial model and ownership

The area map is authoritative simulation data, not prompt-only text and not a
Bevy-only debug overlay. `cathedral-sim` owns the area types, validation,
containment lookup, nearest-area lookup, compass directions, and the location
description injected into actor prompts. It must remain pure and IO-free: the
host reads the JSON as a string and passes it into the sim, as it does for other
world data.

Store the source of truth at `assets/world/areas.json`. The Bevy debug layer in
Step 2 must visualize this same parsed area map; it must not maintain a second
copy of the coordinates.

An **area** is a logical named place with:

* a stable machine-readable ID,
* the exact display label used in prompts, and
* one or more axis-aligned 3D bounding boxes.

One area may need several boxes so irregular places such as the cruciform
Lanthorn, a dogleg street, or the ground surrounding a building can be
represented without one overly broad box. The boxes belonging to one area form
one logical union for containment and nearest-distance calculations.

The JSON should be straightforward to inspect and modify programmatically. Use
a schema along these lines (with real coordinates in the actual file):

```json
{
  "schema_version": 1,
  "coordinate_system": {
    "units": "meters",
    "north": "+x",
    "east": "-z",
    "up": "+y"
  },
  "areas": [
    {
      "id": "lanthorn_interior",
      "label": "Inside the Lanthorn (Great Church of Saint Ambrelle)",
      "boxes": [
        {
          "min_m": { "x": 0.0, "y": 0.0, "z": 0.0 },
          "max_m": { "x": 1.0, "y": 1.0, "z": 1.0 }
        }
      ]
    }
  ]
}
```

The coordinate-system metadata is authoritative. It follows the existing
world: the Lanthorn's west entrance is at positive Z and its east end is at
negative Z; north is positive X.

### Boxes must not overlap

Overlapping area boxes are not supported. Any overlap is a data bug which must
be fixed in `areas.json`, not resolved through priority rules.

This applies both to boxes belonging to different areas and to boxes belonging
to the same logical area. Boxes may share faces or edges, but their interiors
must not intersect. Containment uses an inclusive minimum and exclusive maximum
on every axis (`min <= coordinate < max`), so adjacent boxes have deterministic
ownership at a shared boundary.

Reject invalid area data during loading with a useful error identifying both
boxes. Also reject duplicate area IDs, empty labels or box lists, non-finite
coordinates, and any box where `min >= max` on an axis. Do not silently choose
one of two overlapping boxes.

## Location descriptions

Compute a character's location from their current XYZ position whenever their
prompt is rendered. This replaces the character seed's static
`location_description` as the prompt's authoritative location description. Keep
the raw XYZ `position_m` in the prompt as well.

If the position is inside an area box, use that area's exact label, for example:

```text
Inside the Lanthorn (Great Church of Saint Ambrelle)
```

If the position is outside every box, find the closest logical area and render:

```text
30 meters north-northwest of The Draper's Reach
```

Use these exact rules:

* Measure horizontal Euclidean distance in the XZ plane from the character to
  the nearest point on a box, not to the box's centre. An area's distance is the
  minimum distance across all its boxes.
* Round the distance to the nearest integer meter, with an exact half meter
  rounding upward. Use `meter` only for exactly 1 and `meters` otherwise.
* Calculate the bearing from the nearest point on the chosen box towards the
  character, using the coordinate system declared in the JSON.
* Quantize to the closest of 16 equal compass sectors: north,
  north-northeast, northeast, east-northeast, east, east-southeast, southeast,
  south-southeast, south, south-southwest, southwest, west-southwest, west,
  west-northwest, northwest, and north-northwest.
* If two logical areas are exactly equally near, choose the lexicographically
  smaller stable area ID so results remain deterministic.
* A position can be outside a 3D box while still lying directly above or below
  its XZ footprint. In that case the horizontal distance and direction vector
  are both zero; render `0 meters from <label>` without a compass direction.

Add deterministic tests for JSON validation, shared box boundaries, rejection
of overlaps, multi-box logical areas, containment, nearest-point distance,
rounded integer distances, all 16 compass sectors, equal-distance tie-breaking,
and a moved character receiving a newly computed prompt location.

## Step 1: establish the first areas

Implement the complete spatial system above, but initially add only:

* **The Lanthorn grounds** — the exterior grounds around the building, labelled
  `The grounds of the Lanthorn (Great Church of Saint Ambrelle)`. Represent the
  ground around the interior with several non-overlapping boxes rather than one
  large box which contains the building.
* **Inside the Lanthorn** — the accessible cathedral interior, labelled
  `Inside the Lanthorn (Great Church of Saint Ambrelle)`.
* **Next to the Dawn Bearer statue** — the vicinity of the existing Dawn Bearer
  approach monument, labelled `Next to the Dawn Bearer statue`.
* **Next to the Seraph statue** — the vicinity of the existing Seraph approach
  monument, labelled `Next to the Seraph statue`.

Use clear final display labels for the two monument areas rather than `"..."`.
Their boxes must also remain disjoint from the Lanthorn grounds and each other.

**IMPLEMENT ONLY STEP 1 FIRST. Stop for user review and input before starting
Step 2.**

## Step 2: add an area debug layer

Implement an area debug mode toggled by `B`.

When enabled:

* select at most the eight closest logical areas whose nearest horizontal point
  is no more than 350 meters from the player,
* draw every box belonging to those selected areas as a skeleton/wireframe box,
* show its area label and stable ID in the game world,
* distinguish multiple boxes belonging to the same logical area by index, and
* show the player's currently resolved location description so containment and
  nearest-area behavior can be checked while moving.

Use the same parsed `assets/world/areas.json` data that the sim uses. Debug
rendering must not affect simulation results.

Stop for user review after Steps 1 and 2 work before modifying the game world.

## Step 3: build Coswald's Yard

From now on: do read lore/second_sun

Find the established descriptions of Coswald's Yard in the core lore and modify
the 3D game world so that an actual place matches them. Then add the area's
non-overlapping boxes to `areas.json`.

This is intentionally limited to one place because authored world changes will
be time-consuming. Inspect both the image and its generation prompt under
`lore/inspiration_images/places/` for visual inspiration.

If an inspiration image conflicts with written core lore, the written lore
wins.

Stop for user review after Coswald's Yard.

## Step 4: Figure out positions for the rest of the places

So, right now the game world is generic (other than the church + coswald's yard that we just implemented).

But now we want to add the rest of the established places.

But first, we need to figure out where these are positioned, so it all makes sense.

So, act like a "(fictional) medieval city planner" and plan the rough positions of all the named places.
Also feel free to add more places.

The result should be an authoritative guide. Later on, we'll fill it in with more lore.

## Step 5: the rest of the owl

Implement the remaining established places one at a time, treating each place
as its own reviewable feature: modify the world, add and validate its area
boxes, test it in debug mode, and then continue to the next place.

Follow the guide created in step 4.
