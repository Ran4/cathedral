# Occupations and roles in the Second Sun lore

This is a consolidated implementation inventory of the occupations, offices,
affiliations, life stages, and other usable social roles mentioned in the
Markdown lore collection. The `html/` folder was excluded.

## How this list is ranked

Within each section, roles are ordered from the most reusable and reasonable to
implement to the most specific or situational. A top-level bullet is a plausible
game role. Indented bullets are variants that should normally be baked into the
parent through traits, knowledge, faction, age, location, or dialogue—not
implemented as separate roles.

For example, implement **Merchant**, then express a relic seller as a merchant
with relic stock and lore. Implement **Porter**, then express Avel as a porter
with the `flour` specialization. A child map seller is a **Merchant** or
**Guide** with the `child` and `map seller` traits.

A person can combine several entries: Mara can be a **Builder** with the
`plasterer`, `repair contractor`, `widow`, `route-keeper`, and `Unwaller` traits.

## Crafts, construction, and maintenance

- **Builder** — broad construction role with knowledge of foundations, loads,
  roofs, walls, doors, scaffolds, cracks, and hidden voids.
  - **Mason** — stone specialist.
    - **Master mason** — senior designer or superintendent of major stonework.
  - **Carpenter** — timber-frame, joist, roof, door, and scaffold specialist.
  - **Plasterer** — wall, lime, and lath specialist with White Mortar traditions.
    - **Repair contractor** — independently hired repairer; Mara combines this
      with plastering.
  - **Roofer** — roof and tile specialist with access to elevated routes.
  - **Stair repairer** — the person mending a stair in a walking chapter.
- **Glazier** — stained-glass worker with unusually important setting knowledge.
  - **Master glazier** — workshop leader or conservator of the rose.
  - **Glazier apprentice** — trainee with workshop knowledge and scaffold access.
  - **Leadworker** — specialist responsible for lead came and window leadwork.
  - **Furnace worker / furnace-crew member** — prepares and works molten glass.
  - **Ash Reader** — guild specialist studying furnace waste and debt glass.
- **Craftsperson** — fallback role for skilled trades that do not justify unique
  AI behaviour or schedules.
  - **Master craftsperson** — workshop leader and trainer.
  - **Apprentice** — trainee attached to a guild or master.
    - **Runaway apprentice** — apprentice hiding from a master or guild.
  - **Clockmaker** — makes clocks and measuring instruments.
  - **Bellfounder** — casts bells and carries Bellfounders' Steps rumours.
  - **Lens grinder** — produces optical instruments and viewing lenses.
  - **Goldsmith** — carries the lore that gold blocks wrong light.
  - **Cooper** — makes and repairs barrels.
  - **Candle maker** — makes ordinary and double-wicked candles.
  - **Soap boiler / soapmaker** — Hessa Dault's livelihood.
  - **Dyer** — cloth worker; one prompt-ready actor is a dyer's widow.
  - **Printer** — produces guides, handbills, notices, and market material.
- **Cleaner** — low-status worker with access outside ceremonial hours.
  - **Cathedral sweeper** — nave cleaner; Enna Voss is the named example.
  - **Window cleaner** — works during the First Glass observance.
- **Fire-brigade member** — ladder-and-bucket emergency worker.
- **Gardener** — appears as Marten Pell's occupation in an absent life.

## Food, markets, lodging, and performance

- **Merchant** — broad trading role, from stallholder to wealthy merchant-family
  patron.
  - **Market seller / stallholder / vendor** — ordinary retail variant.
  - **Lens seller** — sells smoked or coloured viewing lenses.
    - **Amber-lens seller** — the very specific rumour-planting example.
  - **Relic seller / relic broker** — trades alleged fragments and sacred goods.
  - **Charm seller** — sells protective or allegedly anomalous charms.
  - **Candle seller** — sells Two Vespers and ordinary candles.
  - **Map seller** — sells present, absent, or inaccurate maps.
    - **Child map seller** — Pip's `merchant + child + map seller` combination.
  - **Viewing-token seller / booth keeper** — sells places on a viewing cord.
  - **Indulgence seller** — participates in the pilgrimage economy.
  - **Collector** — privately buys or holds anomalous glass and relics.
  - **Customer / buyer** — opposite side of a market interaction.
    - **Soap buyer** — very narrow rumour-carrier variant.
- **Baker** — bakes and may deliver bread; bakeries recur in present and absent
  city life.
- **Hostel keeper / hosteler** — provides lodging to pilgrims and visitors.
- **Guide** — conducts visitors to sites, routes, and viewing positions.
  - **Licensed guide** — officially permitted pilgrimage guide.
  - **Route guide** — leads ordinary, absent, or Fellowship routes.
  - **Child guide** — combines the guide role with the child trait.
