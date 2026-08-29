# Pipeline

Parent: [Compilation](README.md). Files: `src/compiler.rs`, `src/module.rs`, `src/main.rs`.

## Path compilation (production)

```
lilscript.toml
  → discover_modules_configured
  → parse_modules
  → link_modules                    # private names, lazy init-free check
  → semantic::analyze
  → lower_to_control_flow
  → single: optimize_and_select_javascript
       → IR variants + emission search + optional parsed-JS finalization
    or split: one optimize + plan_javascript_chunks + direct chunk emit
    or preserve-modules: one optimize + fixed source partitions + direct emit
  → native: clone IR, optimize with native options, emit C / exec
```

`compile_source` / `compile_to_js` in `src/codegen_js.rs` are convenience facades
that analyze, lower, optimize, and use IR codegen. Production JavaScript emission
lives in `src/codegen_ir_js.rs`.

For `--target all` with `split` or `preserve-modules`,
`compile_path_all_to_js_bundle_configured` performs discovery, parsing, linking,
semantic analysis, and lowering once. It clones that lowered IR for native
optimization, runs one bundle JavaScript pipeline on the original, and returns
the bundle plus C in `BundledCompilationArtifacts`. The CLI does not compile a
discarded single JavaScript artifact before invoking the bundle pipeline.

## Single artifact vs bundle

| Path | Function | Search |
|---|---|---|
| `js` / `js-module` + `bundle.mode = single` | `optimize_and_select_javascript` | Full two-level candidate search |
| `split` | `compile_path_to_js_bundle_configured` | One `optimize_control_flow_with_guidance(..., preserve_exports: true)`; chunk plans scored by deploy cost; **no** per-chunk identifier-alphabet beam. `joint-chunk-symbol-search` adds `function_layout` + `local_name_reserve` only |
| `preserve-modules` | `compile_path_to_js_bundle_configured` | One whole-program optimization, then fixed source-module partitions with configured `js_options()`; no chunk-plan scorer |
| `all` + `split` / `preserve-modules` | `compile_path_all_to_js_bundle_configured` | The corresponding bundle path plus native optimization from one shared lowered IR |
| `--delegate-bundling` (Lilpack) | forces `bundle.mode = single` | Vite chunks the mixed graph after LilScript ESM |

## CLI overrides (`src/main.rs`)

| Flag | Effect |
|---|---|
| `--config` | Explicit TOML |
| `-j N` / `--jobs N` | Override `compiler.resources.threads` for configured JavaScript compilation |
| `--codec-jobs N` | Override the terminal Brotli finalizer worker count |
| `--mode development` | `candidate_search = off` (no multi-IR/emission expansion; configured finalization features may remain) |
| `--mode production` | project policy |
| `--delegate-bundling` | `bundle.mode = single` |
| `--explain human\|json` | selection metrics; requires single bundle |
| `--profile-template` | write PGO key file |
| `--write-lock` | rewrite `lilscript.lock` only |
| `--target js \| js-module \| c \| native \| all` | backend |

## Explain metrics

`JavaScriptSelectionMetrics`: contract/objective fingerprints, codec name,
transfer bytes, startup score, syntax and performance versus baseline,
candidates/plans, pre-budget optimizer and bounded structural emissions,
proposal/terminal work, family reserves/starvation, peephole rewrites, stop
reason, and compiler microseconds. Complete replayable recipe serialization is
planned, not implemented.
Optimizer `OptimizationReport` lists pass changed/unchanged.

## Verification posture

`scripts/verify-matrix.sh` compiles many programs `--target all` and requires JS, native exec, independently compiled C, and expected stdout to match. `lilscript-differential` is an AST evaluator that does **not** go through SSA — an oracle for shared transforms. See [`docs/differential-testing.md`](../../differential-testing.md).
