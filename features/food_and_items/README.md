# Food & items: the city eats

Status: M0–M4 implemented (2026-07-18). M5 (the supply chain) is **scoped but not started** —
see [07_the_supply_chain.md](07_the_supply_chain.md), which splits it into M5a–M5d. The specs below
were updated where the implementation differed (rung-3 meal-office gate, tavern-node hearths, silent
auto-eat, template-derived `you_sell`).

| | |
|---|---|
| **The brief** | items with metadata (flour, quality), quantities ("transfer 3 coins"), copper-only money, hunger that eating cures, food vendors, and a magic morning restock that is *explicitly a placeholder* for a real supply chain |
| **The sibling** | `features/movement/` — the clock, the ladder, the round, and the water round this design copies shamelessly |
| **This folder** | the design the brief asked for |

---

## 1. The one-paragraph version

A pilgrim named Ilse stood in the Wickmarket yesterday with one copper coin, asking which stall sells
the cheapest food. Three NPCs confidently directed her to the bread stalls by the market edge. There
are no bread stalls. There is no bread. The entire world inventory is one fish and her one coin, and
"hungry" is a static string on her character sheet that no amount of eating will ever remove. Every
NPC in that scene was improvising a market economy that the sim cannot back. This plan makes the
market real: a small item catalog with kinds, metadata and quantities; one coin; a hunger gauge that
eating refills; vendors whose stalls hold real loaves; and a code-driven meal round that feeds the
city the way the water round waters it — visibly, cheaply, and without spending a token.

---

## 2. The thesis: the slots are pre-cut

This is not new architecture. The movement plan reserved space for food at every layer, and the code
shipped with the reservations intact:

- **The needs struct is built to grow.** `crates/cathedral-sim/src/character.rs:111-124`: *"M3
  ships thirst only, with hunger/fatigue/duty following."* `Needs` is a one-field struct waiting for
  its second field.
- **The ladder reserved the rungs.** `features/movement/03_the_ladder.md` §3 already specifies
  **rung 3 starving** (`hunger < 15` → food) and **rung 7 hungry** (`hunger < 70` → food), exactly
  mirroring parched (2) / thirsty (6), with destinations *"home hearth, a cookshop, a tavern, a food
  stall."*
- **An error variant sits reserved and unraised.** `crates/cathedral-sim/src/error.rs:39` has
  `TooFar`, with the milestone note *"`TooFar` is reserved, unraised, until hunger exists"*
  (`features/movement/07_milestones.md:373`).
- **The vendors are already routed.** `assets/world/rounds.json` sends `baker`,
  `food_provisioner` and `grocer_and_spicer` to the Wickmarket, `fish_trader` and `market_seller`
  to Maren's Green, `cook`/`tavern_worker`/`brewer` to the Hungry Ox and the Bellstand — via the
  `market_trader` archetypes whose `only_on` legs already move the crowd on market days. **~92 of
  the 500 authored characters work a food trade.**
- **The stalls are already standing.** Sixty stall fixtures (`wick_fixture_01`…`60`,
  `lore/places/ombreval_buildings.json`) are permanent street geometry with positions and angles.
  Nobody has ever sold anything from one.
- **The template is shipped and green.** The water round (`crates/cathedral-sim/src/round.rs`) is
  a complete worked example of the exact shape food needs: a source with a keeper, a queue, a timed
  service, a need snapped back to full, self-percepts so the actor can be *asked* about it, and
  player-only sounds so none of it costs a token.

What is genuinely new is smaller than it looks: **quantity and metadata on items, one hunger gauge,
a stall table, prices, and a two-line change to the prompt.** The deep-dives cover each.

---

## 3. The five decisions that shape everything

**(a) Items become kinds + stacks.** `Item {id, name, visual_key}` becomes
`Item {id, kind, quantity, metadata}` with the display name derived from a small embedded catalog
(`assets/world/items.json`). Two stacks merge iff same holder, same kind, same metadata. "Transfer
3 coins" is `offer_item {"item_id": "c0prs", "quantity": 3}`. Full design in
[01_items_and_stacks.md](01_items_and_stacks.md).

**(b) One coin.** The lore's spark/bell/lantern triad collapses to the **spark** (street names: a
penny, a copper). The silver bell dies happily — a city whose life runs on seven *bell* offices
never needed a coin called the bell too. Spark prices are already right; bell prices convert at 12:
the loaf keeps its canonical 2. [02_the_spark_standard.md](02_the_spark_standard.md) has the sweep
list.

**(c) Hunger is thirst with a slower clock.** A second `f64` on `Needs`, decaying over ~10 game
hours, refilled by `satiety` on whatever you eat, surfaced to the LLM as a **computed condition**
("hungry", "famished") on the sheet line that today only carries static lore text. This pays the
known "most expensive small change" — regenerating the 20 golden prompt fixtures — once, and
[01](01_items_and_stacks.md) needs the same regeneration anyway, so they land together.
[03_hunger.md](03_hunger.md).

