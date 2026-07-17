# Items described in the lore

*A structural/meta document: an inventory of every portable physical thing the canon lore
mentions that could plausibly become an item kind. Compiled 2026-07-17 from `core_lore/`,
`families/`, `second_sun/` (including the documents, design notes and index.html),
`places/`, the root chronicles (`wells_and_water.md`, `founding_story_the_four_hundred_marks.md`,
`the_dry_boatmen.md`, `the_great_rains_and_the_hammering.md`), the inspiration-image prompts,
and all 516 character bios. `wip_lore_please_ignore_this_is_NOT_canon/` was excluded.*

**How this relates to the item system.** The current catalog
(`features/food_and_items/README.md` §5) is deliberately tiny: `spark`, `loaf`, `herring`,
`smoked_eel`, `stew`. This document is the quarry the catalog can grow from — nothing here is
implemented or a request to implement; per the spec, everything the market merely *talks about*
stays talk until a feature needs it.

**Structure:**
- **Part 1 — the raw sweep.** Everything, grouped by category (~240 entries). Duplicates across
  lore files are merged, but variants are kept visible.
- **Part 2 — the combining pass.** Rules for folding variants together, and the resulting
  canonical list (~95 kinds), each noting what it absorbs.
- **Part 3 — ranked by gameplay relevance.** Tiers; Tier 0–2 is roughly the ~50 worth ever
  implementing.
- **Appendix — deliberately not items** (vehicles, animals, furniture, fixed gear, great bells).

Source tags are abbreviated: a specific file where one file owns the item, or a broad tag
(`families`, `second_sun`, `chars: baker`, `wells`, `founding`, `everywhere`) where it is pervasive.

---

## Part 1 — the raw sweep

### Food & drink

- **loaf of bread** — the two-spark staple; variants: heel, crust, half-loaf, alms/dole bread, winter loaf, twice-baked rusk (everywhere)
- **dough** — brought to the common oven; the oven-keeper's toll is "a fist from every batch" (chars: baker)
- **flour** — rye daily, wheat fine; feuding mothers "do not lend each other flour" (chars: baker, second_sun/08)
- **grain / corn** — rye the bread grain; sampled and stored at Seven Lofts; seed-corn (places/03, second_sun/04)
- **malt** — the malt-house above Malt Passage; the price of malt (families, places/03)
- **barm** — brewing yeast the brewers sell to bakers (chars: brewer)
- **ale** — small ale, pot of ale; bench-fare, tavern tabs, watered by cheats (everywhere)
- **wort** — brewing liquor drawn from the copper (family_rasp.md)
- **milk** — the milk-seller's can and short measure (family_fitch.md, chars: food_provisioner)
- **egg** — provisioner's ware (chars: food_provisioner)
- **cheese** — the cheesemonger; salt "entered cheese and curing" (chars: food_provisioner, founding)
- **honey / honeycomb** — Wickmarket signature ("hot fat and honey on the air"); the Skep comb (second_sun/10, family_vell.md)
- **apple** — "the last bruised apple", a stock street-vendor image across dozens of bios (chars: everywhere)
- **fruit** — orchard produce lost in the Hammering (the_great_rains)
- **onion** — Ede Pennyhand "sold onions" in the founding tale (founding)
- **cress** — Annet sold cress by the Gradine steps (second_sun/heretic_catechism)
- **herbs / simples** — Mother Gude's reedmace for the chest, bittercress for the blood (second_sun/05, chars: healer)
- **spice** — grocer-and-spicer stock kept under seal (chars: grocer_and_spicer)
- **cooking oil** — in jars, disputed as watered (chars: grocer_and_spicer)
- **stew / pottage / broth / soup** — the shared household pot; the Hungry Ox's never-scraped pot (everywhere)
- **hot pie** — fried and sold off a cook's board (chars: cook, butcher)
- **dripping / fried fat** — sold off the same board (chars: cook)
- **meat** — butchered at the Shambles; salted for the winter store; brisket at the ox-roast (places/03, families, second_sun/08)
- **roast ox** — an ox roasted whole on the frozen Cut, the great-frost story (second_sun/04)
- **marrow bone** — the priest's portion at the sturgeon dinner (second_sun/08)
- **hare** — poached game (family_pell.md)
- **fish (fresh)** — the slab at Maren's Green (everywhere)
- **herring** — bench-fare and its unit of measure (core_lore, second_sun/07)
- **eel / smoked eel** — trapped, strung, smoked in the sheds, sold at the eel fair; a courting gift (everywhere)
- **sturgeon + roe** — the eleven-foot fish of F.271; the boatmen quietly kept the roe (second_sun/04, 08)
- **salt** — pan-salt from Salorge; the Ashe trade; coarse/smoking/curing grades; the funeral salt-dish (everywhere)
- **water** — bought by the bucket in a dry city; cold, chalky or bitter by ward (wells)

### Money & tokens

- **spark** — the copper coin; the mouth says "penny", the coin says spark; "piss-penny" the slang smallest (everywhere)
- **bell** — the silver coin, twelve sparks (historical under the spark standard — see Part 3) (everywhere)
- **lantern** — the gold Ostrelle coin, mostly money of account (historical) (core_lore, second_sun/11)
- **mark** — the founding's unit: four hundred marks for the peace of Harne (founding)
- **silver / plate / bullion** — weighed for the ransom; sacred plate laid down (founding)
- **pennyhand token** — wood/lead tokens marked with hand, dots, salt sack or chain (founding)
- **weighday badge** — ward tokens inherited as receipts (founding)
- **lead token** — a low denomination Tib the market-seller counts (chars: market_seller)
- **dry money** — laundered informer pay through the Tally Bridge pawnshop (a *provenance*, not a coin) (second_sun/00, 03)
- **tally stick** — notched, split and halved; debt and count records; burned at Gaudry's feast (founding, chars)
- **bond** — debt-bonds burned with the tally-sticks (second_sun/lives_of_the_saints)
- **dice / gaming piece** — licensed stakes; rigged or marked pieces an offence (core_lore/secular_government, chars)
- **pledge** — any pawned thing at Lise Copp's counter (a *state*, not a kind) (core_lore, second_sun)

