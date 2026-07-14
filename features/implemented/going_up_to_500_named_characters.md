# Going up to 500 named characters

## Summary

Expand the authored, interactive cast from 103 to exactly 500 named characters.

The 500 are not Ombreval's literal population. Ombreval is a dense 1.2 x 1.0 km
city-state and must contain many thousands of people in the fiction. The 500 are
the named people the game can instantiate and let the player interact with: a
deliberately visibility-weighted sample of the city rather than a census.

The present 103 are a good set of principals and occupation anchors, but as a
population they are much too established, skilled, connected and narratively
consequential. The additional 397 should make Ombreval feel inhabited by
households, servants, wage workers, children, migrants, paupers, beggars,
travellers, criminals, entertainers, prisoners and ordinary people whose lives
do not revolve around the Great Rose.

This feature uses the significance values defined in
`lore/characters/AGENTS.md`:

- `major`: canon-critical characters;
- `minor`: authored supporting characters who may be mentioned in lore;
- `ambient`: individually non-canonical, cheaply simulated population
  characters who remain interactable and specific.

Significance is narrative importance, not social status, wealth, virtue or
human worth. A beggar can be major and a rich merchant can be ambient.

## Goals

1. Preserve the existing 103 characters unless a separate lore review says
   otherwise.
2. Add the missing "significance" to the existing 103 characters
3. Add 397 characters for a total of exactly 500.
4. Correct the current over-representation of masters, officials and unique
   trades.
5. Represent domestic service, casual labour, poverty, childhood, migration,
   civic government, defence, crime, entertainment and other ordinary urban
   life.
6. Make most of the new cast `ambient`, with enough specificity to converse
   coherently but without 397 new quest-sized biographies.
7. Keep occupation, rank, status, criminal activity and significance as
   separate concepts.
8. Implement a compute policy that makes 500 actors affordable before enabling
   all 500 in normal play (but make NO changes right now to that; 500 will render just fine atm).

## Non-goals

- Declaring 500 to be Ombreval's total population in the lore or elsewhere.
- Giving every character a tragedy, conspiracy, unique secret or faction role.
- Making poverty, disability or criminality interchangeable.
- Treating `ambient` as a synonym for poor. Every social layer may contain
  ambient characters.
- Turning `beggar`, `widow`, `orphan`, `prisoner` or `homeless` into ordinary
  trade families.
- Establishing monasteries, hospitals, noble courts or other major institutions
  solely to provide jobs. New institutions require their own canon decision.
- Making the Candor the secular government. Core lore deliberately leaves the
  exact civic constitution open at the moment.

## Baseline audit

The current 103 sheets use all 48 occupation families in
`lore/core_lore/occupations.json`. There are no missing `occupation_id` values.

Current occupation counts:

```text
5 each:
  candor_cleric, cloth_worker, mason

4:
  glazier

3 each:
  boatworker, cargo_worker, carpenter_and_builder, chandler,
  church_attendant, custody_clerk, fish_trader, general_labourer,
  healer, messenger, scribe_and_clerk, watchman_and_keeper

2 each:
  baker, bell_ringer, cook, draper, funerary_worker, lamplighter,
  laundress, leather_worker, market_seller, merchant, money_dealer,
  revenue_worker, salt_trader, scavenger, smith, tavern_worker

1 each:
  anchoress, bellfounder, brewer, butcher, court_officer, executioner,
  farmer, freight_broker, guide, instrument_maker, miller, painter,
  pilgrim, roper, salt_worker, scholar
```

Other baseline facts:

- 26 of 103 are masters, mistresses or wardens.
- Only 3 are general labourers.
- There are no domestic servants.
- There are 8 paupers, but nearly all still have stable work.
- Nobody's main means of survival is street begging.
- There are no sex workers, prisoners, gaolers, soldiers or entertainers.
- Only 7 characters are under 16, and only one is under 12.
- 14 have an `illegal_activity`, but 6 of those are heresy. Conventional crime
  is limited to three thieves, two smugglers, two fences and one forger.
- 97 distinct exact titles are used by 103 people. The cast therefore reads
  more like a catalogue of trades than a city crowd.
- 102 of 103 know at least one other named character, with 774 `knows` links.
- The median `core_character_description` is about 222 words.
- The occupation registry already contains 160 valid alternative titles which
  no current character uses.

