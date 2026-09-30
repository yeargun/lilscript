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