### Light & fire

- **candle (wax)** — the chandlers' chief ware; altar and workshop light (everywhere)
- **tallow dip** — the poor man's smoky candle (families)
- **taper** — great tapers and Paschal-work for the Concurrence; the oath taper (family_vell.md, trial_records)
- **rushlight** — sold three for a spark (chars: market_seller)
- **candle-stub** — counted, raked from grates, kept in boxes as keepsakes (families)
- **green-dipped candle** — burned for drowned Colm on Colm's Night; verdigris-tinted (everywhere)
- **votive wick** — left on Ford Well's dry ledge (wells)
- **wick / wicking** — the Wickmarket's namesake coils (core_lore, second_sun/10)
- **lamp** — street lamps, Belwyn's lamp, a funeral lamp (everywhere)
- **lantern (carried)** — fair-lanterns strung on rings at Marenstide (families)
- **lamp oil** — "bought before dusk", a recurring errand (chars: everywhere)
- **taper-can** — the lamplighter's can carried up the western round (family_vell.md, chars: lamplighter)
- **horn box** — the lamplighter's coal carried in horn (second_sun/08)
- **coal / charcoal** — lamp-coal; filtering charcoal (second_sun/08, wells)
- **firewood / kindling** — bundles gathered, kept dry, bought dear in winter (chars: everywhere)

### Tools & implements

- **knife** — butcher's honed set in oiled cloth; borrowed and returned; the glazier's blade (chars, second_sun)
- **key / ring of keys** — sexton's ring, sluice keys, "three keys at the belt", copied pantry keys (everywhere)
- **lock** — made, read when forced, bound onto coffers (family_vell.md, founding)
- **file** — for filing a copied key; its marks readable on a forced lock (chars: fine_metalworker)
- **rope / cord** — spliced, whipped and tarred at the Moorings; the hangman's oiled rope; well rope (everywhere)
- **sounding line / sounding pole** — knotted line for well depth; cistern pole (wells)
- **net** — fishing net; Ede mended nets in the founding tale (founding)
- **eel-trap** — set on the river, poled beyond the south wall (family_alder.md)
- **gaff** — Dunstan Alder hooked Colm's body out of the sluice with his own gaff (family_alder.md)
- **boat-pole / fore-pole** — the boatman's pole; laid on the coffin, hung over the door (the_dry_boatmen, families)
- **oar** — the boat-families' donor device (second_sun/09)
- **bucket / pail** — water bought by the bucket; fire buckets; iron-bound public buckets (wells, everywhere)
- **yoke** — shoulder yoke bearing two pails (wells)
- **dipper / ladle** — the clean transfer dipper; the Bell and Ladle's namesake (wells, second_sun/05)
- **mop / swab / broom** — the nave sweeper's broom; work implements barred from the clean curb (wells, chars)
- **rake** — candle-stub rake, salt rake, gutter rake (families, chars)
- **sieve** — for lime and yard chips (chars: general_labourer)
- **spade / mattock / shovel** — grave-digging; the sexton's silver grave-spade (family_fitch.md, chars)
- **hod** — for carrying mortar (chars: general_labourer)
- **ladder** — the lamplighter's, the fire watch's, Colm's in the skipping verse (everywhere)
- **fire-hook** — Dunstan Hook's, carried to every alarm; demolition kit (core_lore/ward_politics, places/03)
- **stock-hammer / mallet** — the fuller's hammer; fullers' mallets on Tenterhook Lane (family_rud.md, places)
- **chisel** — mason's and smith's, re-steeled at the fire (chars: mason, smith)
- **saw** — the carpenter apprentice's (family_marle.md)
- **axe** — re-steeled at the smith's fire (chars: smith)
- **punch / lewis-iron** — smith's yard tools (chars: smith)
- **nail** — driven fast and mostly true (chars: carpenter_and_builder)
- **trowel** — Saint Coswald's gilded relic trowel, lodge-kept (second_sun/09, lives_of_the_saints)
- **plumb-line / level / square / template** — masons' and Ferrant's measuring kit (second_sun, places/02)
- **needle / palm-and-needle** — the shroud's last hem; canvas repair at the Moorings (families)
- **shears** — the cloth bench's cutting shears (family_marle.md)
- **baker's peel** — kept on the peel-rack the lame oven-keeper runs from his stool (chars: baker)
- **quern** — the poor household's hand-mill (family_quern.md)
- **dipping-rod** — the chandler's rod, held by every hand in the Melt Yard (family_vell.md)
- **mash-paddle** — the brewer's paddle bearing Wray's mark (second_sun/09)
- **scythe** — the haymaking panel (second_sun/09)
- **balance / scales** — the pawnshop's brass scale, the proved beam, the milk-scale (families, chars)
- **sealed weight** — the brass Gaudry standards; the chandlers' heavy pound; false weights in fraud (everywhere)
- **ell rod** — the draper's measuring rod (the public iron ell is a fixture) (chars: draper, second_sun/11)
- **pen / quill** — the scrivener's good pen; quill-and-honest-weight the scriveners' device (everywhere)
- **ink / oak-gall** — forger's aged inks; ink-stained thumbs (second_sun/05, chars)
- **chalk** — charnel-door names, the Needle diamond, prices by the door, tally strokes (everywhere)
- **slate** — the debt slate hung by the door; the moth's funeral-name slate (chars, second_sun)
- **pencil** — clerks' annotations on the Tallage edict board (second_sun/edict)
- **wax tablet** — Ferrant's pane-position records (second_sun/01)
- **seal / stamp** — the closed-eye Custody seal, guild seals, sealing wax, an impressed clay seal (everywhere)
- **lens** — ground finer than any in the city; grey-funded (second_sun, chars: instrument_maker)
- **quadrant / sighting-rod / string-grid** — Ferrant's measuring apparatus (second_sun/01, letters)
- **dark box** — the pinhole chamber that draws the nave on a white leaf; the sailcloth tent version (second_sun/letters)
- **smoked glass / coloured slip** — sun-viewing aids (second_sun/01, letters)
- **mirror** — carried out the west doors face-down; the barber's mirror on a pike (second_sun/00, 08)
- **glazier's diamond** — the cutting lozenge, also Ede's chalked meeting-sign (second_sun, places/02)
- **grozing-iron / soldering iron / glass-painter's brush / emery** — the glazing bench kit (second_sun/09, chars: glazier)
- **crucible** — the year's one melt standing in it (second_sun/lives_of_the_saints)
- **bandage / splint** — "two clean bandages"; splints tied from kindling (chars: healer, everywhere)
- **physic / bottle of Bitter water** — quack medicine sold in small bottles (wells, second_sun/letters)
- **cudgel** — the watch's (chars: watchman_and_keeper)
- **spearhead / pike** — muster spearheads; the mirror-bearing pike (chars: militia, second_sun/letters)
- **sword** — the besiegers' swords in the Coswald panel; gate-armoury arms (second_sun/09, core_lore)
- **armour** — the Ashe and fine-metal armourers' trade (family_ashe.md, chars)
- **branding iron / tongs** — the Edict's penalty implements (second_sun/edict)
- **handbell** — Jos Brant's crier bell; cast hand-bells at the founders' yard (second_sun/design, chars: bellfounder)
- **reed whistle** — the lamplighters' three-note wick-call; stuck in many sleeves (second_sun/design, chars)
- **lute** — hired for courting; the entertainer's (chars: entertainer, fish_trader)
- **staff / walking stick** — Belwyn's hidden relic staff; the lame tanner's stick (second_sun/lives, chars)
- **horn spoon** — tucked through the belt, a signature bio detail (chars: everywhere)