## Significance distribution

Use this provisional final target:

| Significance | Final count | Purpose |
|---|---:|---|
| `major` | 30 | Canon-critical people, future quest anchors, faction and institutional principals |
| `minor` | 120 | Named supporting cast, occupation anchors and recurring local characters |
| `ambient` | 350 | Replaceable street, household and workplace population |
| **Total** | **500** | |

The existing 103 must be audited rather than automatically declared major.
Long backstory does not make a character major; external canon dependency does.
If the existing cast contains no ambient characters, the new batch will contain
350 ambient characters and only 47 new major/minor characters. If some existing
characters are reclassified as ambient, adjust the new counts so the final
targets remain 30/120/350.

### Major

- Individually required by canonical lore, a faction, an institution or a
  planned quest.
- May be referenced freely by other character sheets and lore documents.
- Receives the largest reasoning budget and richest persistent memory.
- Has a full authored description, consequential relationships and a clear
  place in the city.
- Removal requires a repository-wide lore and reference audit.

### Minor

- A stable named supporting character: a master tailor, ward officer, known
  beggar, brothel keeper, militia captain, gang organiser or recurring servant.
- May be mentioned in lore, but usually only locally or once or twice.
- Receives normal interaction compute and persistent memory.
- Has a compact but real backstory, several relationships and at least one
  continuing concern.
- Removal also requires a reference audit.

### Ambient

- Must not be individually named outside their own character sheet by core
  lore, second-sun lore, features presented as canon, major/minor character
  relationships, quests, canonical events or unique items.
- May have relationships with other ambient characters. Those relations are
  replaceable data, not canon.
- May point outward to a major/minor employer or public figure from their own
  sheet, but the stable character must not point back to the ambient character
  by id or name.
- Must have enough specificity to avoid becoming a generic crowd token:
  locality, livelihood/status, speech manner, current activity and one
  immediate material concern.
- Should normally have no unique faction office, quest item or world-changing
  secret.
- Is replaceable between authored versions. Once encountered in a save, their
  runtime identity and memories must remain stable for that save.
- Dynamic player interest may temporarily increase compute and memory without
  changing the character's canonical significance.

Do not include the words `major`, `minor` or `ambient` in the NPC's own prompt
as a statement about who they are. `significance` is host scheduling and
authoring metadata, not self-knowledge.

## Profile depth by significance

These are authoring targets, not hard byte limits:

| Significance | Core description | Extended description | Static relationships |
|---|---|---|---:|
| `major` | Roughly 150-300 words | As much as needed | Usually 5-15 meaningful links |
| `minor` | Roughly 80-180 words | Optional, roughly 600-2000 words | Usually 2-6 links |
| `ambient` | Roughly 40-90 words | Normally empty | Usually 0-2 links |

An ambient description should normally answer five things in a few sentences:

1. What are you doing here?
2. How do you materially survive?
3. How do you speak or react to strangers?
4. What small thing do you need today?
5. What immediately visible fact makes you distinct from the next person?

The immediate concern can be a wet blanket, a disputed penny, an employer who
is late, a missing chicken, sore feet, a place in the hiring line or fear of
losing a sleeping place. It does not need to be a secret or quest.

## Occupation and status model

`occupation_id` remains a trade or livelihood family. These belong in separate
fields:

- `rank`: guild or institutional rank;
- `faction_role`: a special role in a faction;
- `illegal_activity`: prohibited conduct;
- `statuses`: poverty, housing, family, residency and legal statuses;
- `conditions`: physical and health conditions;
- `significance`: canonical and computational importance.

A character who begs should usually retain a real former, occasional or
intermittent occupation. Examples include an injured porter, an unemployed
labourer, a widowed laundress, a retired watchman or an out-of-work servant.
Begging is then represented by statuses such as `pauper`, `alms_dependent`,
`unhoused` and `begs_regularly`, plus the description and goal.

### Recommended schema additions

Add:

```json
{
  "significance": "ambient",
  "statuses": ["pauper", "unhoused", "begs_regularly"]
}
```

Keep `conditions` for health and body rather than mixing it with widowhood,
poverty and legal status. Migrate the current status-like condition values when
the field is introduced.

