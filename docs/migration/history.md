# Migration evidence

Records for the focused plan adopted on 2026-09-29. Earlier batches, including Y5, are preserved in [old-history/migration/history.md](../old-history/migration/history.md); the still earlier milestones are in [record-2026-09.md](../old-history/migration/record-2026-09.md).

## 2026-09-29: focused plan and archive

**Authorization.** The owner accepted the architectural review and requested an `old-history` archive and a fresh, focused migration plan, emphasizing configurable TOML behavior and the compilation-time versus compressed-size tradeoff. This authorizes the planning revision; it does not mark its implementation tasks complete.

**Starting point.** Repository commit `8b3d5cc5a1429919b6000831085ce807ec9aeed3`, after Y5a/b. The accepted ratchet pins `y5-1`, SHA-256 `d786769853487741caa076aeecbfca41a1937da2f211607434c8337ce02c6d9e`. The prior record labels Y5 as 2026-09-30; that historical date is preserved. This planning record uses the actual UTC date of the revision.

**Changes.** Archived the prior design, migration documents and stale status/architecture snapshots with source hashes. The active plan now includes the architecture and configuration contract in one place. The checklist holds progress; the coverage map assigns every unfinished legacy task and carried defect. Active entry points point to this plan, and old section references point to the archive.

The plan brings flag enforcement and usable measurement forward, interleaves mangling with structural optimization, gives gzip its own calibrated policy, and makes Closure parity a combined qualification milestone. It replaces universal compressed-size and runtime claims with semantic proofs, measured defaults and workload evidence. It specifies bounded exploration with a protected incumbent, independent objectives, incremental work and explicit compatibility for the current level-16 startup grant. It retains the native commitment without making native completion a prerequisite for JavaScript compression work.

**Evidence used.** The previous plan/checklist, owner briefs, Y5 results, source configuration and family implementation, and mangling research. Research suggestions are hypotheses, not adopted default thresholds. Current ratchet totals against Closure ADVANCED:

| Corpus | Raw, ours / Closure | Gzip 9, ours / Closure | Brotli 11, ours / Closure |
|---|---:|---:|---:|
| Applications, 7 | 1,117 / 1,332 | 941 / 982 | 764 / 834 |
| Algorithms, 11 | 5,076 / 4,522 | 3,195 / 3,031 | 2,900 / 2,703 |

These are the [existing accepted baseline](../../tests/ratchet/baseline.json), not new compiler measurements. Y5's prior record contains the markedlil 14 → 46 exact judgments, the 13-byte Brotli difference against V1, the port results and the limits of the runtime evidence.

An isolated JavaScript compression check also refuted the raw-bound implication: for `const a=[1001*2001,...,1032*2032];`, replacing each product with its exact result changed raw 330 → 266, gzip-9 148 → 155, and Brotli-11 86 → 113 bytes. This is a check of that mathematical implication, not a full compiler benchmark or a claim that this is the best representation of the data.

**Verification.** All 11 archived documents match their source revisions apart from the archive banner and navigation links, with hashes recorded. All 95 unfinished legacy tasks and 43 carried entries have owners among the 21 work packages. Local Markdown paths and anchors pass validation. The TOML example is parsed and accepted without warnings by the pinned Y5 compiler's `--print-policy`. The historical receipt validator now reads the archived milestone table; its 17 existing tests pass. `git diff --check` passes. No compiler rebuild, benchmark rerun, accepted-baseline update or port-output change is part of this documentation revision.

## Recording subsequent batches

Before implementation, record task IDs, numbered changes, predicted size/work/runtime effects and verification scope. After verification, append pinned identities, actual results, explanations of surprises, replacements deleted and open work with owners. Keep full reports in the established evidence locations; link them here instead of copying their output. A documentation revision does not satisfy an implementation gate.

## 2026-09-30: C1 controls, first implementation batch (in progress)

**Authorization and baseline.** The owner requested complete implementation of the revised plan. Planning and archival work is checkpointed at `f4c181e5`; compiler behavior starts at Y5 (`8b3d5cc5`). This batch begins C1; it does not complete the other work packages.

