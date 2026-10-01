# Q2 graph, effect and range admission

Call graphs, iterative alias resolution, SCCs, local dominance and initialization
schedules now have admitted builders. Source normalization owns current and
previous effect generations and releases them before publication. Declared-pure
checking uses a temporary admitted analysis; JavaScript demand owns its effects
and ranges until formation ends. Sorted occurrence tables replace local hash
maps. Range propagation no longer allocates the discarded per-operation flow
history: exact live branch/loop states release their backing on success, refusal
and unwind.

Nested constant-space stage receipts keep effect SCC/member work and memory
admission identical when `cache.normalization_reuse` is disabled. Both physical
modes retain dependency/stage metadata. This is policy algorithm **55**, with
higher bills for work/backing omitted by prior compilers. No search heuristic,
semantic permission, pure-termination rule or compression effort default changes.

Pin `q2-analysis-1`, SHA-256
`d74be18c449049cdea295968fd5b3259890448835896338392c3e5d1d5677eb3`. [Complete source identity](identity.json).

## Checks

**63 distinct focused checks pass.** [Initial batch](tests.log): 62 passed and
one new invalid forward-read fixture failed in the checker. Its replacement
exercises a 512-cell pending chain and synthetic cycle directly at the resolver.
[Follow-up](final-tests.log): that check and 17 affected flow/range/demand checks
pass. Coverage includes summaries, recursion, callbacks, initialization/TDZ,
parameter mutation, resource ceilings, nested receipt unwind, exact source
publication, and all-objective formation transparency. Initial type/lifetime
build errors and the corrected builds are retained in the build logs. No full
library suite, fleet or ratchet rerun.

[Release comparison](comparison.json), [runner](measure.mjs): **54 compiler
checks**, six controls × three objectives × prior/current/reuse-disabled. Files
match the preceding compiler exactly. Independent codec totals are unchanged:
**1,299 raw / 1,252 gzip / 947 Brotli**. Node observations pass for shared
modules/setters, lazy loading, embedded hosts, pure bounded loops/ranges and an
early read that must throw. Current on/off search and resource receipts agree.

The runner initially reached an invalid final source fixture after completing
45 checks. Those five controls' generated artifacts were recovered and encoded
again, without repeating their compilations. Their detailed resource/search/CPU
receipts were not persisted before the failure and are explicitly null in the
report; the initial comparisons had passed. The corrected sixth control retains
all receipts: baseline work rises 1,424 units, optional work 2,373 and peak
reservations 4,086 bytes in each objective; handoff charges are zero. A final
console-summary error happened after the full report was saved and was fixed
without rerunning compilers. Both [initial](measure-initial.log) and
[reporting](measure-resume.log) failures remain visible. The runner now persists
per-lane results before advancing.

## Larger workload

One [markedlil raw/13 probe](marked-probe.json) preserves the exact prior
**31,882-byte** artifact, **354 judgments** and two Markdown API observations.
It takes **55.24 CPU seconds**; the preceding batch's paired median was 55.91,
but this single sample is not a paired speed estimate. The current run retains
1,429 physical formations and 22 shared tails. Baseline work is 35,822,901,
optional work 7,071,782,766 and peak retained storage 30,854,238 bytes; all charges
release at handoff. Sources and binary identities are in the probe. No port
source or distribution files changed.

Q2 remains active. Class/aggregate/exact-value and rule-planning owners still
need admission, including default-verification and local-fact initialization
consumers. Remaining global dirty dependencies, changed-unit formation and
per-module elaboration are also open. This receipt does not claim complete
compiler allocation accounting or process-RSS control.