Permit `occupation_id` and `title` to be null for a small number of genuine
dependants and people with no present or former trade. Store those sheets under
`lore/characters/no_fixed_trade/`; that directory is a structural bucket and
must not become an occupation registry entry. Loader validation becomes:

- non-null occupation: folder must equal `occupation_id`, and `title` must be a
  valid title for that occupation;
- null occupation: folder must be `no_fixed_trade`, `title` and `rank` must be
  null, and the sheet must explain the person's means of support.

If nullable occupations are deferred, assign the person's honest former or
intermittent trade. Do not create a `beggar` occupation merely to satisfy the
loader.

Suggested controlled status vocabulary:

```text
alms_dependent
begs_regularly
dependent
insecure_lodging
intermittently_employed
noncitizen
orphan
pauper
prisoner
recent_migrant
retired
unemployed
unhoused
widow
widower
```

The vocabulary can grow, but generation and validation should not create many
spelling variants for the same status.

## Proposed new occupation families

Add 17 occupation families and 208 characters. Titles are provisional until
added to `lore/core_lore/occupations.json`.

| New `occupation_id` | Add | Suggested titles and scope |
|---|---:|---|
| `domestic_servant` | 45 | Household servant, housemaid, chamber servant, housekeeper, manservant, kitchen maid, nursemaid, wet nurse, errand child |
| `garment_worker` | 14 | Tailor, cutter, embroiderer, mender, hosier, capper, hatter |
| `shoemaker` | 12 | Cordwainer, cobbler, shoe mender, pattener |
| `cooper` | 10 | Cooper, barrel-maker, cask-maker, tub-maker, hoop-setter |
| `potter` | 8 | Potter, kiln worker, vessel maker, tile maker |
| `cartwright_and_wheelwright` | 6 | Cartwright, wheelwright, wagon repairer, handcart maker |
| `fine_metalworker` | 8 | Cutler, locksmith, brazier, pewterer, goldsmith, silversmith, armourer |
| `animal_worker` | 14 | Ostler, stable hand, drover, horse keeper, swineherd, poultry keeper |
| `sanitation_worker` | 12 | Dung carter, privy cleaner, gutter raker, refuse collector, street sweeper, cesspit digger |
| `water_and_bath_worker` | 8 | Water carrier, well keeper, bathhouse keeper, bath attendant, boiler tender |
| `food_provisioner` | 18 | Poulterer, cheesemonger, milk seller, egg seller, grain dealer, fruit seller, vegetable seller |
| `grocer_and_spicer` | 5 | Grocer, spicer, oil seller, dried-goods seller |
| `entertainer` | 10 | Musician, singer, storyteller, juggler, dancer, puppeteer, gaming-house keeper |
| `civic_officer` | 7 | Councillor, ward officer, chamberlain, city treasurer, civic surveyor; exact titles wait for the government brief |
| `bailiff_and_gaoler` | 8 | Bailiff, sergeant, gaoler, prison guard, debt officer, court usher |
| `militia_and_soldier` | 15 | Militiaman, militia captain, wall guard, armoury keeper, hired soldier |
| `sex_worker` | 8 | Sex worker, brothel keeper, house keeper, procurer; legal status remains separate |
| **Total** | **208** | |

Boundary notes:

- Keep farriers and ordinary iron forging under `smith`; use
  `fine_metalworker` for materially different specialist shops.
- Keep seamstresses who mainly sew household linen under `cloth_worker` if
  appropriate; use `garment_worker` for cutting, fitting and finished clothing.
- Keep tavern cooks and scullions in `cook` or `tavern_worker`; domestic cooks
  and kitchen servants may be `cook` or `domestic_servant` according to their
  main identity.
- Sex work is a livelihood, not automatically an `illegal_activity`. Decide how
  Ombreval licenses, fines or tolerates it separately.
- A public bathhouse is plausible but not yet fixed canon. If one is not
  established, use the eight `water_and_bath_worker` slots mostly for water
  carriers, well keepers and private bath attendants.
- Do not add a hospital merely to house healers or paupers. Existing parish,
  household and alms networks can support them until a hospital is separately
  established in canon.

## Expansion of existing occupation families

Add 179 characters to existing occupation families. Prefer the 160 currently
unused valid alternative titles before inventing more titles.

