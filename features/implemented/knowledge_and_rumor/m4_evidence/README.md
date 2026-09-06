# M4 verification artifacts — 2026-09-06

The full review is in `../plan/m4_review_2026-09-06.md`. These artifacts retain the UI layout,
software-rendered screenshots and 20,000-person cost comparison. Live model evidence is under
`../m0_evidence/`, with its setup and verdict appended to `NOTES.md`.

`VISUALS.json` records the four accepted runs, their original session paths and all 249 X11 window
samples. Every sample is unmapped and unfocused. The five reviewed frames are under `screenshots/`.
The software fallback keeps the real UI fonts and city map but omits 3D texture/shader assets;
the simulation still uses the shipped world data. This is a UI verification, not a real-GPU or
fully textured scene verification. The inaccessible NVIDIA device and failed full-asset runs are
recorded in the review. `run_visuals.py` reproduces the fallback from this checkout:

```sh
uv run --script features/knowledge_and_rumor/m4_evidence/run_visuals.py
```

`journal_layout_probe.rs` includes the actual journal and font source, supplies a dummy camera,
and exercises real `UiPlugin` / `TextPlugin` layout with the bundled fonts. It creates no renderer
or window. From the repository root, the recorded build was:

```sh
journal_probe=$(rg --files features -g journal_layout_probe.rs)
CARGO_MANIFEST_DIR="$PWD" CARGO_PKG_NAME=cathedralbevy \
  rustc --edition 2024 -C opt-level=1 "$journal_probe" \
  --extern bevy=target/debug/deps/libbevy-0e5eec39d7583f38.rlib \
  --extern cathedral_sim=target/debug/deps/libcathedral_sim-4acf819a9445c146.rlib \
  -L dependency=target/debug/deps -o /tmp/cathedral_m4_journal_layout
timeout 30s /tmp/cathedral_m4_journal_layout 1
timeout 30s /tmp/cathedral_m4_journal_layout 24
timeout 30s /tmp/cathedral_m4_journal_layout 24 small
```

The dependency hashes are this build's and may differ after rebuilding. All three layout logs
are retained. They cover actual font loading, 120 open frames, wheel movement to the bottom, and
ten close/reopen cycles; they also bound the four glyph atlases to 512×512.

`cost.json` contains the command, ON/OFF environments, CPU/RSS measurements and final saturated
census. The two `.time` files are the raw `/usr/bin/time -v` output. This is the M2 comparison shape
(20,000 extra bodies, 1.2 game hours, 3-second step); M5 owns the final longer cap run and tune.