**(d) Vendor stock is real items, conjured at the Kindling.** Each morning, every bound food vendor's
stall stock is created from a template — real `world.items` entries held by the vendor, so the
LLM consent verbs and the code-driven purchases share one inventory. **This is the accepted cheat,
and it must eventually die**: the real chain is bakers baking at the bakehouses from millers' flour,
grain carts entering by the Wool Gate from the near countryside through Seven Lofts
(`features/the_near_countryside__aka_add_market_stalls.md` is the other half of this feature). The
nightly wallet reset in [02](02_the_spark_standard.md) is the same cheat from the money side —
both are the placeholder the supply chain replaces. [04_the_bread_round.md](04_the_bread_round.md).

**(e) Code-driven purchases are silent; LLM purchases are conversational.** A ladder-driven buy is
an atomic swap (coins for food, price from the catalog) with **self-percepts only** — the buyer and
vendor each remember it, nobody's inbox is spammed, the player hears a coin-clink the way they hear
the windlass. LLM actors keep the existing offer/accept consent dance, now with quantities, and
vendors' sheets gain a `you_sell` price line so they stop inventing prices.
[05_the_llm_seam.md](05_the_llm_seam.md).

---

## 4. What the lore already decided, and we should not re-decide

- **The loaf costs 2.** Three canon files state it (`lore/core_lore/trade_and_daily_life.md:71`,
  `lore/second_sun/07_what_everyone_knows.md:38`, `11_glossary_and_naming.md:16`): *"A loaf, two
  sparks."* The catalog keeps it verbatim: rye loaf, 2 sparks.
- **Lawful bench-fare is bread, herring, and small ale** (`07_what_everyone_knows.md:42`) — the
  plain civic meal, and the spine of the catalog below.
- **Dinner is at noon.** *"Peace with the impossible, and dinner at noon"*
  (`lore/second_sun/08_folk_culture.md:160`); High Wick is *"the main meal and the market's peak"*
  (`features/movement/01_the_clock.md:118`). The meal legs in [03](03_hunger.md) hang on this.