| Existing `occupation_id` | Current | Add | Final |
|---|---:|---:|---:|
| `general_labourer` | 3 | 20 | 23 |
| `cargo_worker` | 3 | 13 | 16 |
| `market_seller` | 2 | 10 | 12 |
| `scavenger` | 2 | 5 | 7 |
| `tavern_worker` | 2 | 7 | 9 |
| `cook` | 2 | 6 | 8 |
| `baker` | 2 | 6 | 8 |
| `brewer` | 1 | 3 | 4 |
| `butcher` | 1 | 3 | 4 |
| `miller` | 1 | 2 | 3 |
| `farmer` | 1 | 6 | 7 |
| `carpenter_and_builder` | 3 | 7 | 10 |
| `mason` | 5 | 5 | 10 |
| `cloth_worker` | 5 | 7 | 12 |
| `chandler` | 3 | 4 | 7 |
| `smith` | 2 | 3 | 5 |
| `leather_worker` | 2 | 3 | 5 |
| `laundress` | 2 | 5 | 7 |
| `roper` | 1 | 2 | 3 |
| `boatworker` | 3 | 6 | 9 |
| `fish_trader` | 3 | 4 | 7 |
| `salt_worker` | 1 | 3 | 4 |
| `merchant` | 2 | 2 | 4 |
| `pilgrim` | 1 | 7 | 8 |
| `watchman_and_keeper` | 3 | 6 | 9 |
| `court_officer` | 1 | 2 | 3 |
| `revenue_worker` | 2 | 2 | 4 |
| `scribe_and_clerk` | 3 | 3 | 6 |
| `healer` | 3 | 3 | 6 |
| `church_attendant` | 3 | 3 | 6 |
| `candor_cleric` | 5 | 2 | 7 |
| `funerary_worker` | 2 | 1 | 3 |
| `bell_ringer` | 2 | 1 | 3 |
| `glazier` | 4 | 2 | 6 |
| `draper` | 2 | 1 | 3 |
| `lamplighter` | 2 | 2 | 4 |
| `messenger` | 3 | 3 | 6 |
| `guide` | 1 | 2 | 3 |
| `money_dealer` | 2 | 1 | 3 |
| `scholar` | 1 | 1 | 2 |
| `painter` | 1 | 1 | 2 |
| `instrument_maker` | 1 | 1 | 2 |
| `bellfounder` | 1 | 1 | 2 |
| `freight_broker` | 1 | 1 | 2 |
| `salt_trader` | 2 | 1 | 3 |
| **Added to existing families** |  | **179** | |

Leave `anchoress`, `custody_clerk` and `executioner` at their present counts
unless later lore establishes a reason to add one. The Custody is already
strongly represented relative to ordinary civic life.

Add 10 further characters with genuinely no fixed occupation, producing the
complete arithmetic:

```text
103 existing
+ 179 in existing occupation families
+ 208 in 17 new occupation families
+  10 with no fixed occupation
= 500 named characters
```

The exact occupation allocation is a design target. Moving a few slots between
closely related families is acceptable if the final social targets and total
remain intact, but do not let skilled masters consume the servant, labourer,
poor or dependent allocations.

## Poverty, begging and housing insecurity

Target the following overlapping totals in the final cast:

- 80-110 materially poor characters;
- 30-45 who regularly beg or depend heavily on alms;
- 25-40 with no secure sleeping place or only unstable lodging;
- 50-70 precarious workers one injury, flood or missed wage away from pauperism;
- a mixture of lifelong, situational, seasonal and old-age poverty.

Most regular beggars should be ambient, but include roughly 2-5 minor beggars
and allow a major beggar if future canon or a quest needs one.

Do not make every beggar old, disabled or fraudulent. Include:

- disabled people who work and disabled people who cannot;
- able-bodied people unable to find hire;
- widows, widowers and abandoned spouses;
- orphaned and displaced children;
- retired workers without family support;
- injured craftspeople and porters;
- migrants whose money or promised job has disappeared;
- former servants turned out by a household;
- seasonal workers between jobs;
- honest pilgrims in distress and a small number of false pilgrims;
- a few organised, territorial or deceptive beggars without making fraud the
  norm.

