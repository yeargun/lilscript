# phase2-03 — normalize compilation policy

Parent: [ledger](../LEDGER.md). Status: active.

## Question

Can lower optimizer and emission layers consume one normalized compilation
contract and execution plan instead of reinterpreting raw project config?

## Current hypothesis

Typed IR already receives `OptimizationOptions`, emission receives `IrJsOptions`,
and final admission owns legality. Remaining raw-policy reads can be inventoried
and removed incrementally without changing candidate reachability.

## Constraints specific to this task

Do not alter defaults, ABI, unsafe assumptions, objective selection, or candidate
budgets. Each batch must be byte-identical under focused fixtures.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Predecessor acceptance consolidation | focused canonical/search-off/terminal tests and `cargo check --lib` | shared admission, scoring, ordering, and registry recipe batches pass | gate |

## Log

- 2026-08-29 — Began raw-policy read inventory below `optimize_and_select_javascript_inner`. — **OPEN**

## Next step

Identify one duplicated raw `ProjectConfig` policy read in candidate preparation
and replace it with the normalized contract or options value already in scope.
