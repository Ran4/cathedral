# Ambient residents evidence

M0–M4 are implemented and verified (2026-09-06). The milestone records retain
the status and findings at each handoff. The [spec](../ambient_residents.md)
is the current overall status.

- [M0: original behaviour](m0.md) — actual movement baselines and measurement
  corrections.
- [M1: resident places](m1.md) — reserved frontage positions and bounded paths.
- [M2: default residents](m2.md) — jobless identities, local routines and
  domestic support.
- [M3: integration](m3.md) — interruptions, shelter, shared route clearance,
  authored starting positions and regression results.
- [M4: acceptance](m4.md) — final two-day measurements, capacity, visual
  verification and performance, including the measured CPU increase.
- [Independent behaviour review](acceptance_review.json) — both supported
  populations pass the formal two-day requirements.
- [Integration review](review.md) — decisions and independent checks across
  the sequential milestone handoffs.
- [GPU launch notes](gpu_launch.md) and [camera notes](m4_camera_notes.md) —
  hidden-window safeguards, original captures and timing qualifications.

Raw logs, JSON traces and geometry reports accompany the records. Screenshots
remain in their recorded `logs/session_*` directories. Original captures with
`approx30/180/600` names do not reliably represent those simulation ages;
use their HUD times and the camera notes. Final capture sidecars record the
authoritative state and clock at the screenshot request, before asynchronous
image completion.

Raw metadata retains the paths and commands recorded before archival. The
Markdown reproduction commands and helper locations use this archived folder.
