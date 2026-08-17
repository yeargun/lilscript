# Phase 00 — freeze current evidence

Parent: [migration](README.md). Contracts: [case layout](../verification/case-layout.md),
[codec measurement](../verification/codec-measurement.md).

## Objective

Make today’s comparison lanes reproducible before changing their rules. A later win
must be attributable to compiler/source changes rather than tool drift, newline
normalization, a different boundary, or a regenerated expectation.

## Existing assets to preserve

| Lane | Current role |
|---|---|
| `comparison/cases/` | Catalog-generated LilScript vs Terser/Oxc/esbuild micro cases; generated report owns the current count |
| `comparison/apps/` | Seven maintained closed-world apps vs pinned Closure ADVANCED |
| `comparison/artifacts/` | Re-runnable emitted JS/C/native snapshots |
| `benchmarks/paired/` | Mechanically paired transfer/parse/runtime workloads |
| `benchmarks/browser/` | Headless Chromium runtime non-inferiority gate |
| `lilscript-differential` | Independent checked-AST semantic oracle |
| `benchmarks/popular/` | Public-package and port pressure tests, including jQuery |

## Work

- Record Node, LilScript, baseline tool, and Closure versions next to every report.
- Record the complete LilScript config or a content hash plus all CLI mode/target
  arguments.
- Preserve emitted bytes; never trim, normalize final newlines, or reconstruct code
  before measurement.
- Separate checked-in evidence from ignored scratch outputs.
- Add a report schema version before extending fields.
- Run the current commands once unchanged and archive failures as baseline blockers.
- Reconcile prose that cites old `benchmarks/popular/RESULTS.md` rows with the current
  JSON producer; JSON and Markdown must come from one run.

## Exit criteria

- A clean checkout can reproduce each maintained summary with documented install and
  run commands.
- Report provenance identifies tool versions, target, boundary, config, and codecs.
- Re-running with no source changes produces byte-identical artifacts and result JSON
  except explicitly volatile timing/timestamp fields.
- No migration phase relies on an unlabelled scratch `.raw.js` file as evidence.

This phase does not tighten gates. That happens in [phase 01](01-paired-runner-contract.md).
