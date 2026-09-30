# Migration handoff — S1 checkpoint, S2 active

The owner requested committing and pushing S1, then finishing S2. Focused
checks after substantial implementation batches are authorized where they
answer correctness or size questions. Do not resume the interrupted S1
qualification chain or repeatedly run the entire library suite.

## Current state

- Checkout: `/home/azureuser/lilscript`, branch
  `finer/059-idiom-directed-naming`; pre-S1 HEAD was `9793853b`.
- C1, C2, C3 and S1 implementation are complete. The S1 checkpoint retains
  deferred final verification; do not label that verification passed. S2 is
  the sole active implementation milestone.
- S1 implements bounded finite/path primitive facts, constant execution,
  source forwarding, common computations, checked default transport,
  dead-work cleanup, shared primitive/range facts and a native nullable
  constant boxing repair. The three old target default passes are deleted.
- Target alias/root-literal cleanup remains intentionally: it handles storage
  created after source rules. Q1 supplies representation storage contracts;
  S4 owns its eventual replacement. Aggregate store collection belongs to S2.
  These dependency corrections are in `s1-acceptance.md` and `coverage.md`.
- Retain candidate **s1-complete-4**, compiler SHA-256
  `628028fe56197da6daa2cd09088fc21b34db11e7ff848020ff822b958ef87268`.
  Every Rust source and Cargo file matches its recorded identity. Do not switch
  to candidate 5: it recovers two local Brotli bytes but loses 92 raw, 133 gzip
  and 46 Brotli bytes overall. The checkout source is candidate 4; binaries in
  `target/release` are not the authoritative qualification pin.

## Evidence already sufficient to retain

Reports: `benchmarks/migration-results/2026-10-01-s1-complete/`.

- Generic corpus: 642 programs × three objectives, no behavior failures.
  Totals change by **−593 raw / −340 gzip / −278 Brotli**. The sole growing
  corpus subtotal is applications Brotli +2, an equivalent naming change.
  The supported ratchet override records the exact tradeoff and requires
  nonincreasing totals independently for all three objectives. Its default
  gate stays strict. `tests/ratchet/baseline.json` already records candidate 4.
- Language matrix: 403 cases × 18 lanes; 5,466 pass, 180 ledgered failures,
  1,608 masked, **zero unexpected failures**. Existing native Record debt is
  retained under N2; the previous compiler reproduces that refusal.
- Reference ports: Marked **29/29**, Zod **1,353/1,353**, PostHog **21/21**.
  Zod's valid result is in `ports-patch-attempt.json`; the other two are in
  `ports.json`. The first invocation tried to reapply already-landed patches
  to Marked/PostHog and failed before compiling them; their corrected run uses
  `--patches none`. Zod has no migration patch and was not needlessly rerun.
- Runner tests: 20 pass. Generated configuration reference and whitespace
  checks pass. Focused compiler tests passed in coherent implementation batches.
- **Deferred:** final full library/CLI run, completed nine-cell paired compile
  costs, and the retained semantic demonstration run. The interrupted cost
  run completed only Marked raw/gzip before termination; no complete cost
  verdict exists. Its initial policy rejection was only changed tactic
  documentation, not changed effort, permissions, schedule or resource limits.

## Next implementation work

1. The S1 checkpoint retains implementation and existing evidence, with
   deferred verification explicit. Do not restart qualification or tune
   another variant to recover the two-byte local loss.
2. Make **S2 the sole active implementation milestone**. Finish its entire
   finite contract: allocation escape/alias/uniqueness facts; read/write and
   observability facts for fields; dead/constant/overwritten fields; private
   namespace/object flattening; scalar replacement through branches and loops;
   value-struct move/copy elision in both supported targets. Preserve aliases,
   captures, initialization/TDZ, host reflection and value-copy semantics.
   Build on existing record/product families instead of introducing another
   optimizer. Keep representation alternatives eligible for exact objective
   judging; wire controls, receipts and documentation with the implementation.
3. Only after S2 implementation is complete, finish **S3**: remaining inlining,
   known-call devirtualization, specialization and identical-body sharing.
4. Continue the existing dependency order:
   **Q1 → G1 → G2 → G3 → S4 → Q2 → Q3 → Q4 → D1 → D2 → D3 → N1 → N2 → V1 → V2**.
   Each milestone needs a bounded implementation checklist; finish it before
   opening the next. Do not repeatedly return to already-established evidence.

## Product rules that remain in force

Optimize raw, gzip and Brotli independently. Keep effort 13 size-focused;
expensive marginal strategies may default to 14/15. Preserve the best admitted
artifact and deterministic effort progression. TOML permissions must control
initial, searched and reused output, with documented purpose and size/compile/
runtime tradeoffs. No workload-name heuristics and no blended objective score.
The ultimate target includes stronger mangling and beating Closure ADVANCED on
fairly matched workloads; current application wins do not close the remaining
algorithm/fleet gaps. Use focused tests and matched measurements after substantial changes when
they resolve a concrete question. Do not repeat the full library suite.
