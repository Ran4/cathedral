# Independent host admission review

The [library audit](allocation-library-audit.json) verifies eleven installed
allocator/parser source files and the exact compiler against the earlier proof.
It also retains the actual Arc header and stable-sort allocation excerpts.
This is an allocation bound, not a measurement of process RSS or complete-save
coexistence. Final workspace and release results are recorded separately.

The generic charge remains `4096 + 4 * expanded + 3 * encoded`, with the
unchanged 128 MiB encoded/expanded and 1 GiB shared limits. The extra host charge
is 4,096 bytes per lexical container plus 4 MiB of sequential validation work.
Raw input is charged before traversal, and row/container growth is charged before
private typed decoding. Export checks the actual output again after measuring
a potentially mutable borrowed source; its output writer cannot grow beyond
the premeasured capacity.

Every closed host row has both an outer tagged object and a `data` object.
The concrete wire test requires `size_of::<RecordV1<String>>() + 64 < 4096`.
Those two containers supply at least 12,288 bytes of peak structure allowance
per row: 8,192 extra host bytes and 4,096 generic expanded-copy bytes, before
counting fields or strings. This covers the decoded Vec's geometric capacity
and the separately temporary stable-sort buffer even at three inline rows of
storage per element. The sort's fixed small-input scratch fits the separate
4 MiB allowance. Sort scratch is gone before validation indexes are built.

Ordering validation borrows receipt text in three BTree indexes. On the verified
64-bit layout, even charging a whole 376-byte internal node per entry for each
index remains below the same per-row allowance alongside twice the maximum
inline row storage. Other textual validation copies are covered by the generic
string/encoded-copy allowance. Ledger validation and bounded journal formatting
run sequentially; expected publication bytes are streamed against the candidate
instead of collected into an unbounded second output. Original whitespace and
serde escape scratch retain their charges through candidate validation.

The speech receipt charge reserves all cloned String lengths, the retained
message wrapper and the two-word Arc header before cloning. The actual
`PresentSpeech` fields need no alignment beyond a word on this target. Receipts
share their charge across captions, subtitles and bubbles; the trailing lease
drops after the message's Strings. The 8 MiB receipt allowance is separate from
the small fixed counter/state bookkeeping and from checkpoint cohorts. Failure
to retain an original preserves ordinary display/audio and refuses capture
while that committed readable authority survives. It never creates an early
speech acknowledgement.

These checks establish this component's ownership and allocation policy. They
do not account for all Running, Save, Load and retiring allocations together;
the complete envelope must solve that separate lifetime problem under the same
limits.
