# Parent camera review, before M2–M4 acceptance

Inspected all eight initial session 810 captures at 1,000 extras. They use the
unchanged pre-feature executable. The images have loaded city assets and
normal 1× HUD state. Their `approx30` filenames are **not** actual 30-second
simulation ages: the controller's 100 ms virtual clamp means the HUD reads
07:05–07:07, approximately 12.5–17.5 elapsed simulation seconds since 07:00.
See `gpu_launch.md` for the corrected 10:1 wall/simulation timing estimate.

The elevated Wickmarket view shows the crossing routes and perimeter clearly.
Its ground view is partly blocked by a foreground market stall and faces
away from most nearby resident candidates. Keep it for direct comparison,
but add a frontage-facing view for assessing resident placement. Most safe
spots in this observation box belong to buildings `omb_f0057`/`omb_f0058`
around Z276–286, behind the original ground camera looking toward −Z.
A candidate supplemental camera is `tp -17.375 1.7 262 180 0`, looking toward
those frontages; verify framing and walkable camera coordinates before using.
Do not move actors to fit the camera or select only an unusually busy patch.

Tenterhook Lane's ground/elevated views include the well and a residential
street. Burnt Court ground faces a clear building frontage and its door;
the elevated view shows both the court and connecting corners. Needle ground
shows the staircase/pass-through area, and the elevated view shows the narrow
connection and adjoining court. These are useful geometry views even when
few generated residents occupy them; the actual choke should remain empty
of standing residents. The baseline views show only a few people at this
early age, which is why later captures and formal two-day population counts
are necessary.

M4 must inspect later images at their actual displayed time and use the
latest measured occupancy/actor IDs to interpret who is stationary. A clean
view is not evidence of the cause of a particular actor's movement. Add debug
or status views for that separate claim. The screenshot timing limitation
does not affect the normal-step headless two-day acceptance measurements.

The second Wickmarket captures in session 810 provide a useful normal-speed
reproduction: `ambient_before_wickmarket_ground_approx180.png` reads 09:34
at 1× and shows a long line along the crossing route; the elevated counterpart
reads 09:35 and shows the procession stretching across the square. The
parent inspected both images. Actual simulation age is approximately 386
seconds, because GPU pacing recovered mid-run (see `gpu_launch.md`). These
are visual evidence of a line; identify individual actors through simulation
or debug evidence when attributing a particular walk to generated routines.

The final session 810 views named `approx600` are actually evening: Wickmarket
ground/elevated read **20:32**, and Burnt Court ground reads **20:33**, Day 2,
1×. The parent inspected those three images. At roughly 2,030 elapsed
simulation seconds, the ground Wickmarket view still has a compact line of
silhouettes crossing near the stall; the elevated and court images are very
dark. Keep these as truthful evening records, but do not use the darkness as
proof of empty or occupied frontage. M4 should capture an earlier evening
as well, when individual bodies can still be assessed.

The parent also inspected session 811's 2,000-extra Wickmarket ground and
elevated `approx180` images. Both show **08:10**, Day 2, 1× (about 175 elapsed
simulation seconds), with a long procession across the square. These are
especially useful comparisons for a final 180-second view. Session 810's
1,000-extra comparison is later, at about 386 seconds; retain that difference
or add a final view near the same moment instead of claiming identical ages.

Session 811 finished with 24 images and 2,208 unmapped/unfocused checks. Its
final Wickmarket ground/elevated views read **18:55/18:56**, and Burnt Court
ground **18:57**, Day 2, 1×. The line is still visible in Wickmarket. These
late views contain speech/interaction overlays, so they are not clean views.
The fake speech backend's default "What's your name?" appears even though the
drive script contains no speech action: the microphone was still enabled.
For controlled M4 captures, use the existing `key KeyV` toggle once at startup
to disable microphone input and verify `MIC OFF`; this does not persist a
configuration change. Do not infer a particular actor's walking cause from
the late baseline silhouettes or use those speech-contaminated frames as a
calm interaction benchmark.

`inspect_gpu_timing.py` reads existing frame/drive logs only. The matching
`ambient_before_1000_timing.json` and `ambient_before_2000_timing.json` report
estimated virtual ages and all complete 60-second frame intervals, including
camera changes and suspected driver pacing. GPU runs overlapped implementation
checks, so an isolated comparison is needed for causal frame-speedup claims.
