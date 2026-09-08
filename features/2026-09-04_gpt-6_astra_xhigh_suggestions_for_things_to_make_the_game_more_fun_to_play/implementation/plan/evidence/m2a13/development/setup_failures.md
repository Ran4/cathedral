# Retained M2a13 development failures — 2026-09-09

- owner_initial: test harness named Admission::Fresh; existing enum is Admission::New. Compile failure retained; production check had already passed.
- owner_fixed: 13 passes/one failure, test attempted admitted encode on a LoadCandidate lease; correct production refusal. The corrected canonical comparison uses serde_json only inside test code.
- public_initial: 5 passes/one failure. Coordinator's direct ledger.advance setup left an unflushed receipt update. Production boundary refusal was correct; coordinator drains/asserts that update without another Engine poll. Public source correction and both maps are retained; production validation was not relaxed.

No failed original or provisional smoke is relabeled final. Generator creates only the four new component fixtures. Source map was frozen before final public/focused/debug/workspace commands. GPU unavailable evidence is inherited, never reprobed.
