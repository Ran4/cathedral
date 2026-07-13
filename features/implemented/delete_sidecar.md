So, right now we have a python-based sidecar, where the "smart actor" stuff lies.

Good thing: it's python-based and separated from the core rust+bevy game.
Bad thing: it's python-based and separated from the core rust+bevy game... so we need to copy all of the
commands etc. plus things like drawing rays to see what a character can see gets really hard.

What if we remove the sidecar and instead have it be written in Rust?

Now, we probably still want it to be somewhat separate in the sense that the
core social game simulation should be runnable without bevy;
so that the agent can test things for example without needing a window.

Suggestion on how to do this?

End goals we want to be able to achieve, is stuff like:

* The player can draw an image (in-game) on a map, then we can give that image to the llm model
  (example: the player has a paper map item (holding a .png).
  The player can write on it and give it to an npc and tell them
  "Please tell me about this neighborhood" and they'll answer.

  Or, to begin with, the player can show a map to an npc and point their finger on it (with the mouse) and
  say the same thing (so no editing required).

* The llm can request to get an image of what they're seeing ingame (yes, that would require rendering
  the game; but it's ok, simply disallow it when running it without bevy connection)

* In the end, we'll probably have 1000 characters; each should have their own character sheet,
  which is stored on disk as json files.
  As they interact with the world they of course learn new things. In the future, this should be
  stored in a db, but we skip that for now (new game = fresh memories).
