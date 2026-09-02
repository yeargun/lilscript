# Config

Compiler policy lives in `lilscript.toml`. Unknown keys and invalid numeric limits are errors (`deny_unknown_fields`). The schema dump with examples is [`docs/configuration.md`](../../configuration.md). This folder explains **layers, precedence, and how each knob changes compilation**.

Which behaviors are actually searched vs hardcoded is the
[decision registry](../compilation/decision-registry.md), not only this folder.
How `priority` and `cost_model` combine: [objectives](../compilation/objectives.md).
Root `lilscript.toml` is a **subset** of size-first tactics; omitting a name
disables that canonical representation even when `priority = "size-first"`.

Parent: [tree](../README.md). Behavior: [compilation](../compilation/README.md).

## Pages

### Layers

- [Discovery and precedence](discovery-precedence.md)
- [package / dependencies / lockfiles](package-dependencies.md)
- [optimization](optimization.md) — semantic IR passes (all backends)

### JavaScript policy

- [`javascript.priority`](javascript-priority.md)
- [`javascript.compression`](compression-decisions.md) — which representations are legal
- [`javascript.optimizations`](javascript-optimizations.md) — which searches run
- [Cost model and search budgets](cost-model.md)
- [Source maps](../../source-maps.md) — final-artifact provenance and publication modes
- [Analysis maps](../../analysis-maps.md) — selected mangling rules and evidence

### ABI and shape

- [mangle](mangle.md)
- [JavaScript shape and ABI](javascript-shape-abi.md)
- [Startup and performance](startup-performance.md)

### After optimize

- [bundle](bundle.md)
- [profile](profile.md)
- [native](native.md)
- [lint / format](lint-format.md)

### Matrices

- [Tradeoff matrix](tradeoffs.md)
- [Behavior matrix](behavior-matrix.md)
- [Build profiles](build-profiles.md)

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
`compression` list omitted the new name. Most other omitted names stay off;
`length-to-number-elision` is registry-gated and remains off when omitted.

## Mental model

```
[optimization]          → what IR rewrites are allowed (false is a hard off)
javascript.priority     → default compression set + inline budgets + rank key
javascript.compression  → overlay / opt-in names (optional; `[]` disables)
javascript.optimizations / optimization_level → search dimensions
javascript.cost_model   → what “smaller” means (raw | gzip | brotli)
javascript.source_map   → optional external debug artifact; never a search input
javascript.analysis_map → optional mangling-decision sidecar; never a search input
candidate_*             → compile-time budget for measuring “smaller”
[mangle]                → highest-precedence name/pool overrides
[bundle]                → artifact layout; does not by itself define public world
[profile]               → optional hotness for specialization
[native]                → C storage placement only
[lint] / [format]       → author constraints (eager host, allocations); not codegen
```
