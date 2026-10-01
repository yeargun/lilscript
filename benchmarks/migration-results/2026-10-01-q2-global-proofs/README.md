# Q2 dependency scheduling and completion

Policy algorithm 63; preceding commit `a43db466`. This closes Q2 under its
explicit [scope correction](../../../docs/migration/q2-acceptance.md).

- Six target-local passes now retain complete cold-stage receipts qualified by
  all semantic dependencies and every journal arena length. Cache on/off modes
  admit identical work/peak storage before reuse or physical execution. Empty
  passes leaving retained metadata cannot certify reuse.
- Three newly classified passes cover double negations, empty declarations and
  bare blocks. Their traversal/mention storage is admitted; cloned expression
  payloads and block-splice growth belong to the target owner.
- Whole-program passes use the existing stable-suffix scheduler. The attempted
  additional global cache was rejected: it cannot hit if the suffix scheduler
  is working. The initial experiment and failures are preserved here.
- Public resource reports explicitly label target as well as frontend allocation
  coverage partial. Counted budgets are not process-memory caps.

## Qualification

The final focused batch passes **21 checks** in 23.18 seconds. It covers every
mask and journal arena, a real narrow-proof hit across an unrelated edit, complete
stage refusal prefixes, source publication cleanup, cold/reused source decisions,
print formation, all-objective search/admission parity, timing observation and a
native captured-bank case. The installed Clang 18 was selected explicitly. No
full library, fleet or release measurement was repeated for this internal batch.
The later report-label addition is a reviewed metadata change, not separately
rebuilt. The initial build needed a method visibility correction.

`focused-initial.log`/`qualification-initial.json` preserve the first experiment:
19 pass, two fail. One attempted to certify a no-op producer that retained
metadata; the other found zero hits for the redundant global source cache.
`rejected-global-cache.patch` preserves that abandoned experiment. The final
implementation removes it and narrows capture eligibility; this is negative
evidence, not a hidden passing claim. `focused.log` and `qualification.json`
record the corrected batch. `run.py` reproduces the connected selection.

## Completion boundary

Previous Q2 evidence already covers codec/build persistence, explicit decision
locks, target/source owners, delivery/rendering, dirty SCC/value/class facts,
shared heads/tails and changed print-site formation. The current recorded
markedlil trace spends 73 ms in four demand analyses and 42,635 ms in target
rules; arbitrary source-fragment relocation would not address that repeated
family-tail cost. Source-only fragment keys are also unsound across global
storage/effect/interface dependencies. The plan now requires actual dependency
closure and uses the implemented safe boundaries. Per-module elaboration remains
explicitly open in **D3**, whose shared graph/session owns source lifetimes and
interface invalidation. Neither capability is falsely marked implemented.
