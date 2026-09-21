# M3d diagnostic frame seam — 2026-09-21

This is bounded diagnostic capture orchestration, **not complete M3d frame
acceptance**. Whole-App admission, live checkpoint adoption and complete
renderer/codec/native allocation accounting remain unavailable. A PNG cannot
prove those gates.

## Source-backed correction

Ordinary F5 previously spawned a detached encode for every key edge. Drive used
Bevy's `save_to_disk` observer plus a separate unconditional saved observer.
Bevy logs conversion/write errors and returns from `save_to_disk`; its other
observer could therefore let Drive Quit succeed without a PNG. The local
Bevy 0.19 screenshot implementation also allocates transfer buffers before
delivering an image, and has no cancellation/worker-termination receipt exposed
to this host. We cannot infer released GPU ownership from a host timeout.

Both paths now use one shared busy slot across readback and encoding. Staged
startup pre-admits a disjoint 4 KiB allowance for this closed owner, status
Arcs and bounded retained output path. It is not an allowance for Bevy's
observer registry, ECS, pixel buffers, PNG encoder or GPU allocations. Those
remain explicit complete-frame accounting gaps, never estimates charged to
the world. The lease survives Requests removal through actual flight and
receipt ownership.

An ordinary diagnostic request requires the actual committed startup and
same-budget checkpoint control owner; enabled actors must also have the actual
active LocalEngine/Bridge endpoint identity. A real main-world RenderDevice and
one primary Window are required. Primary physical dimensions are checked before
requesting readback: each side at most 4096, total at most 4096*2160 pixels,
nonzero. Retained path capacity is at most 2048 bytes. Returned images are
checked again before the host pixel copy: one 2D layer/mip/sample, supported
8-bit RGBA/BGRA formats and exact expected byte length with bounded capacity.
The handoff constructs a fresh Image from only those validated bytes, extent
and format. It does not clone arbitrary render-only sampler metadata; a
regression with a one-megabyte sampler label measures only the pixel copy.
Resizing during Bevy readback can still allocate outside this host's bound;
post-readback refusal does not retroactively admit that allocation.

## Completion, failure and timeout

One observer hands the frame to one existing AsyncCompute task. The encoder
preserves existing RGB output semantics (HDR alpha is discarded). The receipt
becomes Saved only after conversion and the actual PNG write succeed, and
WriteFailed on errors. This is successful diagnostic file output, not fsync or
checkpoint durability. Drive's request-time resident JSON remains explicitly
request-time state; failures of either evidence output now latch a failed Quit.

A fixed ten-second monotonic wall deadline covers missing readback and encoding.
Timeout is terminal for the receipt. It does not release the busy slot while
an observer or worker survives, does not spawn retries and cannot be overwritten
by late success. A late observer skips pixel copying. Actual image/encoder
disposal precedes the flight's slot release. A hung GPU therefore leaves one
unavailable slot rather than an accumulating task queue. Drive's existing
independent watchdog still covers a host that stops ticking entirely.

Drive logs capture completion/failure before advancing to its next directive;
normal Quit checks the sticky capture/evidence failure as well as the existing
session evidence latch. F5 reports terminal completion/failure through the
existing diagnostic logger. Missing session/renderer, busy slot or invalid
request is an explicit refusal, not a successful skipped shot.

## Closed acceptance and safe testing

The separate checkpoint-frame acceptance gate requires valid startup/control
ownership, the main startup's actual headless/audio-disabled flags, an invisible
Window, fake cognition, TTS off and a renderer. It then checks complete world
admission and still refuses unproved renderer bounds. The gate never activates
devices/providers and never turns diagnostic PNG success into adoption
acceptance. Ordinary visible-play screenshots retain their diagnostic role.
No window visibility, cursor, microphone, audio plugin or provider setting is
changed by a capture request.

Tests use minimal Apps without renderer/window/audio plugins, 2x2 CPU images,
actual observers/PNG writes and fake/off staged services. They check terminal
error honesty, late completion, retained ownership after timeout, input bounds,
and no-renderer refusal before a screenshot entity is created. Known GPU
unavailability is not reprobed. Full rendered hidden-window evidence, device
failure recovery, complete GPU/codec/frame costs, live adoption latency and
post-adoption continuation remain pending.
