# The copper standard

The brief: *"Simplify the coin system; lore currently is overly complex about this. It might be
realistic, but keep to just copper coins for everything."* This document is the ruling, the
arithmetic, the sweep list, and the wallet mechanics that make one coin actually circulate.

---

## 1. What the lore has today

Three coins, stated canonically in `lore/core_lore/trade_and_daily_life.md:62-74` and restated in
at least four other canon files:

| coin | metal | rate | street name |
|---|---|---|---|
| **spark** | copper | base | "penny" |
| **bell** | silver | 12 sparks | — |
| **lantern** | gold | 60 bells (= 720 sparks) | "mostly a money of account" |

Plus one historical unit: the **mark** of the founding ransom, which the lore itself already
disclaims as a coin (*"The F.54 mark was not a coin. It was a weight and money of account"* —
`lore/founding_story_the_four_hundred_marks.md:161-166`).

The realism is genuine and the confusion is too. The city's day runs on seven **bell** offices rung
from a **bell** tower, and its silver coin is also called the bell — *"the coin says spark, the
mouth says penny, and no clerk has ever won that argument"* (`11_glossary_and_naming.md:16`). Three
names for the copper, a coin named after the city's loudest other thing, and a gold unit nobody
ever touches. For a game where an LLM narrates prices aloud to a player over lossy speech-to-text,
this is pure noise.

## 2. The ruling

**One circulating coin: the copper coin.** In prose: "a copper", street name "a penny". Both map to
the single item kind `copper_coin` ([01](01_items_and_stacks.md) §2.1).

- The **silver bell and gold lantern are excised as money** — not retconned into rarity, *gone*:
  the mint of Ostrelle struck copper and nothing else. "Bell" belongs to bells again.
- The **mark stays**, exactly as the founding story already frames it: a historical weight of
  silver, never a coin, never convertible. No text change needed there beyond removing the one
  cross-reference to bells/lanterns.
- The word **"spark" is retired** in favor of copper/penny (README §8.2 holds the one open
  question: whether *"not worth a spark"* survives as an archaism the glossary can keep).

### 2.1 The conversion is 1 spark = 1 copper

No redenomination. Every spark price is already right; every bell price multiplies by 12. The
canonical anchors, restated:

