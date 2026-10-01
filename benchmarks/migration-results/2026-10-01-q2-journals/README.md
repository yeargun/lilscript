# Q2 bounded target edit journals

1. Replace repeated unadmitted edit-ID pushes with admitted slot bitsets. A
   rule can record the same slot any number of times without growing storage;
   newly created nodes are owned by their existing parent edit.
2. Admit all journal storage before opening a rule, remap expression bits
   through the existing renumber owner, and release it on success or refusal.
   Keep exact edit counts and the existing independent debug journal checker.
3. Preserve structural rule ordering and output. This closes the journal owner;
   remaining delivery/analysis/edit allocation and dirty scheduling stay Q2.

Prediction: repeated edits consume bounded memory, and no output changes.
Batch the journal, renumber and target-rule checks after implementation. Compare
allocation failures and repeated edits, with no full-library or fleet rerun.

## Result

Target journals now allocate one bit per original slot instead of appending an
ID for every edit. Exact edit counts remain separate. A successful renumbering
moves the expression bits; refusal preserves the old map and releases its
replacement scratch. Rule completion and ordinary failure release the journal
while keeping target arenas charged. Debug checking consumes the same bitsets.

26 focused tests pass in 0.92 seconds after one test build (48.72 seconds).
The checks cover repeated edits, admission at partial-allocation boundaries,
renumber growth/removal/refusal, rule failure, captures, strict inlining,
array packing and retained-target/naming lifetimes. The repeated-edit fixture
records 10,000 mutations using 16 bytes of journal backing, unchanged throughout;
all parent reservations survive success/refusal. Only indentation was cleaned
up after the checked build. Algorithm 44 records the changed resource tariff.

This internal resource batch adds no representation or configuration permission.
No release rebuild, fleet rerun or size/speed claim. The preceding release pin
`q2-use-index-1` remains algorithm 43; Q2's eventual matched release qualification
must use a new pin. Journals are complete; dirty scheduling, placement/analysis/
edit owners, incremental formation and persistent caches remain Q2 work.
