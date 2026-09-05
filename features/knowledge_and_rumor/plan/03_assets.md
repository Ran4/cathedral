# 03 — The assets

The two new world files, the twenty-nine new prompt strings (twenty-four frozen by M0b, five
unmeasured fragments), and where the unconditional paragraph goes. Every id in here was checked
against the shipped data; the real spelling is what is written.

Both JSON files are **embedded with `include_str!`**, following `marks.json` (`marks.rs:47`) and
`rounds.json` (`round.rs:188`) — "everything is embedded so both hosts get it with no wiring"
(`round.rs:23`). A quest pack is *not* embedded: it arrives through
`FactCatalog::extend_from_json(&str)`, host-read, because the loaders take `&str` and never a path.

---

## 1. `assets/world/facts.json`

### Schema

```
{
  "schema_version": 1,        u32, must be 1
  "_doc":           string,   ignored; declared explicitly (deny_unknown_fields cannot ignore a prefix)
  "_topic_doc":     string,   ignored
  "facts": [ FactSpec, … ]
}
```

`FactSpec`, `#[serde(deny_unknown_fields)]`:

| Key | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | string | **yes** | — | unique across the base file and every loaded pack; its dot-separated segments are relevance tokens (M1 step 13d) |
| `topic` | one of `bed blood law omen stranger coin bread craft talk` | **yes** | — | authored spellings are `Topic`'s snake_case serde names (`talk`, **never** `word`) |
| `said` | string | **yes** | — | a **template**. `{subject}` / `{place}` / `{day}` are substituted per reader; never a cast name, never `{subject}'s` |
| `own` | `{actor_id: string}` | no | `{}` | first-person templates; same placeholders |
| `subject` | `[actor_id]` | no | `[]` | drives the self-subject rule, `quiet_among` and `craft_ear` |
| `seeded` | `[actor_id]` | no | `[]` | hops-0 holders |
| `place` | area id | no | `null` | must exist in `areas.json` at `seed()` time |
| `day` | i64 | no | `null` | the world day it happened |
| `decays` | bool | no | `true` | `false` = a standing fact that never cools and is never volunteered (D52) |
| `garble` | `"none"` \| comma list of `subject`/`place`/`day` | no | `"none"` | |
| `source` | `"authored"` \| `{"custody": id}` \| `{"item": id}` \| `{"quest_phase": {"quest": s, "phase": u8}}` | no | `"authored"` | the only authored spelling of provenance, and it is never rendered |

### Shipped content

Deliberately small. **Most of the base game's facts are minted, not authored** — the city gossiping
about its own arrests and notices is the content this feature is tuned on, and it costs no authoring
at all. Two rows ship: one to prove the sealed authored path end to end in M1, one to prove `own` and
a multi-holder seal. This is the file M1 writes, **verbatim** (M1 step 11 explains the three
differences from an earlier draft: `{day}` in both templates, a de-gendered `theirs`, and `p000x`
seeded with **no** `own` line so she renders the unknown-people rule on the real city):

```json
{
  "schema_version": 1,
  "_doc": "Authored facts (features/knowledge_and_rumor/). A fact is a proposition; who holds it at first hand is `seeded`; `said` and `own` are TEMPLATES with {subject}/{place}/{day} substituted per reader, because what a sentence says depends on whether the reader knows the person in it. `said` must never bake a cast name and never put {subject} in the possessive. A fact id's dot-separated segments are the words an asker will use, because relevance selection matches on them. Embedded with include_str! exactly as marks.json is. `source` is never rendered anywhere - not a prompt, not a projection, not a log line. Most of the city's facts are minted from events, not authored here.",

  "_topic_doc": "One of bed, blood, law, omen, stranger, coin, bread, craft, talk. It decides how far the thing travels (assets/world/salience.json), how fast its hedges erode, and whose ear it catches. It is a classification with an external check, never an importance ranking: a promise is `talk` and travels almost nowhere, and that is the point of it.",

  "facts": [
    { "id": "ashe.salt.short",
      "topic": "coin",
      "said": "{subject} sold salt short of the measure at {place}, {day}",
      "own": {
        "fg2sh": "I gave good measure at the Shambles and the beam was wrong, not my hand",
        "e9nan": "I saw the weigher put his thumb on it, and nobody said a word"
      },
      "subject": ["fg2sh"],
      "seeded": ["fg2sh", "e9nan"],
      "place": "shambles",
      "day": 0,
      "decays": true,
      "garble": "place,day",
      "source": "authored" },

    { "id": "vell.stall.pitch",
      "topic": "craft",
      "said": "{subject} took the corner pitch at {place} that was not theirs to take, {day}",
      "own": {
        "dv8ll": "The corner has been mine since the Great Rains and I will not be moved off it",
        "dclsk": "I watched her set her trestles over another woman's pitch, and nobody said a word"
      },
      "subject": ["dv8ll"],
      "seeded": ["dv8ll", "p000x", "dclsk"],
      "place": "wickmarket",
      "day": 0,
      "decays": true,
      "garble": "subject,day",
      "source": "authored" }
  ]
}
```

