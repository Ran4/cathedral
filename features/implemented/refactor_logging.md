Right now, we screenshot into screenshots/

and any logs is just output to stdout (I think?)

Refactor it so it's more like this:

logs/
    session_34_2026-07-13_09_52_30/
        screenshots/
            cathedral_screenshot_2026-07-13_09_54_31__00.png
        logs.jsonl
        prompts/
            2026-07-13_09_52_30__00_k0fb1_Ilse_prompt.md

where session_34_2026-07-13_09_52_30 means session 34, started at yyyy-mm-dd hh:mm 2026-07-13_09:52.30.
Also, whenever we start the game,
1. Create the new session directory
2. Symlink logs/latest_session to the newly created session didr

logs.jsonl should be structured logs, stored as jsonlines (to make it easier for you the agent to parse
a session in the future)

cathedral_screenshot_2026-07-13_09_54_31__00.png means yyyy-mm-dd_hh_mm_ss__nn
where nn starts at 00 for every second then goes up if there's more than one screenshot in a second.
so, 00, 01, 02 and so on.

The prompts folder stores each prompt that is sent to the LLM as well as the answer.

So, `2026-07-13_09_52_30__00__k0fb1_Ilse_prompt.md`

means `yyyy-mm-dd_hh_mm_ss__nn__actorid__actorname_prompt`