Place visible poverty around the Gradine, Saint Maren's, market edges, hiring
places, gates, inns, cheap lodging streets and sheltered passages. Do not make
every poor person stand permanently at a begging pitch.

## Crime and people up to no good

Target 40-60 final characters with a conventional `illegal_activity`, in
addition to the setting's heresy offences. Only around 15-25 should be primarily
predatory criminals. The remainder should be ordinary workers breaking laws at
the edge of survival or profit.

Add a mixture of:

- pickpocketing and purse-cutting;
- burglary and receiving household goods;
- cargo pilfering and warehouse leakage;
- toll evasion, false manifests and hidden passengers;
- coin clipping and counterfeiting;
- false weights, watered ale and adulterated food;
- unlicensed vending and guild evasion;
- clandestine workshops using stolen tools or materials;
- illegal gambling and rigged games;
- brothel keeping or procurement where local law prohibits it;
- protection, intimidation and hired violence;
- corrupt watchmen, bailiffs, clerks and weighers;
- false pilgrims, fraudulent guides and counterfeit badge or relic sellers;
- poaching, stolen livestock and illicit slaughter;
- violent debt collection;
- gate, wharf and customs bribery;
- commercial sabotage or arson.

Criminal activity remains an overlay on a normal occupation. A pickpocket may
be a servant, messenger or market seller; a smuggler may be a boatworker,
merchant or porter; a corrupt official remains an official.

Avoid turning Ombreval into a city where every poor person is dangerous and
every official is corrupt. Exploitation by respectable employers, landlords,
guild masters and lenders should also appear even when it is legal.

## Households, life stages and migration

The added population should be generated as households, workshops, work crews,
lodging houses and street clusters rather than 397 unrelated individuals.

Include:

- artisan households containing family, apprentices, journeymen and servants;
- wealthy clerical and merchant households with several servants;
- one-room households, lodgers and subtenants;
- servant sleeping spaces, shop lofts, stable lofts and tavern woodstores;
- multigenerational families;
- widowers and single fathers as well as widows;
- pregnant people, nursing parents and caregivers;
- retired craftspeople and masters whose children now run the shop;
- recent rural migrants, foreign residents and noncitizens;
- temporary boat crews, pilgrims, rural market visitors and hired soldiers;
- prisoners, ex-prisoners, debtors under surety and families supporting them;
- ordinary children playing, fetching water, minding siblings, running errands,
  helping at stalls or stealing small things.

Suggested final age shape for the interactive sample:

| Age | Target |
|---|---:|
| 0-7 | 25 |
| 8-11 | 25 |
| 12-15 | 25 |
| 16-19 | 55 |
| 20-39 | 180 |
| 40-59 | 125 |
| 60+ | 65 |
| **Total** | **500** |

Aim for approximately equal numbers of women and men overall. Do not achieve
this merely by assigning women to domestic service and poverty: women should
also appear throughout trade, ownership, food, boats, markets, medicine,
crime, civic households and skilled work, but... you know, be somewhat
realistic.

Keep masters, mistresses and wardens to roughly 50-65 of the final 500. There
are already 26. Most new craft characters should therefore be apprentices,
journeymen, wage workers, relatives helping in a shop or people outside formal
guild rank.

## Civic government and defence prerequisite

Before naming the seven `civic_officer` characters and the senior court,
bailiff and militia characters, write a short canon brief (to lore/core_lore) defining Ombreval's
secular constitution. It need only settle:

- who makes civic rules;
- who executes them;
- who controls taxation and expenditure;
- the ward structure, if wards are political rather than merely planning
  regions;
- the court hierarchy and relation to the Candor's court;
- who keeps the gaol;
- who commands the militia and walls;
- how offices are selected and how guilds influence them;
- which offices are paid, temporary, hereditary or purchased.

Do not casually import a mayor, doge, podesta, aldermanry or feudal lord before
this decision. Neutral working titles can be used in the planning table, but
the final character titles must come from Ombreval's chosen constitution.

## Geographic distribution

Use the eight planning wards in `lore/places/00_city_plan.md` as soft final
targets. `district` can remain a more specific place name, while every sheet's
spawn transform is validated spatially against its intended ward and area.