**Pre-registration.**

1. Separate terminal string pooling and string-array packing, declare their permissions with the output families, and carry those permissions through artifact provenance and replay. Project initial seeds onto permitted families; reject explicitly forbidden assignments. Preserve the old combined alternative as a joint move where both permissions allow it.
2. Audit and route the remaining optional JavaScript rules through their owning tactic, including head formation, tail formation and reused heads. A hard veto must cover output lowering as well as the semantic optimizer.
3. Cover direct output, searched output and cached admission with focused configuration and behavior oracles. Update the public configuration reference to match the enforced behavior.

**Prediction.** With unchanged permissions, the rule schedule and prior combined pooling alternative remain available. Independent pooling/packing alternatives may improve bytes and add bounded trials. Explicit vetoes may grow output and should reduce corresponding work. No intended runtime change for unchanged choices; string splitting's existing runtime classification needs its own audit and is not proved neutral by this permission fix. Verification: formatting, compiler unit tests for formation/search/provenance and independent Node behavior checks, followed by the relevant configuration/case and objective ratchet lanes. Only one heavy job runs at a time. Record actual results before accepting the batch.

**Implementation checkpoint.** `8bb9dc3c` wires independent output families and fixed target rules through formation and retained admission evidence. The compiler builds. Initial focused checks passed seven cases; the eighth fixture used a reserved identifier and was corrected. The full debug test run exposed an alternate-seed veto regression (fixed), then aborted in the existing deep checker test on its thread stack limit. The rerun uses a 16 MiB test-thread stack. Acceptance and measurement remain pending; this checkpoint is not a claim that C1 is complete.

**Library verification.** The corrected C1 compiler passed 1,606 tests, with nine existing ignores; 38 native tests failed because Clang was absent. Installed the Ubuntu Clang 18.1.3 compiler, common headers and runtime packages into `~/lilscript-work/toolchains/clang-18/root` without changing system packages. All 38 failed tests passed when rerun with `LILSCRIPT_NATIVE_CLANG` pointing to that compiler. Combined result: **1,644 passed, nine ignored**. Logs: `/tmp/lilscript-c1-unit-tests-stack.log` and `/tmp/lilscript-c1-native-rerun.log`. Size qualification remains pending.

**Generic size gate.** Built `8bb9dc3c` in an isolated checkout and pinned `c1-1`, SHA-256 `aab3f7213aed3fce33f591cea074b86d3a9709d029aaf0d5aa2f11f450124f2c`. The [642-item, 1,926-lane ratchet](../../benchmarks/migration-results/2026-09-30-c1-controls/README.md) passes: every corpus/objective total matches Y5, no new problems or improvements, and the same 36 ledgered failing lanes. No baseline update. This accepts the tested control slice, not completion of C1 or a claim about compile/runtime speed.

## 2026-09-30: S1 constant evaluation

1. Add bounded evaluation of exact primitive builtin methods, sharing the existing exact arithmetic semantics. Replaceable builtins require the resolved pristine-builtins contract; unsupported operations and resource limits return unknown. Preserve UTF-16 strings, integer wrapping, negative zero and throwing cases.
2. Evaluate small functions at each constant call, with explicit arguments and bounded work/depth. Only a completed interpretation of supported operations certifies a removable call; unknown captures, effects, recursion limits and unsupported control flow keep the call. No change to `pure` termination semantics.
3. Intern newly computed strings through the edit owner and correct the claim that a raw text bound proves a compressed-size win. The bound remains a default heuristic pending objective calibration, not a theorem about gzip/Brotli.

**Prediction and checks.** More calls and branches disappear when their arguments differ between constant call sites; runtime work decreases. Raw and codec totals must be measured independently because constant literals can lose repetition. Evaluation is bounded and shares the existing analysis work limit. Check independent interpreter/Node oracles including surrogate halves, negative indices, patched methods, throwing inputs, mutable captures and nontermination; run the relevant rule/configuration suites, then the generic objective ratchet with a pinned compiler. C1 verification continues in the already-built test process while this independent slice is developed.