| thing | was | becomes |
|---|---|---|
| a loaf | 2 sparks | **2 coppers** (unchanged — the catalog's anchor price) |
| a herring, a message run | a penny | **1 copper** |
| Sven's debt to Conny for that fish | 2 coppers | **2 coppers** (a debt in the lore, not a list price — the catalog sells herring at 1) |
| a journeyman's day | 3 bells | **36 coppers** |
| gatehouse week's wage | a roof + 8 sparks | **a roof + 8 coppers** |
| a pauper's funeral with knell | 8 bells | **96 coppers** |
| Ewart's debt to Averil Skell | 4 bells (the lore already computes it: "forty-eight") | **48 coppers** |

Big numbers stay big — a funeral costs 96 coppers and that is fine, because a quantity is one stack
row (`c0prs copper coin ×96`), not 96 entities. The alternative (redenominating wages down so
prices compress) would break the one price three canon files agree on, the 2-spark loaf, and is not
worth it.

## 3. The sweep

The complete list of files that mention silver bells / gold lanterns as money, or price things in
them (from a full-corpus survey, 2026-07-17). The sweep rewrites coin *mechanics* everywhere but
preserves *stories* — debts, dowries, ransoms keep their narrative weight, restated in coppers.

**Canon (the load-bearing rewrites):**
`lore/core_lore/trade_and_daily_life.md` (§Money — becomes three sentences about the copper),
`lore/core_lore/core_lore.md:91`, `lore/second_sun/07_what_everyone_knows.md:36-38`,
`lore/second_sun/11_glossary_and_naming.md` (Bell entry loses its coin sense),
`lore/second_sun/00_canon.md`, `lore/second_sun/04_chronicle_of_the_city.md`,
`lore/second_sun/05_dramatis_personae.md`, `lore/second_sun/12_beyond_the_walls.md`,
`lore/core_lore/setting_and_geography.md`, `lore/core_lore/calendar_and_history.md`,
`lore/core_lore/occupations.json`, `lore/founding_story_the_four_hundred_marks.md` (mark passages
stay; bell/lantern cross-refs go), `lore/the_dry_boatmen.md`.

**Design docs:** `lore/second_sun/design/00_vision.md`, `02_voice_and_the_passphrase.md`,
`03_questlines.md`, `04_systems_integration.md` (its proposed "bell-coins" contraband item becomes
plain coin), `lore/second_sun/documents/edict_of_the_undivided_light.md`, `trial_records.md`.

**Families:** `family_alder.md` (the 4-bell debt → 48 coppers), `family_ashe.md`,
`family_copp.md`, `family_fitch.md`, `family_marle.md`, `family_rasp.md`, `family_rud.md`
(3-bells-a-day fulling → 36 coppers), `family_skell.md`, `family_sparr.md`,
`lore/families/crests/image_prompts.md`.

**Character sheets** (grep hits for coin-sense bells/lanterns): `boatworker/g4ewt_ewart_alder`,
`candor_cleric/a9rnh_renn_hobbe`, `chandler/dpcrk_petronel_crake`, `cloth_worker/et7rd_tam_rud`,
`executioner/hgiha_gile_of_harne`, `fish_trader/g2rhs_averil_skell`,
`general_labourer/b0nll_noll_limeburn`, `mason/b6clm_clemence_limeburn`,
`mason/b9stt_bertran_stott`, `merchant/fa4sg_ansel_of_salorge`,
`watchman_and_keeper/hrnsk_renn_skell`, `leather_worker/ebhid_tobin_tarn`.

**Features:** `features/50_cool_suggestions.md`,
`features/implemented/make_dry_boatmen_consistent_with_lore.md`,
`features/implemented/the_four_hundred_marks.md`.

Caution for the sweeping agent: **"lantern" is almost always a lamp** and "bell" is almost always a
bell — only the coin senses change. The famine line *"Bread was cried at the Bellstand at a price
not to be written"* needs nothing: it was wise enough not to name a denomination.

This sweep is M1 and is pure content — it can run fully in parallel with M0, ideally as one
subagent per file group with the conversion table above as the contract.

## 4. Wallets

One coin only works if people carry it. Today exactly one NPC in the city holds money (Ilse, 1
copper — which is *why* every purchase demo dead-ends).

- **Seeding.** Every enrolled townsperson gets a starting wallet: `2 + floor(6 · hash01("wallet",
  id, 0))` coppers (2–7), the deterministic-hash idiom the round already uses for thirst spread
  (`round.rs:877-878`). Majors with authored holds keep them — **Ilse keeps exactly 1**; her
  reluctance to spend it is her character sheet. Vendors additionally seed a float
  ([04](04_the_bread_round.md) §3).
- **Implementation:** wallets are ordinary `copper_coin` stacks in `World.items`, conjured at round
  seed with deterministic ids (`w_<actor id>`), so they are visible in `you_hold`, offerable,
  stealable-by-consent, and the LLM needs no special money concept — money is just a stack you
  hold, like the prompt already teaches.
- **The nightly ledger, at the Watch:** buyer wallets refill to seed level; vendor wallets and
  unsold stock reset to template. Hooked on the same `offices_crossed` beat as the restock
  ([04](04_the_bread_round.md) §3). Both resets are the confessed placeholder for wages-in /
  costs-out; the supply chain (M5) replaces them.

## 5. What the game says about prices

Prices live in one place — the catalog's `price_coppers` ([01](01_items_and_stacks.md) §2.1) — and
flow to both consumers:

- the **ladder's silent purchase** pays exactly list price ([04](04_the_bread_round.md) §5);
- the **LLM vendor's sheet** quotes list price in a `you_sell` line
  ([05](05_the_llm_seam.md) §3), so a baker asked "how much?" stops improvising and says "two
  coppers." Haggling stays legal — the sheet is an anchor, not a cage; an LLM vendor charmed into
  offering a herring for free is roleplay working as intended.

The prompt needs no glossary of coinage. "Copper coin" in `you_hold`, a price in `you_sell`, and
the transcript's own "I have but one copper" — the model already speaks this language; we are
finally making the world agree with it.