| Planning ward | Soft target |
|---|---:|
| Fabric Ward | 80 |
| Wick Ward | 65 |
| Cloth Ward | 55 |
| Wallwright Ward | 65 |
| Cinder Ward | 55 |
| Weigh Ward | 65 |
| Reed Ward | 65 |
| Bell and Sluice Wards | 50 |
| **Total** | **500** |

Implementation follow-up after city-scale playtesting: those soft counts made
the compact named-ward rectangles crowded while leaving most of the roughly
one-square-kilometre city empty. Spawn allocation now follows each ward's share
of the full safe city footprint: Fabric 42, Wick 40, Cloth 43, Wallwright 31,
Cinder 37, Weigh 74, Reed 64, and Bell-and-Sluice 169. The allocator enforces
at most three NPCs in any actor's 20 m neighbourhood and at most ten in every
sliding 100 x 100 m window.

These are not social ghettos. Every ward needs some combination of households,
food, children, servants, poor residents, petty trade and maintenance work.
Named trade districts should influence weighting, not monopolise an occupation.

Important placement principles:

- Put domestic servants near their households, on errands and at wells and
  markets, not in a separate servant district.
- Put casual workers at hiring places, gates, yards, wharves and market edges.
- Put animal workers and cart repair near gates, freight routes and stables.
- Put sanitation workers throughout the city, with collection routes toward
  gates rather than clustering them decoratively in one poor lane.
- Put pilgrims and strangers along gate-to-lodging-to-Gradine routes.
- Populate the far north-east and west residential/food-storage margins named
  in the city plan; do not leave all non-famous streets empty.
- Keep polluting fires and kilns in plausible edge, yard or fire-conscious
  locations.
- Avoid stacking many static actors on the same spawn point or blocking narrow
  routes such as the Needle.

## Ambient population authoring rules

Ambient characters should be written in batches by ward, household or work
crew, not generated as isolated occupation lists. Each batch should share real
material context without sharing identical personalities.

Good ambient variation includes:

- different relationships to the same employer;
- different levels of competence and tiredness;
- residents, migrants and commuters;
- honest, opportunistic and quietly rule-breaking workers;
- people content with their place and people trying to leave it;
- different speech lengths, confidence, humour and suspicion;
- small rivalries over pitches, customers, beds, tools and turns at a well;
- ordinary affection and irritation within households.

Avoid:

- a unique murder, prophecy, lost inheritance or conspiracy for every person;
- making every ambient NPC eager to explain their complete biography;
- copying one beggar, servant or porter description with changed names;
- assigning faction roles just to make a person interesting;
- connecting ambient people individually to the Green Sun plot;
- presenting social misery as decorative grotesquerie;
- making all children thieves, all sex workers victims, all migrants smugglers
  or all disabled people beggars.

## Runtime and compute policy

Do not simply change the cast count to 500 while all actors retain the current
round-robin cognition frequency. Significance-aware scheduling is a prerequisite
for enabling the full population outside fake/testing mode.

Suggested policy:

- `major`: bigger reasoning budget; normal autonomous turns; richest memory and
  future quest facilities.
- `minor`: lower reasoning budget; autonomous turns at a lower cadence or
  when locally relevant.
- `ambient`: no expensive background cognition by default; react when spoken
  to, perceived by the player, affected by a nearby event or explicitly
  activated. Use shorter prompt/completion budgets and a lower idle cadence.
- Any tier: recent player engagement temporarily raises scheduling priority.
- Save data preserves encountered ambient identities and memories. <- not implemented yet!

The visible ECS actor, collision, animation, prompt construction, snapshot
projection and speech systems all work with 500 loaded characters, no need
to profile it (it has been verified already!).
Three deterministic voices and one generic appearance may become conspicuously
repetitive at this scale; more voice and appearance variety is desirable but is
not a blocker for the character-data feature.

## Knowledge and introduction

The current loader/tests assume that the player initially knows all 103 NPCs.
Do not blindly expand that to all 500.

Implement:

* The player knows public major/minor figures by reputation but not ambient
   names.

An ambient character should normally begin as an unknown person and acquire a known name through conversation.
Preserve a debug option for headless tests or developer play if knowing everybody remains
useful there.

## Generation workflow

### Phase 1: data model and canon decisions