**Ids verified on disk** (`lore/characters/**`): `fg2sh` Grigor Ashe (salt_trader, weigh) · `e9nan`
Nan (laundress, weigh) · `dv8ll` Osanne Vell (chandler, wick) · `p000x` Petronel Clove (market_seller,
wick; `knows` = `p0021, p002s, p0043` — **not** `dv8ll`) · `dclsk` Clemence Skep (market_seller, wick;
`knows` contains `dv8ll`). Area ids `shambles` and `wickmarket` are both in `areas.json`.

`ashe.salt.short` is `coin` (base 0.45) and crosses a ward or two; `vell.stall.pitch` is `craft`
(base 0.20 × 0.60 off-trade) and dies in the lane unless it finds another chandler. Reading them
against each other is the vocabulary's own worked example. The temptation to give a quest fact a high
band so it "reaches the player" is the one mistake this file exists to make visible: a dull fact
reaches the player by being **asked for**, which relevance selection already handles.

### Loader validation, in order, each a `FactCatalogError`

The error text idiom is `marks.rs:459-521`: a `struct XError { pub message: String }` with `Display`
writing the message, messages that **name the consequence**, and `Default` = `from_embedded().expect(…)`.

| Check | Message |
|---|---|
| serde parse | `invalid facts.json: {error}` |
| `schema_version != 1` | `unsupported facts schema {n}; expected 1` |
| duplicate `id` (within a file, or against an already-loaded pack) | `duplicate fact id '{id}' — a fact's identity is its id, so two rows with one id would be one fact with two texts` |
| `id` fails `is_valid_id` | `fact id '{id}' must be 1..=64 characters and free of control characters` |
| unknown `topic` string | `fact {id} has unknown topic '{t}'; expected one of bed, blood, law, omen, stranger, coin, bread, craft, talk` |
| `said` empty | `fact {id} has no said text — a fact nobody can say is a fact nobody can hold` |
| unknown token in `garble` | `fact {id} has unknown garble field '{f}'; expected none, subject, place or day` |
| `garble` includes `subject` but `said` has no `{subject}` | `fact {id} may garble its subject but its said text names no {subject} placeholder — the swap would be invisible, so the chain could never be walked back` |
| `garble` includes `place` but `said` has no `{place}` | `fact {id} may garble its place but its said text names no {place} placeholder` |
| `garble` includes `day` but `said` has no `{day}` | `fact {id} may garble its day but its said text names no {day} placeholder` |
| an `own` key is not in `seeded` | `fact {id} gives {actor} an own line but does not seed them — a first-person telling belongs to somebody who was there` |
| `{subject}` in `said` but `subject` empty | `fact {id} names a {subject} placeholder but has no subject to put in it` |
| a placeholder other than the three | `fact {id} uses unknown placeholder '{p}'; only {subject}, {place} and {day} are substituted` |
| `said` or an `own` value contains `{subject}'s` or `{subject}’s` | `fact {id} puts {subject} in the possessive — an unknown subject renders as "a chandler of the Wick Ward (you don't know their name)", and "…'s" after that is unreadable` |
| `said` or an `own` value contains `%s` | `fact {id} contains a bare %s, which is the hedge's own placeholder — substitution would run twice` |

At `FactCatalog::seed(world)`, resolution failures are **diagnostics, not errors** — a hermetic test
world legitimately lacks the cast, and a panic there would make every unit test depend on the lore:

