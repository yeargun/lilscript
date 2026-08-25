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

If `compression` is omitted, `javascript.priority` supplies the list. Listing a
name opts that representation in even if the profile would leave it off.
`compression = []` disables all contested tactics. Canonical options follow the
listed names when the table is present. Size-first search-only spellings such as
`indexed-char-at` still compete unless the list is empty.

If `optimizations` is omitted, `optimization_level` (0–15) supplies the feature
set. If present, it is an exact feature allowlist; the level still bounds count,
byte, beam, structural-proposal, and terminal-codec effort.

`javascript.ecmascript` and `javascript.browsers` are a third axis: they choose
which JavaScript syntax is legal. They do not enable compression tactics or
searches. Default `es2022` matches the historical backend.

Search may turn **off** an enabled compression tactic to compare. Size-first
search-only spellings still compete from the priority matrix when a non-empty
`compression` list omitted the new name.

## Mental model

```
[optimization]          → what IR rewrites are allowed (false is a hard off)
javascript.priority     → default compression set + inline budgets + rank key
javascript.compression  → overlay / opt-in names (optional; `[]` disables)
javascript.optimizations / optimization_level → search dimensions
javascript.cost_model   → what “smaller” means (raw | gzip | brotli)
candidate_*             → compile-time budget for measuring “smaller”
[mangle]                → highest-precedence name/pool overrides
[bundle]                → after optimize: one file vs scored chunks
[profile]               → optional hotness for specialization
[native]                → C storage placement only
[lint] / [format]       → author constraints (eager host, allocations); not codegen
```
