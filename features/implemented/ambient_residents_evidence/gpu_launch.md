# Hidden GPU launch investigation — 2026-09-06

The initial approximately 1 Hz baseline is a presentation/driver pacing state.
It is **not a measurement of the crowd's frame cost**. No tested launch flag
removed it reliably. Later, unchanged session 810 resumed normal throughput
while remaining unmapped and unfocused. Use measured frame intervals and the
screenshot HUD time; neither a permanent 1 Hz limit nor one wall/simulation
conversion ratio is justified.

## What was established

- The existing binary renders with Vulkan on the **NVIDIA GeForce RTX 4070,
  driver 535.309.01**, when launched by a uniquely named, detached host tmux
  session. The tool execution environment alone selects llvmpipe because its
  device namespace lacks the NVIDIA nodes. Do not use software-rendered runs as
  GPU baselines.
- `src/main.rs` already selects `UpdateMode::Continuous` for both focus states
  when `CATHEDRAL_HEADLESS` is set. `src/perf.rs` already selects
  `PresentMode::AutoNoVsync` except in `CATHEDRAL_PERF=vsync` mode.
- The same one-second pacing occurs with **zero extra residents** and
  `CATHEDRAL_PERF=plain`, which omits GPU timestamp instrumentation.
- Syscall tracing identifies a repeated deliberate sleep inside
  `libnvidia-glcore.so.535.309.01`. For example, a driver-calling thread in
  session 807:

  ```text
  785261 16:33:58.329516 clock_nanosleep(CLOCK_REALTIME, 0,
      {tv_sec=0, tv_nsec=890000000}, ...) = 0
  785261 16:33:59.282930 clock_nanosleep(CLOCK_REALTIME, 0,
      {tv_sec=0, tv_nsec=937000000}, ...) = 0
  785261 16:34:00.301168 clock_nanosleep(CLOCK_REALTIME, 0,
      {tv_sec=0, tv_nsec=918000000}, ...) = 0

  > libc.so.6(clock_nanosleep+0xc8)
  > libc.so.6(__nanosleep+0x17)
  > libnvidia-glcore.so.535.309.01() [0x1514697]
  ```

  The varying sleep durations fill out almost exactly one second per frame;
  other Bevy threads wait on futexes until that thread wakes. This establishes
  the immediate cause of the long frames. The proprietary driver's internal
  reason for selecting that pacing was not identified. In particular, the
  one-second acquire timeout found in wgpu source is **not proof** that an
  acquire timeout caused this: screenshots succeed and the observed syscall is
  a driver nanosleep, not an observed `VK_TIMEOUT` result.

## Bounded probes

Every probe used the unchanged prebuilt `target/debug/cathedralbevy`, zero extra
residents, fake cognition, the same Wickmarket view, and a 12-second drive wait.
The executable was built at 2026-09-06 12:47:06 CEST; its previously recorded
baseline SHA-256 is
`d244c26260a172e3b917f370071a6e9865850d8353f87a89f6f88d8e60a81c25`.
No compilation or game-code edit was performed by this investigation.

| Probe | Session | Frame p50 | Result |
| --- | --- | ---: | --- |
| Plain perf, syscall trace | `session_806_2026-09-06_16_32_47` | 999.88 ms | Reproduces pacing without timestamp queries |
| Plain perf, nanosleep stack trace | `session_807_2026-09-06_16_33_56` | 1001.98 ms | Long sleeps originate in NVIDIA driver |
| Plain perf, `__GL_SYNC_TO_VBLANK=0 __GL_GSYNC_ALLOWED=0 __GL_VRR_ALLOWED=0` | `session_808_2026-09-06_16_35_09` | 999.84 ms | No improvement |
| `CATHEDRAL_PERF=vsync` | `session_809_2026-09-06_16_35_53` | 999.93 ms | No improvement |

The traced runs include tracing overhead and all four runs include startup;
their percentiles only establish the pacing symptom. They are not suitable
for comparing crowd performance. The original 1,000- and 2,000-extra baselines
in sessions 804–805 have the same limitation.

All four probes exited successfully and saved `screenshots/gpu_probe.png` in
their session directory. The screenshot in session 808 was visually inspected:
it contains the loaded city and the HUD reads **Dayspring 07:01, Day 2, 1×**.
The requested 1280×720 logical resolution produces a **1493×840** PNG on this
display; compare actual PNG dimensions when checking images.

The guard checked only windows belonging to the launched process and its
children. **355 observations** across these probes all reported `IsUnMapped`
and none matched the focus target. `CATHEDRAL_HEADLESS_AUDIO` was removed from
each child's environment. No window was mapped as an experiment, no cursor
or keyboard input was synthesized, and no driver/display setting was changed.
Only dedicated detached sessions were created; the user's sessions were not
modified.

Machine-readable results and all sampled map/focus observations are in
[`gpu_launch_probes.json`](gpu_launch_probes.json). Full temporary traces are
`/tmp/cathedral-gpu-trace.trace` and `/tmp/cathedral-gpu-stack.trace`; the stack
excerpt above preserves the relevant finding if those temporary files expire.

## Repeating a safe launch

The temporary guarded runner is `/tmp/cathedral-gpu-probe.py`. It enforces the
headless/audio/asset-root settings, checks map/focus every 250 ms, rejects a CPU
renderer or missing assets, uses a 50-second watchdog, and exits its own child
process group on failure. It runs the prebuilt game without rebuilding it.
An example reproduction, using a fresh session name, is:

