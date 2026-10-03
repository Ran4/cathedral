# Installed lifecycle repair — 2026-10-01

Status: source prepared and lightweight tests passed; independent review and
restart of the idle installed worker are pending. No service operation was run
by the repair author. The original setup review remains historical evidence.

Live installation exposed two controller defects missed by fake-state tests:

1. `systemctl show -p=LoadState` on installed systemd 249 exits successfully but
   selects the literal property `=LoadState`, returning no field. The controller
   now uses `--property=LoadState` and the same syntax for every requested field.
   A read-only actual query returned `LoadState=loaded`, `Nice=15`, and host
   `MainPID=1109123` during preparation.
2. This client's PID namespace cannot resolve that host PID. Controller checks
   no longer call `getpriority(host_pid)` or read `/proc/<host_pid>/cgroup`.
   They require the exact systemd ControlGroup, positive MainPID, configured
   `Nice=15`, and live sysfs `cpu.max=40000 100000` / `cpu.max.burst=0`.
   The current invocation's startup receipt must also report observed nice 15
   and a worker SHA256 matching the current source file. Worker startup adds
   exactly one receipt field: `nice = os.getpriority(os.PRIO_PROCESS, 0)`.

The receipt observes actual priority at worker startup; the unit Nice property
is configuration, not a fresh observation of the process's current priority.
Existing old receipts lacking the field fail verification until the reviewed
idle worker restart. Stop does not require a startup receipt. No unit or installer
bytes changed, so this repair requires no new privileged installation.
Worker queue, admission, refusal, cap validation and execution policy are unchanged.

Validation: 14 control tests and 18 worker tests passed. Added checks cover the
real property-argument form, inaccessible host PID without client priority lookup,
wrong/missing receipt nice, stale/missing worker hash, and the worker's own-PID
priority observation. Existing no-match, refusal, idle lock and lifecycle tests
remain passing. `git diff --check` passed. No Rust, installer or service operation
was run. The fake-worker startup test uses a temporary queue and mocked cap/stop;
it does not launch a job or real service.

Frozen SHA256:

- `control.py`: `209821cbaa0a1adfbe0cd80c60006615a6c9c192cdf79be2fa34e4cea8e1de99`
- `worker.py`: `b16603ff4ddd7a764c2d1191d15c548ad1c75574695024ff66528577725af38d`
- `test_control.py`: `9818f19cd54a3aa4d102980e96a20fad748ed1e4bef5dc2842e67efd2e339d17`
- `test_worker.py`: `c51269a1153fbcc0f93dd720820472dd8bcae64f3999008ee328700db8348969`
- `README.md`: `d1f40f4ec10107bf14292d2b3400a3801149cc290d880c7a3f15d3b08961f95b`
