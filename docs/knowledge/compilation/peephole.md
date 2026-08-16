# Parsed peephole

Parent: [Compilation](README.md). Source: `src/js_peephole.rs`. Feature: `parsed-peephole` (minimum `optimization_level` 9).

## Why it exists

IR emission can still leave legal JS that a **parsed** rewrite can contract. The peephole Pratt-parses eligible expressions and validates the complete artifact. It does **not** do unparsed text substitution (that would fight the codec and risk syntax).

Rewrites compete as extra candidates against the untouched emission under the selected codec. A local raw win that loses Brotli is discarded.

## Allowed rewrites

- `x = x op y` → compound assignment when the local is simple
- remove unreferenced function-scoped bindings
- fuse adjacent same-kind declarations
- fold two-return arrow guards to conditionals
- fold expression-only `if`/`else` sequences to conditionals (never forced)
- rotate `flag = true; while (flag) { ...; flag = cond }` only when the flag is synthetic and there is no `continue`
- reuse a dead `var` binding; **refuses** to reuse a binding captured by an escaped closure (SolidLil disposer regression)

## Startup guard (`startup-cost-guard`, min level 1)

Compares syntax-derived parse / engine-compile / memory estimates to the configured baseline. Overhead percents are **hard rejects**. Weights break equal-transfer ties. Optional `javascript.startup.max_nesting` is an absolute ceiling even if the guard feature is not in an exact allowlist; `0` is invalid.

This is a deterministic proxy, not a browser measurement. Browser parse/compile remains a roadmap item.

## Performance shape (`performance-shape-model`, min level 3)

Static weights: deopt-sensitive control flow, allocations, unresolved indirect calls, hot-code (optional PGO). Combined with transfer according to `javascript.priority`. Not a runtime benchmark.
