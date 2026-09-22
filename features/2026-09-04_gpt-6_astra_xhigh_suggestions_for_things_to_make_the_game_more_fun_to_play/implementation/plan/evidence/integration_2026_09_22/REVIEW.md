# Workspace integration, 2026-09-22

The serial, offline workspace run uses the default Cargo home/compiler/target,
one build job and test thread, nice 15, headless/fake settings. `run.py` records
the actual start, HEAD, complete component input hash map, log hashes, exit and
whether input sources stayed unchanged. Each run name is create-once.

`workspace-01` failed production compilation: the M5 targeting guard called
`CollisionWorld::len`, which exists only under `cfg(test)`. No test pass is
claimed for that run.

`workspace-02` temporarily exposed that method to production. Backend library
210/0/3 ignored and simulator library 878/0/28 ignored passed. The game returned
640 passed, one failed, 11 ignored. The archived host fixture refused its
installed catalog fingerprint because the host definition intentionally hashes
all of controller.rs. Thus this temporary fix itself invalidated the definition.
The source maps retain the exact temporary source hash; it was not committed.

The final fix restores controller.rs byte-for-byte and changes targeting's
guard to count boxes plus convex prisms directly with saturating addition.
No archived fixture is rewritten and the compatibility validator is unchanged.
`workspace-03` verifies that final fix: exit 0, 2,360 passed, zero failed,
47 ignored across 46 test summaries. The archived initial/active host fixture
test passes unchanged. Root independently checked all three compressed/raw
log hashes, each unchanged-source result, and the final complete input map
against the current repository before committing.

The next source-capture patch was developed under /tmp and remained unapplied
through all these runs. Integration evidence does not validate that patch.
