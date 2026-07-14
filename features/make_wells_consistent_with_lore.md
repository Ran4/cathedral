Implement the wells and water described in lore/wells_and_water.md

For now just implement these 5:

1. Add visible fixtures for Slate Cistern, Tenter Cistern, Lodge Well, Three-Curb, Chain Well, Reed Cistern, Step Cistern, and Bitter Well, plus the Shambles well and Seven Lofts fire tanks.
2. Replace the single generic well renderer with distinct well/cistern variants, and improve Ford Well with a hollow opening, visible water, bucket mechanism, trough, drainage, and accurate collision.
3. Add named area records for every source so NPC prompts can identify their locations.
4. Add appropriate water, bucket, chain, windlass, trough, gutter, and cistern audio.
5. Update the hardcoded fixture/place/area tests in src/city/plan.rs:220.

Note: the authoritative runtime city plan is lore/places/ombreval_buildings.json, despite living under lore/.

These things can be added at a LATER point, when the developer explicitly tells you to:

6. Give keepers and water carriers explicit source assignments, source-specific knowledge, schedules, queues, and delivery routes.
7. Add water items/actions and authoritative source state to cathedral-sim.
8. Add level, quality, contamination, closure, repair, drought, fire-reserve, and seasonal-refill mechanics.