**Result.** Implemented at `8826918a`; pinned `s1-1` has SHA-256 `4bd8b39b22560cd2024b77a5d2a6fac7a34a53e6e191382b0e4383397d3514f0`. The [evidence receipt](../../benchmarks/migration-results/2026-09-30-s1-evaluation/README.md) contains the full reports. The generic ratchet improves by **203 raw, 172 gzip and 165 Brotli bytes**, with no new regressions or failures. The supported `--update-baseline` run accepts these tighter totals. All 401 production cases × nine lanes pass or match the existing ledger/masks. The full library run passed 1,649 tests and found one recipe fixture requiring unoptimized helper sites; after its test-only bypass was corrected, that fixture and all 39 rule tests pass. Nine prior ignores remain. The evaluator replaces calls only after a bounded completed interpretation; unsupported methods, unknown captures and exhausted work stay unknown. No language termination contract changed.

**Remaining.** S1 still owns broader supported evaluation and propagation/default/removal coverage. C3 owns calibration of the raw literal-size heuristic. The three frozen port main entries are byte-identical against C1, with similar paired compile times; this is not full runtime or fleet qualification.

## 2026-09-30: C2 paired compilation measurements

Add a manifest-driven, sequential paired compilation runner. Pin both compiler binaries, source/configuration identities and the codec; retain every measured CPU/wall sample, its order and load, final artifact hashes, exact sizes and the warm-up explain reports. Fail on changing inputs or outputs. Keep warm-up, phase telemetry and codec measurement outside the timed samples. Use it first on the existing frozen main-entry workloads to compare C1 with S1 and to support Q2's repeated-work investigation. This changes measurement infrastructure only, with no intended compiler behavior change. Verify it against real pinned binaries; no speed claim is accepted from an unretained median alone.

**Result.** `scripts/compile-cost.py` passes syntax checking and a real three-workload run with three alternating measured pairs each. The [retained report](../../benchmarks/migration-results/2026-09-30-s1-evaluation/compile-cost.json) pins all samples, warm-up telemetry, transitive inputs, effective policies and exact artifacts. A review corrected the policy comparison to use the explain report's actual JavaScript/native policy fields; equality was verified on all retained warm-ups. Median candidate/previous CPU ratios are markedlil 1.001, zodlil 1.000 and posthoglil 1.013; no broad speed claim. C2's runtime runner, controls and full competitor recipe contract remain open.

## 2026-09-30: Q2 exact scoring reuse (pre-registration)

1. Reuse completed per-file compression measurements across discarded candidates, keyed by the exact byte digest and complete encoder settings/role within one compilation. Bound retained entries and account for their storage. Retain no eligibility or winner in this cache.
2. Keep the cold encoder's logical work charge on hits and check the cold scratch requirement, so a hit cannot buy more search or bypass resource admission. Record physical encodes and reuse separately. Failed encodes never become cache entries.
3. Preserve candidate order, incumbent protection, provenance and final admission. Test different bytes/settings, multi-file separation, eviction, vetoes and exhaustion; measure unchanged artifacts and search decisions with reuse enabled/disabled, then alternate pinned CPU samples on the frozen workloads.

**Prediction.** Exact sizes and chosen bytes remain identical; repeated byte streams avoid encoding CPU. Cache storage and digest lookup add bounded work. If markedlil does not repeat byte streams, this mechanism will not explain its cost; formation reuse remains Q2's next investigation. No runtime change is intended. One heavy measurement/build/test job remains the host rule.

**Implementation checkpoint.** A 256-slot compilation-owned table now retains only byte-digest/settings/role keys and completed measurement receipts. Lookup and cold encoder work are charged; replay checks cold scratch capacity without reporting a physical allocation. Storage is released by the artifact owner. Codec/artifact suites pass 22 tests; broader search/service suites pass 29 with one existing ignore. A final audit test proves actual cache hits, identical logical work, search decisions and delivered bytes with physical reuse enabled/disabled under all three objectives. Release measurements and the unchanged-output gate follow this checkpoint.