- `fact {id}: unknown area '{a}'; place left unset`
- `fact {id}: seeded actor {a} is not in this world; dropped from seeded`
- `fact {id}: subject {a} is not in this world; row skipped` (a fact with no resolvable subject cannot
  render `{subject}`)
- `fact {id}: item {i} is in nobody's hands; source left unbound`
- `fact {id}: the store is full at {FACTS_MAX_LIVE} live facts; not installed`

---

## 2. `assets/world/salience.json`

### Schema

```
{
  "schema_version": 1,
  "_doc": string,
  "topics": { "<topic>": { "base": f64, "hedge_band": "top"|"default"|"low" }, … all nine },
  "ears":   { "<topic>": { "occupations": [occupation_id], "multiplier": f64 }, … },
  "craft":  { "own_trade": f64, "other_trade": f64 },
  "no_trade": f64,
  "household": f64
}
```
`#[serde(deny_unknown_fields)]`; every `_`-prefixed key is declared explicitly (`_doc`, `_ear_doc`,
`_no_trade_why`, `_household_why`, and `_why` on an ear), the way `MarksDoc` declares `_places_doc`
(`marks.rs:401-423`). All nine topics required in `topics`; `ears` may omit a topic (no ear).
Occupation ids are validated against `lore/core_lore/occupations.json`'s `occupation_id` values by the
**loader's caller** (`SalienceTable::from_json` cannot see the lore, so `Engine::new` checks and
emits a diagnostic — M1 step 15b).

### Shipped content — all nine bands, every affinity set

Every occupation id below was checked against `lore/core_lore/occupations.json` (65 ids) and against
the cast (`lore/characters/**`). The count in each comment is the number of authored characters
holding that trade — the size of the ear on the shipped city. **M1 writes this file; nothing in M2–M4
edits it; M5 step 11 appends one paragraph to `_no_trade_why`** (the `derived_curiosity` stacking)
and changes no number.

```json
{
  "schema_version": 1,
  "_doc": "How far each kind of news travels (features/knowledge_and_rumor/02_rumor_pollen.md). The topic tag is on the fact; the number is here, and it is the designer's whole tuning surface. base 1.00 IS the cadence target this feature was built to — it is not a new quantity. Set every base and every multiplier to 1.0 and this file is arithmetically the identity: the roll becomes curiosity x heat again, which is the model before salience existed, and M2's pre-salience numbers must reproduce exactly. `hedge_band` is authored per topic rather than derived from `base` for exactly that reason: flattening must move numbers and never move prose.",

  "_ear_doc": "One multiplier from named occupation sets, following notices.rs LAW_OCCUPATIONS, attention.rs RESERVED_TRADES and round.rs TRADE_OCCUPATIONS — a fourth of a kind, not a new pattern. Occupation ids are the spellings in lore/core_lore/occupations.json, which are also the directory names under lore/characters/.",

  "topics": {
    "bed":      { "base": 1.00, "hedge_band": "top" },
    "blood":    { "base": 1.00, "hedge_band": "top" },
    "law":      { "base": 0.80, "hedge_band": "default" },
    "omen":     { "base": 0.80, "hedge_band": "default" },
    "stranger": { "base": 0.80, "hedge_band": "default" },
    "coin":     { "base": 0.45, "hedge_band": "default" },
    "bread":    { "base": 0.35, "hedge_band": "default" },
    "craft":    { "base": 0.20, "hedge_band": "low" },
    "talk":     { "base": 0.15, "hedge_band": "low" }
  },

  "ears": {
    "bed": {
      "_why": "The trades that are inside other people's rooms. domestic_servant is the commonest occupation in the cast (45 of 519). The people who change the sheets know.",
      "occupations": ["domestic_servant", "laundress", "tavern_worker", "sex_worker",
                      "water_and_bath_worker"],
      "multiplier": 1.6
    },
    "law": {
      "_why": "notices::LAW_OCCUPATIONS verbatim (notices.rs:71) — the same instinct notices::carries already encodes absolutely, here in a weaker form, because a fact is not a notice.",
      "occupations": ["bailiff_and_gaoler", "court_officer", "civic_officer", "custody_clerk",
                      "watchman_and_keeper", "militia_and_soldier", "revenue_worker"],
      "multiplier": 1.6
    },
    "coin": {
      "_why": "It is their day.",
      "occupations": ["market_seller", "merchant", "grocer_and_spicer", "baker", "fish_trader",
                      "revenue_worker"],
      "multiplier": 1.5
    },
    "bread": {
      "occupations": ["market_seller", "merchant", "grocer_and_spicer", "baker", "fish_trader",
                      "revenue_worker"],
      "multiplier": 1.5
    }
  },

  "craft": {
    "_why": "A spoiled batch is everything to a cooper and nothing to anybody else. own_trade is matched against the fact's subject's own occupation_id at mint.",
    "own_trade": 2.0,
    "other_trade": 0.6
  },

  "_no_trade_why": "The no-trade quarter — occupation_id null, a support circumstance instead. They already have no round, loiter where they were stood and are twice as likely to speak first (AGENTS.md, the crowd knob); this row makes them hear everything as well. `The beggars know everything before anyone` stops being a mechanism to build and becomes a number in a file.",
  "no_trade": 1.4,

  "_household_why": "Anyone behind the subject's own door, or their kin. The last people to hear a scandal are the ones who live with it — which also makes telling them a scene.",
  "household": 0.15
}
```

