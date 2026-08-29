# Compression verification

Parent: [knowledge tree](../README.md). Execution order:
[planned migration](../migration/planned-migration.md). Evidence:
[evidence](../evidence/README.md). Current gates:
[`docs/current-status.md`](../../current-status.md).

This folder defines what a LilScript compression result must mean. It is deliberately
stricter than “both snippets print the same line and one file is shorter.” Web code
has public APIs, host effects, modules, lazy artifacts, codec-specific sizes, runtime
contracts, and tool-specific eligibility constraints. Ranking math:
[objectives](../compilation/objectives.md). Glue-TS vs missing proof:
[compressor surface](../language/compressor-surface.md).

## Documents

### Contract

- [Paired-case contract](paired-case-contract.md) — equivalence, fairness, and gate
  semantics
- [Case layout](case-layout.md) — folder contents and metadata
- [Baseline toolchains](baseline-toolchains.md) — Terser, Oxc, esbuild, Vite, Closure
- [Codec measurement](codec-measurement.md) — exact raw/gzip/Brotli rules

### Coverage

- [Coverage matrix](coverage-matrix.md) — language/compiler feature ownership
- [Algorithm challenges](algorithm-challenges.md) — escalating whole-program pairs
- [Config matrix](config-matrix.md) — TOML and CLI behavior ownership
- [Browser/host cases](browser-host-cases.md) — DOM/API/delivery oracles

### Process

- [Failure triage](failure-triage.md) — minimize, classify, fix, retain
- [Release gates](release-gates.md) — promotion and evidence policy

## Non-negotiable distinction

Three artifacts can all be useful but cannot be substituted for one another:

1. LilScript compiler output, measured as emitted;
2. independently authored JavaScript processed by an eligible baseline tool;
3. LilScript output post-processed by a JavaScript minifier.

Only 1 vs 2 proves the language+compiler compression claim. Artifact 3 is an
ablation/deployment experiment and must be labelled as such.

A fourth useful lane is the downstream deployment artifact (for example a
banner, facade, Vite bundle, or Oxc pass). It proves that joint pipeline only;
it must not be relabelled as direct compiler output.

## Current known limits

- The micro runner gates separately compiled raw/gzip/Brotli objective
  artifacts against matching metric-specific valid Terser/Oxc/esbuild minima and is
  release-wired. Durable reviewed cases live in
  [`comparison/cases/canonical/`](../../../comparison/cases/canonical/). The
  generated catalog remains a parameterized regression net with a checked-in oracle
  digest. See the [planned migration](../migration/planned-migration.md).
- The micro catalog is not structural whole-program evidence. The separate
  [algorithm lane](algorithm-challenges.md) owns multi-function/module interaction,
  runtime vectors, and codec-window scaling.
- Single-artifact candidate search is not automatically equivalent to split or
  preserve-module selection. Current split/preserve paths optimize once before chunk
  planning. Joint chunk/symbol search preserves its winning emission options in final
  chunk output, but its proposal set is narrower than the full single-artifact beam;
  retain regression cases for both selection and final emission.
- Split partition search is a bounded greedy frontier, so its result is “best found
  under the planner budget,” not a proof of the globally minimal partition.
- “Global optimum” is provable only for a small exhaustively enumerated candidate
  space. Production beam results are best-found under a budget.

## Validation Layers

1. Typed legality proves that a transform preserves language constraints,
   effects, identity, ownership, and explicit obligations.
2. Final-byte validation checks syntax floor, binding resolution, property
   category, module links, observed ABI, and obligation witnesses before scoring.
3. Independent behavior/API suites test semantic equivalence for the declared
   boundary. Parsing bytes cannot prove general program equivalence.
4. Exact selected-codec measurement ranks only candidates that passed the
   applicable earlier layers.