**Result.** `11f249c3`, pinned `q2-1` (SHA-256 `1ace4226c23f44eda3c498018869a4eff3c7e3be31340dd671718f07f3d5e4e6`), passes the complete generic ratchet with unchanged totals and hash prefixes in every passing lane. [Receipts and retained paired samples](../../benchmarks/migration-results/2026-09-30-q2-score-reuse/README.md): markedlil median CPU 4.200 → 4.065 s (×0.968), zodlil ×1.002, posthoglil ×0.993, all three outputs byte-identical. Markedlil avoids four exact and four proxy encodes; its other candidates are distinct streams. No search opportunities were removed. Formation reuse and dirty scheduling remain Q2 work.

**Delivery fixture repair.** The bundle contract passed, but exact file-plan checks first failed because S1 folded the shared constant calls out of `split` and `lazy-cycle`. Reproduced with the S1 binary. Their configurations now veto constant folding to keep testing shared-file placement; the relocated CLI fixture carries the same setting. Existing behavioral and placement expectations are preserved. The full bundle/CLI/all-target/package-lock script passes with local Clang 18.1.3.

## 2026-09-30: G1 measured naming alphabets (pre-registration)

1. Make the identifier alphabet explicit data in the existing naming plan, with the sequential alphabet as the unchanged allocator seed. Validate and retain its identity with naming provenance and search memo keys.
2. Add a bounded joint naming move that ranks identifier characters by their frequency in the current delivered JavaScript bytes. Keep the sequential alphabet available. This is a candidate-generation heuristic; each objective judges the complete renamed artifact with its own exact codec, under `naming-search` and `identifier-mangling` permissions.
3. Cover hygiene/captures, source-only and seed-only vetoes, deterministic ties and exact delivered scores. Measure all three generic objectives and perturbed generic cases, then affected port suites. No library name or threshold enters the heuristic.

**Prediction and order.** Raw bytes should be similar; gzip/Brotli may prefer different repeated spellings. Two additional joint alternatives at most per pass add bounded formation/scoring. Q2's first measured reuse slice finds only four repeated exact encodes on markedlil, so naming work proceeds on the existing target while deeper formation reuse remains owned by Q2. No runtime operation changes are intended; this does not claim measured runtime parity or calibrate a new default alphabet.

**Control and schedule.** Add the dedicated `policy.tactics.naming-alphabet` permission, still requiring identifier mangling and naming search. Walk schedule v3 appends the alternatives. Paired compilation measurements must explicitly record this strategy change; matching source, contracts, codec settings, effort and resources remain mandatory.

## 2026-09-30: Q2 output-owner lifetime repair

The first full library run with G1 found 84 retained-byte assertion failures and one outdated timing test. Callback-local measurement had allocated the memo in the retained compilation arena; retained searches also left its table after their last artifact was dropped. Move scoped measurements into their callback's arena, and release a retained arena's memo when its last artifact is taken or discarded. Keep it between discarded trials while an incumbent remains live, preserving the intended reuse. Existing exact ownership assertions remain unchanged; add a final-artifact take/discard regression. Update timing assertions to account for reuse separately from physical encodes and for the two new buckets. Full verification follows; no size or speed result is inferred from this repair.

**Verification.** The corrected full library run passes 1,657 tests with nine existing ignores; its remaining handoff test assumed that taking the final winner releases only its text buffer. Updated that expectation to include idle cache storage and reran it successfully. Combined result: **1,658 passed, nine ignored**, including G1, timing observational equivalence, cache-hit/cold logical equivalence, scoped-output cleanup and the new exact final-artifact storage regression. Logs: `/tmp/lilscript-g1-unit-tests-fixed.log` and `/tmp/lilscript-q2-lifetime-rerun.log`. Compiler tests use the local Clang 18.1.3 and a 16 MiB debug test-thread stack.
