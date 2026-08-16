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
  → optimize_and_select_javascript  # IR variants + emission search (single)
     or one optimize + plan_javascript_chunks (split / preserve-modules)
  → optional js_peephole
  → native: clone IR, optimize with native options, emit C / exec
```

Legacy `compile_source` → `compile_to_js` (`src/codegen_js.rs`) is AST-direct. Configured production uses IR codegen (`src/codegen_ir_js.rs`) only.

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
| `split` / `preserve-modules` | `compile_path_to_js_bundle_configured` | One `optimize_control_flow_with_guidance(..., preserve_exports: true)`; chunks scored by deploy cost; **no** per-chunk emission beam (unless joint chunk/symbol search is on) |
| `all` + `split` / `preserve-modules` | `compile_path_all_to_js_bundle_configured` | The same one bundle search plus native optimization from one shared lowered IR |
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

`JavaScriptSelectionMetrics`: codec name, transfer bytes, startup score, syntax vs baseline, performance vs baseline, candidates evaluated, peephole rewrites, compiler microseconds. Optimizer `OptimizationReport` lists pass changed/unchanged.

## Verification posture

`scripts/verify-matrix.sh` compiles many programs `--target all` and requires JS, native exec, independently compiled C, and expected stdout to match. `lilscript-differential` is an AST evaluator that does **not** go through SSA — an oracle for shared transforms. See [`docs/differential-testing.md`](../../differential-testing.md).