### Clothing & wearables

- **coat** — the only coat, the oversized coat belted twice, the pot's one good coat (chars: everywhere)
- **grey coat** — the Custody clerks' uniform; "grey keeps the rain off" (second_sun, families)
- **cloak** — steaming by the Hungry Ox hearth; a child's cloak pledged against the frost (places, family_copp.md)
- **rain hood** — at the anchorhold alms shelf (places/02)
- **hat / cap** — the broad hat with half its brim lost; a cap pinned with a goose feather (chars: everywhere)
- **gloves** — mismatched woollen gloves (chars: everywhere)
- **boots / shoes** — polished only across the toes; a leaking shoe; one new wooden sole (chars: everywhere)
- **hose** — the hosier's stockings (chars: garment_worker)
- **shirt / shift / sark** — piecework garments; the Sark name itself (family_sark.md, chars)
- **smock** — butchers' holiday smocks at the frost-feast (second_sun/08)
- **apron** — bearing three square repairs, another signature detail (chars: everywhere)
- **belt** — a horn spoon through it, tied with bright sail thread (chars: everywhere)
- **buckle / clasp** — a bent buckle to straighten; the good neck-clasp a button replaced (chars: everywhere)
- **brass button** — worn where a clasp used to be, the mark of mending (families, chars)
- **wooden button** — dropped and sought (chars: pilgrim)
- **green bead** — a chipped bead hung at the throat (chars: everywhere)
- **blue cord loop** — cheap blue in a child's hair, buried with the dead; holds hair back (family_rud.md, chars)
- **nutshell string** — circling one wrist (chars: everywhere)
- **goose feather** — pinning a cap (chars: everywhere)
- **clay bird** — a tiny keepsake in a pocket (chars: everywhere)
- **green thread** — sewn in the hem by Saint Maren's widows for a mourning-year (second_sun/01)
- **bone skates** — from the Frost of F.187 (second_sun/09)
- **watch badge** — worn a working lifetime (family_rud.md)
- **ring / silver buckle** — given among the founding's household pennies (founding)
- **vestments / Taper's habit / wick-priest's collar** — church dress; burned in the F.341 sacristy fire (second_sun/04, family_marle.md)
- **altar-linen** — mended under the vergers' eye (family_marle.md, chars: cloth_worker)
- **shroud / winding-sheet** — its last hem sewn by the family needle, which is then broken (families)

### Documents & books