**Ear sizes on the shipped cast** (measured, for the record): `bed` 77 (`domestic_servant` 45,
`tavern_worker` 9, `sex_worker` 8, `water_and_bath_worker` 8, `laundress` 7); `law` 63
(`civic_officer` 21, `militia_and_soldier` 15, `watchman_and_keeper` 9, `bailiff_and_gaoler` 8,
`revenue_worker` 4, plus court/custody); `coin`/`bread` 42 (`market_seller` 12, `baker` 8,
`fish_trader` 7, `merchant` 6, `grocer_and_spicer` 5, `revenue_worker` 4); no-trade 10.
Every id verified present in `occupations.json`. Note **`water_and_bath_worker`** is the id; its
display is "Water and bathing worker". Note **`no_fixed_trade` is not an occupation id** — it is the
directory the null-occupation sheets live in (`lore::NO_FIXED_TRADE_FOLDER`), which is why the
no-trade multiplier is a top-level scalar and not an entry in `ears`. **The player has no lore and is
not the no-trade quarter**: `salience()` gives them affinity 1.0 on every topic (D26).

### Loader validation

| Check | Message |
|---|---|
| serde parse | `invalid salience.json: {error}` |
| `schema_version != 1` | `unsupported salience schema {n}; expected 1` |
| a topic missing from `topics` | `salience.json is missing a band for topic '{t}' — every one of the nine must be stated, because an omitted band is a silent 0 and the topic would never travel` |
| `base` not in `0.0..=4.0`, or non-finite | `salience.json: topic '{t}' has base {b}; expected a finite 0..=4` |
| a multiplier not in `0.0..=4.0`, or non-finite | `salience.json: the {t} ear has multiplier {m}; expected a finite 0..=4` |
| an empty `occupations` list | `salience.json: the {t} ear names no occupations — omit the entry instead, so nobody reads an empty list as "everyone"` |
| a duplicated occupation within one ear | `salience.json: the {t} ear names '{o}' twice` |
| `craft.own_trade <= craft.other_trade` | `salience.json: craft.own_trade {a} is not above craft.other_trade {b} — the whole point of the craft rows is that a spoiled batch is everything to that trade and nothing to any other` |
| `household >= 1.0` | `salience.json: household damping is {h}; it must be below 1, or the subject's own house hears it first instead of last` |

`SalienceTable::flat()` is built in code, not through the loader, and sets `household = 1.0` on
purpose — its doc comment says so (D51).

Occupation-id existence is checked **where the lore is in the room** (`Engine::new`, M1 step 15b), and
a bad id is a diagnostic, not an error, because the salience table must load in worlds with no lore at
all: `salience.json: the {t} ear names unknown occupation '{o}'; the ear will match nobody`.

---

## 3. `assets/prompts/strings.toml` — the twenty-nine new keys

**M1 transcribes the twenty-four keys of `../m0_evidence/strings_draft.toml` byte-for-byte, under
their own names**, and adds five unmeasured fragments of its own. The twenty-four are `v6_both`, M0b's
shipping variant, every value measured on both providers at the shipping prompt position; a changed
string is an unmeasured string. **Do not reword anything.** The five fragments (`day_*`,
`place_unknown`) render *inside* measured sentences and are the only unmeasured text M1 ships; they
are marked below.

