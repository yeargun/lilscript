# Failure triage

Parent: [verification](README.md). Release policy: [release gates](release-gates.md).

## Classify before changing code

| Class | Examples | First owner |
|---|---|---|
| Semantic drift | stdout/DOM/API/effect mismatch | parser/semantic/lowering/optimizer/codegen |
| Missing language proof | port cannot state a fact Terser guesses; `assume_*` papering | reusable language/analysis contract, not a fold ([compressor surface](../language/compressor-surface.md)) |
| Glue-TS port | `JsValue` internals, vendored unminified host, `JS.method*` constructor tables | rewrite representation; do not add a library matcher |
| Invalid comparison | unmatched boundary or JS semantics | case author/harness |
| Baseline failure | tool crash, invalid output, unsupported target | tool adapter |
| Raw regression | emitted tokens/layout/retained code | optimizer/codegen/port |
| Gzip/Brotli regression | repetition/order/context changed | candidate search/layout/codec analysis |
| Config contract | allowlist/hard-off/mode ignored | config/compiler dispatch |
| Bundle accounting | missing/lazy chunk or wrong request cost | module/chunk planner/harness |
| Nondeterminism | output hash or result changes | stable ordering/tool pinning |
| Runtime regression | statistical non-inferiority gate | representation/performance model |

## Workflow

1. Preserve the exact failing artifacts, report, versions, config, seed, and command.
2. Re-run only the case twice to separate deterministic failure from infrastructure.
3. Check source equivalence and boundary before blaming the compiler.
4. Minimize semantics and size independently. A semantic minimizer may erase the
   codec context; a codec minimizer must retain the metric regression.
5. Run optimization/config ablations to identify the first drifting decision.
6. Inspect raw diff, symbol/function counts, retained imports, candidate explain data,
   and compressed sizes. Do not infer codec cause from raw diff alone.
7. Fix compiler or case, add the minimized regression permanently, then run family and
   full gates.

## Outcomes

- **Compiler bug:** fix it; retain a semantic and size regression when both mattered.
- **Bad LilScript port:** rewrite idiomatically without weakening the external
  contract; preserve before/after attribution.
- **Dishonest/incorrect JS pair:** correct the pair and invalidate old measurements.
- **Intentional trade:** only non-size-first policy may accept it, and the exact
  config/metric trade stays in the report.
- **Search budget miss:** retain a small exhaustive oracle case and document which
  production budget truncated the winning path.

## Quarantine

Quarantine is exceptional. It requires issue, owner, reason, first-seen compiler/tool
version, exact failing metrics, expiry, and whether it blocks public claims. A
quarantined case remains executed and visible; it is never deleted from totals or
silently changed from strict to parity.