- **ledger / day-book** — every trade keeps one; false ledgers, paired true/false books (everywhere)
- **named civic books** — the Long Book, the Day Ledger, the Line Books, fee book, rent book, guild book, cloth book, tally-book (second_sun, families, core_lore)
- **roll / register** — roll of the drowned, Great Roll, Hammering rolls, pauper-roll, muster/watch/fire-watch rolls, parish register, bell-register, acquaintance rolls, bede-roll (everywhere)
- **letter** — Dunstan's letter that came once or never; Ferrant's correspondence; the Luminary's letter; letters written for hire (everywhere)
- **the Sparr deposition / a Sparr page** — the sealed F.204 paper; a single leaf the city's costliest contraband (second_sun)
- **Colm's Last Letter** — the cell's revered relic, secretly Corin Copp's forgery (second_sun)
- **the Letters on the Doubled Shadow** — the pawned bundle of six philosophers' letters (second_sun/documents)
- **heretic catechism ("the Told Book")** — the forbidden written copy hidden in a choir-book's boards (second_sun)
- **sermon leaf** — the stolen Grey Press sermon with its margins (second_sun/documents)
- **trial record leaves** — the sealed F.288 glaziers' trial; the masons' bill of accusation; the sealed verdict (second_sun/documents)
- **Idonea's recipe-margin leaf** — "cool the green under the open sky", in her own hand (second_sun/00, 05)
- **Attestation leaf** — each new Praelucent's signed witness in the Grey Press (second_sun/03)
- **almanac (official)** — the computed feast-calendar the heretics' figures beat in F.431 (second_sun/04)
- **the Green Almanac** — the cell's embroidered cloth sky-map (second_sun)
- **sky-charts / shadow tables** — the Custody's secret charts; Ferrant's tabulations (second_sun/03, letters)
- **the soundings** — the boat-families' *unwritten* map of the old riverbed, quietly bought (places/02)
- **deed / grant / lease / will / title** — old deeds, the Crake deed-claim in a strongbox (everywhere)
- **charter** — guild charters in locked chests; the lodge's founding charter with the "close the light above" clause (everywhere)
- **contract / articles / indenture** — apprentice articles sworn on the lead; the thirty-year candle contract (everywhere)
- **licence** — Custody glass-work licences, market-stall and pilgrim-lodging licences, a registered house's licence (second_sun, chars)
- **warrant / summons / petition / water order / pleadings / acquittance / recantation formula** — Bench and Custody legal papers (core_lore, wells, families)
- **manifest / bill of lading / bill of sale / toll receipt** — customs-square paper, falsified to understate loads (everywhere)
- **edict / proclamation / posted notice** — cried and nailed at the Bellstand and the five squares (everywhere)
- **informant chit / watch-list / grey notebook / mark list / fair copy** — the Custody's paper ecology (second_sun/03, families)
- **sealed packet** — dry money and archive packets across Lise Copp's counter (second_sun, family_copp.md)
- **book (bound)** — choir-book, the Lesser Legendary, the verger's guide to the rose, the rumored "sorcery-book" (second_sun)
- **verses** — the anchoress trades verses for honest news; the written Glazier's Vespers (core_lore, chars: anchoress)
- **pattern-sheet / cartoon** — glaziers' full-size drawings; the locked page beneath (family_sparr.md, second_sun/09)
- **embroidered sky-record** — Betriss Marle's daily pane-and-colour stitching in plain sight (second_sun/00, 05)
- **knotted cord record** — the cell's forbidden knot-writing ("no chalk, no wax, no knots") (second_sun/02)
- **blank paper / parchment / vellum** — end-leaves and spoiled sheets smuggled for silver; "old paper is easy" (second_sun, chars: custody_clerk)

### Religious & funerary

- **pilgrim badge** — the licensed stamped lead image of the Emblem; sold on the Gradine, counterfeited, worn on a pin (everywhere)
- **Emblem token** — image/badge/token stampable only under Chapter licence (second_sun/edict)
- **censer** — the swinging censer that started the F.341 sacristy fire (second_sun/04)
- **alms box / alms-bowl** — Saint Maren's box; the anchoress's bowl (second_sun/08, chars: anchoress)
- **reliquary** — a saint's knuckle in a jar; the bronze case; the (falsely) broken reliquary of the founding (second_sun, founding)
- **named relics** — Vhairé's ford lamp in its iron cage, Perrin's tuning bone, Belwyn's staff, Maren's reed-crown, Coswald's trowel, Ambrelle's gauntlet-cloth, Gaudry's unburned leaf (second_sun/lives_of_the_saints)
- **the Harne link** — the black chain link kept as civic relic, shown beside the salt bowl at Weighday (founding, family_ashe.md)
- **bell filings** — scraped from a christened bell into a deaf ear, folk remedy (second_sun/lives)
- **cast reed** — a reed cast on water to seek a lost thing (second_sun/lives)
- **coffin** — carried up Maren's Slip under the dead man's pole (everywhere)
- **grave-crock** — thrown by a Fitch potter a lane from the grave (family_fitch.md)
- **gravestone / grave-marker** — a stone most Fitch dead cannot afford; a yard offcut at a Marle grave-head (families)
- **memorial boat-pole** — the blackened row over the boat-family door, the house's record (family_alder.md, the_dry_boatmen)
- **bowl of salt** — set beside the open account at Weighday and in the coffin (family_ashe.md)

### Household & containers

- **cook-pot** — the shared pot all eat from; scoured before sleep; a mother's pot in pawn (everywhere)
- **kettle** — the copper kettle, the archetypal pawned good (family_copp.md)
- **cauldron / copper** — Wickmarket fat-cauldrons; wash- and brew-coppers (places, families)
- **bowl / dish** — the supper bowl; the white-glazed well-testing bowl (everywhere)
- **cup** — a cracked cup replaced; a dead bride's wedding cup (everywhere)
- **jug / jar** — watered by cheats; a saint's knuckle in a jar; oil jars (everywhere)
- **bottle** — small bottles of Bitter water sold as medicine (wells)
- **salver / wedding pewter / silver spoon** — pledged finery; the Tally Bridge benchmark old paper outsells (family_copp.md, second_sun/04)
- **chamber pot** — end use of wash water (wells)
- **barrel / cask / tun / keg / butt / tub** — cooperage in every trade; the Serle tun as cargo measure (everywhere)
- **hoop / stave** — cooper's stock; warped hoops, salvaged hoops (chars: cooper, scavenger)
- **sack / sarpler** — salt sacks, grain sacks, a wool sarpler packed with sand (everywhere)
- **basket / skep** — market baskets (broken-handled, striped-lined); the straw skep (everywhere)
- **crate / bale / bundle / box** — freight at the Tallage and the Moorings (places, founding)
- **chest / strongbox / coffer** — locked charter chests, the deed strongbox, the joint-locked Common Chest (everywhere)
- **iron box** — holds the parish register (family_fitch.md)
- **satchel** — the leaked bag of Custody archive copies, F.433 (second_sun/00, 04)
- **purse** — the quiet purse that buries the poor; the cell's three dues-purses; Averil's empty one (everywhere)
- **begging cup** — a poor child's, at the badge-seller's elbow (family_rasp.md)
- **blanket / bedding / straw** — dried before curfew; carried into the gaol with food (everywhere)
- **rushes** — strewn on the tavern floor (places/02)
- **mirror** — (also under tools) the folk test that never doubles the light (second_sun)
- **rain-barrel** — the vessel the disc's reflection never appears in (second_sun/00)
- **can / covered vessel** — the milk can; funerary washing-water (family_fitch.md)

