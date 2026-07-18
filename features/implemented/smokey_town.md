Add this to the city:

Chimney smoke — the skyline has chimneys but zero smoke. A batched/particle job with no nav
implications; needs a small animated system (drifting, fading billboards on a hash-picked subset
of chimneys), unlike the static overhead-life kit.

(Originally this note also asked for awnings/laundry — both landed 2026-07-18 with the
overhead-life kit, see features/implemented/verticality.md.)

---

Implemented 2026-07-18 (`src/city/smoke.rs`):

- `add_chimneys` now reports every flue top; `stable_hash("smoke-<building>-<stack>") % 4`
  lights ~a quarter of them (709 of 2,858 stacks).
- Every puff in the city lives in **one** mesh entity ("Chimney smoke") rewritten per frame by
  `animate_chimney_smoke`: camera-facing quads that rise from the flue, bend into a prevailing
  wind (with per-plume heading/speed jitter and sway), swell from 0.7 m to 3.4 m and fade over a
  9 s loop, sorted back-to-front for the alpha blend. One draw call, no nav impact, entity-count
  contract untouched.
- Artwork: `assets/textures/ombreval_smoke.png`, a procedural 2x2 puff atlas from
  `scripts/generate_smoke_texture.py` (numpy fbm, no API needed); plumes pick cells and a
  warm-grey..blue-grey tint from their hash via vertex colors.
- Covered by two tests in `smoke.rs` (subset size; the animator fills the batched mesh with one
  quad per live puff).
