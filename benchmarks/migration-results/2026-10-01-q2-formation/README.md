# Q2 shared formation and deterministic admission

A candidate keeps two completed target family/site assignments, including their
delivery plans. Surveys and repeated assignments borrow immutable target trees;
permissions, output verification, naming and artifact admission still run afresh.
Keys include all variable formation choices inside the fixed candidate/contract/
host owner. Eviction and cleanup are deterministic, including callback unwind.

`cache.formation_reuse=true` is the default; `false` physically executes every
formation with the same cache slots and logical reservations. Both modes admit
a repeated assignment as a complete stage using its measured work by kind,
peak memory and surviving backing. The reservation stays live through physical
execution. A recovered budget refusal prevents capture. This accounting is
versioned by policy algorithm 54: partially funded repeats stop before execution
instead of preserving the preceding compiler's individual cold refusal prefix.
It is an accounting change, not extra search permission. Receipt storage is
constant size. Two retained trees can raise memory peaks and affect tight limits.

## Qualification

[Eleven core checks](stage-tests.log) pass: shared targets, every objective,
complete-stage work/memory ceilings, physical-execution gating, baseline
protection, naming continuations and cleanup on error/unwind. The final removal
of a hot-path accounting hook passes [four affected checks](final-tests.log).
Configuration and build-cache audit vetoes passed earlier in the retained
[development tests](tests.log) and [build-cache check](cache-gate-test.log);
the former also includes two failures of the subsequently rejected trace design.
No complete library suite, fleet or ratchet rerun.

Release pin `q2-formation-5`, SHA-256
`0b1f138dbaba54cc46a3d96c1761b50f8335a10080bd172b1a6f938e49e0c7ae`,
algorithm 54 / search 32 / walk 10 / local facts 15. [Source identity](identity.json).
The [runner](measure.mjs) and [comparison](comparison.json) retain **81 checks**:
nine controls × raw/gzip/Brotli × preceding/current/current-with-reuse-disabled.
They cover hashed nested filenames, preloads, single/split/preserved modules,
lazy imports, setters, host wrappers and numeric rendering. Independent codecs
and Node observations agree. Cache on/off files, scores, search decisions and
full logical resource receipts match exactly. Every file also matches algorithm
53. Selected totals remain **2,292 raw / 2,046 gzip / 1,567 Brotli**.

Across these controls, 6–22 physical formations are avoided. Baseline work is
unchanged; optional work changes by 0–16,440 units and peak reservations increase
2,243–22,300 bytes. Every handoff has zero remaining charges. These tiny
compilations do not establish a CPU speed gain.

## Markedlil and negative evidence

[Three alternating pairs](marked-comparison.json), raw/effort 13, use the same
pinned sources and the maintained port's config with only codec/effort changes.
The [runner](marked.mjs) retains two Markdown API observations and requires the
preceding compiler's exact artifact hash. All six results are **31,882 bytes**,
with **354 judgments** and identical resource receipts. Physical formations fall
from **1,451 to 1,429** (22 hits). Median CPU is **55.90 seconds cold / 55.91 with
reuse**; samples overlap, so there is **no demonstrated markedlil speed gain**.
Most assignments differ. Broader changed-unit formation and Q3 candidate policy
remain necessary. Peak retained storage is 30,246,261 bytes in both modes;
all charges release at handoff. No port sources or distribution files changed.

The first proposed mechanism recorded ordered work/allocation events. Its first
library pair performed all 1,451 formations with no hits and took 64.75 seconds
for reuse-on versus 64.79 cold (a second cold sample took 64.23); the preceding
compiler's single sample was 56.12. Remaining samples were stopped. Packed
words and removal of zero releases still overflowed 1,441 captures, with zero
hits and 70.09/70.68-second samples. The packed probes are retained in
[one](marked-packed-overflow.json) and [two](marked-empty-release-overflow.json).
A repeated-sequence experiment added more complexity; its initial two-word
encoding failed the small cache-hit checks ([log](sequence-tests.log)). The
later one-word prototype was built but not qualified and is retained only as
[rejected development code](rejected-event-sequences.rs.txt). The event-log
implementation is removed from the compiler.

Constant-space stage admission first produced 22 hits, but still took 60.66
seconds ([probe](marked-stage1.json)). Removing successful work-charge hooks
and recording the work-counter deltas at stage boundaries produced the final
pin above. Earlier binaries remain separate development pins, never substituted
for the qualified compiler:

| Pin | SHA-256 |
|---|---|
| q2-formation-1 | 45912dc77c04d8fc4b80454c8a1530983fbfcc3759a8e8b2d72a015d6994f0d5 |
| q2-formation-2 | ac1cb4df94b35ff8f8debfdb238b06555c426ea75a068a86ce0ef8ddf1910714 |
| q2-formation-3 | 9bb1ef9094e237a98d0d8fed163542f96bc76c2bcdbd76b7afc2307ff0fa8d6e |
| q2-formation-4 | c54c64b3d844450e5f1fffdb14f3d488973d4892bdd03b545792aac739d1edda |

The development logs preserve compilation and test failures, rather than
counting them as qualification. An accidentally started stale test executable
was stopped and is excluded. Q2 remains active: global analysis/planning
admission, remaining dirty dependencies, changed-unit formation and per-module
elaboration are still open.
