# M3d owner handoff

This cut implements bounded diagnostic request/completion ownership and honest
drive screenshot outcomes. Complete checkpoint frame/render/adoption acceptance
remains explicitly refused; see [DESIGN.md](DESIGN.md).

- New `screenshot/requests.rs` owns a shared single flight, monotonic deadline,
  terminal receipt and staged 4 KiB fixed metadata allowance. Dimension/layout
  preflight bounds the host handoff; full GPU/codec allocations remain unproved.
- F5 and Drive share that owner. Drive completion follows the actual PNG write
  result, replacing the independent unconditional saved observer. Refused,
  failed and timed-out captures latch failed Drive Quit, as does failed resident
  evidence output.
- Timeout neither frees live callback/worker capacity nor permits late success.
  Old receipts retain their own terminal outcome and metadata lifetime.
- The separate acceptance API checks actual startup/control identity and
  hidden/fake/off execution before requiring world/renderer admission. It
  cannot return a complete acceptance on the current implementation.
- Minimal tests use actual CPU-image observers and file writes, lease retention,
  missing-renderer refusal, late completion and image layout preflight. Existing
  startup, control and Drive scheduler tests are included.

No GPU/provider/device probe, visible window or audio execution occurred. No
historical fixture or coordinator test was changed. Existing dirty LOGS_FOLDER
and prior owner handoffs plus unrelated untracked paths remain preserved.

Evidence runner retains exact start maps/environment/helper hashes, stdout and
stderr raw bytes, and deterministic mtime-zero gzip.

`focused-01`: 37 passed / 0 failed / 0 ignored; source unchanged. After this
run, source review tightened the Image handoff to copy only admitted pixel
bytes with fixed metadata, avoiding ImageSamplerDescriptor's arbitrary owned
label. The new allocation regression covers a one-megabyte label and zero
allocation on rejected dimensions. Bevy's actually supported conversion
formats are used. `focused-02` is the final verification of that source.