1. Add `significance` to the character schema and Rust loader.
2. Add `statuses`, or explicitly decide to keep mixed statuses in `conditions`.
3. Decide nullable occupation/title handling for the ten no-fixed-trade people.
4. Write the short secular government brief.
5. Decide the legal treatment of sex work, public gaming and bathhouses without
   overbuilding their lore.
6. Add and validate the 17 new occupation families and alternative titles.

### Phase 2: audit the existing 103

1. Classify every existing character as major, minor or ambient based on canon
   references, not description length.
2. Record all external references to each major/minor character.
3. Preserve existing ids and authored transforms.
4. Do not shorten current profiles merely to make their word counts match new
   targets.

### Phase 3: population skeleton

1. Allocate the exact occupation totals.
2. Allocate final significance totals.
3. Allocate age, gender, rank, status, crime and migration overlays.
4. Build households, workshops, crews, lodging houses and street cohorts.
5. Place those groups across wards and specific areas.
6. Reserve major/minor slots for civic and social anchors before writing
   ambient individuals.

### Phase 4: write in bounded batches

Generate and review characters by household/workplace/ward batch. A useful
batch is roughly 10-25 characters sharing a material context. For each batch:

1. create ids and names using the naming rules;
2. create valid occupation titles and ranks;
3. create relationships with existing ids only where permitted by significance;
4. write profiles to the appropriate depth;
5. author and validate spawn transforms;
6. run data validation before starting the next batch;
7. sample conversations from several ambient characters to detect cloning.

### Phase 5: runtime integration

1. Implement significance-aware scheduling and prompt budgets.
2. Implement player introduction/knowledge policy.
3. Load and project all 500 profiles.
4. Exercise the headless fake backend with mixed significance actors.
5. Profile a full Bevy run and inspect crowd placement in every ward.

## Validation

Add automated checks for:

- exactly 500 character JSON files;
- unique five-character ids and unique file paths;
- folder/occupation/title consistency, including the no-fixed-trade exception;
- valid `significance` and controlled status values;
- every relationship, parent and child id resolving;
- no ambient id or unique ambient name appearing in canonical lore outside its
  own sheet;
- no major/minor sheet naming an ambient character in relationships, parentage,
  backstory, quest, unique item or canonical event;
- final significance totals;
- occupation addition totals;
- age, gender and rank distributions staying within accepted tolerances;
- target ranges for poverty, begging, housing insecurity and conventional
  illegal activity;
- every planning ward receiving its intended population;
- valid transforms, finite facing values and no obvious spawn clusters;
- held item ids resolving into `assets/world/seed.json`;
- prompt and snapshot construction remaining bounded with 500 profiles.

Search the repository for hard-coded assumptions before considering the
feature complete:

```sh
rg -n '\b103\b|103-NPC|103 NPC' . \
  --glob '!target/**' \
  --glob '!logs/**' \
  --glob '!lore/wip_lore_please_ignore_this_is_NOT_canon/**'
```

Known count assumptions currently include tests in
`crates/cathedral-backends/src/world_data.rs`, a test in
`src/smart_actors/mod.rs`, and documentation in
`crates/cathedral-sim/AGENTS.md`.

## Acceptance criteria

- The repository contains exactly 500 valid named character sheets.
- The original 103 remain present unless separately approved by a lore audit.
- The final cast is approximately 30 major, 120 minor and 350 ambient.
- Every character has explicit significance.
- Ambient characters are absent from external canon and stable-character
  dependencies, but remain individually conversable and non-generic.
- The occupation arithmetic totals exactly 500 and domestic service, ordinary
  labour, food, household work, sanitation, civic life and defence are visibly
  represented.
- The final cast meets the poverty, begging, housing, crime, age, gender, rank,
  migration and geographic targets above.
- Begging, homelessness, widowhood, orphanhood and imprisonment are represented
  as statuses rather than flattened into trades.
- The player does not begin by personally knowing all 500 names unless a debug
  mode explicitly requests it.
- Ambient actors do not consume the same idle cognition budget as major actors.
- The fake headless simulation runs successfully with 500 loaded characters.
- The Bevy game loads the full roster without invalid transforms, severe spawn
  overlap or unacceptable frame/simulation cost.
- All hard-coded 103-character assumptions are removed or updated to 500.