Appended **flat**, at the end of the file, with no `[table]` header: `PromptStrings` is
`#[serde(deny_unknown_fields)]` and `strings.toml` has zero table headers in 84 lines today, so one
header would swallow every later top-level key. Remember the **four** edit sites per key (D43).
The placeholder is the file's own `%s` (D44): exactly one in each hedge and in `day_days_past`, two in
`unknown_person_role`, none elsewhere.

```toml
# ── what a person knows (features/knowledge_and_rumor/) ───────────────────────
# Frozen by M0b (m0_evidence/strings_draft.toml, 2026-09-04), byte for byte.
# The parenthesis after `**what_you_know**`; `bullet_section` supplies the
# trailing colon, so this value does not carry one.
know_note = "what you have been told or saw yourself of the city's doings, in the words you have it in; this is the whole of it"

# The block's instruction paragraph, rendered BETWEEN the header and the
# bullets. The line breaks are load-bearing: the measured prompt had them at
# exactly these columns.
know_discipline = """
These are things you know, not lines to deliver. Say one when it bears on what
is in front of you, or when somebody asks about it, and otherwise let it lie —
a person who repeats everything they have heard is a person nobody tells
anything. Say it as it stands here: what you saw yourself you may state
flatly, and what came to you through other mouths you pass on as that, with
the same doubt still in it. Do not sharpen it: a day, a street, a name or a
number you were not given is one you do not have. And what is here is all that
is here — a thing standing right beside one of these, in the same trade or the
same quarter or the same afternoon, you still do not know."""

# The hop x band hedge ladder: one bullet per held fact, `<how you have it>:
# <what you have>`. `%s` is the fact text — the actor's own first-person line
# for the `hops0_own` rung (rendered ALONE, no wrapper), the `said` sentence for
# every other rung. Twenty-one distinct measured values, not one ladder shifted
# by band. No rung names a count or an ordinal, and none may.
know_hedge_default_hops0_own = "First hand, in your own words: %s"
know_hedge_default_hops0     = "You saw this yourself: %s"
know_hedge_default_hops1     = "They say — and the one who told you was there: %s"
know_hedge_default_hops2     = "The one who told you was not there either — they had it from the one who was: %s"
know_hedge_default_hops3     = "It came to you down a line of mouths, and whoever at the end of it actually saw anything is a stranger to you: %s"
know_hedge_default_hops4     = "This has been through a great many mouths before it reached yours, worn smooth in the telling, and nobody can now say who started it: %s"
know_hedge_default_cold      = "You heard something of the sort a while back and it has gone dim: %s"

know_hedge_top_hops0_own = "First hand, in your own words: %s"
know_hedge_top_hops0     = "You saw this yourself: %s"
know_hedge_top_hops1     = "Flatly, as a thing that happened: %s"
know_hedge_top_hops2     = "They say: %s"
know_hedge_top_hops3     = "It is the common word now, and nobody bothers to say where they had it: %s"
know_hedge_top_hops4     = "This is simply what is known in this city now, whoever said it first: %s"
know_hedge_top_cold      = "Old talk now, but you know how it went: %s"

know_hedge_low_hops0_own = "First hand, in your own words: %s"
know_hedge_low_hops0     = "You saw this yourself: %s"
know_hedge_low_hops1     = "They say — and you have it from the one mouth only: %s"
know_hedge_low_hops2     = "You had it from somebody who had it from somebody, and it is trade-talk: %s"
know_hedge_low_hops3     = "Passed to you at a remove you cannot account for, the sort of small news that gets carried because there is nothing else to carry: %s"
know_hedge_low_hops4     = "Stall talk, handed along so often that its edges are gone, and you have never met anyone who was there: %s"
know_hedge_low_cold      = "Somebody mentioned it once, long since, and you would not swear to a word of it: %s"

# A fact's subject the reader has never been told the name of. TWO `%s`, in
# this order: the trade (occupation_display, first letter lowered), then the
# ward (prompt::ward_label, "the Wick Ward"). `unknown_person_name` (line 11)
# is the no-role fallback. No comma between the two: the comma form was tried
# and rejected in M0.
unknown_person_role = "a %s of %s (you don't know their name)"

