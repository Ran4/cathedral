So, as per the lore (see lore/core_lore), there's various places.

Right now, they're imagined - they don't map to the actual game world.
The in-game characters's positions don't map to the names given in the core lore.

Note: second_sun lore is extensive; skip it, ONLY consider core_lore for now!

so, what we want to do is:

## Step 1: establish area bounding boxes

In the game, establish bounding boxes for areas - area boxes.

Each area is an invisible 3d box with a label.

So, for example,
* the grounds outside of the cathedral is the "The cathedral grounds",
* inside of the cathedral it's "Inside of the cathedral",
* a specific church is "inside of the church of X" and their courtyards are "The courtyard of church of X"
* near each of the two huge statues is "Near statue of X" or "Near statue of Y"
* Coswald's Yard -> "Coswald's Yard"

When we inject position data into the characters,
we take the character's XYZ coordinates and finds the corresponding area box, and inject it as e.g.
in the cathedral - something like "[position: Inside of the Lanthorn (Great Church of Saint Ambrelle.)]"
or "The Draper's Reach"

If we're not inside of an area, find the closest area and show it like

"30 meters north-northwest of The Draper's Reach" (distance (integer meters) + 16 directions (North, North-northeast, Northeast, East-northeast, East, East-southeast, Southeast, South-southeast, South, South-southwest, Southwest, West-southwest, West, West-northwest, Northwest, North-northwest))

At first, only create the system and add these boxes:
* The cathedral - "The Lanthorn (Great Church of Saint Ambrelle)"
* Inside of the cathedral - "Inside of The Lanthorn (Great Church of Saint Ambrelle)"
* Near the two status - "..."

IMPLEMENT THIS FIRST, stop for user input before going for step 2.

Note: the coordinates should be stored in a json file somewhere, so it's easy to programmatically/agentically
extract things.


## Step 2: add a debug layer that lets us test this

Implement a debug mode. Pressing B toggles debug mode.

If debug mode is on, areas should be shown as skeleton bounding boxes, and the area name should be shown in
a label in the game world.

## Step 3: modify game world

Note: implement step 1 and 2 first!

Find out places that are mentioned in the core lore,
and actually modify the 3d gameworld so it matches the description of the places.

Then add the areas boxes.