- **Broker / event organizer** — arranges people, access, publicity, and money.
  - **Pilgrim broker** — arranges tokens, lodging, lenses, and experiences.
  - **Spectacle broker** — packages and profits from public wonder.
  - **Civic impresario** — organizes citywide festivals and spectacles.
- **Performer / crier** — public voice used for entertainment, sales, and rumour.
  - **Street crier / hawker** — voices documented market sales cries.
  - **Tavern singer** — circulates rumours and verses.
  - **Poet** — carries the foreign-sky rumour in the oral-culture file.

## Transport, canal, routes, and delivery

- **Boat worker / canal worker** — broad water-labour role with canal customs and
  route knowledge.
  - **Boatman / boatwoman** — transports people and goods.
  - **Canal polewoman / poleworker** — poles a boat beneath low bridges; Nera is
    the named example.
  - **Ferry worker** — carries passengers or goods across or along the canal.
  - **Canal-guild member** — boat worker with organized-labour affiliation.
- **Porter** — carries goods and burdens around the city.
  - **Flour porter** — Avel Noll's specific livelihood.
  - **Canal porter** — quay and cargo specialization.
  - **Delivery worker** — bread, flour, cargo, or other delivery specialization.
- **Courier** — carries observations, documents, objects, or messages.
  - **Passage runner / message runner** — uses hidden or child-known routes.
  - **Lookout** — watches a route or warns a cell.
- **Canal engineer** — specialist for canal structures and the civic-wound
  interpretation.
- **Diver / trained swimmer** — retrieves objects or enters the black-water
  basin when the canal cannot be drained.

## Medicine, care, death, and refuge

- **Physician / carer** — broad health and care role; this distinction is useful
  in dialogue but need not require separate base AI.
  - **Physician** — treats illness and counter-memory.
    - **Ethical physician** — prioritizes consent and humane uncertainty.
    - **Ambitious physician** — sells cures or exploits glass-sickness.
  - **Midwife** — birth and family specialization with its own oral lore.
  - **Carer / caregiver** — practical or emotional care without physician status.
  - **Witness advocate** — protects witnesses from coercion and exposure.
- **Refuge worker / refuge organizer** — operates a witness refuge or One-Shadow
  hall.
- **Burial worker** — broad death-care and mutual-aid role.
  - **Grave worker** — performs burial-ground and body-related work.
  - **Burial-society worker** — handles funerals and burial dues.
  - **Burial-fund organizer** — Hessa's neighbourhood leadership role.

## Cathedral and religious work

- **Clergy** — broad religious role with doctrine, pastoral duties, liturgy, and
  institutional knowledge.
  - **Priest / clergyperson / cleric** — ordinary ordained variant.
    - **Junior priest** — lacks access to sealed evidence.
  - **Canon** — Cathedral Chapter member who may preach, sing, archive, or govern.
  - **Preacher** — sermon and public-speaking specialization.
  - **Pastoral caregiver** — prioritizes care over doctrinal investigation.
  - **Church scholar / theologian** — interprets theology or history.
    - **Licensed scholar** — permitted to investigate the phenomenon.
  - **Novice** — person in early religious formation.
  - **Nun** — member of a women's religious order.
    - **Choir nun** — Sister Belis's specific combination.
  - **Anchoress** — enclosed religious specialist associated with the mercy
    hypothesis.
  - **Inquisitor** — old-school religious investigator.
  - **Bishop** — senior leader of the cathedral.
  - **Dean** — senior Cathedral Chapter officer.
  - **Cathedral treasurer** — officer responsible for funds and property.
  - **Precentor** — officer responsible for music and liturgy.
- **Choir member / singer** — performs services and carries duplicate-vespers
  traditions.
  - **Choir child** — combines choir membership with the child trait.
- **Congregant / worshipper** — resident or pilgrim participating in worship; a
  social activity rather than a separate base role.

## Lucent Custody

- **Custodian** — one reusable Office role, with rank controlling knowledge,
  authority, equipment, and behaviour.
  - **Custody officer** — generic ranked officer.
  - **Custody guard / cathedral guard** — access and crowd-control assignment.
    - **Crowd officer** — checks tokens and handles pilgrim queues.
  - **Custody archivist** — indexes or preserves Office reports.
    - **Assistant archivist** — Brother Caldus's junior assignment.
  - **Lector** — interviews witnesses and compares accounts.
  - **Weight** — operates instruments, marks light-falls, and maintains records.
  - **Veiler** — handles crowds, contraband, searches, and threats to the rose.
  - **Lucent Warden** — head of the Office.
  - **Custody auxiliary** — outside helper or temporary agent.
  - **Custody informant** — recruited source, sometimes a child or debtor.
  - **Douser / Half-Shadow** — street names, not separate roles.

