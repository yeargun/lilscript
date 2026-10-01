# G1 lexical naming completion

Final compiler `g1-complete-2`, SHA-256
`26a73739890c11d9534873969cba6c4a18fbf52e36d91d8f724bdacb89137b0c`.
[identity.json](identity.json) pins its complete Rust/Cargo source manifest and
codec. Policy algorithm 16 / search 31 / walk 10 identify the change.

The compact allocator consumes verifier liveness, follows printed declaration/
use order, preserves independent root/local frequency allocation and scope reuse,
and uses digits in continuation positions. Dead bindings and dead named
functions no longer reserve names in that alternative. The existing allocator
remains eligible. The common representation registry owns `NameAllocation`;
rendering, memoization, receipts and retained permission checks carry its choice.
Exact objective ties prefer complete raw bytes before recipe/naming identities.
The walk memo retains both scores; no unscored rename follows selection.

`naming-compaction` is automatic at 14, explicitly available at 13, and independently
vetoable. It needs `naming-search` and `identifier-mangling`; `naming-alphabet`
separately controls reordered alphabets. The extra stage follows the completed
search and protects its incumbent. This gate follows the negative size evidence
below; it does not claim the strategy earns bytes on these workloads.

## Focused qualification

**39 distinct focused checks pass.** [batch.log](batch.log),
[prefix.log](prefix.log), [live-functions.log](live-functions.log) and
[permissions.log](permissions.log) retain the outcomes. Two older scheduling
assertions needed to account for later stages: the deferred tests now isolate
that strategy, and the local-order test compares the correct completed prefix.
Their corrected checks pass. The new allocation test independently verifies its
protected prefix and remeasures the delivered bytes under every objective.
Coverage includes dead slots/reservations, identifier length boundaries and
uniqueness, >54 locals, unrelated scopes, captures, reflection, direct eval,
imports, independent controls, replay admission and registry risks.

[measure.mjs](measure.mjs) reproduces [comparison.json](comparison.json) against
immutable Q1. Its 18 small workload configurations preserve every individual
raw/gzip/Brotli objective size: all deltas are **zero**. All 108 independent
old/new Node executions pass, and measured files match reported selected scores.
The ordinary 15 rows, explicit recurring-sharing row, explicit startup row and
effort-14 row are reported separately. This includes S3's captured-activation
case: its earlier local +2 Brotli against S2 is **not recovered**. That remains
negative naming evidence for V1, not a new semantic defect or a claimed win.

[medium.mjs](medium.mjs) and [medium.json](medium.json) add the frozen C3 training
module `arithmetic-128` (16,914 source bytes), with eight independent Python
oracle inputs per output. Its selected sizes remain **8,763 raw / 3,986 gzip /
2,743 Brotli**, with all 48 old/new input checks passing. The protected evaluation
set was not used. This is development qualification, not Closure or fleet parity.

Small-cohort work (54 objective cells):

| Counter | Q1 | G1 final |
|---|---:|---:|
| Structural proposals | 180 | 180 |
| Structures | 78 | 78 |
| Structural renders | 234 | 234 |
| Structural codec probes | 86 | 86 |
| Terminal judgments | 416 | 422 |
| Terminal positions | 6,494 | 6,582 |

The medium module's work is unchanged: 4 / 4 / 17 terminal judgments for raw /
gzip / Brotli. These are logical counts, not CPU claims. Candidate 1 enabled the
extra allocation at 13 and spent 437 judgments on the small cohort, without
saving bytes. Its [comparison](candidate1-comparison.json),
[identity](candidate1-identity.json), `candidate1-*.js` files and separate
[80-helper probe](helper-holes.json) retain that negative evidence. This motivated
the automatic-14 gate. The final controls and final binary were then checked.

## Existing refusal found during qualification

The other selected C3 training module, `records-128`, is rejected by **both Q1
and G1** before JavaScript naming with `semantic operation type mismatch:
IntBinary(Add)`. [medium-preflight.json](medium-preflight.json) confirms it also
fails with inlining or scalar replacement disabled. A [frozen source](record-type-reproducer.lil)
and [prefix reduction](record-type-reproducer.json) show 64 helpers pass while
128 fail. No size or behavior success is claimed for this module. **S4 owns
fixing the fact/type transport failure** before its completion and V1 qualification.

No full-library/fleet rerun, accepted ratchet change, port rebuild or native
qualification was performed in G1. The generated configuration reference and
`git diff --check` pass.

## Class-owner audit follow-up

Before G2 changes property allocation, its constructor audit found that compact
printed order must explicitly descend through `Expr::Class` constructors and
methods: expression children alone only visit the base. The source follow-up
adds those owners, with a [passing independent Node check](class-owners.log)
for constructor/method parameters, captures, names and arity. This is the 40th
distinct focused check. It prevents a refused optional compact trial on such
classes; the verifier had rejected the incomplete assignment, so it could not
be delivered. Policy algorithm 17 identifies this correction. The immutable
16/31/10 pin and cohort evidence above remain unchanged; the correction will be
included in G2's next release build instead of repeating the entire G1 cohort.
