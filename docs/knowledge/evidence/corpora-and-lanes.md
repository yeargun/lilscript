# Corpora and evidence lanes

Parent: [evidence](README.md). Gate policy:
[release gates](../verification/release-gates.md).

No one corpus carries the mission. Each lane owns a boundary and failure class.

| Lane | What it can establish | What it cannot establish alone |
|---|---|---|
| `comparison/cases/` | per-feature paired stdout + raw/gzip/Brotli vs Terser/Oxc/esbuild | browser/API/module/large-system behavior |
| `comparison/algorithms/` | 11 structural pairs/42 vectors, exact host traces, Closure-inclusive JS frontier, matching objective gates | browser/public-library ABI or arbitrary-program superiority |
| `comparison/apps/` | seven closed-world apps vs pinned Closure ADVANCED | reusable public libraries or arbitrary programs |
| `comparison/benchmarks/` | focused language-family Closure comparisons | broad corpus coverage |
| `lilscript-differential` | checked-AST vs compiled semantic drift | competitor size or browser ABI |
| `scripts/verify-matrix.sh` | JS/C/native portability | JS-only features and web delivery |
| `benchmarks/browser/` | pinned browser runtime non-inferiority | semantic breadth or library API completeness |
| `benchmarks/paired/` | paired transfer/parse/runtime workloads | all language features |
| apps/libraries/scenarios | application and public-surface delivery | universal per-feature proof |
| `benchmarks/popular/` | version-pinned real packages/ports and public pressure | eligibility unless exact surface/performance gates pass |
| jQuery port | large host/dynamic/public-facade convergence pressure | a win while its generated row is ineligible/red |

Generated reports are source-of-truth for numbers. A lane is eligible only when its
behavior oracle, public boundary, target, config, baseline versions/options, and
artifact set match. Totals are descriptive and never compensate for a required
per-case loss.

Cross-lane promotion is deliberate: a transform begins with a semantic micro case,
adds a size ablation, reaches module/browser/runtime cases when relevant, and only
then supports an app/library claim.