# ── M1's own fragments — UNMEASURED, rendered inside the measured sentences ──
# `{day}` in a said/own template. No digit ever reaches a sheet: `day_days_past`'s
# `%s` is filled with the count in words (two..seven); eight days and beyond,
# every undated fact and every clock-less world render `day_long_ago`.
day_today     = "today"
day_yesterday = "yesterday"
day_days_past = "%s days past"
day_long_ago  = "a long while back"
# `{place}` when the area key no longer resolves.
place_unknown = "somewhere in the city"
```

The `prose/v6_both/hedges.toml` the harness fired used `{said}`/`{own}` placeholders; the frozen
`strings_draft.toml` translates them to `%s`, and that translation is the only difference — a
substitution-only choice, not a prose change, verified to round-trip byte for byte (`NOTES.md`,
"The freeze (M0b)").

### The band × rung table the renderer applies (D18)

`hedge_band` from `salience.json` (§2) picks the column — the **fact's** topic, never the reader's
affinity; the rung comes from `rung_for(band, hops, cold, has_own)`, one rule:

1. the reader has an `own` line → `*_hops0_own`, whatever the heat (D15);
2. else if cold (`!knowledge::volunteers(...)`) → `*_cold`, over every hop count including 0;
3. else `hops0` / `hops1` / `hops2` / `hops3` / `hops4` (four removes or more).

| | `top` (bed, blood) | `default` (law, omen, stranger, coin, bread) | `low` (craft, talk) |
|---|---|---|---|
| own line | `know_hedge_top_hops0_own` | `know_hedge_default_hops0_own` | `know_hedge_low_hops0_own` |
| cold, no own line | `know_hedge_top_cold` | `know_hedge_default_cold` | `know_hedge_low_cold` |
| hops 0 | `know_hedge_top_hops0` | `know_hedge_default_hops0` | `know_hedge_low_hops0` |
| hops 1 | `know_hedge_top_hops1` | `know_hedge_default_hops1` | `know_hedge_low_hops1` |
| hops 2 | `know_hedge_top_hops2` | `know_hedge_default_hops2` | `know_hedge_low_hops2` |
| hops 3 | `know_hedge_top_hops3` | `know_hedge_default_hops3` | `know_hedge_low_hops3` |
| hops ≥ 4 | `know_hedge_top_hops4` | `know_hedge_default_hops4` | `know_hedge_low_hops4` |

Two things this table is: the *only* implementation of "salience shortens the ladder" (no new state,
one column, twenty-one measured cells), and M0b's own evidence — the erosion relationship reads at
three and four removes on both providers (`NOTES.md`, "The band fires"). The `*_hops0_own` rung
renders the `own` template **alone**, with no wrapper: a narrator in front of a witness's own
first-person words swaps person mid-sentence, and that wrapper was rejected in all three M0 variants.
`know_hedge_low_hops3` is the weakest rung of the 21 (n = 1 per provider; thinned on openai,
mis-counted on moonshot) and ships labelled; M3's re-measurement gives it its n = 2.

**M3 adds one key, `known_from`** (the immediate mouth, appended to a bullet at one remove or more;
`" — you had it from %s"`, unmeasured, M3 step 7) and **no rung**.

---

## 4. `assets/prompts/turn.j2` — the unconditional paragraph

Insert the 22 lines between the two markers of `../m0_evidence/ignorance_rule.txt` **verbatim**
(1386 bytes), followed by **one blank line**, immediately **before line 194**:

```
194  Use ONLY the verbs listed below, spelled exactly as shown (lowercase English).
```

That is exactly where M0b measured it (every one of its 110 calls; `ignorance_rule.txt`, "PLACEMENT
WAS MEASURED, NOT ASSUMED"). After the edit the paragraph occupies 194–215, the blank line is 216 and
`Use ONLY the verbs` is 217; every later `turn.j2` line moves by +23 (`gesture` 230 → 253, `set_goal`
231 → 254, `Output like this` 242 → 265).

**Unconditional. Not behind any `{% if %}`.** Its own first sentence is about the block's *absence* —
"A sheet with no what_you_know on it means nobody has told you anything and you saw nothing yourself:
that empty place is itself an answer" — which is the spec's `what_you_do_not_know` counter-block
bought for free, and which is precisely why M0's two block-less sheets are where the losing variant
failed. Gating it would hide it from its only audience.

Verbatim text (1386 bytes, 22 lines; the two bullets differ from round 1's `v2_structural` — repair R2,
measured):

```
what_you_know is the whole of what you have been told, or saw for yourself, of
what has happened in this city — all of it, and there is no more of it further
down your sheet or anywhere in your head. A sheet with no what_you_know on it
means nobody has told you anything and you saw nothing yourself: that empty
place is itself an answer, and a plain one to give.