```sh
tmux new-session -d -s cathedral-gpu-repeat-unique \
  -c /home/ran/src/rust/cathedralbevy \
  'UV_CACHE_DIR=/tmp/cathedral-uv-cache uv run --no-project /tmp/cathedral-gpu-probe.py repeat > /tmp/cathedral-gpu-repeat-run.log 2>&1'
```

The runner's game environment is:

```text
CATHEDRAL_HEADLESS=1
CATHEDRAL_FAKE_BACKEND=1
CATHEDRAL_EXTRA_NPCS=0
CATHEDRAL_PERF=plain
CATHEDRAL_DRIVE_RES=1280x720
CATHEDRAL_DRIVE_TIMEOUT=45
BEVY_ASSET_ROOT=/home/ran/src/rust/cathedralbevy
WAYLAND_DISPLAY=
CATHEDRAL_HEADLESS_AUDIO absent
CATHEDRAL_DRIVE=wait-online; weather clear; tp -17.375 1.7 268.375 0 0; sleep 12; shot gpu_probe; quit
```

It inherits host `DISPLAY=:0` and an X11 session. The existing durable
[`bevy_perf.py`](bevy_perf.py) uses the same guarded host-launch principle for
longer population runs. Do not execute these probes concurrently with another
GPU benchmark, and do not infer throughput improvements by subtracting driver
sleep from the overall frame duration.

## Interpreting waits honestly

`run_drive_script` consumes `Time<Real>` (`src/drive.rs`). The local engine
consumes default virtual `Time` (`src/smart_actors/local_engine.rs`).
`ControllerPlugin` overrides Bevy's 250 ms default by inserting
`Time::<Virtual>::from_max_delta(MAX_FRAME_CATCHUP)` in `src/controller.rs:131`,
where `MAX_FRAME_CATCHUP` is **100 ms** (`src/controller.rs:60`). This limits
virtual time for the whole world, including the engine. Thus at the observed
one-frame-per-second pace, each real second usually advances only about
**0.1 seconds of simulation input time**. With `seconds_per_day: 3600` and
clock scale 1×, that is **2.4 game-clock seconds per real second**.

Correction, 2026-09-06: the initial version of this report overlooked that
controller override and used Bevy's 250 ms default. Its 4:1 real/virtual-time
ratio and associated waiting estimates were incorrect; the observed ratio
here is approximately **10:1**. The driver tracing and probe results above
are unaffected.

For planning a screenshot, 40 seconds of drive sleep therefore corresponds to
about four seconds of simulation input time, or **1 minute 36 seconds on the
game clock**, once startup has settled. A desired 15 game-minute interval
needs roughly **375 real seconds**; 30 seconds of simulation input time needs
roughly **300 real seconds**. Allow additional time for the scripted action
gaps and screenshot completion, and set `CATHEDRAL_DRIVE_TIMEOUT` and the
external guard timeout accordingly. This is an estimate; **record and compare
the HUD time in the saved images** rather than treating a wall-clock wait as
exact simulation progress.

Independent corroboration: session `session_810_2026-09-06_16_48_02` logs `sleep 120`
at drive time 4.4 s, then requests its first Wickmarket screenshot at 125.4 s.
The saved `ambient_before_wickmarket_ground_approx30.png` HUD reads **07:05,
Day 2, 1×**. A 120-second real wait admits about 12 simulation-input seconds,
or 4 minutes 48 seconds on the game clock, consistent with that screenshot.
The legacy `approx30` filename does not establish 30 elapsed simulation
seconds; use the clock shown in the image.

For a more precise elapsed-time audit, `perf_frames.jsonl` records real frame
deltas; summing `min(frame_ms, 100) / 1000` estimates virtual elapsed time.
Account for engine startup, the chosen comparison interval, and any clock
scale changes. The stored frame deltas are rounded and the file need not
contain the final unflushed frame, so the HUD and engine clock remain the
primary evidence of the photographed moment.

## Later observed recovery without launch changes

In session 810, the last slow frame was 1,003.2 ms at 16:55:15.191 CEST
(14:55:15.191 UTC), followed by a 31.3 ms frame at 16:55:15.213 CEST.
`perf_frames.jsonl` line 269 begins the fast interval at elapsed time
430.438 seconds; `logs.jsonl` lines 669/672 record the boundary.

| Real elapsed interval | Frames | Frame p50 | Frame p95 |
|---|---:|---:|---:|
| 60–420 seconds | 360 | 1,000.15 ms | 1,001.53 ms |
| 480–720 seconds | 16,047 | 13.57 ms | 26.40 ms |

The later interval averages 66.86 FPS with a 64.4 ms maximum frame time,
below the 100 ms virtual clamp. This is a 1,000-extra pre-feature run with
the Needle elevated camera. All 732 sampled window checks in the audit
snapshot remained unmapped and unfocused. The transition occurred during
`sleep 600`, 258 seconds after the previous drive action, with no nearby
logged event establishing a cause. No launch flag or game code changed.

The later Wickmarket capture named `approx180` actually reads 09:34 on
the HUD, about 386 elapsed simulation seconds from its 07:00 start. The
frame-delta integration gives about 385.7 seconds. Its filename and initial
ratio estimate cannot be used as timing evidence. Compare stable matching
camera/settings intervals for performance and record actual times for visuals.

Do not press `T` to make a normal-speed behavioral screenshot appear to have
waited longer. The debug clock changes schedules relative to movement; it
does not restore frame throughput. Formal residence, movement and crowd-cost
measurements should use the pure headless engine. Hidden GPU runs establish
visual appearance and observed simulation state; stable matching frame
intervals can establish frame cost once the transient pacing state is excluded
explicitly and consistently from both sides of the comparison.