### Materials & trade goods

- **wool / fleece** — down from Brede, bought off the clip, piled to the gunwales (everywhere)
- **cloth / broadcloth / bolt** — fulled, tentered, measured against the ell, run short at the list (everywhere)
- **linen** — household and altar linen; a household "known by its linen" (family_marle.md)
- **canvas / sailcloth / duck** — repair bolts at the Moorings; the dark-chamber tent; rose-repair sheeting (families, second_sun)
- **thread** — a debt of thread; Marle embroidery recording faces (family_marle.md, second_sun)
- **striped cloth** — lining baskets and belts (chars)
- **rag** — the rag-picker's salvage, sold to paper and stuffing (places/03, families)
- **dyestuff** — woad, madder, cheap blue, verdigris, red ochre/raddle (chars: cloth_worker, family_rud.md)
- **fulling chemicals** — fuller's earth, hot lye, stale urine bought by the pot (family_rud.md)
- **soap** — takes poorly to hard water (wells)
- **hide / pelt** — hauled wet from the Shambles; bundled pelts at Skinners' Court (everywhere)
- **leather** — worked at Tanners' Slip; the keeper's bearing-guard (everywhere)
- **tan-bark / bark tannin** — ground bark in rough sacks (places/02, chars: leather_worker)
- **wax / beeswax** — bought in the block, bleached and cast; the Vell name's anchor (everywhere)
- **tallow / suet / grease** — beef and mutton suet boiled to dips; axle grease kept off the water (everywhere)
- **pitch / tar** — a keg of pitch; tarred rope and the tar-canvas roof (chars, family_alder.md)
- **timber / oak / fir / scaffold pole** — long timber, reused roof timber, the wood-hauling month (everywhere)
- **shingle / thatch / reed / straw** — cheap roofing; paupers' reed thatch (families, second_sun/lives)
- **stone / limestone / rubble** — dressed and rough at Coswald's Yard (everywhere)
- **lime / mortar** — burned at the kilns; the green night-mortar of the saints' story (everywhere)
- **sand** — Serle reed-shoal sand for the melt; fire-smothering sand in tubs (second_sun, wells)
- **clay** — potter's clay; impressed sealing clay; bellfounding loam (everywhere)
- **tile / slate** — roofing broken in the Hammering; "you cannot drown under slate" (everywhere)
- **glass pane** — rose panes, quarries, lozenges, a cracked cull, replica panes (everywhere)
- **cullet / offcut** — scrap glass sorted for remelt; trimmings the clerk counts (family_sparr.md, places/02)
- **coloured glass / colouring metals** — copper-green, iron-green, "the metals the craft appoints" (second_sun)
- **putty** — glazing putty skinning over a reset pane (second_sun/sermon)
- **lead / came** — the H-sectioned strip; the house-mark cut once into a master's came (everywhere)
- **iron / steel / ironwork** — southern iron; tie-bars, cramps, great hinges; re-steeled edges (everywhere)
- **bronze / bell-metal** — cast and re-melted at the founders' yard (chars: bellfounder, second_sun)
- **brass** — the sealed Gaudry weights; Vhairé's river-brass (second_sun/11)
- **pewter** — the pewterer's stock; pledged wedding pewter (chars: fine_metalworker)
- **gold-leaf / pigment** — the draper's selvedge in true gold; linseed and lead-white (second_sun/09, chars: painter)
- **hemp** — raw fibre for the rope-walks (chars: roper, wells)
- **bone / horn** — worked and sold from the Shambles; horn spoons, bone skates (places/03)
- **ash** — sold to trades; beech and fern ash flux for the greens (everywhere)
- **fuel (generic)** — dear in the cold months; the furnace fuel stored apart (everywhere)
- **salvage / scrap** — the scavenger's gather-and-sell; a sodden bale off the river (families, chars)
- **dung / night-soil** — carted outward, sold to gardens; dog-dung to the tanners (places/03, chars)
- **fodder / harness** — for the gate pack-trains (places/03)
- **cartwheel** — straightened before the gate closes (chars: cartwright)
- **lamp fittings / pump-barrels / cast mortars** — founders' and fine-metal wares (places/02, chars: bellfounder, painter)
- **brine** — the tan-pit and salt-pan liquor (family_ashe.md)

### Keepsakes & oddments

- **flowers** — brought in courtship (chars: tavern_worker, fish_trader)
- **painted crest panel** — a house's arms on weathered board, hung over cellar, ledger or door (families/crests)
- **scaffold-blessing panel** — painted, gilded, by mistress panel-painter Osanne Skell (family_copp.md, chars: painter)
- **house-mark came** — a master's mark cut once into lead, kept as the family genealogy (family_sparr.md)
- **the black Harne link** — (also under relics) the chain link shown at Weighday (founding)