- **Bakers begin at the Kindling** (`07_what_everyone_knows.md:29`: *"Bakers, cooks, servants,
  carriers, and well keepers begin work"*) — which is exactly when the morning stock appears.
- **Market days move the crowd**: Highmarket at the Wickmarket and Coswald's Yard, Lowmarket at the
  Tallage and Maren's Green — already implemented as `only_on` legs.
- **The tavern pot is never empty.** The Hungry Ox keeps *"a stew pot that is never scraped to the
  bottom"* (`lore/characters/tavern_worker/g5brt_bertran_of_the_ox.json`). That is a lore-blessed
  license for the one inventory that does not deplete: tavern stew.
- **The rye is the bread grain.** The chronicle's famine is *"the rye failed twice"*; wheat is the
  fine flour. That is the whole quality axis the brief asked for: metadata `flour: rye` is daily
  bread, `flour: wheat` is fine bread at twice the price.

---

## 5. The catalog

Deliberately small — one coin, four foods. No item explosion: one bread, two named fishes, one
tavern dish. Everything else the market *talks about* (honey, cheese, eggs, simples) stays talk
until a feature needs it, and there is no generic "fish" — a fish on a slab in Ombreval is a
herring or an eel.

| kind | metadata | price | satiety | sold by | note |
|---|---|---|---|---|---|
| `spark` | — | — | — | — | the only money; fungible stack |
| `loaf` | `flour: rye\|wheat` | 2 / 4 | 150 | baker, food_provisioner | the canonical 2-spark rye loaf; wheat is fine bread |
| `herring` | — | 1 | 70 | fish_trader, market_seller | lawful bench-fare — and what one spark buys; Sven's `fzbn9` "fish" migrates to this kind |
| `smoked_eel` | — | 3 | 100 | fish_trader | Maren's Green pride; Bertran's courting gift |
| `stew` | — | 2 | 170 | cook, tavern_worker | tavern-only; the never-scraped pot, so it never runs out |

Satiety is on a 0..=255 gauge (high = satisfied), same convention as thirst. A loaf is most of a
day; a herring takes the edge off. Ale is deliberately absent — drink belongs to the thirst system
and opens a tavern-vs-well question this feature does not need to answer yet.

---

## 6. The coin loop, honestly

Coins are conserved within a day: buyers pay, vendors accumulate. Left alone, every vendor becomes a
dragon on a hoard and every buyer runs dry by Thursday. So the nightly ledger, at the Watch:

- **buyer wallets refill to their seeded level** (2–7 sparks, hashed per actor; majors keep
  authored amounts — Ilse keeps exactly 1, that is her story);
- **vendor wallets and unsold stock reset to template** — they "spent it on flour and rent."

Neither side of this is real, and that is the point: the two resets are precisely the shape of the
future supply chain (wages in, costs out), so when the chain arrives it replaces the resets rather
than fighting them. Until then the books balance by decree.

---

## 7. Milestones

Each shippable, each with a verification recipe using the repo's existing tools. Full recipes in
[06_milestones.md](06_milestones.md).

| | | Ships | How you know |
|---|---|---|---|
| **M0** | **Kinds & stacks** | the item catalog, `Item {kind, quantity, metadata}`, merge/split, `offer_item` quantity, seed migration, snapshot/mirror/HUD, **one fixture regeneration** | `cargo test -p cathedral-sim`; headless fake run shows `you_hold: c0prs spark ×3` and a 2-coin offer accepted |
| **M1** | **The spark standard** | the lore sweep (bell/lantern excised as money), the price table live in the catalog | `grep -ri 'silver bell\|gold lantern' lore/` finds only history; a vendor's sheet quotes spark prices |
| **M2** | **Hunger** | the gauge, decay, `eat` satiety, computed conditions on the sheet, meal legs, seeded wallets | headless `--trace-food` census: hunger falls all morning, the city eats at High Wick; Ilse's sheet says famished, she eats, it stops saying it |
| **M3** | **The bread round** *(the vertical slice)* | stalls bound to vendors, the Kindling restock, the queue, the silent purchase, self-percepts, the coin-clink | headless `--trace-food` logs dozens of sales at the Wickmarket on Highmarket; `tp` to the square at noon and watch the queue; ask a buyer what they paid |
| **M4** | **The Ilse purchase** | `you_sell` price lines, quantity verbs in the prompt, the LLM seam polished | replay yesterday's session and it *ends in a sale*: the baker's loaf is 2 sparks, Ilse has 1, the fish stall's herring is 1 — she offers her spark, accepts the herring, eats, and her hunger line clears |
| **M5** | **The supply chain** *(scoped, not started)* | a fixed roster of named merchants and their carters arriving through the Wool and Stone gates; `grain`, `flour`, `cloth`; millers milling, bakers baking at a bakehouse, merchants leaving with a return load — replaces both nightly resets | four sub-milestones with their own recipes in [07_the_supply_chain.md](07_the_supply_chain.md) §8; the end state is one Highmarket morning where grain walks in a gate and comes out as Ilse's loaf, with no conjuring anywhere in the trace |

M0 is where the risk lives (every layer touches items; ~13 golden fixtures pin the bytes). M1 is
pure content and can run in parallel. M2–M4 stack on M0.

---

## 8. What I want a decision on

1. **Does the whole cast hunger, or only the staged?** The plan says everyone decays (it is one
   subtraction in the existing decay loop) but only actors near a food source visibly buy; the rest
   clear hunger at meal legs (home hearth / tavern) without items, the way most of a medieval city
   actually ate. The alternative — items for every meal of 500 NPCs — is 1,000+ items a day for
   nobody to see. I recommend the cheat and the plan assumes it.
   ANSWER: Yes, let everyone decay.
2. **Day-old bread.** Unsold stock vanishes at the nightly reset. A `day_old: true` metadata flag at
   half price is one line of restock code and a nice bit of market color — but it doubles the
   catalog's effective price rows. Cheap to add later; not in the plan.
   ANSWER: Skip old bread.
3. **Player hunger.** The player currently cannot even `eat` (no `PlayerEat` engine command). The
   plan leaves the player unhungering — flying dev-camera people don't need lunch — but M0 makes
   player-held stacks render properly in the HUD, and adding `PlayerEat` later is small.
   ANSWER: No player hunger right now.

---

## 9. Files in this folder

| | |
|---|---|
| [01_items_and_stacks.md](01_items_and_stacks.md) | the item model: kinds, metadata, quantity, merge/split, every layer it touches |
| [02_the_spark_standard.md](02_the_spark_standard.md) | one coin; the lore retcon sweep, prices, wallets, the nightly ledger |
| [03_hunger.md](03_hunger.md) | the gauge, decay, satiety, rungs 3 & 7, meal legs, the computed condition |
| [04_the_bread_round.md](04_the_bread_round.md) | stalls, vendor binding, the Kindling restock, the queue, the silent purchase — and the real supply chain it stands in for |
| [05_the_llm_seam.md](05_the_llm_seam.md) | verb and sheet changes, `you_sell`, percept lines, fixtures, token cost |
| [06_milestones.md](06_milestones.md) | M0–M5, each with a verification recipe |
| [07_the_supply_chain.md](07_the_supply_chain.md) | M5 scoped: gate merchants, presence, grain/flour/cloth, the transform verb, the bakehouse, the return load |