## Civic, legal, guild, and property roles

- **Civic official** — broad government role dealing with law, property,
  markets, access, and administration.
  - **Administrator** — generic civic, chapter, or Custody bureaucrat.
  - **Magistrate** — rules on property, markets, liability, and evidence.
  - **Judge** — presides over trials.
  - **Provost** — senior administrative office.
  - **Property regulator** — part of Provost Salvi's specific duties.
- **Guard** — generic access-control or enforcement role.
  - **Officer** — ranked or official variant.
  - **Cathedral guard** — cathedral assignment.
  - **Custody guard** — Office assignment.
- **Organizer** — coordinates a civic, religious, or neighbourhood event.
  - **Procession organizer** — arranges ceremonial routes.
  - **Festival organizer** — plans conjunction festivities.
- **Guild member** — craft affiliation and career-stage modifier.
  - **Master / guildmaster** — workshop or guild leader.
  - **Apprentice** — trainee; normally combined with a craft role.
- **Employer / patron** — controls work, money, or institutional support.
  - **Employer** — hires workers such as guides or runners.
  - **Patron / financier** — funds clergy, scholars, merchants, or radicals.
- **Property participant** — useful legal-state tag, not an occupation.
  - **Property owner / householder / homeowner**
  - **Tenant / renter**
  - **Property claimant**

## Scholarship, archives, and investigation

- **Scholar / College member** — broad empirical or learned role.
  - **College student** — trainee or junior member.
  - **Natural philosopher** — studies nature, optics, instruments, or astronomy.
  - **Professional skeptic / skeptical scholar** — tests claims without adopting
    their doctrine.
  - **Calculator** — computes conjunction dates.
  - **Surveyor** — maps light-falls, structures, or absent routes.
  - **Researcher / experimenter** — conducts trials.
  - **Instrument operator** — tends frames, pegs, plates, prisms, and related
    equipment.
  - **Spectrist** — optical or natural-instrument school.
  - **Attentionalist** — expectation-and-attention school.
  - **Substructuralist** — buried-foundation and absent-street school.
  - **Harmonicist** — light, bell, lead, and resonance school.
- **Archivist / scribe** — broad record-keeping role that can belong to the
  cathedral, Custody, Keepers, or civic government.
  - **Archivist** — preserves, indexes, conceals, or leaks records.
  - **Scribe** — copies and compares documents or testimony.
  - **Assistant archivist** — junior archival assignment.

## Faction and ideological modifiers

These should almost always modify an occupation rather than become an occupation
of their own. They are ordered from broad faction memberships to narrower internal
positions.

- **Cathedral Chapter member**
  - **Reservist** — supports Reserved Creation and controlled access.
  - **Sufficientist** — minimizes fascination and emphasizes ordinary duty.
  - **Conjunctivist** — prepares discreetly for an overlap of the suns.
- **Guild of Saint Lume member**
  - **Continuist** — favours exact conservation.
  - **Reframer** — favours active reconstruction of parts of the rose.
  - **Ash Reader** — secretly studies debt glass and furnace waste.
- **Unwaller / Fellowship member**
  - **Occlusionist** — believes a real second sun has been walled out.
  - **Route-keeper / passage keeper** — maintains a walking chapter.
  - **Fellowship cell member** — ordinary local participant.
  - **Fellowship host** — household in a hospitality-chain route.
- **Child of the Open Sky / Roofless**
  - **Open Sky follower / radical** — supports physical openings or Unleading.
- **Keeper of the Absent Streets**
  - **Resident** — secret inner tendency, not an ordinary city-resident tag.
- **Concord member / One-Shadow**
  - **Concord organizer** — coordinates protection, refusal, or suppression.
- **College of Measures member**
  - Use the school traits under **Scholar / College member** above.
- **Belief-position trait**
  - **Believer**
  - **Skeptic**
  - **Civic rationalist** — reframes metaphysics as governance and liability.
  - **Reformer**
  - **Iconoclast**
  - **Dissident**
  - **Heretic** — usually a label imposed by an authority.

## Crime, fraud, and coercion

- **Thief** — broad illicit-taking role.
  - **Burglar** — specializes in entering buildings.
  - **Pickpocket** — works pilgrim crowds.
  - **Lead thief** — plot-specific target or radical specialization.
- **Counterfeiter / fraudster** — creates or sells false objects and staged
  manifestations.
  - **Counterfeiter** — makes false panes, tokens, relics, or documents.
  - **Fraudster** — stages or markets a false miracle.
