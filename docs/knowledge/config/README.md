# Config

Compiler policy lives in `lilscript.toml`. Unknown keys and invalid numeric limits are errors (`deny_unknown_fields`). The schema dump with examples is [`docs/configuration.md`](../../configuration.md). This folder explains **layers, precedence, and how each knob changes compilation**.

Parent: [tree](../README.md). Behavior: [compilation](../compilation/README.md).

## Pages

- [Discovery and precedence](discovery-precedence.md)
- [`[package]`, dependencies, and lockfiles](package-dependencies.md)
- [`[optimization]`](optimization.md) — semantic IR passes (all backends)
- [`javascript.priority`](javascript-priority.md) — size vs runtime ranking + inline policy
- [`javascript.compression`](compression-decisions.md) — which representations are **legal**
- [`javascript.optimizations` and levels](javascript-optimizations.md) — which **searches** run
- [Cost model and search budgets](cost-model.md)
- [`[mangle]`](mangle.md)
- [JavaScript shape and ABI](javascript-shape-abi.md)
- [Startup and performance](startup-performance.md)
- [`[bundle]`](bundle.md)
- [`[profile]`](profile.md)
- [`[native]`](native.md)
- [`[lint]` / `[format]`](lint-format.md)
- [Tradeoff matrix](tradeoffs.md)
- [Behavior matrix](behavior-matrix.md) — which layer affects ABI, JS, native, or
  compile work
- [Build profiles](build-profiles.md) — development, oracle, release, maximum,
  reusable/app, PGO recipes

## Two allowlists people confuse

| Key | Question it answers |
|---|---|
| `javascript.compression` | May this representation exist at all? (mangling, packing, outlining, …) |
| `javascript.optimizations` | Which alternative **searches** and post-emit analyses run? |

If `compression` is omitted, `javascript.priority` supplies the list. If present, **only listed names** are on; `compression = []` disables all contested tactics.

If `optimizations` is omitted, `optimization_level` (0–15) supplies the feature set. If present, it is an exact allowlist and the level no longer lowers the candidate cap.

Search may turn **off** an enabled compression tactic to compare. It never turns **on** a tactic missing from the compression allowlist.

## Mental model

```
[optimization]          → what IR rewrites are allowed (false is a hard off)
javascript.priority     → default compression set + inline budgets + rank key
javascript.compression  → exact representation allowlist (optional)
javascript.optimizations / optimization_level → search dimensions
javascript.cost_model   → what “smaller” means (raw | gzip | brotli)
candidate_*             → compile-time budget for measuring “smaller”
[mangle]                → highest-precedence name/pool overrides
[bundle]                → after optimize: one file vs scored chunks
[profile]               → optional hotness for specialization
[native]                → C storage placement only
[lint] / [format]       → author constraints (eager host, allocations); not codegen
```