So when you are asked about a thing that is not there, you do not know it, and
saying so is no failure. Say so plainly, in your own manner, and then send the
asker on — do one of these, not none of them:

- name the trade that would know it, whoever handles that sort of thing all day;
- name the post whose business it is, or the officer who keeps such things;
- name where it happened, or who was standing there.

Having nobody to send them to is an answer too; say that. But never fill a gap
in somebody else's story with a name, a day, a street or a number — not out of
your sheet, not out of what would make sense — and do not slip one past as a
guess, a likelihood or a maybe: whatever you say aloud in this city is carried
on by whoever heard you, and a guess repeated twice is what the ward believes.
Being wrong about a thing you were told is ordinary here; making one up is not.
None of this binds your own trade, your own day, your own household or your own
opinions: speak of those as freely as ever.
```

**The golden consequence, exactly:** every one of the 22 fixtures in
`crates/cathedral-sim/tests/fixtures/prompts/*.txt` gains **+23 lines and +1387 bytes** (the 22 rule
lines plus one blank separator) and loses nothing. `git diff --stat` on that directory must show the
same insertion count on all 22 files and zero deletions; anything else means a section is rendering
unconditionally and the fix is the rendering, not the fixture (D40). The four `contains` assertions
on `turn.j2` prose in `crates/cathedral-sim/tests/prompt_tests.rs` (the tests at `:708`, `:725`,
`:742`, `:764`) check fixed sentences and not positions, so they stay green.

**M4 adds the `raise_word` fence line**, gated on `has_raise_word`, immediately **before `set_goal`**
(`turn.j2:254` after M1) — the position all three M0 Q5 scenarios inserted it at
(`verbs.add[0].before == "set_goal"`, proven by the fired sheet
`m0_evidence/sheets/v2_structural/q5_raise_word_with_occasion.txt:130-132`). The comment is the
measured one (`scenarios/q5_raise_word_with_occasion.json:9`), byte for byte; only the **example**
changes, per D36, to a `bread` claim unrelated to any occasion so the model's topic choice becomes
measurable instead of copyable:

```
{% if has_raise_word %}raise_word {"topic": "bread", "said": "the Wickmarket had no rye at all by midday"}  # Start a word going about something you have just been told and did not know; topics: bed, blood, law, omen, stranger, coin, bread, craft, talk
{% endif %}
```
The `topics:` tail is the one place the closed list is named to a model, carved out of D14 by name.
No prose anywhere in `turn.j2` explains `raise_word` beyond that comment: M0's cleanest result is the
no-occasion control, and it is clean precisely because all three variants deliberately said nothing
about the verb — the sim's gate is what tells the model when, and the sheet's silence is what makes
that measurable.

---

## 5. Fixture files this feature adds

| Path | What |
|---|---|
| `crates/cathedral-sim/tests/fixtures/pollen/cadence_band.json` | 9 facts, one per topic, one place, one shared 4-person `seeded` set carrying none of the `craft_ear` trade — the pack both ends of the band are measured on (`02_numbers.md` §7) |
| `crates/cathedral-sim/tests/fixtures/pollen/craft_ear.json` | the `Craft` fact alone, `seeded` = three coopers — the reported same-trade term |
| `crates/cathedral-sim/tests/fixtures/pollen/household.json` | one `Bed` fact about somebody with ≥ 2 kin present and a door — "the household is last" |
| `crates/cathedral-sim/tests/fixtures/facts/bad_*.json` | one file per loader-validation row above, each asserting its exact message substring |

`../m0_evidence/` is kept as-is and never edited except to append: it is the only record of *why* the
prose is worded the way it is, which the spec's test contract requires. M3's Q4 re-measurement and
M4's C1 re-fire are appended beside the measured replies, never in place of them.