---

## Part 2 — the combining pass

The raw sweep proves the point: the lore mentions ~240 distinct things, and most are the same
few dozen things wearing different clothes. Rules used to fold them:

1. **One kind per economic role; variants become metadata.** The catalog already does this
   (`loaf` + `flour: rye|wheat`). So: `candle {fat: tallow|beeswax, dye: green|none}` absorbs
   dip, taper, rushlight, stub and the green Colm's-Night candle.
2. **Container-of-X is X.** A pot of ale is `ale`; a pail of water is `water`; a sack of flour
   is `flour` ×N. Containers only become kinds where the empty container itself trades
   (barrel, basket, sack, chest).
3. **Synonyms and slang fold.** penny/piss-penny → `spark`; pail → `bucket`; cask/tun/keg/butt
   → `barrel`; hawser/cord → `rope`.
4. **Named one-off documents become one kind + metadata.** `contraband_page {title: ...}`
   covers the Sparr leaf, Colm's Last Letter, the catechism, the sermon leaf, trial leaves —
   they price differently by title, not by kind.
5. **Craft tools collapse to a kit.** A peel, a dipping-rod, a mash-paddle and a grozing-iron
   are all "the tools of my trade" — one `craft_tools {craft: baker|chandler|brewer|glazier|…}`
   kind, with only genuinely cross-craft tools (knife, rope, ladder…) kept separate.
6. **The button rule.** The bio-texture trinkets (brass button, green bead, blue cord, nutshell
   string, clay bird, goose feather, reed whistle) are one `keepsake {kind: …}` — priceless to
   one NPC, worthless to the market.
7. **Fixtures, vehicles, animals and furniture are out** (see Appendix).

### Canonical list (~95 kinds)

Format: **canonical** ← what it absorbs. *(sim)* marks the five already in the catalog.

**Food & drink (24)**
- **loaf** *(sim)* ← heel, crust, half-loaf, alms bread, rusk, winter loaf
- **dough** ← the bakehouse round, the oven toll
- **flour** ← (already loaf metadata; as a carried sack it is its own stack)
- **grain** ← rye, corn, bread-corn, seed-corn, malt (metadata `malted`)
- **ale** ← small ale, pot of ale, wort, mash
- **stew** *(sim)* ← pottage, broth, soup, the supper bowl
- **pie** ← dripping/fried-fat snacks
- **herring** *(sim)*
- **smoked_eel** *(sim)* ← fresh eel (the slab sells it smoked or it goes in the pot)
- **meat** ← salted meat, winter store, brisket, roast-ox portions
- **cheese**; **egg**; **milk**; **honey** ← honeycomb
- **apple** ← bruised apple, fruit
- **onion**; **cress**; **spice**; **cooking oil**
- **simples** ← herbs, reedmace, bittercress, physic
- **salt** ← pan/coarse/smoking/curing grades (metadata if ever needed)
- **water** ← the bought bucket/pail of it
- **hare** ← poached game
- *(curio, not a kind: sturgeon, roe, marrow bone — one historical dinner)*

**Money & play (3)**
- **spark** *(sim)* ← penny, piss-penny, copper
- **tally stick** ← bond (the paper version folds into documents)
- **dice** ← gaming piece, marked piece
- *(historical, per the spark standard (02_the_spark_standard.md): bell, lantern, mark,
  pennyhand token, weighday badge, lead token, bullion/plate — chronicle material, not mintable
  kinds)*

**Light & fire (6)**
- **candle** ← wax candle, tallow dip, taper, rushlight, stub, green-dipped candle, votive wick
- **lamp** ← carried lantern, taper-can, horn coal-box
- **lamp oil** ← oil
- **wick** ← wicking coils
- **firewood** ← kindling, the bundle
- **coal** ← charcoal

