# Merged occupations and trades mentioned in the lore

We have consolidated occupations mentioned or
strongly implied by `lore/core_lore/` and `lore/second_sun/` into a smaller
set of occupation families for later population generation, see occupations.json

Each `occupation` is a merged job family. `alternative_titles` preserves the
more specific lore terms, ranks, and specialisms that can be used when writing
about an individual person. Related locations are supported by the lore but are
not exclusive workplaces.

Mason and glazier remain separate because their rivalry is identity-defining.
Other trades with especially distinctive material lives—such as baker,
butcher, brewer, smith, bellfounder, chandler, miller, physician and
executioner—also remain recognisable rather than being flattened into generic
workers.

Faction roles and character conditions are separate data, not occupations:
`moth`, `Wicket`, `Namekeeper`, `Tracer`, `Lead`, `Unwalled neighbor`, guild
rank, widowhood, poverty, pilgrimage, orphanhood, and being one of the Spared
should be layered onto a person's occupation.

## Deliberately separate secondary fields

The following should not be added to the occupation list. They belong in other
character fields:

- `rank`: master, mistress, journeyman, apprentice, novice, warden, contractor;
- `faction_role`: moth, Wicket, Namekeeper, Tracer, Lead, recorder, page-weigher;
- `illegal_activity`: thief, fence, forger, informer, smuggler;
- `condition_or_status`: pauper, widow, orphan, pilgrim, heretic, prisoner,
  recanted heretic, one of the Spared.

Thus Corin Copp is a **scribe and clerk** by occupation, a **journeyman** by
rank, and a **forger** by illegal activity. Grigor Ashe is a **salt trader** by
occupation and the **Wicket** by faction role.
