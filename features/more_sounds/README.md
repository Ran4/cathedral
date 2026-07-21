# More Sounds workbench

Start the local backend from the repository root:

```sh
uv run --script features/more_sounds/server.py --open
```

Or start it and open the frontend specifically in Firefox:

```sh
make -C features/more_sounds
```

If the browser does not open automatically, visit <http://127.0.0.1:8798/>.
The server binds only to loopback unless `--host` is supplied.

The workbench writes its durable shortlist directly to
`sounds_to_implement.json`. Generated drafts are stored in `generated/`; loops
are converted to WAV with `ffmpeg`, while one-shots and sequences remain MP3.
The single-sound adapter reuses the provider functions in
`../../scripts/generate_sounds.py`. Generation uses `ELEVENLABS_API_KEY` from
the real environment, the repository `.env`, or `prompt_playgound/.env`, in
that order.

Each sound in `more_sounds.json` has an `implemented_in_game` boolean. The
shortlist mirrors that flag and records `generated_audio.path` when a draft
exists. A future implementation pass should use the selected entries, place
their generated files in the game, and then set `implemented_in_game` to
`true`. The backend preserves extra implementation notes added to shortlist
entries and reconciles true implementation flags between both JSON files.
