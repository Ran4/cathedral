# ADHD brainstorm — new cool features (2026-07-30)

69 ideas from six isolated cognitive frames (game design, markets, ant colony,
inversion, 10-year-old, speedrunner), scored, clustered by underlying angle,
top three deepened against the actual codebase. Score chips are
`[N V F]` = novelty / viability / fit, each 0–10.

## Clusters

| File | Angle | Ideas | Standout |
|---|---|---|---|
| [01_bells_as_data_bus.md](01_bells_as_data_bus.md) | The bells are the city's only broadcast medium — let people (and the player) write to it | 6 | False Peals |
| [02_bedtime_is_the_save_state.md](02_bedtime_is_the_save_state.md) | The Night Office is a nightly commit; play happens against the commit window | 6 | The Last Word |
| [03_custody_as_infrastructure.md](03_custody_as_infrastructure.md) | The arrest/gaol pipeline reused as transit, lodging, credit and drama | 7 | The Stone House Inn |
| [04_stigmergy_fields.md](04_stigmergy_fields.md) | Zero-LLM emergent city life: fields, marks, gradients, decay | 12 | Rumor Pollen, Chalk and Tallow |
| [05_the_watched_stranger.md](05_the_watched_stranger.md) | Invert the lens: the player is the one observed, priced, named and misdescribed | 9 | Your Silence Is Heard ★ |
| [06_markets_in_scarce_things.md](06_markets_in_scarce_things.md) | Auctions, scrip, debt, insurance and information asymmetry over what's genuinely scarce | 12 | Curfew Scrip |
| [07_reading_bodies_and_windows.md](07_reading_bodies_and_windows.md) | Stealth and leverage built from body clocks, gaze, weather and patrol patterns | 7 | The Nineteen-Metre Haggle |
| [08_toy_verbs_and_small_play.md](08_toy_verbs_and_small_play.md) | Naive, self-invented play that needs no quest to exist | 10 | Spoken Treasure Maps |

## Converge — the shortlist

1. **The Last Word** (02) — the sim *already* commits memories at per-Major
   bedtimes and the recency bias is already in the buffer; this is telegraphy
   plus one prompt paragraph, and it gives the game a daily deadline structure
   for free. Deepened in [90_focus_the_last_word.md](90_focus_the_last_word.md).
2. **Rumor Pollen** (04) — news that travels at walking speed for zero
   marginal LLM calls, garbles deterministically, and the player can outrun.
   Solves "the off-stage city is frozen" with pure Rust. Deepened in
   [91_focus_rumor_pollen.md](91_focus_rumor_pollen.md).
3. **Chalk and Tallow** (04) — the environment becomes the database: physical
   marks that cheap rules write and read, and the player can scrub or forge.
   A whole mischief verb set with no LLM cost until witnessed. Deepened in
   [92_focus_chalk_and_tallow.md](92_focus_chalk_and_tallow.md).
4. ★ **Your Silence Is Heard** (05) — the non-obvious-but-viable pick. The
   engine already knows when the mic opens and when a reply is owed; letting
   each ward settle its own theory about a mute stranger (holy fool, spy,
   penitent) makes *absence of input* an expressive channel. Almost no other
   voice game has dared this, and here it is nearly free.

## Traps

Attractive, but flagged — each for one load-bearing reason:

- **The Bucket Chain** (04): emergent firefighting is gorgeous, but fire that
  can consume hand-authored geometry is a world-persistence nightmare.
- **The Audience Broker** (06): welds the fiction to an engineering constraint
  (the LLM turn budget) that will change out from under the lore.
- **Carrier Futures** (06): priced off the M5 supply chain, which isn't built.
- **The Map Is Your Memory** (05): fights the already-shipped minimap +
  click-teleport investment head-on, and distorted-map art is expensive.
- **Rumors Are Merchandise** (06): reifying free-text gossip into catalog
  items explodes the item vocabulary and invites nonsense payloads — Rumor
  Pollen gets the same fantasy with a fixed vocabulary.
- **Trade a Pebble Up** (08): an economy balanced on LLM merchants being
  sweet-talked is an economy speedrun to zero. Fun once, structural never.

## Provocation

What if the cathedral itself is the last smart actor — a bodiless Major whose
round is the light through the rose window, whose only voice is the bells,
and whose Night Office runs once a night over everything every ward saw?
The city would literally dream about you, and the knell you hear at dusk
might be its opinion.
