So, right now, all the NPCs aren't moving...

We need to support movement.

Think Fallout 3/NV: characters should walk around, doing their thing.

Also take some inspiration from ~/seagame - people have their own idea of what to do.

Now, ideally an LLM would control all "thinking", but... that's going to be incredibly
expensive. So, we need a solution that's *mostly* "code-driven".

That said, we probably still want *some* llm interaction. Maybe a daily "sleep" where
we try to find new goals, summarize what we've learned during the day, or whatever.
Not relevant for the ambient characters (they might have one huge llm prompt that
updates them all once per day perhaps?).

Oh, and we don't have a day night cycle either, so that needs to be added... I think
what seagame did is good.

Give a really long and hard think about how we might want to implement movement to the game.

Write a LONG and IN DEPTH plan to features/movement folder (feel free to include extra markdown files etc.
in that folder).
