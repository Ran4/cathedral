# Independent permanent-worker setup review — 2026-10-01

**APPROVED_FOR_ONE_TIME_ADMINISTRATOR_INSTALLATION.** Final frozen-source review found no remaining blocker in the fixed worker lifecycle. This approves the installer below; it does not claim installation, live cap verification, Rust execution, or market acceptance.

```sh
sudo sh /home/ran/src/rust/cathedralbevy/scripts/capped_verification/install_capped.sh
```

## Reviewed behavior

- The installer accepts no arguments and uses isolated system Python. It validates four fixed unit hashes, validates those exact bytes in a root-owned temporary directory, and installs them at fixed `/etc/systemd/system` destinations. It rejects differing units, overrides, active units/jobs, populated old cgroups, pending/staging work, unresolved claims, STOP and outstanding lifecycle requests.
- Root opens the existing queue lock read-only and holds it through idle inspection and enable/start. Repository control-file creation runs only after dropping UID, GID and supplementary groups to `ran`. There is no root chown/chmod through user-mutable directories and no user-supplied command executed as root.
- The main worker runs as `ran:ran`, with the 40% CPU quota, 100ms period and nice 15. Both control paths use `PathChanged` on fixed persistent markers. The sole root stop command is `/usr/bin/systemctl --no-block stop cathedral-alibi-build.service`. No Polkit grant or passwordless-sudo policy is added.
- Control serializes lifecycle requests and holds the queue lock after an idle scan, closing the active-verification stop race. Restart requires a new completed stop-broker invocation, no pending main job, an unpopulated/absent cgroup and rearmed paths before starting. Durable outstanding intent prevents a timed-out stop from being silently overtaken by a subsequent start.
- Start waits through transient job state and checks the exact live quota, zero burst, nice 15, PID cgroup and current invocation startup receipt. Journal filtering occurs before the one-result limit; empty no-match exit 1 waits, while actual query errors fail. The overall deadline covers subprocess calls.
- Crash between intent persistence and marker write, and unresolved intent spanning reboot, intentionally require evidence review. README documents this limited recovery case, idle-only lifecycle controls, indefinite retries of other service failures, and the separate queue-drain STOP behavior. Job receipts and STOP are not erased by lifecycle control.

## Validation and limits

The builder reports **12 lifecycle tests and 17 existing worker tests passing**, plus shell syntax, systemd unit verification and diff checks. Those checks were not repeated by this reviewer. The reviewer independently inspected the final source and all previously reported fixes, verified the three supplied final hashes, all four installer unit pins, and the unchanged worker implementation hash. Lifecycle tests use fake state; they do not establish installed service behavior.

No installer, service operation, Rust command, worker main or game was run by this reviewer. No game source, owner runner or HEAD was changed. After installation, inspect live service/cgroup/nice and queue state before submitting any job; a tooling commit also requires refreshing the final HEAD review. The market03 source-only pin review remains distinct from execution authorization and acceptance.

## Frozen files

Reviewed UTC: 2026-10-01T18:21:41.794161+00:00

| File relative to `scripts/capped_verification` | SHA256 |
|---|---|
| `control.py` | `8cd540bdb43bf026978b60193fe37d493f3c77256c49e5edf8bea4d876706f48` |
| `install_capped.sh` | `a64a1f54b245c4605bb7990e75aafdad1789fcc1e49d39cea2c7bf9c7dadfb64` |
| `test_control.py` | `c36d16cee102ca4e69002a5c7e82073621e341e59aab47e77e37b6d2885ab6f9` |
| `systemd/cathedral-alibi-build-start.path` | `9a069eb9f206d1d5bfeb2e167442cdd26202355de9532f75023f39962ff98f18` |
| `systemd/cathedral-alibi-build-stop.path` | `964c26c2e5c9615130ae837e26cab32646b6e36af7c0fb301b2959ccfa16e3bd` |
| `systemd/cathedral-alibi-build-stop.service` | `b65dd65f4cb76e230bc632f53e57635fd9f12696d1a0d37157798f66750b660f` |
| `systemd/cathedral-alibi-build.service` | `a690e4cd1724faf115278c3bab92a2ad84ec2a8e7c91e3e65e5f2ad42920d429` |
| `start_capped.sh` | `dc0f0d3833f6cc33a724504c4ec2c6434ce4c3b21a6587c4956785bc5eb336e2` |
| `README.md` | `9db62c738267ed793f342309b23920b4a9905eec5c77f231a771274a84be3834` |
| `worker.py` | `fbf3bc3d4466b0ce1bdb0940aad3fd533c7461a870a6eee84e34a5381dcf6a75` |
| `test_worker.py` | `734c595dd5655c57ce84598ee33a45e93e28901f0a4b9fa82c6ce2d08f55163a` |
