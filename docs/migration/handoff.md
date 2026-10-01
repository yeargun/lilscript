# Migration handoff — S2 complete, S3 next

The owner authorized committing/pushing S1 and completing S2, with focused
checks after substantial implementation batches. Do not restart the stopped
S1 qualification chain or repeatedly run the entire library suite. This
checkpoint completes S2; it does not start another milestone.

## Current state

- Checkout: `/home/azureuser/lilscript`, branch
  `finer/059-idiom-directed-naming`. S1 is committed and pushed at `343a3af0`.
- C1, C2, C3, S1 and S2 implementation are complete. S1's final library/CLI
  reruns and complete paired compile-cost study remain explicitly deferred.
- S2 adds shared aggregate/field facts, fresh store collection, namespace
  flattening, scalar banks and record-alias normalization. It adds owned
  JavaScript product updates and native final-use transfers, with TOML gates,
  invalidation and receipts. The [acceptance record](s2-acceptance.md) explains
  conservative refusals and the corrected formation dependencies.
- Retained S2 compiler: `/home/azureuser/lilscript-work/bin/s2-complete-2/lilscript`,
  SHA-256 `a1c7c7bc8571e1da983b1178dfb2b1924f73a61e9df0ae64aa6b88dad57bdece`.
  Its [identity](../../benchmarks/migration-results/2026-09-30-s2-complete/identity.json)
  records every Rust/Cargo source hash. Build outputs are not a substitute for
  that immutable pin. No test or measurement job is left running.

## Evidence to retain

[S2 evidence](../../benchmarks/migration-results/2026-09-30-s2-complete/README.md):
22 final S2 checks plus 75 adjacent checks across implementation batches pass;
native cases execute 21 compiler/sanitizer profiles. Fifteen development
programs pass 90 old/new behavior observations at effort 13. Exact totals are
**−59 raw / −47 gzip / −33 Brotli**, with no individual regression. The final
release build and configuration reference checks pass. This is focused
evidence, not full-fleet or timing qualification; the accepted ratchet baseline
is unchanged.

[S1 evidence](../../benchmarks/migration-results/2026-10-01-s1-complete/README.md):
retain `s1-complete-4`, SHA-256
`628028fe56197da6daa2cd09088fc21b34db11e7ff848020ff822b958ef87268`.
All 642 generic programs pass in each objective; totals improve by
593 raw / 340 gzip / 278 Brotli. The 403-case × 18-lane language matrix has
zero unexpected failures; Marked 29/29, Zod 1,353/1,353 and PostHog 21/21 pass.
Its two-byte local Brotli growth is explicitly accepted with smaller totals.
Do not revive the rejected candidate 5 or repeat qualification to recover it.
Native Record debt remains N2. Final library/CLI and complete paired CPU
verification were deferred by the owner, not passed.

## Next implementation work

1. Start **S3 only**, with a finite acceptance checklist: remaining inlining,
   known-call devirtualization, specialization and identical-body sharing.
   Cover cross-module calls, nested closures and tail returns. Use S2's facts;
   make duplication/sharing alternatives available to objective selection.
2. Include the two historical M7.9 formation costs in S3's call transport:
   an inline parameter written through a field still needs a cell, and a
   copied struct argument can hold a waiting operand. Q1 coordinates their
   representation recipes. S2's source ownership proof alone cannot remove
   evaluation/snapshot timing requirements at a call.
3. Then follow the existing order:
   **Q1 → G1 → G2 → G3 → S4 → Q2 → Q3 → Q4 → D1 → D2 → D3 → N1 → N2 → V1 → V2**.
   Finish each milestone before opening another. Target scalar/namespace and
   alias/literal cleanup stays until Q1/S4 can replace its proofs for storage
   introduced after source rules. Do not delete useful coverage early.

## Product rules

Optimize raw, gzip and Brotli independently. Keep default effort 13 focused on
size; expensive marginal strategies may default to 14/15 with explicit flags.
Preserve the best admitted artifact and deterministic effort progression.
TOML permissions control initial, searched and reused output; document their
purpose, defaults/gates and size/compile/runtime tradeoffs. No workload-name
heuristics or blended scores. Stronger mangling and fair Closure ADVANCED wins
remain goals, not claims established by this checkpoint.

Use one heavy job at a time. Implement a coherent batch, then use focused
tests or matched measurements to resolve its concrete risks and effects.
Broaden verification for an observed failure or unresolved concern, not simply
because another small edit landed. Keep evidence limitations explicit.