**Tools & gear (18)**
- **knife** ← butcher's set, glazier's blade, razor
- **key** ← ring of keys, copied key
- **lock** ← (with `file` folded in as the lockpicker's tool)
- **rope** ← cord, hawser, well rope, sounding line
- **bucket** ← pail, fire bucket, yoke-pair
- **ladder**
- **spade** ← mattock, shovel, hod
- **broom** ← mop, swab, rake, sieve
- **hammer** ← stock-hammer, mallet, punch
- **craft_tools {craft}** ← peel, quern, dipping-rod, mash-paddle, scythe, shears, needle,
  saw, chisel, axe, trowel, plumb-line/level/square/template, grozing kit, net, eel-trap, gaff,
  boat-pole, oar, fire-hook, crucible, ell rod, lewis-iron…
- **scales** ← balance, proved beam
- **sealed weight** ← Gaudry weights, chandlers' pound, false weight (metadata `false: true`)
- **writing kit** ← pen/quill, ink, chalk, slate, pencil, wax tablet
- **seal** ← stamp, sealing wax
- **instrument** ← lens, quadrant, sighting-rod, string-grid, dark box, smoked glass, coloured slip
- **mirror**
- **bandage** ← splint
- **weapon {kind}** ← cudgel, spearhead, pike, sword, armour (armour arguably its own)
- *(music: **lute** ← handbell? no — handbell folds into craft_tools (crier); lute stays for the entertainer)*

**Clothing (12)**
- **coat** ← cloak, child's cloak, rain hood; metadata `grey` for the Custody cut
- **shirt** ← shift, sark, smock
- **apron**
- **cap** ← hat, broad hat
- **gloves**
- **shoes** ← boots, wooden sole
- **hose**
- **belt** ← (buckle/clasp are keepsake-tier parts)
- **blanket** ← bedding, straw bed
- **shroud** ← winding-sheet
- **vestments** ← Taper's habit, collar, altar-linen
- **badge {kind: pilgrim|watch|weighday}** ← pilgrim badge (+ `counterfeit` metadata), watch badge, Emblem token

**Documents (11)**
- **ledger {title}** ← every named book, roll and register
- **letter** ← sealed letters of every stripe
- **deed** ← grant, lease, will, title
- **charter** ← guild/lodge charters, articles, indenture, contract
- **licence** ← Custody, market, lodging licences
- **legal paper {kind}** ← warrant, summons, petition, order, pleadings, acquittance, recantation
- **shipping paper {kind}** ← manifest, bill of lading, bill of sale, toll receipt
- **edict sheet** ← proclamation, posted notice
- **contraband_page {title}** ← Sparr leaf, Colm's Last Letter, catechism, sermon leaf, trial
  leaves, Idonea's margin leaf, Green Almanac, sky-charts, embroidered sky-record, knotted cord
- **sealed packet** ← the dry-money/archive packet, the moth's chit
- **blank paper** ← parchment, vellum, leaf; (bound **book {title}** if ever distinct from ledger)

**Religious & funerary (5)**
- **badge** — (above)
- **relic {name}** ← the seven saints' relics, the saint's knuckle, the Harne link, reliquary case
- **censer**
- **coffin** ← (grave-crock, gravestone, memorial pole stay flavor)
- **alms bowl** ← begging cup

**Household (10)**
- **cook-pot** ← kettle, cauldron, stewpot, copper
- **bowl** ← dish
- **cup** ← wedding cup
- **jug** ← jar, bottle, can
- **spoon** ← horn spoon, silver spoon, ladle, dipper (metadata `horn|silver`)
- **pewterware** ← salver, wedding pewter
- **barrel** ← cask, tun, keg, butt, tub, rain-barrel (hoops/staves are cooper's craft_tools stock)
- **sack** ← sarpler
- **basket** ← skep
- **chest** ← strongbox, coffer, box, iron box; **purse**; **satchel** ← bundle, crate, bale

**Materials & trade goods (17)**
- **wool** ← fleece, the clip
- **cloth bolt** ← broadcloth, linen, striped cloth
- **canvas** ← sailcloth, duck
- **thread** ← (with needle in craft_tools)
- **hide** ← pelt
- **leather**
- **wax** (the block) ; **tallow** ← suet, grease
- **timber** ← oak, fir, scaffold pole, shingle
- **stone** ← limestone, rubble
- **lime** ← mortar
- **glass pane** ← cullet, offcut, coloured glass, putty
- **lead came** ← lead
- **iron** ← steel, ironwork, nails
- **dyestuff** ← woad, madder, verdigris, ochre, pigment, gold-leaf
- **rag** ← salvage, scrap
- **clay** ; **sand** ; **ash** ; **tan-bark** ; **hemp** ; **pitch** ← tar; **soap**;
  **bone** ← horn *(this long tail is one-mention bulk; most stays talk forever)*

**Keepsakes & oddments (3)**
- **keepsake {kind}** ← brass button, wooden button, green bead, blue cord, nutshell string,
  clay bird, goose feather, reed whistle, flowers, candle-stub-in-a-box
- **crest panel** ← painted arms, scaffold-blessing panel, house-mark came
- **bottle of Bitter water** ← the quack-medicine bottle

---

## Part 3 — ranked by gameplay relevance

Ranking lens: what does an item *do* in this game? It feeds the hunger/thirst gauges, it crosses
a counter for sparks, it gets gifted or stolen, it gives an LLM actor something concrete to
want, carry, brag about or lie about. Bread and fish clear that bar; a random button doesn't
(which is why the buttons got folded into one `keepsake`). Tiers, most relevant first;
**Tier 0–2 ≈ the ~50 that plausibly ever get implemented.**

### Tier 0 — already in the catalog (5)

`spark`, `loaf`, `herring`, `smoked_eel`, `stew`.

### Tier 1 — the next wave (~22): daily food, drink and light

Everything here hooks straight into systems that already exist or are specced (hunger, thirst,
vendor purchase, the bread round):

1. **ale** — the city's default drink; tavern scenes, bench-fare, tabs. The single biggest gap
   in the current catalog.
2. **water** — thirst already has a gauge; `wells_and_water.md` is 14k words of lore begging
   for a "pail of water" item.
3. **candle** — the most lore-central artifact class after bread (Wickmarket, chandlers,
   Colm's Night green dip as metadata); also the cheapest way to let NPCs *hold light*.
4. **apple** — the recurring street-vendor beat ("the last bruised apple") across dozens of bios.
5. **cheese** — provisioner staple.
6. **egg** — provisioner staple.
7. **milk** — the milk-seller's can and her short measure are a ready-made scene.
8. **pie** — hot hand-food off the cook's board; the street-food counterpart to tavern stew.
9. **meat** — Shambles/butcher trade; `salted` metadata covers the winter store.
10. **honey** — Wickmarket signature ware; courting gift.
11. **salt** — half the city's economy and ritual (the funeral dish, the Weighday bowl); as an
    item it's a pinch/sack vendors sell.
12. **flour** — completes the mill → bakehouse chain the bread round wants.
13. **dough** — the common-oven round and the oven-keeper's toll are already written as fiction.
14. **firewood** — the universal errand ("a bundle of kindling kept dry").
15. **lamp oil** — "bought before dusk", the lamplighter economy.
16. **badge (pilgrim)** — the most-sold object in the city, with a canonical counterfeit
    problem: built-in LLM drama for one lead disc.
17. **simples** — Mother Gude's herbs; the healer verb-set's natural currency.
18. **bucket** — water carrying and fire response.
19. **basket** — the market-carry container every third bio mentions.
20. **rope** — Moorings, wells, gallows; the most-mentioned tool in the corpus.
21. **knife** — the most-borrowed object in the bios.
22. **key** — locks, sluices, cells, pantries; the classic quest object, and the lore hands
    out key-rings by name.

### Tier 2 — rounding out ~50: trade goods, papers, wearables, tableware

Things that make NPCs richer to talk to and trade with, even if no system consumes them yet:

23. **letter** — the LLM seam loves a carryable message; messengers are an occupation.
24. **ledger** — every trade keeps one; theft/forgery plots pre-written.
25. **contraband_page** — the Sparr leaf is *the city's costliest contraband*; one item kind
    carries the whole second-sun questline.
26. **edict sheet** — posted in the five squares; literal wall-content that can also be held.
27. **deed** — the Crake claim, cellar disputes; property drama in one paper.
28. **sealed packet** — dry money and secrets across the pawn counter; the mystery-box item.
29. **tally stick** — debt made physical, split and matched.
30. **coat** — "the only coat", pledged, stolen, `grey` for the Custody.
31. **shoes** — leaking, re-soled, a dead child's pair at the pawn counter.
32. **blanket** — dried before curfew, carried to the gaol; the poverty item.
33. **cap** — with its goose feather; cheap identity.
34. **wool** — the staple inbound trade good.
35. **cloth bolt** — the staple outbound one; measured against the ell.
36. **hide** — Shambles → Tanners' Slip chain.
37. **leather** — the shoemaker's want.
38. **wax** — the chandler chain's raw block.
39. **tallow** — its poor cousin; butcher → chandler link.
40. **wick** — the Wickmarket's namesake; completes candle-making.
41. **cook-pot** — the shared pot is the household's hearth in one object; the pawnable good.
42. **bowl** — what stew actually arrives in.
43. **cup** — cracked and replaced; the tavern's other prop.
44. **dice** — licensed play, marked-piece offences; instant tavern scenes.
45. **keepsake** — one kind, many stories: every bio's button, bead and clay bird.
46. **writing kit** — pen/ink/chalk/slate; scribes and clerks are a whole occupation cluster.
47. **bandage** — "two clean bandages"; pairs with simples.
48. **lamp** — the carried light for lamplighters and night scenes.
49. **grain** — the sack that moves mill-ward; famine lore hangs off it.
50. **purse** — the quiet purse, dues-purses; a container with narrative weight.

### Tier 3 — plausible later, when a feature wants them

- **Craft & trade**: craft_tools {craft}, scales, sealed weight, hammer, spade, ladder, broom,
  lock, barrel, sack, chest, satchel, jug, spoon, pewterware, thread, canvas, malt (via grain),
  spice, cooking oil, onion, cress, hare.
- **Papers**: charter, licence, legal paper, shipping paper, blank paper, book, almanac.
- **Church & death**: vestments, shroud, censer, coffin, alms bowl, relic {name} (the seven
  saints' relics are quest-grade if the second-sun arc ever runs), crest panel.
- **Investigation kit**: instrument (lens/quadrant/dark box), mirror (the folk test of the
  disc!), seal, bottle of Bitter water.
- **Force**: weapon {kind}, armour; **music**: lute.
- **Wearables**: shirt, apron, gloves, hose, belt.
- **Bulk materials with a chain attached**: timber, stone, lime, glass pane, lead came, iron,
  dyestuff, coal.

### Tier 4 — recorded for completeness; almost certainly never items

The long tail of bulk and one-mention things: clay, sand, ash, tan-bark, hemp, pitch, soap,
bone, rag, brine, fodder, harness, cartwheel, lamp fittings, pump-barrels, cullet, putty,
gold-leaf, fulling chemicals (lye, stale urine, fuller's earth), dung/night-soil, salvage,
rushes, straw, chamber pot, yoke, sounding line, branding iron, tongs, bone skates, smoked
glass, marrow bone, sturgeon roe, green thread, watch badge (as its own kind), ring, silver
buckle, wedding cup, grave-crock, gravestone, memorial pole, bell filings, cast reed,
knotted cord, the soundings (explicitly *unwritten*), verses. The superseded denominations
(bell, lantern, mark, pennyhand token, weighday badge, lead token) live here too: chronicle
and dialogue material under the spark standard, never minted as stacks.

---

## Appendix — deliberately not items

Portability is the bar. These recur constantly in the lore but belong to other systems
(props, scenery, actors, economy fictions):

- **Vehicles**: boats, barges, hulls, the ferry, carts, handcarts, barrows, sledges, the
  Dry Race cradle-hull.
- **Live animals**: oxen, cattle, sheep, pigs, horses, hens, bees, dogs, cats, the two-headed
  calf. (Their *products* — meat, milk, egg, honey, hide, wool — are items.)
- **Fixed workshop gear**: ovens, looms, anvils, forges, furnaces, kilns, vats, presses
  (including the Grey Press), fulling stocks, tenter frames, smoke-racks, casting pits,
  grinding wheels, windlasses, hoists, cranes, treadwheels.
- **Fixed civic standards**: the chained Tallage stone, the iron ell at the Draper's Reach,
  the Serle tun (as measure), the chained Gaudry weights *in situ*, weigh-beams, the fee-board.
- **Furniture**: benches (incl. the reckoning bench), tables, desks, counters, stalls, stools,
  chairs, shelves, biers, beds, pallets, cradles.
- **The great bells**: Gravemouth, Ironthroat, Farcall, Truetongue, Evenblow, Smallvoice, the
  Orphan, the Scold — audio design and lore, never inventory.
- **Infrastructure**: wells, cisterns, sluices, well covers, mooring rings, scaffolds, the
  Harne chain (the link kept as relic *is* an item; the chain is history).