- **Informant / accomplice** — covert supporting role.
  - **Informant** — reports names, routes, or activity.
  - **Accomplice** — assists a fraud or covert entry.
  - **Conspirator** — participates in an organized covert act.
- **Vandal / saboteur** — damages or obstructs a site for criminal or ideological
  reasons.
  - **Vandal** — opportunistic or expressive damage.
  - **Saboteur** — planned factional damage.
- **Legal-status trait** — situational state rather than occupation.
  - **Prisoner / detainee**
  - **Accused person / defendant**
  - **Debtor**
  - **Scapegoat**

## Age, residence, travel, and crowd modifiers

These should be implemented as traits or temporary states layered onto an
occupation.

- **Resident / local** — default established inhabitant.
  - **Citizen**
  - **Adult resident**
  - **Elder / elderly resident**
  - **Neighbour**
  - **Householder / occupant**
  - **Refuge resident**
- **Child** — age role with distinct routes, games, rules, and vulnerability.
  - **Adolescent** — older-child dare-group variant.
  - **Cathedral / choir child**
  - **Rooftop child**
  - **Child guide / map seller**
  - **Child informant**
  - **Child witness**
- **Visitor / outsider** — lacks local language and trust.
  - **Pilgrim** — visits for religion or spectacle.
    - **Grieving pilgrim** — seeks healing, contact, or a second chance.
  - **Passenger**
  - **Stranger**
  - **Newcomer**
- **Worker** — generic fallback when no occupation is specified.
  - **Low-status worker**
  - **Night worker**
- **Crowd participant** — temporary public-event state.
  - **Bystander**
  - **Crowd member**
  - **Spectator**
  - **Protester**
  - **Volunteer**
  - **Congregant / worshipper**
- **Test subject** — temporary role in a College trial or controlled observation.

## Witness, memory, illness, and harm modifiers

- **Witness** — directly perceives or reports a manifestation.
  - **Eyewitness**
  - **Unlicensed witness** — the player's initial role outside official systems.
  - **Celebrated witness** — public reputation variant.
  - **Testifier / deponent** — makes a formal statement.
  - **Source** — origin of a rumour or report.
- **Counter-rememberer / affected person** — has coherent absent memories or
  another lasting effect.
  - **Glass-sickness sufferer** — common and sometimes hostile label.
  - **Patient** — receiving physical or counter-memory care.
  - **Sick person / plague patient**
- **Survivor / victim** — harm-history trait.
  - **Plague survivor**
  - **Fire, crowd-crush, riot, fraud, or exposure survivor**
  - **Bereaved person / mourner**
  - **Refuge resident**
- **Conversation role** — temporary position in information flow.
  - **Listener**
    - **Safe listener** — hears without exploiting disclosure.
  - **Gossip / rumour carrier**
  - **Confessor**

## Family and relationship modifiers

These are social links, never standalone occupations.

- **Parent**
  - **Mother / father**
  - **Bereaved or grieving parent**
- **Spouse / partner**
  - **Husband / wife**
  - **Former spouse**
  - **Widow / widower**
  - **Present, deceased, or counter-remembered partner**
- **Sibling**
  - **Brother / sister**
- **Friend**
- **Guardian** — especially the chosen guardian of a child witness.
- **Extended kin / family member**
  - **Aunt / uncle / niece / nephew**
  - **Son / daughter**
  - **Kin / family member**
  - **Estranged relative**
- **Lineage and inheritance relation**
  - **Descendant / ancestor**
  - **Heir / inheritor**

## Player-facing identities

These are ways the city may understand the player. They should emerge from
conduct or affiliation, not become separate NPC occupations.

- **Unlicensed witness**
- **Courier of observations**
- **Surveyor of wrong light**
- **Route-keeper**
- **Custody auxiliary**
- **Professional skeptic**
- **Spectacle broker**
- **Safe listener / dangerous gossip**
- **Naive pilgrim**
- **Unwelcome independent**
- **Celebrated witness / Quiet Witness**
- **Lead-Bringer**
- **Window-Eater**

The final three entries are civic reputations or epithets rather than jobs.

## Source coverage

This inventory was made from `README.md` and
`01_CANON_AND_MYSTERIES.md` through `12_PROMPT_READY_CONTEXT_PACKS.md`.
The largest direct occupation inventories are in
`03_FAITHS_AND_FACTIONS.md`, `04_CITIZEN_KNOWLEDGE_AND_NPC_BIBLE.md`,
`05_CHARACTER_SEEDS.md`, `07_RUMOURS_AND_ORAL_CULTURE.md`, and
`12_PROMPT_READY_CONTEXT_PACKS.md`. The other files supply one-scene roles such
as anchoress, inquisitor, cooper, gardener, bell ringer, judge, and fire-brigade
member.
