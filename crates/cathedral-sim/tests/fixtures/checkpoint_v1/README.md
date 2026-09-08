# V1 private component fixtures

These pin the real M2a1 owner wire format: an accepted timed operation and its
ledger, an exact manifest component, the clock segment, and accepted host time
with 400 ms ordinary debt / 15 ms fixed residual / 25 ms simulation residual.
They are **not complete supported saves** and must not be used as M3 slot files.
World/characters, Round/residents, knowledge, law and pending external-work
owners must join the complete M2a envelope before full save fixtures exist.

The normal `supported_component_fixtures` test validates and byte-compares these
with production-owner exports. Intentional changes can regenerate them through
`cargo test -p cathedral-sim --lib regenerate_checkpoint_component_fixtures -- --ignored`.
Existing prompt/snapshot golden fixtures are unrelated and unchanged.
