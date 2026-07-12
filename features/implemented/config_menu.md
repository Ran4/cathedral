We need a config menu.

If you press esc, open a menu where we can configure various settings, looking like:

* Continue
* NPC voices' text to speech (TTS) model |  (Cloud OpenAI) / (Local whatthemodeliscalled) | Working
* Your voice's speech to text (STT) model | (Cloud ???) / (Local) | Loading file...

You can click on cloud or local (cloud/local should be a "pill" toggle) for each to switch to that.

So, multiple lines, each line containing name of setting, toggleable options and status.
First line is just the words "Continue".

You close the menu by pressing esc again or clicking continue.

The settings DO update the config.ron file,
but first add config.ron to .gitignore and create a new file
default_config.ron which we commit.

Are there any questions? If not, implement.
