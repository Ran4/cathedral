# Setup and helper provenance

`check-sim-01-sources.json` is an aborted setup enumeration, not a Cargo run.
The runner failed constructing its start record because system Python 3.10 lacks
`datetime.UTC`. No start/result/raw log was written and Cargo was not invoked.
The timestamp expression was corrected to `datetime.timezone.utc` for check-sim-02.
An earlier uv invocation failed before running the helper because its default
cache directory was not writable; subsequent invocations explicitly set
`UV_CACHE_DIR=/tmp/alibi-uv-cache`.

`run-check-sim-02.py` exactly reproduces the helper bytes recorded for check-sim-02
(SHA256 11aaf1773121e65b398a28fe8f501177baca48eaa6334d1cf2db0e7066265616). This variant was reconstructed by removing the later
`sys.dont_write_bytecode = True` line and verified against the contemporaneous
start record. It is preserved as a byte-exact historical helper, not a new run.
That single line was subsequently added before importing the source enumerator.
The generated component_input_sources.cpython-310.pyc cache from initial setup
was removed; all subsequent invocations set PYTHONDONTWRITEBYTECODE=1.

Development runs whose result says `sources_unchanged: false` are mixed-source
feedback only. Final frozen checks will be named separately. Failed runs and
lossless raw-log gzip archives are retained unchanged.
