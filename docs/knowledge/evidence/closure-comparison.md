# Closure and corpus

Parent: [Evidence](README.md). Mapping: [`docs/optimization-coverage.md`](../../optimization-coverage.md). Audit: [`docs/vite-closure-minification-audit.md`](../../vite-closure-minification-audit.md).

## What LilScript claims relative to Closure `ADVANCED`

LilScript does **not** run Closure and does not translate `.lil` to annotated JavaScript before optimizing. It maps Closure *responsibilities* (inline, collapse properties, DCE, rename, modules) onto typed SSA + codec search.

Closure-only input processing (JSDoc, `goog.*`, CommonJS, Angular, Polymer, J2CL, extern collection as source syntax) is **out of scope**. Host interaction is `extern`, not JSDoc.

## Maintained Closure lane

`comparison/` contains seven small paired apps with matching-stdout and size gates:
aggregate-model, control-flow-engine, higher-order-pipeline, module-graph,
numerical-kernel, optimizer-pressure, and text-report. The current schema compiles
LilScript separately for raw, gzip-9, and Brotli-11, then gates each objective
artifact only on its selected metric against Closure `ADVANCED`. The default/native
build is behavior evidence, not a substitute for those three JS artifacts. Generated
`comparison/summary.json` owns pass counts and bytes after a full rebuild; this page
does not preserve approximate live totals.

`docs/benchmark-results.md` — core `benchmarks/` corpus, similar story, **no jQuery**.

These programs are typed, closed-world, and written in LilScript. They show the **language+compiler** stack working. They do not show that an arbitrary existing JS library, ported with `JsValue`, will win.

## Where post-hoc minification loses

A historical Vite/Oxc/Closure audit found that running a JS minifier on **already
codec-scored** LilScript output could shorten raw and **worsen Brotli**. A
comma-conditional rewrite on a large surface also displaced a better bounded-beam
candidate. These are retained search lessons, not current byte totals: extra local
passes after selection are proposals that must be remeasured with the canonical
scorer.

## Completion rule (roadmap)

A capability is complete only with explicit semantics, optimized vs disabled agreement, portable backend agreement, differential tests, a checked-in ablation (win or recorded neutral), and claims that name corpus/codec/scope.

jQuery is the reminder that the synthetic gate is necessary but not sufficient for the “world’s most compression-friendly” mission.
